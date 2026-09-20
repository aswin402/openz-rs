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

fn job_id_arg(arguments: &Value) -> Result<String> {
    if let Some(s) = arguments.as_str() {
        let trimmed = s.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
    }
    for key in &[
        "id",
        "job_id",
        "jobId",
        "name",
        "job",
        "target",
        "job_name",
        "jobName",
    ] {
        if let Some(val) = arguments.get(key) {
            if let Some(s) = val.as_str() {
                let trimmed = s.trim();
                if !trimmed.is_empty() {
                    return Ok(trimmed.to_string());
                }
            } else if let Some(n) = val.as_i64() {
                return Ok(n.to_string());
            } else if let Some(u) = val.as_u64() {
                return Ok(u.to_string());
            }
        }
    }
    Err(anyhow!("Missing 'id' or 'job_id' argument"))
}

fn generate_job_id(prompt: &str) -> String {
    let slug: String = prompt
        .chars()
        .take(24)
        .filter_map(|c| {
            if c.is_ascii_alphanumeric() {
                Some(c.to_ascii_lowercase())
            } else if c == ' ' || c == '_' || c == '-' {
                Some('_')
            } else {
                None
            }
        })
        .collect();
    let cleaned_slug = slug.trim_matches('_');
    let short_uuid = &uuid::Uuid::new_v4().to_string()[..8];
    if cleaned_slug.is_empty() {
        format!("job_{}", short_uuid)
    } else {
        format!("{}_{}", cleaned_slug, short_uuid)
    }
}

fn extract_schedule_arg(arguments: &Value) -> Option<String> {
    for key in &[
        "schedule",
        "cron",
        "cron_expression",
        "cronExpression",
        "expression",
        "interval",
        "time",
        "when",
        "every",
        "duration",
        "frequency",
    ] {
        if let Some(val) = arguments.get(key) {
            if let Some(s) = val.as_str() {
                let trimmed = s.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            } else if let Some(n) = val.as_i64() {
                return Some(format!("{}s", n));
            } else if let Some(u) = val.as_u64() {
                return Some(format!("{}s", u));
            }
        }
    }
    None
}

fn extract_prompt_arg(arguments: &Value) -> Option<String> {
    for key in &[
        "prompt",
        "goal",
        "task",
        "command",
        "cmd",
        "message",
        "instruction",
        "description",
        "action",
        "job_prompt",
        "text",
    ] {
        if let Some(val) = arguments.get(key) {
            if let Some(s) = val.as_str() {
                let trimmed = s.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
    }
    None
}

fn get_system_crontab() -> Vec<Value> {
    #[cfg(unix)]
    {
        match std::process::Command::new("crontab").arg("-l").output() {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                stdout
                    .lines()
                    .map(str::trim)
                    .filter(|l| !l.is_empty())
                    .map(|line| {
                        serde_json::json!({
                            "raw": line,
                            "is_comment": line.starts_with('#')
                        })
                    })
                    .collect()
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                if stderr.is_empty() {
                    vec![]
                } else {
                    vec![serde_json::json!({ "info": stderr })]
                }
            }
            Err(err) => {
                vec![serde_json::json!({ "error": err.to_string() })]
            }
        }
    }
    #[cfg(not(unix))]
    {
        vec![serde_json::json!({ "info": "System crontab is only available on Unix-like operating systems." })]
    }
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
                    "description": "Unique identifier for this scheduled task (e.g. 'health_check', 'report_writer'). Optional: auto-generated if omitted."
                },
                "job_id": {
                    "type": "string",
                    "description": "Alias for 'id'."
                },
                "schedule": {
                    "type": "string",
                    "description": "When to run (aliases: cron, interval, time, every). Supported formats: simple durations like '30s', '5m', '12h', local clock times like '18:00', or standard 5-field local-time Unix cron like '0 18 * * *'."
                },
                "cron": {
                    "type": "string",
                    "description": "Alias for 'schedule' (Unix cron expression or duration like '5m', '1h')."
                },
                "prompt": {
                    "type": "string",
                    "description": "The prompt or goal for the AI agent to execute when the schedule triggers."
                },
                "command": {
                    "type": "string",
                    "description": "Alias for 'prompt' (command, task, or action to execute)."
                },
                "run_once": {
                    "type": "boolean",
                    "description": "When true, disable the job after its next execution. Use for one-time reminders and 'at HH:MM do X' tasks."
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
            "required": []
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let schedule = extract_schedule_arg(arguments)
            .ok_or_else(|| anyhow!("Missing 'schedule' or 'cron' argument. Provide schedule or cron (e.g. '10s', '5m', '1h', '0 18 * * *')."))?;
        let prompt = extract_prompt_arg(arguments)
            .ok_or_else(|| anyhow!("Missing 'prompt' or 'command' argument. Provide prompt, goal, or command to execute."))?;
        let id = job_id_arg(arguments).unwrap_or_else(|_| generate_job_id(&prompt));
        let run_once = arguments
            .get("run_once")
            .or_else(|| arguments.get("runOnce"))
            .and_then(|v| v.as_bool().or_else(|| v.as_str().and_then(|s| s.parse::<bool>().ok())))
            .unwrap_or(false);
        let quiet = arguments
            .get("quiet")
            .and_then(|v| v.as_bool().or_else(|| v.as_str().and_then(|s| s.parse::<bool>().ok())))
            .unwrap_or(true);
        let notify_on = parse_notify_policy(arguments)?;
        let now = chrono::Utc::now().to_rfc3339();

        if crate::cron::calculate_next_run(&schedule, None).is_none() {
            return Err(anyhow!(
                "Invalid schedule format: {}. Use simple duration like '10s', '5m', '1h', local clock time like '18:00', or standard Unix cron like '*/5 * * * *'",
                schedule
            ));
        }

        let mut found = false;
        let id_str = id.clone();
        let schedule_str = schedule.clone();
        let prompt_str = prompt.clone();

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
                let mut job = CronJob::new(id_str.clone(), schedule_str.clone(), prompt_str, true, run_once);
                job.quiet = quiet;
                job.notify_on = notify_on;
                job.created_at = Some(now.clone());
                job.updated_at = Some(now);
                jobs.push(job);
            }
        })?;

        Ok(serde_json::json!({
            "status": "success",
            "id": id_str,
            "job_id": id_str,
            "schedule": schedule_str,
            "message": format!("Job '{}' successfully scheduled/updated.", id_str),
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
        "List all scheduled cron jobs and their execution status. Optionally inspect system crontab."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "include_system_crontab": {
                    "type": "boolean",
                    "description": "When true, returns host OS system crontab entries alongside OpenZ internal scheduled cron jobs."
                }
            }
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let jobs = load_jobs()?;
        let job_values: Vec<Value> = jobs
            .into_iter()
            .filter_map(|j| {
                let mut val = serde_json::to_value(&j).ok()?;
                if let Some(obj) = val.as_object_mut() {
                    obj.insert("job_id".to_string(), serde_json::Value::String(j.id.clone()));
                }
                Some(val)
            })
            .collect();

        let include_system = arguments
            .get("include_system_crontab")
            .or_else(|| arguments.get("include_system"))
            .or_else(|| arguments.get("system_crontab"))
            .or_else(|| arguments.get("system"))
            .and_then(|v| v.as_bool().or_else(|| v.as_str().and_then(|s| s.parse::<bool>().ok())))
            .unwrap_or(false);

        if include_system {
            let system_entries = get_system_crontab();
            Ok(serde_json::json!({
                "status": "success",
                "openz_jobs": job_values,
                "system_crontab": system_entries
            }))
        } else {
            Ok(serde_json::Value::Array(job_values))
        }
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
                },
                "job_id": {
                    "type": "string",
                    "description": "Alias for 'id'."
                }
            }
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let id = job_id_arg(arguments)?;
        let id_str = id.clone();
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
            "id": id,
            "job_id": id,
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
                },
                "job_id": {
                    "type": "string",
                    "description": "Alias for 'id'."
                }
            }
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let id = job_id_arg(arguments)?;
        let job = load_jobs()?
            .into_iter()
            .find(|j| j.id == id)
            .ok_or_else(|| anyhow!("Cron job with ID '{}' not found.", id))?;
        let mut job_val = serde_json::to_value(&job)?;
        if let Some(obj) = job_val.as_object_mut() {
            obj.insert("job_id".to_string(), serde_json::Value::String(job.id.clone()));
        }
        Ok(serde_json::json!({
            "status": "success",
            "id": job.id,
            "job_id": job.id,
            "job": job_val
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
                },
                "job_id": {
                    "type": "string",
                    "description": "Alias for 'id'."
                }
            }
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
            "id": id,
            "job_id": id,
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
                },
                "job_id": {
                    "type": "string",
                    "description": "Alias for 'id'."
                }
            }
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
            "id": id,
            "job_id": id,
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
                },
                "job_id": {
                    "type": "string",
                    "description": "Alias for 'id'."
                }
            }
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
            "id": id,
            "job_id": id,
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
                "job_id": {
                    "type": "string",
                    "description": "Alias for 'id'."
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of run records to return. Defaults to 20."
                }
            }
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let id = if let Some(s) = arguments.as_str() {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        } else {
            job_id_arg(arguments).ok()
        };
        let limit = arguments
            .get("limit")
            .and_then(|v| {
                v.as_u64().or_else(|| {
                    v.as_str()
                        .and_then(|s| s.trim().parse::<u64>().ok())
                })
            })
            .unwrap_or(20) as usize;
        let runs = load_cron_run_records(id.as_deref(), limit)?;

        Ok(serde_json::json!({
            "status": "success",
            "id": id,
            "job_id": id,
            "runs": runs
        }))
    }
}

#[cfg(test)]
#[path = "cron_tests.rs"]
mod tests;

