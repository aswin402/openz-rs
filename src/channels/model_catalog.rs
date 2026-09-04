//! Curated provider/model metadata used by model selection surfaces.
//!
//! Provider identity and routing metadata live in `config::provider_catalog`.
//! This module owns the user-facing curated model list and its stable display
//! order. Runtime model discovery may extend these lists, but must not replace
//! the curated fallback entries.

use crate::config::provider_catalog;

#[derive(Debug, Clone, Copy)]
pub struct ProviderModels {
    pub name: &'static str,
    pub display: &'static str,
    pub models: &'static [&'static str],
}

#[derive(Debug, Clone)]
pub struct ProviderModelsOption {
    pub name: String,
    pub display: String,
    pub models: Vec<String>,
    pub available: bool,
}

/// Stable curated provider/model ordering used by the CLI, TUI, and WebUI.
pub static PROVIDER_REGISTRY: &[ProviderModels] = &[
    ProviderModels {
        name: "mivi",
        display: "Mivi Local (custom)",
        models: &["mivi llm", "mivi-llm", "mivi"],
    },
    ProviderModels {
        name: "openai",
        display: "OpenAI",
        models: &[
            "gpt-4.5",
            "gpt-4o",
            "gpt-4o-mini",
            "o1",
            "o1-mini",
            "o3",
            "o3-mini",
            "o4-mini",
        ],
    },
    ProviderModels {
        name: "anthropic",
        display: "Anthropic",
        models: &[
            "claude-3-5-sonnet-20241022",
            "claude-3-5-sonnet",
            "claude-3-5-haiku-20241022",
            "claude-3-5-haiku",
            "claude-3-opus-20240229",
            "claude-3-opus",
        ],
    },
    ProviderModels {
        name: "openrouter",
        display: "OpenRouter",
        models: &[
            "google/gemini-2.5-pro",
            "google/gemini-2.5-flash",
            "anthropic/claude-3.5-sonnet",
            "meta-llama/llama-3.3-70b-instruct",
            "deepseek/deepseek-r1",
        ],
    },
    ProviderModels {
        name: "deepseek",
        display: "DeepSeek",
        models: &["deepseek-chat", "deepseek-reasoner"],
    },
    ProviderModels {
        name: "groq",
        display: "Groq",
        models: &[
            "deepseek-r1-distill-llama-70b",
            "llama-3.3-70b-versatile",
            "llama-3.1-8b-instant",
            "mixtral-8x7b-32768",
            "gemma2-9b-it",
        ],
    },
    ProviderModels {
        name: "ollama_local",
        display: "Ollama Local (Auto-Start)",
        models: &["llama3", "mistral", "phi3", "qwen2.5", "deepseek-r1"],
    },
    ProviderModels {
        name: "ollama",
        display: "Ollama",
        models: &["llama3", "mistral", "phi3", "qwen2.5", "deepseek-r1"],
    },
    ProviderModels {
        name: "minimax",
        display: "minimax.io",
        models: &[
            "MiniMax-M3",
            "MiniMax-M2.7",
            "MiniMax-M2.5",
            "MiniMax-M2.1",
            "MiniMax-M2",
            "MiniMax-M1",
        ],
    },
    ProviderModels {
        name: "mistral",
        display: "Mistral AI",
        models: &[
            "mistral-large-latest",
            "pixtral-large-latest",
            "mistral-moderation-latest",
            "codestral-latest",
            "mistral-small-latest",
            "ministral-8b-latest",
            "ministral-14b-latest",
        ],
    },
    ProviderModels {
        name: "z.ai",
        display: "z.ai (Zhipu GLM)",
        models: &[
            "glm-5.1",
            "glm-5",
            "glm-5v-turbo",
            "glm-4.7",
            "glm-4.7-flash",
            "glm-4-flash",
        ],
    },
    ProviderModels {
        name: "nvidia",
        display: "NVIDIA NIM",
        models: &[
            "meta/llama3-70b-instruct",
            "nvidia/llama-3.1-nemotron-70b-instruct",
            "meta/llama-3.1-70b-instruct",
            "mistralai/mixtral-8x22b-instruct-v0.1",
            "google/gemma-2-27b-it",
        ],
    },
    ProviderModels {
        name: "opencode_zen",
        display: "OpenCode Zen",
        models: &[
            "deepseek-v4-flash-free",
            "mimo-v2.5-free",
            "north-mini-code-free",
            "nemotron-3-ultra-free",
        ],
    },
    ProviderModels {
        name: "cerebras",
        display: "Cerebras",
        models: &["llama-3.3-70b", "llama3.1-8b", "llama3.1-70b"],
    },
    ProviderModels {
        name: "google_ai_studio",
        display: "Google AI Studio (Gemini)",
        models: &[
            "gemini-3.5-flash",
            "gemini-3.1-pro-preview",
            "gemini-3.1-flash-lite",
            "gemini-2.5-pro",
            "gemini-2.5-flash",
            "gemini-2.0-flash",
            "gemini-1.5-pro",
        ],
    },
    ProviderModels {
        name: "cohere",
        display: "Cohere",
        models: &[
            "command-a-plus-05-2026",
            "command-r7b-12-2024",
            "command-r7-12-2025",
            "command-r-plus-08-2024",
            "command-r-08-2024",
        ],
    },
    ProviderModels {
        name: "sambanova",
        display: "SambaNova",
        models: &[
            "DeepSeek-V3.2",
            "Meta-Llama-3.3-70B-Instruct",
            "Qwen2.5-72B-Instruct",
            "QwQ-32B",
            "gemma-4-31B-it",
        ],
    },
    ProviderModels {
        name: "huggingface",
        display: "Hugging Face Inference",
        models: &[
            "meta-llama/Llama-3.3-70B-Instruct",
            "Qwen/QwQ-32B",
            "deepseek-ai/DeepSeek-R1",
        ],
    },
    ProviderModels {
        name: "llm7",
        display: "LLM7",
        models: &["gpt-4o", "gpt-4o-mini", "claude-3-5-sonnet"],
    },
];

pub fn provider_model_catalog() -> &'static [ProviderModels] {
    PROVIDER_REGISTRY
}

pub fn provider_models_by_name(name: &str) -> Option<&'static ProviderModels> {
    provider_model_catalog()
        .iter()
        .find(|provider| provider.name == name)
}

pub fn configured_provider_models(
    config: &crate::config::schema::Config,
) -> Vec<&'static ProviderModels> {
    provider_model_catalog()
        .iter()
        .filter(|provider| config.is_provider_available(provider.name))
        .collect()
}

pub fn configured_provider_model_options(
    config: &crate::config::schema::Config,
) -> Vec<ProviderModelsOption> {
    provider_model_catalog_options(config, true)
}

pub fn provider_model_catalog_options(
    config: &crate::config::schema::Config,
    configured_only: bool,
) -> Vec<ProviderModelsOption> {
    let mut providers = provider_model_catalog()
        .iter()
        .filter_map(|provider| {
            let available = config.is_provider_available(provider.name);
            if configured_only && !available {
                return None;
            }
            Some(ProviderModelsOption {
                name: provider.name.to_string(),
                display: provider.display.to_string(),
                models: provider
                    .models
                    .iter()
                    .map(|model| model.to_string())
                    .collect(),
                available,
            })
        })
        .collect::<Vec<_>>();

    for name in config.custom_provider_names() {
        let available = config.is_provider_available(&name);
        if configured_only && !available {
            continue;
        }
        let default_model = config.custom_provider_default_model(&name);
        let models = default_model.clone().into_iter().collect::<Vec<_>>();
        let display_model = default_model.unwrap_or_else(|| "custom model".to_string());
        providers.push(ProviderModelsOption {
            name: name.clone(),
            display: format!("Custom: {} ({})", name, display_model),
            models,
            available,
        });
    }

    providers
}

pub fn provider_model_option_by_name(
    config: &crate::config::schema::Config,
    name: &str,
) -> Option<ProviderModelsOption> {
    provider_models_by_name(name)
        .map(|provider| ProviderModelsOption {
            name: provider.name.to_string(),
            display: provider.display.to_string(),
            models: provider
                .models
                .iter()
                .map(|model| model.to_string())
                .collect(),
            available: config.is_provider_available(provider.name),
        })
        .or_else(|| {
            if !config.is_custom_provider(name) {
                return None;
            }
            let default_model = config.custom_provider_default_model(name);
            Some(ProviderModelsOption {
                name: name.to_string(),
                display: format!(
                    "Custom: {} ({})",
                    name,
                    default_model.as_deref().unwrap_or("custom model")
                ),
                models: default_model.into_iter().collect(),
                available: config.is_provider_available(name),
            })
        })
}

/// Get curated fallback models for a provider.
pub fn curated_models_for(provider_name: &str) -> Vec<String> {
    let canonical_name = provider_catalog::find_provider(provider_name)
        .map(|descriptor| descriptor.canonical_name)
        .unwrap_or(provider_name);
    provider_models_by_name(canonical_name)
        .map(|provider| provider.models.iter().map(|model| model.to_string()).collect())
        .unwrap_or_else(|| vec!["default".to_string()])
}

pub fn preview_models_for_provider(
    provider: &ProviderModelsOption,
    config: &crate::config::schema::Config,
    limit: usize,
) -> Vec<String> {
    let mut models = provider.models.clone();
    if let Some(default_model) = config
        .get_provider_config(&provider.name)
        .and_then(|provider| provider.default_model.clone())
        .or_else(|| config.custom_provider_default_model(&provider.name))
        .filter(|model| !model.trim().is_empty())
    {
        if !models
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(&default_model))
        {
            models.insert(0, default_model);
        }
    }
    models.truncate(limit);
    models
}

pub async fn resolved_provider_models_for_webui(
    provider: &ProviderModelsOption,
    config: &crate::config::schema::Config,
) -> Vec<String> {
    let mut models = fetch_provider_models(&provider.name, config)
        .await
        .unwrap_or_default();

    for model in &provider.models {
        if !models
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(model))
        {
            models.push(model.clone());
        }
    }

    if let Some(default_model) = config
        .get_provider_config(&provider.name)
        .and_then(|provider| provider.default_model.clone())
        .or_else(|| config.custom_provider_default_model(&provider.name))
        .filter(|model| !model.trim().is_empty())
    {
        if !models
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(&default_model))
        {
            models.push(default_model);
        }
    }

    models.sort();
    models
}

/// Fetch models from a provider's OpenAI-compatible or Ollama-style endpoint.
pub async fn fetch_provider_models(
    provider_name: &str,
    config: &crate::config::schema::Config,
) -> Option<Vec<String>> {
    static HTTP: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    let client = HTTP.get_or_init(|| {
        reqwest::Client::builder()
            .use_rustls_tls()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap_or_default()
    });

    let (api_key, api_base) = config.resolve_provider_config(provider_name);

    if provider_name != "ollama"
        && provider_name != "ollama_local"
        && api_key.is_empty()
        && !config.custom_provider_allows_empty_key(provider_name)
    {
        return None;
    }

    let url = if api_base.ends_with('/') {
        format!("{}models", api_base)
    } else {
        format!("{}/models", api_base)
    };

    let mut request = client.get(&url);
    if provider_name == "anthropic" {
        request = request
            .header("x-api-key", &api_key)
            .header("anthropic-version", "2023-06-01");
    } else if !api_key.is_empty() {
        request = request.bearer_auth(&api_key);
    }

    let response = request.send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }

    let json: serde_json::Value = response.json().await.ok()?;
    let mut models = Vec::new();

    if let Some(data) = json.get("data").and_then(|value| value.as_array()) {
        for model in data {
            if let Some(id) = model.get("id").and_then(|id| id.as_str()) {
                models.push(id.to_string());
            }
        }
    } else if let Some(data) = json.get("models").and_then(|value| value.as_array()) {
        for model in data {
            if let Some(name) = model.get("name").and_then(|name| name.as_str()) {
                models.push(name.strip_prefix("models/").unwrap_or(name).to_string());
            }
        }
    }

    if models.is_empty() {
        None
    } else {
        models.sort();
        Some(models)
    }
}
