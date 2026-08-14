use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus { Pending, Running, Success, Failed, Skipped, AwaitingReview }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowStatus { Running, Success, Failed, Cancelled, AwaitingReview }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StepRunResult {
    pub step_id: String,
    pub agent: String,
    pub status: StepStatus,
    pub output: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default)]
    pub duration_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkflowRunResult {
    pub run_id: String,
    pub status: WorkflowStatus,
    pub summary: String,
    pub steps: Vec<StepRunResult>,
    #[serde(default)]
    pub sources: Vec<String>,
}
