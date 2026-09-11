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
    assert_eq!(
        substituted,
        "Repo is openz-rs and issue is #42. Result: No issues found."
    );
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
    let temp_dir =
        std::env::temp_dir().join(format!("openz_sop_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    crate::config::loader::CONFIG_DIR_OVERRIDE
        .scope(temp_dir.clone(), async move {
            initialize_sop_system().unwrap();

            // Check default definitions are created
            let defs = load_definitions().unwrap();
            assert!(!defs.is_empty());
            let pr_review = defs.iter().find(|d| d.id == "pr-review").unwrap();
            assert_eq!(pr_review.steps.len(), 3);

            // Save a mock instance
            let steps = pr_review
                .steps
                .iter()
                .map(|step| StepExecutionState {
                    name: step.name.clone(),
                    status: "Pending".to_string(),
                    started_at: None,
                    completed_at: None,
                    output: None,
                    error: None,
                })
                .collect();

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
        })
        .await;
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
    assert_eq!(
        step.depends_on,
        vec!["Step B".to_string(), "Step C".to_string()]
    );
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
        steps: vec![SopStep {
            name: "A".to_string(),
            description: "Step A".to_string(),
            prompt_template: "Run A".to_string(),
            depends_on: vec!["C".to_string()],
            agent: None,
        }],
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

    crate::config::loader::CONFIG_DIR_OVERRIDE
        .scope(temp_dir.clone(), async move {
            initialize_sop_system().unwrap();

            let config = crate::config::schema::Config::default();
            let payload = serde_json::json!({
                "feature_request": "Implement SOP simulator"
            });

            // Trigger simulation for feature-release SOP
            let result =
                engine::trigger_sop_simulation(config, "feature-release".to_string(), payload)
                    .await;
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
        })
        .await;
}
