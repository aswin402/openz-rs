use super::*;
use crate::tools::self_management::TestEnvLock;
use crate::tools::Tool;

#[test]
fn test_manage_sessions() {
    let env_lock = TestEnvLock::acquire("openz_manage_sessions");
    let openz_dir = env_lock.dir();

    let tool = ManageSessionsTool;
    let rt = tokio::runtime::Runtime::new().unwrap();

    let sessions_dir = openz_dir.join("sessions");
    std::fs::create_dir_all(&sessions_dir).unwrap();

    // 1. Create a dummy session file for testing.
    let test_session_key = format!("test_session_xyz_{}", uuid::Uuid::new_v4());
    let session_file = sessions_dir.join(format!("{}.json", test_session_key));
    let _ = std::fs::remove_file(&session_file);
    std::fs::write(
        &session_file,
        serde_json::json!({
            "messages": [
                {
                    "role": "user",
                    "content": "Hello"
                }
            ]
        })
        .to_string(),
    )
    .unwrap();

    // 2. List sessions and check that our dummy is present
    let list_res = rt
        .block_on(tool.call(&serde_json::json!({
            "action": "list"
        })))
        .unwrap();
    assert_eq!(list_res["status"].as_str().unwrap(), "success");
    let sessions = list_res["sessions"].as_array().unwrap();
    let found = sessions
        .iter()
        .any(|s| s["session_key"].as_str().unwrap() == test_session_key);
    assert!(found);

    // 3. Archive our dummy session
    let archive_res = rt
        .block_on(tool.call(&serde_json::json!({
            "action": "archive",
            "session_key": &test_session_key
        })))
        .unwrap();
    assert_eq!(archive_res["status"].as_str().unwrap(), "success");
    assert!(!session_file.exists());

    // 4. Create another dummy session and delete it
    std::fs::write(&session_file, "{\"messages\": []}").unwrap();
    assert!(session_file.exists());

    let delete_res = rt
        .block_on(tool.call(&serde_json::json!({
            "action": "delete",
            "session_key": &test_session_key
        })))
        .unwrap();
    assert_eq!(delete_res["status"].as_str().unwrap(), "success");
    assert!(!session_file.exists());

    // 5. Test pruning (prune should execute without errors)
    let prune_res = rt
        .block_on(tool.call(&serde_json::json!({
            "action": "prune",
            "older_than_days": 30
        })))
        .unwrap();
    assert_eq!(prune_res["status"].as_str().unwrap(), "success");
    assert!(prune_res["details"]["files_removed"].as_u64().is_some());
}

#[test]
fn test_manage_sessions_colon_keys_and_coercion() {
    let env_lock = TestEnvLock::acquire("openz_manage_sessions_colon");
    let openz_dir = env_lock.dir();

    let tool = ManageSessionsTool;
    let rt = tokio::runtime::Runtime::new().unwrap();

    let sessions_dir = openz_dir.join("sessions");
    std::fs::create_dir_all(&sessions_dir).unwrap();

    // Stored on disk with safe_key replacing : with _
    let colon_session_key = format!("cli:headless_{}", uuid::Uuid::new_v4());
    let safe_key = crate::session::SessionManager::safe_key(&colon_session_key);
    let session_file = sessions_dir.join(format!("{}.json", safe_key));

    std::fs::write(
        &session_file,
        serde_json::json!({
            "messages": [{"role": "user", "content": "Test colon key"}]
        })
        .to_string(),
    )
    .unwrap();

    // 1. Test alias "ls" and list
    let list_res = rt
        .block_on(tool.call(&serde_json::json!({
            "action": "LS"
        })))
        .unwrap();
    assert_eq!(list_res["status"].as_str().unwrap(), "success");

    // 2. Test export with raw colon key
    let export_res = rt
        .block_on(tool.call(&serde_json::json!({
            "action": "dump",
            "session_key": &colon_session_key
        })))
        .unwrap();
    assert_eq!(export_res["status"].as_str().unwrap(), "success");
    assert_eq!(
        export_res["session"]["messages"][0]["content"],
        "Test colon key"
    );

    // 3. Test delete with raw colon key via alias "rm"
    let delete_res = rt
        .block_on(tool.call(&serde_json::json!({
            "action": "rm",
            "session_key": &colon_session_key
        })))
        .unwrap();
    assert_eq!(delete_res["status"].as_str().unwrap(), "success");
    assert!(!session_file.exists());

    // 4. Test prune with string older_than_days via alias "clean"
    let prune_res = rt
        .block_on(tool.call(&serde_json::json!({
            "action": "clean",
            "older_than_days": "14"
        })))
        .unwrap();
    assert_eq!(prune_res["status"].as_str().unwrap(), "success");

    // 5. Test empty object defaults to "list" action
    let default_list = rt.block_on(tool.call(&serde_json::json!({}))).unwrap();
    assert_eq!(default_list["status"].as_str().unwrap(), "success");

    // 6. Test direct string "list"
    let str_list = rt.block_on(tool.call(&serde_json::json!("list"))).unwrap();
    assert_eq!(str_list["status"].as_str().unwrap(), "success");
}
