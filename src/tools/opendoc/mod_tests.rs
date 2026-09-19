use super::*;
use crate::tools::Tool;

#[tokio::test]
async fn test_opendoc_check_ocr_available() {
    let tool = OpendocCheckOcrAvailableTool;
    assert_eq!(tool.name(), "opendoc_check_ocr_available");
    let result = tool.call(&serde_json::json!({})).await;
    assert!(result.is_ok(), "Expected tool call to succeed");
    let val = result.unwrap();
    assert!(val.get("available").is_some());
}

#[test]
fn test_server_init() {
    let server = get_server();
    let res = server.check_ocr_available();
    assert!(!res.is_empty());
}

#[tokio::test]
async fn test_opendoc_extract_structured_metadata_optional_template_type() {
    let tool = OpendocExtractStructuredMetadataTool;
    assert_eq!(tool.name(), "opendoc_extract_structured_metadata");

    let temp_file = std::env::temp_dir().join(format!("test_meta_{}.txt", uuid::Uuid::new_v4()));
    std::fs::write(&temp_file, "On 2026-09-17, Acme signed the contract for $100,000.").unwrap();

    // Call without template_type
    let result = tool.call(&serde_json::json!({
        "file_path": temp_file.to_str().unwrap()
    })).await;

    assert!(result.is_ok(), "Expected tool call with omitted template_type to succeed");
    let val = result.unwrap();
    assert!(val.get("timeline").is_some() || val.get("legal").is_some() || val.get("financial").is_some());

    let _ = std::fs::remove_file(temp_file);
}

#[tokio::test]
async fn test_opendoc_open_and_read_direct_string_and_aliases() {
    let open_tool = OpendocOpenDocumentTool;
    let read_tool = OpendocReadDocumentTextTool;

    let temp_file = std::env::temp_dir().join(format!("test_read_{}.txt", uuid::Uuid::new_v4()));
    let temp_path = temp_file.to_str().unwrap().to_string();
    std::fs::write(&temp_file, "OpenDoc test content for verification.").unwrap();

    // 1. Direct string argument to opendoc_open_document
    let res = open_tool.call(&json!(temp_path)).await;
    assert!(res.is_ok(), "Direct string call to open_document should succeed: {:?}", res);

    // 2. Direct string argument to opendoc_read_document_text
    let res = read_tool.call(&json!(temp_path)).await;
    assert!(res.is_ok(), "Direct string call to read_document_text should succeed: {:?}", res);
    let val = res.unwrap();
    let text = val.get("text").and_then(|v| v.as_str()).unwrap_or("");
    assert!(text.contains("OpenDoc test content"));

    // 3. "path" alias instead of "file_path"
    let res = read_tool.call(&json!({ "path": temp_path })).await;
    assert!(res.is_ok(), "path alias should succeed");

    // 4. "file://" prefix stripping with "document" alias
    let uri = format!("file://{}", temp_path);
    let res = read_tool.call(&json!({ "document": uri })).await;
    assert!(res.is_ok(), "file:// URI and document alias should succeed");

    let _ = std::fs::remove_file(temp_file);
}

#[tokio::test]
async fn test_opendoc_search_and_find_tables() {
    let search_tool = OpendocSearchDocumentTool;
    let tables_tool = OpendocFindTablesTool;

    let temp_file = std::env::temp_dir().join(format!("test_search_{}.txt", uuid::Uuid::new_v4()));
    let temp_path = temp_file.to_str().unwrap().to_string();
    std::fs::write(&temp_file, "OpenDoc searching for keyword 'target_key' in document.").unwrap();

    // Search with "path" and "q" aliases
    let res = search_tool.call(&json!({
        "path": temp_path,
        "q": "target_key"
    })).await;
    assert!(res.is_ok(), "search with aliases should succeed: {:?}", res);

    // Find tables with direct string
    let res = tables_tool.call(&json!(temp_path)).await;
    assert!(res.is_ok(), "find_tables with direct string should succeed: {:?}", res);

    let _ = std::fs::remove_file(temp_file);
}

#[tokio::test]
async fn test_opendoc_creation_tools_resilience() {
    let html_tool = OpendocCreateHtmlTool;
    let docx_tool = OpendocCreateDocxTool;
    let xlsx_tool = OpendocCreateXlsxTool;

    let dir = std::env::temp_dir();
    let temp_html = dir.join(format!("test_create_{}.html", uuid::Uuid::new_v4()));
    let temp_docx = dir.join(format!("test_create_{}.docx", uuid::Uuid::new_v4()));
    let temp_xlsx = dir.join(format!("test_create_{}.xlsx", uuid::Uuid::new_v4()));

    // Create HTML with "path" and "content" aliases
    let res = html_tool.call(&json!({
        "path": temp_html.to_str().unwrap(),
        "content": "Paragraph 1\nParagraph 2"
    })).await;
    assert!(res.is_ok(), "create_html with path/content aliases should succeed: {:?}", res);
    assert!(temp_html.exists());
    let _ = std::fs::remove_file(temp_html);

    // Create DOCX with direct string output path
    let res = docx_tool.call(&json!(temp_docx.to_str().unwrap())).await;
    assert!(res.is_ok(), "create_docx with direct string should succeed: {:?}", res);
    assert!(temp_docx.exists());
    let _ = std::fs::remove_file(temp_docx);

    // Create XLSX with "path" alias and default sheets
    let res = xlsx_tool.call(&json!({
        "path": temp_xlsx.to_str().unwrap()
    })).await;
    assert!(res.is_ok(), "create_xlsx with default sheets should succeed: {:?}", res);
    assert!(temp_xlsx.exists());
    let _ = std::fs::remove_file(temp_xlsx);
}

#[tokio::test]
async fn test_opendoc_complexity_and_ocr_tools() {
    let complexity_tool = OpendocAnalyzeDocumentComplexityTool;
    let ocr_tool = OpendocOcrDocumentTool;

    let temp_file = std::env::temp_dir().join(format!("test_complex_{}.txt", uuid::Uuid::new_v4()));
    let temp_path = temp_file.to_str().unwrap().to_string();
    std::fs::write(&temp_file, "Document complexity sample content.").unwrap();

    // Direct string to complexity analyzer
    let res = complexity_tool.call(&json!(temp_path)).await;
    assert!(res.is_ok(), "complexity tool with direct string should succeed: {:?}", res);

    // OCR tool with "path" and "lang" aliases
    let res = ocr_tool.call(&json!({
        "path": temp_path,
        "lang": "eng"
    })).await;
    assert!(res.is_ok(), "ocr tool with path/lang aliases should succeed: {:?}", res);

    let _ = std::fs::remove_file(temp_file);
}


