use crate::config::schema::Config;
use crate::providers::LLMProvider;
use crate::session::SessionManager;
use crate::tools::ToolRegistry;
use anyhow::Result;
use std::sync::Arc;

pub fn register_all_tools(
    registry: &ToolRegistry,
    config: &Config,
    provider: Arc<dyn LLMProvider>,
    session_manager: SessionManager,
) -> Result<()> {
    crate::cli::tool_registration::core::register_core_tools(
        registry,
        config,
        provider.clone(),
        session_manager.clone(),
    );
    crate::cli::tool_registration::memory::register_sequential_thinking_tools(registry);
    crate::cli::tool_registration::memory::register_headroom_tools(registry);
    crate::cli::tool_registration::memory::register_graph_memory_tools(registry);
    crate::cli::tool_registration::memory::register_memory_extra_tools(registry);
    crate::cli::tool_registration::integrations::register_searchxyz_tools(registry);
    crate::cli::tool_registration::media::register_openmedia_tools(registry);
    crate::cli::tool_registration::media::register_opendoc_tools(registry);
    crate::cli::tool_registration::integrations::register_github_mcp_tools(registry);
    crate::cli::tool_registration::integrations::register_docs_mcp_tools(registry);
    crate::cli::tool_registration::integrations::register_lazy_mcp_tools(registry, config);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schema_required_fields(schema: &serde_json::Value) -> Vec<String> {
        schema
            .get("required")
            .and_then(|v| v.as_array())
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect()
    }

    fn schema_properties(
        schema: &serde_json::Value,
    ) -> Option<&serde_json::Map<String, serde_json::Value>> {
        schema.get("properties").and_then(|v| v.as_object())
    }

    fn is_opaque_json_property(prop: &serde_json::Value) -> bool {
        prop.get("type").is_none()
            && prop.get("anyOf").is_none()
            && prop.get("oneOf").is_none()
            && prop.get("allOf").is_none()
            && prop.get("$ref").is_none()
            && prop.get("properties").is_none()
            && prop.get("items").is_none()
    }

    fn has_actionable_json_example(prop: &serde_json::Value) -> bool {
        prop.get("description")
            .and_then(|v| v.as_str())
            .map(|description| {
                let lower = description.to_lowercase();
                lower.contains("json") && (description.contains('{') || description.contains('['))
            })
            .unwrap_or(false)
    }

    fn has_raw_json_normalizer(tool_name: &str, field_name: &str) -> bool {
        matches!(
            (tool_name, field_name),
            ("openmedia_create_svg", "elements")
                | ("openmedia_template_create", "parameter_schema")
                | ("openmedia_template_create", "scene_template")
                | ("openmedia_video_create", "scene")
                | ("openmedia_video_preview", "scene")
                | ("openmedia_video_from_template", "parameters")
        )
    }

    #[tokio::test]
    async fn required_raw_json_tool_fields_are_documented_or_normalized() {
        let registry = ToolRegistry::new();
        let config = Config::default();
        let provider = Arc::new(crate::providers::mock::MockProvider::new());
        let sessions = SessionManager::new(std::path::PathBuf::from("/tmp/openz-test-sessions"));

        register_all_tools(&registry, &config, provider, sessions).unwrap();

        let mut offenders = Vec::new();
        for entry in registry.catalog_entries(true) {
            let name = entry["name"].as_str().unwrap_or("<unknown>");
            let schema = &entry["parameters"];
            let Some(properties) = schema_properties(schema) else {
                continue;
            };

            for field in schema_required_fields(schema) {
                let Some(prop) = properties.get(&field) else {
                    continue;
                };
                if is_opaque_json_property(prop)
                    && !has_actionable_json_example(prop)
                    && !has_raw_json_normalizer(name, &field)
                {
                    offenders.push(format!("{name}.{field}"));
                }
            }
        }

        assert!(
            offenders.is_empty(),
            "required raw JSON fields need concrete schemas, examples, or normalizer coverage: {offenders:?}"
        );
    }

    #[tokio::test]
    async fn tool_registry_exposes_every_registered_tool() {
        let config = Config::default();
        let provider = Arc::new(crate::providers::mock::MockProvider::new());
        let sessions = SessionManager::new(std::path::PathBuf::from("/tmp/openz-test-sessions"));
        let registry = ToolRegistry::new_with_context(
            config.clone(),
            provider.clone(),
            sessions.clone(),
        );

        register_all_tools(&registry, &config, provider, sessions).unwrap();

        let names = registry.tool_names();
        let unique: std::collections::BTreeSet<_> = names.iter().collect();
        assert_eq!(
            names.len(),
            unique.len(),
            "duplicate tool names must not appear"
        );

        // Drift guard in both directions: every curated entry must be backed
        // by its canonical registration, and curated aliases must not become
        // executable registrations. Dynamic integrations are intentionally
        // outside this report.
        let drift = registry.static_tool_drift();
        assert!(
            drift.is_clean(),
            "curated tool registry drift: missing_registered={:?}, noncanonical_registered={:?}",
            drift.missing_registered,
            drift.noncanonical_registered
        );

        // Every curated alias must resolve to exactly the canonical tool that
        // owns it. This catches both alias drift and ambiguous compatibility
        // names before they can reach the model/tool execution boundary.
        for spec in crate::tools::all_tool_specs() {
            let canonical = registry
                .get(spec.name)
                .unwrap_or_else(|| panic!("curated tool '{}' is not resolvable", spec.name));
            assert_eq!(canonical.name(), spec.name);
            for alias in spec.aliases {
                let resolved = registry
                    .get(alias)
                    .unwrap_or_else(|| panic!("tool alias '{alias}' is not resolvable"));
                assert_eq!(
                    resolved.name(),
                    spec.name,
                    "tool alias '{alias}' resolves to '{}' instead of '{}'",
                    resolved.name(),
                    spec.name
                );
            }
        }

        let openai_tools = registry.to_openai_format_for_prompt("");
        let openai_names: Vec<&str> = openai_tools
            .iter()
            .filter_map(|tool| tool["function"]["name"].as_str())
            .collect();
        let unique_openai_names: std::collections::BTreeSet<_> = openai_names.iter().copied().collect();
        assert_eq!(
            openai_names.len(),
            unique_openai_names.len(),
            "OpenAI tool payload must contain one entry per canonical tool name"
        );
        for name in openai_names {
            assert!(
                registry.get(name).is_some(),
                "OpenAI payload contains an unresolvable tool '{name}'"
            );
        }

        assert!(names.contains(&"read_file".to_string()));
        assert!(names.contains(&"exec_command".to_string()));
        assert!(names.contains(&"delegate_task".to_string()));
        assert!(names.contains(&"tool_catalog".to_string()));
        assert!(names.contains(&"openz_inventory".to_string()));
        assert!(names.contains(&"manage_servers".to_string()));
        assert!(names.contains(&"device_inventory".to_string()));
        assert!(names.contains(&"workflow_memory".to_string()));
        assert!(names.contains(&"sequentialthinking".to_string()));
        assert!(names.contains(&"scope_context".to_string()));
        assert!(names.contains(&"create_entities".to_string()));
        assert!(names.contains(&"searchxyz_doctor".to_string()));
        assert!(names.contains(&"searchxyz_search_web".to_string()));
        assert!(names.contains(&"openmedia_ping".to_string()));
        assert!(names.contains(&"opendoc_open_document".to_string()));

        let find_path = registry
            .catalog_entries(true)
            .into_iter()
            .find(|entry| entry["name"].as_str() == Some("find_path"))
            .expect("find_path must be registered");
        let find_path_schema = &find_path["parameters"];
        assert!(find_path_schema["properties"].get("startEntity").is_some());
        assert!(find_path_schema["properties"].get("targetEntity").is_some());
        let required = find_path_schema["required"]
            .as_array()
            .expect("find_path required fields");
        assert!(required.iter().any(|field| field == "startEntity"));
        assert!(required.iter().any(|field| field == "targetEntity"));

        assert!(
            registry.tool_count() > 128,
            "full registry should exceed one OpenAI tool payload"
        );
    }

    #[test]
    fn openai_format_prioritizes_high_value_tools_when_truncated() {
        let registry = ToolRegistry::new();
        for i in 0..140 {
            registry.register(Arc::new(MetaTestTool {
                name: format!("low_tool_{i:03}"),
                domain: "general",
                priority: 1,
                risk: crate::tools::ToolRisk::Low,
            }));
        }
        registry.register(Arc::new(MetaTestTool {
            name: "cargo_manager".to_string(),
            domain: "code",
            priority: 95,
            risk: crate::tools::ToolRisk::Medium,
        }));
        registry.register(Arc::new(MetaTestTool {
            name: "read_file".to_string(),
            domain: "filesystem",
            priority: 90,
            risk: crate::tools::ToolRisk::Low,
        }));

        let tools =
            registry.to_openai_format_for_prompt("run cargo test and fix the rust compile errors");
        assert_eq!(tools.len(), 128);
        let names: Vec<_> = tools
            .iter()
            .map(|tool| tool["function"]["name"].as_str().unwrap().to_string())
            .collect();
        assert!(names.contains(&"cargo_manager".to_string()));
        assert!(names.contains(&"read_file".to_string()));
        assert!(
            !names.contains(&"low_tool_139".to_string()),
            "low priority unrelated tools should be dropped first"
        );
    }

    #[test]
    fn runtime_management_tools_stay_exposed_under_tool_limit() {
        let registry = ToolRegistry::new();
        for i in 0..180 {
            registry.register(Arc::new(MetaTestTool {
                name: format!("low_tool_{i:03}"),
                domain: "general",
                priority: 1,
                risk: crate::tools::ToolRisk::Low,
            }));
        }
        registry.register(Arc::new(MetaTestTool {
            name: "openz_inventory".to_string(),
            domain: "self_management",
            priority: 85,
            risk: crate::tools::ToolRisk::Low,
        }));
        registry.register(Arc::new(MetaTestTool {
            name: "manage_servers".to_string(),
            domain: "self_management",
            priority: 85,
            risk: crate::tools::ToolRisk::Medium,
        }));
        registry.register(Arc::new(MetaTestTool {
            name: "workflow_memory".to_string(),
            domain: "self_management",
            priority: 85,
            risk: crate::tools::ToolRisk::Medium,
        }));

        let tools = registry.to_openai_format_for_prompt(
            "what features do you have and stop the dev server after preview",
        );
        let names: Vec<_> = tools
            .iter()
            .map(|tool| tool["function"]["name"].as_str().unwrap().to_string())
            .collect();
        assert!(names.contains(&"openz_inventory".to_string()));
        assert!(names.contains(&"manage_servers".to_string()));
        assert!(names.contains(&"workflow_memory".to_string()));
    }

    #[test]
    fn cron_scheduler_tools_stay_exposed_under_tool_limit() {
        let registry = ToolRegistry::new();
        for i in 0..180 {
            registry.register(Arc::new(MetaTestTool {
                name: format!("low_tool_{i:03}"),
                domain: "general",
                priority: 1,
                risk: crate::tools::ToolRisk::Low,
            }));
        }
        registry.register(Arc::new(crate::tools::cron::ScheduleJobTool));
        registry.register(Arc::new(crate::tools::cron::ListJobsTool));
        registry.register(Arc::new(crate::tools::cron::RemoveJobTool));

        let tools = registry.to_openai_format_for_prompt(
            "test the cronjob: at 18:00 open browser and play a song, then remove that cronjob",
        );
        let names: Vec<_> = tools
            .iter()
            .map(|tool| tool["function"]["name"].as_str().unwrap().to_string())
            .collect();
        assert!(names.contains(&"schedule_job".to_string()));
        assert!(names.contains(&"list_jobs".to_string()));
        assert!(names.contains(&"remove_job".to_string()));

        let schedule_metadata = crate::tools::ToolMetadata::infer("schedule_job");
        assert_eq!(schedule_metadata.domain, "self_management");
        assert!(schedule_metadata.aliases.contains(&"cron job"));
        assert!(schedule_metadata.when_to_use.contains("schedule"));
    }

    #[test]
    fn cron_management_tools_stay_exposed_under_tool_limit() {
        let registry = ToolRegistry::new();
        for i in 0..180 {
            registry.register(Arc::new(MetaTestTool {
                name: format!("low_tool_{i:03}"),
                domain: "general",
                priority: 1,
                risk: crate::tools::ToolRisk::Low,
            }));
        }
        registry.register(Arc::new(crate::tools::cron::ScheduleJobTool));
        registry.register(Arc::new(crate::tools::cron::ListJobsTool));
        registry.register(Arc::new(crate::tools::cron::RemoveJobTool));
        registry.register(Arc::new(crate::tools::cron::PauseJobTool));
        registry.register(Arc::new(crate::tools::cron::ResumeJobTool));
        registry.register(Arc::new(crate::tools::cron::GetJobTool));
        registry.register(Arc::new(crate::tools::cron::GetJobLogsTool));
        registry.register(Arc::new(crate::tools::cron::RunJobNowTool));

        let names = registry
            .to_openai_format_for_prompt("check all cron jobs and show logs for job daily")
            .into_iter()
            .filter_map(|tool| tool["function"]["name"].as_str().map(str::to_string))
            .collect::<Vec<_>>();

        assert!(names.contains(&"list_jobs".to_string()));
        assert!(names.contains(&"get_job".to_string()));
        assert!(names.contains(&"get_job_logs".to_string()));
        assert!(names.contains(&"pause_job".to_string()));
        assert!(names.contains(&"resume_job".to_string()));
        assert!(names.contains(&"run_job_now".to_string()));
    }

    #[test]
    fn metadata_includes_aliases_and_examples_for_tool_choice() {
        let metadata = crate::tools::ToolMetadata::infer("cargo_manager");
        assert!(metadata.aliases.contains(&"cargo test"));
        assert!(metadata.aliases.contains(&"cargo check"));
        assert!(metadata
            .examples
            .iter()
            .any(|example| example.contains("cargo test")));
        assert!(metadata.when_to_use.contains("Rust"));
        assert!(metadata.when_not_to_use.contains("read"));
    }

    #[tokio::test]
    async fn provider_tool_description_includes_compact_choice_hints() {
        let registry = ToolRegistry::new();
        let config = Config::default();
        let provider = Arc::new(crate::providers::mock::MockProvider::new());
        let sessions = SessionManager::new(std::path::PathBuf::from("/tmp/openz-test-sessions"));
        register_all_tools(&registry, &config, provider, sessions).unwrap();

        let tools = registry.to_openai_format_for_prompt("run cargo test");
        let description = tools
            .iter()
            .find(|tool| tool["function"]["name"].as_str() == Some("cargo_manager"))
            .and_then(|tool| tool["function"]["description"].as_str())
            .expect("cargo_manager description");
        assert!(description.contains("Use when:"));
        assert!(description.contains("Avoid when:"));
        assert!(description.contains("Aliases:"));
        assert!(description.contains("cargo test"));
    }

    #[test]
    fn route_analysis_formats_compact_status_line() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(MetaTestTool {
            name: "cargo_manager".to_string(),
            domain: "code",
            priority: 95,
            risk: crate::tools::ToolRisk::Medium,
        }));
        for i in 0..140 {
            registry.register(Arc::new(MetaTestTool {
                name: format!("general_tool_{i:03}"),
                domain: "general",
                priority: 1,
                risk: crate::tools::ToolRisk::Low,
            }));
        }

        let summary = registry.tool_router_status_line("run cargo test");
        assert!(summary.contains("Tool Router selected 128/141 tools"));
        assert!(summary.contains("code"));
        assert!(summary.contains("filesystem"));
        assert!(summary.contains("dropped 13"));
    }

    #[test]
    fn route_analysis_reports_api_limit_hidden_reason() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(MetaTestTool {
            name: "cargo_manager".to_string(),
            domain: "code",
            priority: 95,
            risk: crate::tools::ToolRisk::Medium,
        }));
        for i in 0..140 {
            registry.register(Arc::new(MetaTestTool {
                name: format!("general_tool_{i:03}"),
                domain: "general",
                priority: 1,
                risk: crate::tools::ToolRisk::Low,
            }));
        }

        let route = registry.route_for_prompt("run cargo test");
        assert!(route.selected_domains.contains(&"code".to_string()));
        assert!(route.dropped_count > 0);
        let hidden = route
            .entries
            .iter()
            .find(|entry| entry.hidden_reason == Some("api_limit"))
            .expect("at least one tool hidden by API limit");
        assert!(!hidden.exposed_to_model);
    }

    #[test]
    fn prompt_aware_format_selects_relevant_tool_domains() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(MetaTestTool {
            name: "web_fetch".to_string(),
            domain: "web",
            priority: 80,
            risk: crate::tools::ToolRisk::Medium,
        }));
        registry.register(Arc::new(MetaTestTool {
            name: "gsd_browser".to_string(),
            domain: "web",
            priority: 80,
            risk: crate::tools::ToolRisk::Medium,
        }));
        registry.register(Arc::new(MetaTestTool {
            name: "openmedia_image_resize".to_string(),
            domain: "media",
            priority: 80,
            risk: crate::tools::ToolRisk::Medium,
        }));
        for i in 0..140 {
            registry.register(Arc::new(MetaTestTool {
                name: format!("general_tool_{i:03}"),
                domain: "general",
                priority: 1,
                risk: crate::tools::ToolRisk::Low,
            }));
        }

        let website_tools =
            registry.to_openai_format_for_prompt("research this website and summarize the page");
        let website_names: Vec<_> = website_tools
            .iter()
            .map(|tool| tool["function"]["name"].as_str().unwrap().to_string())
            .collect();
        assert!(website_names.contains(&"web_fetch".to_string()));
        assert!(website_names.contains(&"gsd_browser".to_string()));

        let image_tools =
            registry.to_openai_format_for_prompt("resize this image and make an svg preview");
        let image_names: Vec<_> = image_tools
            .iter()
            .map(|tool| tool["function"]["name"].as_str().unwrap().to_string())
            .collect();
        assert!(image_names.contains(&"openmedia_image_resize".to_string()));
    }

    #[tokio::test]
    async fn openai_format_reserves_api_slots_for_dynamic_subagents() {
        let config = Config::default();
        let provider = Arc::new(crate::providers::mock::MockProvider::new());
        let sessions = SessionManager::new(std::path::PathBuf::from("/tmp/openz-test-sessions"));
        let registry =
            ToolRegistry::new_with_context(config.clone(), provider.clone(), sessions.clone());

        register_all_tools(&registry, &config, provider, sessions).unwrap();

        let tools = registry.to_openai_format();
        assert_eq!(tools.len(), 128);
        let names: Vec<_> = tools
            .iter()
            .map(|tool| tool["function"]["name"].as_str().unwrap().to_string())
            .collect();
        assert!(
            names.contains(&"vision_agent".to_string()),
            "vision_agent must stay available even when native tools exceed provider API limits"
        );
        assert!(
            names.contains(&"planner".to_string()),
            "planner must stay available for orchestrated subagent workflows"
        );
    }

    struct MetaTestTool {
        name: String,
        domain: &'static str,
        priority: u8,
        risk: crate::tools::ToolRisk,
    }

    #[async_trait::async_trait]
    impl crate::tools::Tool for MetaTestTool {
        fn name(&self) -> &str {
            &self.name
        }
        fn description(&self) -> &str {
            "test"
        }
        fn parameters(&self) -> serde_json::Value {
            serde_json::json!({ "type": "object" })
        }
        fn metadata(&self) -> crate::tools::ToolMetadata {
            crate::tools::ToolMetadata {
                presentation_name: crate::tools::presentation_name(&self.name),
                domain: self.domain,
                risk: self.risk,
                uses_network: self.domain == "web",
                writes_disk: false,
                spawns_process: false,
                requires_approval: matches!(self.risk, crate::tools::ToolRisk::High),
                priority: self.priority,
                aliases: &[],
                examples: &[],
                when_to_use: "",
                when_not_to_use: "",
                recommended_timeout_secs: None,
            }
        }
        async fn call(&self, _arguments: &serde_json::Value) -> anyhow::Result<serde_json::Value> {
            Ok(serde_json::json!({ "ok": true }))
        }
    }
}
