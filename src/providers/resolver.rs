use crate::config::schema::Config;
use crate::config::provider_catalog::{
    environment_keys_for_provider, keyword_provider_candidates, provider_prefix_for_model,
};
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

/// Resolve API key and base URL for a given provider name from config + env vars.
pub fn resolve_api_config(config: &Config, provider_name: &str) -> (String, String) {
    let (key, base) = config.resolve_provider_config(provider_name);
    if key.is_empty()
        && provider_name != "ollama"
        && provider_name != "ollama_local"
        && provider_name != "mivi"
        && !config.custom_provider_allows_empty_key(provider_name)
    {
        tracing::warn!(
            "No API key configured for provider '{}'. Requests will likely fail with 401.",
            provider_name
        );
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
    match target_provider {
        "openrouter" => {
            if original_lower.contains("claude") {
                "google/gemini-2.0-flash-exp:free".to_string()
            } else if original_lower.contains("gpt") {
                "meta-llama/llama-3.3-70b-instruct:free".to_string()
            } else {
                "google/gemini-2.0-flash-exp:free".to_string()
            }
        }
        "opencode_zen" => {
            if original_lower.contains("claude") {
                "mimo-v2.5-free".to_string()
            } else if original_lower.contains("gpt") {
                "nemotron-3-ultra-free".to_string()
            } else {
                "deepseek-v4-flash-free".to_string()
            }
        }
        _ => original_model.to_string(),
    }
}

/// Full provider resolution pipeline: prefix detection, auto-detection, key resolution,
/// fallback key resolution, model name cleanup, and provider construction.
///
/// Single entry point for all provider routing in the system.
pub fn resolve_provider_full(config: &Config, model: &str) -> Result<ResolvedProvider> {
    let defaults = &config.agents.defaults;
    let mut provider_name = defaults.provider.clone();
    let mut clean_model = model;

    let model_lower = model.to_lowercase();
    let has_openrouter_key = config.is_provider_available("openrouter");
    let has_nvidia_key = config.is_provider_available("nvidia");
    let provider_is_auto = defaults.provider == "auto";

    // 1. Explicit model prefixes are routing hints only when provider selection is auto.
    let mut prefix_matched = false;
    if provider_is_auto {
        for custom_name in config.custom_provider_names() {
            let custom_prefix = format!("{}/", custom_name.to_lowercase());
            if model_lower.starts_with(&custom_prefix) {
                provider_name = custom_name.clone();
                clean_model = &model[custom_name.len() + 1..];
                prefix_matched = true;
                break;
            }
        }
    }
    if provider_is_auto && !prefix_matched {
        if model_lower == "mivi" {
            provider_name = "mivi".to_string();
            clean_model = model;
            prefix_matched = true;
        } else if model_lower.starts_with("mivi/") {
            provider_name = "mivi".to_string();
            clean_model = &model["mivi/".len()..];
            prefix_matched = true;
        }
    }
    if provider_is_auto && !prefix_matched && model_lower.starts_with("openrouter/") {
        provider_name = "openrouter".to_string();
        clean_model = &model["openrouter/".len()..];
        prefix_matched = true;
    }
    if provider_is_auto
        && !prefix_matched
        && model_lower.ends_with(":free")
        && has_openrouter_key
        && !(model_lower.starts_with("nvidia/") && has_nvidia_key)
    {
        provider_name = "openrouter".to_string();
        clean_model = model;
        prefix_matched = true;
    }
    if provider_is_auto && !prefix_matched {
        if let Some((descriptor, prefix)) = provider_prefix_for_model(model) {
            provider_name = descriptor.canonical_name.to_string();
            clean_model = &model[prefix.len()..];
            prefix_matched = true;
        }
    }

    // 2. Auto-detect from model keywords using the same fallback order as
    // before, now expressed by the provider catalog.
    if provider_is_auto && !prefix_matched {
        let candidates = keyword_provider_candidates(model);
        if let Some(provider) = candidates
            .iter()
            .find(|candidate| config.is_provider_available(candidate))
        {
            provider_name = (*provider).to_string();
        } else if let Some(provider) = candidates.first() {
            provider_name = (*provider).to_string();
        } else {
            provider_name = "openai".to_string();
        }
    }

    // 3. Resolve API key + base
    let (mut final_api_key, mut final_api_base) = resolve_api_config(config, &provider_name);

    // 4. Fallback: if no key found (and not ollama), try openrouter / opencode_zen
    let mut final_provider_name = provider_name.clone();
    let mut final_model = clean_model.to_string();

    if final_provider_name != "ollama"
        && final_provider_name != "ollama_local"
        && final_provider_name != "mivi"
        && !config.custom_provider_allows_empty_key(&final_provider_name)
        && final_api_key.is_empty()
    {
        let has_openrouter = config.is_provider_available("openrouter");
        let has_opencode_zen = config.is_provider_available("opencode_zen");

        if has_openrouter {
            (final_api_key, final_api_base) = resolve_api_config(config, "openrouter");
            final_provider_name = "openrouter".to_string();
            let fb_model = resolve_fallback_model("openrouter", clean_model);
            final_model = if fb_model.contains('/') {
                fb_model
            } else {
                format!("{}/{}", provider_name, fb_model)
            };
        } else if has_opencode_zen {
            (final_api_key, final_api_base) = resolve_api_config(config, "opencode_zen");
            final_provider_name = "opencode_zen".to_string();
            let fb_model = resolve_fallback_model("opencode_zen", clean_model);
            final_model = if fb_model.contains('/') {
                fb_model
            } else {
                format!("{}/{}", provider_name, fb_model)
            };
        } else {
            let env_var = provider_api_key_env_var(&final_provider_name);
            return Err(anyhow!(
                "No API key found for provider '{}'. Set {} or run `openz configure` to add the provider key. No fallback key was available for OPENROUTER_API_KEY or OPENCODE_ZEN_API_KEY.",
                final_provider_name,
                env_var
            ));
        }
    }

    // 5. Model name cleanup (strip remaining prefixes, normalize nvidia/google)
    let mut clean_model_str = final_model.clone();
    let clean_lower = clean_model_str.to_lowercase();
    let prefixes = [
        "openrouter/",
        "ollama_local/",
        "ollama/",
        "anthropic/",
        "openai/",
        "mivi/",
        "deepseek/",
        "groq/",
        "google_ai_studio/",
        "google-ai-studio/",
        "opencode_zen/",
        "opencode-zen/",
        "z.ai/",
        "z_ai/",
        "nvidia/",
        "minimax/",
        "mistral/",
        "cerebres/",
        "cerebras/",
        "cohere/",
        "llm7/",
        "sambanova/",
        "huggingface/",
    ];
    for prefix in &prefixes {
        if final_provider_name == "openrouter" && *prefix != "openrouter/" {
            continue;
        }
        if clean_lower.starts_with(prefix) {
            clean_model_str = clean_model_str[prefix.len()..].to_string();
            break;
        }
    }
    if config.is_custom_provider(&final_provider_name) {
        let custom_prefix = format!("{}/", final_provider_name.to_lowercase());
        if clean_model_str.to_lowercase().starts_with(&custom_prefix) {
            clean_model_str = clean_model_str[final_provider_name.len() + 1..].to_string();
        }
    }
    if final_provider_name == "nvidia" {
        if clean_model_str.ends_with(":free") {
            clean_model_str = clean_model_str[..clean_model_str.len() - 5].to_string();
        }
        if !clean_model_str.contains('/') {
            clean_model_str = format!("nvidia/{}", clean_model_str);
        }
    } else if final_provider_name == "google_ai_studio" || final_provider_name == "google ai studio"
    {
        if clean_model_str.starts_with("google/") {
            clean_model_str = clean_model_str["google/".len()..].to_string();
        } else if clean_model_str.starts_with("models/") {
            clean_model_str = clean_model_str["models/".len()..].to_string();
        }
    }

    // 6. Build provider
    let instance: Arc<dyn LLMProvider> = if final_provider_name == "anthropic" {
        Arc::new(AnthropicProvider::new(
            final_api_key.clone(),
            final_api_base.clone(),
            clean_model_str.clone(),
        ))
    } else {
        Arc::new(OpenAIProvider::new(
            final_api_key.clone(),
            final_api_base.clone(),
            clean_model_str.clone(),
        ))
    };
    // Handle local Ollama process and model management
    if final_provider_name == "ollama" || final_provider_name == "ollama_local" {
        super::ollama_manager::ensure_local_ollama(config);
    }

    let old_active = super::ollama_manager::get_active_ollama_model();
    if let Some(old_mdl) = old_active {
        let is_still_same = (final_provider_name == "ollama"
            || final_provider_name == "ollama_local")
            && clean_model_str == old_mdl;
        if !is_still_same {
            let config_clone = config.clone();
            let old_mdl_clone = old_mdl;
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                handle.spawn(async move {
                    super::ollama_manager::unload_ollama_model(&config_clone, &old_mdl_clone).await;
                });
            }

            if final_provider_name != "ollama" && final_provider_name != "ollama_local" {
                super::ollama_manager::stop_local_ollama();
            }
        }
    }

    if final_provider_name == "ollama" || final_provider_name == "ollama_local" {
        super::ollama_manager::set_active_ollama_model(Some(clean_model_str.clone()));
    } else {
        super::ollama_manager::set_active_ollama_model(None);
    }

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
