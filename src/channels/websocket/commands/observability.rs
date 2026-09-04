//! MCP and gateway-log WebSocket response builders.

use serde_json::Value;

pub(crate) async fn mcp_servers_event(config: &crate::config::schema::Config) -> Value {
    let (loaded, failed, _total) = crate::channels::cli::mcp::get_mcp_stats();
    let mcp_done = crate::channels::cli::mcp::is_mcp_done();
    let mut servers = Vec::new();
    for (name, server_cfg) in &config.mcp_servers {
        let status = if !server_cfg.enabled {
            "disabled"
        } else if !mcp_done {
            "starting"
        } else {
            "connected"
        };
        let tools_count = if server_cfg.enabled {
            crate::tools::mcp::spawned_tools_count(&server_cfg.command, &server_cfg.args).await
        } else {
            0
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
    if let Ok(content) = std::fs::read_to_string(&log_path) {
        let lines: Vec<&str> = content.lines().rev().take(100).collect();
        for (index, line) in lines.iter().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let level = if line.contains("ERROR") {
                "ERROR"
            } else if line.contains("WARN") {
                "WARN"
            } else {
                "INFO"
            };
            let timestamp = if line.len() >= 19 && line.as_bytes()[10] == b'T' {
                line[11..19].to_string()
            } else {
                chrono::Local::now().format("%H:%M:%S").to_string()
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
    super::super::protocol::logs_data(log_entries)
}
