use super::*;

#[test]
fn test_parse_command_line() {
    let cmd = "my_program arg1 \"arg 2\" 'arg 3'";
    let args = parse_command_line(cmd);
    assert_eq!(args.len(), 4);
    assert_eq!(args[0], "my_program");
    assert_eq!(args[1], "arg1");
    assert_eq!(args[2], "arg 2");
    assert_eq!(args[3], "arg 3");
}

#[test]
fn test_find_wasm_file_nonexistent() {
    let path = find_wasm_file("nonexistent_wasm_file_12345");
    assert!(path.is_none());
}

#[test]
fn test_find_wasm_file_uses_workspace_openz_skills_not_repo_skills() {
    let original = std::env::current_dir().unwrap();
    let temp_dir =
        std::env::temp_dir().join(format!("openz_wasm_skills_{}", uuid::Uuid::new_v4()));
    let repo_skills = temp_dir.join("skills");
    let workspace_skills = temp_dir.join(".openz").join("skills");
    std::fs::create_dir_all(&repo_skills).unwrap();
    std::fs::create_dir_all(&workspace_skills).unwrap();
    std::fs::write(repo_skills.join("tool.wasm"), b"repo").unwrap();
    std::fs::write(workspace_skills.join("tool.wasm"), b"workspace").unwrap();
    std::env::set_current_dir(&temp_dir).unwrap();

    let found = find_wasm_file("tool");

    std::env::set_current_dir(original).unwrap();
    let _ = std::fs::remove_dir_all(&temp_dir);

    assert!(
        found
            .as_ref()
            .map(|path| path.ends_with(".openz/skills/tool.wasm"))
            .unwrap_or(false),
        "expected workspace .openz/skills WASM, got {found:?}"
    );
}

#[tokio::test]
async fn test_exec_command_fallback() {
    let tool = ExecCommandTool;
    let args = serde_json::json!({
        "command": "echo 'hello openz'"
    });
    let res = tool.call(&args).await.unwrap();
    assert!(res.get("status_code").is_some());
    let stdout = res["stdout"].as_str().unwrap();
    assert!(stdout.contains("hello openz"));
}

#[tokio::test]
async fn test_exec_command_wasm() {
    let temp_dir = std::env::temp_dir();
    let wasm_path = temp_dir.join("test_exec_command_wasm_temp_file_12345.wasm");

    let wasm_bytes: &[u8] = &[
        0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x04, 0x01, 0x60, 0x00, 0x00,
        0x03, 0x02, 0x01, 0x00, 0x07, 0x0a, 0x01, 0x06, 0x5f, 0x73, 0x74, 0x61, 0x72, 0x74,
        0x00, 0x00, 0x0a, 0x04, 0x01, 0x02, 0x00, 0x0b,
    ];

    std::fs::write(&wasm_path, wasm_bytes).unwrap();

    let tool = ExecCommandTool;
    let args = serde_json::json!({
        "command": format!("{} arg1 arg2", wasm_path.to_string_lossy())
    });

    let res = tool.call(&args).await.unwrap();

    // Clean up
    let _ = std::fs::remove_file(wasm_path);

    assert_eq!(res["status_code"].as_i64().unwrap(), 0);
    assert!(res.get("stdout").is_some());
    assert!(res.get("stderr").is_some());
}

#[tokio::test]
async fn test_exec_command_direct_string() {
    let tool = ExecCommandTool;
    let res = tool.call(&serde_json::json!("echo 'direct string shell'")).await.unwrap();
    assert_eq!(res["status_code"].as_i64().unwrap(), 0);
    assert!(res["stdout"].as_str().unwrap().contains("direct string shell"));
}

#[tokio::test]
async fn test_exec_command_aliases() {
    let tool = ExecCommandTool;
    let res = tool.call(&serde_json::json!({ "cmd": "echo 'alias cmd'" })).await.unwrap();
    assert_eq!(res["status_code"].as_i64().unwrap(), 0);
    assert!(res["stdout"].as_str().unwrap().contains("alias cmd"));
}

#[tokio::test]
async fn test_exec_command_cwd() {
    let tool = ExecCommandTool;
    let res = tool.call(&serde_json::json!({ "command": "pwd", "cwd": "src" })).await.unwrap();
    assert_eq!(res["status_code"].as_i64().unwrap(), 0);
    let stdout = res["stdout"].as_str().unwrap();
    assert!(stdout.trim().ends_with("/src") || stdout.trim().ends_with("\\src"));
}
