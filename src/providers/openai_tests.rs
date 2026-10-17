use super::*;

#[test]
fn split_think_blocks_preserves_markdown_without_think_tags() {
    let markdown = "## Title\n\n- one\n- two\n\nFinal paragraph.";
    let (content, reasoning) = split_think_blocks(markdown);
    assert_eq!(content.as_deref(), Some(markdown));
    assert_eq!(reasoning, None);
}

#[test]
fn split_think_blocks_strips_single_block_from_visible_content() {
    let (content, reasoning) =
        split_think_blocks("<think>private reasoning</think>\n\nfinal answer");
    assert_eq!(content.as_deref(), Some("final answer"));
    assert_eq!(reasoning.as_deref(), Some("private reasoning"));
}

#[test]
fn split_think_blocks_strips_multiple_blocks_and_preserves_answer() {
    let (content, reasoning) =
        split_think_blocks("before <think>one</think> middle <think>two</think> after");
    assert_eq!(content.as_deref(), Some("before middle after"));
    assert_eq!(reasoning.as_deref(), Some("one\n\n---\n\ntwo"));
}

#[test]
fn split_think_blocks_handles_think_only_content() {
    let (content, reasoning) = split_think_blocks("<think>private only</think>");
    assert_eq!(content, None);
    assert_eq!(reasoning.as_deref(), Some("private only"));
}

#[tokio::test]
async fn test_serialize_messages_vision_fallback() {
    let system_prompt = "system instructions";

    let temp_dir = std::env::temp_dir();
    let test_img_path = temp_dir.join(format!("test_img_{}.png", uuid::Uuid::new_v4()));
    let mut png_bytes = b"\x89PNG\r\n\x1a\n".to_vec();
    png_bytes.extend(vec![0u8; 92]);
    std::fs::write(&test_img_path, png_bytes).unwrap();

    let image_tag = format!("![](file://{})", test_img_path.to_string_lossy());
    let msg_content = format!("{} check this image", image_tag);

    let messages = vec![Message {
        role: "user".to_string(),
        content: msg_content.clone(),
        timestamp: None,
        extra: serde_json::Map::new(),
    }];

    // Test non-vision model (deepseek-v4-flash-free)
    let serialized_non_vision = OpenAIProvider::serialize_messages(
        "deepseek-v4-flash-free",
        "https://api.openai.com/v1",
        system_prompt,
        &messages,
    )
    .await;

    assert_eq!(serialized_non_vision.len(), 2);
    assert_eq!(serialized_non_vision[0].role, "system");
    assert_eq!(serialized_non_vision[1].role, "user");

    // It must be serialized as String, keeping the original content (including the tag)
    let content_str = serialized_non_vision[1].content.as_str().unwrap();
    assert_eq!(content_str, msg_content);

    // Test vision model (gpt-4o)
    let serialized_vision = OpenAIProvider::serialize_messages(
        "gpt-4o",
        "https://api.openai.com/v1",
        system_prompt,
        &messages,
    )
    .await;

    assert_eq!(serialized_vision.len(), 2);
    assert_eq!(serialized_vision[1].role, "user");

    // It must be serialized as Array containing text and image_url parts
    let content_array = serialized_vision[1].content.as_array().unwrap();
    assert_eq!(content_array.len(), 2);

    assert_eq!(content_array[0]["type"], "image_url");
    assert!(content_array[0]["image_url"]["url"]
        .as_str()
        .unwrap()
        .starts_with("data:image/png;base64,"));

    assert_eq!(content_array[1]["type"], "text");
    assert_eq!(content_array[1]["text"], " check this image");

    // Cleanup
    let _ = std::fs::remove_file(test_img_path);
}

#[test]
fn test_think_stream_filter_splits_chunks_cleanly() {
    let mut filter = ThinkStreamFilter::new();
    let chunks = vec![
        ChatStreamChunk::Content("<think>\nUser asks who I am.\n</think>\n\nI am OpenZ.".to_string()),
    ];
    let mut processed = Vec::new();
    for c in chunks {
        processed.extend(filter.process_chunk(c));
    }
    processed.extend(filter.flush_buffer());

    assert_eq!(processed.len(), 2);
    match &processed[0] {
        ChatStreamChunk::Reasoning(r) => assert_eq!(r, "\nUser asks who I am.\n"),
        other => panic!("Expected reasoning, got {:?}", other),
    }
    match &processed[1] {
        ChatStreamChunk::Content(c) => assert_eq!(c, "I am OpenZ."),
        other => panic!("Expected content, got {:?}", other),
    }
}

#[test]
fn test_think_stream_filter_handles_split_tags() {
    let mut filter = ThinkStreamFilter::new();
    let chunks = vec![
        ChatStreamChunk::Content("<th".to_string()),
        ChatStreamChunk::Content("ink>Inner reasoning</th".to_string()),
        ChatStreamChunk::Content("ink>\n\nVisible reply".to_string()),
    ];
    let mut processed = Vec::new();
    for c in chunks {
        processed.extend(filter.process_chunk(c));
    }
    processed.extend(filter.flush_buffer());

    assert_eq!(processed.len(), 2);
    match &processed[0] {
        ChatStreamChunk::Reasoning(r) => assert_eq!(r, "Inner reasoning"),
        other => panic!("Expected reasoning, got {:?}", other),
    }
    match &processed[1] {
        ChatStreamChunk::Content(c) => assert_eq!(c, "Visible reply"),
        other => panic!("Expected content, got {:?}", other),
    }
}

#[test]
fn test_think_stream_filter_non_think_content_passes_through() {
    let mut filter = ThinkStreamFilter::new();
    let chunks = vec![
        ChatStreamChunk::Content("Normal text with <3 heart and <b>html</b>".to_string()),
    ];
    let mut processed = Vec::new();
    for c in chunks {
        processed.extend(filter.process_chunk(c));
    }
    processed.extend(filter.flush_buffer());

    assert_eq!(processed.len(), 1);
    match &processed[0] {
        ChatStreamChunk::Content(c) => assert_eq!(c, "Normal text with <3 heart and <b>html</b>"),
        other => panic!("Expected content, got {:?}", other),
    }
}

#[test]
fn test_parse_fallback_tool_calls_supports_minimax_arrow_and_xml_tags() {
    let minimax_text =
        "[TOOL_CALL]\n{tool => \"list_jobs\", args => {}}\n[/TOOL_CALL]";
    let calls = parse_fallback_tool_calls(minimax_text);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "list_jobs");
    assert_eq!(calls[0].arguments, serde_json::json!({}));

    let minimax_with_params =
        "[TOOL_CALL]\n{tool => \"read_file\", args => {path => \"src/main.rs\"}}\n[/TOOL_CALL]";
    let calls2 = parse_fallback_tool_calls(minimax_with_params);
    assert_eq!(calls2.len(), 1);
    assert_eq!(calls2[0].name, "read_file");
    assert_eq!(calls2[0].arguments, serde_json::json!({"path": "src/main.rs"}));

    let xml_text = "<tool_call>\n{\"tool\": \"schedule_job\", \"input\": {\"name\": \"ping\", \"schedule\": \"10m\"}}\n</tool_call>";
    let calls3 = parse_fallback_tool_calls(xml_text);
    assert_eq!(calls3.len(), 1);
    assert_eq!(calls3[0].name, "schedule_job");
    assert_eq!(
        calls3[0].arguments,
        serde_json::json!({"name": "ping", "schedule": "10m"})
    );
}

#[test]
fn test_parse_sse_data_chunks_preserves_reasoning_content_and_tool_calls_in_same_frame() {
    let sse_json = r#"{
        "choices": [{
            "delta": {
                "reasoning_content": "Let me execute the command.",
                "content": "\n",
                "tool_calls": [
                    {
                        "index": 0,
                        "id": "call_rm_1",
                        "type": "function",
                        "function": {
                            "name": "exec_command",
                            "arguments": "{\"command\":\"rm -f /tmp/openz_test_delete.txt\"}"
                        }
                    },
                    {
                        "index": 1,
                        "id": "call_jobs_2",
                        "type": "function",
                        "function": {
                            "name": "list_jobs",
                            "arguments": ""
                        }
                    }
                ]
            },
            "finish_reason": "tool_calls"
        }]
    }"#;

    let (chunks, is_done) = parse_sse_data_chunks(sse_json);
    assert!(!is_done);
    assert_eq!(chunks.len(), 5);
    match &chunks[0] {
        ChatStreamChunk::Reasoning(r) => assert_eq!(r, "Let me execute the command."),
        _ => panic!("expected Reasoning chunk"),
    }
    match &chunks[1] {
        ChatStreamChunk::Content(c) => assert_eq!(c, "\n"),
        _ => panic!("expected Content chunk"),
    }
    match &chunks[2] {
        ChatStreamChunk::ToolCall {
            index: 0,
            id: Some(id),
            name: Some(name),
            arguments: Some(args),
        } => {
            assert_eq!(id, "call_rm_1");
            assert_eq!(name, "exec_command");
            assert!(args.contains("openz_test_delete.txt"));
        }
        _ => panic!("expected first ToolCall chunk"),
    }
    match &chunks[3] {
        ChatStreamChunk::ToolCall {
            index: 1,
            id: Some(id),
            name: Some(name),
            arguments: None,
        } => {
            assert_eq!(id, "call_jobs_2");
            assert_eq!(name, "list_jobs");
        }
        _ => panic!("expected second ToolCall chunk"),
    }
    match &chunks[4] {
        ChatStreamChunk::Done {
            finish_reason: Some(reason),
        } => assert_eq!(reason, "tool_calls"),
        _ => panic!("expected Done chunk"),
    }
}
