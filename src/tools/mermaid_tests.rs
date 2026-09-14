use super::*;

#[tokio::test]
async fn test_render_mermaid() -> Result<()> {
    let tool = MermaidRendererTool;
    let temp_dir =
        std::env::temp_dir().join(format!("openz_mermaid_test_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&temp_dir)?;
    let output_file = temp_dir.join("test_chart.svg");

    let args = json!({
        "chart": "flowchart LR\n  A --> B",
        "output_path": output_file.to_str().unwrap()
    });

    let res = tool.call(&args).await?;
    assert_eq!(res["status"], "success");
    assert!(output_file.exists());

    let content = fs::read_to_string(output_file)?;
    assert!(content.contains("<svg"));

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}
