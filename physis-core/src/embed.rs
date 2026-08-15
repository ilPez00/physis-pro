//! Deterministic, dependency-light embedding. The `VectorEmbed` trait is the
//! seam where a heavier embedder (ONNX/MiniLM) can plug in later; the default
//! `RandomProjectionEmbedder` ships zero model files and is fully deterministic.

use std::collections::HashMap;
use std::sync::Mutex;

use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;
use sha2::{Digest, Sha256};

/// Trait for embedding text into fixed-dimension vectors.
///
/// Implementors must guarantee the same input always produces the same vector
/// (determinism), and the vector must be L2-normalized (unit length).
pub trait VectorEmbed: Send + Sync {
    fn embed(&self, text: &str) -> Vec<f32>;
    fn embed_batch(&self, texts: &[&str]) -> Vec<Vec<f32>> {
        texts.iter().map(|t| self.embed(t)).collect()
    }
    fn dimension(&self) -> usize;
}

impl<T: VectorEmbed + ?Sized> VectorEmbed for Box<T> {
    fn embed(&self, text: &str) -> Vec<f32> {
        (**self).embed(text)
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
/// hashing with random projections. Same input → same vector (seed fixed at
/// construction). Cheap and model-free; cosine between related texts is meaningful
/// but coarser than a trained model.
pub struct RandomProjectionEmbedder {
    dim: usize,
    seed: u64,
    basis: Mutex<HashMap<u64, Vec<f32>>>,
}

impl RandomProjectionEmbedder {
    pub fn new(dim: usize) -> Self {
        Self { dim, seed: 42, basis: Mutex::new(HashMap::new()) }
    }

    fn get_basis(&self, key: u64) -> Vec<f32> {
        let mut cache = self.basis.lock().unwrap();
        cache.entry(key).or_insert_with(|| random_vector(self.dim, key ^ self.seed)).clone()
    }
}

impl VectorEmbed for RandomProjectionEmbedder {
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

/// Semantic self-test: a related pair must clearly out-score an unrelated pair,
/// and all vectors finite. Guards against a broken embedder.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_and_normalized() {
        let e = RandomProjectionEmbedder::new(64);
        let v1 = e.embed("hello world");
        let v2 = e.embed("hello world");
        assert_eq!(v1, v2, "same input must produce same vector");
        let norm: f32 = v1.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-5, "must be unit normalized, got {norm}");
        assert_eq!(e.embed("").len(), 64);
    }

    #[test]
    fn different_inputs_differ() {
        let e = RandomProjectionEmbedder::new(64);
        assert_ne!(e.embed("hello"), e.embed("world"));
    }

    #[test]
    fn batch_matches_single() {
        let e = RandomProjectionEmbedder::new(64);
        let texts = &["first", "second", "third"];
        for (i, t) in texts.iter().enumerate() {
            assert_eq!(e.embed_batch(texts)[i], e.embed(t));
        }
    }

    #[test]
    fn selftest_passes() {
        assert!(semantic_self_test(&RandomProjectionEmbedder::new(384)));
    }
}
