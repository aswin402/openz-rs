use super::*;
use crate::tools::Tool;

#[test]
fn test_manage_backups() {
    let tool = ManageBackupsTool;
    let rt = tokio::runtime::Runtime::new().unwrap();

    // 1. Create a backup
    let create_res = rt
        .block_on(tool.call(&serde_json::json!({
            "action": "create"
        })))
        .unwrap();
    assert_eq!(create_res["status"].as_str().unwrap(), "success");
    let backup_name = create_res["backup_name"].as_str().unwrap();

    // 2. List backups and ensure it exists
    let list_res = rt
        .block_on(tool.call(&serde_json::json!({
            "action": "list"
        })))
        .unwrap();
    assert_eq!(list_res["status"].as_str().unwrap(), "success");
    let backups = list_res["backups"].as_array().unwrap();
    let found = backups
        .iter()
        .any(|b| b["backup_name"].as_str().unwrap() == backup_name);
    assert!(found);

    // 3. Restore from the backup
    let restore_res = rt
        .block_on(tool.call(&serde_json::json!({
            "action": "restore",
            "backup_name": backup_name
        })))
        .unwrap();
    assert_eq!(restore_res["status"].as_str().unwrap(), "success");

    // 4. Delete the backup
    let delete_res = rt
        .block_on(tool.call(&serde_json::json!({
            "action": "delete",
            "backup_name": backup_name
        })))
        .unwrap();
    assert_eq!(delete_res["status"].as_str().unwrap(), "success");

    // 5. Ensure it is gone from the list
    let list_res_2 = rt
        .block_on(tool.call(&serde_json::json!({
            "action": "list"
        })))
        .unwrap();
    let backups_2 = list_res_2["backups"].as_array().unwrap();
    let found_2 = backups_2
        .iter()
        .any(|b| b["backup_name"].as_str().unwrap() == backup_name);
    assert!(!found_2);
}
