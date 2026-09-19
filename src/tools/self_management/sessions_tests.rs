use super::*;
use crate::tools::Tool;

struct TestEnvLock;

impl TestEnvLock {
    fn acquire() -> Self {
        let lock_path = std::env::temp_dir().join("openz_test_config_dir.lock");
        loop {
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&lock_path)
            {
                Ok(_) => break,
                Err(_) => std::thread::sleep(std::time::Duration::from_millis(50)),
            }
        }
        TestEnvLock
    }
}

impl Drop for TestEnvLock {
    fn drop(&mut self) {
        let lock_path = std::env::temp_dir().join("openz_test_config_dir.lock");
        let _ = std::fs::remove_file(lock_path);
    }
}

#[test]
fn test_manage_sessions() {
    let _env_lock = TestEnvLock::acquire();
    let previous_config_dir = std::env::var("OPENZ_CONFIG_DIR").ok();
    let openz_dir =
        std::env::temp_dir().join(format!("openz_manage_sessions_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&openz_dir).unwrap();
    std::env::set_var("OPENZ_CONFIG_DIR", &openz_dir);

    let tool = ManageSessionsTool;
    let rt = tokio::runtime::Runtime::new().unwrap();

    let sessions_dir = openz_dir.join("sessions");
    std::fs::create_dir_all(&sessions_dir).unwrap();

    // 1. Create a dummy session file for testing. Use a unique key because
    // the full lib test suite runs session-management tests in parallel.
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

    // 2. List sessions and check if our dummy is present
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

    // Clean up archived files
    let archives_dir = openz_dir.join("archives");
    if let Ok(entries) = std::fs::read_dir(&archives_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_str().unwrap_or("");
            if name_str.starts_with(&test_session_key) {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }

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

    if let Some(prev) = previous_config_dir {
        std::env::set_var("OPENZ_CONFIG_DIR", prev);
    } else {
        std::env::remove_var("OPENZ_CONFIG_DIR");
    }
    let _ = std::fs::remove_dir_all(&openz_dir);
}

#[test]
fn test_manage_sessions_colon_keys_and_coercion() {
    let _env_lock = TestEnvLock::acquire();
    let previous_config_dir = std::env::var("OPENZ_CONFIG_DIR").ok();
    let openz_dir =
        std::env::temp_dir().join(format!("openz_manage_sessions_colon_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&openz_dir).unwrap();
    std::env::set_var("OPENZ_CONFIG_DIR", &openz_dir);

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
    assert_eq!(export_res["session"]["messages"][0]["content"], "Test colon key");

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

    // 6. Test direct string "list" and "prune"
    let str_list = rt.block_on(tool.call(&serde_json::json!("list"))).unwrap();
    assert_eq!(str_list["status"].as_str().unwrap(), "success");

    if let Some(prev) = previous_config_dir {
        std::env::set_var("OPENZ_CONFIG_DIR", prev);
    } else {
        std::env::remove_var("OPENZ_CONFIG_DIR");
    }
    let _ = std::fs::remove_dir_all(&openz_dir);
}
