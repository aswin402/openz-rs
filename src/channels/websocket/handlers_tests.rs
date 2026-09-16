use super::*;

#[test]
fn websocket_chat_uses_shared_stop_command_detection() {
    assert!(crate::channels::is_stop_command("/stop"));
    assert!(!crate::channels::is_stop_command("/stopwatch"));
}

#[test]
fn test_normalize_model_name() {
    assert_eq!(normalize_model_name("gpt-4o"), "openai/gpt-4o");
    assert_eq!(
        normalize_model_name("claude-3-5-sonnet"),
        "anthropic/claude-3-5-sonnet"
    );
    assert_eq!(
        normalize_model_name("deepseek-chat"),
        "deepseek/deepseek-chat"
    );
    assert_eq!(normalize_model_name("custom/my-model"), "custom/my-model");
}

#[test]
fn test_determine_routed_model_complex() {
    let mut config = crate::config::schema::Config::default();
    config.agents.defaults.model = "anthropic/claude-3-5-sonnet".to_string();

    // Complex prompts should use requested or default premium
    let model =
        determine_routed_model(&config, "gpt-4o", "Please fix this error in my rust code");
    assert_eq!(model, "gpt-4o");

    let model_fallback = determine_routed_model(
        &config,
        "some-random-model",
        "Please design a new database schema for a blog",
    );
    assert_eq!(model_fallback, "anthropic/claude-3-5-sonnet");
}

#[test]
fn test_determine_routed_model_simple_fallback() {
    let mut config = crate::config::schema::Config::default();
    config.agents.defaults.model = "anthropic/claude-3-5-sonnet".to_string();

    // Simple prompt with env vars -> routes to cheapest available provider
    let _model = determine_routed_model(&config, "gpt-4o", "Hello!");

    // Simple prompt, deepseek key set -> should route to deepseek-chat
    config.providers.deepseek = Some(crate::config::schema::ProviderConfig {
        api_key: Some("test-key".to_string()),
        api_key_env: None,
        api_key_file: None,
        api_base: None,
        default_model: None,
        extra: std::collections::HashMap::new(),
    });
    let model_routed = determine_routed_model(&config, "gpt-4o", "Hi there");
    assert_eq!(model_routed, "deepseek/deepseek-chat");
}
