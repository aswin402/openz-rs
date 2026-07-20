use crate::cron::{load_jobs, CronJob};
use crate::tools::Tool;
use anyhow::{anyhow, Result};
use serde_json::Value;

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
                }
            },
            "required": ["id", "schedule", "prompt"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let id = arguments
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'id' argument"))?;
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
                    job.next_run = None; // Reset next run calculation
                    found = true;
                    break;
                }
            }

            if !found {
                jobs.push(CronJob {
                    id: id_str,
                    schedule: schedule_str,
                    prompt: prompt_str,
                    enabled: true,
                    run_once,
                    last_run: None,
                    next_run: None,
                });
            }
        })?;

        Ok(serde_json::json!({
            "status": "success",
            "message": format!("Job '{}' successfully scheduled/updated.", id),
            "run_once": run_once
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
        let id = arguments
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'id' argument"))?;

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
