use std::collections::HashMap;
use std::sync::Mutex;

use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;
use sha2::{Sha256, Digest};

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

/// Hash a text into n-gram hashes using SHA-256.
fn hash_ngrams(text: &str, n: usize) -> Vec<u64> {
    let padded = format!(" {text} ");
    let chars: Vec<char> = padded.chars().collect();
    let mut hashes = Vec::new();
    for i in 0..chars.len().saturating_sub(n - 1) {
        let gram: String = chars[i..i + n].iter().collect();
        let mut h = Sha256::new();
        h.update(gram.as_bytes());
        let result = h.finalize();
        hashes.push(u64::from_le_bytes(result[..8].try_into().unwrap()));
    }
    hashes
}

/// Generate a random unit vector of dimension `dim` seeded by `seed`.
fn random_vector(dim: usize, seed: u64) -> Vec<f32> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut v = Vec::with_capacity(dim);
    for _ in 0..dim {
        let u1: f64 = rng.gen();
        let u2: f64 = rng.gen();
        let z = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
        v.push(z as f32);
    }
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-8);
    v.iter_mut().for_each(|x| *x /= norm);
    v
}

/// Deterministic random-projection embedder using feature hashing.
///
/// Maps any text to a fixed-dimension vector (default 384) using locality-sensitive
/// hashing with random projections. The same input always produces the same vector
/// (deterministic — seed is fixed at construction).
pub struct RandomProjectionEmbedder {
    dim: usize,
    seed: u64,
    basis: Mutex<HashMap<u64, Vec<f32>>>,
}

impl RandomProjectionEmbedder {
    /// Create a new embedder with the given vector dimension.
    ///
    /// Use `384` for compatibility with MiniLM (upgraded via `embed-onnx` feature).
    pub fn new(dim: usize) -> Self {
        Self {
            dim,
            seed: 42,
            basis: Mutex::new(HashMap::new()),
        }
    }

    fn get_basis(&self, key: u64) -> Vec<f32> {
        let mut cache = self.basis.lock().unwrap();
        cache.entry(key).or_insert_with(|| random_vector(self.dim, key ^ self.seed)).clone()
    }
}

impl VectorEmbed for RandomProjectionEmbedder {
    /// Embed text using unigram + bigram feature hashing with random projection.
    ///
    /// The output is always L2-normalized (unit length).
    fn embed(&self, text: &str) -> Vec<f32> {
        let lower = text.to_lowercase();
        let mut vec = vec![0.0f32; self.dim];

        for h in hash_ngrams(&lower, 1) {
            let basis = self.get_basis(h);
            for (i, val) in basis.iter().enumerate() {
                vec[i] += val;
            }
        }
        for h in hash_ngrams(&lower, 2) {
            let basis = self.get_basis(h ^ 0xFFFF);
            for (i, val) in basis.iter().enumerate() {
                vec[i] += val * 0.5;
            }
        }

        let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-8);
        vec.iter_mut().for_each(|x| *x /= norm);
        vec
    }

    fn dimension(&self) -> usize {
        self.dim
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
