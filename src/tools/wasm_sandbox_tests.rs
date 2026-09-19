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

#[tokio::test]
async fn test_wasm_execute_direct_string() -> Result<()> {
    let tool = WasmSandboxTool;
    let res = tool.call(&json!("nonexistent_direct.wasm")).await;
    assert!(res.is_err());
    let err = res.unwrap_err().to_string();
    assert!(err.contains("does not exist"));
    Ok(())
}

#[tokio::test]
async fn test_wasm_execute_aliases() -> Result<()> {
    let tool = WasmSandboxTool;
    let res = tool.call(&json!({
        "file": "nonexistent_alias.wasm",
        "args": "arg1 arg2"
    })).await;
    assert!(res.is_err());
    let err = res.unwrap_err().to_string();
    assert!(err.contains("does not exist"));
    Ok(())
}
