pub mod engine;

use serde::{Serialize, Deserialize};
use std::fs;
use std::path::PathBuf;
use anyhow::{Result, Context};
use crate::config::resolve_path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SopStep {
    pub name: String,
    pub description: String,
    pub prompt_template: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SopDefinition {
    pub id: String,
    pub name: String,
    pub description: String,
    pub steps: Vec<SopStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SopStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Paused,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepExecutionState {
    pub name: String,
    pub status: String, // "Pending", "Running", "Completed", "Failed"
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub output: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SopInstance {
    pub id: String,
    pub sop_id: String,
    pub name: String,
    pub status: SopStatus,
    pub current_step_index: usize,
    pub steps: Vec<StepExecutionState>,
    pub context: serde_json::Value,
    pub started_at: String,
    pub completed_at: Option<String>,
}

pub fn sop_dir() -> PathBuf {
    resolve_path("~/.openz/sop")
}

pub fn sop_definitions_path() -> PathBuf {
    sop_dir().join("definitions.json")
}

pub fn sop_instances_dir() -> PathBuf {
    sop_dir().join("instances")
}

pub fn initialize_sop_system() -> Result<()> {
    let dir = sop_dir();
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }
    let inst_dir = sop_instances_dir();
    if !inst_dir.exists() {
        fs::create_dir_all(&inst_dir)?;
    }

    let defs_path = sop_definitions_path();
    if !defs_path.exists() {
        let defaults = get_default_sop_definitions();
        let content = serde_json::to_string_pretty(&defaults)?;
        fs::write(&defs_path, content)?;
    }

    Ok(())
}

pub fn load_definitions() -> Result<Vec<SopDefinition>> {
    initialize_sop_system()?;
    let path = sop_definitions_path();
    let content = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read SOP definitions at {:?}", path))?;
    let defs: Vec<SopDefinition> = serde_json::from_str(&content)?;
    Ok(defs)
}

pub fn get_definition(sop_id: &str) -> Result<Option<SopDefinition>> {
    let defs = load_definitions()?;
    Ok(defs.into_iter().find(|d| d.id == sop_id))
}

pub fn save_definition(def: &SopDefinition) -> Result<()> {
    let mut defs = load_definitions().unwrap_or_default();
    defs.retain(|d| d.id != def.id);
    defs.push(def.clone());
    let content = serde_json::to_string_pretty(&defs)?;
    fs::write(sop_definitions_path(), content)?;
    Ok(())
}

pub fn load_instance(instance_id: &str) -> Result<SopInstance> {
    let path = sop_instances_dir().join(format!("{}.json", instance_id));
    let content = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read SOP instance at {:?}", path))?;
    let inst: SopInstance = serde_json::from_str(&content)?;
    Ok(inst)
}

pub fn save_instance(inst: &SopInstance) -> Result<()> {
    initialize_sop_system()?;
    let path = sop_instances_dir().join(format!("{}.json", inst.id));
    let content = serde_json::to_string_pretty(inst)?;
    fs::write(&path, content)?;
    Ok(())
}

pub fn list_instances() -> Result<Vec<SopInstance>> {
    initialize_sop_system()?;
    let mut instances = Vec::new();
    let entries = fs::read_dir(sop_instances_dir())?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && path.extension().map(|e| e == "json").unwrap_or(false) {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(inst) = serde_json::from_str::<SopInstance>(&content) {
                    instances.push(inst);
                }
            }
        }
    }
    // Sort by started_at descending
    instances.sort_by(|a, b| b.started_at.cmp(&a.started_at));
    Ok(instances)
}

fn get_default_sop_definitions() -> Vec<SopDefinition> {
    vec![
        SopDefinition {
            id: "pr-review".to_string(),
            name: "PR Review Standard Operating Procedure".to_string(),
            description: "Automated Pull Request analyzer and code quality reviewer".to_string(),
            steps: vec![
                SopStep {
                    name: "Analyze Diff".to_string(),
                    description: "Analyzes the PR diff for bugs and style issues".to_string(),
                    prompt_template: "We received a webhook for a new pull request. Here is the payload: {{payload}}.\n\nExtract the code diff or description from the payload. Analyze the changes for logic bugs, security issues, or missing test cases.".to_string(),
                },
                SopStep {
                    name: "Generate Recommendations".to_string(),
                    description: "Generates actionable code suggestions to resolve issues".to_string(),
                    prompt_template: "Based on the code analysis: {{steps.Analyze Diff.output}}, generate concrete, actionable code refactoring suggestions or fixes. Output them clearly in Markdown formatting.".to_string(),
                },
                SopStep {
                    name: "Draft Comment".to_string(),
                    description: "Prepares GitHub/GitLab comment text representing the full review".to_string(),
                    prompt_template: "Draft a polite and constructive comment summarizing our findings from the analysis: {{steps.Analyze Diff.output}} and recommendations: {{steps.Generate Recommendations.output}}.".to_string(),
                },
            ],
        },
        SopDefinition {
            id: "incident-response".to_string(),
            name: "Incident & Log Response SOP".to_string(),
            description: "Workflow triggered on errors or alerts to triage and document mitigations".to_string(),
            steps: vec![
                SopStep {
                    name: "Triage Alert".to_string(),
                    description: "Classifies alert severity and extracts failure signature".to_string(),
                    prompt_template: "An error alert was triggered. Alert payload: {{payload}}.\n\nTriage the alert. Extract the error message, severity level, source file, and line number if present. Summarize the failure signature.".to_string(),
                },
                SopStep {
                    name: "Investigate Root Cause".to_string(),
                    description: "Suggests query commands or checks to run".to_string(),
                    prompt_template: "Based on failure signature: {{steps.Triage Alert.output}}, propose the root cause of this incident and describe what files, ports, or databases we should inspect to fix it.".to_string(),
                },
                SopStep {
                    name: "Formulate Mitigation".to_string(),
                    description: "Creates a step-by-step resolution plan".to_string(),
                    prompt_template: "Write a step-by-step resolution plan to address root cause: {{steps.Investigate Root Cause.output}}. Include verification commands to check if the fix was successful.".to_string(),
                },
            ],
        },
    ]
}

pub fn substitute_template(template: &str, context: &serde_json::Value) -> String {
    let re = regex::Regex::new(r"\{\{([^}]+)\}\}").unwrap();
    re.replace_all(template, |caps: &regex::Captures| {
        let path = caps.get(1).unwrap().as_str().trim();
        resolve_path_value(context, path)
    }).into_owned()
}

fn resolve_path_value(json_value: &serde_json::Value, path: &str) -> String {
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = json_value;
    for part in parts {
        if let Some(next) = current.get(part) {
            current = next;
        } else {
            return format!("{{{{{}}}}}", path);
        }
    }
    if let Some(s) = current.as_str() {
        s.to_string()
    } else {
        current.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_substitute_template_simple() {
        let context = serde_json::json!({
            "payload": {
                "repository": "openz-rs",
                "number": 42
            },
            "steps": {
                "Analyze": {
                    "output": "No issues found"
                }
            }
        });

        let template = "Repo is {{payload.repository}} and issue is #{{payload.number}}. Result: {{steps.Analyze.output}}.";
        let substituted = substitute_template(template, &context);
        assert_eq!(substituted, "Repo is openz-rs and issue is #42. Result: No issues found.");
    }

    #[test]
    fn test_substitute_template_missing() {
        let context = serde_json::json!({
            "payload": {}
        });
        let template = "Hello {{payload.missing_key}}!";
        let substituted = substitute_template(template, &context);
        assert_eq!(substituted, "Hello {{payload.missing_key}}!");
    }

    #[tokio::test]
    async fn test_sop_lifecycle() {
        let temp_dir = std::env::temp_dir().join(format!("openz_sop_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        // Override config directory for testing
        std::env::set_var("OPENZ_CONFIG_DIR", temp_dir.to_str().unwrap());

        initialize_sop_system().unwrap();

        // Check default definitions are created
        let defs = load_definitions().unwrap();
        assert!(!defs.is_empty());
        let pr_review = defs.iter().find(|d| d.id == "pr-review").unwrap();
        assert_eq!(pr_review.steps.len(), 3);

        // Save a mock instance
        let steps = pr_review.steps.iter().map(|step| StepExecutionState {
            name: step.name.clone(),
            status: "Pending".to_string(),
            started_at: None,
            completed_at: None,
            output: None,
            error: None,
        }).collect();

        let inst = SopInstance {
            id: "test-inst-123".to_string(),
            sop_id: pr_review.id.clone(),
            name: "Test Instance".to_string(),
            status: SopStatus::Pending,
            current_step_index: 0,
            steps,
            context: serde_json::json!({
                "payload": {"repo": "test"},
                "steps": {}
            }),
            started_at: Utc::now().to_rfc3339(),
            completed_at: None,
        };

        save_instance(&inst).unwrap();

        let loaded = load_instance("test-inst-123").unwrap();
        assert_eq!(loaded.id, "test-inst-123");
        assert_eq!(loaded.sop_id, "pr-review");
        assert_eq!(loaded.status, SopStatus::Pending);

        let list = list_instances().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, "test-inst-123");

        // Clean up temp dir
        std::fs::remove_dir_all(&temp_dir).unwrap();
        std::env::remove_var("OPENZ_CONFIG_DIR");
    }
}
