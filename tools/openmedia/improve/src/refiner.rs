use openmedia_core::QualityScore;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RefinedPrompt {
    pub prompt: String,
    pub negative_prompt: String,
    pub suggested_steps: u32,
    pub suggested_cfg_scale: f32,
    pub changes: Vec<String>,
}

pub struct PromptRefiner {
    pub quality_suffixes: Vec<String>,
    pub negative_defaults: Vec<String>,
}

impl PromptRefiner {
    pub fn new() -> Self {
        Self {
            quality_suffixes: vec![
                "highly detailed".into(),
                "professional".into(),
                "sharp focus".into(),
                "studio lighting".into(),
                "8k uhd".into(),
                "masterpiece".into(),
            ],
            negative_defaults: vec![
                "blurry".into(),
                "low quality".into(),
                "distorted".into(),
                "deformed".into(),
                "disfigured".into(),
                "bad anatomy".into(),
                "watermark".into(),
                "text".into(),
                "signature".into(),
            ],
        }
    }

    pub fn refine(
        &self,
        original_prompt: &str,
        original_negative: &str,
        scores: &QualityScore,
        round: u32,
    ) -> RefinedPrompt {
        let mut changes = Vec::new();

        // 1. Refine the positive prompt
        let mut prompt_parts: Vec<String> = original_prompt
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        // Select a quality suffix based on the round to vary output, avoiding existing terms
        let suffix_idx = (round as usize) % self.quality_suffixes.len();
        let target_suffix = &self.quality_suffixes[suffix_idx];

        let has_suffix = prompt_parts
            .iter()
            .any(|p| p.to_lowercase() == target_suffix.to_lowercase());
        if !has_suffix {
            prompt_parts.push(target_suffix.clone());
            changes.push(format!("Added quality suffix: '{}'", target_suffix));
        }

        // Add masterpiece if clip score is low
        if let Some(clip) = scores.clip_score {
            if clip < 0.22
                && !prompt_parts
                    .iter()
                    .any(|p| p.to_lowercase() == "masterpiece")
            {
                prompt_parts.push("masterpiece".to_string());
                changes.push("Added positive bias keyword: 'masterpiece'".to_string());
            }
        }

        let prompt = prompt_parts.join(", ");

        // 2. Refine the negative prompt
        let mut negative_parts: Vec<String> = original_negative
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        for neg in &self.negative_defaults {
            if !negative_parts
                .iter()
                .any(|p| p.to_lowercase() == neg.to_lowercase())
            {
                negative_parts.push(neg.clone());
            }
        }

        if negative_parts.len()
            > original_negative
                .split(',')
                .filter(|s| !s.trim().is_empty())
                .count()
        {
            changes.push("Appended defect avoidance keywords to negative prompt".to_string());
        }

        let negative_prompt = negative_parts.join(", ");

        // 3. Refine inference parameters
        // Increase step count by 25% per round
        let base_steps = 20;
        let suggested_steps = base_steps + (round * 5);
        if round > 0 {
            changes.push(format!(
                "Increased step count from {} to {}",
                base_steps, suggested_steps
            ));
        }

        // Adjust CFG scale slightly
        let suggested_cfg_scale = if round % 2 == 1 { 8.0 } else { 7.5 };
        if suggested_cfg_scale != 7.5 {
            changes.push(format!(
                "Optimized guidance scale (CFG) to {}",
                suggested_cfg_scale
            ));
        }

        RefinedPrompt {
            prompt,
            negative_prompt,
            suggested_steps,
            suggested_cfg_scale,
            changes,
        }
    }
}
