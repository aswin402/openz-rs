use anyhow::Result;
use std::path::PathBuf;

fn truncate_log_preview(message: &str) -> String {
    match message.len() > 50 {
        true => format!("{}...", &message[..47]),
        false => message.to_string(),
    }
}

fn select_running_session_id() -> Result<Option<String>> {
    let sessions = crate::logs::get_running_sessions()?;
    match sessions.is_empty() {
        true => {
            println!("No active agent sessions found in logs database.");
            return Ok(None);
        }
        false => {}
    }

    let mut options: Vec<String> = sessions
        .iter()
        .map(|s| {
            let truncated_msg = truncate_log_preview(&s.last_log_message);
            format!(
                "{} ({}) - \"{}\" [Last: {}]",
                s.session_id, s.session_type, truncated_msg, s.last_seen
            )
        })
        .collect();
    options.push("Exit".to_string());

    let selection = crate::agent::style::select_menu_custom(
        "Select a running agent/session to view logs:",
        &options,
        "",
        None,
        true,
    )?;

    match selection.and_then(|idx| sessions.get(idx)) {
        Some(selected) => Ok(Some(selected.session_id.clone())),
        None => {
            println!("Selection cancelled.");
            Ok(None)
        }
    }
}

fn resolve_default_session_id(session: Option<String>) -> String {
    match session {
        Some(s) => s,
        None => match crate::agent::activity::get_activity() {
            Some(act) => act.session_id,
            None => crate::logs::get_latest_session_id().unwrap_or_else(|| "all".to_string()),
        },
    }
}

pub async fn handle_logs(
    path: Option<PathBuf>,
    tail: usize,
    session: Option<String>,
    level: Option<String>,
    global: bool,
    search: Option<String>,
) -> Result<()> {
    let is_global = global || session.as_deref() == Some("global");

    let target_session = match is_global {
        true => match select_running_session_id()? {
            Some(id) => id,
            None => return Ok(()),
        },
        false => resolve_default_session_id(session),
    };

    let effective_tail = match tail {
        0 => 50,
        n => n,
    };

    let filter = crate::logs::SessionFilter::from_opt(Some(&target_session));
    let level_filter = crate::logs::LogLevelFilter::from_opt(level.as_deref());
    crate::logs::run_logs_viewer(path, effective_tail, filter, level_filter, search).await
}
