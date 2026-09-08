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
    if !risk.risky {
        return String::new();
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
    if let Some(record) = health {
        if record.failure_count > 0
            || record.blank_response_count > 0
            || record.think_leak_count > 0
        {
            out.push_str(&format!(
                "\n- prior health: {} failures, {} blank replies, {} think leaks",
                record.failure_count, record.blank_response_count, record.think_leak_count
            ));
        }
    }
    out
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
        let label = format!("★ {}", fav.model);
        if !out.iter().any(|existing| existing == &label) {
            out.push(label);
        }
    }
    for recent in prefs
        .recent
        .iter()
        .filter(|entry| entry.provider == provider)
    {
        let label = format!("◷ {}", recent.model);
        if !out.iter().any(|existing| existing == &label) {
            out.push(label);
        }
    }
    for model in models {
        if !out
            .iter()
            .any(|existing| model_menu_model_name(existing).eq_ignore_ascii_case(&model))
        {
            out.push(model);
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
    if parts.next() != Some("/switch-model") {
        return ModelSwitchCommand::None;
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
    if command == ModelSwitchCommand::None {
        return None;
    }
    let config = match crate::config::loader::load_config() {
        Ok(config) => config,
        Err(e) => return Some(format!("Failed to load OpenZ config: {e}")),
    };
    Some(render_model_switch_command(&config, command))
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
    if providers.is_empty() {
        return "No configured LLM providers found. Run `openz configure` first.".to_string();
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
    let Some(provider_models) = provider_model_option_by_name(config, provider) else {
        return format!("Unknown provider `{provider}`. Use `/switch-model` to list providers.");
    };
    if !config.is_provider_available(provider) {
        return format!(
            "Provider `{provider}` is not configured. Run `openz configure` or set its API key first."
        );
    }

    let mut response = format!(
        "Models for `{}` ({}):\n",
        provider_models.name, provider_models.display
    );
    if provider_models.models.is_empty() {
        response.push_str("- Type any OpenAI-compatible model name manually\n");
    } else {
        for model in &provider_models.models {
            response.push_str(&format!("- `{model}`\n"));
        }
    }
    response.push_str(&format!(
        "\nUsage: `/switch-model {} <model>`",
        provider_models.name
    ));
    response
}

fn spawn_model_smoke_test(mut config: crate::config::schema::Config, provider: &str, model: &str) {
    let Ok(handle) = tokio::runtime::Handle::try_current() else {
        return;
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
                if ok {
                    let _ = crate::model_registry::record_model_success(
                        &provider, &model, risk.tier, risk.risky, reasons, blank, think_leak, false,
                    );
                } else {
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
    if provider_models_by_name(provider).is_none() && !base_config.is_custom_provider(provider) {
        anyhow::bail!("unknown provider `{provider}`");
    }
    if !base_config.is_provider_available(provider) {
        anyhow::bail!("provider `{provider}` is not configured");
    }
    if model.trim().is_empty() {
        anyhow::bail!("model cannot be empty");
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
    crate::config::loader::save_config(&config)?;
    spawn_model_smoke_test(config, provider, model);
    Ok(())
}
