use anyhow::Result;

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
    match pid == std::process::id() {
        true => return true,
        false => {}
    }
    #[cfg(unix)]
    {
        match pid > i32::MAX as u32 {
            true => false,
            false => {
                let result = unsafe { libc::kill(pid as libc::pid_t, 0) };
                result == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
            }
        }
    }
    #[cfg(not(unix))]
    {
        false
    }
}

pub fn is_last_live_tui_in_dir(dir: &std::path::Path, current_pid: u32) -> bool {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return true,
    };

    let mut found_other_live = false;
    for entry in entries.flatten() {
        let path = entry.path();
        let pid = match path
            .file_stem()
            .and_then(|value| value.to_str())
            .and_then(|stem| stem.parse::<u32>().ok())
        {
            Some(p) => p,
            None => continue,
        };
        match pid == current_pid {
            true => continue,
            false => {}
        }
        match process_is_alive(pid) {
            true => found_other_live = true,
            false => {
                let _ = std::fs::remove_file(path);
            }
        }
    }
    !found_other_live
}

pub fn is_session_claimed_by_other_live_tui_in_dir(
    dir: &std::path::Path,
    current_pid: u32,
    session_key: &str,
) -> bool {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return false,
    };

    let mut claimed = false;
    for entry in entries.flatten() {
        let path = entry.path();
        let pid = match path
            .file_stem()
            .and_then(|value| value.to_str())
            .and_then(|stem| stem.parse::<u32>().ok())
        {
            Some(p) => p,
            None => continue,
        };
        match pid == current_pid {
            true => continue,
            false => {}
        }
        match process_is_alive(pid) {
            true => {
                let matches_key = std::fs::read_to_string(&path)
                    .ok()
                    .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
                    .and_then(|val| {
                        val.get("session_key")
                            .and_then(|v| v.as_str())
                            .map(|k| k == session_key)
                    })
                    .unwrap_or(false);
                match matches_key {
                    true => claimed = true,
                    false => {}
                }
            }
            false => {
                let _ = std::fs::remove_file(path);
            }
        }
    }
    claimed
}

pub fn allocate_tui_session_key_in_dir(
    dir: &std::path::Path,
    current_pid: u32,
    default_base_key: &str,
) -> String {
    match is_session_claimed_by_other_live_tui_in_dir(dir, current_pid, default_base_key) {
        false => default_base_key.to_string(),
        true => format!("{default_base_key}_tab_{current_pid}"),
    }
}

pub async fn save_session_model_override(
    session_manager: &crate::session::SessionManager,
    session_key: &str,
    provider: &str,
    model: &str,
) -> Result<()> {
    let mut overrides = serde_json::Map::new();
    overrides.insert(
        "provider".to_string(),
        serde_json::Value::String(provider.to_string()),
    );
    overrides.insert(
        "model".to_string(),
        serde_json::Value::String(model.to_string()),
    );
    crate::channels::record_recent_model(provider, model);
    crate::agent::agent_loop::save_session_overrides(session_manager, session_key, &overrides).await
}

pub async fn save_session_streaming_override(
    session_manager: &crate::session::SessionManager,
    session_key: &str,
    streaming: bool,
) -> Result<()> {
    let mut overrides = serde_json::Map::new();
    overrides.insert("streaming".to_string(), serde_json::Value::Bool(streaming));
    crate::agent::agent_loop::save_session_overrides(session_manager, session_key, &overrides).await
}

pub fn save_default_model_selection(provider: &str, model: &str) -> Result<()> {
    let mut cfg = crate::config::loader::load_config()?;
    cfg.agents.defaults.provider = provider.to_string();
    cfg.agents.defaults.model = model.to_string();
    crate::channels::record_recent_model(provider, model);
    crate::config::loader::save_config(&cfg)
}

pub async fn apply_session_model_selection(
    session_manager: &crate::session::SessionManager,
    session_key: &str,
    marker_dir: &std::path::Path,
    provider: &str,
    model: &str,
) -> Result<()> {
    let mut cfg = crate::config::loader::load_config()?;
    cfg.agents.defaults.provider = provider.to_string();
    cfg.agents.defaults.model = model.to_string();
    let _resolved = crate::providers::resolver::resolve_provider_full(&cfg, model)?;
    save_session_model_override(session_manager, session_key, provider, model).await?;
    write_tui_marker_in_dir(marker_dir, std::process::id(), session_key, model, provider)?;
    Ok(())
}

/// Archive the current session (unless already an archived history key or claimed
/// by another live TUI instance) and switch the active session key to `base_key`.
pub async fn reset_active_session(
    shared: &tokio::sync::RwLock<String>,
    session_manager: &crate::session::SessionManager,
    base_key: &str,
) {
    let cur = shared.read().await.clone();
    let claimed_by_other = is_session_claimed_by_other_live_tui_in_dir(
        &tui_marker_dir(),
        std::process::id(),
        &cur,
    );
    match cur.starts_with("cli:history_") || claimed_by_other {
        true => {}
        false => {
            let _ = crate::cli::archive_current_session(session_manager, &cur).await;
        }
    }
    *shared.write().await = base_key.to_string();
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
