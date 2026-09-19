// Vision embedders need the `image` crate (feature `image-embed`); without
// it the registry builds text-only and image slots stay None.
#[cfg(all(feature = "embed-onnx", feature = "image-embed"))]
use super::clip::ClipEmbedder;
#[cfg(all(feature = "embed-onnx", feature = "image-embed"))]
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
    /// Kind label of the image slot when present (mirrors the text-slot
    /// `embedder_kind` provenance; "clip" or "jina-clip-v2").
    pub image_kind: Option<&'static str>,
    /// Dimension of the text embedder
    pub text_dim: usize,
}

/// Built embedders: (text embedder, image slot, image-slot kind label).
type BuiltEmbedders = (
    Box<dyn VectorEmbed>,
    Option<Box<dyn VectorEmbed>>,
    Option<&'static str>,
);

impl EmbedderRegistry {
    /// Build registry from config. Falls back through: configured → ONNX MiniLM → RP.
    pub fn from_config(config: &EmbeddersConfig, onnx_cfg: &OnnxConfig) -> Self {
        let (text, image, image_kind) = Self::build_embedders(config, onnx_cfg);
        let text_dim = text.dimension();
        Self {
            text,
            image,
            image_kind,
            text_dim,
        }
    }

    fn build_embedders(config: &EmbeddersConfig, onnx_cfg: &OnnxConfig) -> BuiltEmbedders {
        // Primary text embedder (falls back through MiniLM → RP inside).
        let (text, mut image, mut image_kind) = if let Some(ref ec) = config.primary {
            match ec.kind {
                EmbedderKindConfig::JinaV2 => {
                    #[cfg(feature = "image-embed")]
                    {
                        let jina = JinaEmbedder::with_model_dir(&ec.model_dir);
                        if jina.is_available() {
                            (Box::new(jina) as Box<dyn VectorEmbed>, None, None)
                        // Jina handles text+image via one model
                        } else {
                            Self::fallback_onnx(onnx_cfg)
                        }
                    }
                    #[cfg(not(feature = "image-embed"))]
                    {
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
                        (Box::new(bge) as Box<dyn VectorEmbed>, None, None)
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
                        (Box::new(m3) as Box<dyn VectorEmbed>, None, None)
                    } else {
                        Self::fallback_onnx(onnx_cfg)
                    }
                }
                EmbedderKindConfig::Clip => {
                    #[cfg(not(feature = "image-embed"))]
                    {
                        let _ = onnx_cfg;
                        Self::fallback_onnx(onnx_cfg)
                    }
                    #[cfg(feature = "image-embed")]
                    {
                        let clip = ClipEmbedder::with_config(onnx_cfg);
                        if clip.is_available() {
                            let vision_available = clip.vision_session.lock().unwrap().is_some();
                            let img: Option<Box<dyn VectorEmbed>> = if vision_available {
                                Some(Box::new(ClipEmbedder::with_config(onnx_cfg)))
                            } else {
                                None
                            };
                            let kind = if vision_available { Some("clip") } else { None };
                            (Box::new(clip) as Box<dyn VectorEmbed>, img, kind)
                        } else {
                            Self::fallback_onnx(onnx_cfg)
                        }
                    }
                }
                EmbedderKindConfig::MiniLM => Self::fallback_onnx(onnx_cfg),
                EmbedderKindConfig::SentenceTransformers => {
                    let (dim, pooling) = crate::config::st_layout(&ec.model_dir);
                    let cfg = OnnxConfig {
                        enabled: true,
                        model_dir: Some(ec.model_dir.clone()),
                        dim,
                        max_length: 512,
                        pooling,
                    };
                    let st = OnnxEmbedder::with_config(&cfg);
                    if st.is_available() {
                        (Box::new(st) as Box<dyn VectorEmbed>, None, None)
                    } else {
                        Self::fallback_onnx(onnx_cfg)
                    }
                }
                EmbedderKindConfig::RandomProjection => Self::fallback_rp(onnx_cfg.dim),
            }
        } else {
            Self::fallback_onnx(onnx_cfg)
        };
        // Dedicated image-only embedder (`config.image:`) fills an empty slot.
        // This is how a text-only primary (bge-base) gains image capability
        // without touching the text path.
        if image.is_none() {
            if let Some(ref ic) = config.image {
                let (img, kind) = Self::build_image_only(ic, onnx_cfg);
                image = img;
                image_kind = kind;
            }
        }
        (text, image, image_kind)
    }

    /// Build the dedicated image-only slot from `config.image`. Only vision
    /// kinds populate it; anything else leaves the slot empty (honest 503
    /// downstream). Never downloads — availability is a local-sessions check.
    fn build_image_only(
        ec: &crate::config::EmbedderConfig,
        onnx_cfg: &OnnxConfig,
    ) -> (Option<Box<dyn VectorEmbed>>, Option<&'static str>) {
        match ec.kind {
            EmbedderKindConfig::Clip => {
                #[cfg(not(feature = "image-embed"))]
                {
                    let _ = (ec, onnx_cfg);
                    (None, None)
                }
                #[cfg(feature = "image-embed")]
                {
                    let clip = ClipEmbedder::with_config(onnx_cfg);
                    if clip.vision_session.lock().unwrap().is_some() {
                        (Some(Box::new(clip) as Box<dyn VectorEmbed>), Some("clip"))
                    } else {
                        (None, None)
                    }
                }
            }
            EmbedderKindConfig::JinaV2 => {
                #[cfg(not(feature = "image-embed"))]
                {
                    let _ = (ec, onnx_cfg);
                    (None, None)
                }
                #[cfg(feature = "image-embed")]
                {
                    let jina = JinaEmbedder::with_model_dir(&ec.model_dir);
                    if jina.is_available() {
                        (
                            Some(Box::new(jina) as Box<dyn VectorEmbed>),
                            Some("jina-clip-v2"),
                        )
                    } else {
                        (None, None)
                    }
                }
            }
            _ => (None, None),
        }
    }

    fn fallback_onnx(onnx_cfg: &OnnxConfig) -> BuiltEmbedders {
        let onnx = OnnxEmbedder::with_config(onnx_cfg);
        if onnx.is_available() {
            (Box::new(onnx), None, None)
        } else {
            Self::fallback_rp(onnx_cfg.dim)
        }
    }

    fn fallback_rp(dim: usize) -> BuiltEmbedders {
        (Box::new(RandomProjectionEmbedder::new(dim)), None, None)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{EmbedderConfig, EmbedderKindConfig};

    fn rp_config() -> (EmbeddersConfig, OnnxConfig) {
        (
            EmbeddersConfig {
                primary: Some(EmbedderConfig {
                    kind: EmbedderKindConfig::RandomProjection,
                    model_dir: String::new(),
                    enabled: true,
                }),
                image: None,
            },
            OnnxConfig::default(),
        )
    }

    #[test]
    fn text_only_registry_leaves_image_slot_empty() {
        let (ec, oc) = rp_config();
        let reg = EmbedderRegistry::from_config(&ec, &oc);
        assert!(reg.image_embedder().is_none());
        assert_eq!(reg.image_kind, None);
        // Explicit error kind, not a silent fallback: the slot is simply absent.
        assert!(!reg.text_embedder().handles_images());
    }

    #[test]
    fn unknown_image_kind_leaves_slot_empty() {
        let (mut ec, oc) = rp_config();
        ec.image = Some(EmbedderConfig {
            kind: EmbedderKindConfig::BgeBase, // text-only kind: no vision slot
            model_dir: String::new(),
            enabled: true,
        });
        let reg = EmbedderRegistry::from_config(&ec, &oc);
        assert!(reg.image_embedder().is_none());
        assert_eq!(reg.image_kind, None);
    }
}
