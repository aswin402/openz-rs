//! Registry mechanics and dynamic tool resolution for OpenZ.
//!
//! Owns [`ToolRegistry`], tool name normalization, alias indexing, dynamic subagent
//! resolution, capability policy enforcement, and route analysis.

use super::{normalize_tool_name, tool_spec, Tool, ToolMetadata};
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
    let _ = tool_spec(&canonical).map(|spec| {
        keys.extend(
            spec.aliases
                .iter()
                .map(|alias| normalize_tool_name(alias)),
        );
    });
    keys.sort();
    keys.dedup();
    keys
}

fn expand_requested_domain(domain: &str) -> &'static [&'static str] {
    match domain {
        "repo_read" | "repo_write" | "repo" | "files" | "file" | "fs" => {
            &["filesystem", "code", "git"]
        }
        "local_exec" | "exec" | "bash" | "terminal" => &["shell", "code"],
        "web_research" | "research" | "search" | "browser" => &["web", "search"],
        "subagents" | "delegation" | "workflow" | "orchestration" | "orchestrator" => &["subagent"],
        "docs" | "doc" | "pdf" | "office" => &["document"],
        "visual" | "image" | "video" | "svg" | "diagram" => &["media"],
        "cron" | "scheduler" | "jobs" => &["cron", "self_management"],
        "admin" | "system" | "diagnostics" | "config" => &["self_management"],
        _ => &[],
    }
}

pub(crate) fn insert_unique_tool(
    tools: &mut HashMap<String, Arc<dyn Tool>>,
    tool: Arc<dyn Tool>,
) -> bool {
    let name = tool.name().to_string();
    let new_keys = identity_keys(&name);
    match tools.values().find(|existing| {
        identity_keys(existing.name())
            .iter()
            .any(|key| new_keys.iter().any(|new_key| new_key == key))
    }) {
        Some(existing) => {
            tracing::error!(
                tool = %name,
                existing_tool = %existing.name(),
                "Ignoring tool registration with a conflicting canonical name or alias"
            );
            false
        }
        None => {
            tools.insert(name, tool);
            true
        }
    }
}

pub(crate) fn resolve_static_name(
    tools: &HashMap<String, Arc<dyn Tool>>,
    requested: &str,
) -> Option<String> {
    let requested = normalize_tool_name(requested);
    (!requested.is_empty()).then_some(())?;
    let compat_canonical = crate::tools::metadata::compatibility_canonical_name(&requested);

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
                || compat_canonical == Some(canonical.as_str());
            is_match.then(|| tool.name().to_string())
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
        missing_registered: super::all_tool_specs()
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

const STICKY_TOOL_TTL_TURNS: u8 = 2;
const MAX_STICKY_TOOLS: usize = 8;

fn is_pinned_meta_tool(name: &str) -> bool {
    matches!(
        name,
        "tool_catalog" | "openz_inventory" | "request_tool_scope"
    )
}

pub(crate) fn format_tool_description(description: &str, metadata: &ToolMetadata) -> String {
    let lower_desc = description.to_ascii_lowercase();
    let mut parts = vec![description.to_string()];
    let _ = (!metadata.when_to_use.is_empty()
        && !lower_desc.contains(&metadata.when_to_use.to_ascii_lowercase()))
    .then(|| parts.push(format!("Use when: {}", metadata.when_to_use)));
    let _ = (!metadata.when_not_to_use.is_empty()
        && !lower_desc.contains(&metadata.when_not_to_use.to_ascii_lowercase()))
    .then(|| parts.push(format!("Avoid when: {}", metadata.when_not_to_use)));
    let _ = (!metadata.aliases.is_empty())
        .then(|| parts.push(format!("Aliases: {}.", metadata.aliases.join(", "))));
    let _ = metadata
        .examples
        .first()
        .filter(|example| !lower_desc.contains(&example.to_ascii_lowercase()))
        .map(|example| parts.push(format!("Example: {}.", example)));
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
    pending_tools: Vec<String>,
    pending_domains: Vec<String>,
    sticky_tools: Vec<String>,
    capability_policy: Option<crate::orchestrator::spec::CapabilityPolicy>,
}

#[derive(Clone)]
pub struct ToolRegistry {
    static_tools: Arc<std::sync::RwLock<HashMap<String, Arc<dyn Tool>>>>,
    pub context: Option<(Config, Arc<dyn LLMProvider>, SessionManager)>,
    pub filter_scope: Arc<std::sync::Mutex<Option<Vec<String>>>>,
    capability_policy: Arc<std::sync::Mutex<Option<crate::orchestrator::spec::CapabilityPolicy>>>,
    route_cache: Arc<std::sync::Mutex<Option<(ToolRouteCacheKey, ToolRouteAnalysis)>>>,
    pending_scope: Arc<std::sync::Mutex<PendingToolScope>>,
    sticky_tools: Arc<std::sync::Mutex<std::collections::BTreeMap<String, u8>>>,
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
            sticky_tools: Arc::new(std::sync::Mutex::new(std::collections::BTreeMap::new())),
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
            sticky_tools: Arc::new(std::sync::Mutex::new(std::collections::BTreeMap::new())),
        }
    }

    pub fn begin_turn(&self) {
        let _ = self
            .pending_scope
            .lock()
            .map(|mut pending| *pending = PendingToolScope::default());
        let _ = self.sticky_tools.lock().map(|mut sticky| {
            for ttl in sticky.values_mut() {
                *ttl = ttl.saturating_sub(1);
            }
            sticky.retain(|_, ttl| *ttl > 0);
        });
        self.clear_route_cache();
    }

    pub fn record_tool_used(&self, requested: &str) {
        let canonical = self.resolve_canonical_name(requested);
        let should_record = (!canonical.is_empty()) && (!is_pinned_meta_tool(&canonical));
        let _ = should_record.then(|| {
            let _ = self.sticky_tools.lock().map(|mut map| {
                map.insert(canonical, STICKY_TOOL_TTL_TURNS);
                while map.len() > MAX_STICKY_TOOLS {
                    let evict_key = map
                        .iter()
                        .min_by_key(|(k, ttl)| (**ttl, (*k).clone()))
                        .map(|(k, _)| k.clone());
                    match evict_key {
                        Some(k) => {
                            map.remove(&k);
                        }
                        None => break,
                    }
                }
            });
            self.clear_route_cache();
        });
    }

    pub fn current_sticky_tools(&self) -> Vec<String> {
        self.sticky_tools
            .lock()
            .ok()
            .map(|map| map.keys().cloned().collect())
            .unwrap_or_default()
    }

    pub fn seed_subagent_working_set(
        &self,
        goal: &str,
        context: &str,
        inherited_sticky: &[String],
    ) {
        let combined = format!("{goal}\n{context}");
        let static_tools = self.read_tools();
        let explicit_tools =
            routing::explicitly_requested_tool_names(&combined, &static_tools);
        let candidates: Vec<String> = inherited_sticky
            .iter()
            .cloned()
            .chain(explicit_tools)
            .filter_map(|name| {
                let canonical = self.resolve_canonical_name(&name);
                let is_valid = (!canonical.is_empty())
                    && (!is_pinned_meta_tool(&canonical))
                    && static_tools.contains_key(&canonical);
                is_valid.then_some(canonical)
            })
            .collect();
        drop(static_tools);

        let _ = self.sticky_tools.lock().map(|mut map| {
            let initial_ttl = STICKY_TOOL_TTL_TURNS.saturating_add(1);
            for canonical in candidates {
                map.insert(canonical, initial_ttl);
                while map.len() > MAX_STICKY_TOOLS {
                    let evict_key = map
                        .iter()
                        .min_by_key(|(k, ttl)| (**ttl, (*k).clone()))
                        .map(|(k, _)| k.clone());
                    match evict_key {
                        Some(k) => {
                            map.remove(&k);
                        }
                        None => break,
                    }
                }
            }
        });
        self.clear_route_cache();
    }

    pub fn request_tool_scope<I, J>(&self, tools: I, domains: J)
    where
        I: IntoIterator<Item = String>,
        J: IntoIterator<Item = String>,
    {
        let resolved_tools: Vec<String> = tools
            .into_iter()
            .map(|name| self.resolve_canonical_name(&name).to_ascii_lowercase())
            .collect();
        let mut resolved_domains: Vec<String> = Vec::new();
        for domain in domains {
            let normalized = domain.trim().to_ascii_lowercase();
            match normalized.is_empty() {
                true => {}
                false => {
                    for mapped in expand_requested_domain(&normalized) {
                        resolved_domains.push((*mapped).to_string());
                    }
                    resolved_domains.push(normalized);
                }
            }
        }
        let _ = self.pending_scope.lock().map(|mut pending| {
            pending.tools.extend(resolved_tools);
            pending.domains.extend(resolved_domains);
        });
        self.clear_route_cache();
    }

    pub fn current_turn_scope(&self) -> (Vec<String>, Vec<String>) {
        self.pending_scope
            .lock()
            .ok()
            .map(|pending| {
                let mut tools: Vec<String> = pending.tools.iter().cloned().collect();
                let mut domains: Vec<String> = pending.domains.iter().cloned().collect();
                tools.sort();
                domains.sort();
                (tools, domains)
            })
            .unwrap_or_default()
    }

    pub(crate) fn read_tools(&self) -> std::sync::RwLockReadGuard<'_, HashMap<String, Arc<dyn Tool>>> {
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
        let _ = self.route_cache.lock().map(|mut cache| *cache = None);
    }

    pub fn set_capability_policy(
        &self,
        policy: Option<crate::orchestrator::spec::CapabilityPolicy>,
    ) {
        let _ = self
            .capability_policy
            .lock()
            .map(|mut capability_policy| *capability_policy = policy);
        self.clear_route_cache();
    }

    fn tool_allowed_by_active_policy(&self, name: &str) -> bool {
        self.active_capability_policy()
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
        let _ = insert_unique_tool(&mut tools, tool).then(|| self.clear_route_cache());
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

    pub fn resolve_canonical_name(&self, requested: &str) -> String {
        self.resolve_static_name(requested)
            .unwrap_or_else(|| crate::tools::canonical_tool_name(requested))
    }

    fn active_cancellation_token() -> CancellationToken {
        crate::agent::agent_loop::current_turn_cancellation_context()
            .map(|ctx| ctx.token)
            .unwrap_or_default()
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
        let allowed_by_filter = filter.as_ref().is_none_or(|prefixes| {
            routing::tool_allowed_by_filter(name, Some(prefixes))
                || (!self.read_tools().contains_key(name)
                    && prefixes.iter().any(|prefix| prefix == "subagent"))
        });
        allowed_by_filter.then_some(())?;
        self.tool_allowed_by_active_policy(name).then_some(())?;
        self.tool_allowed_for_active_subagent(name).then_some(())?;

        match name {
            "orchestrate_workflow" => {
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
                        Self::active_cancellation_token(),
                        self.active_capability_policy(),
                    ),
                ));
            }
            "delegate_task" => {
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
                    cancellation_token: Self::active_cancellation_token(),
                    capability_policy: self.active_capability_policy(),
                });
                return self
                    .tool_allowed_by_active_policy_for_tool(tool.as_ref())
                    .then_some(tool);
            }
            "parallel_research" => {
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
                    cancellation_token: Self::active_cancellation_token(),
                    capability_policy: self.active_capability_policy(),
                });
                return self
                    .tool_allowed_by_active_policy_for_tool(tool.as_ref())
                    .then_some(tool);
            }
            "evaluator_optimizer_loop" => {
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
                        cancellation_token: Self::active_cancellation_token(),
                        capability_policy: self.active_capability_policy(),
                    });
                return self
                    .tool_allowed_by_active_policy_for_tool(tool.as_ref())
                    .then_some(tool);
            }
            _ => {}
        }

        // 2. Check static tools
        let static_match = self.read_tools().get(name).cloned();
        match static_match {
            Some(tool) => {
                return self
                    .tool_allowed_by_active_policy_for_tool(tool.as_ref())
                    .then_some(tool);
            }
            None => {}
        }

        // 3. If not found, check if it matches a custom subagent profile dynamically
        let (config, provider, session_manager) = self.context.as_ref()?;
        let active_subagent = crate::tools::subagent::ACTIVE_SUBAGENT
            .try_with(|s| s.clone())
            .unwrap_or_default();
        let subagent_allowed = active_subagent.is_empty()
            || (name != active_subagent
                && crate::tools::subagent::nested_delegation_allowed_for_active_context(
                    &active_subagent,
                ));
        subagent_allowed.then_some(())?;
        let profiles = crate::subagents::load_profiles().ok()?;
        let profile = profiles.into_iter().find(|p| p.name == name)?;
        let profile_metadata = crate::tools::subagent::subagent_tool_metadata(&profile.name);
        let policy_allowed = self
            .active_capability_policy()
            .as_ref()
            .map(|policy| {
                crate::tools::orchestrator::tool_allowed_by_policy_with_metadata(
                    &profile.name,
                    &profile_metadata,
                    policy,
                )
            })
            .unwrap_or(true);
        policy_allowed.then_some(())?;

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
            cancellation_token: Self::active_cancellation_token(),
            capability_policy: self.active_capability_policy(),
        }))
    }

    pub fn get_static_tools(&self) -> Vec<Arc<dyn Tool>> {
        let filter = self.filter_scope.lock().ok().and_then(|g| g.clone());
        self.read_tools()
            .values()
            .filter(|t| {
                let name = t.name();
                self.tool_allowed_by_active_policy_for_tool(t.as_ref())
                    && routing::tool_allowed_by_filter(name, filter.as_ref())
            })
            .cloned()
            .collect()
    }

    pub fn set_filter_scope(&self, prefixes: Option<Vec<String>>) {
        let _ = self.filter_scope.lock().map(|mut g| *g = prefixes);
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
                let exposure = entry.metadata.exposure(&entry.name).as_str();
                let annotations = entry.metadata.annotations_json();
                let mut value = serde_json::json!({
                    "name": entry.name,
                    "description": entry.description,
                    "domain": entry.metadata.domain,
                    "risk": entry.metadata.risk.as_str(),
                    "exposure": exposure,
                    "annotations": annotations,
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
                let _ = include_schema.then(|| {
                    value["parameters"] = entry.parameters;
                });
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
        let (pending_tools, pending_domains) = self.current_turn_scope();
        let sticky_tools = self.current_sticky_tools();
        let capability_policy = self.active_capability_policy();
        let static_tools = self.read_tools();
        let mut static_tool_names: Vec<String> = static_tools.keys().cloned().collect();
        static_tool_names.sort();
        let cache_key = ToolRouteCacheKey {
            prompt: prompt.to_string(),
            filter_scope: filter.clone(),
            static_tool_names: static_tool_names.clone(),
            pending_tools,
            pending_domains,
            sticky_tools: sticky_tools.clone(),
            capability_policy,
        };
        let cached = self.route_cache.lock().ok().and_then(|cache| {
            cache.as_ref().and_then(|(cached_key, cached_route)| {
                (cached_key == &cache_key).then(|| cached_route.clone())
            })
        });
        match cached {
            Some(route) => return route,
            None => {}
        }

        let routing_cfg = self
            .context
            .as_ref()
            .map(|(cfg, _, _)| cfg.agents.defaults.layered_tool_routing.clone())
            .unwrap_or_default();
        let max_visible = match routing_cfg.enabled {
            true => routing_cfg.max_visible_tools,
            false => usize::MAX,
        };
        let intent = crate::agent::agent_loop::intent::classify_turn_intent(prompt);
        let mut scope =
            crate::tools::scope::ToolScopeEngine::default().decide(&intent, max_visible);
        let always_visible_set: HashSet<String> =
            routing_cfg.always_visible_tools.iter().cloned().collect();
        scope
            .allowed_names
            .extend(routing_cfg.always_visible_tools);
        let pending_scope = self
            .pending_scope
            .lock()
            .map(|pending| pending.clone())
            .unwrap_or_default();
        let sticky_set: HashSet<String> = sticky_tools.into_iter().collect();
        let mut explicit_names = routing::explicitly_requested_tool_names(prompt, &static_tools);
        explicit_names.extend(pending_scope.tools.iter().cloned());
        let bm25_scores = routing::compute_bm25_scores(prompt, &static_tools);
        let selected_domains_set =
            routing::select_domains_with_bm25(prompt, &bm25_scores, &static_tools);
        let mut selected_domain_labels: std::collections::BTreeSet<String> = selected_domains_set
            .iter()
            .map(|domain| (*domain).to_string())
            .collect();
        for pack in &scope.packs {
            selected_domain_labels.insert(format!("{:?}", pack).to_lowercase());
        }
        let selected_domains: Vec<String> = selected_domain_labels.into_iter().collect();
        let _static_names: HashSet<String> = static_tool_names.into_iter().collect();
        let static_limit = match routing_cfg.enabled {
            true => scope
                .max_visible_tools
                .saturating_add(pending_scope.tools.len())
                .min(48),
            false => scope.max_visible_tools,
        };

        let mut entries: Vec<ToolRouteEntry> = static_tools
            .values()
            .filter(|tool| routing::tool_allowed_by_filter(tool.name(), filter.as_ref()))
            .map(|tool| {
                let metadata = tool.metadata();
                let bm25 = bm25_scores.get(tool.name()).copied().unwrap_or(0.0);
                let is_always_visible = always_visible_set.contains(tool.name());
                let is_sticky = sticky_set.contains(tool.name());
                let in_scope = !routing_cfg.enabled
                    || scope.allowed_names.contains(tool.name())
                    || crate::tools::scope::tool_matches_pack(tool.name(), &metadata, &scope.packs)
                    || is_sticky
                    || bm25 > 2.0;
                let base_score = routing::tool_selection_score_with_bm25(
                    tool.name(),
                    &metadata,
                    &selected_domains_set,
                    bm25,
                );
                let direct_tool_requested = explicit_names.contains(tool.name());
                let domain_requested = pending_scope.domains.contains(tool.metadata().domain);
                let explicitly_requested = direct_tool_requested || domain_requested;
                let scope_bonus = match (
                    is_always_visible,
                    direct_tool_requested,
                    is_sticky,
                    domain_requested,
                    in_scope,
                ) {
                    (true, _, _, _, _) => 30_000,
                    (false, true, _, _, _) => 20_000,
                    (false, false, true, _, _) => 12_000,
                    (false, false, false, true, _) => 10_000,
                    (false, false, false, false, true) => 100,
                    (false, false, false, false, false) => 0,
                };
                let selected_score = base_score.saturating_add(scope_bonus);
                let matched_prompt_domain = selected_domains_set.contains(metadata.domain);
                let mut selection_reason = routing::tool_selection_reasons_with_bm25(
                    tool.name(),
                    &metadata,
                    &selected_domains_set,
                    bm25,
                )
                .into_iter()
                .map(str::to_string)
                .collect::<Vec<_>>();
                let _ = in_scope.then(|| selection_reason.push("intent_scope".to_string()));
                let _ = is_sticky.then(|| selection_reason.push("session_continuity".to_string()));
                let _ = explicitly_requested
                    .then(|| selection_reason.push("explicit_tool_request".to_string()));
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
            let bm25 = bm25_scores.get(&entry.name).copied().unwrap_or(0.0);
            let is_sticky = sticky_set.contains(entry.name.as_str());
            let in_scope = !routing_cfg.enabled
                || scope.allowed_names.contains(entry.name.as_str())
                || crate::tools::scope::tool_matches_pack(
                    &entry.name,
                    &entry.metadata,
                    &scope.packs,
                )
                || is_sticky
                || bm25 > 2.0;
            let explicitly_requested = explicit_names.contains(entry.name.as_str())
                || pending_scope.domains.contains(entry.metadata.domain);
            let should_expose = (in_scope || explicitly_requested) && selected_count < static_limit;
            match should_expose {
                true => {
                    entry.exposed_to_model = true;
                    selected_count += 1;
                }
                false => {
                    entry.hidden_reason = Some(match in_scope {
                        true => "scope_limit",
                        false => "out_of_scope",
                    });
                }
            }
        }
        let dropped_count = entries.len().saturating_sub(selected_count);

        let route = ToolRouteAnalysis {
            selected_domains,
            selected_count,
            dropped_count,
            entries,
        };
        let _ = self
            .route_cache
            .lock()
            .map(|mut cache| *cache = Some((cache_key, route.clone())));
        route
    }

    pub fn tool_router_status_line(&self, prompt: &str) -> String {
        let route = self.route_for_prompt(prompt);
        let total = route.entries.len();
        let domains = match route.selected_domains.is_empty() {
            true => "none".to_string(),
            false => route.selected_domains.join(", "),
        };
        format!(
            "Tool Router selected {}/{} tools: {} · dropped {}",
            route.selected_count, total, domains, route.dropped_count
        )
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
        let routing_cfg = self
            .context
            .as_ref()
            .map(|(cfg, _, _)| cfg.agents.defaults.layered_tool_routing.clone())
            .unwrap_or_default();
        let max_visible = match routing_cfg.enabled {
            true => routing_cfg.max_visible_tools,
            false => usize::MAX,
        };
        let intent = crate::agent::agent_loop::intent::classify_turn_intent(prompt);
        let scope = crate::tools::scope::ToolScopeEngine::default().decide(&intent, max_visible);
        let pending_scope = self
            .pending_scope
            .lock()
            .map(|pending| pending.clone())
            .unwrap_or_default();
        let include_subagents = scope.packs.iter().any(|pack| {
            matches!(
                pack,
                crate::tools::scope::ToolPack::Subagent
                    | crate::tools::scope::ToolPack::Orchestrator
            )
        }) || prompt_explicitly_requests_profile(prompt, "vision_agent")
            || prompt_contains_image_reference(prompt)
            || pending_scope.domains.contains("subagent")
            || pending_scope.domains.contains("orchestrator")
            || pending_scope.tools.contains("vision_agent");
        let mut subagent_tools = match include_subagents {
            true => self.dynamic_subagent_tools_for_prompt(filter.as_ref(), &static_names, prompt),
            false => Vec::new(),
        };
        let route = self.route_for_prompt(prompt);
        let total_tools = route.entries.len() + subagent_tools.len();
        tracing::debug!(
            total_registered = total_tools,
            selected_static = route.selected_count,
            dropped_static = route.dropped_count,
            selected_domains = ?route.selected_domains,
            "Tool router selected dynamic JIT tool payload."
        );

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
        selected.sort_by(|a, b| {
            let name_a = a["function"]["name"].as_str().unwrap_or("");
            let name_b = b["function"]["name"].as_str().unwrap_or("");
            let tier_a = !routing::is_core_tool(name_a);
            let tier_b = !routing::is_core_tool(name_b);
            tier_a.cmp(&tier_b).then_with(|| name_a.cmp(name_b))
        });
        let max_subagent_slots = match routing_cfg.enabled {
            true => routing_cfg.max_visible_tools.max(24),
            false => 128usize.saturating_sub(selected.len()),
        };
        subagent_tools.truncate(max_subagent_slots.min(128usize.saturating_sub(selected.len())));
        selected.extend(subagent_tools);
        selected
    }

    fn dynamic_subagent_tools_for_prompt(
        &self,
        filter: Option<&Vec<String>>,
        static_names: &std::collections::HashSet<String>,
        prompt: &str,
    ) -> Vec<serde_json::Value> {
        let active_subagent = crate::tools::subagent::ACTIVE_SUBAGENT
            .try_with(|s| s.clone())
            .unwrap_or_default();
        let nested_allowed = active_subagent.is_empty()
            || crate::tools::subagent::nested_delegation_allowed_for_active_context(
                &active_subagent,
            );
        let policy = self.active_capability_policy();
        let prompt_lower = prompt.to_ascii_lowercase();
        let prompt_tokens: std::collections::HashSet<String> =
            routing::tokenize(prompt).into_iter().collect();

        let mut scored_subagent_tools: Vec<(i32, String, serde_json::Value)> = self
            .context
            .as_ref()
            .filter(|_| nested_allowed)
            .and_then(|_| crate::subagents::load_profiles().ok())
            .into_iter()
            .flatten()
            .filter(|profile| active_subagent.is_empty() || profile.name != active_subagent)
            .filter(|profile| {
                filter.is_none_or(|prefixes| {
                    prefixes
                        .iter()
                        .any(|prefix| profile.name.starts_with(prefix) || prefix == "subagent")
                })
            })
            .filter(|profile| {
                let profile_metadata =
                    crate::tools::subagent::subagent_tool_metadata(&profile.name);
                policy
                    .as_ref()
                    .map(|pol| {
                        crate::tools::orchestrator::tool_allowed_by_policy_with_metadata(
                            &profile.name,
                            &profile_metadata,
                            pol,
                        )
                    })
                    .unwrap_or(true)
            })
            .filter(|profile| !static_names.contains(&profile.name))
            .map(|profile| {
                let name_lower = profile.name.to_ascii_lowercase();
                let spaced = name_lower.replace('_', " ");
                let direct_bonus = match (
                    routing::contains_phrase_bounded(&prompt_lower, &name_lower),
                    spaced.len() >= 4 && routing::contains_phrase_bounded(&prompt_lower, &spaced),
                ) {
                    (true, _) => 1_000,
                    (false, true) => 800,
                    (false, false) => 0,
                };
                let profile_tokens = routing::tokenize(&format!(
                    "{} {}",
                    profile.name, profile.description
                ));
                let token_overlap = profile_tokens
                    .iter()
                    .filter(|tok| prompt_tokens.contains(*tok))
                    .count() as i32;
                let profile_metadata =
                    crate::tools::subagent::subagent_tool_metadata(&profile.name);
                let score = direct_bonus + (token_overlap * 50);
                let schema = serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": profile.name,
                        "description": format_tool_description(&profile.description, &profile_metadata),
                        "parameters": crate::tools::subagent::delegate_profile_parameters_schema()
                    }
                });
                (score, profile.name, schema)
            })
            .collect();
        scored_subagent_tools.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        scored_subagent_tools
            .into_iter()
            .map(|(_, _, schema)| schema)
            .collect()
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
