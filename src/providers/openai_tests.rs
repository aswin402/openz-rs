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
    let test_img_path = temp_dir.join("test_img.png");
    std::fs::write(&test_img_path, vec![0; 100]).unwrap();

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
