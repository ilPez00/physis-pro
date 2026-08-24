use super::VectorEmbed;
use std::path::Path;
use std::sync::Mutex;

use ort::value::{DynTensorValueType, Tensor};

use crate::config::OnnxConfig;

/// CLIP embedder with separate text and vision ONNX models.
/// Both produce 512-d vectors in the same shared embedding space.
pub struct ClipEmbedder {
    dim: usize,
    text_session: Mutex<Option<ort::session::Session>>,
    pub vision_session: Mutex<Option<ort::session::Session>>,
    tokenizer: Option<tokenizers::Tokenizer>,
    #[allow(dead_code)]
    model_dir: String,
    max_length: usize,
}

use super::load_onnx_session as load_session;

impl ClipEmbedder {
    fn load_text(
        model_dir: &str,
    ) -> (Option<ort::session::Session>, Option<tokenizers::Tokenizer>) {
        let model_path = Path::new(model_dir).join("onnx/text_model.onnx");
        let tok_path = Path::new(model_dir).join("tokenizer.json");
        let session = if model_path.exists() {
            load_session(&model_path)
        } else {
            None
        };
        let tokenizer = if tok_path.exists() {
            tokenizers::Tokenizer::from_file(&tok_path).ok()
        } else {
            None
        };
        (session, tokenizer)
    }

    fn load_vision(model_dir: &str) -> Option<ort::session::Session> {
        let model_path = Path::new(model_dir).join("onnx/vision_model.onnx");
        if model_path.exists() {
            load_session(&model_path)
        } else {
            None
        }
    }

    pub fn new(model_dir: &str) -> Self {
        let (ts, tok) = Self::load_text(model_dir);
        let vs = Self::load_vision(model_dir);
        Self {
            dim: 512,
            text_session: Mutex::new(ts),
            vision_session: Mutex::new(vs),
            tokenizer: tok,
            model_dir: model_dir.to_string(),
            max_length: 77,
        }
    }

    pub fn with_config(config: &OnnxConfig) -> Self {
        let model_dir = config
            .model_dir
            .as_deref()
            .unwrap_or("./models/clip-vit-base-patch32");
        Self::new(model_dir)
    }

    pub fn is_available(&self) -> bool {
        self.text_session.lock().unwrap().is_some()
            && self.vision_session.lock().unwrap().is_some()
            && self.tokenizer.is_some()
    }

    /// Preprocess image bytes into a [1, 3, 224, 224] f32 tensor
    fn preprocess_image(bytes: &[u8]) -> Option<Vec<f32>> {
        let img = image::load_from_memory(bytes).ok()?.into_rgb8();
        let (w, h) = img.dimensions();
        let short = w.min(h);
        let scale = 224.0 / short as f32;
        let new_w = (w as f32 * scale) as u32;
        let new_h = (h as f32 * scale) as u32;
        let resized = image::imageops::resize(&img, new_w, new_h, image::imageops::Lanczos3);
        let cx = new_w / 2;
        let cy = new_h / 2;
        let crop_size = 224.min(new_w).min(new_h);
        let cropped = image::imageops::crop_imm(
            &resized,
            cx - crop_size / 2,
            cy - crop_size / 2,
            crop_size,
            crop_size,
        )
        .to_image();
        let mean = [0.48145466f32, 0.4578275, 0.40821073];
        let std = [0.26862954f32, 0.261_302_6, 0.275_777_1];
        let mut tensor = Vec::with_capacity(3 * 224 * 224);
        for c in 0..3 {
            for y in 0..224 {
                for x in 0..224 {
                    let px = cropped.get_pixel(x, y)[c];
                    let val = (px as f32 / 255.0 - mean[c]) / std[c];
                    tensor.push(val);
                }
            }
        }
        Some(tensor)
    }

    fn run_text_session(&self, text: &str) -> Option<Vec<f32>> {
        let tokenizer = self.tokenizer.as_ref()?;
        let encoding = tokenizer.encode(text, true).ok()?;
        let actual_len = encoding.len().min(self.max_length);
        let mut ids: Vec<i64> = encoding
            .get_ids()
            .iter()
            .take(self.max_length)
            .map(|&v| v as i64)
            .collect();
        ids.resize(self.max_length, 0i64);
        let mut type_ids: Vec<i64> = encoding
            .get_type_ids()
            .iter()
            .take(self.max_length)
            .map(|&v| v as i64)
            .collect();
        type_ids.resize(self.max_length, 0i64);
        let mask: Vec<i64> = (0..self.max_length)
            .map(|i| if i < actual_len { 1i64 } else { 0i64 })
            .collect();
        let shape = vec![1i64, self.max_length as i64];
        let input_ids = Tensor::<i64>::from_array((shape.clone(), ids)).ok()?;
        let attn_mask = Tensor::<i64>::from_array((shape.clone(), mask)).ok()?;
        let tok_types = Tensor::<i64>::from_array((shape, type_ids)).ok()?;
        let inputs: Vec<(&str, ort::value::Value<ort::value::DynValueTypeMarker>)> = vec![
            ("input_ids", input_ids.into()),
            ("attention_mask", attn_mask.into()),
            ("token_type_ids", tok_types.into()),
        ];
        let mut guard = self.text_session.lock().unwrap();
        let s = guard.as_mut()?;
        let outputs = s.run(inputs).ok()?;
        let out = outputs
            .get("text_embeds")
            .or_else(|| (outputs.len() > 0).then(|| &outputs[0]))?;
        let t = out.downcast_ref::<DynTensorValueType>().ok()?;
        let view = t.try_extract_array::<f32>().ok()?;
        let shape = view.shape();
        let features = if shape.len() >= 2 { shape[1] } else { self.dim };
        let mut pooled = vec![0.0f32; features.min(self.dim)];
        for d in 0..pooled.len() {
            pooled[d] = view[[0, d]];
        }
        let norm = pooled.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-8);
        pooled.iter_mut().for_each(|x| *x /= norm);
        Some(pooled)
    }
}

impl VectorEmbed for ClipEmbedder {
    fn embed(&self, text: &str) -> Vec<f32> {
        self.run_text_session(text)
            .unwrap_or_else(|| fallback_embed(text, self.dim))
    }

    fn embed_image(&self, bytes: &[u8]) -> Option<Vec<f32>> {
        let pixels = Self::preprocess_image(bytes)?;
        let pixel_tensor = Tensor::<f32>::from_array((vec![1i64, 3, 224, 224], pixels)).ok()?;
        let inputs: Vec<(&str, ort::value::Value<ort::value::DynValueTypeMarker>)> =
            vec![("pixel_values", pixel_tensor.into())];
        let mut guard = self.vision_session.lock().unwrap();
        let s = guard.as_mut()?;
        let outputs = s.run(inputs).ok()?;
        let out = outputs
            .get("image_embeds")
            .or_else(|| (outputs.len() > 0).then(|| &outputs[0]))?;
        let t = out.downcast_ref::<DynTensorValueType>().ok()?;
        let view = t.try_extract_array::<f32>().ok()?;
        let shape = view.shape();
        let features = if shape.len() >= 2 { shape[1] } else { self.dim };
        let mut pooled = vec![0.0f32; features.min(self.dim)];
        for d in 0..pooled.len() {
            pooled[d] = view[[0, d]];
        }
        let norm = pooled.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-8);
        pooled.iter_mut().for_each(|x| *x /= norm);
        Some(pooled)
    }

    fn dimension(&self) -> usize {
        self.dim
    }
}

fn fallback_embed(text: &str, dim: usize) -> Vec<f32> {
    let mut v = vec![0.0f32; dim];
    let h: u64 = text
        .bytes()
        .fold(0u64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64));
    let mut rng: rand::rngs::StdRng = rand::SeedableRng::seed_from_u64(h);
    use rand::Rng;
    for x in &mut v {
        *x = rng.random_range(-1.0..1.0);
    }
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-8);
    v.iter_mut().for_each(|x| *x /= norm);
    v
}
