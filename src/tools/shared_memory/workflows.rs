use crate::tools::Tool;
use anyhow::{anyhow, Result};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::db::{get_db_mutex, with_db};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowCard {
    pub id: String,
    pub name: String,
    pub triggers: Vec<String>,
    pub summary: String,
    pub steps: Value,
    pub preconditions: Vec<String>,
    pub verification: Vec<String>,
    pub risk: String,
    pub status: String,
    pub success_count: i64,
    pub failure_count: i64,
    pub last_used: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub score: f64,
}

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.trim().to_string()))
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn parse_array(raw: &str) -> Vec<String> {
    serde_json::from_str(raw).unwrap_or_default()
}

fn score_text(query: &str, fields: &[&str]) -> f64 {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return 0.0;
    }
    let joined = fields.join(" ").to_lowercase();
    let mut score = if joined.contains(&query) { 4.0 } else { 0.0 };
    for term in query.split_whitespace().filter(|s| s.len() > 1) {
        if joined.contains(term) {
            score += 1.0;
        }
    }
    score
}

fn normalized_terms(text: &str) -> std::collections::HashSet<String> {
    const STOP_WORDS: &[&str] = &[
        "about", "again", "also", "and", "any", "are", "can", "did", "for", "from", "how", "into",
        "now", "please", "that", "the", "then", "this", "to", "use", "using", "was", "were",
        "what", "when", "where", "with", "you", "your",
    ];
    text.split(|c: char| !c.is_alphanumeric())
        .map(|t| t.to_lowercase())
        .filter(|t| t.len() >= 3 && !STOP_WORDS.contains(&t.as_str()))
        .collect()
}

fn workflow_rank_score(query: &str, item: &WorkflowCard) -> f64 {
    let triggers = item.triggers.join(" ");
    let mut score = score_text(query, &[&item.name, &triggers, &item.summary]);
    if score <= 0.0 && !query.trim().is_empty() {
        return 0.0;
    }
    let q = query.trim().to_lowercase();
    if !q.is_empty() {
        if item.name.trim().eq_ignore_ascii_case(query.trim()) {
            score += 8.0;
        }
        if item
            .triggers
            .iter()
            .any(|trigger| trigger.trim().eq_ignore_ascii_case(query.trim()))
        {
            score += 7.0;
        }
        let query_terms = normalized_terms(query);
        let workflow_terms =
            normalized_terms(&format!("{} {} {}", item.name, triggers, item.summary));
        if !query_terms.is_empty() {
            let overlap = query_terms.intersection(&workflow_terms).count();
            score += overlap as f64 * 0.7;
        }
    }
    if item.status == "active" {
        score += 1.0;
    }
    score += (item.success_count.max(0) as f64 + 1.0).ln() * 0.8;
    score -= (item.failure_count.max(0) as f64) * 0.35;
    if item.risk.eq_ignore_ascii_case("high") {
        score -= 0.5;
    }
    score
}

fn string_contains_secret(raw: &str) -> bool {
    let lower = raw.to_lowercase();
    lower.contains("token=")
        || lower.contains("bot_token")
        || lower.contains("api_key")
        || lower.contains("authorization:")
        || lower.contains("api.telegram.org/bot")
}

fn redact_value(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (k, v) in map.iter_mut() {
                let key = k.to_lowercase();
                if key.contains("token")
                    || key.contains("secret")
                    || key.contains("password")
                    || key.contains("api_key")
                    || key.contains("authorization")
                    || v.as_str().is_some_and(string_contains_secret)
                {
                    *v = Value::String("********".to_string());
                } else {
                    redact_value(v);
                }
            }
        }
        Value::Array(arr) => arr.iter_mut().for_each(redact_value),
        Value::String(raw) if string_contains_secret(raw) => {
            *value = Value::String("********".to_string());
        }
        _ => {}
    }
}

fn workflow_from_row(row: &rusqlite::Row<'_>, score: f64) -> rusqlite::Result<WorkflowCard> {
    let triggers: String = row.get(2)?;
    let steps_json: String = row.get(4)?;
    let preconditions: String = row.get(5)?;
    let verification: String = row.get(6)?;
    Ok(WorkflowCard {
        id: row.get(0)?,
        name: row.get(1)?,
        triggers: parse_array(&triggers),
        summary: row.get(3)?,
        steps: serde_json::from_str(&steps_json).unwrap_or(Value::Array(vec![])),
        preconditions: parse_array(&preconditions),
        verification: parse_array(&verification),
        risk: row.get(7)?,
        status: row.get(8)?,
        success_count: row.get(9)?,
        failure_count: row.get(10)?,
        last_used: row.get(11)?,
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
        score,
    })
}

pub async fn add_workflow_card(
    name: &str,
    triggers: Vec<String>,
    summary: &str,
    mut steps: Value,
    preconditions: Vec<String>,
    verification: Vec<String>,
    risk: &str,
    status: &str,
) -> Result<WorkflowCard> {
    if name.trim().is_empty() || summary.trim().is_empty() {
        return Err(anyhow!("name and summary are required"));
    }
    redact_value(&mut steps);
    let id = uuid::Uuid::new_v4().to_string();
    let now = now_rfc3339();
    let triggers_json = serde_json::to_string(&triggers)?;
    let steps_json = serde_json::to_string(&steps)?;
    let preconditions_json = serde_json::to_string(&preconditions)?;
    let verification_json = serde_json::to_string(&verification)?;
    {
        let _lock = get_db_mutex().lock().await;
        with_db(|conn| {
            conn.execute(
                "INSERT INTO workflow_cards (id, name, triggers, summary, steps_json, preconditions, verification, risk, status, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)
                 ON CONFLICT(name) DO UPDATE SET triggers=excluded.triggers, summary=excluded.summary, steps_json=excluded.steps_json, preconditions=excluded.preconditions, verification=excluded.verification, risk=excluded.risk, status=excluded.status, updated_at=excluded.updated_at",
                params![id, name.trim(), triggers_json, summary.trim(), steps_json, preconditions_json, verification_json, risk, status, now],
            )?;
            Ok(())
        })?;
    }
    get_workflow_by_name(name)
        .await?
        .ok_or_else(|| anyhow!("workflow save failed"))
}

pub async fn get_workflow_by_name(name: &str) -> Result<Option<WorkflowCard>> {
    let _lock = get_db_mutex().lock().await;
    with_db(|conn| {
        conn.query_row(
            "SELECT id, name, triggers, summary, steps_json, preconditions, verification, risk, status, success_count, failure_count, last_used, created_at, updated_at FROM workflow_cards WHERE name = ?1 OR id = ?1",
            params![name], |row| workflow_from_row(row, 1.0),
        ).optional().map_err(Into::into)
    })
}

pub async fn search_workflow_cards(
    query: &str,
    limit: usize,
    active_only: bool,
) -> Result<Vec<WorkflowCard>> {
    let _lock = get_db_mutex().lock().await;
    let mut rows = with_db(|conn| {
        let sql = if active_only {
            "SELECT id, name, triggers, summary, steps_json, preconditions, verification, risk, status, success_count, failure_count, last_used, created_at, updated_at FROM workflow_cards WHERE status = 'active' ORDER BY success_count DESC, updated_at DESC LIMIT 1000"
        } else {
            "SELECT id, name, triggers, summary, steps_json, preconditions, verification, risk, status, success_count, failure_count, last_used, created_at, updated_at FROM workflow_cards ORDER BY success_count DESC, updated_at DESC LIMIT 1000"
        };
        let mut stmt = conn.prepare(sql)?;
        let mapped = stmt.query_map([], |row| workflow_from_row(row, 0.0))?;
        let mut out = Vec::new();
        for item in mapped {
            out.push(item?);
        }
        Ok(out)
    })?;
    for item in &mut rows {
        item.score = workflow_rank_score(query, item);
    }
    rows.retain(|item| item.score > 0.0 || query.trim().is_empty());
    rows.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    rows.truncate(limit.max(1));
    Ok(rows)
}

pub async fn record_workflow_run(
    name: &str,
    session_key: &str,
    task: &str,
    success: bool,
    error: Option<&str>,
) -> Result<WorkflowCard> {
    let workflow = get_workflow_by_name(name)
        .await?
        .ok_or_else(|| anyhow!("workflow not found"))?;
    let now = now_rfc3339();
    {
        let _lock = get_db_mutex().lock().await;
        with_db(|conn| {
            conn.execute(
                "INSERT INTO workflow_runs (id, workflow_id, session_key, task, success, error, timestamp) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![uuid::Uuid::new_v4().to_string(), workflow.id, session_key, task, if success { 1 } else { 0 }, error, now],
            )?;
            if success {
                conn.execute("UPDATE workflow_cards SET success_count = success_count + 1, last_used = ?2, updated_at = ?2 WHERE name = ?1", params![name, now])?;
            } else {
                conn.execute("UPDATE workflow_cards SET failure_count = failure_count + 1, last_used = ?2, updated_at = ?2 WHERE name = ?1", params![name, now])?;
            }
            Ok(())
        })?;
    }
    get_workflow_by_name(name)
        .await?
        .ok_or_else(|| anyhow!("workflow run update failed"))
}

pub async fn set_workflow_status(name: &str, status: &str) -> Result<usize> {
    let now = now_rfc3339();
    let _lock = get_db_mutex().lock().await;
    with_db(|conn| {
        Ok(conn.execute(
            "UPDATE workflow_cards SET status = ?2, updated_at = ?3 WHERE name = ?1 OR id = ?1",
            params![name, status, now],
        )?)
    })
}

pub async fn delete_workflow(name: &str) -> Result<usize> {
    let _lock = get_db_mutex().lock().await;
    with_db(|conn| {
        conn.execute(
            "DELETE FROM workflow_runs WHERE workflow_id IN (SELECT id FROM workflow_cards WHERE name = ?1 OR id = ?1)",
            params![name],
        )?;
        Ok(conn.execute(
            "DELETE FROM workflow_cards WHERE name = ?1 OR id = ?1",
            params![name],
        )?)
    })
}

pub struct WorkflowMemoryTool;

#[async_trait::async_trait]
impl Tool for WorkflowMemoryTool {
    fn name(&self) -> &str {
        "workflow_memory"
    }
    fn description(&self) -> &str {
        "CRUD/search reusable procedural workflow cards so OpenZ can repeat successful tool sequences like screenshot-to-Telegram without rediscovering them."
    }
    fn parameters(&self) -> Value {
        json!({"type":"object","properties":{"action":{"type":"string","enum":["add","search","get","delete","record_run","activate","deactivate"]},"name":{"type":"string"},"query":{"type":"string"},"triggers":{"type":"array","items":{"type":"string"}},"summary":{"type":"string"},"steps":{"type":"array","items":{"type":"object"}},"preconditions":{"type":"array","items":{"type":"string"}},"verification":{"type":"array","items":{"type":"string"}},"risk":{"type":"string"},"status":{"type":"string"},"session_key":{"type":"string"},"task":{"type":"string"},"success":{"type":"boolean"},"error":{"type":"string"},"limit":{"type":"integer"},"active_only":{"type":"boolean"}},"required":["action"]})
    }
    async fn call(&self, arguments: &Value) -> Result<Value> {
        let action = arguments
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing action"))?;
        match action {
            "add" => Ok(
                json!({"status":"success","workflow":add_workflow_card(arguments.get("name").and_then(|v| v.as_str()).ok_or_else(|| anyhow!("Missing name"))?, string_array(arguments.get("triggers")), arguments.get("summary").and_then(|v| v.as_str()).ok_or_else(|| anyhow!("Missing summary"))?, arguments.get("steps").cloned().unwrap_or_else(|| Value::Array(vec![])), string_array(arguments.get("preconditions")), string_array(arguments.get("verification")), arguments.get("risk").and_then(|v| v.as_str()).unwrap_or("normal"), arguments.get("status").and_then(|v| v.as_str()).unwrap_or("draft")).await?}),
            ),
            "search" => Ok(
                json!({"status":"success","matches":search_workflow_cards(arguments.get("query").and_then(|v| v.as_str()).unwrap_or(""), arguments.get("limit").and_then(|v| v.as_u64()).unwrap_or(5) as usize, arguments.get("active_only").and_then(|v| v.as_bool()).unwrap_or(false)).await?}),
            ),
            "get" => {
                let name = arguments
                    .get("name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing name"))?;
                Ok(json!({"status":"success","workflow":get_workflow_by_name(name).await?}))
            }
            "delete" => {
                let name = arguments
                    .get("name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing name"))?;
                Ok(json!({"status":"success","deleted":delete_workflow(name).await?}))
            }
            "activate" => {
                let name = arguments
                    .get("name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing name"))?;
                Ok(json!({"status":"success","updated":set_workflow_status(name, "active").await?}))
            }
            "deactivate" => {
                let name = arguments
                    .get("name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing name"))?;
                Ok(
                    json!({"status":"success","updated":set_workflow_status(name, "disabled").await?}),
                )
            }
            "record_run" => {
                let name = arguments
                    .get("name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing name"))?;
                Ok(
                    json!({"status":"success","workflow":record_workflow_run(name, arguments.get("session_key").and_then(|v| v.as_str()).unwrap_or("unknown"), arguments.get("task").and_then(|v| v.as_str()).unwrap_or(""), arguments.get("success").and_then(|v| v.as_bool()).unwrap_or(true), arguments.get("error").and_then(|v| v.as_str())).await?}),
                )
            }
            _ => Err(anyhow!("Invalid action")),
        }
    }
}

#[cfg(test)]
mod tests {
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
}
