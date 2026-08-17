use physis_core::embed::VectorEmbed as _;

/// Load an ONNX session, logging the actual `ort` error on failure instead of
/// swallowing it. A returned `None` plus a `warn!` line tells the operator
/// *why* a model (e.g. Jina `model_q4f16.onnx`) didn't load. PREMORTEM.md P4.
#[cfg(feature = "embed-onnx")]
pub(crate) fn load_onnx_session(model_path: &std::path::Path) -> Option<ort::session::Session> {
    use ort::session::builder::GraphOptimizationLevel;
    // Intra-op threads = all cores: matmul parallelism within ONE inference. The
    // embedder serializes runs behind a Mutex (one embed at a time), so per-run
    // threading is the only core-level parallelism available.
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
    let build = |opt: GraphOptimizationLevel| -> ort::Result<ort::session::Session> {
        ort::session::Session::builder()?
            .with_optimization_level(opt)?
            .with_intra_threads(threads)?
            .commit_from_file(model_path)
    };
    // Try FULL optimization first — it enables fused, multi-threaded kernels
    // (big speedup for BERT models like BGE/MiniLM). Only Jina CLIP v2's exported
    // graph trips ort rc.12's SimplifiedLayerNormFusion pass (GetIndexFromName
    // panic), so fall back to Disable just for that. PREMORTEM.md P4.
    match build(GraphOptimizationLevel::Level3) {
        Ok(s) => Some(s),
        Err(e_opt) => {
            log::warn!("optimized load of {} failed ({e_opt}); retrying with optimization disabled", model_path.display());
            match build(GraphOptimizationLevel::Disable) {
                Ok(s) => Some(s),
                Err(e) => {
                    log::warn!("failed to load ONNX model {}: {e}", model_path.display());
                    None
                }
            }
        }
    }
}

/// Trait for embedding text and images into fixed-dimension vectors.
///
/// Implementors must guarantee that the same input always produces the
/// same vector (determinism). The vector must be L2-normalized (unit length).
pub trait VectorEmbed: Send + Sync {
    /// Embed a single text string into a fixed-dimension vector.
    fn embed(&self, text: &str) -> Vec<f32>;
    /// Embed an image (raw bytes, e.g. JPEG/PNG) into a fixed-dimension vector.
    /// Returns None if this embedder doesn't support images.
    fn embed_image(&self, _bytes: &[u8]) -> Option<Vec<f32>> {
        None
    }
    /// Embed multiple texts in batch. Default impl calls `embed` for each.
    fn embed_batch(&self, texts: &[&str]) -> Vec<Vec<f32>> {
        texts.iter().map(|t| self.embed(t)).collect()
    }
    /// Return the dimension of the embedding vectors.
    fn dimension(&self) -> usize;
}

/// Semantic self-test (PREMORTEM P0/P4): a model can load yet emit garbage
/// (fp16 outputs read as f32, opset drift, truncated dims). A related pair
/// must clearly out-score an unrelated pair, all vectors finite. Every
/// cascade that selects an embedder must gate candidates with this — the
/// CLI shipped without it and silently ran on Jina noise.
pub fn semantic_self_test(e: &dyn VectorEmbed) -> bool {
    let a = e.embed("a photograph of a dog");
    let related = e.embed("a photograph of a puppy");
    let unrelated = e.embed("quarterly corporate tax accounting spreadsheet");
    let finite = |v: &[f32]| !v.is_empty() && v.iter().all(|x| x.is_finite());
    if !(finite(&a) && finite(&related) && finite(&unrelated)) {
        return false;
    }
    crate::models::cosine_sim(&a, &related) > crate::models::cosine_sim(&a, &unrelated) + 0.05
}

/// Persistent cache of embedder self-test verdicts.
///
/// Loading an ONNX model is eager (the session is built in the embedder
/// constructor), so a cascade that self-tests candidates pays a full model
/// read on every run — even for multi-GB exports that always fail the test
/// (bge-m3 is 2.2 GB). This cache records pass/fail keyed by the model file
/// path + mtime, letting later runs skip building a known-bad model entirely.
/// Replacing a model changes its mtime, so a fixed export is re-tested.
#[derive(Default)]
pub struct SelfTestCache {
    path: std::path::PathBuf,
    map: std::collections::HashMap<String, bool>,
    dirty: bool,
}

impl SelfTestCache {
    pub fn load(data_dir: &std::path::Path) -> Self {
        let path = data_dir.join("embedder_selftest.json");
        let map = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        Self { path, map, dirty: false }
    }

    /// Key by the resolved model file (flat `model.onnx`, else `onnx/model.onnx`,
    /// matching OnnxEmbedder) plus its mtime, so a changed model re-tests.
    fn key(&self, label: &str, model_dir: &str) -> String {
        let flat = std::path::Path::new(model_dir).join("model.onnx");
        let nested = std::path::Path::new(model_dir).join("onnx/model.onnx");
        let file = if flat.exists() { flat } else { nested };
        let mtime = std::fs::metadata(&file)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        format!("{label}:{}:{mtime}", file.display())
    }

    /// Build + self-test an embedder, but skip the (eager, possibly multi-GB)
    /// build entirely when a prior run recorded this model as failing.
    pub fn gated<F>(&mut self, label: &str, model_dir: &str, build: F) -> Option<Box<dyn VectorEmbed>>
    where
        F: FnOnce() -> Option<Box<dyn VectorEmbed>>,
    {
        let key = self.key(label, model_dir);
        if self.map.get(&key) == Some(&false) {
            return None; // known bad: do not load the model at all
        }
        let e = build()?; // eager model load happens here
        let ok = semantic_self_test(e.as_ref());
        if self.map.insert(key, ok) != Some(ok) {
            self.dirty = true;
        }
        ok.then_some(e)
    }

    pub fn save(&self) {
        if !self.dirty {
            return;
        }
        if let Some(parent) = self.path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(s) = serde_json::to_string_pretty(&self.map) {
            let _ = std::fs::write(&self.path, s);
        }
    }
}

impl<T: VectorEmbed + ?Sized> VectorEmbed for Box<T> {
    fn embed(&self, text: &str) -> Vec<f32> {
        (**self).embed(text)
    }
    fn embed_image(&self, bytes: &[u8]) -> Option<Vec<f32>> {
        (**self).embed_image(bytes)
    }
    fn dimension(&self) -> usize {
        (**self).dimension()
    }
}

/// Deterministic random-projection embedder using feature hashing.
///
/// Maps any text to a fixed-dimension vector (default 384) using locality-sensitive
/// hashing with random projections. The same input always produces the same vector
/// (deterministic — seed is fixed at construction).
///
/// Facade over `physis_core::embed::RandomProjectionEmbedder`, whose algorithm is
/// the canonical implementation; this wrapper only satisfies physis_pro's own
/// `VectorEmbed` trait (which adds `embed_image`).
pub struct RandomProjectionEmbedder {
    inner: physis_core::RandomProjectionEmbedder,
}

impl RandomProjectionEmbedder {
    /// Create a new embedder with the given vector dimension.
    ///
    /// Use `384` for compatibility with MiniLM (upgraded via `embed-onnx` feature).
    pub fn new(dim: usize) -> Self {
        Self { inner: physis_core::RandomProjectionEmbedder::new(dim) }
    }
}

impl VectorEmbed for RandomProjectionEmbedder {
    fn embed(&self, text: &str) -> Vec<f32> {
        self.inner.embed(text)
    }

    fn dimension(&self) -> usize {
        self.inner.dimension()
    }
}

#[cfg(feature = "embed-onnx")]
pub mod onnx;

#[cfg(feature = "embed-onnx")]
pub mod clip;

#[cfg(feature = "embed-onnx")]
pub mod jina;

#[cfg(feature = "embed-onnx")]
pub mod registry;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embed_deterministic() {
        let embedder = RandomProjectionEmbedder::new(64);
        let v1 = embedder.embed("hello world");
        let v2 = embedder.embed("hello world");
        assert_eq!(v1, v2, "same input must produce same vector");
    }

    #[test]
    fn test_embed_different_inputs_differ() {
        let embedder = RandomProjectionEmbedder::new(64);
        let v1 = embedder.embed("hello");
        let v2 = embedder.embed("world");
        assert_ne!(v1, v2, "different inputs must produce different vectors");
    }

    #[test]
    fn test_embed_normalized() {
        let embedder = RandomProjectionEmbedder::new(64);
        let v = embedder.embed("test string");
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-5, "vector must be unit normalized, got {norm}");
    }

    #[test]
    fn test_embed_dimension() {
        let embedder = RandomProjectionEmbedder::new(128);
        let v = embedder.embed("dim test");
        assert_eq!(v.len(), 128, "embedding dimension must match constructor");
    }

    #[test]
    fn test_embed_empty_string() {
        let embedder = RandomProjectionEmbedder::new(64);
        let v = embedder.embed("");
        assert_eq!(v.len(), 64);
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-5, "empty string must produce normalized vector");
    }

    #[test]
    fn test_embed_batch() {
        let embedder = RandomProjectionEmbedder::new(64);
        let texts = &["first", "second", "third"];
        let batch = embedder.embed_batch(texts);
        assert_eq!(batch.len(), 3);
        for (i, text) in texts.iter().enumerate() {
            let single = embedder.embed(text);
            assert_eq!(batch[i], single, "batch[{i}] must match single embed");
        }
    }
}

use crate::config::{EmbedderKindConfig, PhysisConfig};
#[cfg(feature = "embed-onnx")]
use crate::embed::clip::ClipEmbedder;
#[cfg(feature = "embed-onnx")]
use crate::embed::jina::JinaEmbedder;
#[cfg(feature = "embed-onnx")]
use crate::embed::onnx::OnnxEmbedder;

/// Resolve a bundled model directory by name, searching the places it can
/// legitimately live.
///
/// The cascade used to name these as bare relative paths (`models/bge-base…`),
/// which resolve against the *working directory*. That works when running from
/// the repo root and nowhere else: an installed binary invoked from a user's
/// project directory found no model, fell through the cascade, and landed on
/// [`RandomProjectionEmbedder`] — plausible-looking scores over pure noise,
/// with no error. Searching an explicit list makes the installed layout work
/// without breaking the dev one.
///
/// Order, first hit wins:
/// 1. `PHYSIS_MODELS` — explicit override, for unusual deployments.
/// 2. `MODEL_DIR` — conventional service-unit override (the model root).
/// 3. `./models/<name>` — running from the repo, which developers expect.
/// 4. `<data_dir>/models/<name>` — where the installer puts them.
///
/// Falls back to the relative path when nothing exists, so the caller's
/// `is_available()` check reports a missing model exactly as before.
#[cfg(feature = "embed-onnx")]
fn model_dir(config: &PhysisConfig, name: &str) -> String {
    use std::path::PathBuf;

    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(root) = std::env::var_os("PHYSIS_MODELS") {
        candidates.push(PathBuf::from(root).join(name));
    }
    if let Some(root) = std::env::var_os("MODEL_DIR") {
        candidates.push(PathBuf::from(root).join(name));
    }
    candidates.push(PathBuf::from("models").join(name));
    candidates.push(config.data_dir.join("models").join(name));

    candidates
        .iter()
        .find(|p| p.exists())
        .unwrap_or(&candidates[candidates.len() - 2])
        .to_string_lossy()
        .into_owned()
}

/// Pick the best available embedder, self-test gated.
///
/// Extracted so every entry point resolves the embedder the same way. The MCP
/// server previously hard-coded [`RandomProjectionEmbedder`], which meant every
/// tool it served — classification, packing, similarity — answered from random
/// projections rather than semantics, silently and with no error.
///
/// Returns the embedder and a short provenance label for logging.
pub fn select_embedder(config: &PhysisConfig) -> (Box<dyn VectorEmbed>, &'static str) {
        #[cfg(feature = "embed-onnx")]
        {
            // Self-test gate (same as the web server): a candidate that loads
            // but emits non-semantic vectors (e.g. Jina fp16 → garbage) is
            // skipped so the next working candidate wins. Without this gate
            // the CLI silently classified with noise.
            let gate = |opt: Option<(Box<dyn VectorEmbed>, &'static str)>| {
                opt.filter(|(e, _)| crate::embed::semantic_self_test(e.as_ref()))
            };
            let try_primary = || -> Option<(Box<dyn VectorEmbed>, &'static str)> {
                let ec = config.embedders.primary.as_ref()?;
                if !ec.enabled { return None; }
                match ec.kind {
                    EmbedderKindConfig::JinaV2 => {
                        let j = JinaEmbedder::with_model_dir(&ec.model_dir);
                        j.is_available().then(|| (Box::new(j) as Box<dyn VectorEmbed>, "jina-v2"))
                    }
                    EmbedderKindConfig::Clip => {
                        let c = ClipEmbedder::with_config(&config.onnx);
                        c.is_available().then(|| (Box::new(c) as Box<dyn VectorEmbed>, "clip"))
                    }
                    EmbedderKindConfig::MiniLM => {
                        let m = OnnxEmbedder::with_config(&config.onnx);
                        m.is_available().then(|| (Box::new(m) as Box<dyn VectorEmbed>, "minilm"))
                    }
                    EmbedderKindConfig::BgeBase => {
                        let cfg = crate::config::OnnxConfig {
                            enabled: true,
                            model_dir: Some(model_dir(config, "bge-base-en-v1.5")),
                            dim: 768,
                            max_length: 512,
                            pooling: crate::config::PoolingStrategy::Mean,
                        };
                        let b = OnnxEmbedder::with_config(&cfg);
                        b.is_available().then(|| (Box::new(b) as Box<dyn VectorEmbed>, "bge-base"))
                    }
                    EmbedderKindConfig::BgeM3 => {
                        let cfg = crate::config::OnnxConfig {
                            enabled: true,
                            model_dir: Some(model_dir(config, "bge-m3")),
                            dim: 1024,
                            max_length: 512,
                            pooling: crate::config::PoolingStrategy::Cls,
                        };
                        let b = OnnxEmbedder::with_config(&cfg);
                        b.is_available().then(|| (Box::new(b) as Box<dyn VectorEmbed>, "bge-m3"))
                    }
                    EmbedderKindConfig::RandomProjection => {
                        Some((Box::new(RandomProjectionEmbedder::new(config.embed_dim)), "random-projection"))
                    }
                }
            };
            // Cascade: BGE → BGE-M3 → CLIP → MiniLM → RP, every candidate
            // self-test-gated. Jina is deliberately NOT in the auto-fallback:
            // the bundled jina-clip-v2 fp16 export is broken (self-test always
            // fails, ~0.06 top score on real text) yet mmapping it cost a cold
            // 1.7 GB disk read on every scan — minutes stuck in state-D before
            // the gate rejected it. It stays reachable only when explicitly set
            // as `embedders.primary` (try_primary), so a fixed export can opt in.
            // Heavy ONNX fallbacks (bge-base 417 MB, bge-m3 2.2 GB) are routed
            // through a persistent self-test cache: after the first run records
            // them as failing, later scans skip the multi-GB load entirely
            // instead of re-reading and re-rejecting them every time. Sequential
            // `if let` (not a `.or_else` chain) so `stc` is borrowed once at a
            // time. `break 'pick` yields the first embedder that passes.
            let mut stc = crate::embed::SelfTestCache::load(&config.data_dir);
            let chosen: (Box<dyn VectorEmbed>, &'static str) = 'pick: {
                if let Some(e) = gate(try_primary()) {
                    break 'pick e;
                }
                if let Some(e) = stc.gated("bge-base", "models/bge-base-en-v1.5", || {
                    let cfg = crate::config::OnnxConfig {
                        enabled: true,
                        model_dir: Some(model_dir(config, "bge-base-en-v1.5")),
                        dim: 768,
                        max_length: 512,
                        pooling: crate::config::PoolingStrategy::Mean,
                    };
                    let b = OnnxEmbedder::with_config(&cfg);
                    b.is_available().then(|| Box::new(b) as Box<dyn VectorEmbed>)
                }).map(|e| (e, "bge-base")) {
                    break 'pick e;
                }
                if let Some(e) = stc.gated("bge-m3", "models/bge-m3", || {
                    let cfg = crate::config::OnnxConfig {
                        enabled: true,
                        model_dir: Some(model_dir(config, "bge-m3")),
                        dim: 1024,
                        max_length: 512,
                        pooling: crate::config::PoolingStrategy::Cls,
                    };
                    let b = OnnxEmbedder::with_config(&cfg);
                    b.is_available().then(|| Box::new(b) as Box<dyn VectorEmbed>)
                }).map(|e| (e, "bge-m3")) {
                    break 'pick e;
                }
                if let Some(e) = gate({
                    let c = ClipEmbedder::with_config(&config.onnx);
                    c.is_available().then(|| (Box::new(c) as Box<dyn VectorEmbed>, "clip"))
                }) {
                    break 'pick e;
                }
                if let Some(e) = gate({
                    let m = OnnxEmbedder::with_config(&config.onnx);
                    m.is_available().then(|| (Box::new(m) as Box<dyn VectorEmbed>, "minilm"))
                }) {
                    break 'pick e;
                }
                (Box::new(RandomProjectionEmbedder::new(config.embed_dim)), "random-projection")
            };
            stc.save();
            chosen
        }
        #[cfg(not(feature = "embed-onnx"))]
        (Box::new(RandomProjectionEmbedder::new(config.embed_dim)), "random-projection")
}
