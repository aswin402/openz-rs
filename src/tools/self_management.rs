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

fn normalize_diagnose_mock_args(tool_name: &str, mock_args: Value) -> Value {
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

pub struct ToolCatalogTool {
    registry: crate::tools::ToolRegistry,
}

impl ToolCatalogTool {
    pub fn new(registry: crate::tools::ToolRegistry) -> Self {
        Self { registry }
    }
}

#[async_trait::async_trait]
impl Tool for ToolCatalogTool {
    fn name(&self) -> &str {
        "tool_catalog"
    }

    fn description(&self) -> &str {
        "List registered native tools with domain, risk, resource metadata, and whether each tool is currently exposed to the model."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "domain": {
                    "type": "string",
                    "description": "Optional domain filter, such as filesystem, shell, web, subagent, memory, document, media, or self_management."
                },
                "risk": {
                    "type": "string",
                    "enum": ["low", "medium", "high"],
                    "description": "Optional risk filter."
                },
                "include_schema": {
                    "type": "boolean",
                    "description": "Include each tool JSON parameter schema. Default false to keep output compact."
                },
                "only_exposed": {
                    "type": "boolean",
                    "description": "If true, only return tools currently included in the provider tool payload."
                },
                "prompt": {
                    "type": "string",
                    "description": "Optional prompt/task text to explain prompt-aware tool routing for that turn."
                },
                "resource_overrides": {
                    "type": "object",
                    "description": "Optional diagnostic overrides for resource-policy preview, such as allow_network_tools, min_free_disk_gb, free_disk_gb, active_process_tools, max_concurrent_process_tools, and warn_before_expensive_tools."
                }
            }
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let include_schema = arguments
            .get("include_schema")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let only_exposed = arguments
            .get("only_exposed")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let domain_filter = arguments.get("domain").and_then(|v| v.as_str());
        let risk_filter = arguments.get("risk").and_then(|v| v.as_str());
        let prompt = arguments
            .get("prompt")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let mut defaults = crate::config::loader::load_config()
            .map(|config| config.agents.defaults)
            .unwrap_or_default();
        let mut runtime = crate::tools::resource_policy::RuntimeResourceSnapshot::current();
        if let Some(overrides) = arguments
            .get("resource_overrides")
            .and_then(|v| v.as_object())
        {
            if let Some(value) = overrides
                .get("allow_network_tools")
                .and_then(|v| v.as_bool())
            {
                defaults.allow_network_tools = value;
            }
            if let Some(value) = overrides.get("min_free_disk_gb").and_then(|v| v.as_f64()) {
                defaults.min_free_disk_gb = value;
            }
            if let Some(value) = overrides
                .get("max_concurrent_process_tools")
                .and_then(|v| v.as_u64())
            {
                defaults.max_concurrent_process_tools = value as usize;
            }
            if let Some(value) = overrides
                .get("warn_before_expensive_tools")
                .and_then(|v| v.as_bool())
            {
                defaults.warn_before_expensive_tools = value;
            }
            if let Some(value) = overrides.get("free_disk_gb").and_then(|v| v.as_f64()) {
                runtime.free_disk_gb = Some(value);
            }
            if let Some(value) = overrides
                .get("active_process_tools")
                .and_then(|v| v.as_u64())
            {
                runtime.active_process_tools = value as usize;
            }
        }

        let mut all_entries = self
            .registry
            .catalog_entries_for_prompt(include_schema, prompt);
        for entry in &mut all_entries {
            let risk = match entry["risk"].as_str().unwrap_or("low") {
                "high" => crate::tools::ToolRisk::High,
                "medium" => crate::tools::ToolRisk::Medium,
                _ => crate::tools::ToolRisk::Low,
            };
            let metadata = crate::tools::ToolMetadata {
                domain: "general",
                risk,
                uses_network: entry["uses_network"].as_bool().unwrap_or(false),
                writes_disk: entry["writes_disk"].as_bool().unwrap_or(false),
                spawns_process: entry["spawns_process"].as_bool().unwrap_or(false),
                requires_approval: entry["requires_approval"].as_bool().unwrap_or(false),
                priority: entry["priority"].as_u64().unwrap_or(0) as u8,
                aliases: &[],
                examples: &[],
                when_to_use: "",
                when_not_to_use: "",
                recommended_timeout_secs: None,
            };
            let decision = crate::tools::resource_policy::ToolResourcePolicy::evaluate(
                &metadata, &defaults, &runtime,
            );
            entry["resource_policy"] = serde_json::json!({
                "decision": decision.as_str(),
                "reason": decision.reason(),
                "free_disk_gb": runtime.free_disk_gb,
                "active_process_tools": runtime.active_process_tools,
                "min_free_disk_gb": defaults.min_free_disk_gb,
                "allow_network_tools": defaults.allow_network_tools,
                "max_concurrent_process_tools": defaults.max_concurrent_process_tools,
                "warn_before_expensive_tools": defaults.warn_before_expensive_tools,
            });
        }
        let exposed_count = all_entries
            .iter()
            .filter(|entry| entry["exposed_to_model"].as_bool().unwrap_or(false))
            .count();
        let entries: Vec<Value> = all_entries
            .into_iter()
            .filter(|entry| {
                if only_exposed && !entry["exposed_to_model"].as_bool().unwrap_or(false) {
                    return false;
                }
                if let Some(domain) = domain_filter {
                    if entry["domain"].as_str() != Some(domain) {
                        return false;
                    }
                }
                if let Some(risk) = risk_filter {
                    if entry["risk"].as_str() != Some(risk) {
                        return false;
                    }
                }
                true
            })
            .collect();

        let mut domains = std::collections::BTreeMap::<String, usize>::new();
        let mut risks = std::collections::BTreeMap::<String, usize>::new();
        for entry in &entries {
            if let Some(domain) = entry["domain"].as_str() {
                *domains.entry(domain.to_string()).or_default() += 1;
            }
            if let Some(risk) = entry["risk"].as_str() {
                *risks.entry(risk.to_string()).or_default() += 1;
            }
        }

        Ok(serde_json::json!({
            "success": true,
            "tool_count": entries.len(),
            "exposed_count": exposed_count,
            "selected_domains": self.registry.selected_domains_for_prompt(prompt),
            "domains": domains,
            "risks": risks,
            "tools": entries
        }))
    }
}

pub struct OpenZInventoryTool {
    registry: crate::tools::ToolRegistry,
}

impl OpenZInventoryTool {
    pub fn new(registry: crate::tools::ToolRegistry) -> Self {
        Self { registry }
    }
}

#[async_trait::async_trait]
impl Tool for OpenZInventoryTool {
    fn name(&self) -> &str {
        "openz_inventory"
    }

    fn description(&self) -> &str {
        "Return a live OpenZ capability inventory from the running binary: version, runtime model/provider identity, channels, registered tools by domain, subagents, server state, and exact counts. Use this before answering feature/tool/model identity questions."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "include_tools": {
                    "type": "boolean",
                    "description": "Include the full registered tool name list. Default true."
                },
                "include_subagents": {
                    "type": "boolean",
                    "description": "Include loaded subagent profile names. Default true."
                },
                "prompt": {
                    "type": "string",
                    "description": "Optional user prompt for prompt-aware exposed-tool routing analysis."
                }
            }
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let include_tools = arguments
            .get("include_tools")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let include_subagents = arguments
            .get("include_subagents")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let prompt = arguments
            .get("prompt")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let entries = self.registry.catalog_entries_for_prompt(false, prompt);
        let mut domains = std::collections::BTreeMap::<String, usize>::new();
        let mut risks = std::collections::BTreeMap::<String, usize>::new();
        let mut tools_by_domain = std::collections::BTreeMap::<String, Vec<String>>::new();
        let mut exposed_count = 0usize;
        for entry in &entries {
            let name = entry["name"].as_str().unwrap_or("unknown").to_string();
            let domain = entry["domain"].as_str().unwrap_or("general").to_string();
            let risk = entry["risk"].as_str().unwrap_or("low").to_string();
            if entry["exposed_to_model"].as_bool().unwrap_or(false) {
                exposed_count += 1;
            }
            *domains.entry(domain.clone()).or_default() += 1;
            *risks.entry(risk).or_default() += 1;
            tools_by_domain.entry(domain).or_default().push(name);
        }
        for names in tools_by_domain.values_mut() {
            names.sort();
        }

        let subagents = if include_subagents {
            crate::subagents::load_profiles()
                .map(|profiles| profiles.into_iter().map(|p| p.name).collect::<Vec<_>>())
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        let active_servers = crate::shutdown::list_registered_children();

        let runtime_identity = match crate::config::loader::load_config() {
            Ok(config) => {
                let configured_model = config.agents.defaults.model.clone();
                let configured_provider = config.agents.defaults.provider.clone();
                let resolved =
                    crate::providers::resolver::resolve_provider_full(&config, &configured_model);
                let (effective_provider, effective_model, provider_resolution_error) =
                    match resolved {
                        Ok(resolved) => (
                            Some(resolved.provider_name),
                            Some(resolved.model),
                            serde_json::Value::Null,
                        ),
                        Err(err) => (None, None, serde_json::json!(err.to_string())),
                    };
                serde_json::json!({
                    "configured_model": configured_model,
                    "configured_provider": configured_provider,
                    "effective_provider": effective_provider,
                    "effective_model": effective_model,
                    "provider_resolution_error": provider_resolution_error,
                    "model_supports_vision": crate::providers::model_supports_vision(&config.agents.defaults.model),
                    "caveman_mode": config.agents.defaults.caveman_mode,
                    "streaming": config.agents.defaults.streaming,
                    "note": "Runtime config exposes model/provider labels, not hidden architecture, training data, parameter count, or benchmark ranking."
                })
            }
            Err(err) => serde_json::json!({
                "error": err.to_string(),
                "note": "Runtime config could not be loaded; do not guess model/provider identity."
            }),
        };

        let channels = vec![
            "cli_tui",
            "websocket_webui",
            "telegram",
            "discord",
            "whatsapp",
            "email_imap_smtp",
        ];
        let commands = vec![
            "onboard",
            "configure",
            "agent",
            "gateway",
            "telegram",
            "discord",
            "whatsapp",
            "subagent",
            "sop",
            "mcp-bridge",
            "logs",
            "changelog",
            "streaming",
            "doctor",
        ];
        let core_capabilities = vec![
            "agent_loop",
            "prompt_aware_tool_router",
            "managed_dev_server_lifecycle",
            "self_improvement_curator",
            "persistent_skills",
            "workflow_memory",
            "knowledge_graph_memory",
            "working_memory_ttl",
            "semantic_search",
            "context_compression_ccr",
            "subagent_delegation",
            "sop_workflows",
            "security_guard",
            "optional_seccomp_bpf_sandbox",
            "audit_ledger",
            "mcp_bridge",
            "browser_automation",
            "document_tools",
            "media_tools",
            "rust_cargo_tools",
        ];

        Ok(serde_json::json!({
            "success": true,
            "version": env!("CARGO_PKG_VERSION"),
            "runtime_identity": runtime_identity,
            "tool_count": entries.len(),
            "exposed_count": exposed_count,
            "domains": domains,
            "risks": risks,
            "selected_domains": self.registry.selected_domains_for_prompt(prompt),
            "channels": channels,
            "channel_count": channels.len(),
            "commands": commands,
            "command_count": commands.len(),
            "core_capabilities": core_capabilities,
            "subagent_count": subagents.len(),
            "subagents": if include_subagents { serde_json::json!(subagents) } else { serde_json::Value::Null },
            "active_server_count": active_servers.len(),
            "active_servers": active_servers.iter().map(|s| serde_json::json!({
                "id": s.id,
                "pid": s.pid,
                "kind": s.kind,
                "command": s.command,
                "started_at": s.started_at,
            })).collect::<Vec<_>>(),
            "tools_by_domain": if include_tools { serde_json::json!(tools_by_domain) } else { serde_json::Value::Null },
            "guidance": "Use this live inventory instead of guessing feature/tool counts or model/provider identity from memory."
        }))
    }
}

pub struct CurateSkillTool;

#[async_trait::async_trait]
impl Tool for CurateSkillTool {
    fn name(&self) -> &str {
        "curate_skill"
    }

    fn description(&self) -> &str {
        "Curate, list, add, or delete procedural skills and guidelines in the OpenZ skills SQLite database."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["list", "add", "delete"],
                    "description": "The curation action to perform."
                },
                "skill_name": {
                    "type": "string",
                    "description": "Name/identifier of the skill (e.g. 'rust-compilation-tricks')."
                },
                "content": {
                    "type": "string",
                    "description": "Markdown instructions/guidelines for the skill. Required for 'add'."
                },
                "profile": {
                    "type": "string",
                    "description": "Optional subagent profile name to restrict this skill to."
                }
            },
            "required": ["action"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let action = arguments
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing action"))?;

        match action {
            "list" => {
                let profile = arguments.get("profile").and_then(|v| v.as_str());
                let skills = crate::agent::skills::load_skills_with_profile(profile)?;
                Ok(serde_json::json!({
                    "success": true,
                    "skills": skills
                }))
            }
            "add" => {
                let skill_name = arguments
                    .get("skill_name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing skill_name for action 'add'"))?;
                let content = arguments
                    .get("content")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing content for action 'add'"))?;
                let profile = arguments.get("profile").and_then(|v| v.as_str());

                if let Some(prof) = profile {
                    crate::agent::skills::save_subagent_skill(prof, skill_name, content)?;
                } else {
                    crate::agent::skills::save_skill(skill_name, content)?;
                }

                Ok(serde_json::json!({
                    "success": true,
                    "message": format!("Skill '{}' successfully saved", skill_name)
                }))
            }
            "delete" => {
                let skill_name = arguments
                    .get("skill_name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing skill_name for action 'delete'"))?;
                let profile = arguments.get("profile").and_then(|v| v.as_str());

                crate::agent::skills::delete_skill_with_profile(skill_name, profile)?;
                Ok(serde_json::json!({
                    "success": true,
                    "message": format!("Skill '{}' successfully deleted", skill_name)
                }))
            }
            _ => Err(anyhow::anyhow!("Invalid action")),
        }
    }
}

pub struct OptimizeToolScopeTool {
    registry: crate::tools::ToolRegistry,
}

impl OptimizeToolScopeTool {
    pub fn new(registry: crate::tools::ToolRegistry) -> Self {
        Self { registry }
    }
}

#[async_trait::async_trait]
impl Tool for OptimizeToolScopeTool {
    fn name(&self) -> &str {
        "optimize_tool_scope"
    }

    fn description(&self) -> &str {
        "Restrict or reset the set of active tool prefixes exposed to the agent loop to reduce prompt size and prevent hallucinations."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "active_prefixes": {
                    "type": "array",
                    "items": {
                        "type": "string"
                    },
                    "description": "List of prefix strings to restrict the scope to (e.g. ['fs_', 'opendoc_']). Pass null or empty list to reset."
                }
            }
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let prefixes = arguments.get("active_prefixes").and_then(|v| v.as_array());

        match prefixes {
            Some(arr) if !arr.is_empty() => {
                let prefixes_vec: Vec<String> = arr
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect();
                self.registry.set_filter_scope(Some(prefixes_vec.clone()));
                Ok(serde_json::json!({
                    "success": true,
                    "message": format!("Tool scope restricted to prefixes: {:?}", prefixes_vec)
                }))
            }
            _ => {
                self.registry.set_filter_scope(None);
                Ok(serde_json::json!({
                    "success": true,
                    "message": "Tool scope filter reset; all tools are now active."
                }))
            }
        }
    }
}

fn redact_secrets(val: &mut Value) {
    match val {
        Value::Object(map) => {
            for (k, v) in map.iter_mut() {
                let lower = k.to_lowercase();
                if lower.contains("api_key")
                    || lower.contains("bot_token")
                    || lower.contains("verify_token")
                    || lower.contains("password")
                    || lower.contains("secret")
                {
                    if v.is_string() {
                        *v = Value::String("********".to_string());
                    }
                } else {
                    redact_secrets(v);
                }
            }
        }
        Value::Array(arr) => {
            for item in arr {
                redact_secrets(item);
            }
        }
        _ => {}
    }
}

pub struct ManageConfigTool;

#[async_trait::async_trait]
impl Tool for ManageConfigTool {
    fn name(&self) -> &str {
        "manage_config"
    }

    fn description(&self) -> &str {
        "View the active configuration (redacting secrets) or modify default hyperparameters (model, temperature, max tokens, caveman mode) to optimize agent behavior."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["view", "update"],
                    "description": "Whether to view the current configuration or update hyperparameters."
                },
                "updates": {
                    "type": "object",
                    "properties": {
                        "model": {
                            "type": "string",
                            "description": "Default model prefix to use (e.g. 'openai/gpt-4o')."
                        },
                        "provider": {
                            "type": "string",
                            "description": "Default provider (e.g. 'openai')."
                        },
                        "max_tokens": {
                            "type": "integer",
                            "description": "Maximum completion tokens."
                        },
                        "temperature": {
                            "type": "number",
                            "description": "Generation temperature."
                        },
                        "caveman_mode": {
                            "type": "boolean",
                            "description": "Toggles terse/concise system prompt instructions."
                        },
                        "tool_timeout_secs": {
                            "type": "integer",
                            "description": "Max timeout for tool executions."
                        },
                        "streaming": {
                            "type": "boolean",
                            "description": "Enable/disable token response streaming."
                        },
                        "show_tool_router_status": {
                            "type": "boolean",
                            "description": "Show compact tool-router selection summaries in the TUI."
                        },
                        "min_free_disk_gb": {
                            "type": "number",
                            "description": "Minimum free disk space required before disk-writing tools run."
                        },
                        "allow_network_tools": {
                            "type": "boolean",
                            "description": "Allow tools marked as network-capable to run."
                        },
                        "max_concurrent_process_tools": {
                            "type": "integer",
                            "description": "Maximum active process-spawning tools allowed before new process tools are blocked."
                        },
                        "warn_before_expensive_tools": {
                            "type": "boolean",
                            "description": "Require approval before tools that combine network/process/disk behavior."
                        },
                        "max_tool_iterations": {
                            "type": "integer",
                            "description": "Maximum execution steps per turn."
                        },
                        "skills_workspace_skills_enabled": {
                            "type": "boolean",
                            "description": "Enable workspace-scoped skills from .openz/skills."
                        },
                        "skills_external_dirs": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Additional skill directories to scan after the OpenZ skill store."
                        },
                        "skills_write_approval": {
                            "type": "boolean",
                            "description": "Require approval/staging before agent-created skill writes."
                        }
                    },
                    "description": "Key-value map of configuration defaults to update. Ignored for action 'view'."
                }
            },
            "required": ["action"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let action = arguments
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing action"))?;

        match action {
            "view" => {
                let config = crate::config::loader::load_config()?;
                let mut config_val = serde_json::to_value(&config)?;
                redact_secrets(&mut config_val);
                Ok(serde_json::json!({
                    "success": true,
                    "config": config_val
                }))
            }
            "update" => {
                let updates = arguments
                    .get("updates")
                    .and_then(|v| v.as_object())
                    .ok_or_else(|| anyhow::anyhow!("Missing updates for action 'update'"))?;

                let mut config = crate::config::loader::load_config()?;

                for (k, v) in updates {
                    match k.as_str() {
                        "model" => {
                            if let Some(s) = v.as_str() {
                                config.agents.defaults.model = s.to_string();
                            }
                        }
                        "provider" => {
                            if let Some(s) = v.as_str() {
                                config.agents.defaults.provider = s.to_string();
                            }
                        }
                        "max_tokens" => {
                            if let Some(n) = v.as_u64() {
                                config.agents.defaults.max_tokens = n as usize;
                            }
                        }
                        "temperature" => {
                            if let Some(f) = v.as_f64() {
                                config.agents.defaults.temperature = f as f32;
                            }
                        }
                        "caveman_mode" => {
                            if let Some(b) = v.as_bool() {
                                config.agents.defaults.caveman_mode = b;
                            }
                        }
                        "tool_timeout_secs" => {
                            if let Some(n) = v.as_u64() {
                                config.agents.defaults.tool_timeout_secs = n;
                            }
                        }
                        "streaming" => {
                            if let Some(b) = v.as_bool() {
                                config.agents.defaults.streaming = b;
                            }
                        }
                        "show_tool_router_status" => {
                            if let Some(b) = v.as_bool() {
                                config.agents.defaults.show_tool_router_status = b;
                            }
                        }
                        "min_free_disk_gb" => {
                            if let Some(n) = v.as_f64() {
                                config.agents.defaults.min_free_disk_gb = n;
                            }
                        }
                        "allow_network_tools" => {
                            if let Some(b) = v.as_bool() {
                                config.agents.defaults.allow_network_tools = b;
                            }
                        }
                        "max_concurrent_process_tools" => {
                            if let Some(n) = v.as_u64() {
                                config.agents.defaults.max_concurrent_process_tools = n as usize;
                            }
                        }
                        "warn_before_expensive_tools" => {
                            if let Some(b) = v.as_bool() {
                                config.agents.defaults.warn_before_expensive_tools = b;
                            }
                        }
                        "max_tool_iterations" => {
                            if let Some(n) = v.as_u64() {
                                config.agents.defaults.max_tool_iterations = n as usize;
                            }
                        }
                        "skills_workspace_skills_enabled" => {
                            if let Some(b) = v.as_bool() {
                                config.skills.workspace_skills_enabled = b;
                            }
                        }
                        "skills_external_dirs" => {
                            if let Some(values) = v.as_array() {
                                config.skills.external_dirs = values
                                    .iter()
                                    .filter_map(|value| value.as_str().map(str::to_string))
                                    .collect();
                            }
                        }
                        "skills_write_approval" => {
                            if let Some(b) = v.as_bool() {
                                config.skills.write_approval = b;
                            }
                        }
                        other => {
                            return Ok(serde_json::json!({
                                "success": false,
                                "error": format!("Cannot modify field '{}' via manage_config", other)
                            }));
                        }
                    }
                }

                crate::config::loader::save_config(&config)?;
                Ok(serde_json::json!({
                    "success": true,
                    "message": "Configuration successfully updated."
                }))
            }
            _ => Err(anyhow::anyhow!("Invalid action")),
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
                let client = reqwest::Client::new();
                let mut endpoints = Vec::new();

                if let Some(ref openai) = config.providers.openai {
                    if let Some(ref api_key) = openai.api_key {
                        if !api_key.is_empty() {
                            let base = openai
                                .api_base
                                .as_deref()
                                .unwrap_or("https://api.openai.com/v1");
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
                                .unwrap_or("https://api.anthropic.com/v1");
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
                                .unwrap_or("https://openrouter.ai/api/v1");
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
                                .unwrap_or("https://api.deepseek.com");
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

pub struct ManageSessionsTool;

#[async_trait::async_trait]
impl Tool for ManageSessionsTool {
    fn name(&self) -> &str {
        "manage_sessions"
    }

    fn description(&self) -> &str {
        "Manage, clean up, or archive session history files and temporary tool outputs to prevent disk space exhaustion."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["list", "prune", "archive", "delete"],
                    "description": "The curation action to perform."
                },
                "session_key": {
                    "type": "string",
                    "description": "Required for 'archive' or 'delete'. The key of the session to target."
                },
                "older_than_days": {
                    "type": "integer",
                    "description": "Optional for 'prune'. Delete tool output files older than this number of days. Default is 7."
                }
            },
            "required": ["action"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let action = arguments
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'action'"))?;
        let openz_dir = crate::config::loader::runtime_data_dir();
        let sessions_dir = openz_dir.join("sessions");

        match action {
            "list" => {
                let mut sessions_list = Vec::new();
                if let Ok(entries) = std::fs::read_dir(&sessions_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_file()
                            && path.extension().and_then(|s| s.to_str()) == Some("json")
                        {
                            let session_key = path
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or("")
                                .to_string();
                            let size_bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
                            let last_updated = entry
                                .metadata()
                                .and_then(|m| m.modified())
                                .map(|t| {
                                    let dt: chrono::DateTime<chrono::Utc> = t.into();
                                    dt.to_rfc3339()
                                })
                                .unwrap_or_default();

                            let mut msg_count = 0;
                            if let Ok(content) = std::fs::read_to_string(&path) {
                                if let Ok(val) = serde_json::from_str::<Value>(&content) {
                                    if let Some(msgs) =
                                        val.get("messages").and_then(|v| v.as_array())
                                    {
                                        msg_count = msgs.len();
                                    }
                                }
                            }

                            sessions_list.push(serde_json::json!({
                                "session_key": session_key,
                                "size_bytes": size_bytes,
                                "message_count": msg_count,
                                "last_updated": last_updated
                            }));
                        }
                    }
                }
                Ok(serde_json::json!({
                    "status": "success",
                    "sessions": sessions_list
                }))
            }
            "prune" => {
                crate::tools::subagent::cleanup_stale_resources();

                let older_than_days = arguments
                    .get("older_than_days")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(7);
                let outputs_dir = openz_dir.join("tool_outputs");
                let mut files_removed = 0;
                let mut bytes_reclaimed = 0;

                if let Ok(entries) = std::fs::read_dir(&outputs_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
                        if path.is_file() && name.starts_with("output_") && name.ends_with(".json")
                        {
                            if let Ok(metadata) = entry.metadata() {
                                if let Ok(modified) = metadata.modified() {
                                    if let Ok(elapsed) = modified.elapsed() {
                                        if elapsed.as_secs() > older_than_days * 86400 {
                                            let size = metadata.len();
                                            if std::fs::remove_file(&path).is_ok() {
                                                files_removed += 1;
                                                bytes_reclaimed += size;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                Ok(serde_json::json!({
                    "status": "success",
                    "message": format!("Successfully pruned {} output files and ran stale worktree cleanup.", files_removed),
                    "details": {
                        "files_removed": files_removed,
                        "bytes_reclaimed": bytes_reclaimed,
                        "stale_worktree_cleanup_ran": true
                    }
                }))
            }
            "archive" => {
                let session_key = arguments
                    .get("session_key")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing 'session_key' for action 'archive'"))?;
                let session_file = sessions_dir.join(format!("{}.json", session_key));
                let lock_file = sessions_dir.join(format!("{}.lock", session_key));

                if !session_file.exists() {
                    return Err(anyhow::anyhow!("Session '{}' does not exist.", session_key));
                }

                let archives_dir = openz_dir.join("archives");
                std::fs::create_dir_all(&archives_dir)?;

                let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
                let archive_file = archives_dir.join(format!("{}_{}.json", session_key, timestamp));

                let size = session_file.metadata().map(|m| m.len()).unwrap_or(0);
                std::fs::copy(&session_file, &archive_file)?;
                std::fs::remove_file(&session_file)?;
                if lock_file.exists() {
                    let _ = std::fs::remove_file(&lock_file);
                }

                Ok(serde_json::json!({
                    "status": "success",
                    "message": format!("Session '{}' successfully archived.", session_key),
                    "details": {
                        "files_removed": 1,
                        "bytes_reclaimed": size
                    }
                }))
            }
            "delete" => {
                let session_key = arguments
                    .get("session_key")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing 'session_key' for action 'delete'"))?;
                let session_file = sessions_dir.join(format!("{}.json", session_key));
                let lock_file = sessions_dir.join(format!("{}.lock", session_key));

                if !session_file.exists() {
                    return Err(anyhow::anyhow!("Session '{}' does not exist.", session_key));
                }

                let size = session_file.metadata().map(|m| m.len()).unwrap_or(0);
                std::fs::remove_file(&session_file)?;
                if lock_file.exists() {
                    let _ = std::fs::remove_file(&lock_file);
                }

                Ok(serde_json::json!({
                    "status": "success",
                    "message": format!("Session '{}' permanently deleted.", session_key),
                    "details": {
                        "files_removed": 1,
                        "bytes_reclaimed": size
                    }
                }))
            }
            _ => Err(anyhow::anyhow!("Invalid action")),
        }
    }
}

pub struct ManageBackupsTool;

#[async_trait::async_trait]
impl Tool for ManageBackupsTool {
    fn name(&self) -> &str {
        "manage_backups"
    }

    fn description(&self) -> &str {
        "Create, list, restore, or delete backups of the agent configuration, subagent profiles, and markdown skills."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["create", "list", "restore", "delete"],
                    "description": "The backup curation action to perform."
                },
                "backup_name": {
                    "type": "string",
                    "description": "Required for 'restore' or 'delete'. The filename of the backup target."
                }
            },
            "required": ["action"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let action = arguments
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'action'"))?;
        let openz_dir = crate::config::loader::runtime_data_dir();
        let backups_dir = openz_dir.join("backups");

        match action {
            "create" => {
                std::fs::create_dir_all(&backups_dir)?;

                let config_file = openz_dir.join("config.json");
                let subagents_file = openz_dir.join("subagents.json");
                let skills_dir = openz_dir.join("skills");

                let config_val = if config_file.exists() {
                    let content = std::fs::read_to_string(&config_file)?;
                    serde_json::from_str::<Value>(&content).unwrap_or(Value::Null)
                } else {
                    Value::Null
                };

                let subagents_val = if subagents_file.exists() {
                    let content = std::fs::read_to_string(&subagents_file)?;
                    serde_json::from_str::<Value>(&content).unwrap_or(Value::Null)
                } else {
                    Value::Null
                };

                let mut skills_map = serde_json::Map::new();
                if skills_dir.exists() {
                    if let Ok(entries) = std::fs::read_dir(&skills_dir) {
                        for entry in entries.flatten() {
                            let path = entry.path();
                            if path.is_file()
                                && path.extension().and_then(|s| s.to_str()) == Some("md")
                            {
                                if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                                    if let Ok(content) = std::fs::read_to_string(&path) {
                                        skills_map.insert(name.to_string(), Value::String(content));
                                    }
                                }
                            }
                        }
                    }
                }

                let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
                let backup_filename = format!("backup_{}.json", timestamp);
                let backup_path = backups_dir.join(&backup_filename);

                let backup_data = serde_json::json!({
                    "version": "1.0",
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                    "config": config_val,
                    "subagents": subagents_val,
                    "skills": skills_map
                });

                std::fs::write(&backup_path, serde_json::to_string_pretty(&backup_data)?)?;

                Ok(serde_json::json!({
                    "status": "success",
                    "message": format!("Backup successfully created: {}", backup_filename),
                    "backup_name": backup_filename
                }))
            }
            "list" => {
                let mut backups_list = Vec::new();
                if backups_dir.exists() {
                    if let Ok(entries) = std::fs::read_dir(&backups_dir) {
                        for entry in entries.flatten() {
                            let path = entry.path();
                            let name = path
                                .file_name()
                                .and_then(|s| s.to_str())
                                .unwrap_or("")
                                .to_string();
                            if path.is_file()
                                && name.starts_with("backup_")
                                && name.ends_with(".json")
                            {
                                let size_bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
                                let created_at = entry
                                    .metadata()
                                    .and_then(|m| m.modified())
                                    .map(|t| {
                                        let dt: chrono::DateTime<chrono::Utc> = t.into();
                                        dt.to_rfc3339()
                                    })
                                    .unwrap_or_default();

                                backups_list.push(serde_json::json!({
                                    "backup_name": name,
                                    "size_bytes": size_bytes,
                                    "created_at": created_at
                                }));
                            }
                        }
                    }
                }
                backups_list.sort_by(|a, b| {
                    b["backup_name"]
                        .as_str()
                        .unwrap_or("")
                        .cmp(a["backup_name"].as_str().unwrap_or(""))
                });

                Ok(serde_json::json!({
                    "status": "success",
                    "backups": backups_list
                }))
            }
            "restore" => {
                let backup_name = arguments
                    .get("backup_name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing 'backup_name' for restore action"))?;

                if backup_name.contains('/')
                    || backup_name.contains('\\')
                    || backup_name.contains("..")
                {
                    return Err(anyhow::anyhow!("Invalid backup_name specified."));
                }

                let backup_path = backups_dir.join(backup_name);
                if !backup_path.exists() {
                    return Err(anyhow::anyhow!(
                        "Backup file '{}' does not exist.",
                        backup_name
                    ));
                }

                let content = std::fs::read_to_string(&backup_path)?;
                let backup_data: Value = serde_json::from_str(&content)?;

                if let Some(config_val) = backup_data.get("config") {
                    if !config_val.is_null() {
                        let config_file = openz_dir.join("config.json");
                        std::fs::write(&config_file, serde_json::to_string_pretty(config_val)?)?;
                    }
                }

                if let Some(subagents_val) = backup_data.get("subagents") {
                    if !subagents_val.is_null() {
                        let subagents_file = openz_dir.join("subagents.json");
                        std::fs::write(
                            &subagents_file,
                            serde_json::to_string_pretty(subagents_val)?,
                        )?;
                    }
                }

                if let Some(skills_val) = backup_data.get("skills").and_then(|v| v.as_object()) {
                    let skills_dir = openz_dir.join("skills");
                    std::fs::create_dir_all(&skills_dir)?;
                    for (name, content_val) in skills_val {
                        if let Some(content_str) = content_val.as_str() {
                            let skill_path = skills_dir.join(name);
                            std::fs::write(&skill_path, content_str)?;
                        }
                    }
                }

                Ok(serde_json::json!({
                    "status": "success",
                    "message": format!("Backup '{}' successfully restored.", backup_name)
                }))
            }
            "delete" => {
                let backup_name = arguments
                    .get("backup_name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing 'backup_name' for delete action"))?;

                if backup_name.contains('/')
                    || backup_name.contains('\\')
                    || backup_name.contains("..")
                {
                    return Err(anyhow::anyhow!("Invalid backup_name specified."));
                }

                let backup_path = backups_dir.join(backup_name);
                if !backup_path.exists() {
                    return Err(anyhow::anyhow!(
                        "Backup file '{}' does not exist.",
                        backup_name
                    ));
                }

                std::fs::remove_file(&backup_path)?;

                Ok(serde_json::json!({
                    "status": "success",
                    "message": format!("Backup '{}' successfully deleted.", backup_name)
                }))
            }
            _ => Err(anyhow::anyhow!("Invalid action")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestEnvLock;

    impl TestEnvLock {
        fn acquire() -> Self {
            let lock_path = std::env::temp_dir().join("openz_test_config_dir.lock");
            loop {
                match std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&lock_path)
                {
                    Ok(_) => break,
                    Err(_) => std::thread::sleep(std::time::Duration::from_millis(50)),
                }
            }
            TestEnvLock
        }
    }

    impl Drop for TestEnvLock {
        fn drop(&mut self) {
            let lock_path = std::env::temp_dir().join("openz_test_config_dir.lock");
            let _ = std::fs::remove_file(lock_path);
        }
    }

    #[test]
    fn test_tool_catalog_reports_metadata_and_exposure() {
        let registry = crate::tools::ToolRegistry::new();
        registry.register(std::sync::Arc::new(crate::tools::shell::ExecCommandTool));
        registry.register(std::sync::Arc::new(crate::tools::filesystem::ReadFileTool));

        let catalog = ToolCatalogTool::new(registry);
        let res = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(catalog.call(&serde_json::json!({
                "include_schema": false,
                "prompt": "run cargo test and inspect files"
            })))
            .unwrap();

        assert_eq!(res["success"].as_bool().unwrap(), true);
        assert_eq!(res["tool_count"].as_u64().unwrap(), 2);
        assert_eq!(res["exposed_count"].as_u64().unwrap(), 2);

        let tools = res["tools"].as_array().unwrap();
        let exec = tools
            .iter()
            .find(|tool| tool["name"].as_str() == Some("exec_command"))
            .expect("exec_command entry");
        assert_eq!(exec["domain"].as_str().unwrap(), "shell");
        assert_eq!(exec["risk"].as_str().unwrap(), "high");
        assert_eq!(exec["spawns_process"].as_bool().unwrap(), true);
        assert_eq!(exec["requires_approval"].as_bool().unwrap(), true);
        assert_eq!(exec["matched_prompt_domain"].as_bool().unwrap(), true);
        assert!(exec["selection_reason"]
            .as_str()
            .unwrap()
            .contains("prompt_domain"));
        assert!(exec["selected_score"].as_i64().unwrap() > 0);
        assert!(exec["aliases"]
            .as_array()
            .unwrap()
            .iter()
            .any(|alias| alias.as_str() == Some("shell command")));
        assert!(exec["examples"]
            .as_array()
            .unwrap()
            .iter()
            .any(|example| example
                .as_str()
                .unwrap_or("")
                .contains("safe project-local command")));
        assert!(exec["when_to_use"]
            .as_str()
            .unwrap()
            .contains("shell commands"));
        assert!(exec["when_not_to_use"]
            .as_str()
            .unwrap()
            .contains("file reads"));

        let selected_domains = res["selected_domains"].as_array().unwrap();
        assert!(selected_domains.iter().any(|d| d.as_str() == Some("shell")));

        let read_file = tools
            .iter()
            .find(|tool| tool["name"].as_str() == Some("read_file"))
            .expect("read_file entry");
        assert_eq!(read_file["domain"].as_str().unwrap(), "filesystem");
        assert_eq!(read_file["risk"].as_str().unwrap(), "low");
        assert_eq!(read_file["writes_disk"].as_bool().unwrap(), false);
    }

    #[test]
    fn test_openz_inventory_reports_runtime_identity() {
        let registry = crate::tools::ToolRegistry::new();
        registry.register(std::sync::Arc::new(
            crate::tools::self_management::ManageConfigTool,
        ));

        let inventory = OpenZInventoryTool::new(registry);
        let res = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(inventory.call(&serde_json::json!({
                "include_tools": false,
                "include_subagents": false,
                "prompt": "what model are you and which language are you best at"
            })))
            .unwrap();

        assert_eq!(res["success"].as_bool().unwrap(), true);
        assert!(res["runtime_identity"].is_object());
        assert!(res["runtime_identity"]["configured_model"]
            .as_str()
            .is_some());
        assert!(res["runtime_identity"]["configured_provider"]
            .as_str()
            .is_some());
        assert!(res["runtime_identity"]["model_supports_vision"]
            .as_bool()
            .is_some());
        assert!(res["guidance"]
            .as_str()
            .unwrap()
            .contains("model/provider identity"));
    }

    #[test]
    fn test_tool_catalog_reports_resource_policy_visibility() {
        struct NetworkTool;
        #[async_trait::async_trait]
        impl Tool for NetworkTool {
            fn name(&self) -> &str {
                "web_fetch"
            }
            fn description(&self) -> &str {
                "Fetch a web page"
            }
            fn parameters(&self) -> serde_json::Value {
                serde_json::json!({})
            }
            async fn call(&self, _args: &serde_json::Value) -> Result<serde_json::Value> {
                Ok(serde_json::json!({"ok": true}))
            }
        }

        let registry = crate::tools::ToolRegistry::new();
        registry.register(std::sync::Arc::new(NetworkTool));

        let catalog = ToolCatalogTool::new(registry);
        let res = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(catalog.call(&serde_json::json!({
                "prompt": "fetch this webpage",
                "resource_overrides": {
                    "allow_network_tools": false,
                    "free_disk_gb": 100.0,
                    "active_process_tools": 0
                }
            })))
            .unwrap();

        let tools = res["tools"].as_array().unwrap();
        let web_fetch = tools
            .iter()
            .find(|tool| tool["name"].as_str() == Some("web_fetch"))
            .expect("web_fetch entry");

        assert_eq!(
            web_fetch["resource_policy"]["decision"].as_str(),
            Some("block")
        );
        assert!(web_fetch["resource_policy"]["reason"]
            .as_str()
            .unwrap()
            .contains("Network tools are disabled"));
        assert_eq!(
            web_fetch["resource_policy"]["free_disk_gb"].as_f64(),
            Some(100.0)
        );
        assert_eq!(
            web_fetch["resource_policy"]["active_process_tools"].as_u64(),
            Some(0)
        );
    }

    #[test]
    fn test_diagnose_openmedia_video_args_use_minimal_scene_for_placeholder() {
        let create_args = normalize_diagnose_mock_args(
            "openmedia_video_create",
            serde_json::json!({ "test": true }),
        );
        assert_eq!(create_args["scene"]["width"], 640);
        assert!(create_args.get("test").is_none());

        let preview_args =
            normalize_diagnose_mock_args("openmedia_video_preview", serde_json::json!({}));
        assert_eq!(preview_args["scene"]["fps"], 1);
        assert_eq!(preview_args["output_format"], "png");
    }

    #[test]
    fn test_diagnose_openmedia_video_placeholder_is_visible() {
        let args = normalize_diagnose_mock_args("openmedia_video_create", serde_json::json!({}));
        assert_eq!(args["scene"]["background"], "#1e293b");
        assert_eq!(
            args["scene"]["scenes"][0]["elements"][0]["content"],
            "OpenZ"
        );
        assert_eq!(
            args["scene"]["scenes"][0]["elements"][0]["style"]["font_size"],
            48.0
        );
        assert_eq!(
            args["scene"]["scenes"][0]["elements"][0]["style"]["font_weight"],
            800
        );
    }

    #[test]
    fn test_diagnose_and_optimize_tools() {
        let registry = crate::tools::ToolRegistry::new();
        struct DummyTool;
        #[async_trait::async_trait]
        impl Tool for DummyTool {
            fn name(&self) -> &str {
                "dummy_tool"
            }
            fn description(&self) -> &str {
                "dummy"
            }
            fn parameters(&self) -> serde_json::Value {
                serde_json::json!({})
            }
            async fn call(&self, _args: &serde_json::Value) -> Result<serde_json::Value> {
                Ok(serde_json::json!({ "ok": true }))
            }
        }

        let dummy = std::sync::Arc::new(DummyTool);
        registry.register(dummy.clone());

        let diagnose = DiagnoseToolTool::new(registry.clone());
        let res = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(diagnose.call(&serde_json::json!({ "tool_name": "dummy_tool" })))
            .unwrap();
        assert!(res["success"].as_bool().unwrap());
        assert_eq!(res["output"]["ok"].as_bool().unwrap(), true);

        // Test OptimizeToolScopeTool
        let optimizer = OptimizeToolScopeTool::new(registry.clone());
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(optimizer.call(&serde_json::json!({ "active_prefixes": ["other_"] })))
            .unwrap();

        // DummyTool starts with "dummy_", which does not match prefix filter "other_"
        // It should be filtered out
        assert!(registry.get("dummy_tool").is_none());

        // Restore filter
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(optimizer.call(&serde_json::json!({ "active_prefixes": [] })))
            .unwrap();
        assert!(registry.get("dummy_tool").is_some());
    }

    #[test]
    fn test_curate_skills() {
        // Run database queries through curate_skill tool
        let tool = CurateSkillTool;
        let rt = tokio::runtime::Runtime::new().unwrap();

        // 1. Delete skill if exists
        let _ = rt.block_on(tool.call(&serde_json::json!({
            "action": "delete",
            "skill_name": "test_curate_skills_temp"
        })));

        // 2. Add skill
        let add_res = rt
            .block_on(tool.call(&serde_json::json!({
                "action": "add",
                "skill_name": "test_curate_skills_temp",
                "content": "This is a test skill content"
            })))
            .unwrap();
        assert!(add_res["success"].as_bool().unwrap());

        // 3. List skills and verify
        let list_res = rt
            .block_on(tool.call(&serde_json::json!({
                "action": "list"
            })))
            .unwrap();
        assert!(list_res["success"].as_bool().unwrap());
        let skills = list_res["skills"].as_array().unwrap();
        let found = skills
            .iter()
            .any(|s| s["name"].as_str().unwrap() == "test_curate_skills_temp");
        assert!(found);

        // 4. Delete skill
        let del_res = rt
            .block_on(tool.call(&serde_json::json!({
                "action": "delete",
                "skill_name": "test_curate_skills_temp"
            })))
            .unwrap();
        assert!(del_res["success"].as_bool().unwrap());
    }

    #[test]
    fn test_manage_config() {
        let tool = ManageConfigTool;
        let rt = tokio::runtime::Runtime::new().unwrap();

        // Save original config
        let original_config = crate::config::loader::load_config().unwrap();

        // 1. View config
        let view_res = rt
            .block_on(tool.call(&serde_json::json!({
                "action": "view"
            })))
            .unwrap();
        assert!(view_res["success"].as_bool().unwrap());
        // Verify redacted api_key / secret key format if they exist
        let config_val = &view_res["config"];
        if let Some(providers) = config_val.get("providers") {
            if let Some(openai) = providers.get("openai") {
                if let Some(api_key) = openai.get("api_key") {
                    if api_key.is_string() {
                        assert_eq!(api_key.as_str().unwrap(), "********");
                    }
                }
            }
        }

        // 2. Update config hyperparameters
        let update_res = rt
            .block_on(tool.call(&serde_json::json!({
                "action": "update",
                "updates": {
                    "max_tokens": 1234,
                    "temperature": 0.25,
                    "caveman_mode": false,
                    "streaming": false,
                    "min_free_disk_gb": 3.5,
                    "allow_network_tools": false,
                    "max_concurrent_process_tools": 2,
                    "warn_before_expensive_tools": false,
                    "skills_workspace_skills_enabled": false,
                    "skills_external_dirs": ["~/.agents/skills", "/tmp/openz-team-skills"],
                    "skills_write_approval": true
                }
            })))
            .unwrap();
        assert!(update_res["success"].as_bool().unwrap());

        // 3. Verify they were updated and saved
        let updated_config = crate::config::loader::load_config().unwrap();
        assert_eq!(updated_config.agents.defaults.max_tokens, 1234);
        assert_eq!(updated_config.agents.defaults.temperature, 0.25f32);
        assert_eq!(updated_config.agents.defaults.caveman_mode, false);
        assert_eq!(updated_config.agents.defaults.streaming, false);
        assert_eq!(updated_config.agents.defaults.min_free_disk_gb, 3.5);
        assert_eq!(updated_config.agents.defaults.allow_network_tools, false);
        assert_eq!(
            updated_config.agents.defaults.max_concurrent_process_tools,
            2
        );
        assert_eq!(
            updated_config.agents.defaults.warn_before_expensive_tools,
            false
        );
        assert_eq!(updated_config.skills.workspace_skills_enabled, false);
        assert_eq!(
            updated_config.skills.external_dirs,
            vec![
                "~/.agents/skills".to_string(),
                "/tmp/openz-team-skills".to_string()
            ]
        );
        assert_eq!(updated_config.skills.write_approval, true);

        // 4. Try updating an invalid/restricted field (should be blocked)
        let invalid_res = rt
            .block_on(tool.call(&serde_json::json!({
                "action": "update",
                "updates": {
                    "invalid_field": "some_value"
                }
            })))
            .unwrap();
        assert_eq!(invalid_res["success"].as_bool().unwrap(), false);
        assert!(invalid_res["error"]
            .as_str()
            .unwrap()
            .contains("invalid_field"));

        // Restore original config
        crate::config::loader::save_config(&original_config).unwrap();
    }

    #[test]
    fn test_diagnose_system() {
        let tool = DiagnoseSystemTool;
        let rt = tokio::runtime::Runtime::new().unwrap();

        // 1. Run diagnostic without latency checking to keep it fast
        let res = rt
            .block_on(tool.call(&serde_json::json!({
                "check_latency": false,
                "check_db_integrity": false
            })))
            .unwrap();

        assert_eq!(res["status"].as_str().unwrap(), "success");
        assert!(res["system"]["os"].as_str().is_some());
        assert!(res["system"]["architecture"].as_str().is_some());
        assert!(res["system"]["cores"].as_u64().unwrap() >= 1);

        // Verify directory statistics keys exist
        assert!(res["directories"]["sessions"].is_object());
        assert!(res["directories"]["tool_outputs"].is_object());
        assert!(res["directories"]["traces"].is_object());
        assert!(res["directories"]["skills"].is_object());

        // Verify databases status keys exist
        assert!(res["databases"]["memory"].is_object());
        assert!(res["databases"]["docs"].is_object());
        assert!(res["databases"]["graph_memory"].is_object());
        assert!(res["databases"]["ccr_cache"].is_object());
        assert!(res["databases"]["thoughts"].is_object());
    }

    #[test]
    fn test_manage_sessions() {
        let _env_lock = TestEnvLock::acquire();
        let previous_config_dir = std::env::var("OPENZ_CONFIG_DIR").ok();
        let openz_dir =
            std::env::temp_dir().join(format!("openz_manage_sessions_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&openz_dir).unwrap();
        std::env::set_var("OPENZ_CONFIG_DIR", &openz_dir);

        let tool = ManageSessionsTool;
        let rt = tokio::runtime::Runtime::new().unwrap();

        let sessions_dir = openz_dir.join("sessions");
        std::fs::create_dir_all(&sessions_dir).unwrap();

        // 1. Create a dummy session file for testing. Use a unique key because
        // the full lib test suite runs session-management tests in parallel.
        let test_session_key = format!("test_session_xyz_{}", uuid::Uuid::new_v4());
        let session_file = sessions_dir.join(format!("{}.json", test_session_key));
        let _ = std::fs::remove_file(&session_file);
        std::fs::write(
            &session_file,
            serde_json::json!({
                "messages": [
                    {
                        "role": "user",
                        "content": "Hello"
                    }
                ]
            })
            .to_string(),
        )
        .unwrap();

        // 2. List sessions and check if our dummy is present
        let list_res = rt
            .block_on(tool.call(&serde_json::json!({
                "action": "list"
            })))
            .unwrap();
        assert_eq!(list_res["status"].as_str().unwrap(), "success");
        let sessions = list_res["sessions"].as_array().unwrap();
        let found = sessions
            .iter()
            .any(|s| s["session_key"].as_str().unwrap() == test_session_key);
        assert!(found);

        // 3. Archive our dummy session
        let archive_res = rt
            .block_on(tool.call(&serde_json::json!({
                "action": "archive",
                "session_key": &test_session_key
            })))
            .unwrap();
        assert_eq!(archive_res["status"].as_str().unwrap(), "success");
        assert!(!session_file.exists());

        // Clean up archived files
        let archives_dir = openz_dir.join("archives");
        if let Ok(entries) = std::fs::read_dir(&archives_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_str().unwrap_or("");
                if name_str.starts_with(&test_session_key) {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }

        // 4. Create another dummy session and delete it
        std::fs::write(&session_file, "{\"messages\": []}").unwrap();
        assert!(session_file.exists());

        let delete_res = rt
            .block_on(tool.call(&serde_json::json!({
                "action": "delete",
                "session_key": &test_session_key
            })))
            .unwrap();
        assert_eq!(delete_res["status"].as_str().unwrap(), "success");
        assert!(!session_file.exists());

        // 5. Test pruning (prune should execute without errors)
        let prune_res = rt
            .block_on(tool.call(&serde_json::json!({
                "action": "prune",
                "older_than_days": 30
            })))
            .unwrap();
        assert_eq!(prune_res["status"].as_str().unwrap(), "success");
        assert!(prune_res["details"]["files_removed"].as_u64().is_some());

        if let Some(prev) = previous_config_dir {
            std::env::set_var("OPENZ_CONFIG_DIR", prev);
        } else {
            std::env::remove_var("OPENZ_CONFIG_DIR");
        }
        let _ = std::fs::remove_dir_all(&openz_dir);
    }

    #[test]
    fn test_manage_backups() {
        let tool = ManageBackupsTool;
        let rt = tokio::runtime::Runtime::new().unwrap();

        // 1. Create a backup
        let create_res = rt
            .block_on(tool.call(&serde_json::json!({
                "action": "create"
            })))
            .unwrap();
        assert_eq!(create_res["status"].as_str().unwrap(), "success");
        let backup_name = create_res["backup_name"].as_str().unwrap();

        // 2. List backups and ensure it exists
        let list_res = rt
            .block_on(tool.call(&serde_json::json!({
                "action": "list"
            })))
            .unwrap();
        assert_eq!(list_res["status"].as_str().unwrap(), "success");
        let backups = list_res["backups"].as_array().unwrap();
        let found = backups
            .iter()
            .any(|b| b["backup_name"].as_str().unwrap() == backup_name);
        assert!(found);

        // 3. Restore from the backup
        let restore_res = rt
            .block_on(tool.call(&serde_json::json!({
                "action": "restore",
                "backup_name": backup_name
            })))
            .unwrap();
        assert_eq!(restore_res["status"].as_str().unwrap(), "success");

        // 4. Delete the backup
        let delete_res = rt
            .block_on(tool.call(&serde_json::json!({
                "action": "delete",
                "backup_name": backup_name
            })))
            .unwrap();
        assert_eq!(delete_res["status"].as_str().unwrap(), "success");

        // 5. Ensure it is gone from the list
        let list_res_2 = rt
            .block_on(tool.call(&serde_json::json!({
                "action": "list"
            })))
            .unwrap();
        let backups_2 = list_res_2["backups"].as_array().unwrap();
        let found_2 = backups_2
            .iter()
            .any(|b| b["backup_name"].as_str().unwrap() == backup_name);
        assert!(!found_2);
    }
}
