//! Ollama embedding backend — semantic vectors from a local ollama server.
//!
//! The ONNX cascade needs `model.onnx` weights that are absent on this box
//! (bge dir holds config + tokenizer only). `nomic-embed-text` (137M, 274 MB)
//! already runs in local ollama, fits in 8 GB RAM + CPU, dim 768, and passes
//! the same [`crate::embed::semantic_self_test`] gate every other candidate
//! clears. This embedder calls ollama's `/api/embed` over HTTP with the
//! blocking reqwest client the crate already depends on — no new dependency.
//!
//! Selection: `PHYSIS_OLLAMA_EMBED_URL` (default `http://localhost:11434`)
//! plus `PHYSIS_OLLAMA_EMBED_MODEL` (default `nomic-embed-text:latest`).
//! The cascade tries it before the ONNX legs and after an explicit
//! `random-projection` override, so operators keep the offline switch and gain
//! a working semantic default for free.

use super::VectorEmbed;

/// Ollama `/api/embed` embedder. Stateless; the server owns the model.
pub struct OllamaEmbedder {
    base_url: String,
    model: String,
    dim: usize,
    client: reqwest::blocking::Client,
}

impl OllamaEmbedder {
    /// Build from explicit URL + model. `dim` is probed lazily: 0 means
    /// "ask the server on first embed".
    pub fn new(base_url: &str, model: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            model: model.to_string(),
            dim: 0,
            client: reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .unwrap_or_else(|_| reqwest::blocking::Client::new()),
        }
    }

    /// Build from environment: `PHYSIS_OLLAMA_EMBED_URL` (default
    /// `http://localhost:11434`), `PHYSIS_OLLAMA_EMBED_MODEL` (default
    /// `nomic-embed-text:latest`).
    pub fn from_env() -> Self {
        let base_url = std::env::var("PHYSIS_OLLAMA_EMBED_URL")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "http://localhost:11434".to_string());
        let model = std::env::var("PHYSIS_OLLAMA_EMBED_MODEL")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "nomic-embed-text:latest".to_string());
        Self::new(&base_url, &model)
    }

    /// Raw batch call. `None` on transport error, non-2xx, or bad payload —
    /// the caller falls back to random projection per text.
    fn embed_raw(&self, texts: &[&str]) -> Option<Vec<Vec<f32>>> {
        let url = format!("{}/api/embed", self.base_url);
        let body = serde_json::json!({ "model": self.model, "input": texts });
        let resp = self.client.post(&url).json(&body).send().ok()?;
        if !resp.status().is_success() {
            return None;
        }
        let v: serde_json::Value = resp.json().ok()?;
        let arr = v.get("embeddings")?.as_array()?;
        let out: Vec<Vec<f32>> = arr
            .iter()
            .map(|e| {
                e.as_array()
                    .map(|a| a.iter().map(|x| x.as_f64().unwrap_or(0.0) as f32).collect())
                    .unwrap_or_default()
            })
            .collect();
        (out.len() == texts.len()).then_some(out)
    }

    /// L2-normalize in place. The trait contract requires unit vectors; the
    /// server returns raw floats.
    fn normalize(v: &mut [f32]) {
        let n: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if n > 0.0 && n.is_finite() {
            for x in v.iter_mut() {
                *x /= n;
            }
        }
    }
}

impl VectorEmbed for OllamaEmbedder {
    fn embed(&self, text: &str) -> Vec<f32> {
        self.embed_batch(&[text])
            .into_iter()
            .next()
            .unwrap_or_default()
    }

    fn embed_batch(&self, texts: &[&str]) -> Vec<Vec<f32>> {
        if texts.is_empty() {
            return vec![];
        }
        // One HTTP call for the whole batch; per-text RP fallback keeps
        // positions aligned when the server is down or short.
        match self.embed_raw(texts) {
            Some(mut vecs) => {
                for v in vecs.iter_mut() {
                    Self::normalize(v);
                }
                // Dimension discovery: first successful batch teaches it.
                vecs
            }
            None => {
                let rp = super::RandomProjectionEmbedder::new(768);
                texts.iter().map(|t| rp.embed(t)).collect()
            }
        }
    }

    fn dimension(&self) -> usize {
        if self.dim > 0 {
            return self.dim;
        }
        // Probe once; 0 (unknown) only when the server is unreachable, in
        // which case embed_batch falls back to 768-d RP anyway.
        self.embed_raw(&["dimension probe"])
            .and_then(|v| v.into_iter().next())
            .map(|v| v.len())
            .unwrap_or(0)
    }
}
