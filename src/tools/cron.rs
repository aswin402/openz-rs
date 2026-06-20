use crate::tools::Tool;
use crate::cron::{load_jobs, save_jobs, CronJob};
use anyhow::{Result, anyhow};
use serde_json::Value;

pub struct ScheduleJobTool;

#[async_trait::async_trait]
impl Tool for ScheduleJobTool {
    fn name(&self) -> &str {
        "schedule_job"
    }

    fn description(&self) -> &str {
        "Schedule a new automated cron job or update an existing one. Schedules use format: 10s, 1m, 5m, 1h, 1d."
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
                    "description": "Frequency interval. Supported formats: s (seconds), m (minutes), h (hours), d (days). E.g. '30s', '5m', '12h'."
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
        let id = arguments.get("id").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'id' argument"))?;
        let schedule = arguments.get("schedule").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'schedule' argument"))?;
        let prompt = arguments.get("prompt").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'prompt' argument"))?;

        if crate::cron::calculate_next_run(schedule, None).is_none() {
            return Err(anyhow!("Invalid schedule format: {}. Use simple duration like '10s', '5m', '1h' OR standard Unix cron like '*/5 * * * *'", schedule));
        }

        let mut jobs = load_jobs()?;
        let mut found = false;

        for job in &mut jobs {
            if job.id == id {
                job.schedule = schedule.to_string();
                job.prompt = prompt.to_string();
                job.next_run = None; // Reset next run calculation
                found = true;
                break;
            }
        }

        if !found {
            jobs.push(CronJob {
                id: id.to_string(),
                schedule: schedule.to_string(),
                prompt: prompt.to_string(),
                enabled: true,
                last_run: None,
                next_run: None,
            });
        }

        save_jobs(&jobs)?;

        Ok(serde_json::json!({
            "status": "success",
            "message": format!("Job '{}' successfully scheduled/updated.", id)
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
                .collect()
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
        let id = arguments.get("id").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'id' argument"))?;

        let mut jobs = load_jobs()?;
        let original_len = jobs.len();
        
        jobs.retain(|j| j.id != id);

        if jobs.len() == original_len {
            return Err(anyhow!("Cron job with ID '{}' not found.", id));
        }

        save_jobs(&jobs)?;

        Ok(serde_json::json!({
            "status": "success",
            "message": format!("Job '{}' successfully removed.", id)
        }))
    }
}
