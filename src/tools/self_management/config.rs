use super::{
    extract_first_str, parse_bool_value, parse_f64_value, parse_u64_value, to_non_empty_trimmed,
    ACTION_KEYS,
};
use crate::tools::Tool;
use anyhow::Result;
use serde_json::Value;

pub(crate) fn redact_secrets(val: &mut Value) {
    match val {
        Value::Object(map) => {
            for (key, value) in map.iter_mut() {
                match crate::core::secrets::is_secret_key(key) && !value.is_null() {
                    true => *value = Value::String("********".to_string()),
                    false => redact_secrets(value),
                }
            }
        }
        Value::Array(arr) => {
            for item in arr {
                redact_secrets(item);
            }
        }
        _ => {}
    }
}

fn assign_optional_string_field(
    slot: &mut Option<String>,
    credential: &serde_json::Map<String, Value>,
    keys: &[&str],
) {
    match keys
        .iter()
        .find_map(|&k| credential.get(k).and_then(|v| v.as_str()))
    {
        Some(raw) => *slot = to_non_empty_trimmed(Some(raw)),
        None => {}
    }
}

fn assign_trimmed_string(slot: &mut String, val: &Value) {
    match to_non_empty_trimmed(val.as_str()) {
        Some(s) => *slot = s,
        None => {}
    }
}

fn assign_bool_field(slot: &mut bool, val: &Value) {
    match parse_bool_value(val) {
        Some(b) => *slot = b,
        None => {}
    }
}

fn merge_provider_credential(
    config: &mut crate::config::schema::Config,
    provider_name: &str,
    credential: &serde_json::Map<String, Value>,
) -> Result<()> {
    let provider_name = Some(provider_name.trim())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow::anyhow!("provider_name is required for provider credentials"))?;

    let mut provider = config
        .get_provider_config(provider_name)
        .cloned()
        .unwrap_or_default();

    assign_optional_string_field(
        &mut provider.api_key,
        credential,
        &["api_key", "apiKey", "key", "token"],
    );
    assign_optional_string_field(&mut provider.api_key_env, credential, &["api_key_env"]);
    assign_optional_string_field(&mut provider.api_key_file, credential, &["api_key_file"]);
    assign_optional_string_field(&mut provider.api_base, credential, &["api_base"]);
    assign_optional_string_field(&mut provider.default_model, credential, &["default_model"]);

    config.set_provider_config(provider_name, provider);
    Ok(())
}

fn merge_git_credential(
    config: &mut crate::config::schema::Config,
    service: &str,
    credential: &serde_json::Map<String, Value>,
) -> Result<()> {
    let mut git_config = match service {
        "github" => config.integrations.github.clone().unwrap_or_default(),
        "gitlab" => config.integrations.gitlab.clone().unwrap_or_default(),
        other => anyhow::bail!("Unsupported git credential service '{}'", other),
    };

    assign_optional_string_field(
        &mut git_config.token,
        credential,
        &[
            "token",
            "github_token",
            "gitlab_token",
            "github_pat",
            "gitlab_pat",
            "access_token",
        ],
    );
    assign_optional_string_field(&mut git_config.token_env, credential, &["token_env"]);
    assign_optional_string_field(&mut git_config.token_file, credential, &["token_file"]);
    assign_optional_string_field(&mut git_config.api_base, credential, &["api_base"]);

    match service {
        "github" => config.integrations.github = Some(git_config),
        "gitlab" => config.integrations.gitlab = Some(git_config),
        _ => unreachable!(),
    }
    Ok(())
}

fn extract_credential_map(
    arguments: &serde_json::Map<String, Value>,
) -> (String, serde_json::Map<String, Value>) {
    let mut cred = arguments
        .get("credential")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();

    for (k, v) in arguments {
        let is_meta_key = matches!(
            k.as_str(),
            "credential"
                | "action"
                | "act"
                | "command"
                | "cmd"
                | "op"
                | "mode"
                | "updates"
                | "config"
                | "settings"
                | "params"
        );
        match is_meta_key {
            true => {}
            false => {
                cred.entry(k.clone()).or_insert_with(|| v.clone());
            }
        }
    }

    match arguments
        .get("github_token")
        .or_else(|| arguments.get("github_pat"))
        .or_else(|| arguments.get("github_access_token"))
    {
        Some(tok) => {
            cred.entry("target".to_string())
                .or_insert_with(|| Value::String("github".to_string()));
            cred.insert("token".to_string(), tok.clone());
        }
        None => {}
    }

    match arguments
        .get("gitlab_token")
        .or_else(|| arguments.get("gitlab_pat"))
    {
        Some(tok) => {
            cred.entry("target".to_string())
                .or_insert_with(|| Value::String("gitlab".to_string()));
            cred.insert("token".to_string(), tok.clone());
        }
        None => {}
    }

    match arguments.get("bot_token") {
        Some(tok) => {
            match (
                cred.contains_key("target"),
                arguments.get("channel").and_then(|c| c.as_str()),
            ) {
                (false, Some(chan)) => {
                    cred.insert("target".to_string(), Value::String(chan.to_lowercase()));
                }
                _ => {}
            }
            cred.insert("bot_token".to_string(), tok.clone());
        }
        None => {}
    }

    match arguments
        .get("provider_name")
        .or_else(|| arguments.get("provider"))
    {
        Some(pname) => {
            cred.entry("target".to_string())
                .or_insert_with(|| Value::String("provider".to_string()));
            cred.insert("provider_name".to_string(), pname.clone());
        }
        None => {}
    }

    let explicit_target = cred
        .get("target")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty());

    let target = explicit_target.unwrap_or_else(|| {
        let inferred = [
            (cred.contains_key("provider_name"), "provider"),
            (
                cred.contains_key("token")
                    || cred.contains_key("github_token")
                    || cred.contains_key("github_pat"),
                "github",
            ),
            (
                cred.contains_key("phone_number_id") || cred.contains_key("verify_token"),
                "whatsapp",
            ),
            (cred.contains_key("bot_token"), "telegram"),
        ]
        .into_iter()
        .find_map(|(matched, name)| matched.then_some(name))
        .unwrap_or("");
        inferred.to_string()
    });

    (target, cred)
}

fn merge_channel_credential(
    config: &mut crate::config::schema::Config,
    channel: &str,
    credential: &serde_json::Map<String, Value>,
) -> Result<()> {
    match channel {
        "telegram" => {
            let mut cfg = config.channels.telegram.clone().unwrap_or_default();
            cfg.enabled = credential
                .get("enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(cfg.enabled);
            cfg.bot_token = credential
                .get("bot_token")
                .and_then(|v| v.as_str())
                .map(ToString::to_string)
                .unwrap_or(cfg.bot_token);
            config.channels.telegram = Some(cfg);
        }
        "discord" => {
            let mut cfg = config.channels.discord.clone().unwrap_or_default();
            cfg.enabled = credential
                .get("enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(cfg.enabled);
            cfg.bot_token = credential
                .get("bot_token")
                .and_then(|v| v.as_str())
                .map(ToString::to_string)
                .unwrap_or(cfg.bot_token);
            config.channels.discord = Some(cfg);
        }
        "whatsapp" => {
            let mut cfg = config.channels.whatsapp.clone().unwrap_or_default();
            cfg.enabled = credential
                .get("enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(cfg.enabled);
            cfg.api_key = credential
                .get("api_key")
                .and_then(|v| v.as_str())
                .map(ToString::to_string)
                .unwrap_or(cfg.api_key);
            cfg.phone_number_id = credential
                .get("phone_number_id")
                .and_then(|v| v.as_str())
                .map(ToString::to_string)
                .unwrap_or(cfg.phone_number_id);
            cfg.verify_token = credential
                .get("verify_token")
                .and_then(|v| v.as_str())
                .map(ToString::to_string)
                .unwrap_or(cfg.verify_token);
            cfg.webhook_port = credential
                .get("webhook_port")
                .and_then(|v| v.as_u64())
                .map(|p| p as u16)
                .unwrap_or(cfg.webhook_port);
            config.channels.whatsapp = Some(cfg);
        }
        other => anyhow::bail!("Unsupported channel credential target '{}'", other),
    }
    Ok(())
}

fn insert_git_integration_summary(
    integrations_map: &mut serde_json::Map<String, Value>,
    key: &str,
    git_cfg: Option<&crate::config::schema::GitIntegrationConfig>,
) {
    integrations_map
        .entry(key.to_string())
        .or_insert_with(|| {
            serde_json::json!({
                "token_configured": git_cfg.and_then(|g| g.token.as_ref()).is_some(),
                "token_env": git_cfg.and_then(|g| g.token_env.as_ref()),
                "token_file": git_cfg.and_then(|g| g.token_file.as_ref()),
                "api_base": git_cfg.and_then(|g| g.api_base.as_ref()),
            })
        });
}

pub struct ManageConfigTool;

#[async_trait::async_trait]
impl Tool for ManageConfigTool {
    fn name(&self) -> &str {
        "manage_config"
    }

    fn description(&self) -> &str {
        "View redacted configuration, update agent defaults, or store credentials after explicit user approval. Supports provider, GitHub/GitLab, and channel credential updates."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["view", "update", "set_credential"],
                    "description": "Whether to view config, update defaults, or store a credential. Inferred automatically if credential fields or target are passed."
                },
                "target": {
                    "type": "string",
                    "enum": ["provider", "github", "gitlab", "telegram", "discord", "whatsapp"],
                    "description": "Credential target to store. Can be specified directly at top level or inside 'credential'."
                },
                "token": { "type": "string", "description": "GitHub or GitLab personal access token." },
                "github_token": { "type": "string", "description": "Convenience alias to store a GitHub personal access token (target=github)." },
                "gitlab_token": { "type": "string", "description": "Convenience alias to store a GitLab token (target=gitlab)." },
                "provider_name": { "type": "string", "description": "Provider id for target=provider, such as openai, anthropic, groq, deepseek, openrouter, opencode_zen." },
                "api_key": { "type": "string", "description": "API key for LLM provider or WhatsApp." },
                "bot_token": { "type": "string", "description": "Telegram or Discord bot token." },
                "api_base": { "type": "string", "description": "Optional API base URL for provider/GitHub/GitLab." },
                "updates": {
                    "type": "object",
                    "properties": {
                        "firefox_webdriver_port": {
                            "type": "integer",
                            "minimum": 1024,
                            "maximum": 65535,
                            "description": "Firefox WebDriver port for headless and visible sessions."
                        },
                        "firefox_attach_port": {
                            "type": "integer",
                            "minimum": 1024,
                            "maximum": 65535,
                            "description": "Dedicated WebDriver port for attaching to an existing Firefox session."
                        },
                        "model": {
                            "type": "string",
                            "description": "Default model prefix to use (e.g. 'openai/gpt-4o')."
                        },
                        "provider": {
                            "type": "string",
                            "description": "Default provider (e.g. 'openai')."
                        },
                        "max_tokens": {
                            "type": "integer",
                            "description": "Maximum completion tokens."
                        },
                        "temperature": {
                            "type": "number",
                            "description": "Generation temperature."
                        },
                        "caveman_mode": {
                            "type": "boolean",
                            "description": "Toggles terse/concise system prompt instructions."
                        },
                        "tool_timeout_secs": {
                            "type": "integer",
                            "description": "Max timeout for tool executions."
                        },
                        "streaming": {
                            "type": "boolean",
                            "description": "Enable/disable token response streaming."
                        },
                        "show_tool_router_status": {
                            "type": "boolean",
                            "description": "Show compact tool-router selection summaries in the TUI."
                        },
                        "tui_thought_display": {
                            "type": "string",
                            "enum": ["full", "compact", "off"],
                            "description": "Controls TUI reasoning display: full Thought blocks with seconds, compact summaries, or off."
                        },
                        "min_free_disk_gb": {
                            "type": "number",
                            "description": "Minimum free disk space required before disk-writing tools run."
                        },
                        "allow_network_tools": {
                            "type": "boolean",
                            "description": "Allow tools marked as network-capable to run."
                        },
                        "max_concurrent_process_tools": {
                            "type": "integer",
                            "description": "Maximum active process-spawning tools allowed before new process tools are blocked."
                        },
                        "warn_before_expensive_tools": {
                            "type": "boolean",
                            "description": "Require approval before tools that combine network/process/disk behavior."
                        },
                        "max_tool_iterations": {
                            "type": "integer",
                            "description": "Maximum execution steps per turn."
                        },
                        "skills_workspace_skills_enabled": {
                            "type": "boolean",
                            "description": "Enable workspace-scoped skills from .openz/skills."
                        },
                        "skills_external_dirs": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Additional skill directories to scan after the OpenZ skill store."
                        },
                        "skills_write_approval": {
                            "type": "boolean",
                            "description": "Require approval/staging before agent-created skill writes."
                        },
                        "github_token": {
                            "type": "string",
                            "description": "GitHub personal access token."
                        },
                        "gitlab_token": {
                            "type": "string",
                            "description": "GitLab personal access token."
                        }
                    },
                    "description": "Key-value map of configuration defaults to update. Ignored for action 'view'."
                },
                "credential": {
                    "type": "object",
                    "properties": {
                        "target": {
                            "type": "string",
                            "enum": ["provider", "github", "gitlab", "telegram", "discord", "whatsapp"],
                            "description": "Credential target to update. Use provider with provider_name for LLM providers."
                        },
                        "provider_name": {
                            "type": "string",
                            "description": "Provider id for target=provider, such as openai, anthropic, openrouter, opencode_zen, or a custom provider key."
                        },
                        "api_key": { "type": "string", "description": "API key for provider or WhatsApp." },
                        "api_key_env": { "type": "string", "description": "Environment variable name that contains a provider API key." },
                        "api_key_file": { "type": "string", "description": "Path to a local file containing a provider API key." },
                        "token": { "type": "string", "description": "GitHub/GitLab token." },
                        "token_env": { "type": "string", "description": "Environment variable name that contains a GitHub/GitLab token." },
                        "token_file": { "type": "string", "description": "Path to a local file containing a GitHub/GitLab token." },
                        "bot_token": { "type": "string", "description": "Telegram or Discord bot token." },
                        "api_base": { "type": "string", "description": "Optional API base URL for provider/GitHub/GitLab." },
                        "default_model": { "type": "string", "description": "Optional preferred model for an LLM provider." },
                        "enabled": { "type": "boolean", "description": "Enable or disable channel after credential update." },
                        "phone_number_id": { "type": "string", "description": "WhatsApp phone number id." },
                        "verify_token": { "type": "string", "description": "WhatsApp webhook verify token." },
                        "webhook_port": { "type": "integer", "description": "WhatsApp webhook port." }
                    },
                    "description": "Credential update payload. Values are saved to ~/.openz/config.json and redacted by view."
                }
            }
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let mut obj_map = serde_json::Map::new();
        let (action_str, direct_updates) = match arguments {
            Value::String(s) => (Some(s.to_string()), None),
            Value::Object(obj) => {
                obj_map = obj.clone();
                let act = extract_first_str(arguments, ACTION_KEYS).map(ToString::to_string);
                (act, Some(obj))
            }
            _ => (None, None),
        };

        let has_credential_material = obj_map.contains_key("credential")
            || obj_map.contains_key("target")
            || obj_map.contains_key("github_token")
            || obj_map.contains_key("github_pat")
            || obj_map.contains_key("gitlab_token")
            || obj_map.contains_key("gitlab_pat")
            || obj_map.contains_key("bot_token")
            || (obj_map.contains_key("token")
                && (obj_map.contains_key("target")
                    || obj_map.contains_key("github")
                    || obj_map.contains_key("gitlab")))
            || (obj_map.contains_key("api_key")
                && (obj_map.contains_key("target")
                    || obj_map.contains_key("provider_name")
                    || obj_map.contains_key("provider")));

        let normalized_action = match action_str {
            Some(a) => {
                let trimmed = a.trim().to_lowercase();
                let redirect_to_cred = matches!(trimmed.as_str(), "update" | "set")
                    && has_credential_material
                    && !obj_map.contains_key("updates");
                match redirect_to_cred {
                    true => "set_credential".to_string(),
                    false => trimmed,
                }
            }
            None => match (has_credential_material, obj_map.contains_key("updates")) {
                (true, _) => "set_credential".to_string(),
                (false, true) => "update".to_string(),
                (false, false) => "view".to_string(),
            },
        };

        let action = match normalized_action.as_str() {
            "" | "view" | "show" | "get" | "read" | "list" | "inspect" => "view",
            "update" | "set" | "modify" | "write" => "update",
            "set_credential" | "credential" | "credentials" => "set_credential",
            other => other,
        };

        match action {
            "view" => {
                let config = crate::config::loader::load_config()?;
                let mut config_val = serde_json::to_value(&config)?;
                redact_secrets(&mut config_val);

                match config_val
                    .get_mut("integrations")
                    .and_then(|v| v.as_object_mut())
                {
                    Some(integrations_map) => {
                        insert_git_integration_summary(
                            integrations_map,
                            "github",
                            config.integrations.github.as_ref(),
                        );
                        insert_git_integration_summary(
                            integrations_map,
                            "gitlab",
                            config.integrations.gitlab.as_ref(),
                        );
                    }
                    None => {}
                }

                Ok(serde_json::json!({
                    "success": true,
                    "config": config_val,
                    "supported_credential_targets": [
                        "github",
                        "gitlab",
                        "provider",
                        "telegram",
                        "discord",
                        "whatsapp"
                    ],
                    "credential_instructions": "To store GitHub, GitLab, LLM provider, or channel credentials into config, call manage_config with action: 'set_credential' (or pass target and token/api_key directly). OpenZ requests user confirmation via SecurityGuard before persisting."
                }))
            }
            "update" => {
                let updates_opt = ["updates", "config", "settings", "params"]
                    .iter()
                    .find_map(|&k| arguments.get(k).and_then(|v| v.as_object()));
                let updates = match updates_opt {
                    Some(m) => m,
                    None => direct_updates
                        .ok_or_else(|| anyhow::anyhow!("Missing updates for action 'update'"))?,
                };

                let mut config = crate::config::loader::load_config()?;

                for (k, v) in updates {
                    match k.as_str() {
                        "action" | "act" | "command" | "cmd" | "op" | "mode" => {}
                        "firefox_webdriver_port" | "firefox_attach_port" => {
                            let port = match parse_u64_value(v)
                                .and_then(|value| u16::try_from(value).ok())
                            {
                                Some(p) => p,
                                None => {
                                    return Ok(serde_json::json!({
                                        "success": false,
                                        "error": format!("{} must be an integer port", k)
                                    }));
                                }
                            };
                            match (1024..=65535).contains(&port) {
                                false => {
                                    return Ok(serde_json::json!({
                                        "success": false,
                                        "error": format!("{} must be between 1024 and 65535", k)
                                    }));
                                }
                                true => match k.as_str() {
                                    "firefox_webdriver_port" => {
                                        config.browser.firefox_webdriver_port = port;
                                    }
                                    _ => {
                                        config.browser.firefox_attach_port = port;
                                    }
                                },
                            }
                        }
                        "model" => assign_trimmed_string(&mut config.agents.defaults.model, v),
                        "provider" => {
                            assign_trimmed_string(&mut config.agents.defaults.provider, v)
                        }
                        "max_tokens" => match parse_u64_value(v) {
                            Some(n) => config.agents.defaults.max_tokens = n as usize,
                            None => {}
                        },
                        "temperature" => match parse_f64_value(v) {
                            Some(f) => config.agents.defaults.temperature = f as f32,
                            None => {}
                        },
                        "caveman_mode" => {
                            assign_bool_field(&mut config.agents.defaults.caveman_mode, v)
                        }
                        "tool_timeout_secs" => match parse_u64_value(v) {
                            Some(n) => config.agents.defaults.tool_timeout_secs = n,
                            None => {}
                        },
                        "streaming" => assign_bool_field(&mut config.agents.defaults.streaming, v),
                        "show_tool_router_status" => {
                            assign_bool_field(
                                &mut config.agents.defaults.show_tool_router_status,
                                v,
                            )
                        }
                        "tui_thought_display" => match v.as_str() {
                            Some(s) => {
                                let mode = match s.trim().to_lowercase().as_str() {
                                    "full" | "on" | "default" => "full",
                                    "compact" | "summary" => "compact",
                                    "off" | "none" | "hide" => "off",
                                    other => {
                                        return Ok(serde_json::json!({
                                            "success": false,
                                            "error": format!("Invalid tui_thought_display '{}'. Use full, compact, or off.", other)
                                        }));
                                    }
                                };
                                config.agents.defaults.tui_thought_display = mode.to_string();
                            }
                            None => {}
                        },
                        "min_free_disk_gb" => match parse_f64_value(v) {
                            Some(n) => config.agents.defaults.min_free_disk_gb = n,
                            None => {}
                        },
                        "allow_network_tools" => {
                            assign_bool_field(&mut config.agents.defaults.allow_network_tools, v)
                        }
                        "max_concurrent_process_tools" => match parse_u64_value(v) {
                            Some(n) => {
                                config.agents.defaults.max_concurrent_process_tools = n as usize
                            }
                            None => {}
                        },
                        "warn_before_expensive_tools" => {
                            assign_bool_field(
                                &mut config.agents.defaults.warn_before_expensive_tools,
                                v,
                            )
                        }
                        "max_tool_iterations" => match parse_u64_value(v) {
                            Some(n) => config.agents.defaults.max_tool_iterations = n as usize,
                            None => {}
                        },
                        "skills_workspace_skills_enabled" => {
                            assign_bool_field(&mut config.skills.workspace_skills_enabled, v)
                        }
                        "skills_external_dirs" => match v.as_array() {
                            Some(values) => {
                                config.skills.external_dirs = values
                                    .iter()
                                    .filter_map(|value| value.as_str().map(str::to_string))
                                    .collect();
                            }
                            None => {}
                        },
                        "skills_write_approval" => {
                            assign_bool_field(&mut config.skills.write_approval, v)
                        }
                        "github_token" | "github_pat" | "github_access_token" => {
                            let mut git_config =
                                config.integrations.github.clone().unwrap_or_default();
                            git_config.token = to_non_empty_trimmed(v.as_str());
                            config.integrations.github = Some(git_config);
                        }
                        "gitlab_token" | "gitlab_pat" => {
                            let mut git_config =
                                config.integrations.gitlab.clone().unwrap_or_default();
                            git_config.token = to_non_empty_trimmed(v.as_str());
                            config.integrations.gitlab = Some(git_config);
                        }
                        "vault_path" | "vault_dir" => {
                            assign_trimmed_string(&mut config.vault.path, v)
                        }
                        "vault_name" => assign_trimmed_string(&mut config.vault.name, v),
                        "vault_enabled" => assign_bool_field(&mut config.vault.enabled, v),
                        "vault_auto_create" => assign_bool_field(&mut config.vault.auto_create, v),
                        "vault" => match v.as_object() {
                            Some(obj) => {
                                match obj.get("path").and_then(|p| p.as_str()) {
                                    Some(path) => config.vault.path = path.trim().to_string(),
                                    None => {}
                                }
                                match obj.get("name").and_then(|p| p.as_str()) {
                                    Some(name) => config.vault.name = name.trim().to_string(),
                                    None => {}
                                }
                                match obj.get("enabled").and_then(|p| p.as_bool()) {
                                    Some(enabled) => config.vault.enabled = enabled,
                                    None => {}
                                }
                                match obj
                                    .get("auto_create")
                                    .or_else(|| obj.get("autoCreate"))
                                    .and_then(|p| p.as_bool())
                                {
                                    Some(auto_create) => config.vault.auto_create = auto_create,
                                    None => {}
                                }
                            }
                            None => {}
                        },
                        "target" => {
                            let target_str = v.as_str().unwrap_or("").trim().to_lowercase();
                            match target_str.as_str() {
                                "github" | "gitlab" => {
                                    merge_git_credential(&mut config, &target_str, updates)?;
                                }
                                "provider" => {
                                    match updates.get("provider_name").and_then(|p| p.as_str()) {
                                        Some(pname) => {
                                            merge_provider_credential(&mut config, pname, updates)?;
                                        }
                                        None => {}
                                    }
                                }
                                "telegram" | "discord" | "whatsapp" => {
                                    merge_channel_credential(&mut config, &target_str, updates)?;
                                }
                                _ => {}
                            }
                        }
                        "token" | "provider_name" | "bot_token" | "credential" => {}
                        other => {
                            return Ok(serde_json::json!({
                                "success": false,
                                "error": format!("Cannot modify field '{}' via manage_config", other)
                            }));
                        }
                    }
                }

                crate::config::loader::save_config(&config)?;
                let _ = crate::core::vault::ensure_vault_initialized(&config);
                Ok(serde_json::json!({
                    "success": true,
                    "message": "Configuration successfully updated."
                }))
            }
            "set_credential" => {
                let (target, credential) = extract_credential_map(&obj_map);
                match target.is_empty() {
                    true => {
                        return Ok(serde_json::json!({
                            "success": false,
                            "error": "Missing credential target. Specify target ('github', 'gitlab', 'provider', 'telegram', 'discord', 'whatsapp') or use a dedicated token field (e.g. 'github_token')."
                        }));
                    }
                    false => {}
                }

                let mut config = crate::config::loader::load_config()?;
                match target.as_str() {
                    "provider" => {
                        let provider_name = credential
                            .get("provider_name")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| {
                                anyhow::anyhow!(
                                    "credential.provider_name is required for provider credentials"
                                )
                            })?;
                        merge_provider_credential(&mut config, provider_name, &credential)?;
                    }
                    "github" | "gitlab" => merge_git_credential(&mut config, &target, &credential)?,
                    "telegram" | "discord" | "whatsapp" => {
                        merge_channel_credential(&mut config, &target, &credential)?
                    }
                    other => {
                        return Ok(serde_json::json!({
                            "success": false,
                            "error": format!("Unsupported credential target '{}'", other)
                        }));
                    }
                }

                crate::config::loader::save_config(&config)?;
                Ok(serde_json::json!({
                    "success": true,
                    "message": format!("Credential target '{}' updated. Secrets are redacted in config views.", target)
                }))
            }
            _ => Err(anyhow::anyhow!("Invalid action '{}'", action)),
        }
    }
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
