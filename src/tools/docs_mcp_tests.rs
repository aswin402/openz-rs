use super::*;
use crate::tools::Tool;

#[test]
fn test_docs_tools_metadata() {
    let list_tool = DocsListDocsetsTool;
    assert_eq!(list_tool.name(), "docs_list_docsets");
    assert!(!list_tool.description().is_empty());

    let install_tool = DocsInstallDocsetTool;
    assert_eq!(install_tool.name(), "docs_install_docset");
    assert!(!install_tool.description().is_empty());

    let search_docs_tool = DocsSearchDocsTool;
    assert_eq!(search_docs_tool.name(), "docs_search_docs");
    assert!(!search_docs_tool.description().is_empty());

    let read_page_tool = DocsReadDocPageTool;
    assert_eq!(read_page_tool.name(), "docs_read_doc_page");
    assert!(!read_page_tool.description().is_empty());

    let search_crate_tool = DocsSearchRustCrateTool;
    assert_eq!(search_crate_tool.name(), "docs_search_rust_crate");
    assert!(!search_crate_tool.description().is_empty());

    let read_crate_tool = DocsReadRustDocsTool;
    assert_eq!(read_crate_tool.name(), "docs_read_rust_docs");
    assert!(!read_crate_tool.description().is_empty());
}

#[test]
fn test_docs_tools_parameter_schemas_conform_to_openapi() {
    let tools: Vec<Box<dyn Tool>> = vec![
        Box::new(DocsListDocsetsTool),
        Box::new(DocsInstallDocsetTool),
        Box::new(DocsSearchDocsTool),
        Box::new(DocsReadDocPageTool),
        Box::new(DocsSearchRustCrateTool),
        Box::new(DocsReadRustDocsTool),
    ];

    for tool in tools {
        let params = tool.parameters();
        assert_eq!(
            params.get("type").and_then(|v| v.as_str()),
            Some("object"),
            "Tool {} parameters must have type 'object'",
            tool.name()
        );
        assert!(
            params.get("properties").and_then(|v| v.as_object()).is_some(),
            "Tool {} must define 'properties' object",
            tool.name()
        );
        assert_eq!(
            params.get("additionalProperties").and_then(|v| v.as_bool()),
            Some(true),
            "Tool {} must set additionalProperties: true to prevent schema filter rejection",
            tool.name()
        );
    }
}

#[tokio::test]
async fn test_docs_tools_parameter_validation() {
    let install_tool = DocsInstallDocsetTool;
    let res = install_tool.call(&json!({})).await.unwrap();
    assert_eq!(res["success"], false);
    assert!(res["error"].as_str().unwrap().contains("docset_name"));

    let search_tool = DocsSearchDocsTool;
    let res = search_tool.call(&json!({"docset_name": "react"})).await.unwrap();
    assert_eq!(res["success"], false);
    assert!(res["error"].as_str().unwrap().contains("query"));

    let read_page_tool = DocsReadDocPageTool;
    let res = read_page_tool.call(&json!({"docset_name": "react"})).await.unwrap();
    assert_eq!(res["success"], false);
    assert!(res["error"].as_str().unwrap().contains("path"));

    let search_crate_tool = DocsSearchRustCrateTool;
    let res = search_crate_tool.call(&json!({})).await.unwrap();
    assert_eq!(res["success"], false);
    assert!(res["error"].as_str().unwrap().contains("query"));

    let read_crate_tool = DocsReadRustDocsTool;
    let res = read_crate_tool.call(&json!({})).await.unwrap();
    assert_eq!(res["success"], false);
    assert!(res["error"].as_str().unwrap().contains("crate_name"));
}

#[test]
fn test_docs_service_in_memory_db_and_queries() {
    let temp_dir = std::env::temp_dir().join(format!("openz_docs_test_{}", uuid::Uuid::new_v4()));
    let _ = std::fs::create_dir_all(&temp_dir);
    let db_path = temp_dir.join("test_docs.db");

    let service = DocsService::new(db_path.clone());
    assert!(service.get_db_conn().is_ok());

    // Initially empty
    let list = service.list_docsets().unwrap();
    assert!(list.is_empty());

    // Populate mock docset
    let conn = service.get_db_conn().unwrap();
    conn.execute(
        "INSERT INTO docsets (name, last_updated) VALUES ('react', CURRENT_TIMESTAMP)",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO entries (docset, name, path, type) VALUES 
         ('react', 'useState', 'hooks/useState.html', 'Hook'),
         ('react', 'useEffect', 'hooks/useEffect.html', 'Hook')",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO contents (docset, path, html, markdown) VALUES 
         ('react', 'hooks/useState.html', '<h1>useState</h1><p>State hook</p>', NULL)",
        [],
    )
    .unwrap();

    // Verify list_docsets
    let list = service.list_docsets().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, "react");

    // Verify search_docs
    let search_res = service.search_docs("react", "useState").unwrap();
    assert_eq!(search_res.len(), 1);
    assert_eq!(search_res[0].name, "useState");
    assert_eq!(search_res[0].path, "hooks/useState.html");

    // Verify read_doc_page (converts HTML to Markdown and caches)
    let md = service.read_doc_page("react", "hooks/useState.html").unwrap();
    assert!(md.contains("useState"));

    // Verify cached markdown is returned on subsequent read
    let md_cached = service.read_doc_page("react", "hooks/useState.html").unwrap();
    assert_eq!(md, md_cached);

    let _ = std::fs::remove_dir_all(&temp_dir);
}
