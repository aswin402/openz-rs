use anyhow::Result;
use rusqlite::params;
use serde_json::{json, Value};

use super::db::{get_db_mutex, with_db};

pub async fn log_interaction(session_key: &str, query: &str) -> Result<String> {
    let id = uuid::Uuid::new_v4().to_string();
    let timestamp = chrono::Utc::now().to_rfc3339();
    let _lock = get_db_mutex().lock().await;
    with_db(|conn| {
        conn.execute(
            "INSERT INTO interaction_history (id, session_key, query, timestamp, success, errors)
             VALUES (?1, ?2, ?3, ?4, 1, NULL)",
            params![id, session_key, query, timestamp],
        )?;
        Ok(())
    })?;
    Ok(id)
}

pub async fn update_interaction_errors(id: &str, errors: &str) -> Result<()> {
    let _lock = get_db_mutex().lock().await;
    with_db(|conn| {
        conn.execute(
            "UPDATE interaction_history SET success = 0, errors = ?1 WHERE id = ?2",
            params![errors, id],
        )?;
        Ok(())
    })?;
    Ok(())
}

pub async fn get_recent_interactions(limit: usize) -> Result<Vec<Value>> {
    let _lock = get_db_mutex().lock().await;
    let results = with_db(|conn| {
        let mut stmt = conn.prepare("SELECT query, timestamp, success, errors FROM interaction_history ORDER BY timestamp DESC LIMIT ?1")?;
        let mapped = stmt.query_map(params![limit as i64], |row| {
            Ok(json!({
                "query": row.get::<_, String>(0)?,
                "timestamp": row.get::<_, String>(1)?,
                "success": row.get::<_, i64>(2)? == 1,
                "errors": row.get::<_, Option<String>>(3)?,
            }))
        })?;

        let mut collected = Vec::new();
        for item in mapped {
            collected.push(item?);
        }
        Ok(collected)
    })?;
    Ok(results)
}
