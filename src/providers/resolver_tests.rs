use super::*;
use std::sync::OnceLock;

/// Serialize env-modifying tests to prevent race conditions from parallel execution.
fn env_lock() -> &'static std::sync::Mutex<()> {
    static ENV_LOCK: OnceLock<std::sync::Mutex<()>> = OnceLock::new();
    ENV_LOCK.get_or_init(|| std::sync::Mutex::new(()))
}
use crate::config::schema::{AgentDefaults, AgentsConfig, Config, ProviderConfig};

fn config_with(provider: &str) -> Config {
    Config {
        agents: AgentsConfig {
            defaults: AgentDefaults {
                provider: provider.to_string(),
                model: "gpt-4o".to_string(),
                ..AgentDefaults::default()
            },
        },
        ..Config::default()
    }
}

#[test]
fn test_prefix_anthropic() {
    let _guard = env_lock().lock().unwrap();
    let cfg = config_with("auto");
    std::env::set_var("ANTHROPIC_API_KEY", "k");
    let r = resolve_provider_full(&cfg, "anthropic/claude-3-5-sonnet").unwrap();
    assert_eq!(r.provider_name, "anthropic");
    assert_eq!(r.model, "claude-3-5-sonnet");
    std::env::remove_var("ANTHROPIC_API_KEY");
}

#[test]
fn test_mivi_prefix_routes_to_local_mivi_provider() {
    let _guard = env_lock().lock().unwrap();
    std::env::remove_var("MIVI_API_KEY");
    let cfg = config_with("auto");
    let r = resolve_provider_full(&cfg, "mivi/mivi").unwrap();
    assert_eq!(r.provider_name, "mivi");
    assert_eq!(r.model, "mivi");
    assert_eq!(r.api_base, "http://127.0.0.1:8000/v1");
}

#[test]
fn test_bare_mivi_routes_to_local_mivi_provider() {
    let _guard = env_lock().lock().unwrap();
    std::env::remove_var("MIVI_API_KEY");
    let cfg = config_with("auto");
    let r = resolve_provider_full(&cfg, "mivi").unwrap();
    assert_eq!(r.provider_name, "mivi");
    assert_eq!(r.model, "mivi");
}

#[test]
fn test_custom_provider_prefix_routes_to_openai_compatible_provider() {
    let _guard = env_lock().lock().unwrap();
    std::env::remove_var("OPENZ_PROVIDER_ACME_API_KEY");
    let mut cfg = config_with("auto");
    cfg.providers.others.insert(
        "acme".to_string(),
        ProviderConfig {
            api_key: Some("acme-key".to_string()),
            api_key_env: None,
            api_key_file: None,
            api_base: Some("https://acme.example/v1".to_string()),
            default_model: Some("acme-model".to_string()),
            extra: Default::default(),
        },
    );

    let r = resolve_provider_full(&cfg, "acme/acme-model").unwrap();
    assert_eq!(r.provider_name, "acme");
    assert_eq!(r.model, "acme-model");
    assert_eq!(r.api_key, "acme-key");
    assert_eq!(r.api_base, "https://acme.example/v1");
}

#[test]
fn test_prefix_openai() {
    let _guard = env_lock().lock().unwrap();
    let cfg = config_with("auto");
    std::env::set_var("OPENAI_API_KEY", "k");
    let r = resolve_provider_full(&cfg, "openai/gpt-4o").unwrap();
    assert_eq!(r.provider_name, "openai");
    assert_eq!(r.model, "gpt-4o");
    std::env::remove_var("OPENAI_API_KEY");
}

#[test]
fn test_openrouter_free_model_routes_to_openrouter_when_provider_auto() {
    let _guard = env_lock().lock().unwrap();
    let cfg = config_with("auto");
    std::env::set_var("OPENROUTER_API_KEY", "rk");
    let r = resolve_provider_full(&cfg, "google/gemma-4-31b-it:free").unwrap();
    assert_eq!(r.provider_name, "openrouter");
    assert_eq!(r.model, "google/gemma-4-31b-it:free");
    std::env::remove_var("OPENROUTER_API_KEY");
}

#[test]
fn test_empty_configured_openrouter_key_is_not_used_for_free_model_routing() {
    let _guard = env_lock().lock().unwrap();
    for var in &[
        "OPENAI_API_KEY",
        "OPENROUTER_API_KEY",
        "OPENCODE_ZEN_API_KEY",
    ] {
        std::env::remove_var(var);
    }
    let mut cfg = config_with("auto");
    cfg.providers.openrouter = Some(ProviderConfig {
        api_key: Some(String::new()),
        api_key_env: None,
        api_key_file: None,
        api_base: None,
        default_model: None,
        extra: Default::default(),
    });

    let err = match resolve_provider_full(&cfg, "google/gemma-4-31b-it:free") {
        Ok(r) => panic!(
            "expected missing Google AI Studio key error, got {}",
            r.provider_name
        ),
        Err(err) => err.to_string(),
    };

    assert!(err.contains("google_ai_studio"), "unexpected error: {err}");
    assert!(
        err.contains("GOOGLE_AI_STUDIO_API_KEY"),
        "unexpected error: {err}"
    );
}

#[test]
fn test_empty_configured_openrouter_key_is_not_used_as_fallback() {
    let _guard = env_lock().lock().unwrap();
    for var in &[
        "DEEPSEEK_API_KEY",
        "OPENROUTER_API_KEY",
        "OPENCODE_ZEN_API_KEY",
    ] {
        std::env::remove_var(var);
    }
    let mut cfg = config_with("auto");
    cfg.providers.openrouter = Some(ProviderConfig {
        api_key: Some(String::new()),
        api_key_env: None,
        api_key_file: None,
        api_base: None,
        default_model: None,
        extra: Default::default(),
    });

    let err = match resolve_provider_full(&cfg, "deepseek-chat") {
        Ok(r) => panic!(
            "expected missing DeepSeek key error, got {}",
            r.provider_name
        ),
        Err(err) => err.to_string(),
    };

    assert!(err.contains("deepseek"), "unexpected error: {err}");
    assert!(err.contains("DEEPSEEK_API_KEY"), "unexpected error: {err}");
}

#[test]
fn test_empty_configured_nvidia_key_does_not_block_openrouter_free_routing() {
    let _guard = env_lock().lock().unwrap();
    for var in &["OPENAI_API_KEY", "OPENROUTER_API_KEY", "NVIDIA_API_KEY"] {
        std::env::remove_var(var);
    }
    let mut cfg = config_with("auto");
    cfg.providers.openrouter = Some(ProviderConfig {
        api_key: Some("rk".to_string()),
        api_key_env: None,
        api_key_file: None,
        api_base: None,
        default_model: None,
        extra: Default::default(),
    });
    cfg.providers.nvidia = Some(ProviderConfig {
        api_key: Some(String::new()),
        api_key_env: None,
        api_key_file: None,
        api_base: None,
        default_model: None,
        extra: Default::default(),
    });

    let r = resolve_provider_full(&cfg, "nvidia/nemotron-3-nano-omni-30b-a3b-reasoning:free")
        .unwrap();
    assert_eq!(r.provider_name, "openrouter");
    assert_eq!(r.api_key, "rk");
}

#[test]
fn test_openrouter_nvidia_free_preserves_provider_slug_when_nvidia_key_absent() {
    let _guard = env_lock().lock().unwrap();
    let cfg = config_with("auto");
    std::env::set_var("OPENROUTER_API_KEY", "rk");
    std::env::remove_var("NVIDIA_API_KEY");
    let r = resolve_provider_full(&cfg, "nvidia/nemotron-3-nano-omni-30b-a3b-reasoning:free")
        .unwrap();
    assert_eq!(r.provider_name, "openrouter");
    assert_eq!(
        r.model,
        "nvidia/nemotron-3-nano-omni-30b-a3b-reasoning:free"
    );
    std::env::remove_var("OPENROUTER_API_KEY");
}

#[test]
fn test_prefix_nvidia_free() {
    let _guard = env_lock().lock().unwrap();
    let cfg = config_with("auto");
    std::env::set_var("NVIDIA_API_KEY", "k");
    let r = resolve_provider_full(&cfg, "nvidia/llama-3.1-8b-instruct:free").unwrap();
    assert_eq!(r.provider_name, "nvidia");
    assert_eq!(r.model, "nvidia/llama-3.1-8b-instruct");
    std::env::remove_var("NVIDIA_API_KEY");
}

#[test]
fn test_auto_claude() {
    let _guard = env_lock().lock().unwrap();
    let cfg = config_with("auto");
    std::env::set_var("ANTHROPIC_API_KEY", "k");
    let r = resolve_provider_full(&cfg, "claude-3-5-sonnet").unwrap();
    assert_eq!(r.provider_name, "anthropic");
    std::env::remove_var("ANTHROPIC_API_KEY");
}

#[test]
fn test_auto_gpt() {
    let _guard = env_lock().lock().unwrap();
    let cfg = config_with("auto");
    std::env::set_var("OPENAI_API_KEY", "k");
    let r = resolve_provider_full(&cfg, "gpt-4o").unwrap();
    assert_eq!(r.provider_name, "openai");
    std::env::remove_var("OPENAI_API_KEY");
}

#[test]
fn test_auto_deepseek() {
    let _guard = env_lock().lock().unwrap();
    let cfg = config_with("auto");
    std::env::set_var("DEEPSEEK_API_KEY", "k");
    let r = resolve_provider_full(&cfg, "deepseek-chat").unwrap();
    assert_eq!(r.provider_name, "deepseek");
    std::env::remove_var("DEEPSEEK_API_KEY");
}

#[test]
fn test_auto_gemini() {
    let _guard = env_lock().lock().unwrap();
    let cfg = config_with("auto");
    std::env::set_var("GOOGLE_AI_STUDIO_API_KEY", "k");
    let r = resolve_provider_full(&cfg, "gemini-2.0-flash").unwrap();
    assert_eq!(r.provider_name, "google_ai_studio");
    std::env::remove_var("GOOGLE_AI_STUDIO_API_KEY");
}

#[test]
fn test_default_provider_not_auto() {
    let _guard = env_lock().lock().unwrap();
    let cfg = config_with("openai");
    std::env::set_var("OPENAI_API_KEY", "k");
    let r = resolve_provider_full(&cfg, "claude-some-model").unwrap();
    assert_eq!(r.provider_name, "openai");
    std::env::remove_var("OPENAI_API_KEY");
}

#[test]
fn test_explicit_provider_wins_over_model_prefix_and_free_suffix() {
    let _guard = env_lock().lock().unwrap();
    std::env::set_var("OPENAI_API_KEY", "k");
    std::env::set_var("ANTHROPIC_API_KEY", "ak");
    std::env::set_var("OPENROUTER_API_KEY", "rk");
    let cfg = config_with("openai");

    let prefixed = resolve_provider_full(&cfg, "anthropic/claude-3-5-sonnet").unwrap();
    assert_eq!(prefixed.provider_name, "openai");

    let free = resolve_provider_full(&cfg, "google/gemma-4-31b-it:free").unwrap();
    assert_eq!(free.provider_name, "openai");

    std::env::remove_var("OPENROUTER_API_KEY");
    std::env::remove_var("ANTHROPIC_API_KEY");
    std::env::remove_var("OPENAI_API_KEY");
}

#[test]
fn test_cerebras_prefix() {
    let _guard = env_lock().lock().unwrap();
    let cfg = config_with("auto");
    std::env::set_var("CEREBRAS_API_KEY", "k");
    let r = resolve_provider_full(&cfg, "cerebras/llama-3.3-70b").unwrap();
    assert_eq!(r.provider_name, "cerebras");
    assert_eq!(r.model, "llama-3.3-70b");
    std::env::remove_var("CEREBRAS_API_KEY");
}

#[test]
fn test_no_key_fallback_fails() {
    let _guard = env_lock().lock().unwrap();
    // Clear all known API key env vars to ensure no leakage from other tests
    for var in &[
        "ANTHROPIC_API_KEY",
        "OPENAI_API_KEY",
        "DEEPSEEK_API_KEY",
        "OPENROUTER_API_KEY",
        "OPENCODE_ZEN_API_KEY",
        "GOOGLE_AI_STUDIO_API_KEY",
        "GROQ_API_KEY",
        "MISTRAL_API_KEY",
        "NVIDIA_API_KEY",
        "Z_AI_API_KEY",
        "CEREBRAS_API_KEY",
        "COHERE_API_KEY",
        "LLM7_API_KEY",
        "SAMBANOVA_API_KEY",
        "HUGGINGFACE_API_KEY",
    ] {
        std::env::remove_var(var);
    }
    let cfg = config_with("openai");
    let r = resolve_provider_full(&cfg, "some-model");
    assert!(r.is_err());
}

#[test]
fn test_builtin_provider_alias_missing_key_errors_use_canonical_env_vars() {
    let _guard = env_lock().lock().unwrap();
    for var in &[
        "Z_AI_API_KEY",
        "OPENCODE_ZEN_API_KEY",
        "GOOGLE_AI_STUDIO_API_KEY",
        "OPENROUTER_API_KEY",
    ] {
        std::env::remove_var(var);
    }

    let z_err = match resolve_provider_full(&config_with("z_ai"), "glm-4.7-flash") {
        Ok(_) => panic!("expected missing z.ai key error"),
        Err(err) => err.to_string(),
    };
    assert!(z_err.contains("Z_AI_API_KEY"), "unexpected error: {z_err}");
    assert!(
        !z_err.contains("OPENZ_PROVIDER_Z_AI_API_KEY"),
        "unexpected error: {z_err}"
    );

    let zen_err = match resolve_provider_full(&config_with("opencode-zen"), "mimo-v2.5-free") {
        Ok(_) => panic!("expected missing OpenCode Zen key error"),
        Err(err) => err.to_string(),
    };
    assert!(
        zen_err.contains("OPENCODE_ZEN_API_KEY"),
        "unexpected error: {zen_err}"
    );
    assert!(
        !zen_err.contains("OPENZ_PROVIDER_OPENCODE_ZEN_API_KEY"),
        "unexpected error: {zen_err}"
    );

    let google_err = match resolve_provider_full(
        &config_with("google-ai-studio"),
        "models/gemini-2.0-flash",
    ) {
        Ok(_) => panic!("expected missing Google AI Studio key error"),
        Err(err) => err.to_string(),
    };
    assert!(
        google_err.contains("GOOGLE_AI_STUDIO_API_KEY"),
        "unexpected error: {google_err}"
    );
    assert!(
        !google_err.contains("OPENZ_PROVIDER_GOOGLE_AI_STUDIO_API_KEY"),
        "unexpected error: {google_err}"
    );
}

#[test]
fn test_missing_openai_key_error_is_actionable() {
    let _guard = env_lock().lock().unwrap();
    for var in &[
        "OPENAI_API_KEY",
        "OPENROUTER_API_KEY",
        "OPENCODE_ZEN_API_KEY",
    ] {
        std::env::remove_var(var);
    }
    let cfg = config_with("openai");
    let err = match resolve_provider_full(&cfg, "openai/gpt-4o") {
        Ok(_) => panic!("expected missing OpenAI key error"),
        Err(err) => err.to_string(),
    };

    assert!(err.contains("openai"));
    assert!(err.contains("OPENAI_API_KEY"));
    assert!(err.contains("openz configure"));
}

#[test]
fn test_missing_anthropic_key_error_is_actionable() {
    let _guard = env_lock().lock().unwrap();
    for var in &[
        "ANTHROPIC_API_KEY",
        "OPENROUTER_API_KEY",
        "OPENCODE_ZEN_API_KEY",
    ] {
        std::env::remove_var(var);
    }
    let cfg = config_with("auto");
    let err = match resolve_provider_full(&cfg, "anthropic/claude-3-5-sonnet") {
        Ok(_) => panic!("expected missing Anthropic key error"),
        Err(err) => err.to_string(),
    };

    assert!(err.contains("anthropic"));
    assert!(err.contains("ANTHROPIC_API_KEY"));
    assert!(err.contains("openz configure"));
}

#[test]
fn test_ollama_no_key_needed() {
    let cfg = config_with("auto");
    let r = resolve_provider_full(&cfg, "ollama/llama3").unwrap();
    assert_eq!(r.provider_name, "ollama");
    assert_eq!(r.model, "llama3");
}

#[test]
fn test_google_ai_studio_models_prefix() {
    let _guard = env_lock().lock().unwrap();
    let cfg = config_with("auto");
    std::env::set_var("GOOGLE_AI_STUDIO_API_KEY", "k");
    let r = resolve_provider_full(&cfg, "google_ai_studio/models/gemini-2.0-flash").unwrap();
    assert_eq!(r.provider_name, "google_ai_studio");
    assert_eq!(r.model, "gemini-2.0-flash");
    std::env::remove_var("GOOGLE_AI_STUDIO_API_KEY");
}
