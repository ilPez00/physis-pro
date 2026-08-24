use super::VectorEmbed;
use std::path::Path;
use std::sync::Mutex;

use ort::value::{DynTensorValueType, Tensor};

/// Jina CLIP v2 embedder — single ONNX model for text + image.
/// Produces 1024-d L2-normalized vectors in a shared embedding space.
pub struct JinaEmbedder {
    dim: usize,
    session: Mutex<Option<ort::session::Session>>,
    tokenizer: Option<tokenizers::Tokenizer>,
    #[allow(dead_code)]
    model_dir: String,
    max_length: usize,
    image_size: u32,
}

use super::load_onnx_session as load_session;

impl JinaEmbedder {
    pub fn new(model_dir: &str) -> Self {
        // Preference order: fp16 (standalone, loads under ort rc.12) → fp32
        // model.onnx → q4f16 (the 4-bit quant trips rc.12's graph optimizer, so
        // it is tried last). `find_map` falls through to the next candidate when
        // one fails to load, instead of short-circuiting to None on the first
        // file that merely *exists*. See PREMORTEM.md P4.
        let session = [
            "onnx/model_fp16.onnx",
            "onnx/model.onnx",
            "onnx/model_q4f16.onnx",
        ]
        .iter()
        .map(|p| Path::new(model_dir).join(p))
        .filter(|p| p.exists())
        .find_map(|p| load_session(&p));
        let tok_path = Path::new(model_dir).join("tokenizer.json");
        let tokenizer = if tok_path.exists() {
            tokenizers::Tokenizer::from_file(&tok_path).ok()
        } else {
            None
        };
        Self {
            dim: 1024,
            session: Mutex::new(session),
            tokenizer,
            model_dir: model_dir.to_string(),
            max_length: 512,
            image_size: 512,
        }
    }

    pub fn with_model_dir(model_dir: &str) -> Self {
        Self::new(model_dir)
    }

    pub fn is_available(&self) -> bool {
        self.session.lock().unwrap().is_some() && self.tokenizer.is_some()
    }

    /// Preprocess image bytes into a [1, 3, 512, 512] f32 tensor
    fn preprocess_image(bytes: &[u8], image_size: u32) -> Option<Vec<f32>> {
        let img = image::load_from_memory(bytes).ok()?.into_rgb8();
        let (w, h) = img.dimensions();
        let short = w.min(h);
        let scale = image_size as f32 / short as f32;
        let new_w = (w as f32 * scale) as u32;
        let new_h = (h as f32 * scale) as u32;
        let resized = image::imageops::resize(&img, new_w, new_h, image::imageops::Lanczos3);
        let cx = new_w / 2;
        let cy = new_h / 2;
        let crop_sz = image_size.min(new_w).min(new_h);
        let cropped = image::imageops::crop_imm(
            &resized,
            cx - crop_sz / 2,
            cy - crop_sz / 2,
            crop_sz,
            crop_sz,
        )
        .to_image();
        let is = image_size as usize;
        let mut tensor = Vec::with_capacity(3 * is * is);
        for c in 0..3 {
            for y in 0..is {
                for x in 0..is {
                    let px = cropped.get_pixel(x as u32, y as u32)[c];
                    let val = (px as f32 / 255.0 - 0.5) / 0.5;
                    tensor.push(val);
                }
            }
        }
        Some(tensor)
    }
}

impl VectorEmbed for JinaEmbedder {
    fn embed(&self, text: &str) -> Vec<f32> {
        let tokenizer = match &self.tokenizer {
            Some(t) => t,
            None => return fallback_embed(text, self.dim),
        };
        let encoding = match tokenizer.encode(text, true) {
            Ok(e) => e,
            Err(_) => return fallback_embed(text, self.dim),
        };
        let actual_len = encoding.len().min(self.max_length);
        let mut ids: Vec<i64> = encoding
            .get_ids()
            .iter()
            .take(self.max_length)
            .map(|&v| v as i64)
            .collect();
        ids.resize(self.max_length, 0i64);
        let mask: Vec<i64> = (0..self.max_length)
            .map(|i| if i < actual_len { 1i64 } else { 0i64 })
            .collect();
        let shape = vec![1i64, self.max_length as i64];

        let input_ids = match Tensor::<i64>::from_array((shape.clone(), ids)) {
            Ok(t) => t,
            Err(_) => return fallback_embed(text, self.dim),
        };

        // Jina fused model needs dummy pixel_values even for text-only
        let is = self.image_size as usize;
        let dummy_pixels = vec![0.0f32; 3 * is * is];
        let pixel_tensor =
            match Tensor::<f32>::from_array((vec![1i64, 3, is as i64, is as i64], dummy_pixels)) {
                Ok(t) => t,
                Err(_) => return fallback_embed(text, self.dim),
            };

        let attention_mask = match Tensor::<i64>::from_array((shape, mask)) {
            Ok(t) => t,
            Err(_) => return fallback_embed(text, self.dim),
        };

        let inputs: Vec<(&str, ort::value::Value<ort::value::DynValueTypeMarker>)> = vec![
            ("input_ids", input_ids.into()),
            ("attention_mask", attention_mask.into()),
            ("pixel_values", pixel_tensor.into()),
        ];

        let mut guard = self.session.lock().unwrap();
        let s = match guard.as_mut() {
            Some(s) => s,
            None => return fallback_embed(text, self.dim),
        };

        let outputs = match s.run(inputs) {
            Ok(o) => o,
            Err(_) => return fallback_embed(text, self.dim),
        };

        // Use pre-normalized embedding
        let out = outputs
            .get("l2norm_text_embeddings")
            .or_else(|| outputs.get("text_embeddings"))
            .or_else(|| (outputs.len() > 0).then(|| &outputs[0]));

        let out = match out {
            Some(v) => v,
            None => return fallback_embed(text, self.dim),
        };

        let t = match out.downcast_ref::<DynTensorValueType>() {
            Ok(t) => t,
            Err(_) => return fallback_embed(text, self.dim),
        };

        let view = match t.try_extract_array::<f32>() {
            Ok(v) => v,
            Err(_) => return fallback_embed(text, self.dim),
        };

        let shape = view.shape();
        let features = if shape.len() >= 2 { shape[1] } else { self.dim };
        let mut pooled = vec![0.0f32; features.min(self.dim)];
        for d in 0..pooled.len() {
            pooled[d] = view[[0, d]];
        }

        // Ensure normalized (should be for l2norm_*, but safe)
        let norm = pooled.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-8);
        pooled.iter_mut().for_each(|x| *x /= norm);
        pooled
    }

    fn embed_image(&self, bytes: &[u8]) -> Option<Vec<f32>> {
        let pixels = Self::preprocess_image(bytes, self.image_size)?;
        let is = self.image_size as i64;
        let pixel_tensor = Tensor::<f32>::from_array((vec![1i64, 3, is, is], pixels)).ok()?;

        // Dummy text input for fused model
        let dummy_ids = vec![0i64; self.max_length];
        let input_shape = vec![1i64, self.max_length as i64];
        let input_ids = Tensor::<i64>::from_array((input_shape.clone(), dummy_ids)).ok()?;
        let attention_mask =
            Tensor::<i64>::from_array((input_shape, vec![0i64; self.max_length])).ok()?;

        let inputs: Vec<(&str, ort::value::Value<ort::value::DynValueTypeMarker>)> = vec![
            ("input_ids", input_ids.into()),
            ("attention_mask", attention_mask.into()),
            ("pixel_values", pixel_tensor.into()),
        ];

        let mut guard = self.session.lock().unwrap();
        let s = guard.as_mut()?;
        let outputs = s.run(inputs).ok()?;

        let out = outputs
            .get("l2norm_image_embeddings")
            .or_else(|| outputs.get("image_embeddings"))
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
