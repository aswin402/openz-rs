use crate::tools::Tool;
use anyhow::{anyhow, Result};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::auto_capture::canonical_research_topic;
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
    pub freshness: String,
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
    pub freshness: String,
    pub created_at: String,
    pub updated_at: String,
    pub use_count: i64,
    pub score: f64,
}

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn default_source_stale_after_secs(kind: &str, requested: i64) -> i64 {
    if requested > 0 {
        return requested.max(60);
    }
    match kind.trim().to_lowercase().as_str() {
        "news" | "social" | "feed" | "market" | "price" => 21_600,
        "api" | "status" => 3_600,
        "repo" | "docs" | "doc" | "website" => 604_800,
        "path" | "file" | "local" => 86_400,
        _ => 604_800,
    }
}

fn freshness_status(timestamp: Option<&str>, stale_after_secs: i64) -> &'static str {
    let Some(raw) = timestamp.map(str::trim).filter(|s| !s.is_empty()) else {
        return "unknown";
    };
    let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(raw) else {
        return "unknown";
    };
    let age = chrono::Utc::now()
        .signed_duration_since(parsed.with_timezone(&chrono::Utc))
        .num_seconds();
    if age > stale_after_secs.max(60) {
        "stale"
    } else {
        "fresh"
    }
}

fn source_kind_bonus(kind: &str) -> f64 {
    match kind.trim().to_lowercase().as_str() {
        "docs" | "doc" | "repo" | "path" | "file" | "local" => 1.0,
        "website" | "api" => 0.7,
        "news" | "social" | "feed" => 0.25,
        _ => 0.0,
    }
}

fn freshness_bonus(freshness: &str) -> f64 {
    match freshness {
        "fresh" => 0.7,
        "unknown" => -0.15,
        "stale" => -0.45,
        _ => 0.0,
    }
}

const STOP_WORDS: &[&str] = &[
    "the", "a", "an", "is", "it", "this", "that", "to", "in", "for", "of", "on", "and", "or", "be",
    "was", "are", "what", "how", "why", "who", "when", "where", "which", "with", "from", "about",
    "do", "does", "did", "can", "could", "will", "would", "should", "may", "might", "shall", "has",
    "have", "had", "been", "being", "not", "no", "nor", "but", "so", "if", "than", "too", "very",
    "just", "get", "got", "let", "make", "made", "use", "used", "using", "like", "also", "new",
    "set", "get", "say", "said", "see", "way", "part", "top", "own",
];

fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 2 && !STOP_WORDS.contains(w))
        .map(|w| w.to_string())
        .collect()
}

fn matching_term_count(query: &str, fields: &[&str]) -> (usize, usize) {
    if query.trim().is_empty() {
        return (0, 0);
    }
    let query_tokens = tokenize(query);
    if query_tokens.is_empty() {
        return (0, 0);
    }
    let total_terms = query_tokens.len();
    let doc_tokens: std::collections::BTreeSet<String> =
        tokenize(&fields.join(" ")).into_iter().collect();
    let matching = query_tokens
        .iter()
        .filter(|t| doc_tokens.contains(t.as_str()))
        .count();
    (matching, total_terms)
}

fn exact_field_match(query: &str, fields: &[&str]) -> bool {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return false;
    }
    fields
        .iter()
        .any(|field| field.trim().eq_ignore_ascii_case(&q))
}

/// Term-frequency based scoring: returns ratio of query tokens that appear as
/// whole words in the document fields, scaled to a 0..10 range.
fn tf_ratio_score(query: &str, fields: &[&str]) -> f64 {
    let (matching, total) = matching_term_count(query, fields);
    if total == 0 || matching == 0 {
        return 0.0;
    }
    let ratio = matching as f64 / total as f64;
    // Scale by sqrt(matching) so a single common-word match doesn't dominate,
    // but meaningful partial matches still score well.
    // Examples:
    //   1/2 match → 0.50 * 10.0 * sqrt(1) / sqrt(3) = 2.89
    //   2/2 match → 1.00 * 10.0 * sqrt(2) / sqrt(3) = 8.16
    //   1/3 match → 0.33 * 10.0 * sqrt(1) / sqrt(3) = 1.92
    //   2/3 match → 0.67 * 10.0 * sqrt(2) / sqrt(3) = 5.44
    //   3/3 match → 1.00 * 10.0 * sqrt(3) / sqrt(3) = 10.00
    ratio * 10.0 * (matching as f64).sqrt() / 3.0f64.sqrt()
}

fn source_rank_score(query: &str, item: &SourceBookmark) -> f64 {
    let aliases: Vec<&str> = item.aliases.iter().map(|s| s.as_str()).collect();
    let fields: Vec<&str> = [item.label.as_str(), item.kind.as_str(), item.uri.as_str()]
        .into_iter()
        .chain(aliases.iter().copied())
        .chain(std::iter::once(item.summary.as_str()))
        .collect();
    let (matching_terms, _) = matching_term_count(query, &fields);

    // If no tokens overlap, check URI substring + exact-field fallback
    if matching_terms == 0 {
        // Exact full-query match on label, URI, or alias → high confidence
        if exact_field_match(query, &[&item.label, &item.uri])
            || item
                .aliases
                .iter()
                .any(|alias| alias.trim().eq_ignore_ascii_case(query.trim()))
        {
            let mut s = 8.0;
            s += item.trust_score.clamp(0.0, 1.0) * 1.0;
            s += source_kind_bonus(&item.kind);
            s += freshness_bonus(&item.freshness);
            return s;
        }
        // URI substring match
        if !query.trim().is_empty()
            && item
                .uri
                .to_lowercase()
                .contains(&query.trim().to_lowercase())
        {
            let mut s = 3.0;
            s += item.trust_score.clamp(0.0, 1.0) * 1.0;
            s += source_kind_bonus(&item.kind);
            s += freshness_bonus(&item.freshness);
            return s;
        }
        return 0.0;
    }

    // At least 1 token matched — compute base from token overlap
    let mut score = tf_ratio_score(query, &fields);

    // Bonus: each query token that exact-matches label, URI, or an alias
    for term in tokenize(query) {
        if exact_field_match(&term, &[item.label.as_str(), item.uri.as_str()])
            || item
                .aliases
                .iter()
                .any(|a| a.trim().eq_ignore_ascii_case(&term))
        {
            score += 2.0;
        }
    }

    // URI substring bonus
    if !query.trim().is_empty()
        && item
            .uri
            .to_lowercase()
            .contains(&query.trim().to_lowercase())
    {
        score += 2.0;
    }

    score += item.trust_score.clamp(0.0, 1.0) * 1.0;
    score += source_kind_bonus(&item.kind);
    score += (item.use_count.max(0) as f64 + 1.0).ln() * 0.25;
    score += freshness_bonus(&item.freshness);
    score
}

fn brief_rank_score(query: &str, item: &ResearchBrief) -> f64 {
    let fields = [item.topic.as_str(), item.summary.as_str()];
    let (matching_terms, _) = matching_term_count(query, &fields);
    let mut score = if matching_terms >= 1 {
        tf_ratio_score(query, &fields)
    } else {
        0.0
    };
    if score <= 0.0 && !query.trim().is_empty() && matching_terms == 0 {
        return 0.0;
    }
    if item.topic.trim().eq_ignore_ascii_case(query.trim()) {
        score += 8.0;
    }
    score += item.confidence.clamp(0.0, 1.0) * 1.0;
    score += (item.use_count.max(0) as f64 + 1.0).ln() * 0.2;
    score += freshness_bonus(&item.freshness);
    score
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

fn source_from_row(row: &rusqlite::Row<'_>, score: f64) -> rusqlite::Result<SourceBookmark> {
    let aliases: String = row.get(4)?;
    let last_checked: Option<String> = row.get(7)?;
    let stale_after_secs: i64 = row.get(8)?;
    let freshness = freshness_status(last_checked.as_deref(), stale_after_secs).to_string();
    Ok(SourceBookmark {
        id: row.get(0)?,
        label: row.get(1)?,
        kind: row.get(2)?,
        uri: row.get(3)?,
        aliases: parse_string_array(&aliases),
        summary: row.get(5)?,
        trust_score: row.get(6)?,
        last_checked,
        stale_after_secs,
        freshness,
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
    let ttl = default_source_stale_after_secs(kind, stale_after_secs);
    let aliases_json = serde_json::to_string(&aliases)?;
    {
        let _lock = get_db_mutex().lock().await;
        with_db(|conn| {
            conn.execute(
                "INSERT INTO source_bookmarks (id, label, kind, uri, aliases, summary, trust_score, last_checked, stale_after_secs, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?9, ?8, ?9, ?9)
                 ON CONFLICT(uri) DO UPDATE SET label=excluded.label, kind=excluded.kind, aliases=excluded.aliases, summary=excluded.summary, trust_score=excluded.trust_score, last_checked=excluded.last_checked, stale_after_secs=excluded.stale_after_secs, updated_at=excluded.updated_at",
                params![id, label.trim(), kind.trim(), uri.trim(), aliases_json, summary.trim(), trust_score.clamp(0.0, 1.0), ttl, now],
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
        item.score = source_rank_score(query, item);
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
    let stale_after_secs: i64 = row.get(5)?;
    let updated_at: String = row.get(7)?;
    let freshness = freshness_status(Some(&updated_at), stale_after_secs).to_string();
    Ok(ResearchBrief {
        id: row.get(0)?,
        topic: row.get(1)?,
        summary: row.get(2)?,
        source_ids: parse_string_array(&source_ids),
        confidence: row.get(4)?,
        stale_after_secs,
        freshness,
        created_at: row.get(6)?,
        updated_at,
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
    let canonical_topic = canonical_research_topic(topic);
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
                params![id, canonical_topic, summary.trim(), source_ids_json, confidence.clamp(0.0, 1.0), stale_after_secs.max(60), now],
            )?;
            Ok(())
        })?;
    }
    search_research_briefs(&canonical_topic, 1)
        .await?
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("research brief save failed"))
}

pub async fn search_research_briefs(query: &str, limit: usize) -> Result<Vec<ResearchBrief>> {
    let canonical_query = canonical_research_topic(query);
    let search_query = if canonical_query.trim().is_empty() {
        query
    } else {
        &canonical_query
    };
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
        item.score = brief_rank_score(search_query, item);
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
    let canonical = canonical_research_topic(id_or_topic);
    let _lock = get_db_mutex().lock().await;
    with_db(|conn| {
        Ok(conn.execute(
            "DELETE FROM research_briefs WHERE id = ?1 OR topic = ?1 OR topic = ?2",
            params![id_or_topic, canonical],
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
        json!({"type":"object","properties":{"action":{"type":"string","enum":["save","search","delete"]},"topic":{"type":"string"},"goal":{"type":"string","description":"Alias for topic when saving a research brief."},"summary":{"type":"string"},"context":{"type":"string","description":"Alias for summary when saving a research brief."},"content":{"type":"string","description":"Alias for summary when saving a research brief."},"source_ids":{"type":"array","items":{"type":"string"}},"sources":{"type":"array","items":{"type":"string"}},"confidence":{"type":"number"},"stale_after_secs":{"type":"integer"},"query":{"type":"string"},"id":{"type":"string"},"limit":{"type":"integer"}},"required":[]})
    }
    async fn call(&self, arguments: &Value) -> Result<Value> {
        let inferred_action = if let Some(action) = arguments.get("action").and_then(|v| v.as_str())
        {
            action
        } else if arguments.get("summary").is_some()
            || arguments.get("context").is_some()
            || arguments.get("content").is_some()
        {
            "save"
        } else {
            "search"
        };
        match inferred_action {
            "save" => {
                let topic = arguments
                    .get("topic")
                    .or_else(|| arguments.get("goal"))
                    .or_else(|| arguments.get("query"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing topic/goal"))?;
                let summary = arguments
                    .get("summary")
                    .or_else(|| arguments.get("context"))
                    .or_else(|| arguments.get("content"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing summary/context"))?;
                let source_ids = if arguments.get("source_ids").is_some() {
                    json_string_array(arguments.get("source_ids"))
                } else {
                    json_string_array(arguments.get("sources"))
                };
                Ok(
                    json!({"status":"success","brief":save_research_brief(topic, summary, source_ids, arguments.get("confidence").and_then(|v| v.as_f64()).unwrap_or(0.5), arguments.get("stale_after_secs").and_then(|v| v.as_i64()).unwrap_or(86400)).await?}),
                )
            }
            "search" => Ok(
                json!({"status":"success","matches":search_research_briefs(arguments.get("query").or_else(|| arguments.get("topic")).or_else(|| arguments.get("goal")).and_then(|v| v.as_str()).unwrap_or(""), arguments.get("limit").and_then(|v| v.as_u64()).unwrap_or(5) as usize).await?}),
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
    async fn research_brief_accepts_goal_context_without_action() {
        let marker = uuid::Uuid::new_v4().to_string();
        let tool = ResearchBriefTool;
        let res = tool
            .call(&json!({
                "goal": format!("Hermes Agent comparison {}", marker),
                "context": "Hermes Agent is a Python/TypeScript self-improving AI agent framework."
            }))
            .await
            .unwrap();
        assert_eq!(res.get("status").and_then(|v| v.as_str()), Some("success"));
        let topic = format!("Hermes Agent comparison {}", marker);
        let canonical = canonical_research_topic(&topic);
        let matches = search_research_briefs(&topic, 1).await.unwrap();
        assert!(matches.iter().any(|m| m.topic == canonical));
        assert_eq!(delete_research_brief(&topic).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn research_brief_uses_canonical_topic_for_url_and_question() {
        let marker = uuid::Uuid::new_v4().to_string();
        let url_topic = format!("https://github.com/example/{marker}?utm_source=chatgpt.com");
        save_research_brief(&url_topic, "First summary", vec![], 0.7, 86400)
            .await
            .unwrap();
        save_research_brief(
            &format!("what is {marker}"),
            "Second summary",
            vec![],
            0.8,
            86400,
        )
        .await
        .unwrap();
        let url_matches = search_research_briefs(&url_topic, 5).await.unwrap();
        assert!(url_matches
            .iter()
            .any(|m| m.topic == format!("example/{marker}")));
        let question_matches = search_research_briefs(&format!("what is {marker}"), 5)
            .await
            .unwrap();
        assert!(question_matches.iter().any(|m| m.topic == marker));
        let _ = delete_research_brief(&url_topic).await;
        let _ = delete_research_brief(&marker).await;
    }

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
        assert_eq!(
            matches.iter().find(|m| m.id == item.id).unwrap().freshness,
            "fresh"
        );
        assert_eq!(delete_source(&item.id).await.unwrap(), 1);
    }

    #[test]
    fn freshness_status_marks_old_sources_stale() {
        let old = (chrono::Utc::now() - chrono::Duration::seconds(120)).to_rfc3339();
        assert_eq!(freshness_status(Some(&old), 60), "stale");
        assert_eq!(freshness_status(None, 60), "unknown");
    }

    #[tokio::test]
    async fn source_search_does_not_return_unrelated_trusted_sources() {
        let marker = uuid::Uuid::new_v4().to_string();
        let item = add_source_bookmark(
            &format!("Rust docs {}", marker),
            "docs",
            &format!("https://doc.rust-lang.org/{}", marker),
            vec![format!("rust {}", marker)],
            "Official Rust documentation",
            1.0,
            604800,
        )
        .await
        .unwrap();
        let matches = search_source_bookmarks("unrelated banana pasta", 5)
            .await
            .unwrap();
        assert!(matches.iter().all(|m| m.id != item.id));
        assert_eq!(delete_source(&item.id).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn source_ranking_prefers_exact_official_sources() {
        let marker = uuid::Uuid::new_v4().to_string();
        let low = add_source_bookmark(
            &format!("Hermes fan note {}", marker),
            "social",
            &format!("https://example.com/hermes-social-{}", marker),
            vec!["hermes".to_string()],
            "Unofficial community mention",
            0.2,
            21600,
        )
        .await
        .unwrap();
        let official = add_source_bookmark(
            &format!("Hermes Agent {}", marker),
            "docs",
            &format!(
                "https://hermes-agent.nousresearch.com/docs/official-{}",
                marker
            ),
            vec![format!("hermes official {}", marker)],
            "Official Hermes Agent docs",
            0.95,
            604800,
        )
        .await
        .unwrap();
        let matches = search_source_bookmarks(&format!("Hermes Agent {}", marker), 2)
            .await
            .unwrap();
        assert_eq!(
            matches.first().map(|m| m.id.as_str()),
            Some(official.id.as_str())
        );
        assert_eq!(delete_source(&low.id).await.unwrap(), 1);
        assert_eq!(delete_source(&official.id).await.unwrap(), 1);
    }
}
