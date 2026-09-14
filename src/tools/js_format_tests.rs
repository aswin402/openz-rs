use super::*;

#[tokio::test]
async fn test_js_format_success() {
    let tool = JsFormatTool;
    let args = json!({
        "code": "const a:number=1;function foo(){return a;}",
        "file_path": "test.ts"
    });

    let res = tool.call(&args).await.unwrap();
    assert_eq!(res["status"], "success");
    let formatted = res["formatted"].as_str().unwrap();
    assert!(formatted.contains("const a"));
    assert!(formatted.contains("function foo"));
}

#[tokio::test]
async fn test_js_format_syntax_error() {
    let tool = JsFormatTool;
    let args = json!({
        "code": "const a = ;",
        "file_path": "test.js"
    });

    let res = tool.call(&args).await.unwrap();
    assert_eq!(res["status"], "error");
    assert!(!res["errors"].as_str().unwrap().is_empty());
}
