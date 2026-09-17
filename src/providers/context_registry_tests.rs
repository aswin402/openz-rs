#[cfg(test)]
use super::context_registry::*;
use crate::config::schema::Config;
use crate::session::Message;

#[test]
fn test_resolve_context_window_builtin_models() {
    let config = Config::default();

    // MiniMax M2.7
    assert_eq!(
        DynamicContextRegistry::resolve_context_window("MiniMax-M2.7", &config),
        204_800
    );
    assert_eq!(
        DynamicContextRegistry::resolve_context_window("minimax/abab6.5s-chat", &config),
        204_800
    );

    // Claude 3.5 Sonnet
    assert_eq!(
        DynamicContextRegistry::resolve_context_window("claude-3-5-sonnet-20241022", &config),
        200_000
    );

    // Gemini
    assert_eq!(
        DynamicContextRegistry::resolve_context_window("gemini-1.5-pro-latest", &config),
        2_097_152
    );
    assert_eq!(
        DynamicContextRegistry::resolve_context_window("gemini-2.0-flash", &config),
        1_048_576
    );

    // OpenAI & Open-weights
    assert_eq!(
        DynamicContextRegistry::resolve_context_window("gpt-4o", &config),
        128_000
    );
    assert_eq!(
        DynamicContextRegistry::resolve_context_window("o1-preview", &config),
        200_000
    );
    assert_eq!(
        DynamicContextRegistry::resolve_context_window("deepseek-v4", &config),
        1_000_000
    );
    assert_eq!(
        DynamicContextRegistry::resolve_context_window("llama-3-8b", &config),
        8_192
    );
    assert_eq!(
        DynamicContextRegistry::resolve_context_window("llama-3.1-70b", &config),
        128_000
    );

    // Unknown model fallback
    assert_eq!(
        DynamicContextRegistry::resolve_context_window("some-unknown-model-xyz", &config),
        128_000
    );
}

#[test]
fn test_resolve_context_window_explicit_config_override() {
    let mut config = Config::default();
    config.agents.defaults.context_limit = Some(524_288);

    // Even on a model that defaults to 204.8k, explicit config wins (Tier 1)
    assert_eq!(
        DynamicContextRegistry::resolve_context_window("MiniMax-M2.7", &config),
        524_288
    );
}

#[test]
fn test_resolve_prompt_budget_chars_no_64k_clamp() {
    let config = Config::default();

    // MiniMax: 204,800 tokens * 0.25 * 4.0 chars/token = 204,800 chars
    let budget = DynamicContextRegistry::resolve_prompt_budget_chars("MiniMax-M2.7", &config);
    assert_eq!(budget, 204_800);
    // Crucially: It is NOT clamped to 64,000!
    assert!(budget > 64_000);

    // Gemini 1.5 Pro: 2,097,152 * 0.25 * 4.0 = 2,097,152 chars
    let gemini_budget =
        DynamicContextRegistry::resolve_prompt_budget_chars("gemini-1.5-pro", &config);
    assert_eq!(gemini_budget, 2_097_152);

    // Small model (e.g. 8k context): 8,192 * 0.25 * 4.0 = 8,192 chars >= 8,000 floor
    let small_budget =
        DynamicContextRegistry::resolve_prompt_budget_chars("llama-3-8b", &config);
    assert_eq!(small_budget, 8_192);
}

#[test]
fn test_resolve_prompt_budget_explicit_limit_override() {
    let mut config = Config::default();
    config.agents.defaults.prompt_budget_limit = Some(99_999);

    assert_eq!(
        DynamicContextRegistry::resolve_prompt_budget_chars("MiniMax-M2.7", &config),
        99_999
    );
}

#[test]
fn test_resolve_tool_output_limit_chars() {
    let config = Config::default();

    // MiniMax: 204,800 tokens * 0.08 * 4.0 chars/token = 65,536 chars
    let limit = DynamicContextRegistry::resolve_tool_output_limit_chars("MiniMax-M2.7", &config);
    assert_eq!(limit, 65_536);
    // Crucially: It is NOT clamped to 4,000!
    assert!(limit > 4_000);

    // Explicit override test
    let mut custom_config = Config::default();
    custom_config.agents.defaults.tool_output_limit = Some(15_000);
    assert_eq!(
        DynamicContextRegistry::resolve_tool_output_limit_chars("MiniMax-M2.7", &custom_config),
        15_000
    );
}

#[test]
fn test_should_compact_context_token_pressure() {
    let config = Config::default();

    // MiniMax 204.8k window: 80% compaction threshold is ~163,840 tokens.
    // At 16,000 tokens (the old buggy clamp threshold): should NOT compact!
    assert!(!DynamicContextRegistry::should_compact_context(
        "MiniMax-M2.7",
        16_000,
        &config
    ));

    // At 100,000 tokens: still should NOT compact!
    assert!(!DynamicContextRegistry::should_compact_context(
        "MiniMax-M2.7",
        100_000,
        &config
    ));

    // At 165,000 tokens (> 80%): triggers compaction!
    assert!(DynamicContextRegistry::should_compact_context(
        "MiniMax-M2.7",
        165_000,
        &config
    ));
}

#[test]
fn test_estimate_total_tokens() {
    let sys_prompt = "You are OpenZ, a high performance AI assistant.";
    let msgs = vec![
        Message {
            role: "user".to_string(),
            content: "Hello OpenZ! Please help me write code.".to_string(),
            timestamp: None,
            extra: serde_json::Map::new(),
        },
        Message {
            role: "assistant".to_string(),
            content: "Certainly! What would you like to build?".to_string(),
            timestamp: None,
            extra: serde_json::Map::new(),
        },
    ];

    let estimated = DynamicContextRegistry::estimate_total_tokens(&msgs, sys_prompt);
    assert!(estimated > 0);
    // Total chars: sys (~47) + user (~38) + assistant (~39) = ~124 chars -> ~31 tokens
    assert!(estimated >= 25 && estimated <= 40);
}
