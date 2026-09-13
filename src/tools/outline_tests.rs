use super::*;

#[tokio::test]
async fn test_code_outline() -> Result<()> {
    let temp_dir =
        std::env::temp_dir().join(format!("openz_outline_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir)?;

    let rust_file = temp_dir.join("main.rs");
    std::fs::write(
        &rust_file,
        "
        pub fn run_app() {
            println!(\"Hello!\");
        }
        struct Config {
            port: u16,
        }
    ",
    )?;

    let tool = CodeOutlineTool;
    let res = tool
        .call(&json!({
            "file_path": rust_file.to_str().unwrap()
        }))
        .await?;

    assert_eq!(res["status"], "success");
    let symbols = res["symbols"].as_array().unwrap();
    assert_eq!(symbols.len(), 2);
    assert_eq!(symbols[0]["kind"], "fn");
    assert_eq!(symbols[0]["name"], "run_app");
    assert_eq!(symbols[1]["kind"], "struct");
    assert_eq!(symbols[1]["name"], "Config");

    // Clean up
    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[tokio::test]
async fn test_code_outline_js_ts() -> Result<()> {
    let temp_dir =
        std::env::temp_dir().join(format!("openz_outline_js_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir)?;

    let ts_file = temp_dir.join("app.ts");
    std::fs::write(
        &ts_file,
        "
        export interface User {
            id: number;
            name: string;
        }
        class UserService {
            getUser(id: number): User {
                return { id, name: 'OpenZ' };
            }
        }
        function logUser(u: User) {
            console.log(u.name);
        }
    ",
    )?;

    let tool = CodeOutlineTool;
    let res = tool
        .call(&json!({
            "file_path": ts_file.to_str().unwrap()
        }))
        .await?;

    assert_eq!(res["status"], "success");
    let symbols = res["symbols"].as_array().unwrap();

    // Should find interface User, class UserService, and function logUser
    assert!(symbols
        .iter()
        .any(|s| s["kind"] == "interface" && s["name"] == "User"));
    assert!(symbols
        .iter()
        .any(|s| s["kind"] == "class" && s["name"] == "UserService"));
    assert!(symbols
        .iter()
        .any(|s| s["kind"] == "function" && s["name"] == "logUser"));

    // Clean up
    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(())
}
