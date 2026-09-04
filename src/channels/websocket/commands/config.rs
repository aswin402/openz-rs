//! Configuration safety helpers shared by WebSocket config commands.

use serde_json::Value;

pub(crate) fn normalized_config_key(key: &str) -> String {
    key.chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn is_masked_config_secret(value: &str) -> bool {
    matches!(value.trim(), "••••••••" | "********")
}

pub(crate) fn mask_config_secret(value: &str) -> &'static str {
    if value.is_empty() {
        ""
    } else {
        "••••••••"
    }
}

fn sensitive_config_value_changed(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::String(value) => !is_masked_config_secret(value),
        _ => true,
    }
}

fn sensitive_default_config_key(key: &str) -> bool {
    matches!(
        normalized_config_key(key).as_str(),
        "securitymode" | "workspace" | "whitelistedcommandprefixes" | "whitelistedpaths"
    )
}

fn credential_config_key(key: &str) -> bool {
    matches!(
        normalized_config_key(key).as_str(),
        "apikey"
            | "apikeyenv"
            | "apikeyfile"
            | "bottoken"
            | "verifytoken"
            | "token"
            | "password"
            | "secret"
            | "clientsecret"
            | "privatekey"
    )
}

fn object_changes_sensitive_key<F>(value: Option<&Value>, is_sensitive_key: F) -> bool
where
    F: Fn(&str) -> bool,
{
    value
        .and_then(Value::as_object)
        .map(|object| {
            object
                .iter()
                .any(|(key, value)| is_sensitive_key(key) && sensitive_config_value_changed(value))
        })
        .unwrap_or(false)
}

fn nested_objects_change_sensitive_key<F>(value: Option<&Value>, is_sensitive_key: F) -> bool
where
    F: Fn(&str) -> bool + Copy,
{
    value
        .and_then(Value::as_object)
        .map(|outer| {
            outer
                .values()
                .any(|inner| object_changes_sensitive_key(Some(inner), is_sensitive_key))
        })
        .unwrap_or(false)
}

pub(crate) fn config_update_requires_gateway_token(envelope: &Value) -> bool {
    object_changes_sensitive_key(envelope.get("defaults"), sensitive_default_config_key)
        || nested_objects_change_sensitive_key(envelope.get("providers"), credential_config_key)
        || nested_objects_change_sensitive_key(envelope.get("channels"), credential_config_key)
}

pub(crate) async fn config_data_event(config: &crate::config::schema::Config) -> Value {
    let skills = crate::agent::skills::load_skill_views().unwrap_or_default();
    let defaults = {
        let d = &config.agents.defaults;
        serde_json::json!({
            "model": d.model,
            "provider": d.provider,
            "temperature": d.temperature,
            "max_tokens": d.max_tokens,
            "streaming": d.streaming,
            "caveman_mode": d.caveman_mode,
            "security_mode": d.security_mode,
            "workspace": d.workspace,
            "bot_name": d.bot_name,
            "max_messages": d.max_messages,
            "max_tool_iterations": d.max_tool_iterations,
            "tool_timeout_secs": d.tool_timeout_secs,
            "enable_sandbox": d.enable_sandbox,
            "context_limit": d.context_limit,
            "tool_output_limit": d.tool_output_limit,
            "show_auto_capture_notices": d.show_auto_capture_notices,
            "tui_thought_display": d.tui_thought_display,
            "firefox_webdriver_port": config.browser.firefox_webdriver_port,
            "firefox_attach_port": config.browser.firefox_attach_port,
        })
    };
    let mcp_resp = super::observability::mcp_servers_event(config).await;
    let mcp_servers = mcp_resp["servers"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let subagents = super::profiles::subagent_profile_events();

    let mask_key = |key: &Option<String>| {
        key.as_ref().map(|value| mask_config_secret(value))
    };
    let map_provider = |provider: &Option<crate::config::schema::ProviderConfig>| {
        provider.as_ref().map(|cfg| {
            serde_json::json!({
                "api_key": mask_key(&cfg.api_key),
                "api_key_env": cfg.api_key_env,
                "api_key_file": cfg.api_key_file,
                "api_base": cfg.api_base,
                "default_model": cfg.default_model
            })
        })
    };

    let p = &config.providers;
    let mut providers_config = serde_json::Map::new();
    for (key, provider) in [
        ("openai", &p.openai),
        ("anthropic", &p.anthropic),
        ("openrouter", &p.openrouter),
        ("deepseek", &p.deepseek),
        ("groq", &p.groq),
        ("ollama", &p.ollama),
        ("minimax", &p.minimax),
        ("mistral", &p.mistral),
        ("z_ai", &p.z_ai),
        ("nvidia", &p.nvidia),
        ("opencode_zen", &p.opencode_zen),
        ("cerebras", &p.cerebras),
        ("google_ai_studio", &p.google_ai_studio),
    ] {
        providers_config.insert(
            key.to_string(),
            serde_json::to_value(map_provider(provider)).unwrap_or(Value::Null),
        );
    }
    for (key, cfg) in &p.others {
        providers_config.insert(
            key.clone(),
            serde_json::json!({
                "api_key": mask_key(&cfg.api_key),
                "api_base": cfg.api_base,
                "default_model": cfg.default_model
            }),
        );
    }

    let ch = &config.channels;
    let telegram = ch.telegram.as_ref().map(|cfg| {
        serde_json::json!({
            "enabled": cfg.enabled,
            "bot_token": mask_config_secret(&cfg.bot_token)
        })
    });
    let discord = ch.discord.as_ref().map(|cfg| {
        serde_json::json!({
            "enabled": cfg.enabled,
            "bot_token": mask_config_secret(&cfg.bot_token)
        })
    });
    let whatsapp = ch.whatsapp.as_ref().map(|cfg| {
        serde_json::json!({
            "enabled": cfg.enabled,
            "api_key": mask_config_secret(&cfg.api_key),
            "phone_number_id": cfg.phone_number_id,
            "webhook_port": cfg.webhook_port,
            "verify_token": mask_config_secret(&cfg.verify_token)
        })
    });
    let channels = serde_json::json!({
        "telegram": telegram,
        "discord": discord,
        "whatsapp": whatsapp,
    });

    super::super::protocol::config_data(
        defaults,
        skills,
        mcp_servers,
        subagents,
        providers_config,
        channels,
        super::super::webui_capabilities(config),
    )
}

fn update_provider_config(
    field: &mut Option<crate::config::schema::ProviderConfig>,
    data: &Value,
    include_lookup_fields: bool,
) {
    let Some(data_object) = data.as_object() else {
        return;
    };
    let mut config = field.clone().unwrap_or_default();
    if let Some(key) = data_object.get("api_key").and_then(Value::as_str) {
        if key != "••••••••" {
            config.api_key = if key.is_empty() {
                None
            } else {
                Some(key.to_string())
            };
        }
    }
    if include_lookup_fields {
        if let Some(value) = data_object.get("api_key_env").and_then(Value::as_str) {
            config.api_key_env = if value.trim().is_empty() {
                None
            } else {
                Some(value.to_string())
            };
        }
        if let Some(value) = data_object.get("api_key_file").and_then(Value::as_str) {
            config.api_key_file = if value.trim().is_empty() {
                None
            } else {
                Some(value.to_string())
            };
        }
    }
    if let Some(value) = data_object.get("api_base") {
        config.api_base = value.as_str().map(str::to_string);
    }
    if let Some(value) = data_object.get("default_model") {
        config.default_model = value.as_str().map(str::to_string);
    }
    *field = Some(config);
}

pub(crate) fn apply_config_update(
    config: &mut crate::config::schema::Config,
    envelope: &Value,
) {
    if let Some(defaults) = envelope.get("defaults") {
        let defaults_config = &mut config.agents.defaults;
        if let Some(value) = defaults.get("model").and_then(Value::as_str) {
            defaults_config.model = value.to_string();
        }
        if let Some(value) = defaults.get("provider").and_then(Value::as_str) {
            defaults_config.provider = value.to_string();
        }
        if let (Some(provider), Some(model)) = (
            defaults.get("provider").and_then(Value::as_str),
            defaults.get("model").and_then(Value::as_str),
        ) {
            crate::channels::record_recent_model(provider, model);
        }
        if let Some(value) = defaults.get("temperature").and_then(Value::as_f64) {
            defaults_config.temperature = value as f32;
        }
        if let Some(value) = defaults.get("max_tokens").and_then(Value::as_u64) {
            defaults_config.max_tokens = value as usize;
        }
        if let Some(value) = defaults.get("streaming").and_then(Value::as_bool) {
            defaults_config.streaming = value;
        }
        if let Some(value) = defaults.get("caveman_mode").and_then(Value::as_bool) {
            defaults_config.caveman_mode = value;
        }
        if let Some(value) = defaults.get("security_mode").and_then(Value::as_str) {
            defaults_config.security_mode = value.to_string();
        }
        if let Some(value) = defaults.get("bot_name").and_then(Value::as_str) {
            defaults_config.bot_name = value.to_string();
        }
        if let Some(value) = defaults.get("workspace").and_then(Value::as_str) {
            defaults_config.workspace = value.to_string();
        }
        if let Some(value) = defaults.get("context_limit") {
            defaults_config.context_limit = value.as_u64().map(|number| number as usize);
        }
        if let Some(value) = defaults.get("tool_output_limit") {
            defaults_config.tool_output_limit = value.as_u64().map(|number| number as usize);
        }
        if let Some(value) = defaults
            .get("show_auto_capture_notices")
            .and_then(Value::as_bool)
        {
            defaults_config.show_auto_capture_notices = value;
        }
        if let Some(value) = defaults.get("tui_thought_display").and_then(Value::as_str) {
            defaults_config.tui_thought_display = value.to_string();
        }
        if let Some(value) = defaults.get("max_messages").and_then(Value::as_u64) {
            defaults_config.max_messages = value as usize;
        }
        if let Some(value) = defaults.get("max_tool_iterations").and_then(Value::as_u64) {
            defaults_config.max_tool_iterations = value as usize;
        }
        if let Some(value) = defaults.get("tool_timeout_secs").and_then(Value::as_u64) {
            defaults_config.tool_timeout_secs = value;
        }
    }

    if let Some(providers) = envelope.get("providers").and_then(Value::as_object) {
        let provider_config = &mut config.providers;
        for (name, field) in [
            ("openai", &mut provider_config.openai),
            ("anthropic", &mut provider_config.anthropic),
            ("openrouter", &mut provider_config.openrouter),
            ("deepseek", &mut provider_config.deepseek),
            ("groq", &mut provider_config.groq),
            ("ollama", &mut provider_config.ollama),
            ("minimax", &mut provider_config.minimax),
            ("mistral", &mut provider_config.mistral),
            ("z_ai", &mut provider_config.z_ai),
            ("nvidia", &mut provider_config.nvidia),
            ("opencode_zen", &mut provider_config.opencode_zen),
            ("cerebras", &mut provider_config.cerebras),
            ("google_ai_studio", &mut provider_config.google_ai_studio),
        ] {
            if let Some(value) = providers.get(name) {
                update_provider_config(field, value, false);
            }
        }

        for (name, value) in providers {
            if [
                "openai",
                "anthropic",
                "openrouter",
                "deepseek",
                "groq",
                "ollama",
                "minimax",
                "mistral",
                "z_ai",
                "nvidia",
                "opencode_zen",
                "cerebras",
                "google_ai_studio",
            ]
            .contains(&name.as_str())
            {
                continue;
            }
            if !value.is_object() {
                continue;
            }
            let existing = provider_config.others.get(name).cloned();
            let mut optional_field = existing;
            update_provider_config(&mut optional_field, value, true);
            if let Some(updated) = optional_field {
                provider_config.others.insert(name.clone(), updated);
            }
        }
    }

    if let Some(channels) = envelope.get("channels").and_then(Value::as_object) {
        let channel_config = &mut config.channels;
        if let Some(data) = channels.get("telegram").and_then(Value::as_object) {
            let mut telegram = channel_config.telegram.clone().unwrap_or_default();
            if let Some(value) = data.get("enabled").and_then(Value::as_bool) {
                telegram.enabled = value;
            }
            if let Some(value) = data.get("bot_token").and_then(Value::as_str) {
                if value != "••••••••" {
                    telegram.bot_token = value.to_string();
                }
            }
            channel_config.telegram = Some(telegram);
        }
        if let Some(data) = channels.get("discord").and_then(Value::as_object) {
            let mut discord = channel_config.discord.clone().unwrap_or_default();
            if let Some(value) = data.get("enabled").and_then(Value::as_bool) {
                discord.enabled = value;
            }
            if let Some(value) = data.get("bot_token").and_then(Value::as_str) {
                if value != "••••••••" {
                    discord.bot_token = value.to_string();
                }
            }
            channel_config.discord = Some(discord);
        }
        if let Some(data) = channels.get("whatsapp").and_then(Value::as_object) {
            let mut whatsapp = channel_config.whatsapp.clone().unwrap_or_default();
            if let Some(value) = data.get("enabled").and_then(Value::as_bool) {
                whatsapp.enabled = value;
            }
            if let Some(value) = data.get("api_key").and_then(Value::as_str) {
                if value != "••••••••" {
                    whatsapp.api_key = value.to_string();
                }
            }
            if let Some(value) = data.get("phone_number_id").and_then(Value::as_str) {
                whatsapp.phone_number_id = value.to_string();
            }
            if let Some(value) = data.get("webhook_port").and_then(Value::as_u64) {
                whatsapp.webhook_port = value as u16;
            }
            if let Some(value) = data.get("verify_token").and_then(Value::as_str) {
                whatsapp.verify_token = value.to_string();
            }
            channel_config.whatsapp = Some(whatsapp);
        }
    }
}

pub(crate) fn config_updated_event(config: &crate::config::schema::Config) -> Value {
    let defaults = &config.agents.defaults;
    super::super::protocol::config_updated(
        serde_json::json!({
            "model": defaults.model,
            "provider": defaults.provider,
            "temperature": defaults.temperature,
            "max_tokens": defaults.max_tokens,
            "streaming": defaults.streaming,
            "caveman_mode": defaults.caveman_mode,
            "security_mode": defaults.security_mode,
            "workspace": defaults.workspace,
            "bot_name": defaults.bot_name,
            "max_messages": defaults.max_messages,
            "max_tool_iterations": defaults.max_tool_iterations,
            "tool_timeout_secs": defaults.tool_timeout_secs,
            "enable_sandbox": defaults.enable_sandbox,
            "context_limit": defaults.context_limit,
            "tool_output_limit": defaults.tool_output_limit,
            "show_auto_capture_notices": defaults.show_auto_capture_notices,
            "tui_thought_display": defaults.tui_thought_display,
        }),
        super::super::webui_capabilities(config),
    )
}
