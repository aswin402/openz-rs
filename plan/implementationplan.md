# OpenZ Task Lifecycle Manager Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a unified OpenZ lifecycle system that can inspect, track, stop, and automatically clean up OpenZ-owned browsers, servers, agents, subagents, MCP bridges, watchers, and background jobs.

**Architecture:** Add a central task registry that records resources with ownership, purpose, TTL, and cleanup policy. Route browser-backed search/research through a browser broker that tries Obscura headless first, Firefox headless second, and GSD/Chrome GUI last, while registering and cleaning resources through the task registry. Expose this through a native `manage_tasks` tool and later surface it in CLI/WebUI.

**Tech Stack:** Rust, Tokio, Serde JSON, existing `Tool` trait, existing `shutdown.rs` child-process registry, CDP helpers in `browser_common.rs`, `obscura_browser`, `firefox_browser`, `gsd_browser`, existing SearchXyz tools.

## Global Constraints

- Do not run full `cargo check`, `cargo test`, or `cargo build` unless explicitly requested; use targeted tests with `cargo test -j 2 --lib <test_name>`.
- Preserve user-owned external processes. Only auto-stop resources that OpenZ started or explicitly owns.
- Use approval/security flow before stopping unknown external processes.
- Prefer Obscura headless for browser automation, then Firefox headless, then GUI Chrome/GSD only as fallback.
- Every task must be independently testable.
- Keep changes scoped; no broad refactors of unrelated tools.
- Do not store credentials or tokens in task registry metadata.

---

## Target File Structure

- Create: `src/tools/task_manager.rs`
  - Owns the task/resource registry and implements `manage_tasks`.
  - Provides functions for registering, listing, stopping, and cleanup of resources.

- Modify: `src/shutdown.rs`
  - Reuse existing process registry but extend metadata and expose safe helpers for task manager.

- Create: `src/tools/browser_broker.rs`
  - Provides backend-neutral browser operations for render/eval/search extraction.
  - Implements fallback order: Obscura headless -> Firefox headless -> GSD/Chrome GUI.

- Modify: `src/tools/searchxyz/web.rs`
  - Replace direct `GsdBrowserTool` usage in `SearchXyzBrowserSearchTool` with `BrowserBroker`.
  - Keep rendered DOM extraction and static HTML fallback.

- Modify: `src/tools/browser_status.rs`
  - Add OpenZ-owned/external resource reporting from task registry.

- Modify: `src/cli/tools.rs`
  - Register `manage_tasks`.

- Modify: `src/channels/cli/mod.rs`, `src/channels/websocket.rs`, `src/channels/telegram.rs`
  - Re-route `/servers` and `/stop-server` to `manage_tasks` semantics or add `/tasks`.

- Optional later: WebUI files under existing webui source
  - Add a Running Tasks panel and browser health controls.

---

### Task 1: Define Task Registry Types

**Files:**
- Create: `src/tools/task_manager.rs`
- Modify: `src/tools/mod.rs`

**Interfaces:**
- Produces:
  - `TaskKind`
  - `TaskOwner`
  - `CleanupPolicy`
  - `ManagedTask`
  - `register_task(task: ManagedTask) -> u64`
  - `list_tasks() -> Vec<ManagedTask>`
  - `cleanup_expired_tasks() -> usize`

- [ ] **Step 1: Write failing tests**

Add this test module to `src/tools/task_manager.rs`:

```rust
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
}
```

- [ ] **Step 2: Run targeted test**

Run:

```bash
cargo test -j 2 --lib task_registry_lists_registered_openz_owned_task -- --nocapture
```

Expected: fail because `task_manager` module/types do not exist.

- [ ] **Step 3: Implement minimal registry**

Create `src/tools/task_manager.rs`:

```rust
use serde::{Deserialize, Serialize};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskOwner {
    OpenZ,
    External,
    User,
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
            started_at: now.clone(),
            last_used_at: now,
            ttl_secs: None,
        }
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

pub fn list_tasks() -> Vec<ManagedTask> {
    registry()
        .lock()
        .map(|guard| guard.clone())
        .unwrap_or_default()
}

pub fn cleanup_expired_tasks() -> usize {
    let now = chrono::Utc::now();
    let Ok(mut guard) = registry().lock() else {
        return 0;
    };
    let before = guard.len();
    guard.retain(|task| {
        if task.owner != TaskOwner::OpenZ {
            return true;
        }
        let Some(ttl_secs) = task.ttl_secs else {
            return true;
        };
        let Ok(last_used) = chrono::DateTime::parse_from_rfc3339(&task.last_used_at) else {
            return true;
        };
        now.signed_duration_since(last_used.with_timezone(&chrono::Utc))
            .num_seconds()
            < ttl_secs as i64
    });
    before.saturating_sub(guard.len())
}

#[cfg(test)]
fn clear_task_registry_for_tests() {
    if let Ok(mut guard) = registry().lock() {
        guard.clear();
    }
}
```

Modify `src/tools/mod.rs`:

```rust
pub mod task_manager;
```

- [ ] **Step 4: Run targeted tests**

Run:

```bash
cargo test -j 2 --lib task_registry_lists_registered_openz_owned_task -- --nocapture
cargo test -j 2 --lib cleanup_expired_tasks_removes_only_expired_openz_tasks -- --nocapture
```

Expected: both pass.

- [ ] **Step 5: Commit**

```bash
git add src/tools/task_manager.rs src/tools/mod.rs
git commit -m "feat: add task lifecycle registry"
```

---

### Task 2: Implement `manage_tasks` Native Tool

**Files:**
- Modify: `src/tools/task_manager.rs`
- Modify: `src/cli/tools.rs`

**Interfaces:**
- Consumes:
  - `list_tasks() -> Vec<ManagedTask>`
  - `cleanup_expired_tasks() -> usize`
- Produces:
  - `pub struct ManageTasksTool`
  - Tool actions: `list`, `cleanup`, `stop`

- [ ] **Step 1: Write failing tests**

Add to `src/tools/task_manager.rs` tests:

```rust
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
    let result = tool.call(&serde_json::json!({ "action": "list" })).await.unwrap();

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
    let result = tool.call(&serde_json::json!({ "action": "cleanup" })).await.unwrap();

    assert_eq!(result["status"], "success");
    assert_eq!(result["cleaned"], 1);
}
```

- [ ] **Step 2: Run targeted test**

```bash
cargo test -j 2 --lib manage_tasks_lists_registered_tasks -- --nocapture
```

Expected: fail because `ManageTasksTool` does not exist.

- [ ] **Step 3: Implement tool**

Append to `src/tools/task_manager.rs`:

```rust
use crate::tools::Tool;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};

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
                    "description": "Task id, kind, purpose, or 'all'. Required for stop."
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
                    "tasks": list_tasks(),
                }))
            }
            other => Err(anyhow!("Unsupported manage_tasks action: {other}")),
        }
    }
}
```

Add this helper:

```rust
pub fn stop_tasks(target: &str) -> usize {
    let Ok(mut guard) = registry().lock() else {
        return 0;
    };
    let normalized = target.trim().to_ascii_lowercase();
    let before = guard.len();
    guard.retain(|task| {
        if task.owner != TaskOwner::OpenZ {
            return true;
        }
        let matches = normalized == "all"
            || task.id.to_string() == normalized
            || format!("{:?}", task.kind).to_ascii_lowercase() == normalized
            || task.purpose.to_ascii_lowercase().contains(&normalized);
        !matches
    });
    before.saturating_sub(guard.len())
}
```

Register in `src/cli/tools.rs` inside core tool registration:

```rust
registry.register(Arc::new(crate::tools::task_manager::ManageTasksTool));
```

- [ ] **Step 4: Run targeted tests**

```bash
cargo test -j 2 --lib manage_tasks_lists_registered_tasks -- --nocapture
cargo test -j 2 --lib manage_tasks_cleanup_reports_count -- --nocapture
```

Expected: both pass.

- [ ] **Step 5: Commit**

```bash
git add src/tools/task_manager.rs src/cli/tools.rs
git commit -m "feat: expose manage_tasks tool"
```

---

### Task 3: Bridge Existing Child Process Registry Into Task Registry

**Files:**
- Modify: `src/shutdown.rs`
- Modify: `src/tools/task_manager.rs`
- Modify: `src/tools/shell.rs`
- Modify: `src/tools/firefox.rs`
- Modify: `src/tools/browser_common.rs`

**Interfaces:**
- Consumes:
  - `register_task(ManagedTask) -> u64`
- Produces:
  - `register_process_task(child, command, kind, purpose, cleanup_policy) -> u64`
  - `stop_process_task(task_id: u64) -> bool`

- [ ] **Step 1: Write failing test**

Add to `src/shutdown.rs` tests:

```rust
#[test]
fn registered_child_metadata_surfaces_as_openz_task() {
    let _ = stop_registered_child("all");
    crate::tools::task_manager::clear_task_registry_for_tests();

    let mut command = std::process::Command::new("sh");
    command.args(["-c", "sleep 30"]);
    #[cfg(unix)]
    unsafe {
        use std::os::unix::process::CommandExt;
        command.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    let child = command.spawn().expect("spawn sleep child");
    let id = register_child_group_with_metadata(child, "sleep 30", "dev_server");

    let tasks = crate::tools::task_manager::list_tasks();
    assert!(tasks.iter().any(|task| task.id == id && task.purpose == "dev_server"));

    assert_eq!(stop_registered_child(&id.to_string()).unwrap(), 1);
}
```

- [ ] **Step 2: Run targeted test**

```bash
cargo test -j 2 --lib registered_child_metadata_surfaces_as_openz_task -- --nocapture
```

Expected: fail because child registry does not mirror into task registry.

- [ ] **Step 3: Implement process task registration**

Update `src/shutdown.rs` registration functions to call:

```rust
crate::tools::task_manager::register_task(
    crate::tools::task_manager::ManagedTask::new(
        crate::tools::task_manager::TaskKind::Server,
        crate::tools::task_manager::TaskOwner::OpenZ,
        kind_string.clone(),
        crate::tools::task_manager::CleanupPolicy::Manual,
    )
    .with_process(command_string.clone(), child_id),
);
```

Add builder methods in `ManagedTask`:

```rust
pub fn with_process(mut self, command: String, pid: u32) -> Self {
    self.command = Some(command);
    self.pid = Some(pid);
    self
}

pub fn with_port(mut self, port: u16) -> Self {
    self.port = Some(port);
    self
}

pub fn with_ttl(mut self, ttl_secs: u64) -> Self {
    self.ttl_secs = Some(ttl_secs);
    self
}
```

- [ ] **Step 4: Run targeted tests**

```bash
cargo test -j 2 --lib registered_child_metadata_surfaces_as_openz_task -- --nocapture
cargo test -j 2 --lib registered_children_can_be_listed_and_stopped -- --nocapture
```

Expected: both pass.

- [ ] **Step 5: Commit**

```bash
git add src/shutdown.rs src/tools/task_manager.rs src/tools/shell.rs src/tools/firefox.rs src/tools/browser_common.rs
git commit -m "feat: track spawned processes as managed tasks"
```

---

### Task 4: Add Browser Broker With Backend Priority

**Files:**
- Create: `src/tools/browser_broker.rs`
- Modify: `src/tools/mod.rs`

**Interfaces:**
- Produces:
  - `BrowserBackendChoice`
  - `BrowserOperation`
  - `BrowserBrokerResult`
  - `render_with_browser_broker(url: &str, timeout_secs: u64) -> Result<BrowserBrokerResult>`
  - `eval_with_browser_broker(url: &str, script: &str, timeout_secs: u64) -> Result<BrowserBrokerResult>`

- [ ] **Step 1: Write failing tests**

Create tests in `src/tools/browser_broker.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_backend_priority_prefers_obscura_then_firefox_then_gsd() {
        assert_eq!(
            browser_backend_priority(),
            [
                BrowserBackendChoice::ObscuraHeadless,
                BrowserBackendChoice::FirefoxHeadless,
                BrowserBackendChoice::GsdChromeGui
            ]
        );
    }

    #[test]
    fn broker_result_records_backend_and_cleanup() {
        let result = BrowserBrokerResult {
            backend: BrowserBackendChoice::ObscuraHeadless,
            status: "success".to_string(),
            output: "content".to_string(),
            cleanup: "closed_tab".to_string(),
            fallbacks_tried: vec![BrowserBackendChoice::ObscuraHeadless],
        };

        assert_eq!(result.backend, BrowserBackendChoice::ObscuraHeadless);
        assert_eq!(result.cleanup, "closed_tab");
    }
}
```

- [ ] **Step 2: Run targeted test**

```bash
cargo test -j 2 --lib browser_backend_priority_prefers_obscura_then_firefox_then_gsd -- --nocapture
```

Expected: fail because `browser_broker` does not exist.

- [ ] **Step 3: Implement broker types**

Create `src/tools/browser_broker.rs`:

```rust
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserBackendChoice {
    ObscuraHeadless,
    FirefoxHeadless,
    GsdChromeGui,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserBrokerResult {
    pub backend: BrowserBackendChoice,
    pub status: String,
    pub output: String,
    pub cleanup: String,
    pub fallbacks_tried: Vec<BrowserBackendChoice>,
}

pub fn browser_backend_priority() -> [BrowserBackendChoice; 3] {
    [
        BrowserBackendChoice::ObscuraHeadless,
        BrowserBackendChoice::FirefoxHeadless,
        BrowserBackendChoice::GsdChromeGui,
    ]
}

pub async fn eval_with_browser_broker(
    url: &str,
    script: &str,
    timeout_secs: u64,
) -> Result<BrowserBrokerResult> {
    let mut errors = Vec::new();
    let mut tried = Vec::new();

    for backend in browser_backend_priority() {
        tried.push(backend);
        match eval_with_backend(backend, url, script, timeout_secs).await {
            Ok(output) => {
                return Ok(BrowserBrokerResult {
                    backend,
                    status: "success".to_string(),
                    output,
                    cleanup: cleanup_label(backend).to_string(),
                    fallbacks_tried: tried,
                });
            }
            Err(err) => errors.push(format!("{backend:?}: {err}")),
        }
    }

    Err(anyhow!("All browser backends failed: {}", errors.join("; ")))
}

async fn eval_with_backend(
    backend: BrowserBackendChoice,
    url: &str,
    script: &str,
    timeout_secs: u64,
) -> Result<String> {
    match backend {
        BrowserBackendChoice::ObscuraHeadless => eval_obscura(url, script, timeout_secs).await,
        BrowserBackendChoice::FirefoxHeadless => eval_firefox(url, script).await,
        BrowserBackendChoice::GsdChromeGui => eval_gsd(url, script).await,
    }
}

fn cleanup_label(backend: BrowserBackendChoice) -> &'static str {
    match backend {
        BrowserBackendChoice::ObscuraHeadless => "closed_tab",
        BrowserBackendChoice::FirefoxHeadless => "close_available",
        BrowserBackendChoice::GsdChromeGui => "left_user_visible_session",
    }
}
```

Add stubs for `eval_obscura`, `eval_firefox`, and `eval_gsd` in the same file using existing tools. The exact shape:

```rust
async fn eval_obscura(url: &str, script: &str, timeout_secs: u64) -> Result<String> {
    let tool = crate::tools::obscura::ObscuraBrowserTool::new();
    let value = tool.call(&serde_json::json!({
        "url": url,
        "action": "eval_js",
        "script": script,
        "timeout": timeout_secs
    })).await?;
    Ok(value["output"].as_str().unwrap_or("").to_string())
}
```

Use matching wrappers for Firefox and GSD.

Modify `src/tools/mod.rs`:

```rust
pub mod browser_broker;
```

- [ ] **Step 4: Run targeted tests**

```bash
cargo test -j 2 --lib browser_backend_priority_prefers_obscura_then_firefox_then_gsd -- --nocapture
cargo test -j 2 --lib broker_result_records_backend_and_cleanup -- --nocapture
```

Expected: both pass.

- [ ] **Step 5: Commit**

```bash
git add src/tools/browser_broker.rs src/tools/mod.rs
git commit -m "feat: add browser broker fallback order"
```

---

### Task 5: Route SearchXyz Browser Search Through Browser Broker

**Files:**
- Modify: `src/tools/searchxyz/web.rs`
- Modify: `src/tools/browser_broker.rs`

**Interfaces:**
- Consumes:
  - `eval_with_browser_broker(url, script, timeout_secs)`
  - existing `extract_rendered_browser_search_results`
  - existing `extract_browser_search_results`

- [ ] **Step 1: Write failing test**

Add a pure helper test in `src/tools/searchxyz/web.rs`:

```rust
#[test]
fn browser_search_response_includes_backend_and_cleanup_diagnostics() {
    let response = browser_search_response_json(
        "success",
        "bing",
        "openz rust",
        vec![serde_json::json!({
            "title": "OpenZ",
            "url": "https://example.com/openz",
            "engine": "bing",
            "source": "browser_search_rendered"
        })],
        1,
        0,
        "rendered_dom",
        Some("obscura_headless"),
        Some("closed_tab"),
        vec![],
    );

    assert_eq!(response["backend"], "obscura_headless");
    assert_eq!(response["cleanup"], "closed_tab");
    assert_eq!(response["extraction_strategy"], "rendered_dom");
}
```

- [ ] **Step 2: Run targeted test**

```bash
cargo test -j 2 --lib browser_search_response_includes_backend_and_cleanup_diagnostics -- --nocapture
```

Expected: fail because helper does not exist.

- [ ] **Step 3: Implement response helper and replace direct GSD call**

Add helper:

```rust
fn browser_search_response_json(
    status: &str,
    engine: &str,
    query: &str,
    results: Vec<Value>,
    rendered_result_count: usize,
    static_result_count: usize,
    extraction_strategy: &str,
    backend: Option<&str>,
    cleanup: Option<&str>,
    read_results: Vec<Value>,
) -> Value {
    json!({
        "status": status,
        "engine": engine,
        "query": query,
        "result_count": results.len(),
        "rendered_result_count": rendered_result_count,
        "static_result_count": static_result_count,
        "extraction_strategy": extraction_strategy,
        "backend": backend,
        "cleanup": cleanup,
        "results": results,
        "read_count": read_results.len(),
        "read_results": read_results,
    })
}
```

Replace current direct:

```rust
let browser = crate::tools::gsd_browser::GsdBrowserTool;
```

with:

```rust
let broker_result = crate::tools::browser_broker::eval_with_browser_broker(
    &search_url,
    &render_script,
    timeout_secs,
).await;
```

Use `broker_result.output` as the rendered payload. If broker fails, return structured `browser_error` with `fallbacks_tried` and error text.

- [ ] **Step 4: Run targeted tests**

```bash
cargo test -j 2 --lib browser_search_response_includes_backend_and_cleanup_diagnostics -- --nocapture
cargo test -j 2 --lib browser_search_extracts_rendered_dom_results -- --nocapture
```

Expected: both pass.

- [ ] **Step 5: Commit**

```bash
git add src/tools/searchxyz/web.rs src/tools/browser_broker.rs
git commit -m "feat: route browser search through headless broker"
```

---

### Task 6: Add Automatic Turn-End Cleanup Hook

**Files:**
- Modify: `src/agent/agent_loop.rs`
- Modify: `src/tools/task_manager.rs`

**Interfaces:**
- Consumes:
  - `cleanup_expired_tasks() -> usize`
  - task cleanup policies
- Produces:
  - `cleanup_turn_end_tasks() -> usize`

- [ ] **Step 1: Write failing test**

Add to `src/tools/task_manager.rs` tests:

```rust
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
```

- [ ] **Step 2: Run targeted test**

```bash
cargo test -j 2 --lib cleanup_turn_end_removes_only_openz_turn_end_tasks -- --nocapture
```

Expected: fail because `cleanup_turn_end_tasks` does not exist.

- [ ] **Step 3: Implement cleanup hook**

Add:

```rust
pub fn cleanup_turn_end_tasks() -> usize {
    let Ok(mut guard) = registry().lock() else {
        return 0;
    };
    let before = guard.len();
    guard.retain(|task| {
        !(task.owner == TaskOwner::OpenZ && task.cleanup_policy == CleanupPolicy::OnTurnEnd)
    });
    before.saturating_sub(guard.len())
}
```

In `agent_loop.rs`, call after each turn reaches Save/Respond completion:

```rust
let cleaned = crate::tools::task_manager::cleanup_turn_end_tasks();
if cleaned > 0 {
    tracing::debug!(cleaned, "Cleaned OpenZ turn-scoped tasks");
}
```

- [ ] **Step 4: Run targeted tests**

```bash
cargo test -j 2 --lib cleanup_turn_end_removes_only_openz_turn_end_tasks -- --nocapture
```

Expected: pass.

- [ ] **Step 5: Commit**

```bash
git add src/tools/task_manager.rs src/agent/agent_loop.rs
git commit -m "feat: clean turn-scoped tasks automatically"
```

---

### Task 7: Upgrade Browser Inspection and CLI Commands

**Files:**
- Modify: `src/tools/browser_status.rs`
- Modify: `src/channels/cli/mod.rs`
- Modify: `src/channels/websocket.rs`
- Modify: `src/channels/telegram.rs`

**Interfaces:**
- Consumes:
  - `list_tasks()`
  - `ManageTasksTool`
- Produces:
  - `/tasks`
  - `/stop-task <id|kind|all>`
  - browser inspection includes OpenZ-owned task list

- [ ] **Step 1: Write failing tests**

Add a helper test in `src/channels/mod.rs` or nearest command parsing module:

```rust
#[test]
fn task_command_aliases_are_detected() {
    assert!(is_task_command("/tasks"));
    assert!(is_task_command("/stop-task all"));
    assert!(!is_task_command("/tasked"));
}
```

- [ ] **Step 2: Run targeted test**

```bash
cargo test -j 2 --lib task_command_aliases_are_detected -- --nocapture
```

Expected: fail because `is_task_command` does not exist.

- [ ] **Step 3: Implement commands**

Add:

```rust
pub fn is_task_command(text: &str) -> bool {
    let first = text.split_whitespace().next().unwrap_or("");
    matches!(first, "/tasks" | "/stop-task")
}
```

Route:

```rust
/tasks -> manage_tasks { "action": "list" }
/stop-task all -> manage_tasks { "action": "stop", "target": "all" }
```

Keep `/servers` and `/stop-server` as aliases for compatibility.

- [ ] **Step 4: Run targeted tests**

```bash
cargo test -j 2 --lib task_command_aliases_are_detected -- --nocapture
```

Expected: pass.

- [ ] **Step 5: Commit**

```bash
git add src/tools/browser_status.rs src/channels/cli/mod.rs src/channels/websocket.rs src/channels/telegram.rs
git commit -m "feat: expose task lifecycle commands"
```

---

### Task 8: Improve Search/Fetch/Research Efficiency

**Files:**
- Modify: `src/tools/searchxyz/web.rs`
- Modify: `src/tools/web_search.rs`
- Modify: `src/agent/agent_loop/research_policy.rs`

**Interfaces:**
- Consumes:
  - Browser broker
  - rendered DOM extraction
  - SearchXyz native search
- Produces:
  - search flow diagnostics
  - smarter fallback policy

- [ ] **Step 1: Write failing test**

Add to `src/tools/web_search.rs` tests:

```rust
#[test]
fn browser_fallback_policy_prefers_headless_broker_after_native_failure() {
    let plan = browser_fallback_plan_for_policy(WebSearchPolicy::NativeThenBrowser);
    assert_eq!(plan.native_first, true);
    assert_eq!(plan.browser_backends, vec![
        "obscura_headless",
        "firefox_headless",
        "gsd_chrome_gui"
    ]);
    assert_eq!(plan.external_after_browser, false);
}
```

- [ ] **Step 2: Run targeted test**

```bash
cargo test -j 2 --lib browser_fallback_policy_prefers_headless_broker_after_native_failure -- --nocapture
```

Expected: fail because `browser_fallback_plan_for_policy` does not exist.

- [ ] **Step 3: Implement planning helper**

Add:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
struct BrowserFallbackPlan {
    native_first: bool,
    browser_backends: Vec<&'static str>,
    external_after_browser: bool,
}

fn browser_fallback_plan_for_policy(policy: WebSearchPolicy) -> BrowserFallbackPlan {
    BrowserFallbackPlan {
        native_first: policy.allows_native(),
        browser_backends: if policy.allows_browser() {
            vec!["obscura_headless", "firefox_headless", "gsd_chrome_gui"]
        } else {
            Vec::new()
        },
        external_after_browser: policy.allows_external(),
    }
}
```

Use the plan in diagnostics returned when all search routes fail.

- [ ] **Step 4: Run targeted test**

```bash
cargo test -j 2 --lib browser_fallback_policy_prefers_headless_broker_after_native_failure -- --nocapture
```

Expected: pass.

- [ ] **Step 5: Commit**

```bash
git add src/tools/searchxyz/web.rs src/tools/web_search.rs src/agent/agent_loop/research_policy.rs
git commit -m "feat: improve search fallback diagnostics"
```

---

## Manual Verification

- [ ] Start OpenZ agent and ask for a live web search.
- [ ] Confirm browser diagnostics show `backend: obscura_headless` when Obscura works.
- [ ] Temporarily make Obscura unavailable and confirm Firefox headless is tried.
- [ ] Temporarily make Firefox unavailable and confirm GSD/Chrome GUI is tried last.
- [ ] Run `manage_tasks {"action":"list"}` and confirm browser resources are registered.
- [ ] Finish a turn and confirm `OnTurnEnd` browser tasks are cleaned.
- [ ] Run `/tasks` in CLI, Telegram, and WebUI.
- [ ] Run `/stop-task all` and confirm OpenZ-owned resources stop.
- [ ] Confirm external/user-owned browsers are not killed without explicit stop approval.

## Release Checklist

- [ ] Update `CHANGELOG.md` with Task Lifecycle Manager and Browser Broker notes.
- [ ] Increment version by `0.0.1`.
- [ ] Run only focused tests for changed modules.
- [ ] Commit with `feat: add task lifecycle manager`.
- [ ] Push to GitHub after user approval.

