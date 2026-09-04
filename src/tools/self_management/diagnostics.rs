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
    if !is_placeholder_mock_args(&mock_args) {
        return mock_args;
    }

    match tool_name {
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
                    "description": "JSON arguments to pass to the tool call."
                }
            },
            "required": ["tool_name"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let tool_name = arguments
            .get("tool_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing tool_name"))?;
        let mock_args = arguments
            .get("mock_args")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));
        let mock_args = normalize_diagnose_mock_args(tool_name, mock_args);

        // Retrieve tool bypassing filter_scope
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
            if let Ok(mut g) = self.registry.filter_scope.lock() {
                *g = filter_scope_backup;
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
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                if meta.is_dir() {
                    let (size, count) = dir_size_and_count(&entry.path());
                    total_size += size;
                    file_count += count;
                } else {
                    total_size += meta.len();
                    file_count += 1;
                }
            }
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
            let status = if resp.status().is_success()
                || resp.status().as_u16() == 401
                || resp.status().as_u16() == 404
                || resp.status().as_u16() == 400
            {
                "reachable".to_string()
            } else {
                format!("status {}", resp.status())
            };
            (Some(elapsed), status)
        }
        Err(e) => (None, format!("error: {}", e)),
    }
}

fn check_db(path: &std::path::Path, run_integrity: bool) -> Value {
    let exists = path.exists();
    if !exists {
        return serde_json::json!({
            "exists": false,
            "connectable": false,
            "size_bytes": 0,
            "integrity": "N/A"
        });
    }

    let size_bytes = path.metadata().map(|m| m.len()).unwrap_or(0);
    match rusqlite::Connection::open(path) {
        Ok(conn) => {
            let integrity = if run_integrity {
                match conn.query_row("PRAGMA integrity_check;", [], |row| row.get::<_, String>(0)) {
                    Ok(s) => s,
                    Err(e) => format!("check error: {}", e),
                }
            } else {
                "skipped".to_string()
            };
            serde_json::json!({
                "exists": true,
                "connectable": true,
                "size_bytes": size_bytes,
                "integrity": integrity
            })
        }
        Err(e) => {
            serde_json::json!({
                "exists": true,
                "connectable": false,
                "size_bytes": size_bytes,
                "integrity": format!("connection error: {}", e)
            })
        }
    }
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
        let check_latency = arguments
            .get("check_latency")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let check_db_integrity = arguments
            .get("check_db_integrity")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let os_type = std::env::consts::OS.to_string();
        let arch = std::env::consts::ARCH.to_string();
        let cores = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);

        let openz_dir = crate::config::loader::runtime_data_dir();
        let sessions_dir = openz_dir.join("sessions");
        let outputs_dir = openz_dir.join("tool_outputs");
        let traces_dir = openz_dir.join("traces");
        let skills_dir = openz_dir.join("skills");

        let (sessions_sz, sessions_ct) = dir_size_and_count(&sessions_dir);
        let (outputs_sz, outputs_ct) = dir_size_and_count(&outputs_dir);
        let (traces_sz, traces_ct) = dir_size_and_count(&traces_dir);
        let (skills_sz, skills_ct) = dir_size_and_count(&skills_dir);

        let directories = serde_json::json!({
            "sessions": {
                "path": sessions_dir.to_string_lossy(),
                "size_bytes": sessions_sz,
                "file_count": sessions_ct
            },
            "tool_outputs": {
                "path": outputs_dir.to_string_lossy(),
                "size_bytes": outputs_sz,
                "file_count": outputs_ct
            },
            "traces": {
                "path": traces_dir.to_string_lossy(),
                "size_bytes": traces_sz,
                "file_count": traces_ct
            },
            "skills": {
                "path": skills_dir.to_string_lossy(),
                "size_bytes": skills_sz,
                "file_count": skills_ct
            }
        });

        let db_memory = openz_dir.join("memory.db");
        let db_docs = openz_dir.join("docs.db");
        let db_graph = openz_dir.join("graph_memory.db");
        let db_ccr = openz_dir.join("ccr_cache.db");
        let db_thoughts = openz_dir.join("thoughts.db");

        let databases = serde_json::json!({
            "memory": check_db(&db_memory, check_db_integrity),
            "docs": check_db(&db_docs, check_db_integrity),
            "graph_memory": check_db(&db_graph, check_db_integrity),
            "ccr_cache": check_db(&db_ccr, check_db_integrity),
            "thoughts": check_db(&db_thoughts, check_db_integrity)
        });

        let mut network_status = serde_json::Map::new();
        if check_latency {
            if let Ok(config) = crate::config::loader::load_config() {
                let client = crate::core::http::default_http_client();
                let mut endpoints = Vec::new();

                if let Some(ref openai) = config.providers.openai {
                    if let Some(ref api_key) = openai.api_key {
                        if !api_key.is_empty() {
                            let base = openai
                                .api_base
                                .as_deref()
                                .unwrap_or_else(|| {
                                    crate::config::provider_catalog::default_api_base_for_provider(
                                        "openai",
                                    )
                                });
                            endpoints.push(("openai", base.to_string()));
                        }
                    }
                }
                if let Some(ref anthropic) = config.providers.anthropic {
                    if let Some(ref api_key) = anthropic.api_key {
                        if !api_key.is_empty() {
                            let base = anthropic
                                .api_base
                                .as_deref()
                                .unwrap_or_else(|| {
                                    crate::config::provider_catalog::default_api_base_for_provider(
                                        "anthropic",
                                    )
                                });
                            endpoints.push(("anthropic", base.to_string()));
                        }
                    }
                }
                if let Some(ref openrouter) = config.providers.openrouter {
                    if let Some(ref api_key) = openrouter.api_key {
                        if !api_key.is_empty() {
                            let base = openrouter
                                .api_base
                                .as_deref()
                                .unwrap_or_else(|| {
                                    crate::config::provider_catalog::default_api_base_for_provider(
                                        "openrouter",
                                    )
                                });
                            endpoints.push(("openrouter", base.to_string()));
                        }
                    }
                }
                if let Some(ref deepseek) = config.providers.deepseek {
                    if let Some(ref api_key) = deepseek.api_key {
                        if !api_key.is_empty() {
                            let base = deepseek
                                .api_base
                                .as_deref()
                                .unwrap_or_else(|| {
                                    crate::config::provider_catalog::default_api_base_for_provider(
                                        "deepseek",
                                    )
                                });
                            endpoints.push(("deepseek", base.to_string()));
                        }
                    }
                }

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
