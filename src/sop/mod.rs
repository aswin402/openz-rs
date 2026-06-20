pub mod engine;

use serde::{Serialize, Deserialize};
use std::fs;
use std::path::PathBuf;
use std::collections::{HashMap, HashSet};
use anyhow::{Result, Context};


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SopStep {
    pub name: String,
    pub description: String,
    pub prompt_template: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
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
    crate::config::loader::config_dir().join("sop")
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
    let mut current_defs = if defs_path.exists() {
        let content = fs::read_to_string(&defs_path)?;
        serde_json::from_str::<Vec<SopDefinition>>(&content).unwrap_or_default()
    } else {
        Vec::new()
    };

    let defaults = get_default_sop_definitions();
    let mut updated = false;
    for def in defaults {
        if !current_defs.iter().any(|d| d.id == def.id) {
            current_defs.push(def);
            updated = true;
        }
    }

    if updated || !defs_path.exists() {
        let content = serde_json::to_string_pretty(&current_defs)?;
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

pub fn validate_sop_definition(def: &SopDefinition) -> Result<()> {
    // 1. Check for duplicate step names
    let mut names = HashSet::new();
    for step in &def.steps {
        if !names.insert(&step.name) {
            anyhow::bail!("Duplicate step name: '{}' in SOP '{}'", step.name, def.id);
        }
    }

    // 2. Check for dependencies on non-existent steps
    for step in &def.steps {
        for dep in &step.depends_on {
            if !names.contains(dep) {
                anyhow::bail!(
                    "Step '{}' in SOP '{}' depends on non-existent step '{}'",
                    step.name,
                    def.id,
                    dep
                );
            }
        }
    }

    // 3. Cycle detection (DFS)
    let step_indices: HashMap<&str, usize> = def.steps.iter().enumerate().map(|(i, s)| (s.name.as_str(), i)).collect();
    let n = def.steps.len();
    let mut state = vec![0u8; n]; // 0 = unvisited, 1 = visiting, 2 = visited

    fn dfs(
        u: usize,
        def: &SopDefinition,
        step_indices: &HashMap<&str, usize>,
        state: &mut Vec<u8>,
    ) -> Result<()> {
        state[u] = 1;

        let step = &def.steps[u];
        for dep_name in &step.depends_on {
            if let Some(&v) = step_indices.get(dep_name.as_str()) {
                if state[v] == 1 {
                    anyhow::bail!("Circular dependency detected involving step '{}' and step '{}'", step.name, dep_name);
                } else if state[v] == 0 {
                    dfs(v, def, step_indices, state)?;
                }
            }
        }

        state[u] = 2;
        Ok(())
    }

    for i in 0..n {
        if state[i] == 0 {
            dfs(i, def, &step_indices, &mut state)?;
        }
    }

    Ok(())
}

pub fn save_definition(def: &SopDefinition) -> Result<()> {
    validate_sop_definition(def)?;
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
                    depends_on: vec![],
                    agent: Some("reviewer".to_string()),
                },
                SopStep {
                    name: "Generate Recommendations".to_string(),
                    description: "Generates actionable code suggestions to resolve issues".to_string(),
                    prompt_template: "Based on the code analysis: {{steps.Analyze Diff.output}}, generate concrete, actionable code refactoring suggestions or fixes. Output them clearly in Markdown formatting.".to_string(),
                    depends_on: vec!["Analyze Diff".to_string()],
                    agent: Some("reviewer".to_string()),
                },
                SopStep {
                    name: "Draft Comment".to_string(),
                    description: "Prepares GitHub/GitLab comment text representing the full review".to_string(),
                    prompt_template: "Draft a polite and constructive comment summarizing our findings from the analysis: {{steps.Analyze Diff.output}} and recommendations: {{steps.Generate Recommendations.output}}.".to_string(),
                    depends_on: vec!["Generate Recommendations".to_string()],
                    agent: Some("reviewer".to_string()),
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
                    depends_on: vec![],
                    agent: Some("debugger".to_string()),
                },
                SopStep {
                    name: "Investigate Root Cause".to_string(),
                    description: "Suggests query commands or checks to run".to_string(),
                    prompt_template: "Based on failure signature: {{steps.Triage Alert.output}}, propose the root cause of this incident and describe what files, ports, or databases we should inspect to fix it.".to_string(),
                    depends_on: vec!["Triage Alert".to_string()],
                    agent: Some("debugger".to_string()),
                },
                SopStep {
                    name: "Formulate Mitigation".to_string(),
                    description: "Creates a step-by-step resolution plan".to_string(),
                    prompt_template: "Write a step-by-step resolution plan to address root cause: {{steps.Investigate Root Cause.output}}. Include verification commands to check if the fix was successful.".to_string(),
                    depends_on: vec!["Investigate Root Cause".to_string()],
                    agent: Some("debugger".to_string()),
                },
            ],
        },
        SopDefinition {
            id: "feature-release".to_string(),
            name: "Feature Release & Security Audit SOP".to_string(),
            description: "Orchestrates schema design, security auditing, baseline implementation, and final compilation validation".to_string(),
            steps: vec![
                SopStep {
                    name: "Audit Schema".to_string(),
                    description: "Designs or reviews database schemas and directory structure".to_string(),
                    prompt_template: "Review the following proposed feature request: {{payload.feature_request}}. Design the database schema and layout files.".to_string(),
                    depends_on: vec![],
                    agent: Some("architect".to_string()),
                },
                SopStep {
                    name: "Security Verification".to_string(),
                    description: "Scans proposed schema and designs for vulnerabilities".to_string(),
                    prompt_template: "Based on the proposed schema: {{steps.Audit Schema.output}}, identify potential security concerns or validation gaps. Propose mitigation practices.".to_string(),
                    depends_on: vec!["Audit Schema".to_string()],
                    agent: Some("code_auditor".to_string()),
                },
                SopStep {
                    name: "Compile Implementation".to_string(),
                    description: "Drafts baseline implementation code".to_string(),
                    prompt_template: "Based on audited designs: {{steps.Security Verification.output}}, write code files conforming to specifications.".to_string(),
                    depends_on: vec!["Security Verification".to_string()],
                    agent: Some("code_synthesizer".to_string()),
                },
                SopStep {
                    name: "Verify Build".to_string(),
                    description: "Runs compilation checks and verifies output is correct".to_string(),
                    prompt_template: "Compile the implemented code from {{steps.Compile Implementation.output}} and execute checks to verify no type or import errors exist.".to_string(),
                    depends_on: vec!["Compile Implementation".to_string()],
                    agent: Some("openz_coordinator".to_string()),
                },
            ],
        },
        SopDefinition {
            id: "ship-pr-until-green".to_string(),
            name: "Ship PR Until Green SOP (from loops!)".to_string(),
            description: "Closed-loop git workflow that implements features on a branch, opens a PR, verifies CI status, and self-heals fixes until all remote tests pass.".to_string(),
            steps: vec![
                SopStep {
                    name: "Implement & Verify Locally".to_string(),
                    description: "Write modifications to code and run local test suite until green".to_string(),
                    prompt_template: "A feature request has been received: {{payload.feature_request}}. Work on the branch and make sure local tests run green.".to_string(),
                    depends_on: vec![],
                    agent: Some("openz_coordinator".to_string()),
                },
                SopStep {
                    name: "Push & Open PR".to_string(),
                    description: "Staged changes commit, push branch to remote, and open a PR using GitHub CLI".to_string(),
                    prompt_template: "Stage and commit the changes from {{steps.Implement & Verify Locally.output}}, push the branch, and open a PR using github CLI.".to_string(),
                    depends_on: vec!["Implement & Verify Locally".to_string()],
                    agent: Some("git_ops_agent".to_string()),
                },
                SopStep {
                    name: "Verify CI Checks".to_string(),
                    description: "Monitor GitHub Actions or remote CI checks, healing and committing fixes if checks fail".to_string(),
                    prompt_template: "Monitor CI checks for PR created in {{steps.Push & Open PR.output}}. Run 'gh pr checks'. If any check fails, inspect the log, self-heal via code edit tools, commit and push the fix, then check again. Loop until green.".to_string(),
                    depends_on: vec!["Push & Open PR".to_string()],
                    agent: Some("openz_coordinator".to_string()),
                },
            ],
        },
        SopDefinition {
            id: "pre-commit-guard".to_string(),
            name: "Pre-Commit Guard SOP (from loops!)".to_string(),
            description: "Hardened git commit hook loop that runs test suite before every commit to block broken changes".to_string(),
            steps: vec![
                SopStep {
                    name: "Configure Pre-Commit Hook".to_string(),
                    description: "Sets up a git hook script to execute test suite before git commit".to_string(),
                    prompt_template: "Create a pre-commit git hook script inside .git/hooks/pre-commit that automatically runs 'cargo test' (or the workspace test suite). Ensure it blocks the commit if tests exit non-zero.".to_string(),
                    depends_on: vec![],
                    agent: Some("git_ops_agent".to_string()),
                },
                SopStep {
                    name: "Verify Hook Loop".to_string(),
                    description: "Simulates a commit with tests passing to verify the hook is functional".to_string(),
                    prompt_template: "Verify the pre-commit hook is active and properly blocks broken code. Report hook installation status.".to_string(),
                    depends_on: vec!["Configure Pre-Commit Hook".to_string()],
                    agent: Some("git_ops_agent".to_string()),
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

        crate::config::loader::CONFIG_DIR_OVERRIDE.scope(temp_dir.clone(), async move {
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
        }).await;
    }

    #[test]
    fn test_sop_step_serialization() {
        let json_str = r#"
        {
            "name": "Step A",
            "description": "First step",
            "prompt_template": "Run A",
            "depends_on": ["Step B", "Step C"],
            "agent": "researcher"
        }
        "#;
        let step: SopStep = serde_json::from_str(json_str).unwrap();
        assert_eq!(step.name, "Step A");
        assert_eq!(step.depends_on, vec!["Step B".to_string(), "Step C".to_string()]);
        assert_eq!(step.agent, Some("researcher".to_string()));

        // Test default value when field is missing
        let json_str_missing = r#"
        {
            "name": "Step A",
            "description": "First step",
            "prompt_template": "Run A"
        }
        "#;
        let step_missing: SopStep = serde_json::from_str(json_str_missing).unwrap();
        assert!(step_missing.depends_on.is_empty());
        assert_eq!(step_missing.agent, None);
    }

    #[test]
    fn test_validate_sop_definition_valid() {
        let def = SopDefinition {
            id: "valid-sop".to_string(),
            name: "Valid SOP".to_string(),
            description: "A valid SOP".to_string(),
            steps: vec![
                SopStep {
                    name: "A".to_string(),
                    description: "Step A".to_string(),
                    prompt_template: "Run A".to_string(),
                    depends_on: vec![],
                    agent: None,
                },
                SopStep {
                    name: "B".to_string(),
                    description: "Step B".to_string(),
                    prompt_template: "Run B".to_string(),
                    depends_on: vec!["A".to_string()],
                    agent: None,
                },
            ],
        };
        assert!(validate_sop_definition(&def).is_ok());
    }

    #[test]
    fn test_validate_sop_definition_cycle() {
        let def = SopDefinition {
            id: "cycle-sop".to_string(),
            name: "Cycle SOP".to_string(),
            description: "A circular SOP".to_string(),
            steps: vec![
                SopStep {
                    name: "A".to_string(),
                    description: "Step A".to_string(),
                    prompt_template: "Run A".to_string(),
                    depends_on: vec!["B".to_string()],
                    agent: None,
                },
                SopStep {
                    name: "B".to_string(),
                    description: "Step B".to_string(),
                    prompt_template: "Run B".to_string(),
                    depends_on: vec!["A".to_string()],
                    agent: None,
                },
            ],
        };
        let err = validate_sop_definition(&def).unwrap_err().to_string();
        assert!(err.contains("Circular dependency"));
    }

    #[test]
    fn test_validate_sop_definition_missing_dep() {
        let def = SopDefinition {
            id: "missing-dep-sop".to_string(),
            name: "Missing Dep SOP".to_string(),
            description: "A missing dep SOP".to_string(),
            steps: vec![
                SopStep {
                    name: "A".to_string(),
                    description: "Step A".to_string(),
                    prompt_template: "Run A".to_string(),
                    depends_on: vec!["C".to_string()],
                    agent: None,
                },
            ],
        };
        let err = validate_sop_definition(&def).unwrap_err().to_string();
        assert!(err.contains("depends on non-existent step"));
    }

    #[test]
    fn test_validate_sop_definition_duplicate_name() {
        let def = SopDefinition {
            id: "dup-sop".to_string(),
            name: "Dup SOP".to_string(),
            description: "A duplicate step name SOP".to_string(),
            steps: vec![
                SopStep {
                    name: "A".to_string(),
                    description: "Step A1".to_string(),
                    prompt_template: "Run A1".to_string(),
                    depends_on: vec![],
                    agent: None,
                },
                SopStep {
                    name: "A".to_string(),
                    description: "Step A2".to_string(),
                    prompt_template: "Run A2".to_string(),
                    depends_on: vec![],
                    agent: None,
                },
            ],
        };
        let err = validate_sop_definition(&def).unwrap_err().to_string();
        assert!(err.contains("Duplicate step name"));
    }

    #[tokio::test]
    async fn test_trigger_sop_simulation() {
        let temp_dir = std::env::temp_dir().join(format!("openz_sop_sim_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        crate::config::loader::CONFIG_DIR_OVERRIDE.scope(temp_dir.clone(), async move {
            initialize_sop_system().unwrap();

            let config = crate::config::schema::Config::default();
            let payload = serde_json::json!({
                "feature_request": "Implement SOP simulator"
            });

            // Trigger simulation for feature-release SOP
            let result = engine::trigger_sop_simulation(config, "feature-release".to_string(), payload).await;
            assert!(result.is_ok());
            
            let sim_id = result.unwrap();
            assert!(sim_id.starts_with("sim-"));

            // Load the simulated instance to verify it completed
            let inst = load_instance(&sim_id).unwrap();
            assert_eq!(inst.status, SopStatus::Completed);
            assert_eq!(inst.steps.len(), 4);
            for step in inst.steps {
                assert_eq!(step.status, "Completed");
                let output = step.output.unwrap();
                assert!(output.contains("[Simulated Output for Step:"));
            }

            std::fs::remove_dir_all(&temp_dir).unwrap();
        }).await;
    }
}
