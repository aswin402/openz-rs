use anyhow::Result;
use std::path::PathBuf;

pub async fn handle_logs(
    path: Option<PathBuf>,
    tail: usize,
    session: Option<String>,
    level: Option<String>,
    global: bool,
    search: Option<String>,
) -> Result<()> {
    let is_global = global || session.as_deref() == Some("global");

    let target_session = if is_global {
        let sessions = crate::logs::get_running_sessions()?;
        if sessions.is_empty() {
            println!("No active agent sessions found in logs database.");
            return Ok(());
        }

        let mut options = Vec::new();
        for s in &sessions {
            let truncated_msg = if s.last_log_message.len() > 50 {
                format!("{}...", &s.last_log_message[..47])
            } else {
                s.last_log_message.clone()
            };
            options.push(format!(
                "{} ({}) - \"{}\" [Last: {}]",
                s.session_id, s.session_type, truncated_msg, s.last_seen
            ));
        }
        options.push("Exit".to_string());

        let selection = crate::agent::style::select_menu_custom(
            "Select a running agent/session to view logs:",
            &options,
            "",
            None,
            true,
        )?;

        match selection {
            Some(idx) if idx < sessions.len() => sessions[idx].session_id.clone(),
            _ => {
                println!("Selection cancelled.");
                return Ok(());
            }
        }
    } else {
        match session {
            Some(s) => s,
            None => {
                if let Some(act) = crate::agent::activity::get_activity() {
                    act.session_id
                } else {
                    crate::logs::get_latest_session_id().unwrap_or_else(|| "all".to_string())
                }
            }
        }
    };

    let effective_tail = if tail == 0 { 50 } else { tail };

    let filter = crate::logs::SessionFilter::from_opt(Some(&target_session));
    let level_filter = crate::logs::LogLevelFilter::from_opt(level.as_deref());
    crate::logs::run_logs_viewer(path, effective_tail, filter, level_filter, search).await
}
