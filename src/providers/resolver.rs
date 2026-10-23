use crate::config::provider_catalog::{
    environment_keys_for_provider, find_provider, keyword_provider_candidates,
    provider_prefix_for_model,
};
use crate::config::schema::Config;
use crate::providers::{anthropic::AnthropicProvider, openai::OpenAIProvider, LLMProvider};
use anyhow::{anyhow, Result};
use std::sync::Arc;

/// Result of the full provider resolution pipeline.
pub struct ResolvedProvider {
    pub provider_name: String,
    pub api_key: String,
    pub api_base: String,
    pub model: String,
    pub instance: Arc<dyn LLMProvider>,
}

const FALLBACK_MODEL_RULES: &[(&str, &[(&str, &str)], &str)] = &[
    (
        "openrouter",
        &[
            ("claude", "google/gemini-2.0-flash-exp:free"),
            ("gpt", "meta-llama/llama-3.3-70b-instruct:free"),
        ],
        "google/gemini-2.0-flash-exp:free",
    ),
    (
        "opencode_zen",
        &[
            ("claude", "mimo-v2.5-free"),
            ("gpt", "nemotron-3-ultra-free"),
        ],
        "deepseek-v4-flash-free",
    ),
];

fn provider_allows_empty_key(config: &Config, provider_name: &str) -> bool {
    find_provider(provider_name).is_some_and(|descriptor| descriptor.local)
        || config.custom_provider_allows_empty_key(provider_name)
}

/// Resolve API key and base URL for a given provider name from config + env vars.
pub fn resolve_api_config(config: &Config, provider_name: &str) -> (String, String) {
    let (key, base) = config.resolve_provider_config(provider_name);
    match key.is_empty() && !provider_allows_empty_key(config, provider_name) {
        true => tracing::warn!(
            "No API key configured for provider '{}'. Requests will likely fail with 401.",
            provider_name
        ),
        false => {}
    }
    (key, base)
}

fn provider_api_key_env_var(provider_name: &str) -> String {
    environment_keys_for_provider(provider_name)
        .first()
        .copied()
        .map(str::to_string)
        .unwrap_or_else(|| Config::custom_provider_env_var(provider_name))
}

pub fn resolve_fallback_model(target_provider: &str, original_model: &str) -> String {
    let original_lower = original_model.to_lowercase();
    FALLBACK_MODEL_RULES
        .iter()
        .find(|(prov, _, _)| *prov == target_provider)
        .map(|(_, rules, default_model)| {
            rules
                .iter()
                .find(|(needle, _)| original_lower.contains(needle))
                .map(|(_, model)| (*model).to_string())
                .unwrap_or_else(|| (*default_model).to_string())
        })
        .unwrap_or_else(|| original_model.to_string())
}

fn auto_resolve_provider_and_model<'a>(config: &Config, model: &'a str) -> (String, &'a str) {
    let model_lower = model.to_lowercase();
    let has_openrouter_key = config.is_provider_available("openrouter");
    let has_nvidia_key = config.is_provider_available("nvidia");

    for custom_name in config.custom_provider_names() {
        let custom_prefix = format!("{}/", custom_name.to_lowercase());
        match model_lower.starts_with(&custom_prefix) {
            true => return (custom_name.clone(), &model[custom_name.len() + 1..]),
            false => {}
        }
    }

    match model_lower == "mivi" {
        true => return ("mivi".to_string(), model),
        false => {}
    }
    match model_lower.ends_with(":free")
        && has_openrouter_key
        && !(model_lower.starts_with("nvidia/") && has_nvidia_key)
    {
        true => return ("openrouter".to_string(), model),
        false => {}
    }
    match provider_prefix_for_model(model) {
        Some((descriptor, prefix)) => {
            return (descriptor.canonical_name.to_string(), &model[prefix.len()..]);
        }
        None => {}
    }

    let candidates = keyword_provider_candidates(model);
    let selected = candidates
        .iter()
        .find(|candidate| config.is_provider_available(candidate))
        .or_else(|| candidates.first())
        .copied()
        .unwrap_or("openai");
    (selected.to_string(), model)
}

fn format_fallback_model(original_provider: &str, fb_model: String) -> String {
    match fb_model.contains('/') {
        true => fb_model,
        false => format!("{original_provider}/{fb_model}"),
    }
}

fn resolve_credentials_with_fallback(
    config: &Config,
    provider_name: &str,
    clean_model: &str,
) -> Result<(String, String, String, String)> {
    let (api_key, api_base) = resolve_api_config(config, provider_name);
    let needs_fallback = api_key.is_empty() && !provider_allows_empty_key(config, provider_name);
    match needs_fallback {
        false => Ok((
            provider_name.to_string(),
            api_key,
            api_base,
            clean_model.to_string(),
        )),
        true => {
            let fallback_target = ["openrouter", "opencode_zen"]
                .into_iter()
                .find(|target| config.is_provider_available(target));
            match fallback_target {
                Some(target) => {
                    let (fb_key, fb_base) = resolve_api_config(config, target);
                    let fb_model = resolve_fallback_model(target, clean_model);
                    let final_model = format_fallback_model(provider_name, fb_model);
                    Ok((target.to_string(), fb_key, fb_base, final_model))
                }
                None => {
                    let env_var = provider_api_key_env_var(provider_name);
                    Err(anyhow!(
                        "No API key found for provider '{}'. Set {} or run `openz configure` to add the provider key. No fallback key was available for OPENROUTER_API_KEY or OPENCODE_ZEN_API_KEY.",
                        provider_name,
                        env_var
                    ))
                }
            }
        }
    }
}

fn normalize_provider_model_name(
    config: &Config,
    final_provider_name: &str,
    final_model: &str,
) -> String {
    let mut clean_model_str = final_model.to_string();
    match provider_prefix_for_model(&clean_model_str) {
        Some((_, prefix)) => {
            let skip = final_provider_name == "openrouter" && prefix != "openrouter/";
            match !skip {
                true => clean_model_str = clean_model_str[prefix.len()..].to_string(),
                false => {}
            }
        }
        None => {}
    }

    match config.is_custom_provider(final_provider_name) {
        true => {
            let custom_prefix = format!("{}/", final_provider_name.to_lowercase());
            match clean_model_str.to_lowercase().starts_with(&custom_prefix) {
                true => {
                    clean_model_str =
                        clean_model_str[final_provider_name.len() + 1..].to_string();
                }
                false => {}
            }
        }
        false => {}
    }

    match final_provider_name {
        "nvidia" => {
            match clean_model_str.strip_suffix(":free") {
                Some(stripped) => clean_model_str = stripped.to_string(),
                None => {}
            }
            match clean_model_str.contains('/') {
                true => {}
                false => clean_model_str = format!("nvidia/{clean_model_str}"),
            }
        }
        "google_ai_studio" | "google ai studio" => {
            match clean_model_str
                .strip_prefix("google/")
                .or_else(|| clean_model_str.strip_prefix("models/"))
            {
                Some(stripped) => clean_model_str = stripped.to_string(),
                None => {}
            }
        }
        _ => {}
    }

    clean_model_str
}

fn sync_ollama_lifecycle(config: &Config, final_provider_name: &str, clean_model_str: &str) {
    let is_ollama = matches!(final_provider_name, "ollama" | "ollama_local");
    match is_ollama {
        true => super::ollama_manager::ensure_local_ollama(config),
        false => {}
    }

    match super::ollama_manager::get_active_ollama_model() {
        Some(old_mdl) => {
            let is_still_same = is_ollama && clean_model_str == old_mdl;
            match is_still_same {
                true => {}
                false => {
                    let config_clone = config.clone();
                    match tokio::runtime::Handle::try_current() {
                        Ok(handle) => {
                            handle.spawn(async move {
                                super::ollama_manager::unload_ollama_model(&config_clone, &old_mdl)
                                    .await;
                            });
                        }
                        Err(_) => {}
                    }
                    match is_ollama {
                        true => {}
                        false => super::ollama_manager::stop_local_ollama(),
                    }
                }
            }
        }
        None => {}
    }

    let next_active = is_ollama.then(|| clean_model_str.to_string());
    super::ollama_manager::set_active_ollama_model(next_active);
}

/// Full provider resolution pipeline: prefix detection, auto-detection, key resolution,
/// fallback key resolution, model name cleanup, and provider construction.
pub fn resolve_provider_full(config: &Config, model: &str) -> Result<ResolvedProvider> {
    let defaults = &config.agents.defaults;
    let (provider_name, clean_model) = match defaults.provider == "auto" {
        true => auto_resolve_provider_and_model(config, model),
        false => (defaults.provider.clone(), model),
    };

    let (final_provider_name, final_api_key, final_api_base, final_model) =
        resolve_credentials_with_fallback(config, &provider_name, clean_model)?;

    let clean_model_str =
        normalize_provider_model_name(config, &final_provider_name, &final_model);

    let instance: Arc<dyn LLMProvider> = match final_provider_name.as_str() {
        "anthropic" => Arc::new(AnthropicProvider::new(
            final_api_key.clone(),
            final_api_base.clone(),
            clean_model_str.clone(),
        )),
        _ => Arc::new(OpenAIProvider::new(
            final_api_key.clone(),
            final_api_base.clone(),
            clean_model_str.clone(),
        )),
    };

    sync_ollama_lifecycle(config, &final_provider_name, &clean_model_str);

    Ok(ResolvedProvider {
        provider_name: final_provider_name,
        api_key: final_api_key,
        api_base: final_api_base,
        model: clean_model_str,
        instance,
    })
}

#[cfg(test)]
#[path = "resolver_tests.rs"]
mod tests;
