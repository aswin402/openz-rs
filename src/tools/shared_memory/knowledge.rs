use crate::tools::Tool;
use anyhow::{anyhow, Result};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::auto_capture::canonical_research_topic;
use crate::memory::with_shared_db as with_db;
use super::db::get_db_mutex;

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

use super::now_rfc3339;

fn default_source_stale_after_secs(kind: &str, requested: i64) -> i64 {
    match requested > 0 {
        true => requested.max(60),
        false => match kind.trim().to_lowercase().as_str() {
            "news" | "social" | "feed" | "market" | "price" => 21_600,
            "api" | "status" => 3_600,
            "repo" | "docs" | "doc" | "website" => 604_800,
            "path" | "file" | "local" => 86_400,
            _ => 604_800,
        },
    }
}

fn freshness_status(timestamp: Option<&str>, stale_after_secs: i64) -> &'static str {
    let raw = match timestamp.map(str::trim).filter(|s| !s.is_empty()) {
        Some(raw) => raw,
        None => return "unknown",
    };
    let parsed = match chrono::DateTime::parse_from_rfc3339(raw) {
        Ok(parsed) => parsed,
        Err(_) => return "unknown",
    };
    let age = chrono::Utc::now()
        .signed_duration_since(parsed.with_timezone(&chrono::Utc))
        .num_seconds();
    match age > stale_after_secs.max(60) {
        true => "stale",
        false => "fresh",
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

pub fn display_source_label(label: &str, uri: &str) -> String {
    let derived = source_label_from_uri(uri);
    let current = label.trim();
    match current.is_empty() || is_low_quality_source_label(current, uri) {
        true => derived.unwrap_or_else(|| uri.chars().take(80).collect()),
        false => current.to_string(),
    }
}

pub(crate) fn source_label_from_uri(uri: &str) -> Option<String> {
    reqwest::Url::parse(uri).ok().and_then(|parsed| {
        let host = parsed.host_str()?.trim_start_matches("www.");
        let parts = parsed
            .path_segments()
            .map(|segments| segments.collect::<Vec<_>>())
            .unwrap_or_default();
        match (host, parts.len() >= 2) {
            ("github.com", true) => {
                let repo = format!("{}/{}", parts[0], parts[1]);
                return match parts.get(2).copied() {
                    Some("issues") => parts
                        .get(3)
                        .map(|num| format!("{repo} issue #{num}"))
                        .or_else(|| Some(format!("{repo} issues"))),
                    Some("pull") => parts
                        .get(3)
                        .map(|num| format!("{repo} PR #{num}"))
                        .or_else(|| Some(format!("{repo} pull requests"))),
                    Some("pulls") => Some(format!("{repo} pull requests")),
                    Some("blob" | "tree") => match parts.len() > 4 {
                        true => Some(format!("{repo} {}", parts[4..].join("/"))),
                        false => Some(repo),
                    },
                    _ => Some(repo),
                };
            }
            ("raw.githubusercontent.com", true) => {
                let repo = format!("{}/{}", parts[0], parts[1]);
                return match parts.len() > 3 {
                    true => Some(format!("{repo} {}", parts[3..].join("/"))),
                    false => Some(repo),
                };
            }
            ("gitlab.com", true) => return Some(format!("gitlab:{}/{}", parts[0], parts[1])),
            ("crates.io", true) => match parts[0] == "crates" {
                true => return Some(format!("crates.io: {}", parts[1])),
                false => {}
            },
            ("npmjs.com", true) => match parts[0] == "package" {
                true => return Some(format!("npm: {}", parts[1])),
                false => {}
            },
            ("pypi.org", true) => match parts[0] == "project" {
                true => return Some(format!("pypi: {}", parts[1])),
                false => {}
            },
            ("arxiv.org", true) => return Some(format!("arxiv: {}", parts[1])),
            _ => {}
        }
        match (
            host == "docs.rs" && !parts.is_empty(),
            host.ends_with("wikipedia.org") && parts.len() >= 2 && parts[0] == "wiki",
        ) {
            (true, _) => Some(format!("docs.rs: {}", parts[0])),
            (false, true) => Some(format!("wiki: {}", parts[1].replace('_', " "))),
            (false, false) => None,
        }
    })
}

fn is_low_quality_source_label(label: &str, uri: &str) -> bool {
    let lower = label.to_lowercase();
    let has_generic_host_prefix = lower.starts_with("github.com - ")
        || lower.starts_with("raw.githubusercontent.com - ")
        || lower.starts_with("gitlab.com - ");
    match has_generic_host_prefix {
        true => true,
        false => match source_label_from_uri(uri) {
            Some(derived) => {
                let derived_lower = derived.to_lowercase();
                matches!(
                    lower.as_str(),
                    "github.com"
                        | "raw.githubusercontent.com"
                        | "gitlab.com"
                        | "crates.io"
                        | "docs.rs"
                        | "npmjs.com"
                        | "pypi.org"
                        | "github"
                        | "pulls"
                        | "issues"
                ) || lower.parse::<u64>().is_ok()
                    || (lower.len() < derived_lower.len() && derived_lower.contains(&lower))
            }
            None => false,
        },
    }
}

fn is_ui_chrome_noise_summary(lower: &str) -> bool {
    let is_assignee_noise = lower.starts_with("assignee:") && lower.contains("sort by");
    let is_footer_noise = lower.contains("footer navigation")
        && lower.contains("terms")
        && lower.contains("privacy")
        && lower.contains("manage cookies");
    let is_action_error = lower.contains("you can’t perform that action")
        || lower.contains("you can't perform that action");
    let chrome_terms = [
        "filter by",
        "sort by",
        "newest",
        "oldest",
        "most commented",
        "least commented",
        "recently updated",
        "least recently updated",
        "best match",
        "most reactions",
        "protip",
        "manage cookies",
    ];
    let chrome_hits = chrome_terms
        .iter()
        .filter(|term| lower.contains(**term))
        .count()
        >= 5;
    is_assignee_noise || is_footer_noise || is_action_error || chrome_hits
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
    match query.trim().is_empty() {
        true => return (0, 0),
        false => {}
    }
    let query_tokens = tokenize(query);
    match query_tokens.is_empty() {
        true => return (0, 0),
        false => {}
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

fn has_anchor_term_match(query: &str, fields: &[&str]) -> bool {
    let query_tokens = tokenize(query);
    match query_tokens.is_empty() {
        true => false,
        false => {
            let anchor_tokens: std::collections::BTreeSet<String> =
                tokenize(&fields.join(" ")).into_iter().collect();
            query_tokens
                .iter()
                .any(|term| anchor_tokens.contains(term.as_str()))
        }
    }
}

fn source_has_relevant_anchor(query: &str, item: &SourceBookmark) -> bool {
    match query.trim().is_empty() {
        true => true,
        false => {
            let aliases: Vec<&str> = item.aliases.iter().map(|s| s.as_str()).collect();
            let anchor_fields: Vec<&str> = [item.label.as_str(), item.uri.as_str()]
                .into_iter()
                .chain(aliases.iter().copied())
                .collect();
            exact_field_match(query, &[item.label.as_str(), item.uri.as_str()])
                || item
                    .aliases
                    .iter()
                    .any(|alias| alias.trim().eq_ignore_ascii_case(query.trim()))
                || item
                    .uri
                    .to_lowercase()
                    .contains(&query.trim().to_lowercase())
                || has_anchor_term_match(query, &anchor_fields)
        }
    }
}

fn brief_has_relevant_anchor(query: &str, item: &ResearchBrief) -> bool {
    query.trim().is_empty()
        || item.topic.trim().eq_ignore_ascii_case(query.trim())
        || item
            .topic
            .to_lowercase()
            .contains(&query.trim().to_lowercase())
        || has_anchor_term_match(query, &[item.topic.as_str()])
}

fn exact_field_match(query: &str, fields: &[&str]) -> bool {
    let q = query.trim().to_lowercase();
    !q.is_empty()
        && fields
            .iter()
            .any(|field| field.trim().eq_ignore_ascii_case(&q))
}

/// Term-frequency based scoring: returns ratio of query tokens that appear as
/// whole words in the document fields, scaled to a 0..10 range.
fn tf_ratio_score(query: &str, fields: &[&str]) -> f64 {
    let (matching, total) = matching_term_count(query, fields);
    match total == 0 || matching == 0 {
        true => 0.0,
        false => {
            let ratio = matching as f64 / total as f64;
            ratio * 10.0 * (matching as f64).sqrt() / 3.0f64.sqrt()
        }
    }
}

fn source_rank_score(query: &str, item: &SourceBookmark) -> f64 {
    let aliases: Vec<&str> = item.aliases.iter().map(|s| s.as_str()).collect();
    let fields: Vec<&str> = [item.label.as_str(), item.kind.as_str(), item.uri.as_str()]
        .into_iter()
        .chain(aliases.iter().copied())
        .chain(std::iter::once(item.summary.as_str()))
        .collect();
    let (matching_terms, _) = matching_term_count(query, &fields);

    let quality_bonus = item.trust_score.clamp(0.0, 1.0)
        + source_kind_bonus(&item.kind)
        + freshness_bonus(&item.freshness);

    match matching_terms {
        0 => {
            let is_exact = exact_field_match(query, &[&item.label, &item.uri])
                || item
                    .aliases
                    .iter()
                    .any(|alias| alias.trim().eq_ignore_ascii_case(query.trim()));
            let is_uri_sub = !query.trim().is_empty()
                && item
                    .uri
                    .to_lowercase()
                    .contains(&query.trim().to_lowercase());
            match (is_exact, is_uri_sub) {
                (true, _) => 8.0 + quality_bonus,
                (false, true) => 3.0 + quality_bonus,
                (false, false) => 0.0,
            }
        }
        _ => {
            let mut score = tf_ratio_score(query, &fields);
            for term in tokenize(query) {
                let term_matched = exact_field_match(&term, &[item.label.as_str(), item.uri.as_str()])
                    || item
                        .aliases
                        .iter()
                        .any(|a| a.trim().eq_ignore_ascii_case(&term));
                match term_matched {
                    true => score += 2.0,
                    false => {}
                }
            }
            let is_uri_sub = !query.trim().is_empty()
                && item
                    .uri
                    .to_lowercase()
                    .contains(&query.trim().to_lowercase());
            match is_uri_sub {
                true => score += 2.0,
                false => {}
            }
            score + quality_bonus + (item.use_count.max(0) as f64 + 1.0).ln() * 0.25
        }
    }
}

fn is_useful_research_brief_summary(summary: &str) -> bool {
    let normalized = summary.split_whitespace().collect::<Vec<_>>().join(" ");
    let lower = normalized.to_lowercase();
    let invalid = lower.is_empty()
        || matches!(lower.as_str(), "skipped" | "skip" | "none" | "n/a")
        || lower.starts_with("skipped web/search lookup:")
        || lower.contains("fresh saved research brief already matches")
        || is_ui_chrome_noise_summary(&lower);
    !invalid && normalized.chars().filter(|c| c.is_alphabetic()).count() >= 24
}

fn brief_rank_score(query: &str, item: &ResearchBrief) -> f64 {
    let fields = [item.topic.as_str(), item.summary.as_str()];
    let (matching_terms, _) = matching_term_count(query, &fields);
    let mut score = match matching_terms >= 1 {
        true => tf_ratio_score(query, &fields),
        false => 0.0,
    };
    match score <= 0.0 && !query.trim().is_empty() && matching_terms == 0 {
        true => return 0.0,
        false => {}
    }
    match item.topic.trim().eq_ignore_ascii_case(query.trim()) {
        true => score += 8.0,
        false => {}
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

fn topic_match_key(topic: &str) -> String {
    canonical_research_topic(topic)
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect()
}

fn source_is_repo_like(source: &SourceBookmark) -> bool {
    source.kind.trim().eq_ignore_ascii_case("repo")
        || reqwest::Url::parse(&source.uri)
            .ok()
            .and_then(|parsed| {
                parsed
                    .host_str()
                    .map(|host| host.trim_start_matches("www.").to_string())
            })
            .map(|host| {
                matches!(
                    host.as_str(),
                    "github.com" | "raw.githubusercontent.com" | "gitlab.com"
                )
            })
            .unwrap_or(false)
}

fn proven_canonical_topic_for_brief(
    topic: &str,
    source_ids: &[String],
    sources: &[SourceBookmark],
) -> Option<String> {
    let canonical_topic = canonical_research_topic(topic);
    let topic_key = topic_match_key(&canonical_topic);
    match topic_key.is_empty() {
        true => return None,
        false => {}
    }

    let source_id_set: std::collections::BTreeSet<&str> =
        source_ids.iter().map(|id| id.as_str()).collect();
    let mut candidates = sources
        .iter()
        .filter_map(|source| {
            let source_topic = canonical_research_topic(&source.uri);
            let linked = source_id_set.contains(source.id.as_str())
                || source_has_relevant_anchor(&canonical_topic, source)
                || topic_match_key(&source_topic) == topic_key;
            let valid = !source_topic.trim().is_empty()
                && source_topic != canonical_topic
                && linked;
            let score = match source_is_repo_like(source) {
                true => 2,
                false => 1,
            };
            valid.then_some((score, source.score, source_topic))
        })
        .collect::<Vec<_>>();

    candidates.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal))
    });
    candidates.into_iter().map(|(_, _, topic)| topic).next()
}

fn merge_string_arrays(left: &[String], right: &[String]) -> Vec<String> {
    let mut seen = std::collections::BTreeSet::new();
    left.iter()
        .chain(right.iter())
        .filter_map(|item| {
            let trimmed = item.trim();
            (!trimmed.is_empty() && seen.insert(trimmed.to_string()))
                .then(|| trimmed.to_string())
        })
        .collect()
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
    match label.trim().is_empty() || uri.trim().is_empty() {
        true => return Err(anyhow!("label and uri are required")),
        false => {}
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
    rows.retain(|item| {
        query.trim().is_empty() || (item.score > 0.0 && source_has_relevant_anchor(query, item))
    });
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

fn resolve_brief_topic_alias(conn: &rusqlite::Connection, topic: &str) -> Result<String> {
    let canonical = canonical_research_topic(topic);
    match canonical.trim().is_empty() || canonical.contains('/') {
        true => Ok(canonical),
        false => {
            let pattern = format!("%/{canonical}");
            let existing = conn
                .query_row(
                    "SELECT topic FROM research_briefs WHERE topic LIKE ?1 ORDER BY confidence DESC, use_count DESC, updated_at DESC LIMIT 1",
                    params![pattern],
                    |row| row.get::<_, String>(0),
                )
                .optional()?;
            Ok(existing.unwrap_or(canonical))
        }
    }
}

fn resolve_brief_stale_after_secs(
    conn: &rusqlite::Connection,
    source_ids: &[String],
    requested: i64,
) -> Result<i64> {
    match requested > 0 {
        true => Ok(requested.max(60)),
        false => {
            let mut ttl_values = Vec::new();
            for source_id in source_ids {
                let ttl_opt = conn
                    .query_row(
                        "SELECT stale_after_secs FROM source_bookmarks WHERE id = ?1",
                        params![source_id],
                        |row| row.get::<_, i64>(0),
                    )
                    .optional()?;
                match ttl_opt {
                    Some(ttl) => ttl_values.push(ttl.max(60)),
                    None => {}
                }
            }
            Ok(ttl_values.into_iter().min().unwrap_or(604_800))
        }
    }
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
    match topic.trim().is_empty() || summary.trim().is_empty() {
        true => return Err(anyhow!("topic and summary are required")),
        false => {}
    }
    match is_useful_research_brief_summary(summary) {
        false => return Err(anyhow!("research brief summary is not useful")),
        true => {}
    }
    let id = uuid::Uuid::new_v4().to_string();
    let now = now_rfc3339();
    let source_ids_json = serde_json::to_string(&source_ids)?;
    let canonical_topic = {
        let _lock = get_db_mutex().lock().await;
        with_db(|conn| {
            let canonical_topic = resolve_brief_topic_alias(conn, topic)?;
            let resolved_stale_after_secs =
                resolve_brief_stale_after_secs(conn, &source_ids, stale_after_secs)?;
            conn.execute(
                "INSERT INTO research_briefs (id, topic, summary, source_ids, confidence, stale_after_secs, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
                 ON CONFLICT(topic) DO UPDATE SET summary=excluded.summary, source_ids=excluded.source_ids, confidence=excluded.confidence, stale_after_secs=excluded.stale_after_secs, updated_at=excluded.updated_at",
                params![id, canonical_topic, summary.trim(), source_ids_json, confidence.clamp(0.0, 1.0), resolved_stale_after_secs, now],
            )?;
            Ok(canonical_topic)
        })?
    };
    search_research_briefs(&canonical_topic, 1)
        .await?
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("research brief save failed"))
}

pub async fn repair_research_brief_topics() -> Result<usize> {
    let now = now_rfc3339();
    let _lock = get_db_mutex().lock().await;
    with_db(|conn| {
        let sources = {
            let mut stmt = conn.prepare("SELECT id, label, kind, uri, aliases, summary, trust_score, last_checked, stale_after_secs, created_at, updated_at, use_count FROM source_bookmarks ORDER BY trust_score DESC, use_count DESC, updated_at DESC LIMIT 1000")?;
            let mapped = stmt.query_map([], |row| source_from_row(row, 0.0))?;
            let mut out = Vec::new();
            for item in mapped {
                out.push(item?);
            }
            out
        };

        let briefs = {
            let mut stmt = conn.prepare("SELECT id, topic, summary, source_ids, confidence, stale_after_secs, created_at, updated_at, use_count FROM research_briefs ORDER BY confidence DESC, use_count DESC, updated_at DESC LIMIT 1000")?;
            let mapped = stmt.query_map([], |row| brief_from_row(row, 0.0))?;
            let mut out = Vec::new();
            for item in mapped {
                out.push(item?);
            }
            out
        };

        let mut repaired = 0usize;
        for brief in briefs {
            let target_topic = match proven_canonical_topic_for_brief(&brief.topic, &brief.source_ids, &sources) {
                Some(t) => t,
                None => continue,
            };
            match target_topic == brief.topic {
                true => continue,
                false => {}
            }

            let existing = conn
                .query_row(
                    "SELECT id, topic, summary, source_ids, confidence, stale_after_secs, created_at, updated_at, use_count FROM research_briefs WHERE topic = ?1",
                    params![target_topic],
                    |row| brief_from_row(row, 0.0),
                )
                .optional()?;

            match existing {
                Some(existing) => {
                    let merged_source_ids =
                        merge_string_arrays(&existing.source_ids, &brief.source_ids);
                    let merged_source_ids_json = serde_json::to_string(&merged_source_ids)?;
                    let use_incoming_summary = brief.confidence >= existing.confidence
                        && brief.updated_at >= existing.updated_at;
                    let summary = match use_incoming_summary {
                        true => brief.summary.as_str(),
                        false => existing.summary.as_str(),
                    };
                    conn.execute(
                        "UPDATE research_briefs SET summary = ?2, source_ids = ?3, confidence = ?4, stale_after_secs = ?5, updated_at = ?6, use_count = ?7 WHERE topic = ?1",
                        params![
                            target_topic,
                            summary,
                            merged_source_ids_json,
                            existing.confidence.max(brief.confidence),
                            existing.stale_after_secs.min(brief.stale_after_secs).max(60),
                            now,
                            existing.use_count.max(brief.use_count)
                        ],
                    )?;
                    conn.execute(
                        "DELETE FROM research_briefs WHERE id = ?1",
                        params![brief.id],
                    )?;
                }
                None => {
                    conn.execute(
                        "UPDATE research_briefs SET topic = ?2, updated_at = ?3 WHERE id = ?1",
                        params![brief.id, target_topic, now],
                    )?;
                }
            }
            repaired += 1;
        }
        Ok(repaired)
    })
}

pub async fn search_research_briefs(query: &str, limit: usize) -> Result<Vec<ResearchBrief>> {
    let canonical_query = canonical_research_topic(query);
    let search_query = match canonical_query.trim().is_empty() {
        true => query,
        false => &canonical_query,
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
    rows.retain(|item| is_useful_research_brief_summary(&item.summary));
    for item in &mut rows {
        item.score = brief_rank_score(search_query, item);
    }
    rows.retain(|item| {
        query.trim().is_empty()
            || (item.score > 0.0 && brief_has_relevant_anchor(search_query, item))
    });
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

fn extract_source_key(arguments: &Value) -> Result<&str> {
    ["id", "uri", "url", "link", "path", "target"]
        .into_iter()
        .find_map(|k| arguments.get(k).and_then(|v| v.as_str()))
        .ok_or_else(|| anyhow!("Missing id, uri, or url"))
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
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["add", "search", "get", "delete", "mark_checked"],
                    "description": "Action to perform (add, search, get, delete, mark_checked). Default inferred from arguments."
                },
                "label": {
                    "type": "string",
                    "description": "Human-readable label for the source (aliases: title, name). Auto-derived if omitted."
                },
                "kind": {
                    "type": "string",
                    "description": "Source kind: website, repo, docs, path, news, etc."
                },
                "uri": {
                    "type": "string",
                    "description": "URL or filesystem path to bookmark (aliases: url, link, path, target)."
                },
                "aliases": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Alternative names or tags for the source (aliases: tags, tag, alias)."
                },
                "summary": {
                    "type": "string",
                    "description": "Summary or key takeaways from this source (aliases: description, desc, notes)."
                },
                "trust_score": {
                    "type": "number",
                    "description": "Trust score from 0.0 to 1.0 (default 0.5, aliases: trust, score, confidence)."
                },
                "stale_after_secs": {
                    "type": "integer",
                    "description": "Time to live in seconds before source is considered stale (aliases: ttl)."
                },
                "query": {
                    "type": "string",
                    "description": "Search query for finding source bookmarks (aliases: q, search, term)."
                },
                "id": {
                    "type": "string",
                    "description": "Bookmark ID for get/delete/mark_checked actions."
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of search results to return (default 5, aliases: max_results, count, top_k)."
                }
            },
            "required": []
        })
    }
    async fn call(&self, arguments: &Value) -> Result<Value> {
        let parsed_args;
        let arguments = match arguments.as_str() {
            Some(raw_str) => {
                let trimmed = raw_str.trim();
                parsed_args = match serde_json::from_str::<Value>(trimmed) {
                    Ok(val) => val,
                    Err(_) => match (
                        trimmed.starts_with("http://") || trimmed.starts_with("https://"),
                        trimmed.starts_with('/')
                            || trimmed.starts_with('~')
                            || trimmed.starts_with("./"),
                        trimmed.strip_prefix("search "),
                    ) {
                        (true, _, _) => json!({
                            "action": "add",
                            "uri": trimmed,
                            "label": display_source_label("", trimmed)
                        }),
                        (false, true, _) => json!({
                            "action": "add",
                            "uri": trimmed,
                            "kind": "path",
                            "label": display_source_label("", trimmed)
                        }),
                        (false, false, Some(rest)) => json!({
                            "action": "search",
                            "query": rest.trim()
                        }),
                        (false, false, None) => json!({
                            "action": "search",
                            "query": trimmed
                        }),
                    },
                };
                &parsed_args
            }
            None => arguments,
        };

        let action_str = ["action", "act", "command", "cmd", "op"]
            .into_iter()
            .find_map(|k| arguments.get(k).and_then(|v| v.as_str()));

        let has_query = ["query", "q", "search"]
            .into_iter()
            .any(|k| arguments.get(k).is_some());
        let has_uri = ["uri", "url", "link"]
            .into_iter()
            .any(|k| arguments.get(k).is_some());
        let has_metadata = ["label", "title", "name", "summary", "description"]
            .into_iter()
            .any(|k| arguments.get(k).is_some());
        let has_id = arguments.get("id").is_some();

        let normalized_action = match action_str {
            Some(a) => a.trim().to_lowercase(),
            None => match (has_query, has_uri, has_metadata, has_id) {
                (true, _, _, _) => "search".to_string(),
                (false, true, true, _) => "add".to_string(),
                (false, true, false, _) | (false, false, _, true) => "get".to_string(),
                _ => "search".to_string(),
            },
        };

        let action = match normalized_action.as_str() {
            "add" | "save" | "create" | "insert" | "bookmark" => "add",
            "search" | "find" | "list" | "ls" | "query" => "search",
            "get" | "read" | "fetch" | "view" | "show" => "get",
            "delete" | "remove" | "rm" => "delete",
            "mark_checked" | "checked" | "touch" | "update" => "mark_checked",
            other => other,
        };

        match action {
            "add" => {
                let uri = ["uri", "url", "link", "href", "path", "target"]
                    .into_iter()
                    .find_map(|k| arguments.get(k).and_then(|v| v.as_str()))
                    .ok_or_else(|| anyhow!("Missing uri or url for action 'add'"))?;

                let label = ["label", "title", "name", "header"]
                    .into_iter()
                    .find_map(|k| arguments.get(k).and_then(|v| v.as_str()))
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(ToString::to_string)
                    .unwrap_or_else(|| display_source_label("", uri));

                let kind = arguments
                    .get("kind")
                    .or_else(|| arguments.get("type"))
                    .and_then(|v| v.as_str())
                    .unwrap_or_else(|| {
                        let is_http = uri.starts_with("http");
                        let is_repo = uri.contains("github.com") || uri.contains("gitlab.com");
                        let is_path =
                            uri.starts_with('/') || uri.starts_with('~') || uri.starts_with("./");
                        match (is_http, is_repo, is_path) {
                            (true, true, _) => "repo",
                            (true, false, _) => "website",
                            (false, _, true) => "path",
                            (false, _, false) => "other",
                        }
                    });

                let aliases = ["aliases", "tags", "tag"]
                    .into_iter()
                    .find_map(|k| arguments.get(k))
                    .map(|arr| match arr {
                        Value::Array(_) => json_string_array(Some(arr)),
                        Value::String(s) => s
                            .split(',')
                            .map(|p| p.trim().to_string())
                            .filter(|p| !p.is_empty())
                            .collect(),
                        _ => Vec::new(),
                    })
                    .unwrap_or_default();

                let summary = ["summary", "description", "desc", "notes", "note", "about"]
                    .into_iter()
                    .find_map(|k| arguments.get(k).and_then(|v| v.as_str()))
                    .unwrap_or("");

                let trust_score = ["trust_score", "trust", "score", "confidence"]
                    .into_iter()
                    .find_map(|k| arguments.get(k).and_then(|v| v.as_f64()))
                    .unwrap_or(0.5);

                let stale_after_secs = ["stale_after_secs", "staleAfterSecs", "ttl"]
                    .into_iter()
                    .find_map(|k| arguments.get(k).and_then(|v| v.as_i64()))
                    .unwrap_or(604800);

                Ok(json!({
                    "status": "success",
                    "source": add_source_bookmark(&label, kind, uri, aliases, summary, trust_score, stale_after_secs).await?
                }))
            }
            "search" => {
                let query = ["query", "q", "search", "term", "keyword"]
                    .into_iter()
                    .find_map(|k| arguments.get(k).and_then(|v| v.as_str()))
                    .unwrap_or("");
                let limit = ["limit", "max_results", "count", "top_k"]
                    .into_iter()
                    .find_map(|k| arguments.get(k).and_then(|v| v.as_u64()))
                    .unwrap_or(5) as usize;
                Ok(json!({
                    "status": "success",
                    "matches": search_source_bookmarks(query, limit).await?
                }))
            }
            "get" => {
                let key = extract_source_key(arguments)?;
                let is_uri_like = key.starts_with("http")
                    || key.starts_with('/')
                    || key.starts_with('~')
                    || key.starts_with("./");
                let item = match is_uri_like {
                    true => get_source_by_uri(key).await?,
                    false => get_source_by_id(key).await?,
                };
                Ok(json!({"status": "success", "source": item}))
            }
            "delete" => {
                let key = extract_source_key(arguments)?;
                Ok(json!({"status": "success", "deleted": delete_source(key).await?}))
            }
            "mark_checked" => {
                let key = extract_source_key(arguments)?;
                Ok(json!({"status": "success", "updated": mark_source_checked(key).await?}))
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
        let has_summary = ["summary", "context", "content"]
            .into_iter()
            .any(|k| arguments.get(k).is_some());
        let inferred_action = arguments
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or(match has_summary {
                true => "save",
                false => "search",
            });
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
                let source_ids = json_string_array(
                    arguments
                        .get("source_ids")
                        .or_else(|| arguments.get("sources")),
                );
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
#[path = "knowledge_tests.rs"]
mod tests;
