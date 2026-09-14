use super::*;

#[test]
fn test_compiler_auto_heal_schema() {
    let config = Config::default();
    let provider = Arc::new(crate::providers::mock::MockProvider::new());
    let tool = CompilerAutoHealTool { config, provider };

    assert_eq!(tool.name(), "compiler_auto_heal");
    let params = tool.parameters();
    assert!(params["required"].as_array().unwrap().contains(&Value::String("file_path".into())));
    assert!(params["required"].as_array().unwrap().contains(&Value::String("instruction".into())));
    assert!(params["required"].as_array().unwrap().contains(&Value::String("compile_command".into())));
}

#[tokio::test]
async fn test_compiler_auto_heal_missing_args() {
    let config = Config::default();
    let provider = Arc::new(crate::providers::mock::MockProvider::new());
    let tool = CompilerAutoHealTool { config, provider };

    let res = tool.call(&serde_json::json!({})).await;
    assert!(res.is_err());
}

#[tokio::test]
async fn test_compiler_auto_heal_success() {
    let temp_dir = std::env::temp_dir().join(format!("auto_heal_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir).unwrap();
    let target_file = temp_dir.join("main.rs");
    std::fs::write(&target_file, "fn main() { broken }").unwrap();

    let provider = Arc::new(
        crate::providers::mock::MockProvider::new().with_default(
            crate::providers::mock::MockResponse::text("```rust\nfn main() {}\n```"),
        ),
    );
    let config = Config::default();
    let tool = CompilerAutoHealTool { config, provider };

    let res = tool
        .call(&serde_json::json!({
            "file_path": target_file.to_str().unwrap(),
            "instruction": "Fix main",
            "compile_command": "true",
            "max_iterations": 2
        }))
        .await
        .unwrap();

    assert_eq!(res["status"], "success");
    assert_eq!(std::fs::read_to_string(&target_file).unwrap(), "fn main() {}");

    let _ = std::fs::remove_dir_all(&temp_dir);
}
