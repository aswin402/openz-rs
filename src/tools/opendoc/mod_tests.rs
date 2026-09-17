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

