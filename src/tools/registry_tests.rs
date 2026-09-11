use super::*;
use crate::config::schema::Config;
use crate::session::SessionManager;
use crate::tools::{normalize_tool_args, Tool, ToolMetadata, ToolRisk};
use anyhow::Result;
use std::sync::Arc;

struct CacheTestTool {
    name: &'static str,
    domain: &'static str,
    priority: u8,
}

#[async_trait::async_trait]
impl Tool for CacheTestTool {
    fn name(&self) -> &str {
        self.name
    }

    fn description(&self) -> &str {
        "cache test tool"
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({ "type": "object" })
    }

    fn metadata(&self) -> ToolMetadata {
        ToolMetadata {
            presentation_name: crate::tools::presentation_name(self.name),
            domain: self.domain,
            risk: ToolRisk::Low,
            uses_network: false,
            writes_disk: false,
            spawns_process: false,
            requires_approval: false,
            priority: self.priority,
            aliases: &[],
            examples: &[],
            when_to_use: "",
            when_not_to_use: "",
            recommended_timeout_secs: None,
        }
    }

    async fn call(&self, _arguments: &serde_json::Value) -> Result<serde_json::Value> {
        Ok(serde_json::json!({ "ok": true }))
    }
}

#[test]
fn normalize_tool_args_preserves_native_keys_and_adds_aliases() {
    let normalized = normalize_tool_args(&serde_json::json!({
        "text": "payload",
        "sessionId": "session-1",
        "entities": [{"entityType": "person"}],
        "CommandLine": "cargo check",
        "Query": "openz",
        "Url": "https://example.com",
        "OutputPath": "/tmp/out.png"
    }));

    assert_eq!(normalized["text"], "payload");
    assert_eq!(normalized["content"], "payload");
    assert_eq!(normalized["sessionId"], "session-1");
    assert_eq!(normalized["session_id"], "session-1");
    assert_eq!(normalized["entities"][0]["entityType"], "person");
    assert_eq!(normalized["entities"][0]["entity_type"], "person");
    assert_eq!(normalized["CommandLine"], "cargo check");
    assert_eq!(normalized["command"], "cargo check");
    assert_eq!(normalized["Query"], "openz");
    assert_eq!(normalized["query"], "openz");
    assert_eq!(normalized["Url"], "https://example.com");
    assert_eq!(normalized["url"], "https://example.com");
    assert_eq!(normalized["OutputPath"], "/tmp/out.png");
    assert_eq!(normalized["output_path"], "/tmp/out.png");
}

#[test]
fn normalize_tool_args_adds_filesystem_path_aliases_without_overwriting_explicit_path() {
    let normalized = normalize_tool_args(&serde_json::json!({
        "TargetFile": "src/main.rs",
        "filepath": "src/lib.rs",
        "file": "README.md",
        "Path": "Cargo.toml",
        "AbsolutePath": "/tmp/out.txt",
        "DirectoryPath": "src"
    }));

    assert_eq!(normalized["TargetFile"], "src/main.rs");
    assert_eq!(normalized["target_file"], "src/main.rs");
    assert_eq!(normalized["filepath"], "src/lib.rs");
    assert_eq!(normalized["file"], "README.md");
    assert_eq!(normalized["Path"], "Cargo.toml");
    assert_eq!(normalized["path"], "Cargo.toml");
    assert_eq!(normalized["absolute_path"], "/tmp/out.txt");
    assert_eq!(normalized["directory_path"], "src");

    let explicit = normalize_tool_args(&serde_json::json!({
        "path": "explicit.txt",
        "Path": "Cargo.toml"
    }));
    assert_eq!(explicit["path"], "explicit.txt");
    assert_eq!(explicit["Path"], "Cargo.toml");
}

#[test]
fn normalize_tool_args_does_not_overwrite_explicit_aliases() {
    let normalized = normalize_tool_args(&serde_json::json!({
        "text": "native",
        "content": "explicit"
    }));

    assert_eq!(normalized["text"], "native");
    assert_eq!(normalized["content"], "explicit");
}

#[test]
fn registry_resolves_unambiguous_aliases_and_canonical_case() {
    let registry = ToolRegistry::new();
    registry.register(Arc::new(crate::tools::filesystem::ReadFileTool));

    assert_eq!(registry.get("READ_FILE").unwrap().name(), "read_file");
    assert_eq!(registry.get("open file").unwrap().name(), "read_file");
}

#[test]
fn registry_ignores_duplicate_canonical_registration() {
    let registry = ToolRegistry::new();
    registry.register(Arc::new(CacheTestTool {
        name: "read_file",
        domain: "filesystem",
        priority: 90,
    }));
    registry.register(Arc::new(CacheTestTool {
        name: "read_file",
        domain: "code",
        priority: 40,
    }));

    assert_eq!(registry.tool_count(), 1);
    assert_eq!(
        registry.get("read_file").unwrap().metadata().domain,
        "filesystem"
    );
}

#[test]
fn route_for_prompt_caches_same_prompt_filter_and_tools() {
    let registry = ToolRegistry::new();
    registry.register(Arc::new(CacheTestTool {
        name: "cargo_manager",
        domain: "code",
        priority: 90,
    }));

    let first = registry.route_for_prompt("run cargo test");
    let cached_after_first = registry.route_cache.lock().unwrap().clone();
    let second = registry.route_for_prompt("run cargo test");
    let cached_after_second = registry.route_cache.lock().unwrap().clone();

    assert_eq!(first.selected_domains, second.selected_domains);
    assert_eq!(first.selected_count, second.selected_count);
    assert_eq!(first.dropped_count, second.dropped_count);
    assert_eq!(
        cached_after_first.as_ref().map(|(key, _)| key.clone()),
        cached_after_second.as_ref().map(|(key, _)| key.clone())
    );
}

#[test]
fn capability_policy_deny_shell_blocks_shell_but_not_subagent_wrappers() {
    let config = Config::default();
    let provider = Arc::new(crate::providers::mock::MockProvider::new());
    let sessions = SessionManager::new(
        std::env::temp_dir().join(format!("openz-policy-wrapper-sessions-{}", uuid::Uuid::new_v4())),
    );
    let registry = ToolRegistry::new_with_context(config, provider, sessions);
    registry.set_capability_policy(Some(crate::orchestrator::spec::CapabilityPolicy {
        allowed_tools: vec![],
        denied_tools: vec![],
        deny_shell: true,
        deny_filesystem_write: false,
        deny_network: false,
    }));

    // Shell tools are blocked outright…
    assert!(registry.get("exec_command").is_none());
    assert!(registry.get("python_sandbox").is_none());
    // …while delegation stays available: the child agent loop inherits the
    // deny_shell policy (set_capability_policy on its registry), so it
    // cannot shell out either — blocking the wrapper would be redundant.
    assert!(registry.get("delegate_task").is_some());
    assert!(registry.get("parallel_research").is_some());
    assert!(registry.get("evaluator_optimizer_loop").is_some());
}

#[tokio::test]
async fn ordinary_subagent_cannot_access_orchestrator_tool() {
    let config = Config::default();
    let provider = Arc::new(crate::providers::mock::MockProvider::new());
    let sessions = SessionManager::new(
        std::env::temp_dir().join(format!("openz-nested-orchestrator-policy-sessions-{}", uuid::Uuid::new_v4())),
    );
    let registry = ToolRegistry::new_with_context(config, provider, sessions);

    crate::tools::subagent::ACTIVE_SUBAGENT
        .scope("coding_agent".to_string(), async {
            assert!(registry.get("orchestrate_workflow").is_none());
            let exposed_names = registry
                .to_openai_format_for_prompt("orchestrate workflow")
                .into_iter()
                .filter_map(|tool| tool["function"]["name"].as_str().map(str::to_string))
                .collect::<Vec<_>>();
            assert!(!exposed_names
                .iter()
                .any(|name| name == "orchestrate_workflow"));
        })
        .await;
}

#[tokio::test]
async fn orchestrated_worker_policy_blocks_nested_delegation_tools() {
    let config = Config::default();
    let provider = Arc::new(crate::providers::mock::MockProvider::new());
    let sessions = SessionManager::new(
        std::env::temp_dir().join(format!("openz-orchestrated-nested-delegation-sessions-{}", uuid::Uuid::new_v4())),
    );
    let registry = ToolRegistry::new_with_context(config, provider, sessions);

    assert!(registry.get("delegate_task").is_some());

    // Mirror production: the orchestrator scopes the nested-delegation flag
    // around delegate.call(), and execute_subagent_run scopes ACTIVE_SUBAGENT
    // around the child run where its registry lookups happen.
    crate::tools::subagent::ORCHESTRATED_NESTED_DELEGATION_ALLOWED
        .scope(false, async {
            crate::tools::subagent::ACTIVE_SUBAGENT
                .scope("coding_agent".to_string(), async {
                    assert!(registry.get("delegate_task").is_none());
                    let exposed_names = registry
                        .to_openai_format_for_prompt("delegate this task")
                        .into_iter()
                        .filter_map(|tool| {
                            tool["function"]["name"].as_str().map(str::to_string)
                        })
                        .collect::<Vec<_>>();
                    assert!(!exposed_names.iter().any(|name| name == "delegate_task"));
                })
                .await;
        })
        .await;
}

#[test]
fn capability_policy_blocks_dynamic_subagent_tool_lookup() {
    let config = Config::default();
    let provider = Arc::new(crate::providers::mock::MockProvider::new());
    let sessions = SessionManager::new(
        std::env::temp_dir().join(format!("openz-policy-sessions-{}", uuid::Uuid::new_v4())),
    );
    let registry = ToolRegistry::new_with_context(config, provider, sessions);
    registry.set_capability_policy(Some(crate::orchestrator::spec::CapabilityPolicy {
        allowed_tools: vec!["read_file".to_string()],
        denied_tools: vec!["coding_agent".to_string()],
        deny_shell: false,
        deny_filesystem_write: false,
        deny_network: false,
    }));

    assert!(registry.get("coding_agent").is_none());
    let exposed_names = registry
        .to_openai_format_for_prompt("")
        .into_iter()
        .filter_map(|tool| tool["function"]["name"].as_str().map(str::to_string))
        .collect::<Vec<_>>();
    assert!(!exposed_names.iter().any(|name| name == "coding_agent"));
}

#[test]
fn simple_prompt_does_not_expose_heavy_execution_tools() {
    let registry = ToolRegistry::new();
    registry.register(Arc::new(CacheTestTool {
        name: "exec_command",
        domain: "shell",
        priority: 90,
    }));
    registry.register(Arc::new(CacheTestTool {
        name: "openz_inventory",
        domain: "self_management",
        priority: 85,
    }));
    registry.register(Arc::new(CacheTestTool {
        name: "request_tool_scope",
        domain: "self_management",
        priority: 100,
    }));

    let exposed_names: Vec<String> = registry
        .to_openai_format_for_prompt("summarize hello")
        .into_iter()
        .filter_map(|value| value["function"]["name"].as_str().map(str::to_string))
        .collect();

    assert!(exposed_names.contains(&"openz_inventory".to_string()));
    assert!(exposed_names.contains(&"request_tool_scope".to_string()));
    assert!(!exposed_names.contains(&"exec_command".to_string()));
}

#[test]
fn repo_prompt_exposes_repo_read_tools() {
    let registry = ToolRegistry::new();
    registry.register(Arc::new(CacheTestTool {
        name: "grep_search",
        domain: "code",
        priority: 90,
    }));
    registry.register(Arc::new(CacheTestTool {
        name: "web_fetch",
        domain: "web",
        priority: 75,
    }));

    let exposed_names: Vec<String> = registry
        .to_openai_format_for_prompt("Where is orchestrate_workflow implemented in this repo?")
        .into_iter()
        .filter_map(|value| value["function"]["name"].as_str().map(str::to_string))
        .collect();

    assert!(exposed_names.contains(&"grep_search".to_string()));
    assert!(!exposed_names.contains(&"web_fetch".to_string()));
}

fn registry_with_named_tools(tools: &[(&'static str, &'static str)]) -> ToolRegistry {
    let registry = ToolRegistry::new();
    for (name, domain) in tools {
        registry.register(Arc::new(CacheTestTool {
            name: *name,
            domain: *domain,
            priority: 90,
        }));
    }
    registry
}

fn exposed_tool_names(registry: &ToolRegistry, prompt: &str) -> Vec<String> {
    registry
        .to_openai_format_for_prompt(prompt)
        .into_iter()
        .filter_map(|value| value["function"]["name"].as_str().map(str::to_string))
        .collect()
}

#[test]
fn current_external_prompt_exposes_web_research_pack() {
    let registry = registry_with_named_tools(&[
        ("web_search", "web"),
        ("web_fetch", "web"),
        ("grep_search", "code"),
    ]);
    let names = exposed_tool_names(&registry, "What is the latest Rust stable version today?");
    assert!(names.contains(&"web_search".to_string()));
    assert!(names.contains(&"web_fetch".to_string()));
    assert!(!names.contains(&"grep_search".to_string()));
}

#[test]
fn cron_prompt_exposes_cron_pack() {
    let registry = registry_with_named_tools(&[
        ("list_jobs", "cron"),
        ("get_job_logs", "cron"),
        ("web_fetch", "web"),
    ]);
    let names = exposed_tool_names(&registry, "what are my running cron jobs and logs?");
    assert!(names.contains(&"list_jobs".to_string()));
    assert!(names.contains(&"get_job_logs".to_string()));
    assert!(!names.contains(&"web_fetch".to_string()));
}

#[test]
fn orchestration_prompt_exposes_orchestrator_tool() {
    let registry = registry_with_named_tools(&[
        ("orchestrate_workflow", "subagent"),
        ("delegate_task", "subagent"),
        ("web_fetch", "web"),
    ]);
    let names = exposed_tool_names(
        &registry,
        "Use orchestrate_workflow to run a simple planner reviewer workflow",
    );
    assert!(names.contains(&"orchestrate_workflow".to_string()));
    assert!(names.contains(&"delegate_task".to_string()));
    assert!(!names.contains(&"web_fetch".to_string()));
}

#[test]
fn pending_tool_scope_is_applied_for_current_turn_only() {
    let registry = registry_with_named_tools(&[("open_path", "general")]);
    registry.request_tool_scope(["open_path".to_string()], Vec::new());
    let names = exposed_tool_names(&registry, "summarize hello");
    assert!(names.contains(&"open_path".to_string()));
    registry.begin_turn();
    let names = exposed_tool_names(&registry, "summarize hello");
    assert!(!names.contains(&"open_path".to_string()));
}

#[test]
fn explicit_vision_agent_request_is_recognized() {
    assert!(prompt_explicitly_requests_profile(
        "Use the vision agent for this image",
        "vision_agent"
    ));
}

#[test]
fn image_reference_is_recognized_for_vision_routing() {
    assert!(prompt_contains_image_reference(
        "Describe ![](file:///tmp/clipboard_image_0.png)"
    ));
    assert!(prompt_contains_image_reference("[image] what is this?"));
    assert!(!prompt_contains_image_reference(
        "Tell me about image processing in Rust"
    ));
}

#[test]
fn explicit_open_path_request_overrides_prompt_scope() {
    let registry = registry_with_named_tools(&[("open_path", "general")]);
    let names =
        exposed_tool_names(&registry, "Open this screenshot in the system image viewer");
    assert!(names.contains(&"open_path".to_string()));
}

#[test]
fn explicit_browser_request_exposes_browser_tools() {
    let registry =
        registry_with_named_tools(&[("firefox_browser", "web"), ("inspect_browsers", "web")]);
    let names = exposed_tool_names(&registry, "open Firefox and play a song on YouTube");
    assert!(names.contains(&"firefox_browser".to_string()));
    assert!(names.contains(&"inspect_browsers".to_string()));
}

#[test]
fn tool_router_status_line_reports_scope() {
    let registry = ToolRegistry::new();
    registry.register(Arc::new(CacheTestTool {
        name: "grep_search",
        domain: "code",
        priority: 90,
    }));

    let line = registry.tool_router_status_line("Where is this implemented in repo?");
    assert!(line.contains("Tool Router selected"));
    assert!(line.contains("code") || line.contains("reporead"));
}

#[test]
fn route_cache_invalidates_when_filter_scope_changes_or_tool_registers() {
    let registry = ToolRegistry::new();
    registry.register(Arc::new(CacheTestTool {
        name: "cargo_manager",
        domain: "code",
        priority: 90,
    }));
    let _ = registry.route_for_prompt("run cargo test");
    assert!(registry.route_cache.lock().unwrap().is_some());

    registry.set_filter_scope(Some(vec!["cargo".to_string()]));
    assert!(registry.route_cache.lock().unwrap().is_none());
    let _ = registry.route_for_prompt("run cargo test");
    assert!(registry.route_cache.lock().unwrap().is_some());

    registry.register(Arc::new(CacheTestTool {
        name: "web_fetch",
        domain: "web",
        priority: 80,
    }));
    assert!(registry.route_cache.lock().unwrap().is_none());
}
