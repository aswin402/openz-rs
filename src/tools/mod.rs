use crate::config::schema::Config;
use crate::providers::LLMProvider;
use crate::session::SessionManager;
use crate::tools::subagent::CancellationToken;
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

pub mod arguments;
pub mod metadata;
pub(crate) mod registry;
pub(crate) mod routing;

pub use metadata::{
    canonical_tool_name, compact_presentation_name, normalize_tool_name, presentation_name,
    tool_names_match, ToolMetadata, ToolRisk, ToolSpec,
};

pub const MIN_TOOL_TIMEOUT_SECS: u64 = 5;
pub const MAX_TOOL_TIMEOUT_SECS: u64 = 1_800;

pub fn clamp_tool_timeout_secs(timeout_secs: u64) -> u64 {
    timeout_secs.clamp(MIN_TOOL_TIMEOUT_SECS, MAX_TOOL_TIMEOUT_SECS)
}

pub fn to_snake_case(s: &str) -> String {
    arguments::to_snake_case(s)
}

pub fn normalize_tool_args(args: &serde_json::Value) -> serde_json::Value {
    arguments::normalize_tool_args(args)
}

#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> serde_json::Value;
    fn metadata(&self) -> ToolMetadata {
        ToolMetadata::infer(self.name())
    }
    async fn call(&self, arguments: &serde_json::Value) -> Result<serde_json::Value>;
}

pub mod defs;
pub use defs::{all_tool_specs, static_tool_def_names, tool_spec, StaticToolDef, STATIC_TOOL_DEFS};

fn format_tool_description(description: &str, metadata: &ToolMetadata) -> String {
    let mut parts = vec![description.to_string()];
    if !metadata.when_to_use.is_empty() {
        parts.push(format!("Use when: {}", metadata.when_to_use));
    }
    if !metadata.when_not_to_use.is_empty() {
        parts.push(format!("Avoid when: {}", metadata.when_not_to_use));
    }
    if !metadata.aliases.is_empty() {
        parts.push(format!("Aliases: {}.", metadata.aliases.join(", ")));
    }
    if let Some(example) = metadata.examples.first() {
        parts.push(format!("Example: {}.", example));
    }
    parts.join(" ")
}

#[derive(Debug, Clone)]
pub struct ToolRouteEntry {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
    pub metadata: ToolMetadata,
    pub selected_score: i32,
    pub matched_prompt_domain: bool,
    pub selection_reason: Vec<String>,
    pub exposed_to_model: bool,
    pub hidden_reason: Option<&'static str>,
}

#[derive(Debug, Clone)]
pub struct ToolRouteAnalysis {
    pub selected_domains: Vec<String>,
    pub selected_count: usize,
    pub dropped_count: usize,
    pub entries: Vec<ToolRouteEntry>,
}

#[derive(Debug, Clone, Default)]
struct PendingToolScope {
    tools: HashSet<String>,
    domains: HashSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ToolRouteCacheKey {
    prompt: String,
    filter_scope: Option<Vec<String>>,
    static_tool_names: Vec<String>,
}

#[derive(Clone)]
pub struct ToolRegistry {
    static_tools: Arc<std::sync::RwLock<HashMap<String, Arc<dyn Tool>>>>,
    pub context: Option<(Config, Arc<dyn LLMProvider>, SessionManager)>,
    pub filter_scope: Arc<std::sync::Mutex<Option<Vec<String>>>>,
    capability_policy: Arc<std::sync::Mutex<Option<crate::orchestrator::spec::CapabilityPolicy>>>,
    route_cache: Arc<std::sync::Mutex<Option<(ToolRouteCacheKey, ToolRouteAnalysis)>>>,
    pending_scope: Arc<std::sync::Mutex<PendingToolScope>>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {
    pub fn new() -> Self {
        ToolRegistry {
            static_tools: Arc::new(std::sync::RwLock::new(HashMap::new())),
            context: None,
            filter_scope: Arc::new(std::sync::Mutex::new(None)),
            capability_policy: Arc::new(std::sync::Mutex::new(None)),
            route_cache: Arc::new(std::sync::Mutex::new(None)),
            pending_scope: Arc::new(std::sync::Mutex::new(PendingToolScope::default())),
        }
    }

    pub fn new_with_context(
        config: Config,
        provider: Arc<dyn LLMProvider>,
        session_manager: SessionManager,
    ) -> Self {
        ToolRegistry {
            static_tools: Arc::new(std::sync::RwLock::new(HashMap::new())),
            context: Some((config, provider, session_manager)),
            filter_scope: Arc::new(std::sync::Mutex::new(None)),
            capability_policy: Arc::new(std::sync::Mutex::new(None)),
            route_cache: Arc::new(std::sync::Mutex::new(None)),
            pending_scope: Arc::new(std::sync::Mutex::new(PendingToolScope::default())),
        }
    }

    pub fn begin_turn(&self) {
        if let Ok(mut pending) = self.pending_scope.lock() {
            *pending = PendingToolScope::default();
        }
        if let Ok(mut cache) = self.route_cache.lock() {
            *cache = None;
        }
    }

    pub fn request_tool_scope<I, J>(&self, tools: I, domains: J)
    where
        I: IntoIterator<Item = String>,
        J: IntoIterator<Item = String>,
    {
        if let Ok(mut pending) = self.pending_scope.lock() {
            pending
                .tools
                .extend(tools.into_iter().map(|name| name.to_ascii_lowercase()));
            pending.domains.extend(
                domains
                    .into_iter()
                    .map(|domain| domain.to_ascii_lowercase()),
            );
        }
        if let Ok(mut cache) = self.route_cache.lock() {
            *cache = None;
        }
    }

    fn read_tools(&self) -> std::sync::RwLockReadGuard<'_, HashMap<String, Arc<dyn Tool>>> {
        self.static_tools.read().unwrap_or_else(|p| {
            tracing::warn!("static_tools read lock poisoned; recovering");
            p.into_inner()
        })
    }

    fn write_tools(&self) -> std::sync::RwLockWriteGuard<'_, HashMap<String, Arc<dyn Tool>>> {
        self.static_tools.write().unwrap_or_else(|p| {
            tracing::warn!("static_tools write lock poisoned; recovering");
            p.into_inner()
        })
    }

    fn clear_route_cache(&self) {
        if let Ok(mut cache) = self.route_cache.lock() {
            *cache = None;
        }
    }

    pub fn set_capability_policy(
        &self,
        policy: Option<crate::orchestrator::spec::CapabilityPolicy>,
    ) {
        if let Ok(mut capability_policy) = self.capability_policy.lock() {
            *capability_policy = policy;
        }
        self.clear_route_cache();
    }

    fn tool_allowed_by_active_policy(&self, name: &str) -> bool {
        let policy = self.capability_policy.lock().ok().and_then(|g| g.clone());
        policy
            .as_ref()
            .map(|policy| crate::tools::orchestrator::tool_allowed_by_policy(name, policy))
            .unwrap_or(true)
    }

    fn tool_allowed_by_active_policy_with_metadata(
        &self,
        name: &str,
        metadata: &ToolMetadata,
    ) -> bool {
        self.active_capability_policy()
            .as_ref()
            .map(|policy| {
                crate::tools::orchestrator::tool_allowed_by_policy_with_metadata(
                    name, metadata, policy,
                )
            })
            .unwrap_or(true)
    }

    fn tool_allowed_by_active_policy_for_tool(&self, tool: &dyn Tool) -> bool {
        self.tool_allowed_by_active_policy_with_metadata(tool.name(), &tool.metadata())
    }

    fn active_capability_policy(&self) -> Option<crate::orchestrator::spec::CapabilityPolicy> {
        self.capability_policy.lock().ok().and_then(|g| g.clone())
    }

    fn collect_parent_tools_excluding(&self, excluded: &[&str]) -> Vec<Arc<dyn Tool>> {
        self.read_tools()
            .values()
            .filter(|tool| !excluded.contains(&tool.name()))
            .filter(|tool| self.tool_allowed_by_active_policy_for_tool(tool.as_ref()))
            .cloned()
            .collect()
    }

    pub fn register(&self, tool: Arc<dyn Tool>) {
        let mut tools = self.write_tools();
        if !registry::insert_unique_tool(&mut tools, tool) {
            return;
        }
        self.clear_route_cache();
    }

    pub fn tool_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.read_tools().keys().cloned().collect();
        names.sort();
        names
    }

    pub fn tool_count(&self) -> usize {
        self.read_tools().len()
    }

    #[cfg(test)]
    pub(crate) fn static_tool_drift(&self) -> registry::StaticToolDriftReport {
        let tools = self.read_tools();
        registry::static_tool_drift(&tools)
    }

    fn resolve_static_name(&self, requested: &str) -> Option<String> {
        let tools = self.read_tools();
        registry::resolve_static_name(&tools, requested)
    }

    pub fn tool_inventory_snapshot(&self) -> Vec<(String, String, ToolMetadata)> {
        let mut tools = self
            .read_tools()
            .values()
            .map(|tool| {
                (
                    tool.name().to_string(),
                    tool.description().to_string(),
                    tool.metadata(),
                )
            })
            .collect::<Vec<_>>();
        tools.sort_by(|a, b| a.0.cmp(&b.0));
        tools
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        let resolved_name = self.resolve_static_name(name);
        let name = resolved_name.as_deref().unwrap_or(name);
        let filter = self.filter_scope.lock().ok().and_then(|g| g.clone());
        if let Some(ref prefixes) = filter {
            if name != "delegate_task"
                && name != "send_remote_input"
                && name != "optimize_tool_scope"
                && name != "request_tool_scope"
                && !prefixes.iter().any(|prefix| name.starts_with(prefix))
            {
                return None;
            }
        }
        if !self.tool_allowed_by_active_policy(name) {
            return None;
        }

        if !self.tool_allowed_for_active_subagent(name) {
            return None;
        }

        if name == "orchestrate_workflow" {
            let (config, provider, session_manager) = self.context.as_ref()?;
            let parent_tools = self.collect_parent_tools_excluding(&[
                "delegate_task",
                "parallel_research",
                "evaluator_optimizer_loop",
                "orchestrate_workflow",
                "send_remote_input",
            ]);
            return Some(Arc::new(
                crate::tools::orchestrator::OrchestrateWorkflowTool::new(
                    config.clone(),
                    provider.clone(),
                    session_manager.clone(),
                    parent_tools,
                    CancellationToken::new(),
                    self.active_capability_policy(),
                ),
            ));
        }

        // 1. If name is "delegate_task", override and inject parent tools dynamically
        if name == "delegate_task" {
            let (config, provider, session_manager) = self.context.as_ref()?;
            let parent_tools = self.collect_parent_tools_excluding(&[
                "delegate_task",
                "parallel_research",
                "send_remote_input",
            ]);
            let tool: Arc<dyn Tool> = Arc::new(crate::tools::subagent::DelegateTaskTool {
                config: config.clone(),
                parent_provider: provider.clone(),
                session_manager: session_manager.clone(),
                parent_tools,
                cancellation_token: CancellationToken::new(),
                capability_policy: self.active_capability_policy(),
            });
            return self
                .tool_allowed_by_active_policy_for_tool(tool.as_ref())
                .then_some(tool);
        }

        // 1b. If name is "parallel_research", override and inject parent tools dynamically
        if name == "parallel_research" {
            let (config, provider, session_manager) = self.context.as_ref()?;
            let parent_tools = self.collect_parent_tools_excluding(&[
                "delegate_task",
                "parallel_research",
                "send_remote_input",
            ]);
            let tool: Arc<dyn Tool> = Arc::new(crate::tools::subagent::ParallelResearchTool {
                config: config.clone(),
                parent_provider: provider.clone(),
                session_manager: session_manager.clone(),
                parent_tools,
                cancellation_token: CancellationToken::new(),
                capability_policy: self.active_capability_policy(),
            });
            return self
                .tool_allowed_by_active_policy_for_tool(tool.as_ref())
                .then_some(tool);
        }

        // 1c. If name is "evaluator_optimizer_loop", override and inject parent tools dynamically
        if name == "evaluator_optimizer_loop" {
            let (config, provider, session_manager) = self.context.as_ref()?;
            let parent_tools = self.collect_parent_tools_excluding(&[
                "delegate_task",
                "parallel_research",
                "evaluator_optimizer_loop",
                "send_remote_input",
            ]);
            let tool: Arc<dyn Tool> =
                Arc::new(crate::tools::subagent::EvaluatorOptimizerLoopTool {
                    config: config.clone(),
                    parent_provider: provider.clone(),
                    session_manager: session_manager.clone(),
                    parent_tools,
                    cancellation_token: CancellationToken::new(),
                    capability_policy: self.active_capability_policy(),
                });
            return self
                .tool_allowed_by_active_policy_for_tool(tool.as_ref())
                .then_some(tool);
        }

        // 2. Check static tools
        if let Some(tool) = self.read_tools().get(name) {
            return self
                .tool_allowed_by_active_policy_for_tool(tool.as_ref())
                .then_some(tool.clone());
        }

        // 3. If not found, check if it matches a custom subagent profile dynamically
        let (config, provider, session_manager) = self.context.as_ref()?;
        let active_subagent = crate::tools::subagent::ACTIVE_SUBAGENT
            .try_with(|s| s.clone())
            .unwrap_or_default();
        if !active_subagent.is_empty() {
            if name == active_subagent {
                return None;
            }
            if !crate::tools::subagent::nested_delegation_allowed_for_active_context(
                &active_subagent,
            ) {
                return None;
            }
        }
        let profiles = crate::subagents::load_profiles().ok()?;
        let profile = profiles.into_iter().find(|p| p.name == name)?;
        let profile_metadata = crate::tools::subagent::subagent_tool_metadata(&profile.name);
        if self
            .active_capability_policy()
            .as_ref()
            .map(|policy| {
                crate::tools::orchestrator::tool_allowed_by_policy_with_metadata(
                    &profile.name,
                    &profile_metadata,
                    policy,
                )
            })
            .unwrap_or(true)
            == false
        {
            return None;
        }

        let parent_tools = self.collect_parent_tools_excluding(&[
            "delegate_task",
            "parallel_research",
            "send_remote_input",
        ]);

        Some(Arc::new(crate::tools::subagent::DelegateProfileTool {
            config: config.clone(),
            parent_provider: provider.clone(),
            session_manager: session_manager.clone(),
            profile,
            parent_tools,
            cancellation_token: CancellationToken::new(),
            capability_policy: self.active_capability_policy(),
        }))
    }

    pub fn get_static_tools(&self) -> Vec<Arc<dyn Tool>> {
        let filter = self.filter_scope.lock().ok().and_then(|g| g.clone());
        self.read_tools()
            .values()
            .filter(|t| {
                let name = t.name();
                if !self.tool_allowed_by_active_policy_for_tool(t.as_ref()) {
                    return false;
                }
                if let Some(ref prefixes) = filter {
                    name == "delegate_task"
                        || name == "send_remote_input"
                        || name == "optimize_tool_scope"
                        || prefixes.iter().any(|prefix| name.starts_with(prefix))
                } else {
                    true
                }
            })
            .cloned()
            .collect()
    }

    pub fn set_filter_scope(&self, prefixes: Option<Vec<String>>) {
        if let Ok(mut g) = self.filter_scope.lock() {
            *g = prefixes;
        }
        self.clear_route_cache();
    }

    pub fn selected_domains_for_prompt(&self, prompt: &str) -> Vec<String> {
        routing::select_domains_for_prompt(prompt)
            .into_iter()
            .map(str::to_string)
            .collect()
    }

    pub fn catalog_entries(&self, include_schema: bool) -> Vec<serde_json::Value> {
        self.catalog_entries_for_prompt(include_schema, "")
    }

    pub fn catalog_entries_for_prompt(
        &self,
        include_schema: bool,
        prompt: &str,
    ) -> Vec<serde_json::Value> {
        let mut entries: Vec<serde_json::Value> = self
            .route_for_prompt(prompt)
            .entries
            .into_iter()
            .map(|entry| {
                let selection_reason = entry.selection_reason.join(",");
                let mut value = serde_json::json!({
                    "name": entry.name,
                    "description": entry.description,
                    "domain": entry.metadata.domain,
                    "risk": entry.metadata.risk.as_str(),
                    "uses_network": entry.metadata.uses_network,
                    "writes_disk": entry.metadata.writes_disk,
                    "spawns_process": entry.metadata.spawns_process,
                    "requires_approval": entry.metadata.requires_approval,
                    "priority": entry.metadata.priority,
                    "aliases": entry.metadata.aliases,
                    "examples": entry.metadata.examples,
                    "when_to_use": entry.metadata.when_to_use,
                    "when_not_to_use": entry.metadata.when_not_to_use,
                    "selected_score": entry.selected_score,
                    "matched_prompt_domain": entry.matched_prompt_domain,
                    "selection_reason": selection_reason,
                    "exposed_to_model": entry.exposed_to_model,
                    "hidden_reason": entry.hidden_reason,
                });
                if include_schema {
                    value["parameters"] = entry.parameters;
                }
                value
            })
            .collect();

        entries.sort_by(|a, b| {
            let domain_a = a["domain"].as_str().unwrap_or("");
            let domain_b = b["domain"].as_str().unwrap_or("");
            domain_a.cmp(domain_b).then_with(|| {
                let name_a = a["name"].as_str().unwrap_or("");
                let name_b = b["name"].as_str().unwrap_or("");
                name_a.cmp(name_b)
            })
        });
        entries
    }

    pub fn route_for_prompt(&self, prompt: &str) -> ToolRouteAnalysis {
        let filter = self.filter_scope.lock().ok().and_then(|g| g.clone());
        let static_tools = self.read_tools();
        let mut static_tool_names: Vec<String> = static_tools.keys().cloned().collect();
        static_tool_names.sort();
        let cache_key = ToolRouteCacheKey {
            prompt: prompt.to_string(),
            filter_scope: filter.clone(),
            static_tool_names: static_tool_names.clone(),
        };
        if let Ok(cache) = self.route_cache.lock() {
            if let Some((cached_key, cached_route)) = cache.as_ref() {
                if cached_key == &cache_key {
                    return cached_route.clone();
                }
            }
        }

        let intent = crate::agent::agent_loop::intent::classify_turn_intent(prompt);
        let scope = crate::tools::scope::ToolScopeEngine::default().decide(&intent, 20);
        let pending_scope = self
            .pending_scope
            .lock()
            .map(|pending| pending.clone())
            .unwrap_or_default();
        let mut explicit_names = routing::explicitly_requested_tool_names(prompt, &static_tools);
        explicit_names.extend(pending_scope.tools.iter().cloned());
        let selected_domains_set = routing::select_domains_for_prompt(prompt);
        let mut selected_domain_labels: std::collections::BTreeSet<String> = selected_domains_set
            .iter()
            .map(|domain| (*domain).to_string())
            .collect();
        for pack in &scope.packs {
            selected_domain_labels.insert(format!("{:?}", pack).to_lowercase());
        }
        let selected_domains: Vec<String> = selected_domain_labels.into_iter().collect();
        let _static_names: HashSet<String> = static_tool_names.into_iter().collect();
        let static_limit = scope.max_visible_tools;

        let mut entries: Vec<ToolRouteEntry> = static_tools
            .values()
            .filter(|tool| routing::tool_allowed_by_filter(tool.name(), filter.as_ref()))
            .map(|tool| {
                let metadata = tool.metadata();
                let in_scope = scope.allowed_names.contains(tool.name())
                    || crate::tools::scope::tool_matches_pack(tool.name(), &metadata, &scope.packs);
                let base_score =
                    routing::tool_selection_score(tool.name(), &metadata, &selected_domains_set);
                let explicitly_requested = explicit_names.contains(tool.name())
                    || pending_scope.domains.contains(tool.metadata().domain);
                let selected_score = if explicitly_requested {
                    base_score.saturating_add(10_000)
                } else if in_scope {
                    base_score.saturating_add(100)
                } else {
                    base_score
                };
                let matched_prompt_domain = selected_domains_set.contains(metadata.domain);
                let mut selection_reason =
                    routing::tool_selection_reasons(tool.name(), &metadata, &selected_domains_set)
                        .into_iter()
                        .map(str::to_string)
                        .collect::<Vec<_>>();
                if in_scope {
                    selection_reason.push("intent_scope".to_string());
                }
                if explicitly_requested {
                    selection_reason.push("explicit_tool_request".to_string());
                }
                ToolRouteEntry {
                    name: tool.name().to_string(),
                    description: tool.description().to_string(),
                    parameters: tool.parameters(),
                    metadata,
                    selected_score,
                    matched_prompt_domain,
                    selection_reason,
                    exposed_to_model: false,
                    hidden_reason: None,
                }
            })
            .collect();
        drop(static_tools);

        entries.sort_by(|a, b| {
            b.selected_score
                .cmp(&a.selected_score)
                .then_with(|| a.name.cmp(&b.name))
        });

        let mut selected_count = 0usize;
        for entry in entries.iter_mut() {
            let in_scope = scope.allowed_names.contains(entry.name.as_str())
                || crate::tools::scope::tool_matches_pack(
                    &entry.name,
                    &entry.metadata,
                    &scope.packs,
                );
            let explicitly_requested = explicit_names.contains(entry.name.as_str())
                || pending_scope.domains.contains(entry.metadata.domain);
            if (in_scope || explicitly_requested) && selected_count < static_limit {
                entry.exposed_to_model = true;
                selected_count += 1;
            } else {
                entry.hidden_reason = Some(if in_scope {
                    "scope_limit"
                } else {
                    "out_of_scope"
                });
            }
        }
        let dropped_count = entries.len().saturating_sub(selected_count);

        let route = ToolRouteAnalysis {
            selected_domains,
            selected_count,
            dropped_count,
            entries,
        };
        if let Ok(mut cache) = self.route_cache.lock() {
            *cache = Some((cache_key, route.clone()));
        }
        route
    }

    pub fn tool_router_status_line(&self, prompt: &str) -> String {
        let route = self.route_for_prompt(prompt);
        let total = route.entries.len();
        let domains = if route.selected_domains.is_empty() {
            "none".to_string()
        } else {
            route.selected_domains.join(", ")
        };
        format!(
            "Tool Router selected {}/{} tools: {} · dropped {}",
            route.selected_count, total, domains, route.dropped_count
        )
    }

    pub fn to_openai_format(&self) -> Vec<serde_json::Value> {
        self.to_openai_format_for_prompt("")
    }

    fn tool_allowed_for_active_subagent(&self, name: &str) -> bool {
        let active_subagent = crate::tools::subagent::ACTIVE_SUBAGENT
            .try_with(|s| s.clone())
            .unwrap_or_default();
        active_subagent.is_empty()
            || crate::tools::subagent::nested_delegation_allowed_for_active_context(
                &active_subagent,
            )
            || !matches!(
                name,
                "delegate_task"
                    | "parallel_research"
                    | "evaluator_optimizer_loop"
                    | "orchestrate_workflow"
            )
    }

    pub fn to_openai_format_for_prompt(&self, prompt: &str) -> Vec<serde_json::Value> {
        let filter = self.filter_scope.lock().ok().and_then(|g| g.clone());
        let static_tools = self.read_tools();
        let static_names: HashSet<String> = static_tools.keys().cloned().collect();
        drop(static_tools);
        let intent = crate::agent::agent_loop::intent::classify_turn_intent(prompt);
        let scope = crate::tools::scope::ToolScopeEngine::default().decide(&intent, 20);
        let pending_scope = self
            .pending_scope
            .lock()
            .map(|pending| pending.clone())
            .unwrap_or_default();
        let mut subagent_tools = if scope.packs.iter().any(|pack| {
            matches!(
                pack,
                crate::tools::scope::ToolPack::Subagent
                    | crate::tools::scope::ToolPack::Orchestrator
            )
        }) || prompt_explicitly_requests_profile(prompt, "vision_agent")
            || prompt_contains_image_reference(prompt)
            || pending_scope.domains.contains("subagent")
            || pending_scope.domains.contains("orchestrator")
            || pending_scope.tools.contains("vision_agent")
        {
            self.dynamic_subagent_tools(filter.as_ref(), &static_names)
        } else {
            Vec::new()
        };
        let route = self.route_for_prompt(prompt);
        let total_tools = route.entries.len() + subagent_tools.len();
        if total_tools > 128 {
            tracing::warn!(
                total_tools,
                selected_static = route.selected_count,
                dropped_static = route.dropped_count,
                selected_domains = ?route.selected_domains,
                "Too many tools registered; selecting top 128 by prompt/domain priority."
            );
        } else {
            tracing::debug!(
                total_tools,
                selected_static = route.selected_count,
                selected_domains = ?route.selected_domains,
                "Tool router selected model tool payload."
            );
        }

        let mut selected: Vec<serde_json::Value> = route
            .entries
            .into_iter()
            .filter(|entry| {
                entry.exposed_to_model
                    && self.tool_allowed_for_active_subagent(&entry.name)
                    && self
                        .tool_allowed_by_active_policy_with_metadata(&entry.name, &entry.metadata)
            })
            .map(|entry| {
                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": entry.name,
                        "description": format_tool_description(&entry.description, &entry.metadata),
                        "parameters": entry.parameters,
                    }
                })
            })
            .collect();
        subagent_tools.truncate(128usize.saturating_sub(selected.len()));
        selected.extend(subagent_tools);
        selected
    }

    fn dynamic_subagent_tools(
        &self,
        filter: Option<&Vec<String>>,
        static_names: &std::collections::HashSet<String>,
    ) -> Vec<serde_json::Value> {
        let mut subagent_tools: Vec<serde_json::Value> = Vec::new();
        if let Some((_, _, _)) = &self.context {
            if let Ok(profiles) = crate::subagents::load_profiles() {
                let active_subagent = crate::tools::subagent::ACTIVE_SUBAGENT
                    .try_with(|s| s.clone())
                    .unwrap_or_default();
                if !active_subagent.is_empty()
                    && !crate::tools::subagent::nested_delegation_allowed_for_active_context(
                        &active_subagent,
                    )
                {
                    return subagent_tools;
                }
                for profile in profiles {
                    if !active_subagent.is_empty() && profile.name == active_subagent {
                        continue;
                    }
                    if let Some(prefixes) = filter {
                        if !prefixes
                            .iter()
                            .any(|prefix| profile.name.starts_with(prefix) || prefix == "subagent")
                        {
                            continue;
                        }
                    }
                    let profile_metadata =
                        crate::tools::subagent::subagent_tool_metadata(&profile.name);
                    let policy = self.active_capability_policy();
                    if policy
                        .as_ref()
                        .map(|policy| {
                            crate::tools::orchestrator::tool_allowed_by_policy_with_metadata(
                                &profile.name,
                                &profile_metadata,
                                policy,
                            )
                        })
                        .unwrap_or(true)
                        == false
                    {
                        continue;
                    }
                    if !static_names.contains(&profile.name) {
                        subagent_tools.push(serde_json::json!({
                            "type": "function",
                            "function": {
                                "name": profile.name,
                                "description": profile.description,
                                "parameters": serde_json::json!({
                                    "type": "object",
                                    "properties": {
                                        "goal": {
                                            "type": "string",
                                            "description": "The specific goal or task for this specialized subagent to accomplish."
                                        },
                                        "context": {
                                            "type": "string",
                                            "description": "Additional context or background details required for the task."
                                        }
                                    },
                                    "required": ["goal"]
                                })
                            }
                        }));
                    }
                }
            }
        }
        subagent_tools.sort_by(|a, b| {
            let name_a = a["function"]["name"].as_str().unwrap_or("");
            let name_b = b["function"]["name"].as_str().unwrap_or("");
            name_a.cmp(name_b)
        });
        subagent_tools
    }
}

fn prompt_explicitly_requests_profile(prompt: &str, profile_name: &str) -> bool {
    let lower = prompt.to_ascii_lowercase();
    let name = profile_name.to_ascii_lowercase();
    lower.contains(&name) || (profile_name == "vision_agent" && lower.contains("vision agent"))
}

fn prompt_contains_image_reference(prompt: &str) -> bool {
    let lower = prompt.to_ascii_lowercase();
    lower.contains("![](")
        || lower.contains("clipboard_image_")
        || lower.contains("[image")
        || lower.contains("image]")
        || lower.contains("attached image")
}

#[cfg(test)]
mod route_cache_tests {
    use super::*;

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
                presentation_name: crate::tools::presentation_name(&self.name),
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
        let sessions = SessionManager::new(std::path::PathBuf::from(
            "/tmp/openz-policy-wrapper-sessions",
        ));
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
        let sessions = SessionManager::new(std::path::PathBuf::from(
            "/tmp/openz-nested-orchestrator-policy-sessions",
        ));
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
        let sessions = SessionManager::new(std::path::PathBuf::from(
            "/tmp/openz-orchestrated-nested-delegation-sessions",
        ));
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
        let sessions = SessionManager::new(std::path::PathBuf::from("/tmp/openz-policy-sessions"));
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
}

pub mod ast_grep;
pub mod browser;
pub use browser::{
    broker as browser_broker,
    common as browser_common,
    firefox,
    gsd as gsd_browser,
    obscura,
    status as browser_status,
};
pub mod cargo_manager;
pub mod clipboard;
pub mod compiler_auto_heal;
pub mod crawl;
pub mod cron;
pub mod db_inspector;
pub mod desktop_notify;
pub mod device_inventory;
pub mod doc_reader;
pub mod docs_mcp;
pub mod filesystem;
pub mod get_logs;
pub mod git_manager;
pub mod github;
pub mod github_mcp;
pub mod graph_memory;
pub mod grep;
pub mod headroom;
pub mod html_video;
pub mod image_generator;
pub mod js_format;
pub mod manage_whitelist;
pub mod mcp;
pub mod mcp_manager;
pub mod memory_extra;
pub mod mermaid;
pub mod network;
pub mod notes;
pub mod onpkg;
pub mod open;
pub mod opendoc;
pub mod openmedia;
pub mod orchestrator;
pub mod outline;
pub mod remote;
pub mod resource_policy;
pub mod rust_docs;
pub mod scope_engine;
pub use scope_engine as scope;
#[path = "searchxyz/mod.rs"]
pub mod searchxyz;
pub mod self_management;
pub mod semantic_search;
pub mod sequential_thinking;
pub mod shared_memory;
pub mod shell;
pub mod social_search;
pub mod sop;
pub mod subagent;
pub mod svg_animator;
pub mod system_info;
pub mod task_manager;
pub mod telegram_send;
pub mod template_compiler;
pub mod video;
pub mod wasm_sandbox;
pub mod watcher;
pub mod web;
pub mod web_search;

#[cfg(test)]
mod static_def_tests {
    use super::*;

    /// Regression guard: these tools were previously referenced by misnamed
    /// match arms (html_video, crawl_site, svg_animator, mermaid) so their
    /// intended timeouts and network flags silently never applied.
    #[test]
    fn curated_defs_apply_intended_timeouts() {
        assert_eq!(
            ToolMetadata::infer("html_to_video").recommended_timeout_secs,
            Some(900)
        );
        assert_eq!(
            ToolMetadata::infer("generate_video").recommended_timeout_secs,
            Some(900)
        );
        assert_eq!(
            ToolMetadata::infer("crawl_website").recommended_timeout_secs,
            Some(600)
        );
        assert_eq!(
            ToolMetadata::infer("create_animated_svg").recommended_timeout_secs,
            Some(300)
        );
        assert_eq!(
            ToolMetadata::infer("generate_image").recommended_timeout_secs,
            Some(300)
        );
        assert_eq!(
            ToolMetadata::infer("render_mermaid").recommended_timeout_secs,
            Some(300)
        );
        assert_eq!(
            ToolMetadata::infer("semantic_search").recommended_timeout_secs,
            Some(300)
        );
        assert_eq!(
            ToolMetadata::infer("python_sandbox").recommended_timeout_secs,
            Some(180)
        );
    }

    #[test]
    fn crawl_website_is_a_network_tool() {
        let metadata = ToolMetadata::infer("crawl_website");
        assert!(metadata.uses_network);
        assert_eq!(metadata.domain, "web");
    }

    #[test]
    fn curated_defs_have_no_duplicate_names() {
        let names = static_tool_def_names();
        let unique: std::collections::BTreeSet<_> = names.iter().collect();
        assert_eq!(
            names.len(),
            unique.len(),
            "duplicate STATIC_TOOL_DEFS entry"
        );
    }
}
