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

pub(crate) async fn logs_event() -> Value {
    let mut log_entries = Vec::new();
    let log_path = crate::logs::default_log_path();
    match std::fs::read_to_string(&log_path) {
        Ok(content) => {
            let lines: Vec<&str> = content.lines().rev().take(100).collect();
            for (index, line) in lines.iter().enumerate() {
                match line.trim().is_empty() {
                    true => continue,
                    false => {}
                }
                let level = match (line.contains("ERROR"), line.contains("WARN")) {
                    (true, _) => "ERROR",
                    (false, true) => "WARN",
                    (false, false) => "INFO",
                };
                let timestamp = match (
                    line.as_bytes().get(10).copied() == Some(b'T'),
                    line.get(11..19),
                ) {
                    (true, Some(ts)) => ts.to_string(),
                    _ => chrono::Local::now().format("%H:%M:%S").to_string(),
                };
                log_entries.push(super::super::protocol::log_entry(
                    format!("log-{}", index),
                    timestamp,
                    level,
                    "openz::gateway",
                    line.to_string(),
                ));
            }
        }
        Err(_) => {}
    }
    super::super::protocol::logs_data(log_entries)
}
