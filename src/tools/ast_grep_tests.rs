use super::*;

#[test]
fn test_ast_grep_match_to_code_element_parses_symbol() {
    let item = json!({
        "text": "fn bridge_symbol(input: String) -> String { input }",
        "file": "src/example.rs",
        "range": { "start": { "line": 9 }, "end": { "line": 11 } },
        "metaVariables": { "single": { "NAME": { "text": "bridge_symbol" } } }
    });

    let element = ast_grep_match_to_code_element("fn $NAME($$$) { $$$ }", &item).unwrap();
    assert_eq!(element.element_type, "Function");
    assert_eq!(element.name, "bridge_symbol");
    assert_eq!(element.file_path, "src/example.rs");
    assert_eq!(element.start_line, 10);
    assert_eq!(element.end_line, 12);
    assert!(element.signature.contains("bridge_symbol"));
}

#[tokio::test]
async fn test_ast_grep_inserted_elements_are_queryable_by_code_graph() -> Result<()> {
    let _lock = crate::tools::graph_memory::test_lock().lock().await;
    let session_id = format!("ast_grep_bridge_{}", uuid::Uuid::new_v4());
    let item = json!({
        "text": "struct BridgeQueryable { value: String }",
        "file": "src/bridge_queryable.rs",
        "range": { "start": { "line": 2 }, "end": { "line": 4 } },
        "metaVariables": { "single": { "NAME": { "text": "BridgeQueryable" } } }
    });
    let element = ast_grep_match_to_code_element("struct $NAME { $$$ }", &item).unwrap();
    crate::tools::graph_memory::with_db(|conn| {
        insert_ast_grep_code_element(conn, &element, "*", &session_id, "*")
    })?;

    let tool = crate::tools::memory_extra::QueryCodeGraphTool;
    let results = tool
        .call(&json!({"query": "BridgeQueryable", "session_id": session_id}))
        .await?;
    let items = results
        .as_array()
        .expect("query_code_graph returns an array");
    assert!(items.iter().any(|item| item["name"] == "BridgeQueryable"));
    Ok(())
}

#[tokio::test]
async fn test_ast_grep_status() -> Result<()> {
    let tool = AstGrepTool;
    let args = json!({
        "pattern": "fn $X($$$)",
        "lang": "rust"
    });

    let res = tool.call(&args).await?;
    assert!(res.is_array());
    assert_eq!(tool.name(), "ast_grep");
    assert!(tool.description().contains("structural"));
    Ok(())
}

#[tokio::test]
async fn test_ast_grep_aliases() -> Result<()> {
    let tool = AstGrepTool;
    let args = json!({
        "query": "fn $X($$$)",
        "language": "rust",
        "dir": "src/tools"
    });

    let res = tool.call(&args).await?;
    assert!(res.is_array());
    Ok(())
}

