use super::*;

#[tokio::test]
async fn test_wasm_execute_metadata() -> Result<()> {
    let tool = WasmSandboxTool;
    assert_eq!(tool.name(), "wasm_execute");
    assert!(tool.description().contains("secure"));

    let args = json!({
        "wasm_path": "nonexistent.wasm"
    });
    let res = tool.call(&args).await;
    assert!(res.is_err());
    Ok(())
}
