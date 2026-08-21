use crate::tools::Tool;
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

static TASKS: OnceLock<Mutex<Vec<ManagedTask>>> = OnceLock::new();
static NEXT_TASK_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    Browser,
    Server,
    Agent,
    Subagent,
    Mcp,
    Watcher,
    BackgroundJob,
}

impl TaskKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Browser => "browser",
            Self::Server => "server",
            Self::Agent => "agent",
            Self::Subagent => "subagent",
            Self::Mcp => "mcp",
            Self::Watcher => "watcher",
            Self::BackgroundJob => "background_job",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskOwner {
    #[serde(rename = "openz")]
    OpenZ,
    External,
    User,
}

impl TaskOwner {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OpenZ => "openz",
            Self::External => "external",
            Self::User => "user",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CleanupPolicy {
    OnTurnEnd,
    OnSessionEnd,
    Manual,
    KeepAlive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedTask {
    pub id: u64,
    pub kind: TaskKind,
    pub owner: TaskOwner,
    pub purpose: String,
    pub cleanup_policy: CleanupPolicy,
    pub command: Option<String>,
    pub pid: Option<u32>,
    pub port: Option<u16>,
    pub session_id: Option<String>,
    pub process_registry_id: Option<u64>,
    pub started_at: String,
    pub last_used_at: String,
    pub ttl_secs: Option<u64>,
}

impl ManagedTask {
    pub fn new(
        kind: TaskKind,
        owner: TaskOwner,
        purpose: String,
        cleanup_policy: CleanupPolicy,
    ) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            id: 0,
            kind,
            owner,
            purpose,
            cleanup_policy,
            command: None,
            pid: None,
            port: None,
            session_id: None,
            process_registry_id: None,
            started_at: now.clone(),
            last_used_at: now,
            ttl_secs: None,
        }
    }

    pub fn with_process(mut self, command: impl Into<String>, pid: u32) -> Self {
        self.command = Some(command.into());
        self.pid = Some(pid);
        self
    }

    pub fn with_process_registry_id(mut self, process_registry_id: u64) -> Self {
        self.process_registry_id = Some(process_registry_id);
        self
    }

    pub fn with_port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }

    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    pub fn with_ttl(mut self, ttl_secs: u64) -> Self {
        self.ttl_secs = Some(ttl_secs);
        self
    }
}

fn registry() -> &'static Mutex<Vec<ManagedTask>> {
    TASKS.get_or_init(|| Mutex::new(Vec::new()))
}

pub fn register_task(mut task: ManagedTask) -> u64 {
    let id = NEXT_TASK_ID.fetch_add(1, Ordering::SeqCst);
    task.id = id;
    if let Ok(mut guard) = registry().lock() {
        guard.push(task);
    }
    id
}

pub fn register_or_replace_process_task(process_registry_id: u64, mut task: ManagedTask) -> u64 {
    task.process_registry_id = Some(process_registry_id);
    if let Ok(mut guard) = registry().lock() {
        if let Some(existing) = guard
            .iter_mut()
            .find(|entry| entry.process_registry_id == Some(process_registry_id))
        {
            let id = existing.id;
            task.id = id;
            *existing = task;
            return id;
        }
    }
    register_task(task)
}

pub fn list_tasks() -> Vec<ManagedTask> {
    cleanup_finished_process_tasks();
    registry()
        .lock()
        .map(|guard| guard.clone())
        .unwrap_or_default()
}

pub fn cleanup_expired_tasks() -> usize {
    let now = chrono::Utc::now();
    stop_matching_tasks(|task| {
        if task.owner != TaskOwner::OpenZ {
            return false;
        }
        let Some(ttl_secs) = task.ttl_secs else {
            return false;
        };
        let Ok(last_used) = chrono::DateTime::parse_from_rfc3339(&task.last_used_at) else {
            return false;
        };
        now.signed_duration_since(last_used.with_timezone(&chrono::Utc))
            .num_seconds()
            >= ttl_secs as i64
    })
}

pub fn cleanup_turn_end_tasks() -> usize {
    stop_matching_tasks(|task| {
        task.owner == TaskOwner::OpenZ && task.cleanup_policy == CleanupPolicy::OnTurnEnd
    })
}

pub fn cleanup_session_end_tasks() -> usize {
    stop_matching_tasks(|task| {
        task.owner == TaskOwner::OpenZ
            && matches!(
                task.cleanup_policy,
                CleanupPolicy::OnTurnEnd | CleanupPolicy::OnSessionEnd
            )
    })
}

pub fn stop_tasks(target: &str) -> usize {
    let normalized = target.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return 0;
    }
    stop_matching_tasks(|task| task_matches_target(task, &normalized))
}

fn task_matches_target(task: &ManagedTask, normalized_target: &str) -> bool {
    if task.owner != TaskOwner::OpenZ {
        return false;
    }
    normalized_target == "all"
        || task.id.to_string() == normalized_target
        || task
            .process_registry_id
            .map(|id| id.to_string() == normalized_target)
            .unwrap_or(false)
        || task.kind.as_str() == normalized_target
        || task
            .purpose
            .to_ascii_lowercase()
            .contains(normalized_target)
}

fn stop_matching_tasks(matches_task: impl Fn(&ManagedTask) -> bool) -> usize {
    let Ok(mut guard) = registry().lock() else {
        return 0;
    };
    let mut stopped = 0usize;
    let mut remaining = Vec::new();
    for task in std::mem::take(&mut *guard) {
        if matches_task(&task) {
            stop_task_resource(&task);
            stopped += 1;
        } else {
            remaining.push(task);
        }
    }
    *guard = remaining;
    stopped
}

fn stop_task_resource(task: &ManagedTask) {
    if let Some(process_registry_id) = task.process_registry_id {
        let _ = crate::shutdown::stop_registered_child_without_task_cleanup(
            &process_registry_id.to_string(),
        );
    }
}

fn cleanup_finished_process_tasks() {
    let active_process_ids = crate::shutdown::list_registered_children()
        .into_iter()
        .map(|child| child.id)
        .collect::<std::collections::HashSet<_>>();
    if let Ok(mut guard) = registry().lock() {
        guard.retain(|task| {
            task.process_registry_id
                .map(|id| active_process_ids.contains(&id))
                .unwrap_or(true)
        });
    }
}

pub struct ManageTasksTool;

#[async_trait::async_trait]
impl Tool for ManageTasksTool {
    fn name(&self) -> &str {
        "manage_tasks"
    }

    fn description(&self) -> &str {
        "List, stop, and clean up OpenZ-managed browsers, servers, agents, subagents, MCP bridges, watchers, and background jobs."
    }

    fn metadata(&self) -> crate::tools::ToolMetadata {
        let mut m = crate::tools::ToolMetadata::infer(self.name());
        m.domain = "system";
        m.risk = crate::tools::ToolRisk::Medium;
        m
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["list", "cleanup", "stop"],
                    "description": "Task management action."
                },
                "target": {
                    "type": "string",
                    "description": "Task id, process registry id, kind, purpose, or 'all'. Required for action=stop."
                }
            },
            "required": ["action"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let action = arguments
            .get("action")
            .and_then(|value| value.as_str())
            .ok_or_else(|| anyhow!("Missing 'action' parameter"))?;

        match action {
            "list" => Ok(json!({
                "status": "success",
                "tasks": list_tasks(),
            })),
            "cleanup" => Ok(json!({
                "status": "success",
                "cleaned": cleanup_expired_tasks(),
                "tasks": list_tasks(),
            })),
            "stop" => {
                let target = arguments
                    .get("target")
                    .and_then(|value| value.as_str())
                    .ok_or_else(|| anyhow!("Missing 'target' parameter for stop"))?;
                Ok(json!({
                    "status": "success",
                    "stopped": stop_tasks(target),
                    "target": target,
                    "tasks": list_tasks(),
                }))
            }
            other => Err(anyhow!("Unsupported manage_tasks action: {other}")),
        }
    }
}

#[cfg(test)]
pub fn clear_task_registry_for_tests() {
    if let Ok(mut guard) = registry().lock() {
        guard.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_registry_lists_registered_openz_owned_task() {
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
}
