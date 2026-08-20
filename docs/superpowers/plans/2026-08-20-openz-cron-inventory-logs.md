# OpenZ Cron Inventory And Quiet Logs Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make OpenZ cron jobs behave like managed background automations: quiet in normal TUI chat, visible through inventory tools, and inspectable through structured run logs.

**Architecture:** Keep the existing JSON-backed scheduler in `src/cron/`, but split responsibilities: `CronJob` stores inventory/status metadata, `scheduler.rs` updates run state and writes structured logs, and `tools/cron.rs` exposes CRUD/log tools to the agent. Normal cron lifecycle messages move from CLI notifications into tracing/log files; only scheduler errors may notify the user depending on job notification settings.

**Tech Stack:** Rust 2021, Tokio, serde/serde_json, chrono UTC timestamps, existing OpenZ `Tool` trait, existing `just test-one <test_name> openz` low-resource test command.

## Global Constraints

- Do not run full `cargo test`, full `cargo check`, full web build, or broad compiles on the laptop.
- Use focused tests only: `just test-one <test_name> openz`, plus `rustfmt` and `git diff --check`.
- Preserve current cron storage path: `~/.openz/cron_jobs.json` via `crate::config::loader::config_dir()`.
- Preserve current per-run text log directory: `~/.openz/cron_logs/`.
- Do not add new external services or background daemons.
- Do not show normal cron start/success/log-saved messages in the CLI/TUI conversation.
- Use UTC RFC3339 timestamps in persisted cron metadata and run logs.
- Keep backward compatibility for old `cron_jobs.json` entries that only have `id`, `schedule`, `prompt`, `enabled`, `run_once`, `last_run`, and `next_run`.
- Commit each task separately.

---

## File Structure

- `src/cron/mod.rs`
  - Extend `CronJob` with inventory fields and defaults.
  - Add `CronJobStatus`, `CronNotifyPolicy`, and `CronRunRecord` data types.
  - Add helpers for cron run index path, append/read run records, and status updates.

- `src/cron/scheduler.rs`
  - Stop sending normal lifecycle messages to CLI/TUI.
  - Update `CronJob` status metadata before and after each run.
  - Write text logs and append structured run records.
  - Keep error notifications only when policy allows.

- `src/tools/cron.rs`
  - Extend `schedule_job` with optional `quiet` and `notify_on` arguments.
  - Keep `list_jobs` as inventory read.
  - Add `pause_job`, `resume_job`, `run_job_now`, `get_job`, and `get_job_logs` tools.

- `src/cli/tools.rs`
  - Register the new cron tools.

- `src/tools/mod.rs`
  - Add static tool metadata entries for the new cron tools.
  - Ensure cron tools remain exposed under the model-facing tool limit.

- `src/tools/subagent/delegate_profile.rs`
  - Add new cron management tools to the `automation_agent` allowlist.

- `CHANGELOG.md`, `Cargo.toml`, `Cargo.lock`, `README.md`, `onpkg.json`
  - Bump version after implementation and focused verification.

---

### Task 1: Cron Inventory Data Model And Run Index

**Files:**
- Modify: `src/cron/mod.rs`

**Interfaces:**
- Produces: `CronJobStatus`, `CronNotifyPolicy`, `CronRunRecord`.
- Produces: `cron_runs_file_path() -> PathBuf`.
- Produces: `append_cron_run_record(record: &CronRunRecord) -> Result<()>`.
- Produces: `load_cron_run_records(job_id: Option<&str>, limit: usize) -> Result<Vec<CronRunRecord>>`.
- Produces: backward-compatible defaults for new `CronJob` fields.

- [ ] **Step 1: Add failing default-deserialization test**

Add this to `src/cron/mod.rs` tests:

```rust
#[test]
fn cron_job_deserializes_inventory_defaults() {
    let job: CronJob = serde_json::from_value(serde_json::json!({
        "id": "legacy",
        "schedule": "5m",
        "prompt": "do work",
        "enabled": true,
        "last_run": null,
        "next_run": null
    }))
    .unwrap();

    assert!(!job.run_once);
    assert_eq!(job.status, CronJobStatus::Idle);
    assert!(job.quiet);
    assert_eq!(job.notify_on, CronNotifyPolicy::Failure);
    assert_eq!(job.run_count, 0);
    assert_eq!(job.failure_count, 0);
    assert!(job.created_at.is_none());
    assert!(job.updated_at.is_none());
    assert!(job.last_started_at.is_none());
    assert!(job.last_finished_at.is_none());
    assert!(job.last_error.is_none());
    assert!(job.last_log_path.is_none());
}
```

- [ ] **Step 2: Add failing run-record append/read test**

Add this test to `src/cron/mod.rs` tests:

```rust
#[test]
fn cron_run_records_append_and_filter_by_job_id() {
    let temp_dir = std::env::temp_dir().join(format!(
        "openz_cron_runs_test_{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();

    crate::config::loader::CONFIG_DIR_OVERRIDE.scope_sync(temp_dir.clone(), || {
        let first = CronRunRecord {
            run_id: "run-a".to_string(),
            job_id: "job-a".to_string(),
            schedule: "10s".to_string(),
            started_at: "2026-08-20T08:00:00Z".to_string(),
            finished_at: Some("2026-08-20T08:00:01Z".to_string()),
            status: CronJobStatus::Success,
            log_path: Some("/tmp/job-a.log".to_string()),
            summary: Some("ok".to_string()),
            error: None,
        };
        let second = CronRunRecord {
            run_id: "run-b".to_string(),
            job_id: "job-b".to_string(),
            schedule: "1m".to_string(),
            started_at: "2026-08-20T08:01:00Z".to_string(),
            finished_at: Some("2026-08-20T08:01:01Z".to_string()),
            status: CronJobStatus::Failed,
            log_path: Some("/tmp/job-b.log".to_string()),
            summary: None,
            error: Some("boom".to_string()),
        };

        append_cron_run_record(&first).unwrap();
        append_cron_run_record(&second).unwrap();

        let only_a = load_cron_run_records(Some("job-a"), 10).unwrap();
        assert_eq!(only_a.len(), 1);
        assert_eq!(only_a[0].run_id, "run-a");
        assert_eq!(only_a[0].status, CronJobStatus::Success);

        let all = load_cron_run_records(None, 10).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].run_id, "run-a");
        assert_eq!(all[1].run_id, "run-b");
    });

    let _ = std::fs::remove_dir_all(temp_dir);
}
```

If `CONFIG_DIR_OVERRIDE.scope_sync` does not exist, implement this test with the existing async `CONFIG_DIR_OVERRIDE.scope(...).await` pattern used in `src/cron/scheduler.rs` tests and change the test to `#[tokio::test] async fn ...`.

- [ ] **Step 3: Run focused tests to verify failure**

Run:

```bash
just test-one cron_job_deserializes_inventory_defaults openz
just test-one cron_run_records_append_and_filter_by_job_id openz
```

Expected before implementation: compile failure because the new types/fields/functions do not exist.

- [ ] **Step 4: Implement inventory enums and defaults**

Add near `CronJob` in `src/cron/mod.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CronJobStatus {
    Idle,
    Running,
    Success,
    Failed,
    Disabled,
}

fn default_cron_job_status() -> CronJobStatus {
    CronJobStatus::Idle
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CronNotifyPolicy {
    Never,
    Failure,
    Always,
}

fn default_cron_notify_policy() -> CronNotifyPolicy {
    CronNotifyPolicy::Failure
}

fn default_quiet() -> bool {
    true
}
```

Extend `CronJob`:

```rust
#[serde(default = "default_cron_job_status")]
pub status: CronJobStatus,
#[serde(default = "default_quiet")]
pub quiet: bool,
#[serde(default = "default_cron_notify_policy")]
pub notify_on: CronNotifyPolicy,
#[serde(default)]
pub created_at: Option<String>,
#[serde(default)]
pub updated_at: Option<String>,
#[serde(default)]
pub last_started_at: Option<String>,
#[serde(default)]
pub last_finished_at: Option<String>,
#[serde(default)]
pub last_error: Option<String>,
#[serde(default)]
pub last_log_path: Option<String>,
#[serde(default)]
pub run_count: u64,
#[serde(default)]
pub failure_count: u64,
```

- [ ] **Step 5: Implement run record storage**

Add to `src/cron/mod.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CronRunRecord {
    pub run_id: String,
    pub job_id: String,
    pub schedule: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub status: CronJobStatus,
    pub log_path: Option<String>,
    pub summary: Option<String>,
    pub error: Option<String>,
}

pub fn cron_runs_file_path() -> PathBuf {
    crate::config::loader::config_dir().join("cron_runs.jsonl")
}

pub fn append_cron_run_record(record: &CronRunRecord) -> Result<()> {
    let path = cron_runs_file_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut line = serde_json::to_string(record)?;
    line.push('\n');
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("Failed to open cron run log at {:?}", path))?;
    file.write_all(line.as_bytes())?;
    Ok(())
}

pub fn load_cron_run_records(job_id: Option<&str>, limit: usize) -> Result<Vec<CronRunRecord>> {
    let path = cron_runs_file_path();
    if !path.exists() || limit == 0 {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read cron run log at {:?}", path))?;
    let mut records = Vec::new();
    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let record: CronRunRecord = serde_json::from_str(line)
            .with_context(|| format!("Failed to parse cron run record from {:?}", path))?;
        if job_id.map(|wanted| wanted == record.job_id).unwrap_or(true) {
            records.push(record);
        }
    }

    if records.len() > limit {
        Ok(records.split_off(records.len() - limit))
    } else {
        Ok(records)
    }
}
```

- [ ] **Step 6: Run focused tests**

Run:

```bash
just test-one cron_job_deserializes_inventory_defaults openz
just test-one cron_run_records_append_and_filter_by_job_id openz
```

Expected: PASS.

- [ ] **Step 7: Format and commit**

Run:

```bash
rustfmt --edition 2021 src/cron/mod.rs
git diff --check
git add src/cron/mod.rs
git commit -m "feat: add cron inventory metadata"
```

---

### Task 2: Quiet Scheduler And Run Metadata Updates

**Files:**
- Modify: `src/cron/scheduler.rs`
- Modify: `src/cron/mod.rs` only if Task 1 helpers need a small adjustment

**Interfaces:**
- Consumes: `CronJobStatus`, `CronNotifyPolicy`, `CronRunRecord`, `append_cron_run_record` from Task 1.
- Produces: scheduler behavior that records start/success/failure without normal TUI spam.
- Produces: `run_job_now(config: &Config, job: CronJob) -> Result<CronRunRecord>` or equivalent helper used by `run_job_now` tool later.

- [ ] **Step 1: Add failing scheduler metadata test**

In `src/cron/scheduler.rs` tests, update `test_tick_scheduler_updates_disk_immediately` job literals with new fields if needed by construction. Use defaults by deserializing JSON when practical, or fill fields explicitly:

```rust
status: crate::cron::CronJobStatus::Idle,
quiet: true,
notify_on: crate::cron::CronNotifyPolicy::Failure,
created_at: None,
updated_at: None,
last_started_at: None,
last_finished_at: None,
last_error: None,
last_log_path: None,
run_count: 0,
failure_count: 0,
```

Then add assertions after `tick_scheduler(&config).await.unwrap();`:

```rust
assert_eq!(j_once.status, crate::cron::CronJobStatus::Disabled);
assert_eq!(j_rec.status, crate::cron::CronJobStatus::Idle);
```

- [ ] **Step 2: Add focused notification-policy unit test**

Add near scheduler tests:

```rust
#[test]
fn cron_should_notify_policy_matches_quiet_defaults() {
    use crate::cron::{CronJobStatus, CronNotifyPolicy};

    assert!(!should_notify_cron(false, &CronNotifyPolicy::Never, &CronJobStatus::Success));
    assert!(!should_notify_cron(true, &CronNotifyPolicy::Always, &CronJobStatus::Success));
    assert!(should_notify_cron(false, &CronNotifyPolicy::Always, &CronJobStatus::Success));
    assert!(should_notify_cron(false, &CronNotifyPolicy::Failure, &CronJobStatus::Failed));
    assert!(!should_notify_cron(false, &CronNotifyPolicy::Failure, &CronJobStatus::Success));
}
```

- [ ] **Step 3: Run focused tests to verify failure**

Run:

```bash
just test-one cron_should_notify_policy_matches_quiet_defaults openz
just test-one test_tick_scheduler_updates_disk_immediately openz
```

Expected before implementation: compile failure because `should_notify_cron` and new scheduler status behavior do not exist.

- [ ] **Step 4: Implement notification policy helper**

Add in `src/cron/scheduler.rs`:

```rust
fn should_notify_cron(
    quiet: bool,
    notify_on: &crate::cron::CronNotifyPolicy,
    status: &crate::cron::CronJobStatus,
) -> bool {
    if quiet {
        return false;
    }
    match notify_on {
        crate::cron::CronNotifyPolicy::Never => false,
        crate::cron::CronNotifyPolicy::Failure => matches!(status, crate::cron::CronJobStatus::Failed),
        crate::cron::CronNotifyPolicy::Always => true,
    }
}
```

- [ ] **Step 5: Remove normal scheduler CLI notifications**

In the spawned job block inside `tick_scheduler`, delete or replace these normal notifications with tracing:

```rust
crate::channels::cli::send_notification(&format!(
    "⏰ Executing Cron Job: {} (schedule: {})",
    job_clone.id, job_clone.schedule
));
```

```rust
crate::channels::cli::send_notification(&format!(
    "⏰ Cron Job {} completed successfully.",
    job_clone.id
));
```

And in `run_job`, remove:

```rust
crate::channels::cli::send_notification(&format!("⏰ Log saved to {:?}", log_file));
```

Use tracing instead:

```rust
tracing::info!(job_id = %job.id, schedule = %job.schedule, "cron job started");
tracing::info!(job_id = %job.id, log_path = %log_file.display(), "cron job log saved");
tracing::info!(job_id = %job_clone.id, "cron job completed successfully");
```

- [ ] **Step 6: Implement run metadata updates**

Before spawning each due job, mark it running in `with_cron_jobs_mut`:

```rust
let started_at = Utc::now();
if let Some(j) = jobs.iter_mut().find(|j| j.id == job.id) {
    j.status = crate::cron::CronJobStatus::Running;
    j.last_started_at = Some(started_at.to_rfc3339());
    j.updated_at = Some(started_at.to_rfc3339());
    j.last_error = None;
}
```

After success:

```rust
j.status = if j.enabled {
    crate::cron::CronJobStatus::Idle
} else {
    crate::cron::CronJobStatus::Disabled
};
j.last_run = Some(completed_at.to_rfc3339());
j.last_finished_at = Some(completed_at.to_rfc3339());
j.updated_at = Some(completed_at.to_rfc3339());
j.run_count = j.run_count.saturating_add(1);
j.last_error = None;
j.last_log_path = run_record.log_path.clone();
```

After failure:

```rust
j.status = crate::cron::CronJobStatus::Failed;
j.last_run = Some(completed_at.to_rfc3339());
j.last_finished_at = Some(completed_at.to_rfc3339());
j.updated_at = Some(completed_at.to_rfc3339());
j.run_count = j.run_count.saturating_add(1);
j.failure_count = j.failure_count.saturating_add(1);
j.last_error = Some(e.to_string());
```

- [ ] **Step 7: Change `run_job` to return a run record**

Change signature:

```rust
async fn run_job(config: &Config, job: &CronJob) -> Result<crate::cron::CronRunRecord>
```

At the end, return:

```rust
Ok(crate::cron::CronRunRecord {
    run_id: format!("{}_{}", job.id, timestamp),
    job_id: job.id.clone(),
    schedule: job.schedule.clone(),
    started_at: started_at.to_rfc3339(),
    finished_at: Some(Utc::now().to_rfc3339()),
    status: crate::cron::CronJobStatus::Success,
    log_path: Some(log_file.to_string_lossy().to_string()),
    summary: Some(res.content.chars().take(500).collect()),
    error: None,
})
```

On success, call:

```rust
let run_record = run_job(&config_clone, &job_clone).await?;
crate::cron::append_cron_run_record(&run_record)?;
```

On failure, append a failed record with `error: Some(e.to_string())` and `log_path: None`.

- [ ] **Step 8: Run focused tests**

Run:

```bash
just test-one cron_should_notify_policy_matches_quiet_defaults openz
just test-one test_tick_scheduler_updates_disk_immediately openz
```

Expected: PASS.

- [ ] **Step 9: Format and commit**

Run:

```bash
rustfmt --edition 2021 src/cron/scheduler.rs src/cron/mod.rs
git diff --check
git add src/cron/scheduler.rs src/cron/mod.rs
git commit -m "fix: keep cron runs quiet in tui"
```

---

### Task 3: Cron CRUD And Logs Tools

**Files:**
- Modify: `src/tools/cron.rs`

**Interfaces:**
- Consumes: `load_jobs`, `with_cron_jobs_mut`, `load_cron_run_records`.
- Produces tools: `GetJobTool`, `PauseJobTool`, `ResumeJobTool`, `RunJobNowTool`, `GetJobLogsTool`.

- [ ] **Step 1: Add focused tool tests**

Add tests in `src/tools/cron.rs` tests module. If no test module exists, create one at bottom:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::loader::CONFIG_DIR_OVERRIDE;
    use crate::cron::{load_jobs_raw, save_jobs_raw, CronJob, CronJobStatus, CronNotifyPolicy};

    fn sample_job(id: &str) -> CronJob {
        CronJob {
            id: id.to_string(),
            schedule: "10s".to_string(),
            prompt: "say hi".to_string(),
            enabled: true,
            run_once: false,
            last_run: None,
            next_run: None,
            status: CronJobStatus::Idle,
            quiet: true,
            notify_on: CronNotifyPolicy::Failure,
            created_at: None,
            updated_at: None,
            last_started_at: None,
            last_finished_at: None,
            last_error: None,
            last_log_path: None,
            run_count: 0,
            failure_count: 0,
        }
    }
```

Add pause/resume test:

```rust
#[tokio::test]
async fn pause_and_resume_job_toggle_enabled_status() {
    let temp_dir = std::env::temp_dir().join(format!(
        "openz_cron_tools_test_{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();

    CONFIG_DIR_OVERRIDE.scope(temp_dir.clone(), async {
        save_jobs_raw(&[sample_job("daily")]).unwrap();

        let pause = PauseJobTool;
        let paused = pause.call(&serde_json::json!({ "id": "daily" })).await.unwrap();
        assert_eq!(paused["status"], "success");
        let jobs = load_jobs_raw().unwrap();
        assert!(!jobs[0].enabled);
        assert_eq!(jobs[0].status, CronJobStatus::Disabled);

        let resume = ResumeJobTool;
        let resumed = resume.call(&serde_json::json!({ "id": "daily" })).await.unwrap();
        assert_eq!(resumed["status"], "success");
        let jobs = load_jobs_raw().unwrap();
        assert!(jobs[0].enabled);
        assert_eq!(jobs[0].status, CronJobStatus::Idle);
        assert!(jobs[0].next_run.is_none());
    }).await;

    let _ = std::fs::remove_dir_all(temp_dir);
}
```

Add get logs test:

```rust
#[tokio::test]
async fn get_job_logs_returns_structured_runs() {
    let temp_dir = std::env::temp_dir().join(format!(
        "openz_cron_logs_tool_test_{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();

    CONFIG_DIR_OVERRIDE.scope(temp_dir.clone(), async {
        crate::cron::append_cron_run_record(&crate::cron::CronRunRecord {
            run_id: "run-1".to_string(),
            job_id: "daily".to_string(),
            schedule: "10s".to_string(),
            started_at: "2026-08-20T08:00:00Z".to_string(),
            finished_at: Some("2026-08-20T08:00:01Z".to_string()),
            status: CronJobStatus::Success,
            log_path: Some("/tmp/daily.log".to_string()),
            summary: Some("done".to_string()),
            error: None,
        }).unwrap();

        let tool = GetJobLogsTool;
        let res = tool.call(&serde_json::json!({ "id": "daily", "limit": 5 })).await.unwrap();
        assert_eq!(res["status"], "success");
        assert_eq!(res["runs"].as_array().unwrap().len(), 1);
        assert_eq!(res["runs"][0]["run_id"], "run-1");
    }).await;

    let _ = std::fs::remove_dir_all(temp_dir);
}
```

- [ ] **Step 2: Run focused tests to verify failure**

Run:

```bash
just test-one pause_and_resume_job_toggle_enabled_status openz
just test-one get_job_logs_returns_structured_runs openz
```

Expected before implementation: compile failure because tool structs do not exist.

- [ ] **Step 3: Extend `schedule_job` schema and creation/update**

Add optional properties:

```rust
"quiet": {
    "type": "boolean",
    "description": "When true, normal cron start/success/log-saved messages are not injected into active TUI chat. Defaults to true."
},
"notify_on": {
    "type": "string",
    "enum": ["never", "failure", "always"],
    "description": "When to notify chat for this job. Defaults to failure. Quiet jobs never notify."
}
```

Parse:

```rust
let quiet = arguments.get("quiet").and_then(|v| v.as_bool()).unwrap_or(true);
let notify_on = match arguments.get("notify_on").and_then(|v| v.as_str()).unwrap_or("failure") {
    "never" => crate::cron::CronNotifyPolicy::Never,
    "failure" => crate::cron::CronNotifyPolicy::Failure,
    "always" => crate::cron::CronNotifyPolicy::Always,
    other => return Err(anyhow!("Invalid notify_on '{}'. Use never, failure, or always.", other)),
};
let now = chrono::Utc::now().to_rfc3339();
```

On update:

```rust
job.quiet = quiet;
job.notify_on = notify_on.clone();
job.updated_at = Some(now.clone());
job.status = if job.enabled { crate::cron::CronJobStatus::Idle } else { crate::cron::CronJobStatus::Disabled };
```

On create, fill all new fields.

- [ ] **Step 4: Implement get/pause/resume/logs tools**

Add structs and `Tool` impls:

```rust
pub struct GetJobTool;
pub struct PauseJobTool;
pub struct ResumeJobTool;
pub struct RunJobNowTool;
pub struct GetJobLogsTool;
```

For pause:

```rust
j.enabled = false;
j.status = crate::cron::CronJobStatus::Disabled;
j.updated_at = Some(chrono::Utc::now().to_rfc3339());
j.next_run = None;
```

For resume:

```rust
j.enabled = true;
j.status = crate::cron::CronJobStatus::Idle;
j.updated_at = Some(chrono::Utc::now().to_rfc3339());
j.next_run = None;
```

For logs:

```rust
let id = arguments.get("id").and_then(|v| v.as_str());
let limit = arguments.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
let runs = crate::cron::load_cron_run_records(id, limit)?;
Ok(serde_json::json!({ "status": "success", "runs": runs }))
```

For `run_job_now`, implement the schema and return a conservative result for Task 3 if scheduler helper from Task 2 is not public yet:

```rust
return Err(anyhow!("run_job_now is registered after scheduler run-now helper is exposed in Task 4"));
```

Then Task 4 wires it fully. If implementing now is easy, call the helper from Task 2.

- [ ] **Step 5: Run focused tests**

Run:

```bash
just test-one pause_and_resume_job_toggle_enabled_status openz
just test-one get_job_logs_returns_structured_runs openz
```

Expected: PASS.

- [ ] **Step 6: Format and commit**

Run:

```bash
rustfmt --edition 2021 src/tools/cron.rs
git diff --check
git add src/tools/cron.rs
git commit -m "feat: add cron management tools"
```

---

### Task 4: Register Cron Tools And Exposure Metadata

**Files:**
- Modify: `src/cli/tools.rs`
- Modify: `src/tools/mod.rs`
- Modify: `src/tools/subagent/delegate_profile.rs`

**Interfaces:**
- Consumes tools from Task 3.
- Produces registered native tools and tool metadata.

- [ ] **Step 1: Add failing registration test**

In `src/cli/tools.rs` tests, add or extend the existing cron exposure test:

```rust
#[tokio::test]
async fn cron_management_tools_stay_exposed_under_tool_limit() {
    let registry = ToolRegistry::new();
    registry.register(Arc::new(crate::tools::cron::ScheduleJobTool));
    registry.register(Arc::new(crate::tools::cron::ListJobsTool));
    registry.register(Arc::new(crate::tools::cron::RemoveJobTool));
    registry.register(Arc::new(crate::tools::cron::PauseJobTool));
    registry.register(Arc::new(crate::tools::cron::ResumeJobTool));
    registry.register(Arc::new(crate::tools::cron::GetJobTool));
    registry.register(Arc::new(crate::tools::cron::GetJobLogsTool));

    let names = registry
        .to_openai_format_for_prompt("check all cron jobs and show logs for job daily")
        .into_iter()
        .filter_map(|tool| tool["function"]["name"].as_str().map(str::to_string))
        .collect::<Vec<_>>();

    assert!(names.contains(&"list_jobs".to_string()));
    assert!(names.contains(&"get_job_logs".to_string()));
    assert!(names.contains(&"pause_job".to_string()));
    assert!(names.contains(&"resume_job".to_string()));
}
```

- [ ] **Step 2: Run focused test to verify failure**

Run:

```bash
just test-one cron_management_tools_stay_exposed_under_tool_limit openz
```

Expected before implementation: compile failure or missing exposure metadata.

- [ ] **Step 3: Register tools**

In `src/cli/tools.rs`, update import:

```rust
use crate::tools::cron::{
    GetJobLogsTool, GetJobTool, ListJobsTool, PauseJobTool, RemoveJobTool, ResumeJobTool,
    RunJobNowTool, ScheduleJobTool,
};
```

Register near existing cron tools:

```rust
registry.register(std::sync::Arc::new(PauseJobTool));
registry.register(std::sync::Arc::new(ResumeJobTool));
registry.register(std::sync::Arc::new(GetJobTool));
registry.register(std::sync::Arc::new(GetJobLogsTool));
registry.register(std::sync::Arc::new(RunJobNowTool));
```

- [ ] **Step 4: Add tool metadata**

In `src/tools/mod.rs` static definitions, add entries for:

```rust
name: "pause_job"
name: "resume_job"
name: "get_job"
name: "get_job_logs"
name: "run_job_now"
```

Use domain `"cron"`, aliases such as `"pause cron job"`, `"cron logs"`, and usage hints:

```rust
when_to_use: "Use to inspect OpenZ-managed scheduled jobs and their logs before reporting automation status."
when_not_to_use: "Avoid shell crontab, systemctl, or filesystem guessing for OpenZ-managed cron jobs."
```

- [ ] **Step 5: Update automation agent allowlist**

In `src/tools/subagent/delegate_profile.rs`, add to `automation_agent` allowlist:

```rust
"pause_job",
"resume_job",
"get_job",
"get_job_logs",
"run_job_now",
```

- [ ] **Step 6: Run focused tests**

Run:

```bash
just test-one cron_management_tools_stay_exposed_under_tool_limit openz
just test-one subagent_allowlisted_tools_exist_in_registry openz
```

Expected: PASS.

- [ ] **Step 7: Format and commit**

Run:

```bash
rustfmt --edition 2021 src/cli/tools.rs src/tools/mod.rs src/tools/subagent/delegate_profile.rs
git diff --check
git add src/cli/tools.rs src/tools/mod.rs src/tools/subagent/delegate_profile.rs
git commit -m "feat: expose cron inventory tools"
```

---

### Task 5: Run-Now Wiring And Manual Log Retrieval

**Files:**
- Modify: `src/cron/scheduler.rs`
- Modify: `src/tools/cron.rs`

**Interfaces:**
- Produces: public scheduler helper for one immediate job execution.
- Consumes: `RunJobNowTool` from Task 3.

- [ ] **Step 1: Add focused run-now lookup test**

In `src/tools/cron.rs` tests, add:

```rust
#[tokio::test]
async fn run_job_now_rejects_unknown_job() {
    let temp_dir = std::env::temp_dir().join(format!(
        "openz_cron_run_now_test_{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();

    CONFIG_DIR_OVERRIDE.scope(temp_dir.clone(), async {
        save_jobs_raw(&[sample_job("daily")]).unwrap();
        let tool = RunJobNowTool;
        let err = tool
            .call(&serde_json::json!({ "id": "missing" }))
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("Cron job with ID 'missing' not found"));
    }).await;

    let _ = std::fs::remove_dir_all(temp_dir);
}
```

- [ ] **Step 2: Run focused test to verify failure**

Run:

```bash
just test-one run_job_now_rejects_unknown_job openz
```

Expected before implementation: FAIL if `RunJobNowTool` still has placeholder behavior.

- [ ] **Step 3: Expose run helper from scheduler**

In `src/cron/scheduler.rs`, add:

```rust
pub async fn run_single_job_now(
    config: &Config,
    job: CronJob,
) -> Result<crate::cron::CronRunRecord> {
    let started_at = Utc::now();
    crate::cron::with_cron_jobs_mut(|jobs| {
        if let Some(j) = jobs.iter_mut().find(|j| j.id == job.id) {
            j.status = crate::cron::CronJobStatus::Running;
            j.last_started_at = Some(started_at.to_rfc3339());
            j.updated_at = Some(started_at.to_rfc3339());
            j.last_error = None;
        }
    })?;

    match run_job(config, &job).await {
        Ok(record) => {
            crate::cron::append_cron_run_record(&record)?;
            let finished_at = Utc::now().to_rfc3339();
            crate::cron::with_cron_jobs_mut(|jobs| {
                if let Some(j) = jobs.iter_mut().find(|j| j.id == job.id) {
                    j.status = if j.enabled { crate::cron::CronJobStatus::Idle } else { crate::cron::CronJobStatus::Disabled };
                    j.last_run = Some(finished_at.clone());
                    j.last_finished_at = Some(finished_at.clone());
                    j.updated_at = Some(finished_at.clone());
                    j.run_count = j.run_count.saturating_add(1);
                    j.last_error = None;
                    j.last_log_path = record.log_path.clone();
                }
            })?;
            Ok(record)
        }
        Err(e) => {
            let finished_at = Utc::now().to_rfc3339();
            crate::cron::with_cron_jobs_mut(|jobs| {
                if let Some(j) = jobs.iter_mut().find(|j| j.id == job.id) {
                    j.status = crate::cron::CronJobStatus::Failed;
                    j.last_run = Some(finished_at.clone());
                    j.last_finished_at = Some(finished_at.clone());
                    j.updated_at = Some(finished_at.clone());
                    j.run_count = j.run_count.saturating_add(1);
                    j.failure_count = j.failure_count.saturating_add(1);
                    j.last_error = Some(e.to_string());
                }
            })?;
            Err(e)
        }
    }
}
```

If this duplicates Task 2 logic, extract a small private helper `finish_job_run_metadata(...)` in `scheduler.rs` and use it in both paths.

- [ ] **Step 4: Wire `RunJobNowTool`**

In `src/tools/cron.rs`, `RunJobNowTool::call`:

```rust
let id = arguments
    .get("id")
    .and_then(|v| v.as_str())
    .ok_or_else(|| anyhow!("Missing 'id' argument"))?;
let jobs = load_jobs()?;
let job = jobs
    .into_iter()
    .find(|j| j.id == id)
    .ok_or_else(|| anyhow!("Cron job with ID '{}' not found.", id))?;
let config = crate::config::loader::load_config()?;
let record = crate::cron::scheduler::run_single_job_now(&config, job).await?;
Ok(serde_json::json!({ "status": "success", "run": record }))
```

Use the correct existing config loader function name if it differs; find it in `src/config/loader.rs` before implementation.

- [ ] **Step 5: Run focused test**

Run:

```bash
just test-one run_job_now_rejects_unknown_job openz
```

Expected: PASS.

Do not run a live `run_job_now` success test because it would invoke an LLM provider and may cost/network/fail.

- [ ] **Step 6: Format and commit**

Run:

```bash
rustfmt --edition 2021 src/cron/scheduler.rs src/tools/cron.rs
git diff --check
git add src/cron/scheduler.rs src/tools/cron.rs
git commit -m "feat: run cron jobs on demand"
```

---

### Task 6: Version, Changelog, And Focused Verification

**Files:**
- Modify: `CHANGELOG.md`
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `README.md`
- Modify: `onpkg.json`

**Interfaces:**
- Consumes all previous tasks.
- Produces release version `0.0.140` if current version is `0.0.139`.

- [ ] **Step 1: Run focused verification only**

Run these focused tests only:

```bash
just test-one cron_job_deserializes_inventory_defaults openz
just test-one cron_run_records_append_and_filter_by_job_id openz
just test-one cron_should_notify_policy_matches_quiet_defaults openz
just test-one test_tick_scheduler_updates_disk_immediately openz
just test-one pause_and_resume_job_toggle_enabled_status openz
just test-one get_job_logs_returns_structured_runs openz
just test-one cron_management_tools_stay_exposed_under_tool_limit openz
just test-one subagent_allowlisted_tools_exist_in_registry openz
just test-one run_job_now_rejects_unknown_job openz
```

If the laptop lags, stop after the current command completes and report the remaining commands.

- [ ] **Step 2: Static checks**

Run:

```bash
git diff --check
rg -n "0\.0\.139|0\.0\.140|Latest Release|version-" Cargo.toml Cargo.lock README.md onpkg.json CHANGELOG.md
```

- [ ] **Step 3: Bump version**

If current version is `0.0.139`, replace it with `0.0.140` in:

```text
Cargo.toml
Cargo.lock
README.md
onpkg.json
CHANGELOG.md
```

Add changelog entry at top:

```markdown
### v0.0.140 (Latest Release)
**Cron Inventory & Quiet Background Runs:**
- **Fix:** Cron jobs no longer inject normal start/success/log-saved messages into active CLI/TUI chat.
- **Feature:** Added cron inventory metadata including status, run counts, failure counts, timestamps, last error, and last log path.
- **Feature:** Added structured cron run history in `cron_runs.jsonl` plus `get_job_logs` for agent-readable run inspection.
- **Feature:** Added cron management tools for get, pause, resume, run-now, and logs.
- **Docs:** Documented the cron UX hardening plan and release behavior.
- **Chore:** Bumped version to `v0.0.140`.
```

Remove `(Latest Release)` from `v0.0.139`.

- [ ] **Step 4: Final status and commit**

Run:

```bash
git status --short --branch
git diff --stat
git add CHANGELOG.md Cargo.toml Cargo.lock README.md onpkg.json
git commit -m "chore: release openz v0.0.140"
```

- [ ] **Step 5: Push**

Run:

```bash
git push origin main
```

Expected:

```text
main -> main
```

---

## Manual Acceptance Checks

After implementation, manually verify in TUI/WebUI:

- [ ] Schedule a job every `10s`; normal job start/success/log-saved lines do not appear in active chat.
- [ ] `list_jobs` shows the job with `status`, `next_run`, `run_count`, and `last_log_path`.
- [ ] `get_job_logs { "id": "job_id" }` returns recent structured runs.
- [ ] `pause_job` stops future runs.
- [ ] `resume_job` re-enables the job and recalculates `next_run`.
- [ ] `remove_job` deletes the job.

## Open Questions For Execution

- `run_job_now` success path invokes the real agent/provider, so automated tests should only cover validation/error paths unless the user explicitly approves a live-provider smoke test.
- Existing `cron_jobs.json` is backward compatible through serde defaults; no migration command is needed.
- Future WebUI work can add a Cron panel, but this plan keeps WebUI out of scope and makes the data/tools ready first.
