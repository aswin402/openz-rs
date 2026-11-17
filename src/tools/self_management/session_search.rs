use crate::session::{Message, Session};
use anyhow::Result;
use rusqlite::{params, Connection};
use serde::Serialize;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct SessionSearchHit {
    pub session_key: String,
    pub message_index: usize,
    pub role: String,
    pub timestamp: Option<String>,
    pub score: f64,
    pub snippet: String,
}

fn resolve_fts_db_path(sessions_dir: &Path) -> PathBuf {
    let default_sessions = crate::config::sessions_dir();
    match sessions_dir == default_sessions {
        true => crate::config::loader::runtime_db_path("memory.db"),
        false => sessions_dir.join(".sessions_fts.db"),
    }
}

fn open_fts_connection(sessions_dir: &Path) -> Result<Connection> {
    let db_path = resolve_fts_db_path(sessions_dir);
    let _ = db_path.parent().map(fs::create_dir_all);
    let conn = Connection::open(&db_path)?;
    let _ = crate::core::sqlite::apply_standard_pragmas(&conn);
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS session_index_state (
            session_key TEXT PRIMARY KEY,
            updated_at TEXT NOT NULL,
            indexed_count INTEGER NOT NULL
        );
        CREATE VIRTUAL TABLE IF NOT EXISTS sessions_fts USING fts5(
            session_key UNINDEXED,
            msg_index UNINDEXED,
            role,
            timestamp UNINDEXED,
            content
        );",
    )?;
    Ok(conn)
}

fn format_indexable_message_text(msg: &Message) -> String {
    let mut out: String = msg.content.chars().take(6_000).collect();
    let _ = msg
        .extra
        .get("tool_calls")
        .and_then(|v| v.as_array())
        .map(|calls| {
            for tc in calls.iter().take(8) {
                let name = tc
                    .get("name")
                    .and_then(|v| v.as_str())
                    .or_else(|| tc.get("function").and_then(|f| f.get("name")).and_then(|v| v.as_str()))
                    .unwrap_or("");
                let args = tc
                    .get("arguments")
                    .or_else(|| tc.get("function").and_then(|f| f.get("arguments")))
                    .map(|v| v.to_string())
                    .unwrap_or_default();
                let args_short: String = args.chars().take(400).collect();
                let _ = (!name.is_empty()).then(|| {
                    out.push_str(&format!("\n[tool_call {} {}]", name, args_short));
                });
            }
        });
    out
}

fn upsert_session_into_conn(conn: &Connection, session: &Session) -> Result<()> {
    let updated_str = session.updated_at.to_rfc3339();
    let msg_len = session.messages.len();

    let prev_state: Option<(String, usize)> = conn
        .query_row(
            "SELECT updated_at, indexed_count FROM session_index_state WHERE session_key = ?1",
            params![session.key],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1).unwrap_or(0) as usize)),
        )
        .ok();

    let start_idx = match prev_state {
        Some((prev_ts, prev_count)) => match (prev_ts == updated_str && prev_count == msg_len, msg_len >= prev_count) {
            (true, _) => return Ok(()),
            (false, true) => prev_count,
            (false, false) => {
                conn.execute(
                    "DELETE FROM sessions_fts WHERE session_key = ?1",
                    params![session.key],
                )?;
                0
            }
        },
        None => 0,
    };

    let mut insert_stmt = conn.prepare(
        "INSERT INTO sessions_fts (session_key, msg_index, role, timestamp, content)
         VALUES (?1, ?2, ?3, ?4, ?5)",
    )?;

    for (offset, msg) in session.messages[start_idx..].iter().enumerate() {
        let idx = start_idx + offset;
        let text = format_indexable_message_text(msg);
        match !text.trim().is_empty() {
            true => {
                let ts = msg.timestamp.as_deref().unwrap_or(&updated_str);
                insert_stmt.execute(params![session.key, idx as i64, msg.role, ts, text])?;
            }
            false => {}
        }
    }

    conn.execute(
        "INSERT INTO session_index_state (session_key, updated_at, indexed_count)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(session_key) DO UPDATE SET updated_at = ?2, indexed_count = ?3",
        params![session.key, updated_str, msg_len as i64],
    )?;
    Ok(())
}

pub fn index_session_for_search(sessions_dir: &Path, session: &Session) {
    let _ = open_fts_connection(sessions_dir)
        .and_then(|conn| upsert_session_into_conn(&conn, session));
}

pub fn remove_session_from_search_index(sessions_dir: &Path, session_key: &str) {
    let _ = open_fts_connection(sessions_dir).map(|conn| {
        let _ = conn.execute(
            "DELETE FROM sessions_fts WHERE session_key = ?1",
            params![session_key],
        );
        let _ = conn.execute(
            "DELETE FROM session_index_state WHERE session_key = ?1",
            params![session_key],
        );
    });
}

fn sync_directory_json_files(conn: &Connection, dir: &Path) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_json = path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("json");
        match is_json {
            false => continue,
            true => {
                let _ = fs::read_to_string(&path)
                    .ok()
                    .and_then(|raw| serde_json::from_str::<Session>(&raw).ok())
                    .map(|session| {
                        let _ = upsert_session_into_conn(conn, &session);
                    });
            }
        }
    }
}

pub fn build_fts5_match_expression(raw_query: &str) -> Option<String> {
    let mut seen = HashSet::new();
    let mut terms = Vec::new();
    for token in raw_query
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| s.len() >= 2)
    {
        let should_push = terms.len() < 12 && seen.insert(token.clone());
        let _ = should_push.then(|| terms.push(format!("\"{}\"*", token)));
    }
    (!terms.is_empty()).then(|| terms.join(" OR "))
}

fn extract_centered_snippet(content: &str, raw_query: &str, max_chars: usize) -> String {
    let normalized = content.replace('\n', " ");
    let lower = normalized.to_ascii_lowercase();
    let first_hit_byte = raw_query
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|tok| tok.len() >= 2)
        .find_map(|tok| lower.find(&tok.to_ascii_lowercase()))
        .unwrap_or(0);

    let hit_char_idx = normalized[..first_hit_byte.min(normalized.len())]
        .chars()
        .count();
    let half = max_chars / 2;
    let start_char = hit_char_idx.saturating_sub(half);
    let total_chars = normalized.chars().count();
    let snippet: String = normalized.chars().skip(start_char).take(max_chars).collect();
    let prefix = (start_char > 0).then_some("...").unwrap_or("");
    let suffix = (start_char + max_chars < total_chars)
        .then_some("...")
        .unwrap_or("");
    format!("{prefix}{}{suffix}", snippet.trim())
}

pub fn search_sessions_fts(
    sessions_dir: &Path,
    raw_query: &str,
    session_prefix: Option<&str>,
    role_filter: Option<&str>,
    exclude_session: Option<&str>,
    limit: usize,
) -> Result<Vec<SessionSearchHit>> {
    let match_expr = match build_fts5_match_expression(raw_query) {
        Some(expr) => expr,
        None => return Ok(Vec::new()),
    };

    let conn = open_fts_connection(sessions_dir)?;
    sync_directory_json_files(&conn, sessions_dir);
    sync_directory_json_files(&conn, &sessions_dir.join("archive"));

    let capped_limit = limit.clamp(1, 25);
    let fetch_cap = (capped_limit * 4).max(20) as i64;
    let mut stmt = conn.prepare(
        "SELECT session_key, msg_index, role, timestamp, content, bm25(sessions_fts) AS rank
         FROM sessions_fts
         WHERE sessions_fts MATCH ?1
         ORDER BY rank ASC
         LIMIT ?2",
    )?;

    let prefix_norm = session_prefix
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_ascii_lowercase);
    let role_norm = role_filter
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_ascii_lowercase);

    let rows = stmt.query_map(params![match_expr, fetch_cap], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1).unwrap_or(0) as usize,
            row.get::<_, String>(2)?,
            row.get::<_, Option<String>>(3).unwrap_or(None),
            row.get::<_, String>(4)?,
            row.get::<_, f64>(5).unwrap_or(0.0),
        ))
    })?;

    let mut hits = Vec::new();
    for (session_key, message_index, role, timestamp, content, raw_bm25) in rows.flatten() {
        let excluded = exclude_session.is_some_and(|ex| ex == session_key);
        let prefix_ok = prefix_norm
            .as_ref()
            .is_none_or(|p| session_key.to_ascii_lowercase().starts_with(p));
        let role_ok = role_norm
            .as_ref()
            .is_none_or(|r| role.eq_ignore_ascii_case(r));

        let keep = !excluded && prefix_ok && role_ok && hits.len() < capped_limit;
        let _ = keep.then(|| {
            hits.push(SessionSearchHit {
                session_key,
                message_index,
                role,
                timestamp,
                score: (-raw_bm25).max(0.0),
                snippet: extract_centered_snippet(&content, raw_query, 360),
            });
        });
    }

    Ok(hits)
}
