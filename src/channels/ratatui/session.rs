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
mod tests {
    use super::*;

    #[test]
    fn test_process_is_alive_self() {
        assert!(process_is_alive(std::process::id()));
    }

    #[test]
    fn test_process_is_alive_invalid_pid() {
        assert!(!process_is_alive(u32::MAX));
    }

    #[test]
    fn test_tui_marker_lifecycle() {
        let temp_dir = std::env::temp_dir().join(format!("test_tui_marker_{}", uuid::Uuid::new_v4()));
        let pid = 999999;
        let session_key = "test:session";
        let model = "test-model";
        let provider = "test-provider";

        assert!(write_tui_marker_in_dir(&temp_dir, pid, session_key, model, provider).is_ok());

        let marker_path = tui_marker_path_in_dir(&temp_dir, pid);
        assert!(marker_path.exists());

        let content = std::fs::read_to_string(&marker_path).unwrap();
        let val: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(val["pid"], pid);
        assert_eq!(val["session_key"], session_key);
        assert_eq!(val["model"], model);
        assert_eq!(val["provider"], provider);

        remove_tui_marker_in_dir(&temp_dir, pid);
        assert!(!marker_path.exists());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_is_last_live_tui_in_dir() {
        let temp_dir = std::env::temp_dir().join(format!("test_last_live_{}", uuid::Uuid::new_v4()));
        let current_pid = std::process::id();

        // Empty directory
        assert!(is_last_live_tui_in_dir(&temp_dir, current_pid));

        // Write marker for current pid
        let _ = write_tui_marker_in_dir(&temp_dir, current_pid, "s1", "m1", "p1");
        assert!(is_last_live_tui_in_dir(&temp_dir, current_pid));

        // Write marker for dead pid (e.g. u32::MAX)
        let dead_pid = u32::MAX;
        let _ = write_tui_marker_in_dir(&temp_dir, dead_pid, "s2", "m2", "p2");
        // is_last_live_tui_in_dir cleans up dead marker and returns true
        assert!(is_last_live_tui_in_dir(&temp_dir, current_pid));
        assert!(!tui_marker_path_in_dir(&temp_dir, dead_pid).exists());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
