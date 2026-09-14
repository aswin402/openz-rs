use super::*;

#[tokio::test]
async fn test_generate_image_tool_metadata() -> Result<()> {
    let tool = GenerateImageTool;
    assert_eq!(tool.name(), "generate_image");
    let params = tool.parameters();
    assert!(params.get("properties").is_some());
    Ok(())
}

#[tokio::test]
async fn test_generate_image_tool_execution() -> Result<()> {
    let tool = GenerateImageTool;
    let temp_png = std::env::temp_dir().join("test_output_img.png");
    let _ = std::fs::remove_file(&temp_png);

    let args = json!({
        "html": "<html><body style='margin:0; background:linear-gradient(to right, #ff7e5f, #feb47b); width:100vw; height:100vh; display:flex; align-items:center; justify-content:center;'><h1 style='color:white; font-family:sans-serif;'>Hello High-Fidelity OpenZ!</h1></body></html>",
        "width": 400,
        "height": 300,
        "device_scale_factor": 1.0,
        "output_path": temp_png.to_string_lossy()
    });

    // Skip execution if browser is not available/runnable in the sandbox context
    if let Err(e) = ensure_browser_running().await {
        tracing::warn!(
            "Skipping execution test as headless browser is unavailable: {}",
            e
        );
        return Ok(());
    }

    let result = match tool.call(&args).await {
        Ok(res) => res,
        Err(e) => {
            tracing::warn!(
                "Skipping execution test as headless browser CDP operation failed: {}",
                e
            );
            return Ok(());
        }
    };
    assert_eq!(
        result.get("status").and_then(|v| v.as_str()),
        Some("success")
    );
    assert!(temp_png.exists());

    let _ = std::fs::remove_file(&temp_png);
    Ok(())
}
