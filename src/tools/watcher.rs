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
        let (action_str, path_opt, command_opt) = match arguments {
            Value::String(s) => {
                let trimmed = s.trim().to_lowercase();
                match trimmed.as_str() {
                    "status" | "info" | "query" | "get" => ("status".to_string(), None, None),
                    "stop" | "cancel" | "kill" | "terminate" | "close" => {
                        ("stop".to_string(), None, None)
                    }
                    "start" | "watch" => ("start".to_string(), Some(".".to_string()), None),
                    _ => ("start".to_string(), Some(s.trim().to_string()), None),
                }
            }
            Value::Object(map) => {
                let path = map
                    .get("path")
                    .or_else(|| map.get("dir"))
                    .or_else(|| map.get("directory"))
                    .or_else(|| map.get("folder"))
                    .or_else(|| map.get("target"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty());

                let command = map
                    .get("command")
                    .or_else(|| map.get("cmd"))
                    .or_else(|| map.get("run"))
                    .or_else(|| map.get("script"))
                    .or_else(|| map.get("exec"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty());

                let action_raw = map
                    .get("action")
                    .or_else(|| map.get("mode"))
                    .or_else(|| map.get("op"))
                    .or_else(|| map.get("operation"))
                    .or_else(|| map.get("type"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.trim().to_string())
                    .unwrap_or_else(|| match path.is_some() {
                        true => "start".to_string(),
                        false => "status".to_string(),
                    });

                (action_raw, path, command)
            }
            _ => return Err(anyhow!("Invalid arguments: expected object or action string")),
        };

        let action_norm = action_str.trim().to_lowercase().replace('-', "_");
        let action = match action_norm.as_str() {
            "status" | "info" | "query" | "get" => "status",
            "stop" | "cancel" | "kill" | "terminate" | "close" => "stop",
            "start" | "watch" | "run" | "begin" => "start",
            _ => action_norm.as_str(),
        };

        match action {
            "start" => {
                let path_str = path_opt
                    .ok_or_else(|| anyhow!("Missing 'path' parameter for 'start' action"))?;
                let command = command_opt.unwrap_or_else(|| "cargo check".to_string());

                let resolved_path = crate::config::resolve_path(&path_str);
                resolved_path
                    .exists()
                    .then_some(())
                    .ok_or_else(|| anyhow!("Watch path does not exist: {:?}", resolved_path))?;

                // Stop any existing watcher first
                let shutdown_to_await = {
                    let mut active = ACTIVE_WATCHER.lock().unwrap_or_else(|e| e.into_inner());
                    active.take().map(|h| (h.path, h.shutdown_tx))
                };
                match shutdown_to_await {
                    Some((path, tx)) => {
                        println!("Stopping existing file watcher on {:?}...", path);
                        let _ = tx.send(()).await;
                    }
                    None => {}
                }

                let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
                let (event_tx, mut event_rx) = mpsc::channel::<()>(100);

                // Initialize RecommendedWatcher
                let event_tx_clone = event_tx.clone();
                let mut watcher = RecommendedWatcher::new(
                    move |res: Result<notify::Event, notify::Error>| {
                        let _ = res.ok().map(|event| {
                            let is_modify = event.kind.is_modify() || event.kind.is_create();
                            let has_valid_file = event.paths.iter().any(|p| {
                                p.extension().is_some_and(|ext| {
                                    let ext_str = ext.to_string_lossy();
                                    matches!(ext_str.as_ref(), "rs" | "toml" | "py" | "js" | "ts")
                                        && !p.to_string_lossy().contains("/target/")
                                        && !p.to_string_lossy().contains("/.git/")
                                })
                            });
                            let _ = (is_modify && has_valid_file)
                                .then(|| event_tx_clone.try_send(()));
                        });
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
                            biased;
                            _ = shutdown_rx.recv() => {
                                crate::channels::cli::send_notification("Watcher task shutdown signal received.");
                                break;
                            }
                            Some(_) = event_rx.recv() => {
                                let now = Instant::now();
                                match now.duration_since(last_trigger) > debounce_duration {
                                    true => {
                                        last_trigger = now;
                                        crate::channels::cli::send_notification(&format!("File watcher triggered: running '{}' in {:?}", cmd_clone, path_clone));

                                        let cmd_parts: Vec<&str> = cmd_clone.split_whitespace().collect();
                                        match cmd_parts.split_first() {
                                            Some((program, rest_args)) => {
                                                let mut cmd = tokio::process::Command::new(program);
                                                cmd.args(rest_args);
                                                cmd.current_dir(&path_clone);

                                                match cmd.output().await {
                                                    Ok(output) => match output.status.success() {
                                                        true => {
                                                            crate::channels::cli::send_notification("File watcher command succeeded.");
                                                        }
                                                        false => {
                                                            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                                                            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                                                            let error_msg = match stderr.trim().is_empty() {
                                                                true => stdout,
                                                                false => stderr,
                                                            };

                                                            crate::channels::cli::send_notification("File watcher command failed! Sending to OpenZ session...");
                                                            let notification = format!(
                                                                "⚠️ [File Watcher] Command '{}' failed after file modification in {:?}:\n\n```\n{}\n```\n\nPlease fix the errors above.",
                                                                cmd_clone, path_clone, error_msg
                                                            );
                                                            let _ = crate::agent::activity::send_inbox_message("cli:direct", &notification, "file_watcher");
                                                        }
                                                    },
                                                    Err(e) => {
                                                        crate::channels::cli::send_notification(&format!("Failed to execute watch trigger command: {}", e));
                                                    }
                                                }
                                            }
                                            None => {}
                                        }
                                    }
                                    false => {}
                                }
                            }
                        }
                    }
                });

                {
                    let mut active = ACTIVE_WATCHER.lock().unwrap_or_else(|e| e.into_inner());
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
                    let mut active = ACTIVE_WATCHER.lock().unwrap_or_else(|e| e.into_inner());
                    active.take().map(|h| h.shutdown_tx)
                };
                match tx_to_await {
                    Some(tx) => {
                        let _ = tx.send(()).await;
                        Ok(json!({
                            "status": "success",
                            "message": "File watcher stopped."
                        }))
                    }
                    None => Ok(json!({
                        "status": "success",
                        "message": "No active file watcher running."
                    })),
                }
            }
            "status" => {
                let active = ACTIVE_WATCHER.lock().unwrap_or_else(|e| e.into_inner());
                match active.as_ref() {
                    Some(handle) => Ok(json!({
                        "status": "active",
                        "path": handle.path.to_string_lossy(),
                        "command": handle.command
                    })),
                    None => Ok(json!({
                        "status": "inactive"
                    })),
                }
            }
            _ => Err(anyhow!("Unsupported action: {}", action)),
        }
    }
}

#[cfg(test)]
#[path = "watcher_tests.rs"]
mod tests;
