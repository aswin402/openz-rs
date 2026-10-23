use crate::channels::model_catalog::{
    configured_provider_model_options, provider_model_option_by_name, provider_models_by_name,
};
use crate::providers::model_prefs::load_model_prefs;
use crate::providers::risk::classify_model_risk;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelSwitchCommand {
    None,
    ShowProviders,
    ShowModels { provider: String },
    Set { provider: String, model: String },
}

pub fn render_model_risk_warning(provider: &str, model: &str) -> String {
    let risk = classify_model_risk(provider, model);
    match risk.risky {
        false => return String::new(),
        true => {}
    }
    let registry = crate::model_registry::ModelRegistry::load();
    let health = registry.get(provider, model);
    let mut out = format!(
        "\n\nWarning: `{model}` via `{provider}` is marked `{}`. OpenZ will still allow it, but weak-model prompt safeguards will be used when applicable.",
        risk.tier
    );
    for reason in risk.reasons {
        out.push_str(&format!("\n- {reason}"));
    }
    match health {
        Some(record) => match record.failure_count > 0
            || record.blank_response_count > 0
            || record.think_leak_count > 0
        {
            true => out.push_str(&format!(
                "\n- prior health: {} failures, {} blank replies, {} think leaks",
                record.failure_count, record.blank_response_count, record.think_leak_count
            )),
            false => {}
        },
        None => {}
    }
    out
}

fn push_unique_label(out: &mut Vec<String>, label: String) {
    match out.iter().any(|existing| existing == &label) {
        true => {}
        false => out.push(label),
    }
}

pub fn model_menu_options_with_prefs(provider: &str, models: Vec<String>) -> Vec<String> {
    let prefs = load_model_prefs();
    let mut out = Vec::new();
    out.push("★ Favorite/Unfavorite current model".to_string());

    for fav in prefs
        .favorites
        .iter()
        .filter(|entry| entry.provider == provider)
    {
        push_unique_label(&mut out, format!("★ {}", fav.model));
    }
    for recent in prefs
        .recent
        .iter()
        .filter(|entry| entry.provider == provider)
    {
        push_unique_label(&mut out, format!("◷ {}", recent.model));
    }
    for model in models {
        match out
            .iter()
            .any(|existing| model_menu_model_name(existing).eq_ignore_ascii_case(&model))
        {
            true => {}
            false => out.push(model),
        }
    }
    out
}

pub fn model_menu_model_name(item: &str) -> &str {
    item.strip_prefix("★ ")
        .or_else(|| item.strip_prefix("◷ "))
        .unwrap_or(item)
}

pub fn parse_model_switch_command(text: &str) -> ModelSwitchCommand {
    let trimmed = text.trim();
    let mut parts = trimmed.split_whitespace();
    match parts.next() {
        Some("/switch-model") => {}
        _ => return ModelSwitchCommand::None,
    }
    match (parts.next(), parts.next()) {
        (None, _) => ModelSwitchCommand::ShowProviders,
        (Some(provider), None) => ModelSwitchCommand::ShowModels {
            provider: provider.to_string(),
        },
        (Some(provider), Some(model)) => {
            let mut model_name = model.to_string();
            for rest in parts {
                model_name.push(' ');
                model_name.push_str(rest);
            }
            ModelSwitchCommand::Set {
                provider: provider.to_string(),
                model: model_name,
            }
        }
    }
}

pub fn model_switch_text_response(text: &str) -> Option<String> {
    let command = parse_model_switch_command(text);
    match command {
        ModelSwitchCommand::None => None,
        command => {
            let config = match crate::config::loader::load_config() {
                Ok(config) => config,
                Err(e) => return Some(format!("Failed to load OpenZ config: {e}")),
            };
            Some(render_model_switch_command(&config, command))
        }
    }
}

pub fn render_model_switch_command(
    config: &crate::config::schema::Config,
    command: ModelSwitchCommand,
) -> String {
    match command {
        ModelSwitchCommand::None => String::new(),
        ModelSwitchCommand::ShowProviders => render_model_switch_providers(config),
        ModelSwitchCommand::ShowModels { provider } => {
            render_model_switch_models(config, &provider)
        }
        ModelSwitchCommand::Set { provider, model } => {
            match save_default_model_selection(config, &provider, &model) {
                Ok(()) => format!(
                    "Model switched to `{}` with provider `{}`. New channel turns will use this default.{}",
                    model,
                    provider,
                    render_model_risk_warning(&provider, &model)
                ),
                Err(e) => format!("Failed to switch model: {e}"),
            }
        }
    }
}

pub fn render_model_switch_providers(config: &crate::config::schema::Config) -> String {
    let providers = configured_provider_model_options(config);
    match providers.is_empty() {
        true => return "No configured LLM providers found. Run `openz configure` first.".to_string(),
        false => {}
    }

    let mut response = format!(
        "Current default: `{}` via `{}`\n\nChoose a provider:\n",
        config.agents.defaults.model, config.agents.defaults.provider
    );
    for provider in providers {
        response.push_str(&format!("- `{}` ({})\n", provider.name, provider.display));
    }
    response.push_str("\nUsage: `/switch-model <provider>` to list models, then `/switch-model <provider> <model>` to switch.");
    response
}

pub fn render_model_switch_models(
    config: &crate::config::schema::Config,
    provider: &str,
) -> String {
    let provider_models = match provider_model_option_by_name(config, provider) {
        Some(provider_models) => provider_models,
        None => {
            return format!(
                "Unknown provider `{provider}`. Use `/switch-model` to list providers."
            );
        }
    };
    match config.is_provider_available(provider) {
        false => {
            return format!(
                "Provider `{provider}` is not configured. Run `openz configure` or set its API key first."
            );
        }
        true => {}
    }

    let mut response = format!(
        "Models for `{}` ({}):\n",
        provider_models.name, provider_models.display
    );
    match provider_models.models.is_empty() {
        true => response.push_str("- Type any OpenAI-compatible model name manually\n"),
        false => {
            for model in &provider_models.models {
                response.push_str(&format!("- `{model}`\n"));
            }
        }
    }
    response.push_str(&format!(
        "\nUsage: `/switch-model {} <model>`",
        provider_models.name
    ));
    response
}

fn spawn_model_smoke_test(mut config: crate::config::schema::Config, provider: &str, model: &str) {
    let handle = match tokio::runtime::Handle::try_current() {
        Ok(handle) => handle,
        Err(_) => return,
    };
    let provider = provider.to_string();
    let model = model.trim().to_string();
    config.agents.defaults.provider = provider.clone();
    config.agents.defaults.model = model.clone();
    handle.spawn(async move {
        let risk = classify_model_risk(&provider, &model);
        let reasons: Vec<String> = risk
            .reasons
            .iter()
            .map(|reason| reason.to_string())
            .collect();
        let provider_instance =
            match crate::providers::resolver::resolve_provider_full(&config, &model) {
                Ok(resolved) => resolved.instance,
                Err(err) => {
                    let _ = crate::model_registry::record_model_failure(
                        &provider,
                        &model,
                        risk.tier,
                        risk.risky,
                        reasons,
                        &format!("resolve failed during smoke test: {err}"),
                    );
                    return;
                }
            };
        let settings = crate::providers::GenerationSettings {
            temperature: 0.0,
            max_tokens: 32,
            reasoning_effort: None,
        };
        let messages = vec![crate::session::Message {
            role: "user".to_string(),
            content: "Reply exactly: OPENZ_MODEL_OK".to_string(),
            timestamp: None,
            extra: serde_json::Map::new(),
        }];
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(12),
            provider_instance.chat(
                "You are OpenZ model validation. Reply exactly with the requested token.",
                &messages,
                &[],
                &settings,
            ),
        )
        .await;
        match result {
            Ok(Ok(resp)) => {
                let content = resp.content.unwrap_or_default();
                let blank = content.trim().is_empty();
                let think_leak = content.contains("<think>") || content.contains("</think>");
                let ok = content.trim().contains("OPENZ_MODEL_OK") && !blank;
                match ok {
                    true => {
                        let _ = crate::model_registry::record_model_success(
                            &provider,
                            &model,
                            risk.tier,
                            risk.risky,
                            reasons,
                            blank,
                            think_leak,
                            false,
                        );
                    }
                    false => {
                        let _ = crate::model_registry::record_model_failure(
                            &provider,
                            &model,
                            risk.tier,
                            risk.risky,
                            reasons,
                            &format!("smoke test unexpected response: {}", content.trim()),
                        );
                    }
                }
            }
            Ok(Err(err)) => {
                let _ = crate::model_registry::record_model_failure(
                    &provider,
                    &model,
                    risk.tier,
                    risk.risky,
                    reasons,
                    &format!("smoke test provider error: {err}"),
                );
            }
            Err(_) => {
                let _ = crate::model_registry::record_model_failure(
                    &provider,
                    &model,
                    risk.tier,
                    risk.risky,
                    reasons,
                    "smoke test timed out after 12s",
                );
            }
        }
    });
}

pub fn save_default_model_selection(
    base_config: &crate::config::schema::Config,
    provider: &str,
    model: &str,
) -> anyhow::Result<()> {
    match provider_models_by_name(provider).is_none() && !base_config.is_custom_provider(provider) {
        true => anyhow::bail!("unknown provider `{provider}`"),
        false => {}
    }
    match base_config.is_provider_available(provider) {
        false => anyhow::bail!("provider `{provider}` is not configured"),
        true => {}
    }
    match model.trim().is_empty() {
        true => anyhow::bail!("model cannot be empty"),
        false => {}
    }

    let risk = classify_model_risk(provider, model);
    let _ = crate::model_registry::record_model_risk(
        provider,
        model.trim(),
        risk.tier,
        risk.risky,
        risk.reasons
            .iter()
            .map(|reason| reason.to_string())
            .collect(),
    );

    let mut config = crate::config::loader::load_config().unwrap_or_else(|_| base_config.clone());
    config.agents.defaults.provider = provider.to_string();
    config.agents.defaults.model = model.trim().to_string();
    crate::channels::record_recent_model(provider, model.trim());
    crate::config::loader::save_config(&config)?;
    spawn_model_smoke_test(config, provider, model);
    Ok(())
}
