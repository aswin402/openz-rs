use super::*;

#[tokio::test]
async fn large_namespaced_tool_output_uses_runtime_dir_and_safe_filename() {
    let _lock = crate::tools::graph_memory::test_lock().lock().await;
    let temp_dir = std::env::temp_dir().join(format!(
        "openz_transcript_tool_outputs_{}",
        uuid::Uuid::new_v4()
    ));
    let config_dir = temp_dir.join("config");
    let home_dir = temp_dir.join("home");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::create_dir_all(&home_dir).unwrap();

    let old_home = std::env::var_os("HOME");
    std::env::set_var("HOME", &home_dir);

    let mut config = Config::default();
    config.agents.defaults.tool_output_limit = Some(10);
    let mut messages = Vec::new();

    crate::config::loader::CONFIG_DIR_OVERRIDE
        .scope(config_dir.clone(), async {
            append_tool_results(
                &mut messages,
                &config,
                vec![ToolTranscriptResult {
                    id: "call_1".to_string(),
                    name: "mcp/server.tool".to_string(),
                    result: serde_json::json!({
                        "payload": "this output is intentionally long enough to be stored"
                    }),
                }],
            )
            .await;
        })
        .await;

    if let Some(old_home) = old_home {
        std::env::set_var("HOME", old_home);
    } else {
        std::env::remove_var("HOME");
    }

    let output_dir = config_dir.join("tool_outputs");
    let entries = std::fs::read_dir(&output_dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 1, "expected one stored tool output file");
    assert!(
        entries[0].is_file(),
        "tool output path must be a file, not a nested path"
    );
    assert!(
        !entries[0]
            .file_name()
            .unwrap()
            .to_string_lossy()
            .contains('/')
    );
    let expected_ref = format!("file://{}", entries[0].to_string_lossy());
    assert!(messages[0].content.contains(&expected_ref));
    assert_eq!(
        messages[0].extra["tool_output"]["original_ref"],
        expected_ref
    );
    assert_eq!(messages[0].extra["tool_output"]["truncated"], true);
    assert_eq!(
        std::fs::read_to_string(&entries[0]).expect("saved full output"),
        serde_json::json!({
            "payload": "this output is intentionally long enough to be stored"
        })
        .to_string()
    );
    assert!(std::fs::read_dir(home_dir.join(".openz")).is_err());

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn retrieve_original_outputs_are_not_truncated_again() {
    let temp_dir = std::env::temp_dir().join(format!(
        "openz_transcript_retrieve_passthrough_{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let mut config = Config::default();
    config.agents.defaults.tool_output_limit = Some(10);
    let mut messages = Vec::new();
    let full_content = "x".repeat(128);

    crate::config::loader::CONFIG_DIR_OVERRIDE
        .scope(temp_dir.clone(), async {
            append_tool_results(
                &mut messages,
                &config,
                vec![ToolTranscriptResult {
                    id: "call_1".to_string(),
                    name: "retrieve_original".to_string(),
                    result: serde_json::json!({ "content": full_content }),
                }],
            )
            .await;
        })
        .await;

    assert!(!messages[0].content.contains("TRUNCATED"));
    assert!(messages[0].extra.get("tool_output").is_none());

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn appends_tool_calls_to_existing_assistant_message() {
    let mut messages = vec![Message {
        role: "assistant".to_string(),
        content: "thinking".to_string(),
        timestamp: None,
        extra: serde_json::Map::new(),
    }];

    append_assistant_tool_calls(
        &mut messages,
        vec![serde_json::json!({
            "id": "call_1",
            "type": "function",
            "function": { "name": "read_file", "arguments": "{\"path\":\"Cargo.toml\"}" }
        })],
        Some("reasoning"),
    );

    assert_eq!(messages.len(), 1);
    assert!(messages[0].extra.get("tool_calls").is_some());
    assert_eq!(messages[0].extra["reasoning_content"], "reasoning");
}
