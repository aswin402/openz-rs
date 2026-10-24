use super::*;
use crate::config::loader::CONFIG_DIR_OVERRIDE;
use crate::tools::Tool;

#[tokio::test]
async fn test_curate_skills() {
    let temp_dir =
        std::env::temp_dir().join(format!("openz_skills_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    CONFIG_DIR_OVERRIDE
        .scope(temp_dir.clone(), async {
            let tool = CurateSkillTool;

            // 1. Delete skill if exists
            let _ = tool
                .call(&serde_json::json!({
                    "action": "delete",
                    "skill_name": "test_curate_skills_temp"
                }))
                .await;

            // 2. Add skill
            let add_res = tool
                .call(&serde_json::json!({
                    "action": "add",
                    "skill_name": "test_curate_skills_temp",
                    "content": "This is a test skill content"
                }))
                .await
                .unwrap();
            assert!(add_res["success"].as_bool().unwrap());

            // 3. List skills and verify
            let list_res = tool
                .call(&serde_json::json!({
                    "action": "list"
                }))
                .await
                .unwrap();
            assert!(list_res["success"].as_bool().unwrap());
            let skills = list_res["skills"].as_array().unwrap();
            let found = skills
                .iter()
                .any(|s| s["name"].as_str().unwrap() == "test_curate_skills_temp");
            assert!(found);

            // 4. Delete skill
            let del_res = tool
                .call(&serde_json::json!({
                    "action": "delete",
                    "skill_name": "test_curate_skills_temp"
                }))
                .await
                .unwrap();
            assert!(del_res["success"].as_bool().unwrap());
        })
        .await;

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_curate_skills_aliases_and_case() {
    let temp_dir = std::env::temp_dir().join(format!(
        "openz_skills_aliases_test_{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();

    CONFIG_DIR_OVERRIDE
        .scope(temp_dir.clone(), async {
            let tool = CurateSkillTool;

            let _ = tool
                .call(&serde_json::json!({
                    "action": "rm",
                    "skill_name": "test_curate_skills_alias"
                }))
                .await;

            // 1. Add via alias "SAVE" with untrimmed skill_name
            let add_res = tool
                .call(&serde_json::json!({
                    "action": "SAVE",
                    "skill_name": "  test_curate_skills_alias  ",
                    "content": "Alias content"
                }))
                .await
                .unwrap();
            assert!(add_res["success"].as_bool().unwrap());

            // 2. List via alias "view"
            let list_res = tool
                .call(&serde_json::json!({
                    "action": "view"
                }))
                .await
                .unwrap();
            assert!(list_res["success"].as_bool().unwrap());
            let skills = list_res["skills"].as_array().unwrap();
            let found = skills
                .iter()
                .any(|s| s["name"].as_str().unwrap() == "test_curate_skills_alias");
            assert!(found);

            // 3. List via direct string "list" and empty object
            let str_res = tool.call(&serde_json::json!("list")).await.unwrap();
            assert!(str_res["success"].as_bool().unwrap());

            let empty_res = tool.call(&serde_json::json!({})).await.unwrap();
            assert!(empty_res["success"].as_bool().unwrap());

            // 4. Delete via alias "rm"
            let del_res = tool
                .call(&serde_json::json!({
                    "action": "rm",
                    "skill_name": "test_curate_skills_alias"
                }))
                .await
                .unwrap();
            assert!(del_res["success"].as_bool().unwrap());
        })
        .await;

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_curate_skills_infers_add_without_explicit_action() {
    let temp_dir = std::env::temp_dir().join(format!(
        "openz_skills_infer_test_{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();

    CONFIG_DIR_OVERRIDE
        .scope(temp_dir.clone(), async {
            let tool = CurateSkillTool;

            let res = tool
                .call(&serde_json::json!({
                    "skill_name": "inferred_skill",
                    "content": "This action was automatically inferred."
                }))
                .await
                .unwrap();
            assert!(res["success"].as_bool().unwrap());

            let list_res = tool.call(&serde_json::json!("list")).await.unwrap();
            let skills = list_res["skills"].as_array().unwrap();
            assert!(skills.iter().any(|s| s["name"].as_str().unwrap() == "inferred_skill"));
        })
        .await;

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_curate_skills_snapshot_history_and_rollback() {
    let temp_dir = std::env::temp_dir().join(format!(
        "openz_skills_rollback_test_{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();

    CONFIG_DIR_OVERRIDE
        .scope(temp_dir.clone(), async {
            let tool = CurateSkillTool;

            let _ = tool
                .call(&serde_json::json!({
                    "action": "add",
                    "skill_name": "versioned_skill",
                    "content": "# Versioned Skill\n\nVersion 1 guideline"
                }))
                .await
                .unwrap();

            let _ = tool
                .call(&serde_json::json!({
                    "action": "add",
                    "skill_name": "versioned_skill",
                    "content": "# Versioned Skill\n\nVersion 2 guideline"
                }))
                .await
                .unwrap();

            let history_res = tool
                .call(&serde_json::json!({
                    "action": "history",
                    "skill_name": "versioned_skill"
                }))
                .await
                .unwrap();
            assert_eq!(history_res["success"].as_bool(), Some(true));
            assert_eq!(history_res["revision_count"].as_u64(), Some(1));

            let rollback_res = tool
                .call(&serde_json::json!({
                    "action": "rollback",
                    "skill_name": "versioned_skill"
                }))
                .await
                .unwrap();
            assert_eq!(rollback_res["success"].as_bool(), Some(true));
            assert!(rollback_res["content"]
                .as_str()
                .unwrap()
                .contains("Version 1 guideline"));
        })
        .await;

    let _ = std::fs::remove_dir_all(&temp_dir);
}
