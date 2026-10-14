use super::*;

#[tokio::test]
async fn test_model_supports_vision() {
    // Local MIVI
    assert!(model_supports_vision("mivi"));
    assert!(model_supports_vision("mivi/mivi"));
    // OpenAI
    assert!(model_supports_vision("gpt-4o"));
    assert!(model_supports_vision("gpt-4o-mini"));
    assert!(model_supports_vision("gpt-4-turbo"));
    assert!(model_supports_vision("gpt-4-vision-preview"));
    assert!(model_supports_vision("o1"));
    assert!(!model_supports_vision("o1-mini"));
    assert!(!model_supports_vision("o1-preview"));
    assert!(model_supports_vision("o3-mini"));
    // Anthropic
    assert!(model_supports_vision("claude-3-5-sonnet"));
    assert!(model_supports_vision("claude-3-opus"));
    assert!(model_supports_vision("claude-4-sonnet"));
    // Google
    assert!(model_supports_vision("google/gemini-2.5-flash"));
    assert!(model_supports_vision("google_ai_studio/gemini-2.0-flash"));
    assert!(model_supports_vision("nvidia/google/gemma-4-31b-it"));
    assert!(model_supports_vision("google/gemma-4-26b-a4b-it:free"));
    assert!(model_supports_vision(
        "nvidia/nemotron-3-nano-omni-30b-a3b-reasoning:free"
    ));
    assert!(!model_supports_vision("nvidia/google/gemma-2-31b-it"));
    assert!(!model_supports_vision("google/gemma-2-27b-it"));
    // Meta Llama vision
    assert!(model_supports_vision("meta/llama-3.2-90b-vision"));
    assert!(model_supports_vision(
        "nvidia/meta/llama-3.2-90b-vision-instruct"
    ));
    // Mistral
    assert!(model_supports_vision("pixtral-12b"));
    assert!(model_supports_vision("pixtral-large-latest"));
    // Other vision models
    assert!(model_supports_vision("deepseek-vl"));
    assert!(model_supports_vision("qwen-vl-plus"));
    assert!(model_supports_vision("nvidia/nemotron-nano-12b-v2-vl:free"));
    assert!(model_supports_vision("qwen/qwen3-vl-32b-instruct"));
    assert!(model_supports_vision("qwen/qwen2.5-vl-72b-instruct"));
    assert!(model_supports_vision(
        "microsoft/phi-3-vision-128k-instruct"
    ));
    assert!(model_supports_vision("microsoft/phi-4-multimodal-instruct"));
    assert!(model_supports_vision(
        "nvidia/llama-3.1-nemotron-nano-vl-8b-v1"
    ));
    assert!(model_supports_vision(
        "nvidia/llama-3.2-nemoretriever-1b-vlm-embed-v1"
    ));
    assert!(model_supports_vision("baidu/ernie-4.5-vl-424b-a47b"));
    // Non-vision models
    assert!(!model_supports_vision("deepseek-chat"));
    assert!(!model_supports_vision("deepseek-v4-flash-free"));
    assert!(!model_supports_vision("gpt-3.5-turbo"));
    assert!(!model_supports_vision("llama-3.1-8b-instant"));
    assert!(!model_supports_vision("openrouter/free"));
    assert!(!model_supports_vision("my-supervision-model"));
}

#[tokio::test]
async fn test_parse_multimodal_blocks_non_image_and_sensitive_paths() {
    // Attempting to read a non-image file or sensitive path via markdown image syntax
    // must fall back to literal text rather than base64-encoding the file contents.
    let text = "Check this ![leak](/etc/passwd) and ![toml](Cargo.toml)";
    let parts = parse_multimodal_content(text).await;
    assert!(parts.iter().all(|p| matches!(p, ContentPart::Text(_))));
}

#[test]
fn test_sanitize_tool_messages_discards_orphans_and_populates_name() {
    let mut assistant = crate::session::Message {
        role: "assistant".to_string(),
        content: String::new(),
        timestamp: None,
        extra: serde_json::Map::new(),
    };
    assistant.extra.insert(
        "tool_calls".to_string(),
        serde_json::json!([{
            "id": "call_valid",
            "function": { "name": "read_file", "arguments": "{}" }
        }]),
    );

    let mut valid_tool = crate::session::Message {
        role: "tool".to_string(),
        content: "ok".to_string(),
        timestamp: None,
        extra: serde_json::Map::new(),
    };
    valid_tool
        .extra
        .insert("tool_call_id".to_string(), serde_json::json!("call_valid"));

    let mut orphan_tool = crate::session::Message {
        role: "tool".to_string(),
        content: "orphan".to_string(),
        timestamp: None,
        extra: serde_json::Map::new(),
    };
    orphan_tool
        .extra
        .insert("tool_call_id".to_string(), serde_json::json!("call_missing"));

    let sanitized = sanitize_tool_messages(&[assistant, valid_tool, orphan_tool]);
    assert_eq!(sanitized.len(), 2);
    assert_eq!(
        sanitized[1].extra.get("name").and_then(|v| v.as_str()),
        Some("read_file")
    );
}
