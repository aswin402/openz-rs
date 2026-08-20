use crate::cron::{load_cron_run_records, load_jobs, CronJob, CronJobStatus, CronNotifyPolicy};
use crate::tools::Tool;
use anyhow::{anyhow, Result};
use serde_json::Value;

fn parse_notify_policy(arguments: &Value) -> Result<CronNotifyPolicy> {
    match arguments
        .get("notify_on")
        .and_then(|v| v.as_str())
        .unwrap_or("failure")
    {
        "never" => Ok(CronNotifyPolicy::Never),
        "failure" => Ok(CronNotifyPolicy::Failure),
        "always" => Ok(CronNotifyPolicy::Always),
        other => Err(anyhow!(
            "Invalid notify_on '{}'. Use never, failure, or always.",
            other
        )),
    }
}

fn job_id_arg(arguments: &Value) -> Result<&str> {
    arguments
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing 'id' argument"))
}

pub struct ScheduleJobTool;

#[async_trait::async_trait]
impl Tool for ScheduleJobTool {
    fn name(&self) -> &str {
        "schedule_job"
    }

    fn description(&self) -> &str {
        "Schedule a new automated cron job or update an existing one. Schedules support simple durations like 10s, 1m, 5m, 1h, 1d, local clock times like '18:00', and local-time Unix cron expressions like '0 18 * * *'."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "Unique identifier for this scheduled task (e.g. 'health_check', 'report_writer')"
                },
                "schedule": {
                    "type": "string",
                    "description": "When to run. Supported formats: simple durations like '30s', '5m', '12h', local clock times like '18:00', or standard 5-field local-time Unix cron like '0 18 * * *'."
                },
                "run_once": {
                    "type": "boolean",
                    "description": "When true, disable the job after its next execution. Use for one-time reminders and 'at HH:MM do X' tasks."
                },
                "prompt": {
                    "type": "string",
                    "description": "The prompt or goal for the AI agent to execute when the schedule triggers."
                },
                "quiet": {
                    "type": "boolean",
                    "description": "When true, normal cron start/success/log-saved messages are not injected into active TUI chat. Defaults to true."
                },
                "notify_on": {
                    "type": "string",
                    "enum": ["never", "failure", "always"],
                    "description": "When to notify chat for this job. Defaults to failure. Quiet jobs never notify."
                }
            },
            "required": ["id", "schedule", "prompt"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let id = job_id_arg(arguments)?;
        let schedule = arguments
            .get("schedule")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'schedule' argument"))?;
        let prompt = arguments
            .get("prompt")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'prompt' argument"))?;
        let run_once = arguments
            .get("run_once")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let quiet = arguments
            .get("quiet")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let notify_on = parse_notify_policy(arguments)?;
        let now = chrono::Utc::now().to_rfc3339();

        if crate::cron::calculate_next_run(schedule, None).is_none() {
            return Err(anyhow!("Invalid schedule format: {}. Use simple duration like '10s', '5m', '1h', local clock time like '18:00', or standard Unix cron like '*/5 * * * *'", schedule));
        }

        let mut found = false;
        let id_str = id.to_string();
        let schedule_str = schedule.to_string();
        let prompt_str = prompt.to_string();

        crate::cron::with_cron_jobs_mut(|jobs| {
            for job in jobs.iter_mut() {
                if job.id == id_str {
                    job.schedule = schedule_str.clone();
                    job.prompt = prompt_str.clone();
                    job.run_once = run_once;
                    job.next_run = None;
                    job.quiet = quiet;
                    job.notify_on = notify_on.clone();
                    job.status = if job.enabled {
                        CronJobStatus::Idle
                    } else {
                        CronJobStatus::Disabled
                    };
                    job.updated_at = Some(now.clone());
                    found = true;
                    break;
                }
            }

            if !found {
                let mut job = CronJob::new(id_str, schedule_str, prompt_str, true, run_once);
                job.quiet = quiet;
                job.notify_on = notify_on;
                job.created_at = Some(now.clone());
                job.updated_at = Some(now);
                jobs.push(job);
            }
        })?;

        Ok(serde_json::json!({
            "status": "success",
            "message": format!("Job '{}' successfully scheduled/updated.", id),
            "run_once": run_once,
            "quiet": quiet
        }))
    }
}

pub struct ListJobsTool;

#[async_trait::async_trait]
impl Tool for ListJobsTool {
    fn name(&self) -> &str {
        "list_jobs"
    }

    fn description(&self) -> &str {
        "List all scheduled cron jobs and their execution status."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn call(&self, _arguments: &Value) -> Result<Value> {
        let jobs = load_jobs()?;
        Ok(serde_json::Value::Array(
            jobs.into_iter()
                .filter_map(|j| serde_json::to_value(j).ok())
                .collect(),
        ))
    }
}

pub struct RemoveJobTool;

#[async_trait::async_trait]
impl Tool for RemoveJobTool {
    fn name(&self) -> &str {
        "remove_job"
    }

    fn description(&self) -> &str {
        "Remove a scheduled cron job by its identifier."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "Identifier of the scheduled cron job to remove."
                }
            },
            "required": ["id"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let id = job_id_arg(arguments)?;
        let id_str = id.to_string();
        let mut removed = false;

        crate::cron::with_cron_jobs_mut(|jobs| {
            let original_len = jobs.len();
            jobs.retain(|j| j.id != id_str);
            removed = jobs.len() < original_len;
        })?;

        if !removed {
            return Err(anyhow!("Cron job with ID '{}' not found.", id));
        }

        Ok(serde_json::json!({
            "status": "success",
            "message": format!("Job '{}' successfully removed.", id)
        }))
    }
}

pub struct GetJobTool;

#[async_trait::async_trait]
impl Tool for GetJobTool {
    fn name(&self) -> &str {
        "get_job"
    }

    fn description(&self) -> &str {
        "Get one scheduled cron job by its identifier."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "Identifier of the scheduled cron job to inspect."
                }
            },
            "required": ["id"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let id = job_id_arg(arguments)?;
        let job = load_jobs()?
            .into_iter()
            .find(|j| j.id == id)
            .ok_or_else(|| anyhow!("Cron job with ID '{}' not found.", id))?;
        Ok(serde_json::json!({
            "status": "success",
            "job": job
        }))
    }
}

pub struct PauseJobTool;

#[async_trait::async_trait]
impl Tool for PauseJobTool {
    fn name(&self) -> &str {
        "pause_job"
    }

    fn description(&self) -> &str {
        "Pause a scheduled cron job without deleting it."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "Identifier of the scheduled cron job to pause."
                }
            },
            "required": ["id"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let id = job_id_arg(arguments)?;
        let now = chrono::Utc::now().to_rfc3339();
        let mut found = false;

        crate::cron::with_cron_jobs_mut(|jobs| {
            if let Some(j) = jobs.iter_mut().find(|j| j.id == id) {
                j.enabled = false;
                j.status = CronJobStatus::Disabled;
                j.updated_at = Some(now);
                j.next_run = None;
                found = true;
            }
        })?;

        if !found {
            return Err(anyhow!("Cron job with ID '{}' not found.", id));
        }

        Ok(serde_json::json!({
            "status": "success",
            "message": format!("Job '{}' paused.", id)
        }))
    }
}

pub struct ResumeJobTool;

#[async_trait::async_trait]
impl Tool for ResumeJobTool {
    fn name(&self) -> &str {
        "resume_job"
    }

    fn description(&self) -> &str {
        "Resume a paused scheduled cron job."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "Identifier of the scheduled cron job to resume."
                }
            },
            "required": ["id"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let id = job_id_arg(arguments)?;
        let now = chrono::Utc::now().to_rfc3339();
        let mut found = false;

        crate::cron::with_cron_jobs_mut(|jobs| {
            if let Some(j) = jobs.iter_mut().find(|j| j.id == id) {
                j.enabled = true;
                j.status = CronJobStatus::Idle;
                j.updated_at = Some(now);
                j.next_run = None;
                found = true;
            }
        })?;

        if !found {
            return Err(anyhow!("Cron job with ID '{}' not found.", id));
        }

        Ok(serde_json::json!({
            "status": "success",
            "message": format!("Job '{}' resumed.", id)
        }))
    }
}

pub struct RunJobNowTool;

#[async_trait::async_trait]
impl Tool for RunJobNowTool {
    fn name(&self) -> &str {
        "run_job_now"
    }

    fn description(&self) -> &str {
        "Run a scheduled cron job immediately by its identifier."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "Identifier of the scheduled cron job to run now."
                }
            },
            "required": ["id"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let id = job_id_arg(arguments)?;
        let job = load_jobs()?
            .into_iter()
            .find(|j| j.id == id)
            .ok_or_else(|| anyhow!("Cron job with ID '{}' not found.", id))?;
        let config = crate::config::loader::load_config()?;
        let record = crate::cron::scheduler::run_single_job_now(&config, job).await?;
        Ok(serde_json::json!({
            "status": "success",
            "run": record
        }))
    }
}

pub struct GetJobLogsTool;

#[async_trait::async_trait]
impl Tool for GetJobLogsTool {
    fn name(&self) -> &str {
        "get_job_logs"
    }

    fn description(&self) -> &str {
        "Get structured cron run records, optionally filtered by job id."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "Optional identifier of the scheduled cron job whose logs should be returned."
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of run records to return. Defaults to 20."
                }
            }
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let id = arguments.get("id").and_then(|v| v.as_str());
        let limit = arguments
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(20) as usize;
        let runs = load_cron_run_records(id, limit)?;

        Ok(serde_json::json!({
            "status": "success",
            "runs": runs
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::loader::CONFIG_DIR_OVERRIDE;
    use crate::cron::{
        load_jobs_raw, save_jobs_raw, CronJob, CronJobStatus, CronNotifyPolicy, CronRunRecord,
    };

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

    #[tokio::test]
    async fn pause_and_resume_job_toggle_enabled_status() {
        let temp_dir =
            std::env::temp_dir().join(format!("openz_cron_tools_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        CONFIG_DIR_OVERRIDE
            .scope(temp_dir.clone(), async {
                save_jobs_raw(&[sample_job("daily")]).unwrap();

                let pause = PauseJobTool;
                let paused = pause
                    .call(&serde_json::json!({ "id": "daily" }))
                    .await
                    .unwrap();
                assert_eq!(paused["status"], "success");
                let jobs = load_jobs_raw().unwrap();
                assert!(!jobs[0].enabled);
                assert_eq!(jobs[0].status, CronJobStatus::Disabled);

                let resume = ResumeJobTool;
                let resumed = resume
                    .call(&serde_json::json!({ "id": "daily" }))
                    .await
                    .unwrap();
                assert_eq!(resumed["status"], "success");
                let jobs = load_jobs_raw().unwrap();
                assert!(jobs[0].enabled);
                assert_eq!(jobs[0].status, CronJobStatus::Idle);
                assert!(jobs[0].next_run.is_none());
            })
            .await;

        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[tokio::test]
    async fn run_job_now_rejects_unknown_job() {
        let temp_dir =
            std::env::temp_dir().join(format!("openz_cron_run_now_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        CONFIG_DIR_OVERRIDE
            .scope(temp_dir.clone(), async {
                save_jobs_raw(&[sample_job("daily")]).unwrap();
                let tool = RunJobNowTool;
                let err = tool
                    .call(&serde_json::json!({ "id": "missing" }))
                    .await
                    .unwrap_err()
                    .to_string();
                assert!(err.contains("Cron job with ID 'missing' not found"));
            })
            .await;

        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[tokio::test]
    async fn get_job_logs_returns_structured_runs() {
        let temp_dir = std::env::temp_dir().join(format!(
            "openz_cron_logs_tool_test_{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();

        CONFIG_DIR_OVERRIDE
            .scope(temp_dir.clone(), async {
                crate::cron::append_cron_run_record(&CronRunRecord {
                    run_id: "run-1".to_string(),
                    job_id: "daily".to_string(),
                    schedule: "10s".to_string(),
                    started_at: "2026-08-20T08:00:00Z".to_string(),
                    finished_at: Some("2026-08-20T08:00:01Z".to_string()),
                    status: CronJobStatus::Success,
                    log_path: Some("/tmp/daily.log".to_string()),
                    summary: Some("done".to_string()),
                    error: None,
                })
                .unwrap();

                let tool = GetJobLogsTool;
                let res = tool
                    .call(&serde_json::json!({ "id": "daily", "limit": 5 }))
                    .await
                    .unwrap();
                assert_eq!(res["status"], "success");
                assert_eq!(res["runs"].as_array().unwrap().len(), 1);
                assert_eq!(res["runs"][0]["run_id"], "run-1");
            })
            .await;

        let _ = std::fs::remove_dir_all(temp_dir);
    }
}
