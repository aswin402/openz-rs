use crate::tools::Tool;
use anyhow::Result;
use serde_json::Value;

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
                    "description": "Include the full registered tool name list. Default false for faster answers; set true only when the user asks for exact tool names."
                },
                "include_subagents": {
                    "type": "boolean",
                    "description": "Include loaded subagent profile names. Default false for faster answers; set true only when the user asks for exact subagent names."
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
            .unwrap_or(false);
        let include_subagents = arguments
            .get("include_subagents")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
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
            "guidance": "Use this compact live inventory instead of guessing feature/tool counts or model/provider identity from memory. Call again with include_tools=true or include_subagents=true only if exact names are needed."
        }))
    }
}

