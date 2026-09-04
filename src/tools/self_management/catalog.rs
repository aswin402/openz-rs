use crate::tools::Tool;
use anyhow::Result;
use serde_json::Value;

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
                presentation_name: entry["name"]
                    .as_str()
                    .map(crate::tools::presentation_name)
                    .unwrap_or_else(|| "Tool".to_string()),
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
