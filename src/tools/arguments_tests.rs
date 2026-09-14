use super::*;

#[test]
fn tool_argument_aliases_normalize_consistently() {
    let normalized = normalize_tool_args(&serde_json::json!({
        "CommandLine": "just check openz",
        "Query": "provider catalog",
        "UrlContent": "https://example.com",
        "OutputPath": "/tmp/output.png",
        "TargetFile": "src/main.rs",
        "sessionId": "session-1",
    }));

    assert_eq!(normalized["command"], "just check openz");
    assert_eq!(normalized["query"], "provider catalog");
    assert_eq!(normalized["url"], "https://example.com");
    assert_eq!(normalized["output_path"], "/tmp/output.png");
    assert_eq!(normalized["target_file"], "src/main.rs");
    assert_eq!(normalized["session_id"], "session-1");
}

#[test]
fn typed_argument_kinds_share_compatibility_aliases() {
    let args = serde_json::json!({
        "AbsolutePath": "/tmp/example.txt",
        "CommandLine": "cargo check",
        "Query": "openz",
        "UrlContent": "https://example.com",
        "OutputPath": "/tmp/output.png",
        "sessionId": "session-1",
        "chat_id": "123",
    });

    assert_eq!(argument_values(&args, ToolArgumentKind::Path), vec![
        "/tmp/example.txt".to_string()
    ]);
    assert_eq!(argument_values(&args, ToolArgumentKind::Command), vec![
        "cargo check".to_string()
    ]);
    assert_eq!(argument_values(&args, ToolArgumentKind::Query), vec![
        "openz".to_string()
    ]);
    assert_eq!(argument_values(&args, ToolArgumentKind::Url), vec![
        "https://example.com".to_string()
    ]);
    assert_eq!(argument_values(&args, ToolArgumentKind::Output), vec![
        "/tmp/output.png".to_string()
    ]);
    assert_eq!(argument_values(&args, ToolArgumentKind::Session), vec![
        "session-1".to_string()
    ]);
    assert_eq!(argument_values(&args, ToolArgumentKind::Target), vec![
        "123".to_string()
    ]);
}
