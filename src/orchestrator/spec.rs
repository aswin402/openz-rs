use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowMode {
    Sequential,
    Parallel,
    ManagerWorker,
    ReviewLoop,
    SelectorGroup,
    Graph,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentRef {
    #[serde(alias = "agent")]
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default)]
    pub tools: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkflowStep {
    pub id: String,
    pub agent: String,
    #[serde(alias = "prompt")]
    pub goal: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub expected_output: String,
    #[serde(default)]
    pub max_retries: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TerminationPolicy {
    #[serde(default = "default_max_rounds")]
    pub max_rounds: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub success_keyword: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_keyword: Option<String>,
}

fn default_max_rounds() -> usize {
    8
}

impl Default for TerminationPolicy {
    fn default() -> Self {
        Self {
            max_rounds: default_max_rounds(),
            success_keyword: None,
            failure_keyword: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReviewMode {
    None,
    Optional,
    Required,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReviewPolicy {
    #[serde(default = "default_review_mode")]
    pub mode: ReviewMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewer: Option<String>,
}

fn default_review_mode() -> ReviewMode {
    ReviewMode::None
}

impl Default for ReviewPolicy {
    fn default() -> Self {
        Self {
            mode: ReviewMode::None,
            reviewer: None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityPolicy {
    #[serde(default)]
    pub allowed_tools: Vec<String>,
    #[serde(default)]
    pub denied_tools: Vec<String>,
    #[serde(default)]
    pub deny_shell: bool,
    #[serde(default)]
    pub deny_filesystem_write: bool,
    #[serde(default)]
    pub deny_network: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkflowSpec {
    pub goal: String,
    pub mode: WorkflowMode,
    #[serde(default)]
    pub agents: Vec<AgentRef>,
    #[serde(default)]
    pub steps: Vec<WorkflowStep>,
    #[serde(default)]
    pub termination: TerminationPolicy,
    #[serde(default)]
    pub review: ReviewPolicy,
    #[serde(default)]
    pub capabilities: CapabilityPolicy,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn workflow_spec_round_trips_from_json() {
        let raw = serde_json::json!({
            "goal": "Research and implement feature",
            "mode": "sequential",
            "agents": [
                { "name": "researcher" },
                { "name": "reviewer", "model": "openrouter/qwen/qwen2.5-vl-72b-instruct:free" }
            ],
            "steps": [
                {
                    "id": "research",
                    "agent": "researcher",
                    "goal": "Collect references",
                    "depends_on": [],
                    "expected_output": "bullet summary with sources"
                },
                {
                    "id": "review",
                    "agent": "reviewer",
                    "goal": "Check result quality",
                    "depends_on": ["research"],
                    "expected_output": "approve or revision notes"
                }
            ],
            "termination": { "max_rounds": 4, "success_keyword": "APPROVE" },
            "review": { "mode": "required", "reviewer": "reviewer" },
            "capabilities": { "allowed_tools": ["searchxyz_read_url"], "deny_shell": true, "deny_network": true }
        });

        let spec: WorkflowSpec = serde_json::from_value(raw).expect("valid workflow spec");
        assert_eq!(spec.goal, "Research and implement feature");
        assert!(matches!(spec.mode, WorkflowMode::Sequential));
        assert_eq!(spec.agents.len(), 2);
        assert_eq!(spec.steps[1].depends_on, vec!["research"]);
        assert_eq!(spec.termination.max_rounds, 4);
        assert_eq!(spec.capabilities.allowed_tools, vec!["searchxyz_read_url"]);
        assert!(spec.capabilities.deny_network);
    }

    #[test]
    fn workflow_spec_accepts_model_friendly_aliases() {
        let raw = serde_json::json!({
            "goal": "Run a small workflow",
            "mode": "sequential",
            "agents": [
                { "agent": "planner" }
            ],
            "steps": [
                {
                    "id": "plan",
                    "agent": "planner",
                    "prompt": "Summarize hello"
                }
            ]
        });

        let spec: WorkflowSpec = serde_json::from_value(raw).expect("aliases are accepted");
        assert_eq!(spec.agents[0].name, "planner");
        assert_eq!(spec.steps[0].goal, "Summarize hello");
    }
}
