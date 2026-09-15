use super::*;

#[test]
fn assembles_split_tool_call_arguments_in_index_order() {
    let mut assembly = StreamingAssembly::new();
    assembly.push_chunk(ChatStreamChunk::ToolCall {
        index: 1,
        id: Some("call_b".to_string()),
        name: Some("read_file".to_string()),
        arguments: Some("{\"path\":\"b".to_string()),
    });
    assembly.push_chunk(ChatStreamChunk::ToolCall {
        index: 1,
        id: None,
        name: None,
        arguments: Some(".rs\"}".to_string()),
    });
    assembly.push_chunk(ChatStreamChunk::ToolCall {
        index: 0,
        id: Some("call_a".to_string()),
        name: Some("list_dir".to_string()),
        arguments: Some("{\"path\":\".\"}".to_string()),
    });

    let response = assembly.into_response();
    assert_eq!(response.tool_calls.len(), 2);
    assert_eq!(response.tool_calls[0].id, "call_a");
    assert_eq!(response.tool_calls[1].id, "call_b");
    assert_eq!(response.tool_calls[1].arguments["path"], "b.rs");
}
