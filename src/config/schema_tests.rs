use super::*;
use std::sync::{Mutex, OnceLock};


fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn blank_config() -> Config {
    Config {
        providers: ProvidersConfig::default(),
        agents: AgentsConfig::default(),
        channels: ChannelsConfig::default(),
        mcp_servers: HashMap::new(),
        embeddings: Some(EmbeddingsConfig::default()),
        skills: SkillsConfig::default(),
        research: ResearchConfig::default(),
        integrations: IntegrationsConfig::default(),
        browser: BrowserConfig::default(),
    }
}

#[test]
fn resolve_provider_config_prefers_env_and_supports_configured_env_reference() {
    let _guard = env_lock().lock().unwrap();
    std::env::set_var("OPENAI_API_KEY", "env-openai-key");
    std::env::set_var("OPENZ_TEST_PROVIDER_KEY", "referenced-provider-key");

    let mut config = blank_config();
    config.providers.openai = Some(ProviderConfig {
        api_key: Some("stored-openai-key".to_string()),
        api_key_env: None,
        api_key_file: None,
        api_base: None,
        default_model: None,
        extra: HashMap::new(),
    });
    config.providers.openrouter = Some(ProviderConfig {
        api_key: None,
        api_key_env: Some("OPENZ_TEST_PROVIDER_KEY".to_string()),
        api_key_file: None,
        api_base: None,
        default_model: None,
        extra: HashMap::new(),
    });

    assert_eq!(config.resolve_provider_config("openai").0, "env-openai-key");
    assert_eq!(
        config.resolve_provider_config("openrouter").0,
        "referenced-provider-key"
    );

    std::env::remove_var("OPENAI_API_KEY");
    std::env::remove_var("OPENZ_TEST_PROVIDER_KEY");
}

#[test]
fn resolve_provider_config_uses_table_aliases_and_defaults() {
    let _guard = env_lock().lock().unwrap();
    std::env::remove_var("Z_AI_API_KEY");
    let mut config = blank_config();
    config.providers.z_ai = Some(ProviderConfig {
        api_key: Some("z-key".to_string()),
        api_key_env: None,
        api_key_file: None,
        api_base: None,
        default_model: None,
        extra: HashMap::new(),
    });

    assert_eq!(
        config.resolve_provider_config("z_ai"),
        (
            "z-key".to_string(),
            "https://api.z.ai/api/paas/v4/".to_string()
        )
    );
}

#[test]
fn hyphenated_builtin_provider_aliases_are_not_custom() {
    assert!(!Config::is_custom_provider_name_valid("opencode-zen"));
    assert!(!Config::is_custom_provider_name_valid("google-ai-studio"));

    let mut config = blank_config();
    config.set_provider_config(
        "opencode-zen",
        ProviderConfig {
            api_key: Some("zen-hyphen-key".to_string()),
            api_key_env: None,
            api_key_file: None,
            api_base: None,
            default_model: None,
            extra: HashMap::new(),
        },
    );
    assert_eq!(
        config
            .get_provider_config("opencode-zen")
            .and_then(|provider| provider.api_key.as_deref()),
        Some("zen-hyphen-key")
    );
    assert!(!config.providers.others.contains_key("opencode-zen"));

    config.set_provider_config(
        "google-ai-studio",
        ProviderConfig {
            api_key: Some("google-hyphen-key".to_string()),
            api_key_env: None,
            api_key_file: None,
            api_base: None,
            default_model: None,
            extra: HashMap::new(),
        },
    );
    assert_eq!(
        config
            .get_provider_config("google-ai-studio")
            .and_then(|provider| provider.api_key.as_deref()),
        Some("google-hyphen-key")
    );
    assert!(!config.providers.others.contains_key("google-ai-studio"));
}

#[test]
fn set_provider_config_uses_builtin_aliases() {
    let mut config = blank_config();
    config.set_provider_config(
        "z_ai",
        ProviderConfig {
            api_key: Some("z-alias-key".to_string()),
            api_key_env: None,
            api_key_file: None,
            api_base: None,
            default_model: None,
            extra: HashMap::new(),
        },
    );

    assert_eq!(
        config
            .get_provider_config("z_ai")
            .and_then(|provider| provider.api_key.as_deref()),
        Some("z-alias-key")
    );
    assert!(!config.providers.others.contains_key("z_ai"));

    config.set_provider_config(
        "opencode zen",
        ProviderConfig {
            api_key: Some("zen-alias-key".to_string()),
            api_key_env: None,
            api_key_file: None,
            api_base: None,
            default_model: None,
            extra: HashMap::new(),
        },
    );
    assert_eq!(
        config
            .get_provider_config("opencode zen")
            .and_then(|provider| provider.api_key.as_deref()),
        Some("zen-alias-key")
    );
    assert!(!config.providers.others.contains_key("opencode zen"));

    config.set_provider_config(
        "google ai studio",
        ProviderConfig {
            api_key: Some("google-alias-key".to_string()),
            api_key_env: None,
            api_key_file: None,
            api_base: None,
            default_model: None,
            extra: HashMap::new(),
        },
    );
    assert_eq!(
        config
            .get_provider_config("google ai studio")
            .and_then(|provider| provider.api_key.as_deref()),
        Some("google-alias-key")
    );
    assert!(!config.providers.others.contains_key("google ai studio"));
}

#[test]
fn provider_available_uses_env_keys_without_marking_unconfigured_local_available() {
    let _guard = env_lock().lock().unwrap();
    std::env::remove_var("OPENAI_API_KEY");
    std::env::remove_var("CEREBRAS_API_KEY");
    std::env::remove_var("CEREBRES_API_KEY");
    std::env::remove_var("CEBRAS_API_KEY");
    let config = blank_config();

    assert!(config.is_provider_available("ollama_local"));
    assert!(config.is_provider_available("mivi"));
    assert!(!config.is_provider_available("ollama"));
    assert!(!config.is_provider_available("openai"));

    std::env::set_var("OPENAI_API_KEY", "test-key");
    assert!(config.is_provider_available("openai"));
    std::env::remove_var("OPENAI_API_KEY");
}

#[test]
fn mivi_provider_uses_local_defaults_without_key() {
    let _guard = env_lock().lock().unwrap();
    std::env::remove_var("MIVI_API_KEY");
    let config = blank_config();

    assert_eq!(
        config.resolve_provider_config("mivi"),
        (String::new(), "http://127.0.0.1:8000/v1".to_string())
    );
    assert!(config.is_provider_available("mivi"));
}

#[test]
fn mivi_provider_default_model_can_be_configured() {
    let mut config = blank_config();
    config.set_provider_config(
        "mivi",
        ProviderConfig {
            api_key: Some("local".to_string()),
            api_key_env: None,
            api_key_file: None,
            api_base: Some("http://127.0.0.1:8000/v1".to_string()),
            default_model: Some("mivi llm".to_string()),
            extra: HashMap::new(),
        },
    );

    assert_eq!(
        config
            .get_provider_config("mivi")
            .and_then(|provider| provider.default_model.clone()),
        Some("mivi llm".to_string())
    );
    assert!(config.is_provider_available("mivi"));
}

#[test]
fn custom_provider_resolves_config_and_local_availability() {
    let _guard = env_lock().lock().unwrap();
    std::env::remove_var("OPENZ_PROVIDER_LOCAL_AI_API_KEY");
    let mut config = blank_config();
    config.providers.others.insert(
        "local_ai".to_string(),
        ProviderConfig {
            api_key: None,
            api_key_env: None,
            api_key_file: None,
            api_base: Some("http://127.0.0.1:9999/v1".to_string()),
            default_model: Some("local-model".to_string()),
            extra: HashMap::new(),
        },
    );

    assert!(Config::is_custom_provider_name_valid("local_ai"));
    assert!(config.is_provider_available("local_ai"));
    assert_eq!(
        config.resolve_provider_config("local_ai"),
        (String::new(), "http://127.0.0.1:9999/v1".to_string())
    );
    assert_eq!(
        config.custom_provider_default_model("local_ai"),
        Some("local-model".to_string())
    );
}

#[test]
fn cerebras_legacy_env_key_still_works() {
    let _guard = env_lock().lock().unwrap();
    std::env::remove_var("CEREBRAS_API_KEY");
    std::env::remove_var("CEREBRES_API_KEY");
    std::env::set_var("CEBRAS_API_KEY", "legacy-key");
    let config = blank_config();

    assert_eq!(
        config.resolve_provider_config("cerebras").0,
        "legacy-key".to_string()
    );
    assert!(config.is_provider_available("cerebras"));
    std::env::remove_var("CEBRAS_API_KEY");
}

#[test]
fn cerebras_documented_legacy_env_key_still_works() {
    let _guard = env_lock().lock().unwrap();
    std::env::remove_var("CEREBRAS_API_KEY");
    std::env::remove_var("CEBRAS_API_KEY");
    std::env::set_var("CEREBRES_API_KEY", "documented-legacy-key");
    let config = blank_config();

    assert_eq!(
        config.resolve_provider_config("cerebras").0,
        "documented-legacy-key".to_string()
    );
    assert!(config.is_provider_available("cerebras"));
    std::env::remove_var("CEREBRES_API_KEY");
}

// ── General Schema Tests ──


#[test]
fn whatsapp_verify_token_has_no_guessable_default() {
    let config = Config::default();
    let whatsapp = config.channels.whatsapp.expect("default whatsapp config");
    assert!(whatsapp.verify_token.is_empty());
}

#[test]
fn default_tui_thought_display_is_hidden_for_public_safety() {
    let defaults = AgentDefaults::default();
    assert_eq!(defaults.tui_thought_display, "off");
}

#[test]
fn auto_capture_notices_are_shown_by_default() {
    let defaults = AgentDefaults::default();
    assert!(defaults.show_auto_capture_notices);
}

#[test]
fn browser_config_includes_configurable_cdp_port() {
    let cfg = Config::default();
    assert_eq!(cfg.browser.cdp_port, 9222);

    let json = serde_json::json!({
        "browser": {
            "cdpPort": 9225
        }
    });
    let parsed: Config = serde_json::from_value(json).unwrap();
    assert_eq!(parsed.browser.cdp_port, 9225);

    let json_snake = serde_json::json!({
        "browser": {
            "cdp_port": 9226
        }
    });
    let parsed_snake: Config = serde_json::from_value(json_snake).unwrap();
    assert_eq!(parsed_snake.browser.cdp_port, 9226);

    let json_legacy = serde_json::json!({
        "browser": {
            "chrome_cdp_port": 9227
        }
    });
    let parsed_legacy: Config = serde_json::from_value(json_legacy).unwrap();
    assert_eq!(parsed_legacy.browser.cdp_port, 9227);
}

// ── Layered Tool Routing Tests ──


#[test]
fn layered_tool_routing_defaults_are_balanced() {
    let config = Config::default();
    assert!(config.agents.defaults.layered_tool_routing.enabled);
    assert_eq!(
        config
            .agents
            .defaults
            .layered_tool_routing
            .max_visible_tools,
        20
    );
    assert!(config
        .agents
        .defaults
        .layered_tool_routing
        .always_visible_tools
        .contains(&"request_tool_scope".to_string()));
}
