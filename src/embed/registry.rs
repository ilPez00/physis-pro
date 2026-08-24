use super::clip::ClipEmbedder;
use super::jina::JinaEmbedder;
use super::onnx::OnnxEmbedder;
use super::{RandomProjectionEmbedder, VectorEmbed};

use crate::config::{EmbedderKindConfig, EmbeddersConfig, OnnxConfig};

/// Registry holds all configured embedders and dispatches by modality.
pub struct EmbedderRegistry {
    /// Text embedder — always present (RP fallback at minimum)
    pub text: Box<dyn VectorEmbed>,
    /// Image embedder — optional (CLIP/Jina vision)
    pub image: Option<Box<dyn VectorEmbed>>,
    /// Dimension of the text embedder
    pub text_dim: usize,
}

impl EmbedderRegistry {
    /// Build registry from config. Falls back through: configured → ONNX MiniLM → RP.
    pub fn from_config(config: &EmbeddersConfig, onnx_cfg: &OnnxConfig) -> Self {
        let (text, image) = Self::build_embedders(config, onnx_cfg);
        let text_dim = text.dimension();
        Self {
            text,
            image,
            text_dim,
        }
    }

    fn build_embedders(
        config: &EmbeddersConfig,
        onnx_cfg: &OnnxConfig,
    ) -> (Box<dyn VectorEmbed>, Option<Box<dyn VectorEmbed>>) {
        // Try configured embedder, fall back to MiniLM, then RP
        if let Some(ref ec) = config.primary {
            return match ec.kind {
                EmbedderKindConfig::JinaV2 => {
                    let jina = JinaEmbedder::with_model_dir(&ec.model_dir);
                    if jina.is_available() {
                        (Box::new(jina), None) // Jina handles both text+image, but image via same model
                    } else {
                        Self::fallback_onnx(onnx_cfg)
                    }
                }
                EmbedderKindConfig::BgeBase => {
                    let cfg = OnnxConfig {
                        enabled: true,
                        model_dir: Some("models/bge-base-en-v1.5".to_string()),
                        dim: 768,
                        max_length: 512,
                        pooling: crate::config::PoolingStrategy::Mean,
                    };
                    let bge = OnnxEmbedder::with_config(&cfg);
                    if bge.is_available() {
                        (Box::new(bge), None)
                    } else {
                        Self::fallback_onnx(onnx_cfg)
                    }
                }
                EmbedderKindConfig::BgeM3 => {
                    let cfg = OnnxConfig {
                        enabled: true,
                        model_dir: Some("models/bge-m3".to_string()),
                        dim: 1024,
                        max_length: 512,
                        pooling: crate::config::PoolingStrategy::Cls,
                    };
                    let m3 = OnnxEmbedder::with_config(&cfg);
                    if m3.is_available() {
                        (Box::new(m3), None)
                    } else {
                        Self::fallback_onnx(onnx_cfg)
                    }
                }
                EmbedderKindConfig::Clip => {
                    let clip = ClipEmbedder::with_config(onnx_cfg);
                    if clip.is_available() {
                        let vision_available = clip.vision_session.lock().unwrap().is_some();
                        let img: Option<Box<dyn VectorEmbed>> = if vision_available {
                            Some(Box::new(ClipEmbedder::with_config(onnx_cfg)))
                        } else {
                            None
                        };
                        (Box::new(clip), img)
                    } else {
                        Self::fallback_onnx(onnx_cfg)
                    }
                }
                EmbedderKindConfig::MiniLM => Self::fallback_onnx(onnx_cfg),
                EmbedderKindConfig::RandomProjection => Self::fallback_rp(onnx_cfg.dim),
            };
        }
        Self::fallback_onnx(onnx_cfg)
    }

    fn fallback_onnx(
        onnx_cfg: &OnnxConfig,
    ) -> (Box<dyn VectorEmbed>, Option<Box<dyn VectorEmbed>>) {
        let onnx = OnnxEmbedder::with_config(onnx_cfg);
        if onnx.is_available() {
            (Box::new(onnx), None)
        } else {
            Self::fallback_rp(onnx_cfg.dim)
        }
    }

    fn fallback_rp(dim: usize) -> (Box<dyn VectorEmbed>, Option<Box<dyn VectorEmbed>>) {
        (Box::new(RandomProjectionEmbedder::new(dim)), None)
    }

    /// Get the primary text embedder
    pub fn text_embedder(&self) -> &dyn VectorEmbed {
        &*self.text
    }

    /// Get the image embedder if available
    pub fn image_embedder(&self) -> Option<&dyn VectorEmbed> {
        self.image.as_deref()
    }
}
