//! Registry mechanics and dynamic tool resolution for OpenZ.
//!
//! Owns [`ToolRegistry`], tool name normalization, alias indexing, dynamic subagent
//! resolution, capability policy enforcement, and route analysis.

use super::{all_tool_specs, normalize_tool_name, tool_spec, Tool, ToolMetadata};
use crate::config::schema::Config;
use crate::providers::LLMProvider;
use crate::session::SessionManager;
use crate::tools::routing;
use crate::tools::subagent::CancellationToken;
#[cfg(test)]
use std::collections::BTreeSet;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[cfg(test)]
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct StaticToolDriftReport {
    pub(crate) missing_registered: Vec<String>,
    pub(crate) noncanonical_registered: Vec<String>,
}

#[cfg(test)]
impl StaticToolDriftReport {
    pub(crate) fn is_clean(&self) -> bool {
        self.missing_registered.is_empty() && self.noncanonical_registered.is_empty()
    }
}

fn identity_keys(name: &str) -> Vec<String> {
    let canonical = normalize_tool_name(name);
    let mut keys = vec![canonical.clone()];
    if let Some(spec) = tool_spec(&canonical) {
        keys.extend(
            spec.aliases
                .iter()
                .map(|alias| normalize_tool_name(alias)),
        );
    }
    keys.sort();
    keys.dedup();
    keys
}

pub(crate) fn insert_unique_tool(
    tools: &mut HashMap<String, Arc<dyn Tool>>,
    tool: Arc<dyn Tool>,
) -> bool {
    let name = tool.name().to_string();
    let new_keys = identity_keys(&name);
    if let Some(existing) = tools.values().find(|existing| {
        identity_keys(existing.name())
            .iter()
            .any(|key| new_keys.iter().any(|new_key| new_key == key))
    }) {
        tracing::error!(
            tool = %name,
            existing_tool = %existing.name(),
            "Ignoring tool registration with a conflicting canonical name or alias"
        );
        return false;
    }
    tools.insert(name, tool);
    true
}

pub(crate) fn resolve_static_name(
    tools: &HashMap<String, Arc<dyn Tool>>,
    requested: &str,
) -> Option<String> {
    let requested = normalize_tool_name(requested);
    if requested.is_empty() {
        return None;
    }

    let mut matches = tools
        .values()
        .filter_map(|tool| {
            let canonical = normalize_tool_name(tool.name());
            // Only curated aliases are executable compatibility names.
            // Broad inferred labels (for example, "delegate" for subagents)
            // are deliberately excluded because they are ambiguous.
            let aliases = tool_spec(&canonical)
                .map(|spec| spec.aliases)
                .unwrap_or(&[]);
            let is_match = canonical == requested
                || aliases
                    .iter()
                    .any(|alias| normalize_tool_name(alias) == requested)
                || match requested.as_str() {
                    "doc_reader" | "read_document_file" => canonical == "read_doc",
                    "read_document" => canonical == "opendoc_read_document_text",
                    "opendoc_extract_tables" => canonical == "opendoc_find_tables",
                    "opendoc_convert_document" => canonical == "opendoc_convert",
                    _ => false,
                };
            if is_match {
                Some(tool.name().to_string())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    matches.sort();
    matches.dedup();
    (matches.len() == 1).then(|| matches.remove(0))
}

/// Compare curated metadata definitions with the tools currently registered.
/// Dynamic integrations are intentionally ignored: only names resolvable by
/// `tool_spec` participate in the reverse check.
#[cfg(test)]
pub(crate) fn static_tool_drift(
    tools: &HashMap<String, Arc<dyn Tool>>,
) -> StaticToolDriftReport {
    let registered_curated_names: BTreeSet<String> = tools
        .values()
        .filter_map(|tool| tool_spec(tool.name()).map(|spec| spec.name.to_string()))
        .collect();

    let mut report = StaticToolDriftReport {
        missing_registered: all_tool_specs()
            .iter()
            .filter(|spec| !registered_curated_names.contains(spec.name))
            .map(|spec| spec.name.to_string())
            .collect(),
        noncanonical_registered: tools
            .values()
            .filter_map(|tool| {
                let spec = tool_spec(tool.name())?;
                (tool.name() != spec.name).then(|| {
                    format!(
                        "{} is registered for curated tool '{}'",
                        tool.name(),
                        spec.name
                    )
                })
            })
            .collect(),
    };
    report.noncanonical_registered.sort();
    report
}

pub(crate) fn format_tool_description(description: &str, metadata: &ToolMetadata) -> String {
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

    pub fn request_tool_scope_for_prompt(&self, prompt: &str) {
        let selected_domains = routing::select_domains_for_prompt(prompt);
        let static_tools = self.read_tools();
        let explicit_tools = routing::explicitly_requested_tool_names(prompt, &static_tools);
        self.request_tool_scope(
            explicit_tools,
            selected_domains.into_iter().map(|d| d.to_string()),
        );
    }

    pub fn current_turn_scope(&self) -> (Vec<String>, Vec<String>) {
        if let Ok(pending) = self.pending_scope.lock() {
            let mut tools: Vec<String> = pending.tools.iter().cloned().collect();
            let mut domains: Vec<String> = pending.domains.iter().cloned().collect();
            tools.sort();
            domains.sort();
            (tools, domains)
        } else {
            (Vec::new(), Vec::new())
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

    pub fn active_tool_capabilities(&self) -> Vec<String> {
        let mut caps = Vec::new();
        if self.tool_allowed_by_active_policy("exec_command") {
            caps.push("terminal_execution".to_string());
        }
        if self.tool_allowed_by_active_policy("write_file") {
            caps.push("filesystem_mutations".to_string());
        }
        if self.tool_allowed_by_active_policy("web_search") {
            caps.push("live_network_research".to_string());
        }
        caps
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
        if !insert_unique_tool(&mut tools, tool) {
            return;
        }
        self.clear_route_cache();
    }

    pub fn register_all<I>(&self, tools: I)
    where
        I: IntoIterator<Item = Arc<dyn Tool>>,
    {
        for tool in tools {
            self.register(tool);
        }
    }

    pub fn register_canonical_static_defs(&self) {
        let mut tools = self.write_tools();
        for spec in all_tool_specs() {
            let canonical_name = spec.name;
            if tools.contains_key(canonical_name) {
                continue;
            }
            if let Some(existing_key) = tools.keys().find(|k| {
                k.eq_ignore_ascii_case(canonical_name)
                    || spec.aliases.iter().any(|&a| a.eq_ignore_ascii_case(k.as_str()))
            }) {
                let existing_key = existing_key.clone();
                let tool = tools.remove(&existing_key).unwrap();
                tools.insert(canonical_name.to_string(), tool);
            }
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

    pub fn is_empty(&self) -> bool {
        self.read_tools().is_empty()
    }

    pub fn contains(&self, name: &str) -> bool {
        self.read_tools().contains_key(name)
    }

    #[cfg(test)]
    pub(crate) fn static_tool_drift(&self) -> StaticToolDriftReport {
        let tools = self.read_tools();
        static_tool_drift(&tools)
    }

    fn resolve_static_name(&self, requested: &str) -> Option<String> {
        let tools = self.read_tools();
        resolve_static_name(&tools, requested)
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
        if !self
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
                    if !policy
                        .as_ref()
                        .map(|policy| {
                            crate::tools::orchestrator::tool_allowed_by_policy_with_metadata(
                                &profile.name,
                                &profile_metadata,
                                policy,
                            )
                        })
                        .unwrap_or(true)
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

pub(crate) fn prompt_explicitly_requests_profile(prompt: &str, profile_name: &str) -> bool {
    let lower = prompt.to_ascii_lowercase();
    let name = profile_name.to_ascii_lowercase();
    lower.contains(&name) || (profile_name == "vision_agent" && lower.contains("vision agent"))
}

pub(crate) fn prompt_contains_image_reference(prompt: &str) -> bool {
    let lower = prompt.to_ascii_lowercase();
    lower.contains("![](")
        || lower.contains("clipboard_image_")
        || lower.contains("[image")
        || lower.contains("image]")
        || lower.contains("attached image")
}

#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;
