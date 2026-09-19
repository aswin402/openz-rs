use super::*;
use crate::config::loader::CONFIG_DIR_OVERRIDE;
use crate::tools::Tool;

#[tokio::test]
async fn test_manage_backups() {
    let temp_dir =
        std::env::temp_dir().join(format!("openz_manage_backups_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    CONFIG_DIR_OVERRIDE
        .scope(temp_dir.clone(), async {
            let tool = ManageBackupsTool;

            // 1. Create a backup
            let create_res = tool
                .call(&serde_json::json!({
                    "action": "create"
                }))
                .await
                .unwrap();
            assert_eq!(create_res["status"].as_str().unwrap(), "success");
            let backup_name = create_res["backup_name"].as_str().unwrap();

            // 2. List backups and ensure it exists
            let list_res = tool
                .call(&serde_json::json!({
                    "action": "list"
                }))
                .await
                .unwrap();
            assert_eq!(list_res["status"].as_str().unwrap(), "success");
            let backups = list_res["backups"].as_array().unwrap();
            let found = backups
                .iter()
                .any(|b| b["backup_name"].as_str().unwrap() == backup_name);
            assert!(found);

            // 3. Restore from the backup
            let restore_res = tool
                .call(&serde_json::json!({
                    "action": "restore",
                    "backup_name": backup_name
                }))
                .await
                .unwrap();
            assert_eq!(restore_res["status"].as_str().unwrap(), "success");

            // 4. Delete the backup
            let delete_res = tool
                .call(&serde_json::json!({
                    "action": "delete",
                    "backup_name": backup_name
                }))
                .await
                .unwrap();
            assert_eq!(delete_res["status"].as_str().unwrap(), "success");

            // 5. Ensure it is gone from the list
            let list_res_2 = tool
                .call(&serde_json::json!({
                    "action": "list"
                }))
                .await
                .unwrap();
            let backups_2 = list_res_2["backups"].as_array().unwrap();
            let found_2 = backups_2
                .iter()
                .any(|b| b["backup_name"].as_str().unwrap() == backup_name);
            assert!(!found_2);
        })
        .await;

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_manage_backups_aliases_and_case() {
    let temp_dir = std::env::temp_dir().join(format!(
        "openz_manage_backups_aliases_test_{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();

    CONFIG_DIR_OVERRIDE
        .scope(temp_dir.clone(), async {
            let tool = ManageBackupsTool;

            // 1. Create via direct string "create"
            let create_res = tool.call(&serde_json::json!("create")).await.unwrap();
            assert_eq!(create_res["status"].as_str().unwrap(), "success");
            let backup_name = create_res["backup_name"].as_str().unwrap();

            // 2. List via empty object and direct string "list"
            let empty_res = tool.call(&serde_json::json!({})).await.unwrap();
            assert_eq!(empty_res["status"].as_str().unwrap(), "success");

            let list_res = tool.call(&serde_json::json!("ls")).await.unwrap();
            assert_eq!(list_res["status"].as_str().unwrap(), "success");
            let backups = list_res["backups"].as_array().unwrap();
            assert!(backups.iter().any(|b| b["backup_name"].as_str().unwrap() == backup_name));

            // 3. Restore via direct string of backup filename
            let restore_res = tool.call(&serde_json::json!(backup_name)).await.unwrap();
            assert_eq!(restore_res["status"].as_str().unwrap(), "success");

            // 4. Delete via alias "rm" with whitespace padding
            let padded_name = format!("  {}  ", backup_name);
            let delete_res = tool
                .call(&serde_json::json!({
                    "action": "rm",
                    "name": padded_name
                }))
                .await
                .unwrap();
            assert_eq!(delete_res["status"].as_str().unwrap(), "success");
        })
        .await;

    let _ = std::fs::remove_dir_all(&temp_dir);
}
