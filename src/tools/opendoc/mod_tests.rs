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
