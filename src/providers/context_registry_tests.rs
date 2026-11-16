#[cfg(test)]
use super::context_registry::*;
use crate::config::schema::Config;
use crate::session::Message;
use serde_json::json;
use std::path::PathBuf;

fn unique_test_dir(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "openz_ctx_reg_{}_{}_{}",
        tag,
        std::process::id(),
        nanos
    ));
    let _ = std::fs::create_dir_all(&dir);
    dir
}

#[test]
fn test_minimax_m3_1_flash_1m_vs_m2_7_204k_from_dynamic_catalogs() {
    let dir = unique_test_dir("minimax_1m");
    let config = Config::default();

    let models_dev_payload = json!({
        "minimax": {
            "models": {
                "MiniMax-M2.7": {
                    "limit": { "context": 204800, "output": 131072 },
                    "modalities": { "input": ["text"], "output": ["text"] },
                    "tool_call": true,
                    "reasoning": true
                },
                "MiniMax-M3.1-Flash": {
                    "limit": { "context": 1000000, "output": 524288 },
                    "modalities": { "input": ["text", "image", "video"], "output": ["text"] },
                    "tool_call": true,
                    "reasoning": true
                }
            }
        },
        "google": {
            "models": {
                "gemini-1.5-pro-002": {
                    "limit": { "context": 2097152, "output": 8192 },
                    "modalities": { "input": ["text", "image", "video"] },
                    "tool_call": true
                },
                "gemini-1.5-flash-002": {
                    "limit": { "context": 1048576, "output": 8192 },
                    "modalities": { "input": ["text", "image"] },
                    "tool_call": true
                }
            }
        }
    });

    let openrouter_payload = json!({
        "data": [
            {
                "id": "meta-llama/llama-3-8b-instruct",
                "context_length": 8192,
                "top_provider": { "max_completion_tokens": 4096 },
                "architecture": { "input_modalities": ["text"] },
                "supported_parameters": ["tools", "temperature"]
            },
            {
                "id": "meta-llama/llama-3.1-70b-instruct",
                "context_length": 131072,
                "top_provider": { "max_completion_tokens": 16384 },
                "architecture": { "input_modalities": ["text"] },
                "supported_parameters": ["tools", "temperature"]
            }
        ]
    });

    assert_eq!(
        DynamicContextRegistry::ingest_models_dev_json_at(&dir, &models_dev_payload),
        4
    );
    assert_eq!(
        DynamicContextRegistry::ingest_openrouter_models_json_at(&dir, &openrouter_payload),
        2
    );

    // MiniMax-M3.1-Flash resolves to 1,000,000 (1M) with vision + reasoning!
    let m3_flash = DynamicContextRegistry::resolve_model_spec_at(
        &dir,
        "MiniMax-M3.1-Flash",
        Some(&config),
    );
    assert_eq!(m3_flash.context_window, 1_000_000);
    assert_eq!(m3_flash.max_output_tokens, 524_288);
    assert!(m3_flash.supports_vision);
    assert!(m3_flash.supports_reasoning);

    // Generation prefix resolution: a future variant `minimax/MiniMax-M3.2-Turbo` inherits M3's 1M root, NOT M2.7's 204.8k!
    let m3_variant = DynamicContextRegistry::resolve_model_spec_at(
        &dir,
        "minimax/MiniMax-M3.2-Turbo",
        Some(&config),
    );
    assert_eq!(m3_variant.context_window, 1_000_000);

    // MiniMax-M2.7 stays accurately at 204,800
    let m27 =
        DynamicContextRegistry::resolve_model_spec_at(&dir, "MiniMax-M2.7", Some(&config));
    assert_eq!(m27.context_window, 204_800);

    // Gemini 1.5 Pro (2M) and Gemini 1.5 Flash (1M) stay cleanly separated
    assert_eq!(
        DynamicContextRegistry::resolve_model_spec_at(&dir, "gemini-1.5-pro-latest", Some(&config))
            .context_window,
        2_097_152
    );
    assert_eq!(
        DynamicContextRegistry::resolve_model_spec_at(&dir, "gemini-1.5-flash", Some(&config))
            .context_window,
        1_048_576
    );

    // Llama 3 (8k) vs Llama 3.1 (131k) never collapse across minor version boundaries
    assert_eq!(
        DynamicContextRegistry::resolve_model_spec_at(&dir, "llama-3-8b", Some(&config))
            .context_window,
        8_192
    );
    assert_eq!(
        DynamicContextRegistry::resolve_model_spec_at(&dir, "llama-3.1-70b", Some(&config))
            .context_window,
        131_072
    );
    assert_eq!(
        DynamicContextRegistry::resolve_model_spec_at(&dir, "llama-3.2-90b", Some(&config))
            .context_window,
        128_000
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_ollama_show_and_provider_endpoint_ingestion() {
    let dir = unique_test_dir("ollama_lmstudio");
    let config = Config::default();

    let ollama_show = json!({
        "parameters": "temperature 0.7\nnum_ctx 65536\nstop \"<|im_end|>\"",
        "model_info": {
            "qwen3.context_length": 262144
        },
        "capabilities": ["completion", "tools", "vision", "thinking"],
        "details": {
            "parameter_size": "30.5B"
        }
    });

    let spec = DynamicContextRegistry::ingest_ollama_show_json_at(
        &dir,
        "qwen3-coder:30b",
        &ollama_show,
    )
    .expect("ollama spec parsed");
    // Explicit Modelfile `num_ctx` wins over theoretical `model_info` ceiling
    assert_eq!(spec.context_window, 65_536);
    assert!(spec.supports_vision);
    assert!(spec.supports_tools);
    assert!(spec.supports_reasoning);
    assert_eq!(spec.parameter_billions, Some(31));

    // LM Studio /v1/models with `loaded_context_length`
    let lmstudio_models = json!({
        "data": [
            {
                "id": "lmstudio-community/deepseek-r1-distill-qwen-14b",
                "loaded_context_length": 24576,
                "max_context_length": 131072
            }
        ]
    });
    assert_eq!(
        DynamicContextRegistry::ingest_provider_models_json_at(
            &dir,
            "openai",
            &lmstudio_models
        ),
        1
    );
    let lms = DynamicContextRegistry::resolve_model_spec_at(
        &dir,
        "lmstudio-community/deepseek-r1-distill-qwen-14b",
        Some(&config),
    );
    assert_eq!(lms.context_window, 24_576);
    assert_eq!(lms.parameter_billions, Some(14));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_reactive_overflow_error_learning_and_tier_precedence() {
    let dir = unique_test_dir("overflow_learn");
    let mut config = Config::default();

    // Seed theoretical 128k catalog entry
    let payload = json!({
        "data": [{ "id": "local/qwen2.5-coder-32b", "context_length": 131072 }]
    });
    DynamicContextRegistry::ingest_provider_models_json_at(&dir, "openai", &payload);
    assert_eq!(
        DynamicContextRegistry::resolve_model_spec_at(
            &dir,
            "local/qwen2.5-coder-32b",
            Some(&config)
        )
        .context_window,
        131_072
    );

    // Simulate vLLM HTTP 400 error revealing `--max-model-len 32768`
    let vllm_err = "HTTP 400 Bad Request: This model's maximum context length is 32768 tokens. However, you requested 41200 tokens (37104 in the messages, 4096 in the completion).";
    let learned = DynamicContextRegistry::learn_context_limit_from_error_at(
        &dir,
        "local/qwen2.5-coder-32b",
        vllm_err,
    );
    assert_eq!(learned, Some(32_768));

    // Tier 2 (learned runtime limit) overrides Tier 4 (catalog)
    let resolved = DynamicContextRegistry::resolve_model_spec_at(
        &dir,
        "local/qwen2.5-coder-32b",
        Some(&config),
    );
    assert_eq!(resolved.context_window, 32_768);
    assert_eq!(resolved.source, "learned_runtime");

    // Anthropic error format: "prompt is too long: 214500 tokens > 200000 maximum"
    let anthropic_err =
        "invalid_request_error: prompt is too long: 214500 tokens > 200000 maximum";
    assert_eq!(
        DynamicContextRegistry::learn_context_limit_from_error_at(
            &dir,
            "claude-custom-proxy",
            anthropic_err
        ),
        Some(200_000)
    );

    // Tier 1 (explicit config_override) overrides everything
    config.agents.defaults.context_limit = Some(64_000);
    assert_eq!(
        DynamicContextRegistry::resolve_model_spec_at(
            &dir,
            "local/qwen2.5-coder-32b",
            Some(&config)
        )
        .context_window,
        64_000
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_slug_grammar_and_user_models_json_catalog() {
    let dir = unique_test_dir("slug_grammar");
    let config = Config::default();

    // Pure structural slug grammar (zero catalog entries)
    let spec_1m = DynamicContextRegistry::resolve_model_spec_at(
        &dir,
        "custom/brandnew-coder-1m-vl-thinking",
        Some(&config),
    );
    assert_eq!(spec_1m.context_window, 1_000_000);
    assert!(spec_1m.supports_vision);
    assert!(spec_1m.supports_reasoning);

    assert_eq!(parse_parameter_billions("mixtral-8x22b-64k"), Some(176));
    assert_eq!(parse_parameter_billions("qwen2.5-1.5b-instruct"), Some(2));
    assert_eq!(
        DynamicContextRegistry::resolve_model_spec_at(&dir, "mixtral-8x22b-64k", Some(&config))
            .context_window,
        65_536
    );

    // User `models.json` rich schema
    std::fs::write(
        dir.join("models.json"),
        serde_json::to_string(&json!({
            "models": {
                "corp/internal-llm": {
                    "context_window": 512000,
                    "max_output_tokens": 65536,
                    "supports_vision": true,
                    "supports_reasoning": true
                }
            }
        }))
        .unwrap(),
    )
    .unwrap();

    let corp = DynamicContextRegistry::resolve_model_spec_at(
        &dir,
        "corp/internal-llm",
        Some(&config),
    );
    assert_eq!(corp.context_window, 512_000);
    assert_eq!(corp.max_output_tokens, 65_536);
    assert!(corp.supports_vision);
    assert!(corp.supports_reasoning);
    assert_eq!(corp.source, "user_catalog");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_budgets_token_estimator_and_sliding_window_split() {
    let mut config = Config::default();
    config.agents.defaults.context_limit = Some(1_000_000);

    // 1M tokens * 0.25 * 4.0 = 1,000,000 chars (never clamped to 64k)
    assert_eq!(
        DynamicContextRegistry::resolve_prompt_budget_chars("MiniMax-M3.1-Flash", &config),
        1_000_000
    );
    // Default inline tool output capped at 65,536 chars unless explicitly configured
    assert_eq!(
        DynamicContextRegistry::resolve_tool_output_limit_chars("MiniMax-M3.1-Flash", &config),
        65_536
    );
    // 80% of 1M = 800,000 tokens
    assert!(!DynamicContextRegistry::should_compact_context(
        "MiniMax-M3.1-Flash",
        500_000,
        &config
    ));
    assert!(DynamicContextRegistry::should_compact_context(
        "MiniMax-M3.1-Flash",
        810_000,
        &config
    ));

    // BPE estimator accounts for prose, JSON punctuation, CJK, and extra tool_calls
    let mut assistant_tc = Message {
        role: "assistant".to_string(),
        content: String::new(),
        timestamp: None,
        extra: serde_json::Map::new(),
    };
    assistant_tc.extra.insert(
        "tool_calls".to_string(),
        json!([{ "id": "call_1", "function": { "name": "read_file", "arguments": "{\"path\":\"src/main.rs\"}" } }]),
    );
    let mut tool_resp = Message {
        role: "tool".to_string(),
        content: "fn main() { println!(\"hello\"); }".repeat(200),
        timestamp: None,
        extra: serde_json::Map::new(),
    };
    tool_resp
        .extra
        .insert("tool_call_id".to_string(), json!("call_1"));

    let msgs = vec![
        Message {
            role: "user".to_string(),
            content: "Read main.rs".to_string(),
            timestamp: None,
            extra: serde_json::Map::new(),
        },
        assistant_tc,
        tool_resp,
        Message {
            role: "assistant".to_string(),
            content: "Done reading.".to_string(),
            timestamp: None,
            extra: serde_json::Map::new(),
        },
        Message {
            role: "user".to_string(),
            content: "Now summarize it.".to_string(),
            timestamp: None,
            extra: serde_json::Map::new(),
        },
    ];

    let mut small_cfg = Config::default();
    small_cfg.agents.defaults.context_limit = Some(4_096);
    let split = DynamicContextRegistry::resolve_compaction_split_by_tokens(
        &msgs,
        "small-model",
        &small_cfg,
    );
    assert!(split > 0 && split < msgs.len());
    // Split MUST align on a `"user"` message so `call_1` and its tool output are never separated!
    assert_eq!(msgs[split].role, "user");
}
