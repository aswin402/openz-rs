pub mod history;
pub mod refiner;
pub mod scorer;
pub mod tokenizer;

use serde::{Deserialize, Serialize};

pub use history::GenerationHistory;
pub use refiner::{PromptRefiner, RefinedPrompt};
pub use scorer::{AestheticScorer, ClipScorer};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationRecord {
    pub id: String,
    pub created_at: String,
    pub tool_name: String,
    pub request_params: serde_json::Value,
    pub output_path: String,
    pub output_format: String,
    pub output_size: u64,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub duration: Option<f64>,
    pub model_used: Option<String>,
    pub backend_used: Option<String>,
    pub generation_time: f64,
    pub clip_score: Option<f32>,
    pub aesthetic_score: Option<f32>,
    pub refined_from: Option<String>,
    pub refinement_round: u32,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Default)]
pub struct HistoryFilter {
    pub tool_name: Option<String>,
    pub limit: u32,
    pub offset: u32,
    pub sort_by: String,
    pub sort_order: String,
    pub min_clip_score: Option<f32>,
    pub min_aesthetic: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryStats {
    pub total_generations: u64,
    pub total_size_bytes: u64,
    pub avg_clip_score: Option<f32>,
    pub avg_aesthetic_score: Option<f32>,
    pub db_size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Feedback {
    pub generation_id: String,
    pub rating: f32,
    pub feedback: Option<String>,
    pub keep: bool,
    pub created_at: String,
}
