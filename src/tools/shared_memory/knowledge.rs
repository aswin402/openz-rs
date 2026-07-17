use crate::tools::Tool;
use anyhow::{anyhow, Result};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::db::{get_db_mutex, with_db};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceBookmark {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub uri: String,
    pub aliases: Vec<String>,
    pub summary: String,
    pub trust_score: f64,
    pub last_checked: Option<String>,
    pub stale_after_secs: i64,
    pub created_at: String,
    pub updated_at: String,
    pub use_count: i64,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchBrief {
    pub id: String,
    pub topic: String,
    pub summary: String,
    pub source_ids: Vec<String>,
    pub confidence: f64,
    pub stale_after_secs: i64,
    pub created_at: String,
    pub updated_at: String,
    pub use_count: i64,
    pub score: f64,
}

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn json_string_array(value: Option<&Value>) -> Vec<String> {
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

fn parse_string_array(raw: &str) -> Vec<String> {
    serde_json::from_str(raw).unwrap_or_default()
}

fn score_text(query: &str, fields: &[&str]) -> f64 {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return 0.0;
    }
    let terms: Vec<&str> = query.split_whitespace().filter(|s| s.len() > 1).collect();
    let joined = fields.join(" ").to_lowercase();
    let mut score = if joined.contains(&query) { 4.0 } else { 0.0 };
    for term in terms {
        if joined.contains(term) {
            score += 1.0;
        }
    }
    score
}

fn source_from_row(row: &rusqlite::Row<'_>, score: f64) -> rusqlite::Result<SourceBookmark> {
    let aliases: String = row.get(4)?;
    Ok(SourceBookmark {
        id: row.get(0)?,
        label: row.get(1)?,
        kind: row.get(2)?,
        uri: row.get(3)?,
        aliases: parse_string_array(&aliases),
        summary: row.get(5)?,
        trust_score: row.get(6)?,
        last_checked: row.get(7)?,
        stale_after_secs: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
        use_count: row.get(11)?,
        score,
    })
}

pub async fn add_source_bookmark(
    label: &str,
    kind: &str,
    uri: &str,
    aliases: Vec<String>,
    summary: &str,
    trust_score: f64,
    stale_after_secs: i64,
) -> Result<SourceBookmark> {
    if label.trim().is_empty() || uri.trim().is_empty() {
        return Err(anyhow!("label and uri are required"));
    }
    let id = uuid::Uuid::new_v4().to_string();
    let now = now_rfc3339();
    let aliases_json = serde_json::to_string(&aliases)?;
    {
        let _lock = get_db_mutex().lock().await;
        with_db(|conn| {
            conn.execute(
                "INSERT INTO source_bookmarks (id, label, kind, uri, aliases, summary, trust_score, stale_after_secs, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)
                 ON CONFLICT(uri) DO UPDATE SET label=excluded.label, kind=excluded.kind, aliases=excluded.aliases, summary=excluded.summary, trust_score=excluded.trust_score, stale_after_secs=excluded.stale_after_secs, updated_at=excluded.updated_at",
                params![id, label.trim(), kind.trim(), uri.trim(), aliases_json, summary.trim(), trust_score.clamp(0.0, 1.0), stale_after_secs.max(60), now],
            )?;
            Ok(())
        })?;
    }
    get_source_by_uri(uri)
        .await?
        .ok_or_else(|| anyhow!("source bookmark save failed"))
}

pub async fn get_source_by_uri(uri: &str) -> Result<Option<SourceBookmark>> {
    let _lock = get_db_mutex().lock().await;
    with_db(|conn| {
        conn.query_row(
            "SELECT id, label, kind, uri, aliases, summary, trust_score, last_checked, stale_after_secs, created_at, updated_at, use_count FROM source_bookmarks WHERE uri = ?1",
            params![uri], |row| source_from_row(row, 1.0),
        ).optional().map_err(Into::into)
    })
}

pub async fn get_source_by_id(id: &str) -> Result<Option<SourceBookmark>> {
    let _lock = get_db_mutex().lock().await;
    with_db(|conn| {
        conn.query_row(
            "SELECT id, label, kind, uri, aliases, summary, trust_score, last_checked, stale_after_secs, created_at, updated_at, use_count FROM source_bookmarks WHERE id = ?1",
            params![id], |row| source_from_row(row, 1.0),
        ).optional().map_err(Into::into)
    })
}

pub async fn search_source_bookmarks(query: &str, limit: usize) -> Result<Vec<SourceBookmark>> {
    let _lock = get_db_mutex().lock().await;
    let mut rows = with_db(|conn| {
        let mut stmt = conn.prepare("SELECT id, label, kind, uri, aliases, summary, trust_score, last_checked, stale_after_secs, created_at, updated_at, use_count FROM source_bookmarks ORDER BY trust_score DESC, use_count DESC, updated_at DESC LIMIT 1000")?;
        let mapped = stmt.query_map([], |row| source_from_row(row, 0.0))?;
        let mut out = Vec::new();
        for item in mapped {
            out.push(item?);
        }
        Ok(out)
    })?;
    for item in &mut rows {
        let aliases = item.aliases.join(" ");
        item.score = score_text(
            query,
            &[&item.label, &item.kind, &item.uri, &aliases, &item.summary],
        ) + item.trust_score;
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

pub async fn delete_source(id_or_uri: &str) -> Result<usize> {
    let _lock = get_db_mutex().lock().await;
    with_db(|conn| {
        Ok(conn.execute(
            "DELETE FROM source_bookmarks WHERE id = ?1 OR uri = ?1",
            params![id_or_uri],
        )?)
    })
}

pub async fn mark_source_checked(id_or_uri: &str) -> Result<usize> {
    let now = now_rfc3339();
    let _lock = get_db_mutex().lock().await;
    with_db(|conn| {
        Ok(conn.execute("UPDATE source_bookmarks SET last_checked = ?2, updated_at = ?2, use_count = use_count + 1 WHERE id = ?1 OR uri = ?1", params![id_or_uri, now])?)
    })
}

fn brief_from_row(row: &rusqlite::Row<'_>, score: f64) -> rusqlite::Result<ResearchBrief> {
    let source_ids: String = row.get(3)?;
    Ok(ResearchBrief {
        id: row.get(0)?,
        topic: row.get(1)?,
        summary: row.get(2)?,
        source_ids: parse_string_array(&source_ids),
        confidence: row.get(4)?,
        stale_after_secs: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
        use_count: row.get(8)?,
        score,
    })
}

pub async fn save_research_brief(
    topic: &str,
    summary: &str,
    source_ids: Vec<String>,
    confidence: f64,
    stale_after_secs: i64,
) -> Result<ResearchBrief> {
    if topic.trim().is_empty() || summary.trim().is_empty() {
        return Err(anyhow!("topic and summary are required"));
    }
    let id = uuid::Uuid::new_v4().to_string();
    let now = now_rfc3339();
    let source_ids_json = serde_json::to_string(&source_ids)?;
    {
        let _lock = get_db_mutex().lock().await;
        with_db(|conn| {
            conn.execute(
                "INSERT INTO research_briefs (id, topic, summary, source_ids, confidence, stale_after_secs, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
                 ON CONFLICT(topic) DO UPDATE SET summary=excluded.summary, source_ids=excluded.source_ids, confidence=excluded.confidence, stale_after_secs=excluded.stale_after_secs, updated_at=excluded.updated_at",
                params![id, topic.trim(), summary.trim(), source_ids_json, confidence.clamp(0.0, 1.0), stale_after_secs.max(60), now],
            )?;
            Ok(())
        })?;
    }
    search_research_briefs(topic, 1)
        .await?
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("research brief save failed"))
}

pub async fn search_research_briefs(query: &str, limit: usize) -> Result<Vec<ResearchBrief>> {
    let _lock = get_db_mutex().lock().await;
    let mut rows = with_db(|conn| {
        let mut stmt = conn.prepare("SELECT id, topic, summary, source_ids, confidence, stale_after_secs, created_at, updated_at, use_count FROM research_briefs ORDER BY confidence DESC, use_count DESC, updated_at DESC LIMIT 1000")?;
        let mapped = stmt.query_map([], |row| brief_from_row(row, 0.0))?;
        let mut out = Vec::new();
        for item in mapped {
            out.push(item?);
        }
        Ok(out)
    })?;
    for item in &mut rows {
        item.score = score_text(query, &[&item.topic, &item.summary]) + item.confidence;
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

pub async fn delete_research_brief(id_or_topic: &str) -> Result<usize> {
    let _lock = get_db_mutex().lock().await;
    with_db(|conn| {
        Ok(conn.execute(
            "DELETE FROM research_briefs WHERE id = ?1 OR topic = ?1",
            params![id_or_topic],
        )?)
    })
}

pub struct KnowledgeSourceTool;

#[async_trait::async_trait]
impl Tool for KnowledgeSourceTool {
    fn name(&self) -> &str {
        "knowledge_source"
    }
    fn description(&self) -> &str {
        "CRUD and search durable source bookmarks: URLs, repos, docs, local paths, social profiles, aliases, and summaries used for future research."
    }
    fn parameters(&self) -> Value {
        json!({"type":"object","properties":{"action":{"type":"string","enum":["add","search","get","delete","mark_checked"]},"label":{"type":"string"},"kind":{"type":"string"},"uri":{"type":"string"},"aliases":{"type":"array","items":{"type":"string"}},"summary":{"type":"string"},"trust_score":{"type":"number"},"stale_after_secs":{"type":"integer"},"query":{"type":"string"},"id":{"type":"string"},"limit":{"type":"integer"}},"required":["action"]})
    }
    async fn call(&self, arguments: &Value) -> Result<Value> {
        let action = arguments
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing action"))?;
        match action {
            "add" => Ok(
                json!({"status":"success","source":add_source_bookmark(arguments.get("label").and_then(|v| v.as_str()).ok_or_else(|| anyhow!("Missing label"))?, arguments.get("kind").and_then(|v| v.as_str()).unwrap_or("other"), arguments.get("uri").and_then(|v| v.as_str()).ok_or_else(|| anyhow!("Missing uri"))?, json_string_array(arguments.get("aliases")), arguments.get("summary").and_then(|v| v.as_str()).unwrap_or(""), arguments.get("trust_score").and_then(|v| v.as_f64()).unwrap_or(0.5), arguments.get("stale_after_secs").and_then(|v| v.as_i64()).unwrap_or(604800)).await?}),
            ),
            "search" => Ok(
                json!({"status":"success","matches":search_source_bookmarks(arguments.get("query").and_then(|v| v.as_str()).unwrap_or(""), arguments.get("limit").and_then(|v| v.as_u64()).unwrap_or(5) as usize).await?}),
            ),
            "get" => {
                let key = arguments
                    .get("id")
                    .or_else(|| arguments.get("uri"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing id or uri"))?;
                let item =
                    if key.starts_with("http") || key.starts_with('/') || key.starts_with('~') {
                        get_source_by_uri(key).await?
                    } else {
                        get_source_by_id(key).await?
                    };
                Ok(json!({"status":"success","source":item}))
            }
            "delete" => {
                let key = arguments
                    .get("id")
                    .or_else(|| arguments.get("uri"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing id or uri"))?;
                Ok(json!({"status":"success","deleted":delete_source(key).await?}))
            }
            "mark_checked" => {
                let key = arguments
                    .get("id")
                    .or_else(|| arguments.get("uri"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing id or uri"))?;
                Ok(json!({"status":"success","updated":mark_source_checked(key).await?}))
            }
            _ => Err(anyhow!("Invalid action")),
        }
    }
}

pub struct ResearchBriefTool;

#[async_trait::async_trait]
impl Tool for ResearchBriefTool {
    fn name(&self) -> &str {
        "research_brief"
    }
    fn description(&self) -> &str {
        "CRUD and search topic-level research briefs that summarize prior research and link to saved source bookmarks."
    }
    fn parameters(&self) -> Value {
        json!({"type":"object","properties":{"action":{"type":"string","enum":["save","search","delete"]},"topic":{"type":"string"},"summary":{"type":"string"},"source_ids":{"type":"array","items":{"type":"string"}},"confidence":{"type":"number"},"stale_after_secs":{"type":"integer"},"query":{"type":"string"},"id":{"type":"string"},"limit":{"type":"integer"}},"required":["action"]})
    }
    async fn call(&self, arguments: &Value) -> Result<Value> {
        let action = arguments
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing action"))?;
        match action {
            "save" => Ok(
                json!({"status":"success","brief":save_research_brief(arguments.get("topic").and_then(|v| v.as_str()).ok_or_else(|| anyhow!("Missing topic"))?, arguments.get("summary").and_then(|v| v.as_str()).ok_or_else(|| anyhow!("Missing summary"))?, json_string_array(arguments.get("source_ids")), arguments.get("confidence").and_then(|v| v.as_f64()).unwrap_or(0.5), arguments.get("stale_after_secs").and_then(|v| v.as_i64()).unwrap_or(86400)).await?}),
            ),
            "search" => Ok(
                json!({"status":"success","matches":search_research_briefs(arguments.get("query").and_then(|v| v.as_str()).unwrap_or(""), arguments.get("limit").and_then(|v| v.as_u64()).unwrap_or(5) as usize).await?}),
            ),
            "delete" => {
                let key = arguments
                    .get("id")
                    .or_else(|| arguments.get("topic"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing id or topic"))?;
                Ok(json!({"status":"success","deleted":delete_research_brief(key).await?}))
            }
            _ => Err(anyhow!("Invalid action")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn knowledge_source_crud_searches_aliases() {
        let item = add_source_bookmark(
            "Hermes Agent",
            "docs",
            &format!(
                "https://hermes-agent.nousresearch.com/docs/{}",
                uuid::Uuid::new_v4()
            ),
            vec!["hermes".to_string(), "nous hermes".to_string()],
            "Official Hermes Agent docs",
            0.95,
            86400,
        )
        .await
        .unwrap();
        let matches = search_source_bookmarks("whats hermes", 5).await.unwrap();
        assert!(matches.iter().any(|m| m.id == item.id));
        assert_eq!(delete_source(&item.id).await.unwrap(), 1);
    }
}
