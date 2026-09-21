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

#[test]
fn test_all_opendoc_tool_parameters_schemas_conform_to_openapi() {
    let tools: Vec<Box<dyn Tool>> = vec![
        Box::new(OpendocOpenDocumentTool),
        Box::new(OpendocReadDocumentTextTool),
        Box::new(OpendocSearchDocumentTool),
        Box::new(OpendocReplaceTextTool),
        Box::new(OpendocDiffDocumentsTool),
        Box::new(OpendocDiffDocumentsVisualTool),
        Box::new(OpendocChunkForEmbeddingTool),
        Box::new(OpendocFillTemplateTool),
        Box::new(OpendocValidateDocumentTool),
        Box::new(OpendocValidatePdfAComplianceTool),
        Box::new(OpendocExtractStructuredMetadataTool),
        Box::new(OpendocConvertTool),
        Box::new(OpendocExtractImagesTool),
        Box::new(OpendocSplitPdfTool),
        Box::new(OpendocCreateHtmlTool),
        Box::new(OpendocBatchConvertTool),
        Box::new(OpendocCreateDocxTool),
        Box::new(OpendocDocxAddParagraphTool),
        Box::new(OpendocDocxAddTableTool),
        Box::new(OpendocDocxAddImageTool),
        Box::new(OpendocCreatePptxTool),
        Box::new(OpendocPptxAddSlideTool),
        Box::new(OpendocCreateXlsxTool),
        Box::new(OpendocEditXlsxTool),
        Box::new(OpendocCreatePdfTool),
        Box::new(OpendocCreateFormattedPdfTool),
        Box::new(OpendocMergePdfsTool),
        Box::new(OpendocExtractPdfTextTool),
        Box::new(OpendocListPdfFieldsTool),
        Box::new(OpendocFillPdfFormTool),
        Box::new(OpendocFindTablesTool),
        Box::new(OpendocAnalyzeDocumentComplexityTool),
        Box::new(OpendocOcrDocumentTool),
        Box::new(OpendocCheckOcrAvailableTool),
        Box::new(OpendocRenderDocumentPagesTool),
        Box::new(OpendocExtractArchiveDigestTool),
    ];

    assert_eq!(tools.len(), 36);

    let mut schema_issues = Vec::new();

    for tool in tools {
        let params = tool.parameters();
        let name = tool.name();

        if params.get("$schema").is_some() {
            schema_issues.push(format!("{}: contains '$schema' which should be stripped", name));
        }

        if params.get("title").is_some() {
            schema_issues.push(format!("{}: contains top-level 'title' which should be stripped", name));
        }

        if params.get("type").and_then(|v| v.as_str()) != Some("object") {
            schema_issues.push(format!("{}: missing top-level type: 'object'", name));
        }

        if let Some(props) = params.get("properties").and_then(|v| v.as_object()) {
            for (prop_name, prop_val) in props {
                // Check for empty schema {}
                if prop_val.as_object().map_or(false, |m| m.is_empty()) {
                    schema_issues.push(format!("{}.{}: empty schema {{}}", name, prop_name));
                }

                // Check for missing type
                let has_type = prop_val.get("type").is_some();
                let has_ref = prop_val.get("$ref").is_some();
                let has_any_of = prop_val.get("anyOf").is_some();
                let has_one_of = prop_val.get("oneOf").is_some();
                if !has_type && !has_ref && !has_any_of && !has_one_of {
                    schema_issues.push(format!("{}.{}: missing 'type'", name, prop_name));
                }

                // Check for bare object type without properties or additionalProperties
                let is_object_type = match prop_val.get("type") {
                    Some(serde_json::Value::String(s)) => s == "object",
                    Some(serde_json::Value::Array(arr)) => arr.iter().any(|v| v.as_str() == Some("object")),
                    _ => false,
                };
                if is_object_type {
                    let has_props = prop_val.get("properties").is_some();
                    let has_add_props = prop_val.get("additionalProperties").is_some();
                    if !has_props && !has_add_props {
                        schema_issues.push(format!("{}.{}: bare object type without properties or additionalProperties", name, prop_name));
                    }
                }
            }
        }
    }

    if !schema_issues.is_empty() {
        panic!("Found {} schema issue(s) across opendoc tools:\n{:#?}", schema_issues.len(), schema_issues);
    }
}

#[test]
fn test_opendoc_schema_sanitization_details() {
    let fill_tool = OpendocFillTemplateTool;
    let fill_params = fill_tool.parameters();
    assert!(fill_params.get("$schema").is_none());
    assert!(fill_params.get("title").is_none());
    let var_prop = fill_params["properties"]["variables"].as_object().unwrap();
    assert_eq!(var_prop.get("type").and_then(|v| v.as_str()), Some("object"));
    assert_eq!(var_prop.get("additionalProperties").and_then(|v| v.as_bool()), Some(true));

    let xlsx_tool = OpendocCreateXlsxTool;
    let xlsx_params = xlsx_tool.parameters();
    assert!(xlsx_params.get("$schema").is_none());
    let sheets_prop = xlsx_params["properties"]["sheets"].as_object().unwrap();
    assert_eq!(sheets_prop.get("type").and_then(|v| v.as_str()), Some("array"));

    let edit_xlsx_tool = OpendocEditXlsxTool;
    let edit_params = edit_xlsx_tool.parameters();
    assert_eq!(edit_params["properties"]["add_sheets"].get("type").and_then(|v| v.as_str()), Some("array"));
    assert_eq!(edit_params["properties"]["cell_updates"].get("type").and_then(|v| v.as_str()), Some("array"));

    let pdf_form_tool = OpendocFillPdfFormTool;
    let pdf_form_params = pdf_form_tool.parameters();
    let values_prop = pdf_form_params["properties"]["values"].as_object().unwrap();
    assert_eq!(values_prop.get("type").and_then(|v| v.as_str()), Some("object"));
    assert_eq!(values_prop.get("additionalProperties").and_then(|v| v.as_bool()), Some(true));
}



