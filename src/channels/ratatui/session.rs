use anyhow::Result;
use std::sync::Arc;

pub fn tui_marker_dir() -> std::path::PathBuf {
    crate::config::loader::runtime_data_dir().join("tui_instances")
}

pub fn tui_marker_path_in_dir(dir: &std::path::Path, pid: u32) -> std::path::PathBuf {
    dir.join(format!("{pid}.json"))
}

pub fn write_tui_marker_in_dir(
    dir: &std::path::Path,
    pid: u32,
    session_key: &str,
    model: &str,
    provider: &str,
) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    let payload = serde_json::json!({
        "pid": pid,
        "session_key": session_key,
        "model": model,
        "provider": provider,
        "updated_at": chrono::Utc::now().to_rfc3339(),
    });
    std::fs::write(
        tui_marker_path_in_dir(dir, pid),
        serde_json::to_string_pretty(&payload)?,
    )?;
    Ok(())
}

pub fn remove_tui_marker_in_dir(dir: &std::path::Path, pid: u32) {
    let _ = std::fs::remove_file(tui_marker_path_in_dir(dir, pid));
}

pub fn process_is_alive(pid: u32) -> bool {
    if pid == std::process::id() {
        return true;
    }
    #[cfg(unix)]
    {
        if pid > i32::MAX as u32 {
            return false;
        }
        let result = unsafe { libc::kill(pid as libc::pid_t, 0) };
        result == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    }
    #[cfg(not(unix))]
    {
        false
    }
}

pub fn is_last_live_tui_in_dir(dir: &std::path::Path, current_pid: u32) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return true;
    };

    let mut found_other_live = false;
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
            continue;
        };
        let Ok(pid) = stem.parse::<u32>() else {
            continue;
        };
        if pid == current_pid {
            continue;
        }
        if process_is_alive(pid) {
            found_other_live = true;
        } else {
            let _ = std::fs::remove_file(path);
        }
    }
    !found_other_live
}

pub async fn save_session_model_override(
    session_manager: &crate::session::SessionManager,
    session_key: &str,
    provider: &str,
    model: &str,
) -> Result<()> {
    let mut session = session_manager.get_or_create_async(session_key).await;
    session.metadata.insert(
        "provider".to_string(),
        serde_json::Value::String(provider.to_string()),
    );
    session.metadata.insert(
        "model".to_string(),
        serde_json::Value::String(model.to_string()),
    );
    crate::channels::record_recent_model(provider, model);
    session_manager.save(&session).await
}

pub async fn save_session_streaming_override(
    session_manager: &crate::session::SessionManager,
    session_key: &str,
    streaming: bool,
) -> Result<()> {
    let mut session = session_manager.get_or_create_async(session_key).await;
    session
        .metadata
        .insert("streaming".to_string(), serde_json::Value::Bool(streaming));
    session_manager.save(&session).await
}

pub fn save_default_model_selection(provider: &str, model: &str) -> Result<()> {
    let mut cfg = crate::config::loader::load_config()?;
    cfg.agents.defaults.provider = provider.to_string();
    cfg.agents.defaults.model = model.to_string();
    crate::channels::record_recent_model(provider, model);
    crate::config::loader::save_config(&cfg)
}

pub async fn apply_session_model_selection(
    agent_loop: &Arc<tokio::sync::Mutex<crate::agent::agent_loop::AgentLoop>>,
    session_manager: &crate::session::SessionManager,
    session_key: &str,
    marker_dir: &std::path::Path,
    provider: &str,
    model: &str,
) -> Result<()> {
    let mut cfg = crate::config::loader::load_config()?;
    cfg.agents.defaults.provider = provider.to_string();
    cfg.agents.defaults.model = model.to_string();
    let resolved = crate::providers::resolver::resolve_provider_full(&cfg, model)?;
    save_session_model_override(session_manager, session_key, provider, model).await?;
    write_tui_marker_in_dir(marker_dir, std::process::id(), session_key, model, provider)?;
    if let Ok(mut loop_lock) = agent_loop.try_lock() {
        loop_lock.update_model_and_provider(cfg, resolved.instance)
    }
    Ok(())
}

/// Archive the current session (unless already an archived history key) and
/// switch the active session key to `base_key`.
pub async fn reset_active_session(
    shared: &tokio::sync::RwLock<String>,
    session_manager: &crate::session::SessionManager,
    base_key: &str,
) {
    let cur = shared.read().await.clone();
    if !cur.starts_with("cli:history_") {
        let _ = crate::cli::archive_current_session(session_manager, &cur).await;
    }
    *shared.write().await = base_key.to_string();
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
