use super::*;

#[tokio::test]
async fn test_cargo_manager() -> Result<()> {
    let project_dir =
        std::env::temp_dir().join(format!("openz_cargo_manager_test_{}", uuid::Uuid::new_v4()));
    let src_dir = project_dir.join("src");
    std::fs::create_dir_all(&src_dir)?;
    std::fs::write(
        project_dir.join("Cargo.toml"),
        "[package]\nname = \"openz-cargo-manager-test\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )?;
    std::fs::write(
        src_dir.join("lib.rs"),
        "pub fn add(a: i32, b: i32) -> i32 { a + b }\n",
    )?;

    let provider = Arc::new(crate::providers::openai::OpenAIProvider::new(
        "".to_string(),
        "".to_string(),
        "".to_string(),
    ));
    let tool = CargoManagerTool::new(provider);
    let res = tool
        .call(&json!({
            "action": "clippy",
            "cwd": project_dir.to_string_lossy(),
            "self_heal": false
        }))
        .await?;

    let _ = std::fs::remove_dir_all(&project_dir);

    assert_eq!(res["status"], "success");
    assert!(res["diagnostics"].is_array());

    Ok(())
}
