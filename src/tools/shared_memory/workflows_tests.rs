use super::*;

#[tokio::test]
async fn workflow_search_does_not_return_unrelated_active_workflows() {
    let name = format!("active_unrelated_workflow_{}", uuid::Uuid::new_v4());
    add_workflow_card(
        &name,
        vec!["send screenshot to telegram".to_string()],
        "Capture active window and send image through Telegram",
        json!([]),
        vec![],
        vec![],
        "normal",
        "active",
    )
    .await
    .unwrap();
    let matches = search_workflow_cards("cook pasta dinner", 5, true)
        .await
        .unwrap();
    assert!(matches.iter().all(|m| m.name != name));
    assert_eq!(delete_workflow(&name).await.unwrap(), 1);
}

#[test]
fn workflow_rank_score_ignores_single_generic_overlap() {
    let item = WorkflowCard {
        id: "id".to_string(),
        name: "test_web_tools_health".to_string(),
        triggers: vec!["test web tools health".to_string()],
        summary: "Verify browser search tooling".to_string(),
        steps: json!([]),
        preconditions: vec![],
        verification: vec![],
        risk: "normal".to_string(),
        status: "active".to_string(),
        success_count: 0,
        failure_count: 0,
        last_used: None,
        created_at: "now".to_string(),
        updated_at: "now".to_string(),
        score: 0.0,
    };

    assert_eq!(workflow_rank_score("why did workflow match", &item), 0.0);
}

#[tokio::test]
async fn workflow_memory_search_and_record_run() {
    let unique = uuid::Uuid::new_v4().to_string();
    let name = format!("screenshot_active_window_to_telegram_{unique}");
    let trigger = format!("send screenshot to telegram {unique}");
    let summary = format!(
        "Capture active window and send image through configured Telegram bot {unique}"
    );
    add_workflow_card(
        &name,
        vec![trigger.clone()],
        &summary,
        json!([{"tool":"exec_command","args":{"cmd":"curl -F bot_token=12345"}}]),
        vec!["Telegram configured".to_string()],
        vec!["Telegram API returns ok=true".to_string()],
        "normal",
        "active",
    )
    .await
    .unwrap();
    let matches = search_workflow_cards(&trigger, 5, true).await.unwrap();
    assert_eq!(
        matches.first().map(|m| m.name.as_str()),
        Some(name.as_str())
    );
    let updated = record_workflow_run(&name, "test", "send screenshot", true, None)
        .await
        .unwrap();
    assert_eq!(updated.success_count, 1);
    let stored = get_workflow_by_name(&name).await.unwrap().unwrap();
    assert!(!stored.steps.to_string().contains("12345"));
    assert_eq!(delete_workflow(&name).await.unwrap(), 1);
}
