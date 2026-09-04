//! Configuration, diagnostics, discovery, and maintenance tools for OpenZ.

#[cfg(test)]
use crate::tools::Tool;
#[cfg(test)]
use anyhow::Result;

mod backups;
mod sessions;
mod config;
mod diagnostics;
mod catalog;
mod scope;
mod inventory;
mod skills;
pub(crate) use skills::CurateSkillTool;
pub use inventory::OpenZInventoryTool;
pub use scope::{OptimizeToolScopeTool, RequestToolScopeTool};
pub use catalog::ToolCatalogTool;
pub use diagnostics::{DiagnoseSystemTool, DiagnoseToolTool};
#[cfg(test)]
pub(crate) use diagnostics::normalize_diagnose_mock_args;
pub(crate) use config::ManageConfigTool;
#[cfg(test)]
pub(crate) use config::redact_secrets;
pub use backups::ManageBackupsTool;
pub use sessions::ManageSessionsTool;

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_tool_catalog_reports_metadata_and_exposure() {
        let registry = crate::tools::ToolRegistry::new();
        registry.register(std::sync::Arc::new(crate::tools::shell::ExecCommandTool));
        registry.register(std::sync::Arc::new(crate::tools::filesystem::ReadFileTool));

        let catalog = ToolCatalogTool::new(registry);
        let res = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(catalog.call(&serde_json::json!({
                "include_schema": false,
                "prompt": "run cargo test and inspect files"
            })))
            .unwrap();

        assert_eq!(res["success"].as_bool().unwrap(), true);
        assert_eq!(res["tool_count"].as_u64().unwrap(), 2);
        assert_eq!(res["exposed_count"].as_u64().unwrap(), 2);

        let tools = res["tools"].as_array().unwrap();
        let exec = tools
            .iter()
            .find(|tool| tool["name"].as_str() == Some("exec_command"))
            .expect("exec_command entry");
        assert_eq!(exec["domain"].as_str().unwrap(), "shell");
        assert_eq!(exec["risk"].as_str().unwrap(), "high");
        assert_eq!(exec["spawns_process"].as_bool().unwrap(), true);
        assert_eq!(exec["requires_approval"].as_bool().unwrap(), true);
        assert_eq!(exec["matched_prompt_domain"].as_bool().unwrap(), true);
        assert!(exec["selection_reason"]
            .as_str()
            .unwrap()
            .contains("prompt_domain"));
        assert!(exec["selected_score"].as_i64().unwrap() > 0);
        assert!(exec["aliases"]
            .as_array()
            .unwrap()
            .iter()
            .any(|alias| alias.as_str() == Some("shell command")));
        assert!(exec["examples"].as_array().unwrap().iter().any(|example| {
            example
                .as_str()
                .unwrap_or("")
                .contains("safe project-local command")
        }));
        assert!(exec["when_to_use"]
            .as_str()
            .unwrap()
            .contains("shell commands"));
        assert!(exec["when_not_to_use"]
            .as_str()
            .unwrap()
            .contains("file reads"));

        let selected_domains = res["selected_domains"].as_array().unwrap();
        assert!(selected_domains.iter().any(|d| d.as_str() == Some("shell")));

        let read_file = tools
            .iter()
            .find(|tool| tool["name"].as_str() == Some("read_file"))
            .expect("read_file entry");
        assert_eq!(read_file["domain"].as_str().unwrap(), "filesystem");
        assert_eq!(read_file["risk"].as_str().unwrap(), "low");
        assert_eq!(read_file["writes_disk"].as_bool().unwrap(), false);
    }

    #[test]
    fn test_openz_inventory_reports_runtime_identity() {
        let registry = crate::tools::ToolRegistry::new();
        registry.register(std::sync::Arc::new(
            crate::tools::self_management::ManageConfigTool,
        ));

        let inventory = OpenZInventoryTool::new(registry);
        let res = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(inventory.call(&serde_json::json!({
                "include_tools": false,
                "include_subagents": false,
                "prompt": "what model are you and which language are you best at"
            })))
            .unwrap();

        assert_eq!(res["success"].as_bool().unwrap(), true);
        assert!(res["runtime_identity"].is_object());
        assert!(res["runtime_identity"]["configured_model"]
            .as_str()
            .is_some());
        assert!(res["runtime_identity"]["configured_provider"]
            .as_str()
            .is_some());
        assert!(res["runtime_identity"]["model_supports_vision"]
            .as_bool()
            .is_some());
        assert!(res["guidance"]
            .as_str()
            .unwrap()
            .contains("model/provider identity"));
    }

    #[test]
    fn test_tool_catalog_reports_resource_policy_visibility() {
        struct NetworkTool;
        #[async_trait::async_trait]
        impl Tool for NetworkTool {
            fn name(&self) -> &str {
                "web_fetch"
            }
            fn description(&self) -> &str {
                "Fetch a web page"
            }
            fn parameters(&self) -> serde_json::Value {
                serde_json::json!({})
            }
            async fn call(&self, _args: &serde_json::Value) -> Result<serde_json::Value> {
                Ok(serde_json::json!({"ok": true}))
            }
        }

        let registry = crate::tools::ToolRegistry::new();
        registry.register(std::sync::Arc::new(NetworkTool));

        let catalog = ToolCatalogTool::new(registry);
        let res = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(catalog.call(&serde_json::json!({
                "prompt": "fetch this webpage",
                "resource_overrides": {
                    "allow_network_tools": false,
                    "free_disk_gb": 100.0,
                    "active_process_tools": 0
                }
            })))
            .unwrap();

        let tools = res["tools"].as_array().unwrap();
        let web_fetch = tools
            .iter()
            .find(|tool| tool["name"].as_str() == Some("web_fetch"))
            .expect("web_fetch entry");

        assert_eq!(
            web_fetch["resource_policy"]["decision"].as_str(),
            Some("block")
        );
        assert!(web_fetch["resource_policy"]["reason"]
            .as_str()
            .unwrap()
            .contains("Network tools are disabled"));
        assert_eq!(
            web_fetch["resource_policy"]["free_disk_gb"].as_f64(),
            Some(100.0)
        );
        assert_eq!(
            web_fetch["resource_policy"]["active_process_tools"].as_u64(),
            Some(0)
        );
    }

    #[test]
    fn test_diagnose_openmedia_video_args_use_minimal_scene_for_placeholder() {
        let create_args = normalize_diagnose_mock_args(
            "openmedia_video_create",
            serde_json::json!({ "test": true }),
        );
        assert_eq!(create_args["scene"]["width"], 640);
        assert!(create_args.get("test").is_none());

        let preview_args =
            normalize_diagnose_mock_args("openmedia_video_preview", serde_json::json!({}));
        assert_eq!(preview_args["scene"]["fps"], 1);
        assert_eq!(preview_args["output_format"], "png");
    }

    #[test]
    fn test_diagnose_openmedia_video_placeholder_is_visible() {
        let args = normalize_diagnose_mock_args("openmedia_video_create", serde_json::json!({}));
        assert_eq!(args["scene"]["background"], "#1e293b");
        assert_eq!(
            args["scene"]["scenes"][0]["elements"][0]["content"],
            "OpenZ"
        );
        assert_eq!(
            args["scene"]["scenes"][0]["elements"][0]["style"]["font_size"],
            48.0
        );
        assert_eq!(
            args["scene"]["scenes"][0]["elements"][0]["style"]["font_weight"],
            800
        );
    }

    #[test]
    fn request_tool_scope_returns_structured_scope_request() {
        let tool = RequestToolScopeTool::new(crate::tools::ToolRegistry::new());
        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(tool.call(&serde_json::json!({
                "reason": "need to inspect repository files",
                "needed_domains": ["code", "filesystem"],
                "needed_tools": ["grep_search", "read_file"]
            })))
            .unwrap();

        assert_eq!(result["status"], "scope_request_recorded");
        assert_eq!(result["needed_domains"][0], "code");
        assert_eq!(result["needed_tools"][0], "grep_search");
    }

    #[test]
    fn test_diagnose_and_optimize_tools() {
        let registry = crate::tools::ToolRegistry::new();
        struct DummyTool;
        #[async_trait::async_trait]
        impl Tool for DummyTool {
            fn name(&self) -> &str {
                "dummy_tool"
            }
            fn description(&self) -> &str {
                "dummy"
            }
            fn parameters(&self) -> serde_json::Value {
                serde_json::json!({})
            }
            async fn call(&self, _args: &serde_json::Value) -> Result<serde_json::Value> {
                Ok(serde_json::json!({ "ok": true }))
            }
        }

        let dummy = std::sync::Arc::new(DummyTool);
        registry.register(dummy.clone());

        let diagnose = DiagnoseToolTool::new(registry.clone());
        let res = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(diagnose.call(&serde_json::json!({ "tool_name": "dummy_tool" })))
            .unwrap();
        assert!(res["success"].as_bool().unwrap());
        assert_eq!(res["output"]["ok"].as_bool().unwrap(), true);

        // Test OptimizeToolScopeTool
        let optimizer = OptimizeToolScopeTool::new(registry.clone());
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(optimizer.call(&serde_json::json!({ "active_prefixes": ["other_"] })))
            .unwrap();

        // DummyTool starts with "dummy_", which does not match prefix filter "other_"
        // It should be filtered out
        assert!(registry.get("dummy_tool").is_none());

        // Restore filter
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(optimizer.call(&serde_json::json!({ "active_prefixes": [] })))
            .unwrap();
        assert!(registry.get("dummy_tool").is_some());
    }

    #[test]
    fn test_curate_skills() {
        // Run database queries through curate_skill tool
        let tool = CurateSkillTool;
        let rt = tokio::runtime::Runtime::new().unwrap();

        // 1. Delete skill if exists
        let _ = rt.block_on(tool.call(&serde_json::json!({
            "action": "delete",
            "skill_name": "test_curate_skills_temp"
        })));

        // 2. Add skill
        let add_res = rt
            .block_on(tool.call(&serde_json::json!({
                "action": "add",
                "skill_name": "test_curate_skills_temp",
                "content": "This is a test skill content"
            })))
            .unwrap();
        assert!(add_res["success"].as_bool().unwrap());

        // 3. List skills and verify
        let list_res = rt
            .block_on(tool.call(&serde_json::json!({
                "action": "list"
            })))
            .unwrap();
        assert!(list_res["success"].as_bool().unwrap());
        let skills = list_res["skills"].as_array().unwrap();
        let found = skills
            .iter()
            .any(|s| s["name"].as_str().unwrap() == "test_curate_skills_temp");
        assert!(found);

        // 4. Delete skill
        let del_res = rt
            .block_on(tool.call(&serde_json::json!({
                "action": "delete",
                "skill_name": "test_curate_skills_temp"
            })))
            .unwrap();
        assert!(del_res["success"].as_bool().unwrap());
    }

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

    #[test]
    fn test_manage_config() {
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
    }

    #[test]
    fn test_diagnose_system() {
        let tool = DiagnoseSystemTool;
        let rt = tokio::runtime::Runtime::new().unwrap();

        // 1. Run diagnostic without latency checking to keep it fast
        let res = rt
            .block_on(tool.call(&serde_json::json!({
                "check_latency": false,
                "check_db_integrity": false
            })))
            .unwrap();

        assert_eq!(res["status"].as_str().unwrap(), "success");
        assert!(res["system"]["os"].as_str().is_some());
        assert!(res["system"]["architecture"].as_str().is_some());
        assert!(res["system"]["cores"].as_u64().unwrap() >= 1);

        // Verify directory statistics keys exist
        assert!(res["directories"]["sessions"].is_object());
        assert!(res["directories"]["tool_outputs"].is_object());
        assert!(res["directories"]["traces"].is_object());
        assert!(res["directories"]["skills"].is_object());

        // Verify databases status keys exist
        assert!(res["databases"]["memory"].is_object());
        assert!(res["databases"]["docs"].is_object());
        assert!(res["databases"]["graph_memory"].is_object());
        assert!(res["databases"]["ccr_cache"].is_object());
        assert!(res["databases"]["thoughts"].is_object());
    }

    #[test]
    fn test_manage_sessions() {
        let _env_lock = TestEnvLock::acquire();
        let previous_config_dir = std::env::var("OPENZ_CONFIG_DIR").ok();
        let openz_dir =
            std::env::temp_dir().join(format!("openz_manage_sessions_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&openz_dir).unwrap();
        std::env::set_var("OPENZ_CONFIG_DIR", &openz_dir);

        let tool = ManageSessionsTool;
        let rt = tokio::runtime::Runtime::new().unwrap();

        let sessions_dir = openz_dir.join("sessions");
        std::fs::create_dir_all(&sessions_dir).unwrap();

        // 1. Create a dummy session file for testing. Use a unique key because
        // the full lib test suite runs session-management tests in parallel.
        let test_session_key = format!("test_session_xyz_{}", uuid::Uuid::new_v4());
        let session_file = sessions_dir.join(format!("{}.json", test_session_key));
        let _ = std::fs::remove_file(&session_file);
        std::fs::write(
            &session_file,
            serde_json::json!({
                "messages": [
                    {
                        "role": "user",
                        "content": "Hello"
                    }
                ]
            })
            .to_string(),
        )
        .unwrap();

        // 2. List sessions and check if our dummy is present
        let list_res = rt
            .block_on(tool.call(&serde_json::json!({
                "action": "list"
            })))
            .unwrap();
        assert_eq!(list_res["status"].as_str().unwrap(), "success");
        let sessions = list_res["sessions"].as_array().unwrap();
        let found = sessions
            .iter()
            .any(|s| s["session_key"].as_str().unwrap() == test_session_key);
        assert!(found);

        // 3. Archive our dummy session
        let archive_res = rt
            .block_on(tool.call(&serde_json::json!({
                "action": "archive",
                "session_key": &test_session_key
            })))
            .unwrap();
        assert_eq!(archive_res["status"].as_str().unwrap(), "success");
        assert!(!session_file.exists());

        // Clean up archived files
        let archives_dir = openz_dir.join("archives");
        if let Ok(entries) = std::fs::read_dir(&archives_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_str().unwrap_or("");
                if name_str.starts_with(&test_session_key) {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }

        // 4. Create another dummy session and delete it
        std::fs::write(&session_file, "{\"messages\": []}").unwrap();
        assert!(session_file.exists());

        let delete_res = rt
            .block_on(tool.call(&serde_json::json!({
                "action": "delete",
                "session_key": &test_session_key
            })))
            .unwrap();
        assert_eq!(delete_res["status"].as_str().unwrap(), "success");
        assert!(!session_file.exists());

        // 5. Test pruning (prune should execute without errors)
        let prune_res = rt
            .block_on(tool.call(&serde_json::json!({
                "action": "prune",
                "older_than_days": 30
            })))
            .unwrap();
        assert_eq!(prune_res["status"].as_str().unwrap(), "success");
        assert!(prune_res["details"]["files_removed"].as_u64().is_some());

        if let Some(prev) = previous_config_dir {
            std::env::set_var("OPENZ_CONFIG_DIR", prev);
        } else {
            std::env::remove_var("OPENZ_CONFIG_DIR");
        }
        let _ = std::fs::remove_dir_all(&openz_dir);
    }

    #[test]
    fn test_manage_backups() {
        let tool = ManageBackupsTool;
        let rt = tokio::runtime::Runtime::new().unwrap();

        // 1. Create a backup
        let create_res = rt
            .block_on(tool.call(&serde_json::json!({
                "action": "create"
            })))
            .unwrap();
        assert_eq!(create_res["status"].as_str().unwrap(), "success");
        let backup_name = create_res["backup_name"].as_str().unwrap();

        // 2. List backups and ensure it exists
        let list_res = rt
            .block_on(tool.call(&serde_json::json!({
                "action": "list"
            })))
            .unwrap();
        assert_eq!(list_res["status"].as_str().unwrap(), "success");
        let backups = list_res["backups"].as_array().unwrap();
        let found = backups
            .iter()
            .any(|b| b["backup_name"].as_str().unwrap() == backup_name);
        assert!(found);

        // 3. Restore from the backup
        let restore_res = rt
            .block_on(tool.call(&serde_json::json!({
                "action": "restore",
                "backup_name": backup_name
            })))
            .unwrap();
        assert_eq!(restore_res["status"].as_str().unwrap(), "success");

        // 4. Delete the backup
        let delete_res = rt
            .block_on(tool.call(&serde_json::json!({
                "action": "delete",
                "backup_name": backup_name
            })))
            .unwrap();
        assert_eq!(delete_res["status"].as_str().unwrap(), "success");

        // 5. Ensure it is gone from the list
        let list_res_2 = rt
            .block_on(tool.call(&serde_json::json!({
                "action": "list"
            })))
            .unwrap();
        let backups_2 = list_res_2["backups"].as_array().unwrap();
        let found_2 = backups_2
            .iter()
            .any(|b| b["backup_name"].as_str().unwrap() == backup_name);
        assert!(!found_2);
    }
}
