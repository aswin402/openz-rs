use crate::tools::Tool;
use anyhow::Result;
use serde_json::Value;

pub(crate) fn redact_secrets(val: &mut Value) {
    match val {
        Value::Object(map) => {
            for (key, value) in map.iter_mut() {
                let normalized_key: String = key
                    .chars()
                    .filter(|c| c.is_ascii_alphanumeric())
                    .flat_map(|c| c.to_lowercase())
                    .collect();
                let is_secret = matches!(
                    normalized_key.as_str(),
                    "apikey"
                        | "apitoken"
                        | "accesstoken"
                        | "authtoken"
                        | "bottoken"
                        | "clientsecret"
                        | "password"
                        | "secret"
                        | "verifytoken"
                        | "webhooksecret"
                        | "privatekey"
                        | "token"
                );
                if is_secret && !value.is_null() {
                    *value = Value::String("********".to_string());
                } else {
                    redact_secrets(value);
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

fn merge_provider_credential(
    config: &mut crate::config::schema::Config,
    provider_name: &str,
    credential: &serde_json::Map<String, Value>,
) -> Result<()> {
    let provider_name = provider_name.trim();
    if provider_name.is_empty() {
        anyhow::bail!("provider_name is required for provider credentials");
    }

    let mut provider = config
        .get_provider_config(provider_name)
        .cloned()
        .unwrap_or_default();
    if let Some(api_key) = credential.get("api_key").and_then(|v| v.as_str()) {
        provider.api_key = if api_key.trim().is_empty() {
            None
        } else {
            Some(api_key.to_string())
        };
    }
    if let Some(api_key_env) = credential.get("api_key_env").and_then(|v| v.as_str()) {
        provider.api_key_env = if api_key_env.trim().is_empty() {
            None
        } else {
            Some(api_key_env.to_string())
        };
    }
    if let Some(api_key_file) = credential.get("api_key_file").and_then(|v| v.as_str()) {
        provider.api_key_file = if api_key_file.trim().is_empty() {
            None
        } else {
            Some(api_key_file.to_string())
        };
    }
    if let Some(api_base) = credential.get("api_base").and_then(|v| v.as_str()) {
        provider.api_base = if api_base.trim().is_empty() {
            None
        } else {
            Some(api_base.to_string())
        };
    }
    if let Some(default_model) = credential.get("default_model").and_then(|v| v.as_str()) {
        provider.default_model = if default_model.trim().is_empty() {
            None
        } else {
            Some(default_model.to_string())
        };
    }
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

    if let Some(token) = credential.get("token").and_then(|v| v.as_str()) {
        git_config.token = if token.trim().is_empty() {
            None
        } else {
            Some(token.to_string())
        };
    }
    if let Some(token_env) = credential.get("token_env").and_then(|v| v.as_str()) {
        git_config.token_env = if token_env.trim().is_empty() {
            None
        } else {
            Some(token_env.to_string())
        };
    }
    if let Some(token_file) = credential.get("token_file").and_then(|v| v.as_str()) {
        git_config.token_file = if token_file.trim().is_empty() {
            None
        } else {
            Some(token_file.to_string())
        };
    }
    if let Some(api_base) = credential.get("api_base").and_then(|v| v.as_str()) {
        git_config.api_base = if api_base.trim().is_empty() {
            None
        } else {
            Some(api_base.to_string())
        };
    }

    match service {
        "github" => config.integrations.github = Some(git_config),
        "gitlab" => config.integrations.gitlab = Some(git_config),
        _ => unreachable!(),
    }
    Ok(())
}

fn merge_channel_credential(
    config: &mut crate::config::schema::Config,
    channel: &str,
    credential: &serde_json::Map<String, Value>,
) -> Result<()> {
    match channel {
        "telegram" => {
            let mut cfg = config.channels.telegram.clone().unwrap_or_default();
            if let Some(enabled) = credential.get("enabled").and_then(|v| v.as_bool()) {
                cfg.enabled = enabled;
            }
            if let Some(token) = credential.get("bot_token").and_then(|v| v.as_str()) {
                cfg.bot_token = token.to_string();
            }
            config.channels.telegram = Some(cfg);
        }
        "discord" => {
            let mut cfg = config.channels.discord.clone().unwrap_or_default();
            if let Some(enabled) = credential.get("enabled").and_then(|v| v.as_bool()) {
                cfg.enabled = enabled;
            }
            if let Some(token) = credential.get("bot_token").and_then(|v| v.as_str()) {
                cfg.bot_token = token.to_string();
            }
            config.channels.discord = Some(cfg);
        }
        "whatsapp" => {
            let mut cfg = config.channels.whatsapp.clone().unwrap_or_default();
            if let Some(enabled) = credential.get("enabled").and_then(|v| v.as_bool()) {
                cfg.enabled = enabled;
            }
            if let Some(api_key) = credential.get("api_key").and_then(|v| v.as_str()) {
                cfg.api_key = api_key.to_string();
            }
            if let Some(phone_number_id) =
                credential.get("phone_number_id").and_then(|v| v.as_str())
            {
                cfg.phone_number_id = phone_number_id.to_string();
            }
            if let Some(verify_token) = credential.get("verify_token").and_then(|v| v.as_str()) {
                cfg.verify_token = verify_token.to_string();
            }
            if let Some(webhook_port) = credential.get("webhook_port").and_then(|v| v.as_u64()) {
                cfg.webhook_port = webhook_port as u16;
            }
            config.channels.whatsapp = Some(cfg);
        }
        other => anyhow::bail!("Unsupported channel credential target '{}'", other),
    }
    Ok(())
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
                    "description": "Whether to view config, update defaults, or store a credential. Credential writes require approval."
                },
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
            },
            "required": ["action"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let action = arguments
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing action"))?;

        match action {
            "view" => {
                let config = crate::config::loader::load_config()?;
                let mut config_val = serde_json::to_value(&config)?;
                redact_secrets(&mut config_val);
                Ok(serde_json::json!({
                    "success": true,
                    "config": config_val
                }))
            }
            "update" => {
                let updates = arguments
                    .get("updates")
                    .and_then(|v| v.as_object())
                    .ok_or_else(|| anyhow::anyhow!("Missing updates for action 'update'"))?;

                let mut config = crate::config::loader::load_config()?;

                for (k, v) in updates {
                    match k.as_str() {
                        "firefox_webdriver_port" | "firefox_attach_port" => {
                            let Some(port) = v.as_u64().and_then(|value| u16::try_from(value).ok())
                            else {
                                return Ok(
                                    serde_json::json!({"success": false, "error": format!("{} must be an integer port", k)}),
                                );
                            };
                            if !(1024..=65535).contains(&port) {
                                return Ok(
                                    serde_json::json!({"success": false, "error": format!("{} must be between 1024 and 65535", k)}),
                                );
                            }
                            if k == "firefox_webdriver_port" {
                                config.browser.firefox_webdriver_port = port;
                            } else {
                                config.browser.firefox_attach_port = port;
                            }
                        }
                        "model" => {
                            if let Some(s) = v.as_str() {
                                config.agents.defaults.model = s.to_string();
                            }
                        }
                        "provider" => {
                            if let Some(s) = v.as_str() {
                                config.agents.defaults.provider = s.to_string();
                            }
                        }
                        "max_tokens" => {
                            if let Some(n) = v.as_u64() {
                                config.agents.defaults.max_tokens = n as usize;
                            }
                        }
                        "temperature" => {
                            if let Some(f) = v.as_f64() {
                                config.agents.defaults.temperature = f as f32;
                            }
                        }
                        "caveman_mode" => {
                            if let Some(b) = v.as_bool() {
                                config.agents.defaults.caveman_mode = b;
                            }
                        }
                        "tool_timeout_secs" => {
                            if let Some(n) = v.as_u64() {
                                config.agents.defaults.tool_timeout_secs = n;
                            }
                        }
                        "streaming" => {
                            if let Some(b) = v.as_bool() {
                                config.agents.defaults.streaming = b;
                            }
                        }
                        "show_tool_router_status" => {
                            if let Some(b) = v.as_bool() {
                                config.agents.defaults.show_tool_router_status = b;
                            }
                        }
                        "tui_thought_display" => {
                            if let Some(s) = v.as_str() {
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
                        }
                        "min_free_disk_gb" => {
                            if let Some(n) = v.as_f64() {
                                config.agents.defaults.min_free_disk_gb = n;
                            }
                        }
                        "allow_network_tools" => {
                            if let Some(b) = v.as_bool() {
                                config.agents.defaults.allow_network_tools = b;
                            }
                        }
                        "max_concurrent_process_tools" => {
                            if let Some(n) = v.as_u64() {
                                config.agents.defaults.max_concurrent_process_tools = n as usize;
                            }
                        }
                        "warn_before_expensive_tools" => {
                            if let Some(b) = v.as_bool() {
                                config.agents.defaults.warn_before_expensive_tools = b;
                            }
                        }
                        "max_tool_iterations" => {
                            if let Some(n) = v.as_u64() {
                                config.agents.defaults.max_tool_iterations = n as usize;
                            }
                        }
                        "skills_workspace_skills_enabled" => {
                            if let Some(b) = v.as_bool() {
                                config.skills.workspace_skills_enabled = b;
                            }
                        }
                        "skills_external_dirs" => {
                            if let Some(values) = v.as_array() {
                                config.skills.external_dirs = values
                                    .iter()
                                    .filter_map(|value| value.as_str().map(str::to_string))
                                    .collect();
                            }
                        }
                        "skills_write_approval" => {
                            if let Some(b) = v.as_bool() {
                                config.skills.write_approval = b;
                            }
                        }
                        other => {
                            return Ok(serde_json::json!({
                                "success": false,
                                "error": format!("Cannot modify field '{}' via manage_config", other)
                            }));
                        }
                    }
                }

                crate::config::loader::save_config(&config)?;
                Ok(serde_json::json!({
                    "success": true,
                    "message": "Configuration successfully updated."
                }))
            }
            "set_credential" => {
                let credential = arguments
                    .get("credential")
                    .and_then(|v| v.as_object())
                    .ok_or_else(|| {
                        anyhow::anyhow!("Missing credential for action 'set_credential'")
                    })?;
                let target = credential
                    .get("target")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing credential.target"))?
                    .trim()
                    .to_lowercase();

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
                        merge_provider_credential(&mut config, provider_name, credential)?;
                    }
                    "github" | "gitlab" => merge_git_credential(&mut config, &target, credential)?,
                    "telegram" | "discord" | "whatsapp" => {
                        merge_channel_credential(&mut config, &target, credential)?
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
            _ => Err(anyhow::anyhow!("Invalid action")),
        }
    }
}

