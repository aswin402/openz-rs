use super::*;

static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn task_registry_lists_registered_openz_owned_task() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    clear_task_registry_for_tests();
    let id = register_task(ManagedTask::new(
        TaskKind::Browser,
        TaskOwner::OpenZ,
        "browser search".to_string(),
        CleanupPolicy::OnTurnEnd,
    ));

    let tasks = list_tasks();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].id, id);
    assert_eq!(tasks[0].kind, TaskKind::Browser);
    assert_eq!(tasks[0].owner, TaskOwner::OpenZ);
    assert_eq!(tasks[0].purpose, "browser search");
}

#[test]
fn cleanup_expired_tasks_removes_only_expired_openz_tasks() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    clear_task_registry_for_tests();
    let mut expired = ManagedTask::new(
        TaskKind::Browser,
        TaskOwner::OpenZ,
        "expired browser".to_string(),
        CleanupPolicy::OnTurnEnd,
    );
    expired.ttl_secs = Some(0);
    register_task(expired);

    register_task(ManagedTask::new(
        TaskKind::Server,
        TaskOwner::External,
        "user server".to_string(),
        CleanupPolicy::Manual,
    ));

    assert_eq!(cleanup_expired_tasks(), 1);
    let tasks = list_tasks();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].owner, TaskOwner::External);
}

#[test]
fn cleanup_turn_end_removes_only_openz_turn_end_tasks() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    clear_task_registry_for_tests();
    register_task(ManagedTask::new(
        TaskKind::Browser,
        TaskOwner::OpenZ,
        "search browser".to_string(),
        CleanupPolicy::OnTurnEnd,
    ));
    register_task(ManagedTask::new(
        TaskKind::Server,
        TaskOwner::OpenZ,
        "webui server".to_string(),
        CleanupPolicy::KeepAlive,
    ));

    assert_eq!(cleanup_turn_end_tasks(), 1);
    let tasks = list_tasks();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].cleanup_policy, CleanupPolicy::KeepAlive);
}

#[test]
fn stop_tasks_does_not_stop_external_tasks() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    clear_task_registry_for_tests();
    register_task(ManagedTask::new(
        TaskKind::Server,
        TaskOwner::External,
        "user server".to_string(),
        CleanupPolicy::Manual,
    ));

    assert_eq!(stop_tasks("all"), 0);
    assert_eq!(list_tasks().len(), 1);
}

#[tokio::test]
async fn manage_tasks_lists_registered_tasks() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    clear_task_registry_for_tests();
    register_task(ManagedTask::new(
        TaskKind::Browser,
        TaskOwner::OpenZ,
        "browser search".to_string(),
        CleanupPolicy::OnTurnEnd,
    ));

    let tool = ManageTasksTool;
    let result = tool.call(&json!({ "action": "list" })).await.unwrap();

    assert_eq!(result["status"], "success");
    assert_eq!(result["tasks"].as_array().unwrap().len(), 1);
    assert_eq!(result["tasks"][0]["kind"], "browser");
}

#[tokio::test]
async fn manage_tasks_cleanup_reports_count() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    clear_task_registry_for_tests();
    let mut task = ManagedTask::new(
        TaskKind::Browser,
        TaskOwner::OpenZ,
        "expired browser".to_string(),
        CleanupPolicy::OnTurnEnd,
    );
    task.ttl_secs = Some(0);
    register_task(task);

    let tool = ManageTasksTool;
    let result = tool.call(&json!({ "action": "cleanup" })).await.unwrap();

    assert_eq!(result["status"], "success");
    assert_eq!(result["cleaned"], 1);
}
