use crate::tools::Tool;
use anyhow::{anyhow, Result};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

struct WatcherHandle {
    path: PathBuf,
    command: String,
    shutdown_tx: mpsc::Sender<()>,
}

static ACTIVE_WATCHER: Mutex<Option<WatcherHandle>> = Mutex::new(None);

pub struct FileWatcherTool;

#[async_trait::async_trait]
impl Tool for FileWatcherTool {
    fn name(&self) -> &str {
        "file_watcher"
    }

    fn description(&self) -> &str {
        "Start, stop, or query a background filesystem watcher that runs a command on file modifications. If the command fails, it sends the output to OpenZ for auto-healing."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["start", "stop", "status"],
                    "description": "Action to perform: 'start' to watch a path, 'stop' to stop watching, 'status' to query status."
                },
                "path": {
                    "type": "string",
                    "description": "The directory path to watch (required for 'start')."
                },
                "command": {
                    "type": "string",
                    "description": "The shell command to run on changes, e.g. 'cargo check' or 'cargo test' (defaults to 'cargo check' if not specified)."
                }
            },
            "required": ["action"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let action = arguments.get("action").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'action' parameter"))?;

        match action {
            "start" => {
                let path_str = arguments.get("path")
                    .or(arguments.get("TargetFile"))
                    .or(arguments.get("filepath"))
                    .or(arguments.get("file"))
                    .or(arguments.get("Path"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'path' parameter for 'start' action"))?;
                let command = arguments.get("command").and_then(|v| v.as_str())
                    .unwrap_or("cargo check")
                    .to_string();

                let resolved_path = crate::config::resolve_path(path_str);
                if !resolved_path.exists() {
                    return Err(anyhow!("Watch path does not exist: {:?}", resolved_path));
                }

                // Stop any existing watcher first
                let shutdown_to_await = {
                    let mut active = ACTIVE_WATCHER.lock().unwrap();
                    active.take().map(|h| (h.path, h.shutdown_tx))
                };
                if let Some((path, tx)) = shutdown_to_await {
                    println!("Stopping existing file watcher on {:?}...", path);
                    let _ = tx.send(()).await;
                }

                let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
                let (event_tx, mut event_rx) = mpsc::channel::<()>(100);

                // Initialize RecommendedWatcher
                let event_tx_clone = event_tx.clone();
                let mut watcher = RecommendedWatcher::new(
                    move |res: Result<notify::Event, notify::Error>| {
                        if let Ok(event) = res {
                            // Filter for file modify/write/create events, ignoring target/git files
                            let is_modify = event.kind.is_modify() || event.kind.is_create();
                            let has_valid_file = event.paths.iter().any(|p| {
                                if let Some(ext) = p.extension() {
                                    let ext_str = ext.to_string_lossy();
                                    (ext_str == "rs" || ext_str == "toml" || ext_str == "py" || ext_str == "js" || ext_str == "ts")
                                        && !p.to_string_lossy().contains("/target/")
                                        && !p.to_string_lossy().contains("/.git/")
                                } else {
                                    false
                                }
                            });

                            if is_modify && has_valid_file {
                                let _ = event_tx_clone.try_send(());
                            }
                        }
                    },
                    notify::Config::default(),
                )?;

                watcher.watch(&resolved_path, RecursiveMode::Recursive)?;

                // Spawn background task to handle events with debouncing
                let path_clone = resolved_path.clone();
                let cmd_clone = command.clone();
                
                tokio::spawn(async move {
                    // Keep watcher alive in this scope
                    let _watcher = watcher;
                    
                    let mut last_trigger = Instant::now() - Duration::from_secs(10);
                    let debounce_duration = Duration::from_millis(800);

                    loop {
                        tokio::select! {
                            _ = shutdown_rx.recv() => {
                                crate::channels::cli::send_notification("Watcher task shutdown signal received.");
                                break;
                            }
                            Some(_) = event_rx.recv() => {
                                let now = Instant::now();
                                if now.duration_since(last_trigger) > debounce_duration {
                                    last_trigger = now;
                                    crate::channels::cli::send_notification(&format!("File watcher triggered: running '{}' in {:?}", cmd_clone, path_clone));
                                    
                                    // Run command
                                    let cmd_parts: Vec<&str> = cmd_clone.split_whitespace().collect();
                                    if !cmd_parts.is_empty() {
                                        let mut cmd = std::process::Command::new(cmd_parts[0]);
                                        if cmd_parts.len() > 1 {
                                            cmd.args(&cmd_parts[1..]);
                                        }
                                        cmd.current_dir(&path_clone);
                                        
                                        match cmd.output() {
                                            Ok(output) => {
                                                if !output.status.success() {
                                                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                                                    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                                                    let error_msg = if stderr.trim().is_empty() { stdout } else { stderr };
                                                    
                                                    crate::channels::cli::send_notification("File watcher command failed! Sending to OpenZ session...");
                                                    let notification = format!(
                                                        "⚠️ [File Watcher] Command '{}' failed after file modification in {:?}:\n\n```\n{}\n```\n\nPlease fix the errors above.",
                                                        cmd_clone, path_clone, error_msg
                                                    );
                                                    let _ = crate::agent::activity::send_inbox_message("cli:direct", &notification, "file_watcher");
                                                } else {
                                                    crate::channels::cli::send_notification("File watcher command succeeded.");
                                                }
                                            }
                                            Err(e) => {
                                                crate::channels::cli::send_notification(&format!("Failed to execute watch trigger command: {}", e));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                });

                {
                    let mut active = ACTIVE_WATCHER.lock().unwrap();
                    *active = Some(WatcherHandle {
                        path: resolved_path.clone(),
                        command: command.clone(),
                        shutdown_tx,
                    });
                }

                Ok(json!({
                    "status": "success",
                    "message": format!("Started file watcher on {:?} running '{}'", resolved_path, command)
                }))
            }
            "stop" => {
                let tx_to_await = {
                    let mut active = ACTIVE_WATCHER.lock().unwrap();
                    active.take().map(|h| h.shutdown_tx)
                };
                if let Some(tx) = tx_to_await {
                    let _ = tx.send(()).await;
                    Ok(json!({
                        "status": "success",
                        "message": "File watcher stopped."
                    }))
                } else {
                    Ok(json!({
                        "status": "success",
                        "message": "No active file watcher running."
                    }))
                }
            }
            "status" => {
                let active = ACTIVE_WATCHER.lock().unwrap();
                if let Some(handle) = active.as_ref() {
                    Ok(json!({
                        "status": "active",
                        "path": handle.path.to_string_lossy(),
                        "command": handle.command
                    }))
                } else {
                    Ok(json!({
                        "status": "inactive"
                    }))
                }
            }
            _ => Err(anyhow!("Unsupported action: {}", action)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_file_watcher_status() -> Result<()> {
        let tool = FileWatcherTool;
        let res = tool.call(&json!({
            "action": "status"
        })).await?;

        assert_eq!(res["status"], "inactive");
        Ok(())
    }
}
