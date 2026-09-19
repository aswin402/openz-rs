use super::*;

#[test]
fn empty_pdf_text_is_ocr_candidate() {
    assert!(document_text_needs_ocr("   "));
    assert!(document_text_needs_ocr(
        "
	  "
    ));
}

#[test]
fn normal_document_text_does_not_need_ocr() {
    assert!(!document_text_needs_ocr(
        "This PDF already contains enough extractable text to answer questions from it."
    ));
}

#[test]
fn image_extensions_are_ocr_supported_documents() {
    assert!(is_ocr_supported_extension(Some("png")));
    assert!(is_ocr_supported_extension(Some("jpg")));
    assert!(is_ocr_supported_extension(Some("jpeg")));
    assert!(is_ocr_supported_extension(Some("tiff")));
    assert!(!is_ocr_supported_extension(Some("docx")));
}

#[test]
fn pdf_complexity_analysis_defaults_on() {
    assert!(should_analyze_document_complexity(Some("pdf"), &json!({})));
}

#[test]
fn complexity_analysis_can_be_disabled() {
    assert!(!should_analyze_document_complexity(
        Some("pdf"),
        &json!({ "analyze_complexity": false })
    ));
}

#[test]
fn spreadsheets_skip_complexity_analysis_by_default() {
    assert!(!should_analyze_document_complexity(
        Some("xlsx"),
        &json!({})
    ));
}

#[test]
fn ocr_json_text_is_extracted_from_success_response() {
    let parsed = ocr_text_from_result(&json!({
        "success": true,
        "text": "Scanned invoice total 42"
    }))
    .unwrap();
    assert_eq!(parsed, "Scanned invoice total 42");
}

#[tokio::test]
async fn test_doc_reader_metadata() -> Result<()> {
    let tool = DocReaderTool;
    assert_eq!(tool.name(), "read_doc");
    assert!(tool.description().contains("PDF"));

    let args = json!({
        "path": "nonexistent.pdf"
    });
    let res = tool.call(&args).await;
    assert!(res.is_err());
    Ok(())
}

#[tokio::test]
async fn test_doc_reader_direct_string_and_aliases() {
    let tool = DocReaderTool;

    // Direct string argument
    let err = tool.call(&json!("nonexistent_sample.pdf")).await.unwrap_err();
    assert!(err.to_string().contains("File does not exist: nonexistent_sample.pdf"));

    // file_path alias
    let err = tool.call(&json!({ "file_path": "missing_doc.docx" })).await.unwrap_err();
    assert!(err.to_string().contains("File does not exist: missing_doc.docx"));

    // file:// prefix stripping
    let err = tool.call(&json!({ "file": "file:///tmp/missing_file.xlsx" })).await.unwrap_err();
    assert!(err.to_string().contains("File does not exist"));

    // document alias with boolean string auto_ocr
    let err = tool.call(&json!({
        "document": "missing.pdf",
        "auto_ocr": "false",
        "analyze_complexity": "0"
    })).await.unwrap_err();
    assert!(err.to_string().contains("File does not exist: missing.pdf"));
}

