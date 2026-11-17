//! MCP and gateway-log WebSocket response builders.

use serde_json::Value;

pub(crate) async fn mcp_servers_event(config: &crate::config::schema::Config) -> Value {
    let (loaded, failed, _total) = crate::tools::mcp::get_mcp_stats();
    let mcp_done = crate::tools::mcp::is_mcp_done();
    let mut servers = Vec::new();
    for (name, server_cfg) in &config.mcp_servers {
        let status = match (server_cfg.enabled, mcp_done) {
            (false, _) => "disabled",
            (true, false) => "starting",
            (true, true) => "connected",
        };
        let tools_count = match server_cfg.enabled {
            true => {
                crate::tools::mcp::spawned_tools_count(&server_cfg.command, &server_cfg.args).await
            }
            false => 0,
        };
        servers.push(super::super::protocol::mcp_server(
            name,
            server_cfg.command.clone(),
            status,
            server_cfg.enabled,
            server_cfg.args.clone(),
            tools_count,
        ));
    }
    super::super::protocol::mcp_servers(servers, loaded, failed, failed + loaded)
}

fn short_hms_timestamp(ts: &str) -> String {
    match (ts.as_bytes().get(10).copied() == Some(b'T'), ts.get(11..19)) {
        (true, Some(hms)) => hms.to_string(),
        _ => ts.chars().take(19).collect(),
    }
}

pub(crate) fn normalize_ws_session_key(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    match (trimmed.is_empty(), trimmed.contains(':')) {
        (true, _) => None,
        (false, true) => Some(trimmed.to_string()),
        (false, false) => Some(format!("ws:{trimmed}")),
    }
}

type RawDbLogRow = (
    i64,
    String,
    String,
    String,
    String,
    Option<String>,
    String,
    Option<String>,
);

fn map_db_row_to_ws_entry(row: RawDbLogRow) -> (i64, super::super::protocol::WsLogEntry) {
    let (id, ts, level, target, message, session, raw_cat, raw_fields) = row;
    let category = match raw_cat.is_empty() {
        true => crate::logs::classify_log_category(&target, session.as_deref(), &message)
            .as_str()
            .to_string(),
        false => raw_cat,
    };
    let clean_level = match level.to_ascii_uppercase().as_str() {
        "ERROR" => "ERROR",
        "WARN" => "WARN",
        "DEBUG" => "DEBUG",
        "TRACE" => "TRACE",
        _ => "INFO",
    };
    let fields = raw_fields
        .and_then(|json_str| {
            serde_json::from_str::<std::collections::BTreeMap<String, String>>(&json_str).ok()
        })
        .filter(|map| !map.is_empty());

    (
        id,
        super::super::protocol::log_entry(
            format!("log-{id}"),
            short_hms_timestamp(&ts),
            clean_level,
            category,
            target,
            message,
            session,
            fields,
        ),
    )
}

fn read_db_log_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawDbLogRow> {
    Ok((
        row.get::<_, i64>(0)?,
        row.get::<_, String>(1)?,
        row.get::<_, String>(2)?,
        row.get::<_, String>(3)?,
        row.get::<_, String>(4)?,
        row.get::<_, Option<String>>(5)?,
        row.get::<_, String>(6)?,
        row.get::<_, Option<String>>(7)?,
    ))
}

fn collect_query_slice<P: rusqlite::Params>(
    conn: &rusqlite::Connection,
    sql: &str,
    params: P,
    by_id: &mut std::collections::BTreeMap<i64, super::super::protocol::WsLogEntry>,
) {
    let _ = conn.prepare(sql).and_then(|mut stmt| {
        let rows = stmt.query_map(params, read_db_log_row)?;
        for raw_row in rows.flatten() {
            let (id, entry) = map_db_row_to_ws_entry(raw_row);
            by_id.entry(id).or_insert(entry);
        }
        Ok(())
    });
}

pub(crate) fn load_ws_logs_from_db(
    db_path: &std::path::Path,
    active_session: Option<&str>,
) -> Vec<super::super::protocol::WsLogEntry> {
    let mut by_id = std::collections::BTreeMap::new();
    match db_path
        .exists()
        .then(|| rusqlite::Connection::open(db_path).ok())
        .flatten()
    {
        Some(conn) => {
            let _ = crate::logs::ensure_logs_schema(&conn);

            let _ = active_session.map(|sess| {
                let child_prefix = format!("{sess}:%");
                collect_query_slice(
                    &conn,
                    "SELECT id, timestamp, level, target, message, session, COALESCE(category, ''), fields_json
                     FROM logs
                     WHERE session = ?1 OR session LIKE ?2
                     ORDER BY id DESC LIMIT 120",
                    rusqlite::params![sess, child_prefix],
                    &mut by_id,
                );
            });

            collect_query_slice(
                &conn,
                "SELECT id, timestamp, level, target, message, session, COALESCE(category, ''), fields_json
                 FROM logs
                 WHERE COALESCE(category, '') IN ('cron', 'curator') OR session LIKE 'cron:%'
                 ORDER BY id DESC LIMIT 80",
                [],
                &mut by_id,
            );

            collect_query_slice(
                &conn,
                "SELECT id, timestamp, level, target, message, session, COALESCE(category, ''), fields_json
                 FROM logs
                 WHERE COALESCE(category, 'system') NOT IN ('cron', 'curator')
                   AND (session IS NULL OR session NOT LIKE 'cron:%')
                 ORDER BY id DESC LIMIT 80",
                [],
                &mut by_id,
            );
        }
        None => {}
    }
    by_id.into_values().rev().collect()
}

pub(crate) async fn logs_event(envelope: &Value, active_chat_id: &str) -> Value {
    let requested = envelope
        .get("session")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            envelope
                .get("chat_id")
                .and_then(Value::as_str)
                .filter(|s| !s.trim().is_empty())
        })
        .unwrap_or(active_chat_id);
    let normalized_session = normalize_ws_session_key(requested);
    let log_entries = tokio::task::spawn_blocking(move || {
        let db_path = crate::logs::default_db_path();
        load_ws_logs_from_db(&db_path, normalized_session.as_deref())
    })
    .await
    .unwrap_or_default();
    super::super::protocol::logs_data(log_entries)
}

#[cfg(test)]
#[path = "observability_tests.rs"]
mod tests;
