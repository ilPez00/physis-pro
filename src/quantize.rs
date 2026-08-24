//! Product quantizer for compressed vector storage — f32 → byte codes.
//! Splits D-dim vector into M sub-vectors, each quantized to K centroids.
//! Compression ratio: D * 4 → M bytes (e.g., 384 * 4 = 1536 → 8 bytes).
// Index-based loops here read parallel slices (vec/sub/codes/centroids) by the
// same index; rewriting as zipped iterators hurts readability with no gain.
#![allow(clippy::needless_range_loop)]
use std::cmp::Ordering;

const DEFAULT_M: usize = 8;
const DEFAULT_K: usize = 256;

/// Compresses f32 vectors into compact byte codes via product quantization.
#[derive(Debug, Clone)]
pub struct ProductQuantizer {
    pub m: usize,       // number of sub-vectors
    pub k: usize,       // centroids per sub-vector
    pub dim: usize,     // full vector dimension
    pub sub_dim: usize, // dimension per sub-vector (dim / m)
    /// Codebooks: M x K x sub_dim
    pub codebooks: Vec<Vec<Vec<f32>>>,
    /// Online training counters
    codebook_counts: Vec<Vec<usize>>,
    trained: bool,
}

impl ProductQuantizer {
    /// Creates a PQ with defaults (M=8, K=256) for the given vector dimension.
    pub fn new(dim: usize) -> Self {
        let m = DEFAULT_M;
        let k = DEFAULT_K;
        let sub_dim = dim.div_ceil(m);
        let codebooks = vec![vec![vec![0.0f32; sub_dim]; k]; m];
        let codebook_counts = vec![vec![0usize; k]; m];
        Self {
            m,
            k,
            dim,
            sub_dim,
            codebooks,
            codebook_counts,
            trained: false,
        }
    }

    /// Creates a PQ with custom M sub-vectors and K centroids per subspace.
    pub fn with_params(dim: usize, m: usize, k: usize) -> Self {
        let sub_dim = dim.div_ceil(m);
        let codebooks = vec![vec![vec![0.0f32; sub_dim]; k]; m];
        let codebook_counts = vec![vec![0usize; k]; m];
        Self {
            m,
            k,
            dim,
            sub_dim,
            codebooks,
            codebook_counts,
            trained: false,
        }
    }

    /// Online training: update codebooks with a new vector.
    /// Uses streaming k-means on each sub-vector independently.
    pub fn train_one(&mut self, vec: &[f32]) {
        let lr = 0.01;
        for m_idx in 0..self.m {
            let start = m_idx * self.sub_dim;
            let end = (start + self.sub_dim).min(self.dim);
            let mut sub = Vec::with_capacity(self.sub_dim);
            for i in start..end {
                sub.push(vec[i]);
            }
            sub.resize(self.sub_dim, 0.0);

            // Find nearest centroid
            let (best_idx, _) = self.codebooks[m_idx]
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    let dist: f32 = sub
                        .iter()
                        .zip(c.iter())
                        .map(|(a, b)| (a - b) * (a - b))
                        .sum();
                    (i, dist)
                })
                .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                .unwrap_or((0, 0.0));

            self.codebook_counts[m_idx][best_idx] += 1;
            let count = self.codebook_counts[m_idx][best_idx] as f32;
            let adj_lr = lr / (1.0 + count * 0.01); // decaying LR

            // Update centroid
            for i in 0..self.sub_dim {
                let delta = sub[i] - self.codebooks[m_idx][best_idx][i];
                self.codebooks[m_idx][best_idx][i] += adj_lr * delta;
            }
        }
        self.trained = true;
    }

    /// Quantize a vector: returns M bytes, each being the centroid index.
    pub fn quantize(&self, vec: &[f32]) -> Vec<u8> {
        let mut codes = Vec::with_capacity(self.m);
        for m_idx in 0..self.m {
            let start = m_idx * self.sub_dim;
            let end = (start + self.sub_dim).min(self.dim);
            let mut sub = Vec::with_capacity(self.sub_dim);
            for i in start..end {
                sub.push(vec[i]);
            }
            sub.resize(self.sub_dim, 0.0);

            let best_idx = self.codebooks[m_idx]
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    let dist: f32 = sub
                        .iter()
                        .zip(c.iter())
                        .map(|(a, b)| (a - b) * (a - b))
                        .sum();
                    (i, dist)
                })
                .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                .map(|(i, _)| i)
                .unwrap_or(0);

            codes.push(best_idx as u8);
        }
        codes
    }

    /// Dequantize: reconstruct approximate vector from codes.
    pub fn dequantize(&self, codes: &[u8]) -> Vec<f32> {
        let mut vec = Vec::with_capacity(self.dim);
        for m_idx in 0..self.m {
            let idx = codes[m_idx] as usize;
            let centroid = &self.codebooks[m_idx][idx.min(self.k - 1)];
            for &val in centroid.iter() {
                vec.push(val);
                if vec.len() >= self.dim {
                    break;
                }
            }
        }
        vec.truncate(self.dim);
        vec
    }

    /// Coarse search: compare query against all centroids, return scores per sub-vector.
    /// Returns a Vec of (code, distance_squared) for each sub-vector.
    pub fn coarse_search(&self, query_sub: &[f32], m_idx: usize) -> Vec<(u8, f32)> {
        self.codebooks[m_idx]
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let dist: f32 = query_sub
                    .iter()
                    .zip(c.iter())
                    .map(|(a, b)| (a - b) * (a - b))
                    .sum();
                (i as u8, dist)
            })
            .collect()
    }

    /// Asymmetric Distance Computation (ADC).
    /// Given a query vector and encoded (id, codes) entries, compute approximate
    /// squared L2 distances without dequantizing. Returns (id, distance_squared) sorted ascending.
    pub fn adc_search(&self, query: &[f32], encoded: &[(String, Vec<u8>)]) -> Vec<(String, f32)> {
        // Pre-compute per-sub-vector distances from query to each centroid
        let mut centroid_dists: Vec<Vec<(u8, f32)>> = Vec::with_capacity(self.m);
        for m_idx in 0..self.m {
            let start = m_idx * self.sub_dim;
            let end = (start + self.sub_dim).min(self.dim);
            let mut sub = Vec::with_capacity(self.sub_dim);
            for i in start..end {
                sub.push(query[i]);
            }
            sub.resize(self.sub_dim, 0.0);
            centroid_dists.push(self.coarse_search(&sub, m_idx));
        }

        let mut results: Vec<(String, f32)> = encoded
            .iter()
            .map(|(id, codes)| {
                let mut dist = 0.0f32;
                for m_idx in 0..self.m {
                    let centroid_idx = codes[m_idx] as usize;
                    if let Some(entry) = centroid_dists[m_idx]
                        .iter()
                        .find(|(ci, _)| *ci == centroid_idx as u8)
                    {
                        dist += entry.1;
                    }
                }
                (id.clone(), dist)
            })
            .collect();

        results.sort_by(|(_, da), (_, db)| da.partial_cmp(db).unwrap_or(Ordering::Equal));
        results
    }

    /// Returns true after at least one vector has been trained.
    pub fn is_trained(&self) -> bool {
        self.trained
    }

    /// Returns the compressed size in bytes, equal to the number of sub-vectors M.
    pub fn compressed_size(&self) -> usize {
        self.m
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quantize_roundtrip() {
        let dim = 64;
        let mut pq = ProductQuantizer::new(dim);
        let v: Vec<f32> = (0..dim).map(|i| (i as f32).sin()).collect();
        pq.train_one(&v);
        pq.train_one(&v);
        let codes = pq.quantize(&v);
        let reconstructed = pq.dequantize(&codes);
        assert_eq!(reconstructed.len(), dim);
        // Should be roughly similar after training
        let dot: f32 = v.iter().zip(reconstructed.iter()).map(|(a, b)| a * b).sum();
        assert!(dot > 0.0);
    }

    #[test]
    fn test_compression_ratio() {
        let dim = 384;
        let pq = ProductQuantizer::new(dim);
        assert_eq!(pq.compressed_size(), 8);
        assert!(pq.dim * 4 > pq.compressed_size());
    }

    #[test]
    fn test_coarse_search() {
        let dim = 32;
        let pq = ProductQuantizer::new(dim);
        let sub = vec![0.5f32; dim / DEFAULT_M];
        let results = pq.coarse_search(&sub, 0);
        assert_eq!(results.len(), 256);
    }
}
