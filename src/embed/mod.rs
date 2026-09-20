#[cfg(feature = "embed-onnx")]
use crate::config::EmbedderKindConfig;
use crate::config::PhysisConfig;
#[cfg(all(feature = "embed-onnx", feature = "image-embed"))]
use crate::embed::clip::ClipEmbedder;
#[cfg(all(feature = "embed-onnx", feature = "image-embed"))]
use crate::embed::jina::JinaEmbedder;
#[cfg(feature = "embed-onnx")]
use crate::embed::onnx::OnnxEmbedder;
use physis_core::embed::VectorEmbed as _;

/// Adapter making a physis_pro [`VectorEmbed`] satisfy `physis_core`'s
/// `VectorEmbed` (which lacks `embed_image`). Public because more than one
/// consumer needs the same bridge: `quality.rs` records embeddings, and the
/// Studio notebook asks `physis_core::rag` for a budgeted answer.
pub struct CoreEmbedAdapter(pub Box<dyn VectorEmbed>);

impl physis_core::embed::VectorEmbed for CoreEmbedAdapter {
    fn embed(&self, text: &str) -> Vec<f32> {
        self.0.embed(text)
    }
    fn embed_batch(&self, texts: &[&str]) -> Vec<Vec<f32>> {
        self.0.embed_batch(texts)
    }
    fn dimension(&self) -> usize {
        self.0.dimension()
    }
}

/// Load an ONNX session, logging the actual `ort` error on failure instead of
/// swallowing it. A returned `None` plus a `warn!` line tells the operator
/// *why* a model (e.g. Jina `model_q4f16.onnx`) didn't load. PREMORTEM.md P4.
#[cfg(feature = "embed-onnx")]
pub(crate) fn load_onnx_session(model_path: &std::path::Path) -> Option<ort::session::Session> {
    use ort::session::builder::GraphOptimizationLevel;
    // Intra-op threads = all cores: matmul parallelism within ONE inference. The
    // embedder serializes runs behind a Mutex (one embed at a time), so per-run
    // threading is the only core-level parallelism available.
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
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
            log::warn!(
                "optimized load of {} failed ({e_opt}); retrying with optimization disabled",
                model_path.display()
            );
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
    /// Whether this embedder can embed images at all (PH-008). Default false;
    /// vision embedders override to true. Lets callers report an honest 503
    /// instead of calling `embed_image` and silently falling back to text.
    fn handles_images(&self) -> bool {
        false
    }
    /// Embed multiple texts in batch. Default impl calls `embed` for each.
    fn embed_batch(&self, texts: &[&str]) -> Vec<Vec<f32>> {
        texts.iter().map(|t| self.embed(t)).collect()
    }
    /// Return the dimension of the embedding vectors.
    fn dimension(&self) -> usize;
}

/// Version tag of the semantic probe. The [`SelfTestCache`] keys verdicts on
/// path + mtime only, so any change to the probe itself must bump this —
/// otherwise a model recorded as passing under an older, laxer probe keeps
/// skipping re-test forever.
pub(crate) const SEMANTIC_PROBE_VERSION: &str = "v2";

/// Required margin between the related-pair similarity and the
/// distractor-pair similarity. Generous enough for real semantic models,
/// tight enough that token-overlap hashers (which score lexical noise at
/// roughly the same level for everything) cannot clear it.
///
/// Public (not `pub(crate)`) because the `physis-pro selftest` CLI command
/// must apply exactly the same gate the library uses — duplicating the
/// literal here and in `src/main.rs` is how gates like this drift apart.
pub const SEMANTIC_PROBE_MARGIN: f32 = 0.05;

/// (related A, related B, unrelated distractor) — A and B share no token.
///
/// Public for the same reason as [`SEMANTIC_PROBE_MARGIN`]: the CLI selftest
/// subcommand walks the identical triples, so the probe set has one home.
pub const SEMANTIC_PROBES: [(&str, &str, &str); 3] = [
    ("car", "automobile", "banana"),
    ("dog", "puppy", "spreadsheet"),
    (
        "spindle motor overheated and tripped",
        "thermal fault shut down the drive",
        "invoice payment terms net thirty days",
    ),
];

/// Semantic self-test (PREMORTEM P0/P4): a model can load yet emit garbage
/// (fp16 outputs read as f32, opset drift, truncated dims). Every cascade that
/// selects an embedder must gate candidates with this — the CLI shipped
/// without it and silently ran on Jina noise.
///
/// The probe uses **lexically disjoint** pairs: the two members of a related
/// pair share no common token, so a bag-of-words or feature-hashing embedder
/// cannot pass by token overlap (the original dog/puppy pair shared 4 of 5
/// tokens and passed under the random-projection embedder, reporting
/// `"semantic": true` for a non-semantic engine — PLAN.md H2). Each related
/// pair must also beat a fixed margin over its distractor, over several
/// domains, so one lucky pair cannot carry a garbage model through.
pub fn semantic_self_test(e: &dyn VectorEmbed) -> bool {
    let finite = |v: &[f32]| !v.is_empty() && v.iter().all(|x| x.is_finite());
    for (a_text, b_text, d_text) in SEMANTIC_PROBES {
        let a = e.embed(a_text);
        let b = e.embed(b_text);
        let d = e.embed(d_text);
        if !(finite(&a) && finite(&b) && finite(&d)) {
            return false;
        }
        let related = crate::models::cosine_sim(&a, &b);
        let unrelated = crate::models::cosine_sim(&a, &d);
        if related <= unrelated + SEMANTIC_PROBE_MARGIN {
            return false;
        }
    }
    true
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
        Self {
            path,
            map,
            dirty: false,
        }
    }

    /// Key by the resolved model file (flat `model.onnx`, else `onnx/model.onnx`,
    /// matching OnnxEmbedder) plus its mtime, so a changed model re-tests. The
    /// probe version prefixes the key: hardening [`semantic_self_test`] must
    /// invalidate verdicts recorded under the older, laxer probe.
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
        format!(
            "{SEMANTIC_PROBE_VERSION}:{label}:{}:{mtime}",
            file.display()
        )
    }

    /// Build + self-test an embedder, but skip the (eager, possibly multi-GB)
    /// build entirely when a prior run recorded this model as failing.
    pub fn gated<F>(
        &mut self,
        label: &str,
        model_dir: &str,
        build: F,
    ) -> Option<Box<dyn VectorEmbed>>
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
        Self {
            inner: physis_core::RandomProjectionEmbedder::new(dim),
        }
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

// Image embedding needs the `image` crate (now optional: it drags in
// moxcms/pxfm, which require GLIBC 2.43 and break feather). The ONNX text
// path — the one the HUD and every CLI command use — stays default.
#[cfg(all(feature = "embed-onnx", feature = "image-embed"))]
pub mod clip;

#[cfg(all(feature = "embed-onnx", feature = "image-embed"))]
pub mod jina;

#[cfg(feature = "embed-onnx")]
pub mod registry;

pub mod ollama;

/// Whether an operator explicitly asked for the offline random-projection
/// embedder, given the raw `PHYSIS_EMBEDDER` value.
///
/// One predicate, three callers (CLI selection, server selection, and the
/// server's DEGRADED decision) — the spelling of this request is a contract
/// with the operator, and three hand-written copies of it drift.
pub fn wants_random_projection(requested: Option<&str>) -> bool {
    matches!(requested, Some("random-projection") | Some("rp"))
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
    // Explicit operator override, honored before any model is touched.
    //
    // The web server and the interest daemon already respect
    // `PHYSIS_EMBEDDER=random-projection`; the CLI did not, so a command run
    // with the variable set loaded the full ONNX cascade anyway and spent
    // minutes embedding the ontology the operator had just asked it to skip.
    // An ignored instruction is worse than an unsupported one: the caller has
    // no way to tell it was ignored except by the wait.
    //
    // This is the supported OFFLINE mode, not a fallback — deterministic,
    // model-free, coarser but meaningful — so it is chosen silently rather
    // than warned about here (`PhysisApp` still prints the non-semantic
    // warning, which is where an operator will read it).
    if wants_random_projection(std::env::var("PHYSIS_EMBEDDER").ok().as_deref()) {
        return (
            Box::new(RandomProjectionEmbedder::new(config.embed_dim)),
            "random-projection",
        );
    }

    // Explicit config choice of random-projection is the same supported
    // OFFLINE mode as the env override above — honor it without the
    // semantic self-test gate (RP is deterministic hashing; the gate would
    // reject it and fall through to minutes of model probing, ignoring an
    // explicit instruction). PH-009: this is what lets embedder-agnostic
    // CLI tests run offline via config instead of racing on process env.
    if let Some(ec) = config.embedders.primary.as_ref() {
        if ec.enabled && matches!(ec.kind, EmbedderKindConfig::RandomProjection) {
            return (
                Box::new(RandomProjectionEmbedder::new(config.embed_dim)),
                "random-projection",
            );
        }
    }

    // Explicit model choice for the E1 measurement: `PHYSIS_EMBEDDER=bge-small`
    // selects the distilled encoder ahead of the cascade, so the same corpus can
    // be packed by both encoders and compared on wall-clock and hit@5 (see
    // docs/plans/2026-09-19-small-encoders.md). Self-test gated like every other
    // candidate: a model that loads but emits non-semantic vectors is refused
    // rather than silently used.
    if std::env::var("PHYSIS_EMBEDDER").ok().as_deref() == Some("bge-small") {
        #[cfg(feature = "embed-onnx")]
        {
            let cfg = crate::config::OnnxConfig {
                enabled: true,
                model_dir: Some(model_dir(config, "bge-small-en-v1.5")),
                dim: 384,
                max_length: 512,
                pooling: crate::config::PoolingStrategy::Mean,
            };
            let b = OnnxEmbedder::with_config(&cfg);
            if b.is_available() && super::embed::semantic_self_test(&b) {
                return (Box::new(b), "bge-small");
            }
            eprintln!(
                "physis: PHYSIS_EMBEDDER=bge-small requested but the model is absent or \
                 fails the semantic self-test; falling through to the cascade"
            );
        }
    }

    // Ollama embedding backend (default `nomic-embed-text:latest` on
    // localhost:11434): 137M params, ~274 MB, CPU-friendly, dim 768. Tried
    // before the ONNX legs because the weights are usually absent while the
    // server is already up. Self-test gated like every other candidate; a
    // down server or a garbage model falls through silently. Opt out with
    // `PHYSIS_OLLAMA_EMBED_OFF=1`.
    // ponytail: one env check inline; a config-file knob if a second caller needs it.
    if std::env::var("PHYSIS_OLLAMA_EMBED_OFF").ok().as_deref() != Some("1") {
        let o = ollama::OllamaEmbedder::from_env();
        if super::embed::semantic_self_test(&o) {
            return (Box::new(o), "ollama-embed");
        }
    }

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
            if !ec.enabled {
                return None;
            }
            match ec.kind {
                EmbedderKindConfig::JinaV2 => {
                    #[cfg(feature = "image-embed")]
                    {
                        let j = JinaEmbedder::with_model_dir(&ec.model_dir);
                        j.is_available()
                            .then(|| (Box::new(j) as Box<dyn VectorEmbed>, "jina-v2"))
                    }
                    #[cfg(not(feature = "image-embed"))]
                    {
                        None // vision model needs the image-embed feature
                    }
                }
                EmbedderKindConfig::Clip => {
                    #[cfg(feature = "image-embed")]
                    {
                        let c = ClipEmbedder::with_config(&config.onnx);
                        c.is_available()
                            .then(|| (Box::new(c) as Box<dyn VectorEmbed>, "clip"))
                    }
                    #[cfg(not(feature = "image-embed"))]
                    {
                        None // vision model needs the image-embed feature
                    }
                }
                EmbedderKindConfig::MiniLM => {
                    let m = OnnxEmbedder::with_config(&config.onnx);
                    m.is_available()
                        .then(|| (Box::new(m) as Box<dyn VectorEmbed>, "minilm"))
                }
                EmbedderKindConfig::BgeSmall => {
                    let cfg = crate::config::OnnxConfig {
                        enabled: true,
                        model_dir: Some(model_dir(config, "bge-small-en-v1.5")),
                        dim: 384,
                        max_length: 512,
                        pooling: crate::config::PoolingStrategy::Mean,
                    };
                    let b = OnnxEmbedder::with_config(&cfg);
                    b.is_available()
                        .then(|| (Box::new(b) as Box<dyn VectorEmbed>, "bge-small"))
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
                    b.is_available()
                        .then(|| (Box::new(b) as Box<dyn VectorEmbed>, "bge-base"))
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
                    b.is_available()
                        .then(|| (Box::new(b) as Box<dyn VectorEmbed>, "bge-m3"))
                }
                EmbedderKindConfig::SentenceTransformers => {
                    let (dim, pooling) = crate::config::st_layout(&ec.model_dir);
                    let cfg = crate::config::OnnxConfig {
                        enabled: true,
                        model_dir: Some(ec.model_dir.clone()),
                        dim,
                        max_length: 512,
                        pooling,
                    };
                    let s = OnnxEmbedder::with_config(&cfg);
                    s.is_available()
                        .then(|| (Box::new(s) as Box<dyn VectorEmbed>, "sentence-transformers"))
                }
                EmbedderKindConfig::RandomProjection => Some((
                    Box::new(RandomProjectionEmbedder::new(config.embed_dim)),
                    "random-projection",
                )),
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
            if let Some(e) = stc
                .gated("bge-base", "models/bge-base-en-v1.5", || {
                    let cfg = crate::config::OnnxConfig {
                        enabled: true,
                        model_dir: Some(model_dir(config, "bge-base-en-v1.5")),
                        dim: 768,
                        max_length: 512,
                        pooling: crate::config::PoolingStrategy::Mean,
                    };
                    let b = OnnxEmbedder::with_config(&cfg);
                    b.is_available()
                        .then(|| Box::new(b) as Box<dyn VectorEmbed>)
                })
                .map(|e| (e, "bge-base"))
            {
                break 'pick e;
            }
            if let Some(e) = stc
                .gated("bge-m3", "models/bge-m3", || {
                    let cfg = crate::config::OnnxConfig {
                        enabled: true,
                        model_dir: Some(model_dir(config, "bge-m3")),
                        dim: 1024,
                        max_length: 512,
                        pooling: crate::config::PoolingStrategy::Cls,
                    };
                    let b = OnnxEmbedder::with_config(&cfg);
                    b.is_available()
                        .then(|| Box::new(b) as Box<dyn VectorEmbed>)
                })
                .map(|e| (e, "bge-m3"))
            {
                break 'pick e;
            }
            if let Some(e) = gate({
                #[cfg(feature = "image-embed")]
                {
                    let c = ClipEmbedder::with_config(&config.onnx);
                    c.is_available()
                        .then(|| (Box::new(c) as Box<dyn VectorEmbed>, "clip"))
                }
                #[cfg(not(feature = "image-embed"))]
                {
                    None::<(Box<dyn VectorEmbed>, &'static str)> // vision needs image-embed
                }
            }) {
                break 'pick e;
            }
            if let Some(e) = gate({
                let m = OnnxEmbedder::with_config(&config.onnx);
                m.is_available()
                    .then(|| (Box::new(m) as Box<dyn VectorEmbed>, "minilm"))
            }) {
                break 'pick e;
            }
            (
                Box::new(RandomProjectionEmbedder::new(config.embed_dim)),
                "random-projection",
            )
        };
        stc.save();
        chosen
    }
    #[cfg(not(feature = "embed-onnx"))]
    (
        Box::new(RandomProjectionEmbedder::new(config.embed_dim)),
        "random-projection",
    )
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_offline_override_is_recognised_by_both_spellings() {
        // The CLI ignored this request entirely and loaded the ONNX cascade
        // anyway; the only symptom was a command that took minutes instead of
        // seconds. Both spellings the server documents must be honored.
        assert!(wants_random_projection(Some("random-projection")));
        assert!(wants_random_projection(Some("rp")));
        assert!(!wants_random_projection(Some("bge-base")));
        assert!(!wants_random_projection(Some("")));
        assert!(!wants_random_projection(None));
    }

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
        assert!(
            (norm - 1.0).abs() < 1e-5,
            "vector must be unit normalized, got {norm}"
        );
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
        assert!(
            (norm - 1.0).abs() < 1e-5,
            "empty string must produce normalized vector"
        );
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

    /// The probe's design contract: a related pair must never share a token,
    /// or a bag-of-words embedder can pass by overlap again (the original
    /// dog/puppy pair shared 4 of 5 tokens — PLAN.md H2).
    #[test]
    fn probe_pairs_are_lexically_disjoint() {
        let tokenize = |s: &str| -> Vec<String> {
            s.split(|c: char| !c.is_alphanumeric())
                .filter(|t| !t.is_empty())
                .map(|t| t.to_ascii_lowercase())
                .collect()
        };
        for (a, b, _) in SEMANTIC_PROBES {
            let (ta, tb) = (tokenize(a), tokenize(b));
            let shared: Vec<&String> = ta.iter().filter(|t| tb.contains(t)).collect();
            assert!(
                shared.is_empty(),
                "probe pair {a:?}/{b:?} shares tokens {shared:?} — a hash embedder could pass by overlap"
            );
        }
    }

    /// The feature-hashing embedder is non-semantic by design — under the lax
    /// dog/puppy probe it reported `semantic: true` on every health surface
    /// (PLAN.md H2). It must fail the disjoint-synonym probe.
    #[test]
    fn random_projection_fails_the_semantic_probe() {
        let e = RandomProjectionEmbedder::new(384);
        assert!(
            !semantic_self_test(&e),
            "random projection must not pass the semantic self-test"
        );
    }

    /// PH-004's unit gate, both arms. Arm (a) is always checkable. Arm (b)
    /// needs a trained model on disk, so the skip has to be *proved* rather
    /// than announced: the absence is asserted, which means dropping a flat
    /// MiniLM export at `models/model.onnx` turns this into the arm that
    /// exercises the probe against real weights instead of quietly staying
    /// green. A skip nobody can falsify is the failure mode PH-004 exists for.
    #[test]
    fn semantic_self_test_rejects_lexical_and_gates_real_weights() {
        // (a) non-semantic feature hashing → must fail
        assert!(
            !semantic_self_test(&RandomProjectionEmbedder::new(384)),
            "random projection must not pass the semantic self-test"
        );

        // (b) the flat MiniLM export the cascade calls "minilm" → must pass
        let flat = std::path::Path::new("models/model.onnx");
        #[cfg(feature = "embed-onnx")]
        {
            if flat.exists() {
                let e = OnnxEmbedder::new("models");
                assert!(
                    semantic_self_test(&e),
                    "{} exists but fails the semantic self-test — the probe or the \
                     export is broken",
                    flat.display()
                );
            } else {
                eprintln!(
                    "SKIP semantic_self_test[real weights]: {} is absent — only the \
                     random-projection arm ran",
                    flat.display()
                );
                assert!(
                    !flat.exists(),
                    "the skip arm ran even though the weights are present"
                );
            }
        }
        #[cfg(not(feature = "embed-onnx"))]
        eprintln!("SKIP semantic_self_test[real weights]: built without `embed-onnx`");
    }
}
