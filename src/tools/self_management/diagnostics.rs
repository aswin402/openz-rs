use super::extract_first_str;
use crate::tools::arguments::get_bool_arg as parse_bool_param;
use crate::tools::Tool;
use anyhow::Result;
use serde_json::Value;

pub struct DiagnoseToolTool {
    registry: crate::tools::ToolRegistry,
}

fn is_placeholder_mock_args(value: &Value) -> bool {
    value
        .as_object()
        .map(|obj| obj.is_empty() || (obj.len() == 1 && obj.contains_key("test")))
        .unwrap_or(false)
}

fn minimal_openmedia_video_scene() -> Value {
    serde_json::json!({
        "width": 640,
        "height": 360,
        "fps": 1,
        "duration": 1.0,
        "background": "#1e293b",
        "scenes": [{
            "id": "scene_1",
            "start": 0.0,
            "end": 1.0,
            "elements": [{
                "type": "text",
                "content": "OpenZ",
                "style": {
                    "font_family": "sans-serif",
                    "font_size": 48.0,
                    "font_weight": 800,
                    "color": "#ffffff",
                    "text_align": "center"
                },
                "position": { "x": 320.0, "y": 180.0 },
                "anchor": "center",
                "timeline": null
            }]
        }],
        "transitions": [],
        "audio": null
    })
}

pub(crate) fn normalize_diagnose_mock_args(tool_name: &str, mock_args: Value) -> Value {
    match is_placeholder_mock_args(&mock_args) {
        false => mock_args,
        true => match tool_name {
            "openmedia_video_create" => serde_json::json!({
                "scene": minimal_openmedia_video_scene()
            }),
            "openmedia_video_preview" => serde_json::json!({
                "scene": minimal_openmedia_video_scene(),
                "time": 0.0,
                "width": 160,
                "height": 120,
                "output_format": "png"
            }),
            _ => mock_args,
        },
    }
}

impl DiagnoseToolTool {
    pub fn new(registry: crate::tools::ToolRegistry) -> Self {
        Self { registry }
    }
}

#[async_trait::async_trait]
impl Tool for DiagnoseToolTool {
    fn name(&self) -> &str {
        "diagnose_tool"
    }

    fn description(&self) -> &str {
        "Diagnose, test, and profile any native tool in the agent loop. Validates arguments against schema and reports execution results."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "tool_name": {
                    "type": "string",
                    "description": "Name of the registered tool to test."
                },
                "mock_args": {
                    "type": "object",
                    "description": "JSON arguments to pass to the tool call.",
                    "additionalProperties": true
                }
            },
            "required": ["tool_name"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let (raw_tool_name, mock_args) = match arguments.as_str() {
            Some(s) => (Some(s), serde_json::json!({})),
            None => (
                extract_first_str(arguments, &["tool_name", "toolName", "name", "tool"]),
                ["mock_args", "mockArgs", "args"]
                    .iter()
                    .find_map(|&k| arguments.get(k).cloned())
                    .unwrap_or_else(|| serde_json::json!({})),
            ),
        };
        let raw_tool_name = raw_tool_name.ok_or_else(|| anyhow::anyhow!("Missing tool_name"))?;
        let tool_name = raw_tool_name.trim();
        let mock_args = normalize_diagnose_mock_args(tool_name, mock_args);

        let tool = {
            let filter_scope_backup = {
                let mut g = self
                    .registry
                    .filter_scope
                    .lock()
                    .map_err(|e| anyhow::anyhow!("Lock poisoned: {}", e))?;
                g.take()
            };
            let t = self.registry.get(tool_name);
            match self.registry.filter_scope.lock() {
                Ok(mut g) => *g = filter_scope_backup,
                Err(_) => {}
            }
            t
        };

        let tool = match tool {
            Some(t) => t,
            None => {
                return Ok(serde_json::json!({
                    "success": false,
                    "error": format!("Tool '{}' not found in registry", tool_name)
                }));
            }
        };

        let schema = tool.parameters();
        let start_time = std::time::Instant::now();
        let result = tool.call(&mock_args).await;
        let elapsed_ms = start_time.elapsed().as_millis();

        match result {
            Ok(output) => Ok(serde_json::json!({
                "success": true,
                "duration_ms": elapsed_ms,
                "schema": schema,
                "output": output
            })),
            Err(e) => Ok(serde_json::json!({
                "success": false,
                "duration_ms": elapsed_ms,
                "schema": schema,
                "error": e.to_string()
            })),
        }
    }
}

fn dir_size_and_count(path: &std::path::Path) -> (u64, usize) {
    let mut total_size = 0;
    let mut file_count = 0;
    for entry in std::fs::read_dir(path).into_iter().flatten().flatten() {
        match entry.metadata() {
            Ok(meta) => match meta.is_dir() {
                true => {
                    let (size, count) = dir_size_and_count(&entry.path());
                    total_size += size;
                    file_count += count;
                }
                false => {
                    total_size += meta.len();
                    file_count += 1;
                }
            },
            Err(_) => {}
        }
    }
    (total_size, file_count)
}

async fn check_endpoint_latency(client: &reqwest::Client, url: &str) -> (Option<u128>, String) {
    let start = std::time::Instant::now();
    match client
        .get(url)
        .timeout(std::time::Duration::from_secs(3))
        .send()
        .await
    {
        Ok(resp) => {
            let elapsed = start.elapsed().as_millis();
            let reachable = resp.status().is_success()
                || matches!(resp.status().as_u16(), 400 | 401 | 404);
            let status = match reachable {
                true => "reachable".to_string(),
                false => format!("status {}", resp.status()),
            };
            (Some(elapsed), status)
        }
        Err(e) => (None, format!("error: {}", e)),
    }
}

fn check_db(path: &std::path::Path, run_integrity: bool) -> Value {
    match path.exists() {
        false => serde_json::json!({
            "exists": false,
            "connectable": false,
            "size_bytes": 0,
            "integrity": "N/A"
        }),
        true => {
            let size_bytes = path.metadata().map(|m| m.len()).unwrap_or(0);
            match rusqlite::Connection::open(path) {
                Ok(conn) => {
                    let integrity = match run_integrity {
                        true => match conn.query_row("PRAGMA integrity_check;", [], |row| {
                            row.get::<_, String>(0)
                        }) {
                            Ok(s) => s,
                            Err(e) => format!("check error: {}", e),
                        },
                        false => "skipped".to_string(),
                    };
                    serde_json::json!({
                        "exists": true,
                        "connectable": true,
                        "size_bytes": size_bytes,
                        "integrity": integrity
                    })
                }
                Err(e) => serde_json::json!({
                    "exists": true,
                    "connectable": false,
                    "size_bytes": size_bytes,
                    "integrity": format!("connection error: {}", e)
                }),
            }
        }
    }
}

fn dir_stat_json(path: &std::path::Path) -> Value {
    let (size_bytes, file_count) = dir_size_and_count(path);
    serde_json::json!({
        "path": path.to_string_lossy(),
        "size_bytes": size_bytes,
        "file_count": file_count
    })
}

pub struct DiagnoseSystemTool;

#[async_trait::async_trait]
impl Tool for DiagnoseSystemTool {
    fn name(&self) -> &str {
        "diagnose_system"
    }

    fn description(&self) -> &str {
        "Retrieve system diagnostics for OpenZ, including storage directory sizes, internal database health checks, and active LLM endpoint latencies."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "check_latency": {
                    "type": "boolean",
                    "description": "If true, tests HTTP ping round-trip times to active provider endpoints. Default is true."
                },
                "check_db_integrity": {
                    "type": "boolean",
                    "description": "If true, executes 'PRAGMA integrity_check;' on SQLite files. Can take longer. Default is false."
                }
            }
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let (check_latency, check_db_integrity) = match arguments.as_str() {
            Some(s) => match s.trim().to_lowercase().as_str() {
                "full" | "integrity" => (true, true),
                "quick" | "fast" => (false, false),
                _ => (true, false),
            },
            None => (
                parse_bool_param(
                    arguments,
                    &["check_latency", "checkLatency", "latency"],
                    true,
                ),
                parse_bool_param(
                    arguments,
                    &[
                        "check_db_integrity",
                        "checkDbIntegrity",
                        "integrity",
                        "db_integrity",
                    ],
                    false,
                ),
            ),
        };

        let os_type = std::env::consts::OS.to_string();
        let arch = std::env::consts::ARCH.to_string();
        let cores = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);

        let openz_dir = crate::config::loader::runtime_data_dir();

        let directories = serde_json::json!({
            "sessions": dir_stat_json(&openz_dir.join("sessions")),
            "tool_outputs": dir_stat_json(&openz_dir.join("tool_outputs")),
            "traces": dir_stat_json(&openz_dir.join("traces")),
            "skills": dir_stat_json(&openz_dir.join("skills")),
        });

        let databases = serde_json::json!({
            "memory": check_db(&openz_dir.join("memory.db"), check_db_integrity),
            "docs": check_db(&openz_dir.join("docs.db"), check_db_integrity),
            "graph_memory": check_db(&openz_dir.join("graph_memory.db"), check_db_integrity),
            "ccr_cache": check_db(&openz_dir.join("ccr_cache.db"), check_db_integrity),
            "thoughts": check_db(&openz_dir.join("thoughts.db"), check_db_integrity)
        });

        let mut network_status = serde_json::Map::new();
        match (check_latency, crate::config::loader::load_config()) {
            (true, Ok(config)) => {
                let client = crate::core::http::default_http_client();
                let providers = [
                    ("openai", config.providers.openai.as_ref()),
                    ("anthropic", config.providers.anthropic.as_ref()),
                    ("openrouter", config.providers.openrouter.as_ref()),
                    ("deepseek", config.providers.deepseek.as_ref()),
                ];
                let endpoints: Vec<(&str, String)> = providers
                    .into_iter()
                    .filter_map(|(name, p_opt)| {
                        let p = p_opt?;
                        p.api_key.as_ref().filter(|k| !k.is_empty())?;
                        let base = p.api_base.as_deref().unwrap_or_else(|| {
                            crate::config::provider_catalog::default_api_base_for_provider(name)
                        });
                        Some((name, base.to_string()))
                    })
                    .collect();

                for (name, url) in endpoints {
                    let (latency, status) = check_endpoint_latency(&client, &url).await;
                    network_status.insert(
                        name.to_string(),
                        serde_json::json!({
                            "endpoint": url,
                            "latency_ms": latency,
                            "status": status
                        }),
                    );
                }
            }
            _ => {}
        }

        Ok(serde_json::json!({
            "status": "success",
            "system": {
                "os": os_type,
                "architecture": arch,
                "cores": cores
            },
            "directories": directories,
            "databases": databases,
            "network": network_status
        }))
    }
}

#[cfg(test)]
#[path = "diagnostics_tests.rs"]
mod tests;
