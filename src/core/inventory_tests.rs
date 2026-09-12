use super::*;

#[test]
fn runtime_inventory_contains_core_paths_and_counts() {
    let config = Config::default();
    let inventory = build_runtime_inventory(&config, None);

    assert_eq!(inventory.version, env!("CARGO_PKG_VERSION"));
    assert!(inventory.paths.memory_db.ends_with("memory.db"));
    assert!(inventory.paths.graph_db.ends_with("graph_memory.db"));
    assert!(inventory.paths.sessions_dir.ends_with("sessions"));
    assert!(inventory.counts.channels >= 4);
    assert!(
        inventory
            .subagents
            .iter()
            .any(|s| s.name == "orchestrator" && s.is_core)
    );
}

#[test]
fn subagent_runtime_inventory_reports_model_and_fallbacks() {
    let mut config = Config::default();
    config.agents.defaults.model = "opencode_zen/muse-spark-1.2-contributor-free".to_string();
    config.agents.defaults.provider = "opencode_zen".to_string();
    let profiles = vec![crate::subagents::SubagentProfile {
        name: "custom_researcher".to_string(),
        description: "Research agent".to_string(),
        system_prompt: "Research with web tools when needed.".to_string(),
        model: None,
        fallbacks: Some(vec![
            "google_ai_studio/gemini-2.5-flash".to_string(),
            "mistral/mistral-large-latest".to_string(),
        ]),
        extra: serde_json::Map::new(),
    }];

    let inventory = build_subagent_inventory_from_profiles(&config, profiles);
    let item = inventory.first().expect("subagent inventory item");

    assert_eq!(item.model, "default");
    assert_eq!(item.provider, "inherit");
    assert_eq!(
        item.effective_model,
        "opencode_zen/muse-spark-1.2-contributor-free"
    );
    assert_eq!(item.effective_provider, "opencode_zen");
    assert_eq!(item.fallback_count, 2);
    assert_eq!(
        item.fallbacks,
        vec![
            "google_ai_studio/gemini-2.5-flash".to_string(),
            "mistral/mistral-large-latest".to_string(),
        ]
    );
    assert!(item.capabilities.contains(&"web".to_string()));
    assert_eq!(item.failure_count, 0);
    assert!(item.last_error.is_none());
}

#[test]
fn subagent_runtime_inventory_marks_vision_support() {
    let mut config = Config::default();
    config.agents.defaults.model = "deepseek-v4-flash-free".to_string();
    config.agents.defaults.provider = "opencode_zen".to_string();
    let profiles = vec![crate::subagents::SubagentProfile {
        name: "vision_agent".to_string(),
        description: "Describe images and screenshots.".to_string(),
        system_prompt: "Use vision for image analysis.".to_string(),
        model: Some("google_ai_studio/gemini-2.5-flash".to_string()),
        fallbacks: None,
        extra: serde_json::Map::new(),
    }];

    let inventory = build_subagent_inventory_from_profiles(&config, profiles);
    let item = inventory.first().expect("subagent inventory item");

    assert_eq!(item.model, "google_ai_studio/gemini-2.5-flash");
    assert_eq!(item.provider, "google_ai_studio");
    assert_eq!(item.effective_model, "google_ai_studio/gemini-2.5-flash");
    assert_eq!(item.effective_provider, "google_ai_studio");
    assert!(item.supports_vision);
    assert!(item.capabilities.contains(&"vision".to_string()));
    assert!(item.is_core);
    assert!(item.is_protected);
}

#[test]
fn session_inventory_groups_active_and_recent_sessions_by_channel() {
    let dir =
        std::env::temp_dir().join(format!("openz_inventory_sessions_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();

    write_session_fixture(&dir, "cli:abc", "hello from tui");
    write_session_fixture(&dir, "ws:control", "hello from webui");
    write_session_fixture(&dir, "telegram:42", "hello from telegram");

    let active = vec![crate::agent::activity::ActiveTuiSession {
        session_key: "cli:abc".to_string(),
        pid: 123,
        cwd: "/workspace/openz".to_string(),
        started_at: "2026-08-21T00:00:00Z".to_string(),
        last_seen_at: "2026-08-21T00:00:05Z".to_string(),
        model: "test-model".to_string(),
        provider: "test-provider".to_string(),
        preview: "hello from tui".to_string(),
    }];

    let inventory = build_session_inventory_from_parts(&dir, active);

    assert_eq!(inventory.active_ui_sessions.len(), 1);
    assert_eq!(inventory.active_ui_sessions[0].channel, "tui");
    assert_eq!(inventory.recent_sessions.len(), 3);
    assert!(
        inventory
            .recent_sessions
            .iter()
            .any(|session| session.key == "cli:abc" && session.active)
    );
    assert!(
        inventory
            .channel_counts
            .iter()
            .any(|count| count.channel == "webui" && count.count == 1)
    );
    assert!(
        inventory
            .channel_counts
            .iter()
            .any(|count| count.channel == "telegram" && count.count == 1)
    );

    let _ = std::fs::remove_dir_all(&dir);
}

fn write_session_fixture(dir: &std::path::Path, key: &str, prompt: &str) {
    let mut session = crate::session::Session::new(key);
    session.add_message("user", prompt);
    session.populate_hashes();
    let safe_key = key.replace(':', "_").replace('/', "_").replace('\\', "_");
    let path = dir.join(format!("{safe_key}.json"));
    std::fs::write(path, serde_json::to_string_pretty(&session).unwrap()).unwrap();
}
