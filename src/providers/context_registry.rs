//! Dynamic Model Context Registry & Proportional Token Budgeting Engine
//!
//! Provides non-hardcoded, model-aware context limits, proportional budgeting,
//! and token pressure calculations inspired by Hermes Agent and Pi Agent.

use crate::config::schema::Config;
use crate::session::Message;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;

pub const DEFAULT_PROMPT_BUDGET_RATIO: f64 = 0.25;
pub const DEFAULT_TOOL_OUTPUT_RATIO: f64 = 0.08;
pub const DEFAULT_COMPACTION_THRESHOLD_RATIO: f64 = 0.80;
pub const DEFAULT_KEEP_RECENT_RATIO: f64 = 0.20;
pub const FALLBACK_CONTEXT_WINDOW_TOKENS: usize = 128_000;
pub const CHARS_PER_TOKEN: f64 = 4.0;
pub const MIN_PROMPT_BUDGET_CHARS: usize = 8_000;
pub const MIN_TOOL_OUTPUT_CHARS: usize = 4_000;

const BUILTIN_CONTEXT_WINDOW_RULES: &[(&[&str], usize)] = &[
    (&["gemini-1.5-pro", "gemini-2.5-pro"], 2_097_152),
    (&["gemini"], 1_048_576),
    (&["minimax"], 204_800),
    (
        &["claude-3-5", "claude-3", "claude", "o1", "o3-mini", "o3"],
        200_000,
    ),
    (&["deepseek-v4"], 1_000_000),
    (
        &[
            "deepseek-v3",
            "deepseek-r1",
            "deepseek-chat",
            "deepseek-reasoner",
            "deepseek",
            "gpt-4.5",
            "gpt-4o",
            "gpt-4",
            "llama-3.1",
            "llama-3.2",
            "llama-3.3",
            "llama3.1",
            "llama3.2",
            "llama3.3",
            "qwen",
            "mistral-large",
        ],
        128_000,
    ),
    (&["mistral", "codestral"], 32_768),
    (&["gpt-3.5"], 16_384),
    (&["llama-3", "llama3"], 8_192),
];

#[derive(Debug, Clone, serde::Deserialize, Default)]
pub struct ModelsCatalogFile {
    #[serde(default)]
    pub models: HashMap<String, usize>,
    #[serde(default)]
    pub patterns: HashMap<String, usize>,
}

pub struct DynamicContextRegistry;

impl DynamicContextRegistry {
    /// Resolve the true context window (in tokens) for a given model.
    pub fn resolve_context_window(model: &str, config: &Config) -> usize {
        match config.agents.defaults.context_limit.filter(|&v| v > 0) {
            Some(explicit) => return explicit,
            None => {}
        }

        let normalized = model.trim().to_lowercase();

        let external_match = Self::load_external_catalog().and_then(|catalog| {
            let exact = catalog.models.get(&normalized).copied().filter(|&v| v > 0);
            exact.or_else(|| {
                catalog.patterns.iter().find_map(|(pattern, &limit)| {
                    let pat_norm = pattern.trim().to_lowercase();
                    (!pat_norm.is_empty() && normalized.contains(&pat_norm) && limit > 0)
                        .then_some(limit)
                })
            })
        });

        external_match
            .or_else(|| Self::lookup_builtin_model_window(&normalized))
            .unwrap_or(FALLBACK_CONTEXT_WINDOW_TOKENS)
    }

    /// Lookup standard context windows from built-in model patterns.
    fn lookup_builtin_model_window(model_lower: &str) -> Option<usize> {
        BUILTIN_CONTEXT_WINDOW_RULES
            .iter()
            .find_map(|(needles, window)| {
                needles
                    .iter()
                    .any(|needle| model_lower.contains(needle))
                    .then_some(*window)
            })
    }

    /// Resolve prompt budget (in characters) dynamically based on context window and ratio.
    pub fn resolve_prompt_budget_chars(model: &str, config: &Config) -> usize {
        match config.agents.defaults.prompt_budget_limit.filter(|&v| v > 0) {
            Some(explicit) => return explicit,
            None => {}
        }

        let window_tokens = Self::resolve_context_window(model, config);
        let ratio = config
            .agents
            .defaults
            .prompt_budget_ratio
            .unwrap_or(DEFAULT_PROMPT_BUDGET_RATIO)
            .clamp(0.05, 0.60);

        let budget = (window_tokens as f64 * ratio * CHARS_PER_TOKEN).round() as usize;
        budget.max(MIN_PROMPT_BUDGET_CHARS)
    }

    /// Resolve tool output limit (in characters) dynamically based on context window and ratio.
    pub fn resolve_tool_output_limit_chars(model: &str, config: &Config) -> usize {
        match config.agents.defaults.tool_output_limit.filter(|&v| v > 0) {
            Some(explicit) => return explicit,
            None => {}
        }

        let window_tokens = Self::resolve_context_window(model, config);
        let ratio = config
            .agents
            .defaults
            .tool_output_ratio
            .unwrap_or(DEFAULT_TOOL_OUTPUT_RATIO)
            .clamp(0.01, 0.30);

        let limit = (window_tokens as f64 * ratio * CHARS_PER_TOKEN).round() as usize;
        limit.max(MIN_TOOL_OUTPUT_CHARS)
    }

    /// Estimate tokens from character count using average 4 chars per token.
    pub fn estimate_tokens_from_chars(char_count: usize) -> usize {
        (char_count as f64 / CHARS_PER_TOKEN).ceil() as usize
    }

    /// Estimate total tokens across conversation messages and system prompt.
    pub fn estimate_total_tokens(messages: &[Message], system_prompt: &str) -> usize {
        let mut total_chars = system_prompt.chars().count();
        for msg in messages {
            total_chars += msg.content.chars().count();
            for (k, v) in &msg.extra {
                total_chars += k.chars().count();
                total_chars += match v.as_str() {
                    Some(s) => s.chars().count(),
                    None => v.to_string().chars().count(),
                };
            }
        }
        Self::estimate_tokens_from_chars(total_chars)
    }

    /// Determine if context has reached the compaction threshold.
    pub fn should_compact_context(
        model: &str,
        estimated_tokens: usize,
        config: &Config,
    ) -> bool {
        let window_tokens = Self::resolve_context_window(model, config);
        let ratio = config
            .agents
            .defaults
            .compaction_threshold_ratio
            .unwrap_or(DEFAULT_COMPACTION_THRESHOLD_RATIO)
            .clamp(0.30, 0.95);

        let threshold = (window_tokens as f64 * ratio).round() as usize;
        estimated_tokens >= threshold
    }

    /// Calculate the number of recent messages to preserve verbatim during compaction.
    pub fn resolve_keep_recent_count(total_msgs: usize, config: &Config) -> usize {
        let ratio = config
            .agents
            .defaults
            .keep_recent_ratio
            .unwrap_or(DEFAULT_KEEP_RECENT_RATIO)
            .clamp(0.05, 0.50);

        let count = (total_msgs as f64 * ratio).round() as usize;
        count.max(5)
    }

    /// Attempt to load the external `models.json` file from config dir if present.
    fn load_external_catalog() -> Option<ModelsCatalogFile> {
        static CACHE: OnceLock<Option<ModelsCatalogFile>> = OnceLock::new();
        CACHE
            .get_or_init(|| {
                let path = Self::models_catalog_path();
                path.is_file().then_some(())?;
                let content = std::fs::read_to_string(&path).ok()?;
                let structured = serde_json::from_str::<ModelsCatalogFile>(&content)
                    .ok()
                    .filter(|s| !s.models.is_empty() || !s.patterns.is_empty());
                structured.or_else(|| {
                    let flat = serde_json::from_str::<HashMap<String, usize>>(&content).ok()?;
                    let mut catalog = ModelsCatalogFile::default();
                    for (k, v) in flat {
                        catalog.models.insert(k.to_lowercase(), v);
                    }
                    Some(catalog)
                })
            })
            .clone()
    }

    fn models_catalog_path() -> PathBuf {
        crate::config::loader::config_dir().join("models.json")
    }
}
