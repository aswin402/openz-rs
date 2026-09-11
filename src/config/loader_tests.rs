use crate::config::schema::Config;

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
fn cli_session_key_changes_with_current_directory() {
    let original = std::env::current_dir().unwrap();
    let first =
        std::env::temp_dir().join(format!("openz_session_key_a_{}", uuid::Uuid::new_v4()));
    let second =
        std::env::temp_dir().join(format!("openz_session_key_b_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&first).unwrap();
    std::fs::create_dir_all(&second).unwrap();

    std::env::set_current_dir(&first).unwrap();
    let first_key = super::get_cli_session_key();
    std::env::set_current_dir(&second).unwrap();
    let second_key = super::get_cli_session_key();
    std::env::set_current_dir(original).unwrap();

    let _ = std::fs::remove_dir_all(first);
    let _ = std::fs::remove_dir_all(second);

    assert_ne!(first_key, second_key);
    assert!(first_key.starts_with("cli:"));
    assert!(second_key.starts_with("cli:"));
}

#[tokio::test]
async fn runtime_db_path_never_resolves_to_workspace_root() {
    let workspace = std::env::temp_dir().join(format!("openz_ws_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&workspace).unwrap();

    // runtime_db_path must be derived from ~/.openz, independent of the
    // active workspace task-local, so it can never land in the repo root.
    let path = super::ACTIVE_WORKSPACE
        .scope(workspace.clone(), async {
            super::runtime_db_path("memory.db")
        })
        .await;

    assert!(
        !path.starts_with(&workspace),
        "runtime DB resolved inside workspace: {path:?}"
    );
    assert!(path.ends_with("memory.db"));
    assert!(path.to_string_lossy().contains(".openz"));
    let _ = std::fs::remove_dir_all(&workspace);
}

#[tokio::test]
async fn active_workspace_or_current_dir_prefers_task_local_workspace() {
    let workspace =
        std::env::temp_dir().join(format!("openz_active_ws_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&workspace).unwrap();

    let resolved = super::ACTIVE_WORKSPACE
        .scope(workspace.clone(), async {
            super::active_workspace_or_current_dir()
        })
        .await;

    assert_eq!(resolved, workspace);
    let _ = std::fs::remove_dir_all(workspace);
}

#[test]
fn workspace_for_agent_turn_uses_cwd_for_default_placeholder() {
    let original = std::env::current_dir().unwrap();
    let workspace =
        std::env::temp_dir().join(format!("openz_turn_ws_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&workspace).unwrap();
    std::env::set_current_dir(&workspace).unwrap();

    let config = Config::default();
    let resolved = super::workspace_for_agent_turn(&config);

    std::env::set_current_dir(original).unwrap();
    let _ = std::fs::remove_dir_all(&workspace);
    assert_eq!(resolved, workspace);
}

#[test]
fn openz_config_dir_moves_db_location() {
    let _env_lock = TestEnvLock::acquire();
    let custom = std::env::temp_dir().join(format!("openz_cfg_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&custom).unwrap();
    std::env::set_var("OPENZ_CONFIG_DIR", &custom);

    let path = super::runtime_db_path("memory.db");

    assert_eq!(path, custom.join("memory.db"));
    std::env::remove_var("OPENZ_CONFIG_DIR");
    let _ = std::fs::remove_dir_all(&custom);
}

#[test]
fn root_memory_db_triggers_diagnostic() {
    let root = std::env::temp_dir().join(format!("openz_root_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("memory.db"), b"stale").unwrap();
    std::fs::write(root.join("memory.db-shm"), b"").unwrap();
    std::fs::write(root.join("memory.db-wal"), b"").unwrap();

    let found = super::root_runtime_db_artifacts(&root);
    let names: Vec<String> = found
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
        .collect();

    assert!(
        names.contains(&"memory.db".to_string()),
        "memory.db not detected: {names:?}"
    );
    assert!(names.contains(&"memory.db-shm".to_string()));
    assert!(names.contains(&"memory.db-wal".to_string()));

    let _ = std::fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[tokio::test]
async fn save_config_temp_write_uses_private_final_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let dir = std::env::temp_dir().join(format!("openz_cfg_perm_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    super::CONFIG_DIR_OVERRIDE
        .scope(dir.clone(), async {
            super::save_config(&crate::config::schema::Config::default()).unwrap();
        })
        .await;

    let mode = std::fs::metadata(dir.join("config.json"))
        .unwrap()
        .permissions()
        .mode()
        & 0o777;

    assert_eq!(mode, 0o600);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn load_config_migrates_historical_tool_timeout_default() {
    let dir = std::env::temp_dir().join(format!(
        "openz_cfg_timeout_migration_{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("config.json"),
        serde_json::json!({
            "agents": { "defaults": { "toolTimeoutSecs": 120 } }
        })
        .to_string(),
    )
    .unwrap();

    let config = super::CONFIG_DIR_OVERRIDE
        .scope(dir.clone(), async { super::load_config().unwrap() })
        .await;

    assert_eq!(config.agents.defaults.tool_timeout_secs, 300);
    let saved: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("config.json")).unwrap())
            .unwrap();
    assert_eq!(saved["agents"]["defaults"]["toolTimeoutSecs"], 300);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn load_config_preserves_custom_tool_timeout() {
    let dir =
        std::env::temp_dir().join(format!("openz_cfg_timeout_custom_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("config.json"),
        serde_json::json!({
            "agents": { "defaults": { "toolTimeoutSecs": 60 } }
        })
        .to_string(),
    )
    .unwrap();

    let config = super::CONFIG_DIR_OVERRIDE
        .scope(dir.clone(), async { super::load_config().unwrap() })
        .await;

    assert_eq!(config.agents.defaults.tool_timeout_secs, 60);
    let saved: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("config.json")).unwrap())
            .unwrap();
    assert_eq!(saved["agents"]["defaults"]["toolTimeoutSecs"], 60);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn load_config_rewrites_legacy_aliases_to_canonical_schema() {
    let dir = std::env::temp_dir().join(format!(
        "openz_cfg_alias_migration_{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("config.json"),
        serde_json::json!({
            "agents": {
                "defaults": {
                    "max_tokens": 1234,
                    "enable_sandbox": true,
                    "tool_timeout_secs": 77,
                    "show_tool_router_status": true
                }
            },
            "skills": {
                "workspace_skills_enabled": false,
                "external_dirs": ["/tmp/skills"],
                "write_approval": true
            },
            "mcp_servers": {
                "memory": { "command": "old-memory", "args": [], "enabled": true }
            }
        })
        .to_string(),
    )
    .unwrap();

    let config = super::CONFIG_DIR_OVERRIDE
        .scope(dir.clone(), async { super::load_config().unwrap() })
        .await;

    assert_eq!(config.agents.defaults.max_tokens, 1234);
    assert!(config.agents.defaults.enable_sandbox);
    assert_eq!(config.agents.defaults.tool_timeout_secs, 77);
    assert!(config.agents.defaults.show_tool_router_status);
    assert!(!config.skills.workspace_skills_enabled);
    assert_eq!(config.skills.external_dirs, vec!["/tmp/skills".to_string()]);
    assert!(config.skills.write_approval);
    assert!(!config.mcp_servers.contains_key("memory"));

    let saved: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("config.json")).unwrap())
            .unwrap();
    let defaults = &saved["agents"]["defaults"];
    assert_eq!(defaults["maxTokens"], 1234);
    assert_eq!(defaults["enableSandbox"], true);
    assert_eq!(defaults["toolTimeoutSecs"], 77);
    assert_eq!(defaults["showToolRouterStatus"], true);
    assert!(defaults.get("max_tokens").is_none());
    assert!(defaults.get("enable_sandbox").is_none());
    assert!(defaults.get("tool_timeout_secs").is_none());
    assert!(defaults.get("show_tool_router_status").is_none());

    assert_eq!(saved["skills"]["workspaceSkillsEnabled"], false);
    assert_eq!(saved["skills"]["externalDirs"][0], "/tmp/skills");
    assert_eq!(saved["skills"]["writeApproval"], true);
    assert!(saved["skills"].get("workspace_skills_enabled").is_none());
    assert!(saved["skills"].get("external_dirs").is_none());
    assert!(saved["skills"].get("write_approval").is_none());
    assert!(saved["mcp_servers"].get("memory").is_none());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn workspace_openz_runtime_files_are_gitignored() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let gitignore = std::fs::read_to_string(format!("{manifest}/.gitignore"))
        .expect(".gitignore must exist at repo root");
    let lower = gitignore.to_lowercase();
    assert!(
        lower.contains(".openz/"),
        ".gitignore must ignore workspace .openz/ runtime dir"
    );
    assert!(
        lower.contains("/memory.db"),
        ".gitignore must ignore root memory.db"
    );
    assert!(
        lower.contains(".db-wal"),
        ".gitignore must ignore sqlite -wal companions"
    );
}

#[tokio::test]
async fn test_config_caching_with_modification_time() {
    let dir =
        std::env::temp_dir().join(format!("openz_cfg_cache_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();

    // 1. Initial load constructs defaults
    let config1 = super::CONFIG_DIR_OVERRIDE
        .scope(dir.clone(), async { super::load_config().unwrap() })
        .await;

    // 2. Load again, must hit cache
    let config2 = super::CONFIG_DIR_OVERRIDE
        .scope(dir.clone(), async { super::load_config().unwrap() })
        .await;
    assert_eq!(
        config1.agents.defaults.tool_timeout_secs,
        config2.agents.defaults.tool_timeout_secs
    );

    // 3. Write directly to file (simulating external edit)
    std::thread::sleep(std::time::Duration::from_millis(50));
    let mut modified_config = config1.clone();
    modified_config.agents.defaults.tool_timeout_secs = 42;
    let content = serde_json::to_string_pretty(&modified_config).unwrap();
    std::fs::write(dir.join("config.json"), content).unwrap();

    // 4. Load again, must detect modification and load updated value
    let config3 = super::CONFIG_DIR_OVERRIDE
        .scope(dir.clone(), async { super::load_config().unwrap() })
        .await;
    assert_eq!(config3.agents.defaults.tool_timeout_secs, 42);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_runtime_data_path_helpers() {
    let base = super::runtime_data_dir();
    assert_eq!(super::sessions_dir(), base.join("sessions"));
    assert_eq!(super::skills_dir(), base.join("skills"));
    assert_eq!(super::traces_dir(), base.join("traces"));
    assert_eq!(super::tool_outputs_dir(), base.join("tool_outputs"));
    assert_eq!(super::cron_logs_dir(), base.join("cron_logs"));
    assert_eq!(super::subagents_file(), base.join("subagents.json"));
    assert_eq!(super::activity_file(), base.join("activity.json"));
}
