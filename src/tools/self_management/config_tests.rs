use super::*;
use crate::tools::Tool;

#[test]
fn redact_secrets_handles_camel_case_and_nested_values() {
    let mut value = serde_json::json!({
        "api_key": "snake-secret",
        "apiKey": "camel-secret",
        "nested": [{
            "botToken": "bot-secret",
            "phone_number_id": "not-a-secret"
        }],
        "model": "openai/gpt-4o"
    });

    redact_secrets(&mut value);

    assert_eq!(value["api_key"], "********");
    assert_eq!(value["apiKey"], "********");
    assert_eq!(value["nested"][0]["botToken"], "********");
    assert_eq!(value["nested"][0]["phone_number_id"], "not-a-secret");
    assert_eq!(value["model"], "openai/gpt-4o");
}

struct TestEnvLock;

impl TestEnvLock {
    fn acquire() -> Self {
        let lock_path = std::env::temp_dir().join("openz_test_config_dir.lock");
        loop {
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&lock_path)
            {
                Ok(_) => break,
                Err(_) => std::thread::sleep(std::time::Duration::from_millis(50)),
            }
        }
        TestEnvLock
    }
}

impl Drop for TestEnvLock {
    fn drop(&mut self) {
        let lock_path = std::env::temp_dir().join("openz_test_config_dir.lock");
        let _ = std::fs::remove_file(lock_path);
    }
}

#[test]
fn test_manage_config() {
    let _env_lock = TestEnvLock::acquire();
    let previous_config_dir = std::env::var("OPENZ_CONFIG_DIR").ok();
    let openz_dir =
        std::env::temp_dir().join(format!("openz_manage_config_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&openz_dir).unwrap();
    std::env::set_var("OPENZ_CONFIG_DIR", &openz_dir);

    let tool = ManageConfigTool;
    let rt = tokio::runtime::Runtime::new().unwrap();

    // Save original config
    let original_config = crate::config::loader::load_config().unwrap();

    // 1. View config
    let view_res = rt
        .block_on(tool.call(&serde_json::json!({
            "action": "view"
        })))
        .unwrap();
    assert!(view_res["success"].as_bool().unwrap());
    // Verify redacted api_key / secret key format if they exist
    let config_val = &view_res["config"];
    if let Some(providers) = config_val.get("providers") {
        if let Some(openai) = providers.get("openai") {
            if let Some(api_key) = openai.get("api_key") {
                if api_key.is_string() {
                    assert_eq!(api_key.as_str().unwrap(), "********");
                }
            }
        }
    }

    // 2. Update config hyperparameters
    let update_res = rt
        .block_on(tool.call(&serde_json::json!({
            "action": "update",
            "updates": {
                "max_tokens": 1234,
                "temperature": 0.25,
                "caveman_mode": false,
                "streaming": false,
                "tui_thought_display": "compact",
                "min_free_disk_gb": 3.5,
                "allow_network_tools": false,
                "max_concurrent_process_tools": 2,
                "warn_before_expensive_tools": false,
                "skills_workspace_skills_enabled": false,
                "skills_external_dirs": ["~/.agents/skills", "/tmp/openz-team-skills"],
                "skills_write_approval": true
            }
        })))
        .unwrap();
    assert!(update_res["success"].as_bool().unwrap());

    // 3. Verify they were updated and saved
    let updated_config = crate::config::loader::load_config().unwrap();
    assert_eq!(updated_config.agents.defaults.max_tokens, 1234);
    assert_eq!(updated_config.agents.defaults.temperature, 0.25f32);
    assert_eq!(updated_config.agents.defaults.caveman_mode, false);
    assert_eq!(updated_config.agents.defaults.streaming, false);
    assert_eq!(
        updated_config.agents.defaults.tui_thought_display,
        "compact"
    );
    assert_eq!(updated_config.agents.defaults.min_free_disk_gb, 3.5);
    assert_eq!(updated_config.agents.defaults.allow_network_tools, false);
    assert_eq!(
        updated_config.agents.defaults.max_concurrent_process_tools,
        2
    );
    assert_eq!(
        updated_config.agents.defaults.warn_before_expensive_tools,
        false
    );
    assert_eq!(updated_config.skills.workspace_skills_enabled, false);
    assert_eq!(
        updated_config.skills.external_dirs,
        vec![
            "~/.agents/skills".to_string(),
            "/tmp/openz-team-skills".to_string()
        ]
    );
    assert_eq!(updated_config.skills.write_approval, true);

    // 4. Store credentials through manage_config and verify views redact them.
    let credential_res = rt
        .block_on(tool.call(&serde_json::json!({
            "action": "set_credential",
            "credential": {
                "target": "github",
                "token": "github_pat_secret_for_test",
                "api_base": "https://api.github.com",
                "token_env": "OPENZ_TEST_GITHUB_TOKEN"
            }
        })))
        .unwrap();
    assert!(credential_res["success"].as_bool().unwrap());

    let credential_config = crate::config::loader::load_config().unwrap();
    let github_config = credential_config.integrations.github.unwrap();
    assert_eq!(
        github_config.token.as_deref(),
        Some("github_pat_secret_for_test")
    );
    assert_eq!(
        github_config.api_base.as_deref(),
        Some("https://api.github.com")
    );
    assert_eq!(
        github_config.token_env.as_deref(),
        Some("OPENZ_TEST_GITHUB_TOKEN")
    );

    let redacted_view = rt
        .block_on(tool.call(&serde_json::json!({
            "action": "view"
        })))
        .unwrap();
    assert_eq!(
        redacted_view["config"]["integrations"]["github"]["token"],
        "********"
    );

    // 5. Try updating an invalid/restricted field (should be blocked)
    let invalid_res = rt
        .block_on(tool.call(&serde_json::json!({
            "action": "update",
            "updates": {
                "invalid_field": "some_value"
            }
        })))
        .unwrap();
    assert_eq!(invalid_res["success"].as_bool().unwrap(), false);
    assert!(invalid_res["error"]
        .as_str()
        .unwrap()
        .contains("invalid_field"));

    // Restore original config
    crate::config::loader::save_config(&original_config).unwrap();

    if let Some(prev) = previous_config_dir {
        std::env::set_var("OPENZ_CONFIG_DIR", prev);
    } else {
        std::env::remove_var("OPENZ_CONFIG_DIR");
    }
    let _ = std::fs::remove_dir_all(&openz_dir);
}

#[test]
fn test_manage_config_coercion_and_aliases() {
    let _env_lock = TestEnvLock::acquire();
    let previous_config_dir = std::env::var("OPENZ_CONFIG_DIR").ok();
    let openz_dir =
        std::env::temp_dir().join(format!("openz_manage_config_coercion_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&openz_dir).unwrap();
    std::env::set_var("OPENZ_CONFIG_DIR", &openz_dir);

    let tool = ManageConfigTool;
    let rt = tokio::runtime::Runtime::new().unwrap();

    let original_config = crate::config::loader::load_config().unwrap();

    // 1. Alias "SHOW"
    let view_res = rt
        .block_on(tool.call(&serde_json::json!({
            "action": "SHOW"
        })))
        .unwrap();
    assert!(view_res["success"].as_bool().unwrap());

    // 2. Alias "set" with string coercion for numbers and booleans
    let update_res = rt
        .block_on(tool.call(&serde_json::json!({
            "action": "set",
            "updates": {
                "max_tokens": "2048",
                "temperature": "0.35",
                "caveman_mode": "true",
                "streaming": "false",
                "firefox_webdriver_port": "4545",
                "min_free_disk_gb": "4.2"
            }
        })))
        .unwrap();
    assert!(update_res["success"].as_bool().unwrap());

    let updated = crate::config::loader::load_config().unwrap();
    assert_eq!(updated.agents.defaults.max_tokens, 2048);
    assert!((updated.agents.defaults.temperature - 0.35f32).abs() < 1e-4);
    assert_eq!(updated.agents.defaults.caveman_mode, true);
    assert_eq!(updated.agents.defaults.streaming, false);
    assert_eq!(updated.browser.firefox_webdriver_port, 4545);
    assert!((updated.agents.defaults.min_free_disk_gb - 4.2).abs() < 1e-4);

    // Restore original config
    crate::config::loader::save_config(&original_config).unwrap();

    if let Some(prev) = previous_config_dir {
        std::env::set_var("OPENZ_CONFIG_DIR", prev);
    } else {
        std::env::remove_var("OPENZ_CONFIG_DIR");
    }
    let _ = std::fs::remove_dir_all(&openz_dir);
}
