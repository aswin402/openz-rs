use super::*;
use crate::tools::Tool;

#[test]
fn test_curate_skills() {
    // Run database queries through curate_skill tool
    let tool = CurateSkillTool;
    let rt = tokio::runtime::Runtime::new().unwrap();

    // 1. Delete skill if exists
    let _ = rt.block_on(tool.call(&serde_json::json!({
        "action": "delete",
        "skill_name": "test_curate_skills_temp"
    })));

    // 2. Add skill
    let add_res = rt
        .block_on(tool.call(&serde_json::json!({
            "action": "add",
            "skill_name": "test_curate_skills_temp",
            "content": "This is a test skill content"
        })))
        .unwrap();
    assert!(add_res["success"].as_bool().unwrap());

    // 3. List skills and verify
    let list_res = rt
        .block_on(tool.call(&serde_json::json!({
            "action": "list"
        })))
        .unwrap();
    assert!(list_res["success"].as_bool().unwrap());
    let skills = list_res["skills"].as_array().unwrap();
    let found = skills
        .iter()
        .any(|s| s["name"].as_str().unwrap() == "test_curate_skills_temp");
    assert!(found);

    // 4. Delete skill
    let del_res = rt
        .block_on(tool.call(&serde_json::json!({
            "action": "delete",
            "skill_name": "test_curate_skills_temp"
        })))
        .unwrap();
    assert!(del_res["success"].as_bool().unwrap());
}
