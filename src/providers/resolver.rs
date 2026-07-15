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

/// Resolve API key and base URL for a given provider name from config + env vars.
pub fn resolve_api_config(config: &Config, provider_name: &str) -> (String, String) {
    let (key, base) = config.resolve_provider_config(provider_name);
    if key.is_empty() && provider_name != "ollama" && provider_name != "ollama_local" {
        tracing::warn!(
            "No API key configured for provider '{}'. Requests will likely fail with 401.",
            provider_name
        );
    }
    (key, base)
}

fn provider_api_key_env_var(provider_name: &str) -> &'static str {
    match provider_name {
        "anthropic" => "ANTHROPIC_API_KEY",
        "openai" => "OPENAI_API_KEY",
        "openrouter" => "OPENROUTER_API_KEY",
        "deepseek" => "DEEPSEEK_API_KEY",
        "groq" => "GROQ_API_KEY",
        "minimax" => "MINIMAX_API_KEY",
        "mistral" => "MISTRAL_API_KEY",
        "z.ai" => "Z_AI_API_KEY",
        "nvidia" => "NVIDIA_API_KEY",
        "opencode_zen" => "OPENCODE_ZEN_API_KEY",
        "google_ai_studio" | "google ai studio" => "GOOGLE_AI_STUDIO_API_KEY",
        "cerebras" => "CEREBRAS_API_KEY",
        "cohere" => "COHERE_API_KEY",
        "llm7" => "LLM7_API_KEY",
        "sambanova" => "SAMBANOVA_API_KEY",
        "huggingface" => "HUGGINGFACE_API_KEY",
        _ => "PROVIDER_API_KEY",
    }
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
    let has_openrouter_key = config
        .providers
        .openrouter
        .as_ref()
        .and_then(|p| p.api_key.as_ref())
        .is_some()
        || std::env::var("OPENROUTER_API_KEY").is_ok();
    let has_nvidia_key = config
        .providers
        .nvidia
        .as_ref()
        .and_then(|p| p.api_key.as_ref())
        .is_some()
        || std::env::var("NVIDIA_API_KEY").is_ok();

    // 1. Explicit provider prefixes
    if model_lower.starts_with("openrouter/") {
        provider_name = "openrouter".to_string();
        clean_model = &model["openrouter/".len()..];
    } else if model_lower.ends_with(":free")
        && has_openrouter_key
        && !(model_lower.starts_with("nvidia/") && has_nvidia_key)
    {
        provider_name = "openrouter".to_string();
        clean_model = model;
    } else if model_lower.starts_with("ollama_local/") {
        provider_name = "ollama_local".to_string();
        clean_model = &model["ollama_local/".len()..];
    } else if model_lower.starts_with("ollama/") {
        provider_name = "ollama".to_string();
        clean_model = &model["ollama/".len()..];
    } else if model_lower.starts_with("anthropic/") {
        provider_name = "anthropic".to_string();
        clean_model = &model["anthropic/".len()..];
    } else if model_lower.starts_with("openai/") {
        provider_name = "openai".to_string();
        clean_model = &model["openai/".len()..];
    } else if model_lower.starts_with("deepseek/") {
        provider_name = "deepseek".to_string();
        clean_model = &model["deepseek/".len()..];
    } else if model_lower.starts_with("groq/") {
        provider_name = "groq".to_string();
        clean_model = &model["groq/".len()..];
    } else if model_lower.starts_with("google_ai_studio/")
        || model_lower.starts_with("google-ai-studio/")
    {
        provider_name = "google_ai_studio".to_string();
        let prefix_len = if model_lower.starts_with("google_ai_studio/") {
            "google_ai_studio/".len()
        } else {
            "google-ai-studio/".len()
        };
        clean_model = &model[prefix_len..];
    } else if model_lower.starts_with("opencode_zen/") || model_lower.starts_with("opencode-zen/") {
        provider_name = "opencode_zen".to_string();
        let prefix_len = if model_lower.starts_with("opencode_zen/") {
            "opencode_zen/".len()
        } else {
            "opencode-zen/".len()
        };
        clean_model = &model[prefix_len..];
    } else if model_lower.starts_with("z.ai/") || model_lower.starts_with("z_ai/") {
        provider_name = "z.ai".to_string();
        let prefix_len = if model_lower.starts_with("z.ai/") {
            "z.ai/".len()
        } else {
            "z_ai/".len()
        };
        clean_model = &model[prefix_len..];
    } else if model_lower.starts_with("nvidia/") {
        provider_name = "nvidia".to_string();
        clean_model = &model["nvidia/".len()..];
    } else if model_lower.starts_with("minimax/") {
        provider_name = "minimax".to_string();
        clean_model = &model["minimax/".len()..];
    } else if model_lower.starts_with("mistral/") {
        provider_name = "mistral".to_string();
        clean_model = &model["mistral/".len()..];
    } else if model_lower.starts_with("cerebras/") || model_lower.starts_with("cerebres/") {
        provider_name = "cerebras".to_string();
        let prefix_len = if model_lower.starts_with("cerebras/") {
            "cerebras/".len()
        } else {
            "cerebres/".len()
        };
        clean_model = &model[prefix_len..];
    } else if model_lower.starts_with("cohere/") {
        provider_name = "cohere".to_string();
        clean_model = &model["cohere/".len()..];
    } else if model_lower.starts_with("llm7/") {
        provider_name = "llm7".to_string();
        clean_model = &model["llm7/".len()..];
    } else if model_lower.starts_with("sambanova/") {
        provider_name = "sambanova".to_string();
        clean_model = &model["sambanova/".len()..];
    } else if model_lower.starts_with("huggingface/") {
        provider_name = "huggingface".to_string();
        clean_model = &model["huggingface/".len()..];
    } else if provider_name == "auto" {
        // 2. Auto-detect from keywords
        let has_key = |prov: &str| -> bool {
            match prov {
                "anthropic" => {
                    config
                        .providers
                        .anthropic
                        .as_ref()
                        .and_then(|p| p.api_key.as_ref())
                        .is_some()
                        || std::env::var("ANTHROPIC_API_KEY").is_ok()
                }
                "openai" => {
                    config
                        .providers
                        .openai
                        .as_ref()
                        .and_then(|p| p.api_key.as_ref())
                        .is_some()
                        || std::env::var("OPENAI_API_KEY").is_ok()
                }
                "deepseek" => {
                    config
                        .providers
                        .deepseek
                        .as_ref()
                        .and_then(|p| p.api_key.as_ref())
                        .is_some()
                        || std::env::var("DEEPSEEK_API_KEY").is_ok()
                }
                "groq" => {
                    config
                        .providers
                        .groq
                        .as_ref()
                        .and_then(|p| p.api_key.as_ref())
                        .is_some()
                        || std::env::var("GROQ_API_KEY").is_ok()
                }
                "openrouter" => {
                    config
                        .providers
                        .openrouter
                        .as_ref()
                        .and_then(|p| p.api_key.as_ref())
                        .is_some()
                        || std::env::var("OPENROUTER_API_KEY").is_ok()
                }
                "opencode_zen" => {
                    config
                        .providers
                        .opencode_zen
                        .as_ref()
                        .and_then(|p| p.api_key.as_ref())
                        .is_some()
                        || std::env::var("OPENCODE_ZEN_API_KEY").is_ok()
                }
                "google_ai_studio" => {
                    config
                        .providers
                        .google_ai_studio
                        .as_ref()
                        .and_then(|p| p.api_key.as_ref())
                        .is_some()
                        || std::env::var("GOOGLE_AI_STUDIO_API_KEY").is_ok()
                }
                "z.ai" => {
                    config
                        .providers
                        .z_ai
                        .as_ref()
                        .and_then(|p| p.api_key.as_ref())
                        .is_some()
                        || std::env::var("Z_AI_API_KEY").is_ok()
                }
                "nvidia" => {
                    config
                        .providers
                        .nvidia
                        .as_ref()
                        .and_then(|p| p.api_key.as_ref())
                        .is_some()
                        || std::env::var("NVIDIA_API_KEY").is_ok()
                }
                "minimax" => {
                    config
                        .providers
                        .minimax
                        .as_ref()
                        .and_then(|p| p.api_key.as_ref())
                        .is_some()
                        || std::env::var("MINIMAX_API_KEY").is_ok()
                }
                "mistral" => {
                    config
                        .providers
                        .mistral
                        .as_ref()
                        .and_then(|p| p.api_key.as_ref())
                        .is_some()
                        || std::env::var("MISTRAL_API_KEY").is_ok()
                }
                "cerebras" => {
                    config
                        .providers
                        .cerebras
                        .as_ref()
                        .and_then(|p| p.api_key.as_ref())
                        .is_some()
                        || std::env::var("CEREBRAS_API_KEY").is_ok()
                        || std::env::var("CEBRAS_API_KEY").is_ok()
                }
                "cohere" => {
                    config
                        .providers
                        .cohere
                        .as_ref()
                        .and_then(|p| p.api_key.as_ref())
                        .is_some()
                        || std::env::var("COHERE_API_KEY").is_ok()
                }
                "llm7" => {
                    config
                        .providers
                        .llm7
                        .as_ref()
                        .and_then(|p| p.api_key.as_ref())
                        .is_some()
                        || std::env::var("LLM7_API_KEY").is_ok()
                }
                "sambanova" => {
                    config
                        .providers
                        .sambanova
                        .as_ref()
                        .and_then(|p| p.api_key.as_ref())
                        .is_some()
                        || std::env::var("SAMBANOVA_API_KEY").is_ok()
                }
                "huggingface" => {
                    config
                        .providers
                        .huggingface
                        .as_ref()
                        .and_then(|p| p.api_key.as_ref())
                        .is_some()
                        || std::env::var("HUGGINGFACE_API_KEY").is_ok()
                }
                _ => false,
            }
        };

        if model_lower.contains("claude") {
            if has_key("anthropic") {
                provider_name = "anthropic".to_string();
            } else if has_key("opencode_zen") {
                provider_name = "opencode_zen".to_string();
            } else if has_key("openrouter") {
                provider_name = "openrouter".to_string();
            } else {
                provider_name = "anthropic".to_string();
            }
        } else if model_lower.contains("gpt") {
            if has_key("openai") {
                provider_name = "openai".to_string();
            } else if has_key("opencode_zen") {
                provider_name = "opencode_zen".to_string();
            } else if has_key("openrouter") {
                provider_name = "openrouter".to_string();
            } else {
                provider_name = "openai".to_string();
            }
        } else if model_lower.contains("deepseek") {
            if has_key("deepseek") {
                provider_name = "deepseek".to_string();
            } else if has_key("opencode_zen") {
                provider_name = "opencode_zen".to_string();
            } else if has_key("openrouter") {
                provider_name = "openrouter".to_string();
            } else {
                provider_name = "deepseek".to_string();
            }
        } else if model_lower.contains("gemini") {
            if has_key("google_ai_studio") {
                provider_name = "google_ai_studio".to_string();
            } else if has_key("opencode_zen") {
                provider_name = "opencode_zen".to_string();
            } else if has_key("openrouter") {
                provider_name = "openrouter".to_string();
            } else {
                provider_name = "google_ai_studio".to_string();
            }
        } else if model_lower.contains("gemma") {
            if has_key("google_ai_studio") {
                provider_name = "google_ai_studio".to_string();
            } else if has_key("openrouter") {
                provider_name = "openrouter".to_string();
            } else if has_key("opencode_zen") {
                provider_name = "opencode_zen".to_string();
            } else {
                provider_name = "google_ai_studio".to_string();
            }
        } else if model_lower.contains("mistral") || model_lower.contains("codestral") {
            if has_key("mistral") {
                provider_name = "mistral".to_string();
            } else if has_key("openrouter") {
                provider_name = "openrouter".to_string();
            } else if has_key("opencode_zen") {
                provider_name = "opencode_zen".to_string();
            } else {
                provider_name = "mistral".to_string();
            }
        } else if model_lower.contains("command-r") || model_lower.contains("command-r7") {
            if has_key("cohere") {
                provider_name = "cohere".to_string();
            } else if has_key("openrouter") {
                provider_name = "openrouter".to_string();
            } else {
                provider_name = "cohere".to_string();
            }
        } else if model_lower.contains("sambanova") {
            provider_name = "sambanova".to_string();
        } else if model_lower.ends_with("-hf") || model_lower.starts_with("meta-") {
            if has_key("huggingface") {
                provider_name = "huggingface".to_string();
            } else if has_key("openrouter") {
                provider_name = "openrouter".to_string();
            } else {
                provider_name = "huggingface".to_string();
            }
        } else if model_lower.contains("ollama_local") {
            provider_name = "ollama_local".to_string();
        } else if model_lower.contains("ollama") {
            provider_name = "ollama".to_string();
        } else {
            let mut found = false;
            for prov in &[
                "opencode_zen",
                "google_ai_studio",
                "anthropic",
                "openai",
                "deepseek",
                "openrouter",
                "groq",
                "mistral",
                "nvidia",
                "z.ai",
                "cohere",
                "llm7",
                "sambanova",
                "huggingface",
            ] {
                if has_key(prov) {
                    provider_name = prov.to_string();
                    found = true;
                    break;
                }
            }
            if !found {
                provider_name = "openai".to_string();
            }
        }
    }

    // 3. Resolve API key + base
    let (mut final_api_key, mut final_api_base) = resolve_api_config(config, &provider_name);

    // 4. Fallback: if no key found (and not ollama), try openrouter / opencode_zen
    let mut final_provider_name = provider_name.clone();
    let mut final_model = clean_model.to_string();

    if final_provider_name != "ollama"
        && final_provider_name != "ollama_local"
        && final_api_key.is_empty()
    {
        let has_openrouter = config
            .providers
            .openrouter
            .as_ref()
            .and_then(|p| p.api_key.as_ref())
            .is_some()
            || std::env::var("OPENROUTER_API_KEY").is_ok();
        let has_opencode_zen = config
            .providers
            .opencode_zen
            .as_ref()
            .and_then(|p| p.api_key.as_ref())
            .is_some()
            || std::env::var("OPENCODE_ZEN_API_KEY").is_ok();

        if has_openrouter {
            let p = config.providers.openrouter.as_ref();
            final_api_key = p
                .and_then(|x| x.api_key.clone())
                .or_else(|| std::env::var("OPENROUTER_API_KEY").ok())
                .unwrap_or_default();
            final_api_base = p
                .and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "https://openrouter.ai/api/v1".to_string());
            final_provider_name = "openrouter".to_string();
            let fb_model = resolve_fallback_model("openrouter", clean_model);
            final_model = if fb_model.contains('/') {
                fb_model
            } else {
                format!("{}/{}", provider_name, fb_model)
            };
        } else if has_opencode_zen {
            let p = config.providers.opencode_zen.as_ref();
            final_api_key = p
                .and_then(|x| x.api_key.clone())
                .or_else(|| std::env::var("OPENCODE_ZEN_API_KEY").ok())
                .unwrap_or_default();
            final_api_base = p
                .and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "https://opencode.ai/zen/v1".to_string());
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
mod tests {
    use super::*;
    use std::sync::OnceLock;

    /// Serialize env-modifying tests to prevent race conditions from parallel execution.
    fn env_lock() -> &'static std::sync::Mutex<()> {
        static ENV_LOCK: OnceLock<std::sync::Mutex<()>> = OnceLock::new();
        ENV_LOCK.get_or_init(|| std::sync::Mutex::new(()))
    }
    use crate::config::schema::{AgentDefaults, AgentsConfig, Config};

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
    fn test_openrouter_free_model_routes_to_openrouter_even_when_default_openai() {
        let _guard = env_lock().lock().unwrap();
        let cfg = config_with("openai");
        std::env::set_var("OPENAI_API_KEY", "k");
        std::env::set_var("OPENROUTER_API_KEY", "rk");
        let r = resolve_provider_full(&cfg, "google/gemma-4-31b-it:free").unwrap();
        assert_eq!(r.provider_name, "openrouter");
        assert_eq!(r.model, "google/gemma-4-31b-it:free");
        std::env::remove_var("OPENROUTER_API_KEY");
        std::env::remove_var("OPENAI_API_KEY");
    }

    #[test]
    fn test_openrouter_nvidia_free_preserves_provider_slug_when_nvidia_key_absent() {
        let _guard = env_lock().lock().unwrap();
        let cfg = config_with("openai");
        std::env::set_var("OPENAI_API_KEY", "k");
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
        std::env::remove_var("OPENAI_API_KEY");
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
}
