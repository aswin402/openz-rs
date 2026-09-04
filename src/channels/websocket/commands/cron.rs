//! Cron-related WebSocket response builders.

use serde_json::Value;

pub(crate) async fn cron_update_event(
    msg_type: &str,
    envelope: &Value,
    config: &crate::config::schema::Config,
    tools: Option<&crate::tools::ToolRegistry>,
) -> Value {
    let id = envelope
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if id.is_empty() {
        return super::super::protocol::event_error("Cron job id is required.");
    }

    let args = serde_json::json!({ "id": id });
    let result = match msg_type {
        "pause_cron_job" => {
            let tool = crate::tools::cron::PauseJobTool;
            crate::tools::Tool::call(&tool, &args).await
        }
        "resume_cron_job" => {
            let tool = crate::tools::cron::ResumeJobTool;
            crate::tools::Tool::call(&tool, &args).await
        }
        "delete_cron_job" => {
            let tool = crate::tools::cron::RemoveJobTool;
            crate::tools::Tool::call(&tool, &args).await
        }
        other => Err(anyhow::anyhow!(
            "Unsupported cron WebSocket command: {}",
            other
        )),
    };

    match result {
        Ok(result) => {
            let inventory = crate::core::inventory::build_runtime_inventory(config, tools);
            let status = match msg_type {
                "pause_cron_job" => "paused",
                "resume_cron_job" => "resumed",
                "delete_cron_job" => "deleted",
                _ => "updated",
            };
            super::super::protocol::cron_jobs_updated(status, id, result, inventory)
        }
        Err(err) => super::super::protocol::event_error(format!(
            "Failed to update cron job: {}",
            err
        )),
    }
}

pub(crate) fn cron_logs_event(envelope: &Value) -> Value {
    let id = envelope
        .get("id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|id| !id.is_empty());
    let limit = envelope
        .get("limit")
        .and_then(Value::as_u64)
        .unwrap_or(20)
        .min(200) as usize;
    match crate::cron::load_cron_run_records(id, limit) {
        Ok(runs) => super::super::protocol::cron_logs(id.map(str::to_string), runs),
        Err(err) => super::super::protocol::event_error(format!(
            "Failed to load cron logs: {}",
            err
        )),
    }
}
