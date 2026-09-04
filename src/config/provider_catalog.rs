//! Canonical provider identity and model-routing metadata.
//!
//! Provider configuration storage still lives in schema.rs. This module owns
//! the metadata that must be shared by configuration, provider resolution, and
//! channel model discovery so those consumers do not maintain independent
//! alias, prefix, and environment-variable tables.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderBackend {
    OpenAiCompatible,
    Anthropic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderDescriptor {
    pub canonical_name: &'static str,
    pub aliases: &'static [&'static str],
    pub model_prefixes: &'static [&'static str],
    pub environment_keys: &'static [&'static str],
    pub default_api_base: &'static str,
    pub backend: ProviderBackend,
    pub local: bool,
}

const PROVIDER_DESCRIPTORS: &[ProviderDescriptor] = &[
    ProviderDescriptor {
        canonical_name: "anthropic",
        aliases: &["anthropic"],
        model_prefixes: &["anthropic/"],
        environment_keys: &["ANTHROPIC_API_KEY"],
        default_api_base: "https://api.anthropic.com",
        backend: ProviderBackend::Anthropic,
        local: false,
    },
    ProviderDescriptor {
        canonical_name: "openai",
        aliases: &["openai"],
        model_prefixes: &["openai/"],
        environment_keys: &["OPENAI_API_KEY"],
        default_api_base: "https://api.openai.com/v1",
        backend: ProviderBackend::OpenAiCompatible,
        local: false,
    },
    ProviderDescriptor {
        canonical_name: "mivi",
        aliases: &["mivi"],
        model_prefixes: &["mivi/"],
        environment_keys: &["MIVI_API_KEY"],
        default_api_base: "http://127.0.0.1:8000/v1",
        backend: ProviderBackend::OpenAiCompatible,
        local: true,
    },
    ProviderDescriptor {
        canonical_name: "openrouter",
        aliases: &["openrouter"],
        model_prefixes: &["openrouter/"],
        environment_keys: &["OPENROUTER_API_KEY"],
        default_api_base: "https://openrouter.ai/api/v1",
        backend: ProviderBackend::OpenAiCompatible,
        local: false,
    },
    ProviderDescriptor {
        canonical_name: "deepseek",
        aliases: &["deepseek"],
        model_prefixes: &["deepseek/"],
        environment_keys: &["DEEPSEEK_API_KEY"],
        default_api_base: "https://api.deepseek.com/v1",
        backend: ProviderBackend::OpenAiCompatible,
        local: false,
    },
    ProviderDescriptor {
        canonical_name: "groq",
        aliases: &["groq"],
        model_prefixes: &["groq/"],
        environment_keys: &["GROQ_API_KEY"],
        default_api_base: "https://api.groq.com/openai/v1",
        backend: ProviderBackend::OpenAiCompatible,
        local: false,
    },
    ProviderDescriptor {
        canonical_name: "ollama_local",
        aliases: &["ollama_local"],
        model_prefixes: &["ollama_local/"],
        environment_keys: &[],
        default_api_base: "http://localhost:11434/v1",
        backend: ProviderBackend::OpenAiCompatible,
        local: true,
    },
    ProviderDescriptor {
        canonical_name: "ollama",
        aliases: &["ollama"],
        model_prefixes: &["ollama/"],
        environment_keys: &[],
        default_api_base: "http://localhost:11434/v1",
        backend: ProviderBackend::OpenAiCompatible,
        local: true,
    },
    ProviderDescriptor {
        canonical_name: "minimax",
        aliases: &["minimax"],
        model_prefixes: &["minimax/"],
        environment_keys: &["MINIMAX_API_KEY"],
        default_api_base: "https://api.minimax.io/v1",
        backend: ProviderBackend::OpenAiCompatible,
        local: false,
    },
    ProviderDescriptor {
        canonical_name: "mistral",
        aliases: &["mistral"],
        model_prefixes: &["mistral/"],
        environment_keys: &["MISTRAL_API_KEY"],
        default_api_base: "https://api.mistral.ai/v1",
        backend: ProviderBackend::OpenAiCompatible,
        local: false,
    },
    ProviderDescriptor {
        canonical_name: "z.ai",
        aliases: &["z.ai", "z_ai"],
        model_prefixes: &["z.ai/", "z_ai/"],
        environment_keys: &["Z_AI_API_KEY"],
        default_api_base: "https://api.z.ai/api/paas/v4/",
        backend: ProviderBackend::OpenAiCompatible,
        local: false,
    },
    ProviderDescriptor {
        canonical_name: "nvidia",
        aliases: &["nvidia"],
        model_prefixes: &["nvidia/"],
        environment_keys: &["NVIDIA_API_KEY"],
        default_api_base: "https://integrate.api.nvidia.com/v1",
        backend: ProviderBackend::OpenAiCompatible,
        local: false,
    },
    ProviderDescriptor {
        canonical_name: "opencode_zen",
        aliases: &["opencode_zen", "opencode zen", "opencode-zen"],
        model_prefixes: &["opencode_zen/", "opencode-zen/"],
        environment_keys: &["OPENCODE_ZEN_API_KEY"],
        default_api_base: "https://opencode.ai/zen/v1",
        backend: ProviderBackend::OpenAiCompatible,
        local: false,
    },
    ProviderDescriptor {
        canonical_name: "cerebras",
        aliases: &["cerebras"],
        model_prefixes: &["cerebras/", "cerebres/"],
        environment_keys: &["CEREBRAS_API_KEY", "CEREBRES_API_KEY", "CEBRAS_API_KEY"],
        default_api_base: "https://api.cerebras.ai/v1",
        backend: ProviderBackend::OpenAiCompatible,
        local: false,
    },
    ProviderDescriptor {
        canonical_name: "google_ai_studio",
        aliases: &["google_ai_studio", "google ai studio", "google-ai-studio"],
        model_prefixes: &["google_ai_studio/", "google-ai-studio/"],
        environment_keys: &["GOOGLE_AI_STUDIO_API_KEY"],
        default_api_base: "https://generativelanguage.googleapis.com/v1beta/openai/",
        backend: ProviderBackend::OpenAiCompatible,
        local: false,
    },
    ProviderDescriptor {
        canonical_name: "cohere",
        aliases: &["cohere"],
        model_prefixes: &["cohere/"],
        environment_keys: &["COHERE_API_KEY"],
        default_api_base: "https://api.cohere.com/v1",
        backend: ProviderBackend::OpenAiCompatible,
        local: false,
    },
    ProviderDescriptor {
        canonical_name: "llm7",
        aliases: &["llm7"],
        model_prefixes: &["llm7/"],
        environment_keys: &["LLM7_API_KEY"],
        default_api_base: "https://token.llm7.io/v1",
        backend: ProviderBackend::OpenAiCompatible,
        local: false,
    },
    ProviderDescriptor {
        canonical_name: "sambanova",
        aliases: &["sambanova"],
        model_prefixes: &["sambanova/"],
        environment_keys: &["SAMBANOVA_API_KEY"],
        default_api_base: "https://api.sambanova.ai/v1",
        backend: ProviderBackend::OpenAiCompatible,
        local: false,
    },
    ProviderDescriptor {
        canonical_name: "huggingface",
        aliases: &["huggingface"],
        model_prefixes: &["huggingface/"],
        environment_keys: &["HUGGINGFACE_API_KEY"],
        default_api_base: "https://api-inference.huggingface.co/v1",
        backend: ProviderBackend::OpenAiCompatible,
        local: false,
    },
];

const CLAUDE_CANDIDATES: &[&str] = &["anthropic", "opencode_zen", "openrouter"];
const GPT_CANDIDATES: &[&str] = &["openai", "opencode_zen", "openrouter"];
const DEEPSEEK_CANDIDATES: &[&str] = &["deepseek", "opencode_zen", "openrouter"];
const GEMINI_CANDIDATES: &[&str] = &["google_ai_studio", "opencode_zen", "openrouter"];
const GEMMA_CANDIDATES: &[&str] = &["google_ai_studio", "openrouter", "opencode_zen"];
const MISTRAL_CANDIDATES: &[&str] = &["mistral", "openrouter", "opencode_zen"];
const COHERE_CANDIDATES: &[&str] = &["cohere", "openrouter"];
const SAMBANOVA_CANDIDATES: &[&str] = &["sambanova"];
const HUGGINGFACE_CANDIDATES: &[&str] = &["huggingface", "openrouter"];
const OLLAMA_LOCAL_CANDIDATES: &[&str] = &["ollama_local"];
const OLLAMA_CANDIDATES: &[&str] = &["ollama"];
const DEFAULT_CANDIDATES: &[&str] = &[
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
];

pub fn provider_descriptors() -> &'static [ProviderDescriptor] {
    PROVIDER_DESCRIPTORS
}

pub fn find_provider(input: &str) -> Option<&'static ProviderDescriptor> {
    let normalized = input.trim().to_ascii_lowercase();
    PROVIDER_DESCRIPTORS.iter().find(|descriptor| {
        descriptor
            .aliases
            .iter()
            .any(|alias| alias.eq_ignore_ascii_case(&normalized))
    })
}

/// Resolve the built-in API base used by setup flows for a provider name or
/// alias. Unknown names retain the historical OpenAI-compatible fallback.
pub fn default_api_base_for_provider(input: &str) -> &'static str {
    find_provider(input)
        .map(|descriptor| descriptor.default_api_base)
        .unwrap_or("https://api.openai.com/v1")
}

pub fn provider_prefix_for_model(
    model: &str,
) -> Option<(&'static ProviderDescriptor, &'static str)> {
    let normalized = model.to_ascii_lowercase();
    PROVIDER_DESCRIPTORS.iter().find_map(|descriptor| {
        descriptor.model_prefixes.iter().find_map(|prefix| {
            normalized
                .starts_with(prefix)
                .then_some((descriptor, *prefix))
        })
    })
}

pub fn environment_keys_for_provider(input: &str) -> &'static [&'static str] {
    find_provider(input)
        .map(|descriptor| descriptor.environment_keys)
        .unwrap_or(&[])
}

pub fn keyword_provider_candidates(model: &str) -> &'static [&'static str] {
    let normalized = model.to_ascii_lowercase();
    if normalized.contains("claude") {
        CLAUDE_CANDIDATES
    } else if normalized.contains("gpt") {
        GPT_CANDIDATES
    } else if normalized.contains("deepseek") {
        DEEPSEEK_CANDIDATES
    } else if normalized.contains("gemini") {
        GEMINI_CANDIDATES
    } else if normalized.contains("gemma") {
        GEMMA_CANDIDATES
    } else if normalized.contains("mistral") || normalized.contains("codestral") {
        MISTRAL_CANDIDATES
    } else if normalized.contains("command-r") || normalized.contains("command-r7") {
        COHERE_CANDIDATES
    } else if normalized.contains("sambanova") {
        SAMBANOVA_CANDIDATES
    } else if normalized.ends_with("-hf") || normalized.starts_with("meta-") {
        HUGGINGFACE_CANDIDATES
    } else if normalized.contains("ollama_local") {
        OLLAMA_LOCAL_CANDIDATES
    } else if normalized.contains("ollama") {
        OLLAMA_CANDIDATES
    } else {
        DEFAULT_CANDIDATES
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_catalog_matches_resolver_aliases() {
        assert_eq!(find_provider("anthropic").unwrap().canonical_name, "anthropic");
        assert_eq!(find_provider("z_ai").unwrap().canonical_name, "z.ai");
        assert_eq!(
            find_provider("google-ai-studio").unwrap().canonical_name,
            "google_ai_studio"
        );
        assert_eq!(
            find_provider("opencode zen").unwrap().canonical_name,
            "opencode_zen"
        );

        let (provider, prefix) = provider_prefix_for_model("CEREBRES/foo").unwrap();
        assert_eq!(provider.canonical_name, "cerebras");
        assert_eq!(prefix, "cerebres/");

        assert_eq!(
            environment_keys_for_provider("z.ai"),
            &["Z_AI_API_KEY"]
        );
        assert_eq!(
            keyword_provider_candidates("claude-3-5-sonnet"),
            &["anthropic", "opencode_zen", "openrouter"]
        );
        assert!(provider_descriptors().iter().all(|descriptor| {
            descriptor
                .aliases
                .iter()
                .all(|alias| find_provider(alias).is_some())
        }));
    }
}
