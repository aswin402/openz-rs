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
    ///
    /// Hierarchy:
    /// 1. User config explicit override (`config.agents.defaults.context_limit`)
    /// 2. External catalog file (`~/.openz/models.json`)
    /// 3. Built-in curated patterns (MiniMax, Claude, Gemini, GPT, DeepSeek, etc.)
    /// 4. Fallback default (128,000 tokens)
    pub fn resolve_context_window(model: &str, config: &Config) -> usize {
        // Tier 1: User explicit config override
        if let Some(explicit) = config.agents.defaults.context_limit {
            if explicit > 0 {
                return explicit;
            }
        }

        let normalized = model.trim().to_lowercase();

        // Tier 2: External models.json catalog
        if let Some(catalog) = Self::load_external_catalog() {
            // Check exact model match
            if let Some(&limit) = catalog.models.get(&normalized) {
                if limit > 0 {
                    return limit;
                }
            }
            // Check pattern match
            for (pattern, &limit) in &catalog.patterns {
                let pat_norm = pattern.trim().to_lowercase();
                if !pat_norm.is_empty() && normalized.contains(&pat_norm) && limit > 0 {
                    return limit;
                }
            }
        }

        // Tier 3: Built-in curated patterns
        if let Some(window) = Self::lookup_builtin_model_window(&normalized) {
            return window;
        }

        // Tier 4: Fallback
        FALLBACK_CONTEXT_WINDOW_TOKENS
    }

    /// Lookup standard context windows from built-in model patterns.
    fn lookup_builtin_model_window(model_lower: &str) -> Option<usize> {
        if model_lower.contains("gemini-1.5-pro") || model_lower.contains("gemini-2.5-pro") {
            Some(2_097_152)
        } else if model_lower.contains("gemini") {
            Some(1_048_576)
        } else if model_lower.contains("minimax") {
            Some(204_800)
        } else if model_lower.contains("claude-3-5")
            || model_lower.contains("claude-3")
            || model_lower.contains("claude")
            || model_lower.contains("o1")
            || model_lower.contains("o3-mini")
            || model_lower.contains("o3")
        {
            Some(200_000)
        } else if model_lower.contains("deepseek-v4") {
            Some(1_000_000)
        } else if model_lower.contains("deepseek-v3")
            || model_lower.contains("deepseek-r1")
            || model_lower.contains("deepseek-chat")
            || model_lower.contains("deepseek-reasoner")
            || model_lower.contains("deepseek")
            || model_lower.contains("gpt-4.5")
            || model_lower.contains("gpt-4o")
            || model_lower.contains("gpt-4")
            || model_lower.contains("llama-3.1")
            || model_lower.contains("llama-3.2")
            || model_lower.contains("llama-3.3")
            || model_lower.contains("llama3.1")
            || model_lower.contains("llama3.2")
            || model_lower.contains("llama3.3")
            || model_lower.contains("qwen")
            || model_lower.contains("mistral-large")
        {
            Some(128_000)
        } else if model_lower.contains("mistral") || model_lower.contains("codestral") {
            Some(32_768)
        } else if model_lower.contains("gpt-3.5") {
            Some(16_384)
        } else if model_lower.contains("llama-3") || model_lower.contains("llama3") {
            Some(8_192)
        } else {
            None
        }
    }

    /// Resolve prompt budget (in characters) dynamically based on context window and ratio.
    ///
    /// Formula: tokens * prompt_budget_ratio * 4.0 chars/token.
    /// Clamped to at least MIN_PROMPT_BUDGET_CHARS (8,000) with NO static 64k upper clamp.
    pub fn resolve_prompt_budget_chars(model: &str, config: &Config) -> usize {
        if let Some(explicit) = config.agents.defaults.prompt_budget_limit {
            if explicit > 0 {
                return explicit;
            }
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
    ///
    /// Formula: tokens * tool_output_ratio * 4.0 chars/token.
    /// Clamped to at least MIN_TOOL_OUTPUT_CHARS (4,000).
    pub fn resolve_tool_output_limit_chars(model: &str, config: &Config) -> usize {
        if let Some(explicit) = config.agents.defaults.tool_output_limit {
            if explicit > 0 {
                return explicit;
            }
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
            // Count tool call / extra metadata characters
            for (k, v) in &msg.extra {
                total_chars += k.chars().count();
                if let Some(s) = v.as_str() {
                    total_chars += s.chars().count();
                } else {
                    total_chars += v.to_string().chars().count();
                }
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
                if !path.is_file() {
                    return None;
                }
                let content = std::fs::read_to_string(&path).ok()?;
                // Try structured format first
                if let Ok(structured) = serde_json::from_str::<ModelsCatalogFile>(&content) {
                    if !structured.models.is_empty() || !structured.patterns.is_empty() {
                        return Some(structured);
                    }
                }
                // Try simple key-value format {"model_name": context_limit}
                if let Ok(flat) = serde_json::from_str::<HashMap<String, usize>>(&content) {
                    let mut catalog = ModelsCatalogFile::default();
                    for (k, v) in flat {
                        catalog.models.insert(k.to_lowercase(), v);
                    }
                    return Some(catalog);
                }
                None
            })
            .clone()
    }

    fn models_catalog_path() -> PathBuf {
        crate::config::loader::config_dir().join("models.json")
    }
}
