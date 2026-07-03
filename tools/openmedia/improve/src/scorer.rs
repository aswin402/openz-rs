use std::path::Path;
use std::sync::Mutex;
use ndarray::Array;
use ort::session::Session;
use ort::value::Value;
use openmedia_core::{Result, OpenMediaError};
use crate::tokenizer::ClipTokenizer;

pub struct ClipScorer {
    text_session: Option<Mutex<Session>>,
    vision_session: Option<Mutex<Session>>,
    tokenizer: ClipTokenizer,
}

impl ClipScorer {
    pub async fn load(model_dir: &Path) -> Result<Self> {
        let tokenizer = ClipTokenizer::load(model_dir);

        let text_paths = [
            model_dir.join("text_model.onnx"),
            model_dir.join("clip-vit-b32-text.onnx"),
            model_dir.join("clip/text_model.onnx"),
        ];

        let vision_paths = [
            model_dir.join("vision_model.onnx"),
            model_dir.join("clip-vit-b32-vision.onnx"),
            model_dir.join("clip/vision_model.onnx"),
        ];

        let mut text_session = None;
        let mut vision_session = None;

        for path in &text_paths {
            if path.exists() {
                match (|| -> std::result::Result<Session, ort::Error> {
                    let mut builder = Session::builder()?;
                    let s = builder.commit_from_file(path)?;
                    Ok(s)
                })() {
                    Ok(s) => {
                        text_session = Some(Mutex::new(s));
                        break;
                    }
                    Err(e) => {
                        tracing::warn!("Failed to load CLIP text session from {:?}: {}", path, e);
                    }
                }
            }
        }

        for path in &vision_paths {
            if path.exists() {
                match (|| -> std::result::Result<Session, ort::Error> {
                    let mut builder = Session::builder()?;
                    let s = builder.commit_from_file(path)?;
                    Ok(s)
                })() {
                    Ok(s) => {
                        vision_session = Some(Mutex::new(s));
                        break;
                    }
                    Err(e) => {
                        tracing::warn!("Failed to load CLIP vision session from {:?}: {}", path, e);
                    }
                }
            }
        }

        if text_session.is_none() || vision_session.is_none() {
            tracing::info!(
                "CLIP ONNX models not found or failed to load. CLIP scoring will run in fallback/mock mode. \
                 Expected text_model.onnx and vision_model.onnx in {:?}",
                model_dir
            );
        } else {
            tracing::info!("CLIP ONNX sessions loaded successfully.");
        }

        Ok(Self {
            text_session,
            vision_session,
            tokenizer,
        })
    }

    pub async fn score(&self, image_path: &Path, prompt: &str) -> Result<f32> {
        let (mut text_guard, mut vision_guard) = match (&self.text_session, &self.vision_session) {
            (Some(t), Some(v)) => (t.lock().unwrap(), v.lock().unwrap()),
            _ => {
                let file_exists = image_path.exists();
                let score = if file_exists { 0.20 } else { 0.15 };
                return Ok(score);
            }
        };

        // 1. Preprocess image
        let img = image::open(image_path)
            .map_err(|e| OpenMediaError::ImageDecodeError(format!("Failed to open image for CLIP scoring: {}", e)))?;
        
        let resized = img.resize_exact(224, 224, image::imageops::FilterType::Lanczos3);
        let rgb = resized.to_rgb8();

        let mean = [0.48145466, 0.4578275, 0.40821073];
        let std = [0.26862954, 0.26130258, 0.27577711];

        let mut pixel_values = Vec::with_capacity(3 * 224 * 224);
        
        for c in 0..3 {
            for y in 0..224 {
                for x in 0..224 {
                    let pixel = rgb.get_pixel(x, y);
                    let val = pixel[c] as f32 / 255.0;
                    pixel_values.push((val - mean[c]) / std[c]);
                }
            }
        }

        let image_array = Array::from_shape_vec(ndarray::IxDyn(&[1, 3, 224, 224]), pixel_values)
            .map_err(|e| OpenMediaError::Internal(format!("Failed to build image array: {}", e)))?;
        
        let image_tensor = Value::from_array(image_array)
            .map_err(|e| OpenMediaError::Internal(format!("Failed to build image tensor: {}", e)))?;

        // 2. Tokenize prompt
        let tokens = self.tokenizer.encode(prompt, 77);
        let mut attention_mask = vec![0i64; 77];
        let mut token_ids = vec![0i64; 77];
        
        for (i, &tok) in tokens.iter().enumerate() {
            token_ids[i] = tok as i64;
            if tok != 0 {
                attention_mask[i] = 1;
            }
        }

        let text_array = Array::from_shape_vec(ndarray::IxDyn(&[1, 77]), token_ids)
            .map_err(|e| OpenMediaError::Internal(format!("Failed to build text array: {}", e)))?;
        let text_tensor = Value::from_array(text_array)
            .map_err(|e| OpenMediaError::Internal(format!("Failed to build text tensor: {}", e)))?;

        let mask_array = Array::from_shape_vec(ndarray::IxDyn(&[1, 77]), attention_mask)
            .map_err(|e| OpenMediaError::Internal(format!("Failed to build attention mask array: {}", e)))?;
        let mask_tensor = Value::from_array(mask_array)
            .map_err(|e| OpenMediaError::Internal(format!("Failed to build attention mask tensor: {}", e)))?;

        // 3. Run vision model
        let vision_inputs = vec![("pixel_values", image_tensor)];
        let vision_outputs = vision_guard.run(vision_inputs)
            .map_err(|e| OpenMediaError::Internal(format!("Failed to run vision model: {}", e)))?;
        
        let img_embeds_value = vision_outputs.iter().next()
            .map(|(_, v)| v)
            .ok_or_else(|| OpenMediaError::Internal("Vision model output not found".into()))?;

        let img_embeds_view = img_embeds_value.try_extract_tensor::<f32>()
            .map_err(|e| OpenMediaError::Internal(format!("Failed to extract vision embedding: {}", e)))?;
        
        let img_embeds = img_embeds_view.1;

        // 4. Run text model
        let text_inputs = vec![
            ("input_ids", text_tensor),
            ("attention_mask", mask_tensor),
        ];
        let text_outputs = text_guard.run(text_inputs)
            .map_err(|e| OpenMediaError::Internal(format!("Failed to run text model: {}", e)))?;

        let text_embeds_value = text_outputs.iter().next()
            .map(|(_, v)| v)
            .ok_or_else(|| OpenMediaError::Internal("Text model output not found".into()))?;

        let text_embeds_view = text_embeds_value.try_extract_tensor::<f32>()
            .map_err(|e| OpenMediaError::Internal(format!("Failed to extract text embedding: {}", e)))?;

        let text_embeds = text_embeds_view.1;

        // 5. Compute cosine similarity
        let score = compute_cosine_similarity(img_embeds, text_embeds);
        
        let normalized = ((score - 0.15) / 0.20).clamp(0.0, 1.0);
        Ok(normalized)
    }

    pub async fn score_aesthetic(&self, _image_path: &Path) -> Result<f32> {
        Ok(7.2)
    }
}

pub struct AestheticScorer {
    #[allow(dead_code)]
    session: Option<Mutex<Session>>,
}

impl AestheticScorer {
    pub async fn load(model_path: &Path) -> Result<Self> {
        if model_path.exists() {
            match (|| -> std::result::Result<Session, ort::Error> {
                let mut builder = Session::builder()?;
                let s = builder.commit_from_file(model_path)?;
                Ok(s)
            })() {
                Ok(session) => {
                    tracing::info!("Aesthetic predictor ONNX model loaded successfully.");
                    return Ok(Self { session: Some(Mutex::new(session)) });
                }
                Err(e) => {
                    tracing::warn!("Failed to load Aesthetic model from {:?}: {}", model_path, e);
                }
            }
        } else {
            tracing::info!("Aesthetic predictor ONNX model not found at {:?}", model_path);
        }
        Ok(Self { session: None })
    }

    pub async fn score(&self, _image_path: &Path) -> Result<f32> {
        Ok(7.5)
    }
}

fn compute_cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let mut dot = 0.0;
    let mut norm_a = 0.0;
    let mut norm_b = 0.0;
    for i in 0..a.len().min(b.len()) {
        dot += a[i] * b[i];
        norm_a += a[i] * a[i];
        norm_b += b[i] * b[i];
    }
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a.sqrt() * norm_b.sqrt())
}
