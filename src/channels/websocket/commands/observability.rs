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

fn load_recent_ws_logs_blocking(limit: usize) -> Vec<super::super::protocol::WsLogEntry> {
    let db_path = crate::logs::default_db_path();
    let mut entries = Vec::new();
    match db_path.exists().then(|| rusqlite::Connection::open(&db_path).ok()).flatten() {
        Some(conn) => {
            let _ = crate::logs::ensure_logs_schema(&conn);
            let _ = conn
                .prepare(
                    "SELECT id, timestamp, level, target, message, session, COALESCE(category, '')
                     FROM logs ORDER BY id DESC LIMIT ?1",
                )
                .and_then(|mut stmt| {
                    let rows = stmt.query_map([limit], |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, String>(3)?,
                            row.get::<_, String>(4)?,
                            row.get::<_, Option<String>>(5)?,
                            row.get::<_, String>(6)?,
                        ))
                    })?;
                    for (id, ts, level, target, message, session, raw_cat) in rows.flatten() {
                        let category = match raw_cat.is_empty() {
                            true => crate::logs::classify_log_category(
                                &target,
                                session.as_deref(),
                                &message,
                            )
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
                        entries.push(super::super::protocol::log_entry(
                            format!("log-{id}"),
                            short_hms_timestamp(&ts),
                            clean_level,
                            category,
                            target,
                            message,
                            session,
                        ));
                    }
                    Ok(())
                });
        }
        None => {}
    }
    entries
}

pub(crate) async fn logs_event() -> Value {
    let log_entries = tokio::task::spawn_blocking(|| load_recent_ws_logs_blocking(150))
        .await
        .unwrap_or_default();
    super::super::protocol::logs_data(log_entries)
}
