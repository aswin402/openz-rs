//! Shared metadata types used by tool routing, resource policy, and security.
//!
//! The registry still owns the compatibility table for now, but the public
//! metadata contract lives here so consumers do not need to know how the
//! registry is stored.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolMetadata {
    /// Human-readable label used by CLI, activity, and approval surfaces.
    /// Dynamic tools derive this once when their metadata is constructed.
    pub presentation_name: String,
    pub domain: &'static str,
    pub risk: ToolRisk,
    pub uses_network: bool,
    pub writes_disk: bool,
    pub spawns_process: bool,
    pub requires_approval: bool,
    pub priority: u8,
    pub aliases: &'static [&'static str],
    pub examples: &'static [&'static str],
    pub when_to_use: &'static str,
    pub when_not_to_use: &'static str,
    /// Recommended timeout in seconds for this tool.
    /// None = use config default. Used when the LLM doesn't explicitly pass _timeout_secs.
    pub recommended_timeout_secs: Option<u64>,
}

/// Curated metadata specification for a statically registered tool.
///
/// Tool descriptions and JSON parameter schemas remain owned by each `Tool`
/// implementation; this record contains only cross-cutting routing/policy
/// data that can be shared by those consumers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolSpec {
    pub name: &'static str,
    /// Stable human-readable label for CLI and activity presentation.
    pub presentation_name: &'static str,
    pub domain: &'static str,
    pub risk: ToolRisk,
    /// Scope packs that should receive an explicit boost for this tool.
    /// Values are stable wire/policy labels kept as strings to avoid a
    /// dependency cycle between metadata and the scope policy module.
    pub packs: &'static [&'static str],
    pub writes_disk: bool,
    pub uses_network: bool,
    pub recommended_timeout_secs: Option<u64>,
    pub aliases: &'static [&'static str],
    pub examples: &'static [&'static str],
    pub when_to_use: &'static str,
    pub when_not_to_use: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolRisk {
    Low,
    Medium,
    High,
}

/// Normalize user/model supplied tool names for case-insensitive alias lookup.
/// Canonical names remain unchanged when emitted by the registry.
pub fn normalize_tool_name(name: &str) -> String {
    name.trim().to_ascii_lowercase()
}

/// Return the canonical static name for a curated alias. Unknown and dynamic
/// tool names are returned trimmed so profile policy can compare them without
/// changing their executable identity.
pub fn canonical_tool_name(name: &str) -> String {
    super::tool_spec(name)
        .map(|spec| spec.name.to_string())
        .unwrap_or_else(|| name.trim().to_string())
}

/// Compare tool names across policy and profile boundaries.
///
/// Curated aliases resolve to their canonical name, while dynamic names remain
/// executable identities and are compared case-insensitively after trimming.
pub fn tool_names_match(left: &str, right: &str) -> bool {
    normalize_tool_name(&canonical_tool_name(left))
        == normalize_tool_name(&canonical_tool_name(right))
}

fn title_case_identifier(name: &str) -> String {
    name.split('_')
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Convert a canonical tool name into the short label used by the CLI tree.
/// Aliases are resolved first so presentation remains consistent at every
/// caller boundary while the emitted/executed tool name stays canonical.
pub fn presentation_name(name: &str) -> String {
    if let Some(spec) = super::tool_spec(name) {
        return spec.presentation_name.to_string();
    }

    let canonical = super::canonical_tool_name(name);
    match canonical.as_str() {
        "api_integrator" => "API Integrator".to_string(),
        "ast_searcher" => "AST Searcher".to_string(),
        "git_ops_agent" => "Git Operations Agent".to_string(),
        "mcps_manager" => "MCPs Manager".to_string(),
        "openz_coordinator" | "openz_maintainer" => {
            format!(
                "OpenZ {}",
                title_case_identifier(canonical.strip_prefix("openz_").unwrap_or_default())
            )
        }
        "sop_designer" => "SOP Designer".to_string(),
        _ => title_case_identifier(&canonical),
    }
}

/// Return the compact label used in inline CLI tool-execution messages.
/// Keep this separate from the full activity-tree presentation label so the
/// existing concise CLI output remains stable while both labels share one
/// metadata boundary.
pub fn compact_presentation_name(name: &str) -> String {
    let canonical = canonical_tool_name(name);
    match canonical.as_str() {
        "grep_search" => "Search".to_string(),
        "read_file" | "view_file" => "Read".to_string(),
        "write_file"
        | "write_to_file"
        | "replace_file_content"
        | "multi_replace_file_content"
        | "patch_file"
        | "replace_lines" => "Edit".to_string(),
        "run_command" | "exec_command" => "Bash".to_string(),
        "list_dir" => "ListDir".to_string(),
        "code_outline" => "Outline".to_string(),
        "ast_grep" => "AstGrep".to_string(),
        "git_manager" => "Git".to_string(),
        "cargo_manager" => "Cargo".to_string(),
        "web_search" => "WebSearch".to_string(),
        "gsd_browser" => "Browser".to_string(),
        "clipboard" => "Clipboard".to_string(),
        "open_path" | "open" => "Open".to_string(),
        "web_fetch" | "read_url_content" | "read_url" => "Fetch".to_string(),
        "generate_image" => "Image".to_string(),
        "generate_video" => "Video".to_string(),
        "html_to_video" => "HtmlVideo".to_string(),
        "create_animated_svg" | "svg_animator" => "SvgAnim".to_string(),
        "obscura_browser" => "Obscura".to_string(),
        "db_inspector" => "DbInspect".to_string(),
        "db_write" => "DbWrite".to_string(),
        "read_doc" => "DocRead".to_string(),
        "crawl" => "Crawl".to_string(),
        "semantic_search" => "SemanticSearch".to_string(),
        "wasm_sandbox" => "Wasm".to_string(),
        "cron" => "Cron".to_string(),
        "watcher" => "Watcher".to_string(),
        _ => canonical,
    }
}

impl ToolRisk {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

/// Whether repeating this read/inspection call should be treated as stale
/// after one duplicate signature. Browser actions are classified by action
/// because navigation and mutation calls have different progress semantics.
pub(crate) fn is_progress_sensitive_repeat(name: &str, args: &serde_json::Value) -> bool {
    let name = canonical_tool_name(name);
    if matches!(
        name.as_str(),
        "web_fetch"
            | "web_search"
            | "crawl"
            | "crawl_site"
            | "social_search"
            | "searchxyz_search_web"
            | "searchxyz_read_url"
            | "searchxyz_search_and_read"
            | "searchxyz_deep_research"
            | "searchxyz_site_map"
            | "searchxyz_read_github_repo"
            | "read_file"
            | "list_dir"
            | "find_files"
            | "grep_search"
            | "ast_grep"
            | "code_outline"
            | "doc_reader"
            | "system_info"
            | "check_port"
            | "browser_status"
            | "git_manager"
            | "open_path"
            | "device_inventory"
            | "retrieve_original"
            | "scope_context"
            | "compress_content"
            | "search_nodes"
            | "open_nodes"
            | "read_graph"
            | "recall_memory"
            | "proactive_recall"
            | "semantic_search"
            | "rust_docs"
            | "get_working_memory"
            | "list_jobs"
    ) {
        return true;
    }

    if name == "gsd_browser" {
        return matches!(
            args.get("action").and_then(|value| value.as_str()),
            Some("snapshot" | "page_source" | "accessibility_tree" | "screenshot" | "eval")
        );
    }

    if name == "obscura_browser" || name == "firefox_browser" {
        return matches!(
            args.get("action").and_then(|value| value.as_str()),
            Some("render" | "screenshot" | "eval" | "page_source" | "accessibility_tree")
        );
    }

    false
}

pub(crate) fn tool_repetition_block_threshold(
    name: &str,
    args: &serde_json::Value,
) -> usize {
    if is_progress_sensitive_repeat(name, args) {
        1
    } else {
        2
    }
}

pub(crate) fn exact_arg_repeat_counts_without_result_signature(name: &str) -> bool {
    matches!(canonical_tool_name(name).as_str(), "grep_search" | "read_file" | "view_file")
}

pub(crate) fn is_state_changing_tool(name: &str) -> bool {
    matches!(
        canonical_tool_name(name).as_str(),
        "write_file"
            | "write_to_file"
            | "replace_file_content"
            | "multi_replace_file_content"
            | "patch_file"
            | "replace_lines"
            | "exec_command"
            | "run_command"
            | "cargo_manager"
            | "git_manager"
            | "db_write"
    )
}

fn infer_tool_domain(name: &str) -> &'static str {
    if let Some(def) = super::tool_spec(name) {
        return def.domain;
    }
    if matches!(
        name,
        "manage_servers"
            | "openz_inventory"
            | "workflow_memory"
            | "curate_skill"
            | "schedule_job"
            | "list_jobs"
            | "remove_job"
            | "pause_job"
            | "resume_job"
            | "get_job"
            | "get_job_logs"
            | "run_job_now"
            | "desktop_notify"
    ) {
        "self_management"
    } else if matches!(
        name,
        "delegate_task" | "parallel_research" | "evaluator_optimizer_loop"
    ) || name.contains("subagent")
    {
        "subagent"
    } else if matches!(
        name,
        "read_file"
            | "write_file"
            | "patch_file"
            | "replace_lines"
            | "list_dir"
            | "find_files"
            | "zenflow_edit"
    ) {
        "filesystem"
    } else if matches!(name, "exec_command" | "python_sandbox" | "wasm_sandbox") {
        "shell"
    } else if name.starts_with("git") || name.starts_with("github") {
        "git"
    } else if name.starts_with("cargo")
        || name.contains("compiler")
        || name.contains("grep")
        || name.contains("outline")
        || name.contains("ast_grep")
        || name.contains("rust_docs")
    {
        "code"
    } else if name.starts_with("web")
        || name.contains("browser")
        || name.contains("crawl")
        || name.starts_with("searchxyz")
        || name.contains("social_search")
    {
        "web"
    } else if name.contains("memory")
        || name.contains("entities")
        || name.contains("relations")
        || name.contains("observations")
        || name.contains("graph")
        || name.contains("recall")
    {
        "memory"
    } else if name.contains("headroom")
        || name.contains("compress")
        || name.contains("cache")
        || name.contains("scope_context")
    {
        "context"
    } else if name.contains("thinking") || name.contains("reasoning") {
        "reasoning"
    } else if name.starts_with("opendoc") || name.starts_with("docs_") || name.contains("document")
    {
        "document"
    } else if name.starts_with("openmedia")
        || name.contains("image")
        || name.contains("video")
        || name.contains("svg")
        || name.contains("mermaid")
    {
        "media"
    } else if name.contains("config")
        || name.contains("diagnose")
        || name.contains("session")
        || name.contains("backup")
        || name.contains("tool_catalog")
        || name.contains("tool_scope")
    {
        "self_management"
    } else if name.starts_with("mcp") || name.contains("mcp") {
        "mcp"
    } else {
        "general"
    }
}

fn tool_writes_disk(name: &str) -> bool {
    if let Some(def) = super::tool_spec(name) {
        return def.writes_disk;
    }
    matches!(
        name,
        "write_file"
            | "patch_file"
            | "replace_lines"
            | "zenflow_edit"
            | "db_write"
            | "manage_config"
            | "manage_sessions"
            | "manage_backups"
            | "curate_skill"
            | "schedule_job"
            | "create_subagent"
            | "delete_subagent"
            | "optimize_subagent"
    ) || name.contains("create")
        || name.contains("update")
        || name.contains("delete")
        || name.contains("remove")
        || name.contains("clear")
        || name.contains("import")
        || name.contains("download")
}

fn tool_uses_network(name: &str) -> bool {
    if let Some(def) = super::tool_spec(name) {
        return def.uses_network;
    }
    matches!(
        name,
        "web_fetch" | "web_search" | "social_search" | "check_port"
    ) || name.starts_with("searchxyz")
        || name.starts_with("github")
        || name.starts_with("docs_install")
        || name.contains("browser")
        || name.contains("download")
        || name.contains("mcp")
}

fn tool_recommended_timeout(name: &str) -> Option<u64> {
    if let Some(def) = super::tool_spec(name) {
        return def.recommended_timeout_secs;
    }
    // Dynamic tool families only — named tools belong in the static registry
    // so name drift is caught by the registration drift test.
    if name.contains("browser") || name.contains("obscura") {
        Some(600)
    } else if name.starts_with("opendoc_") {
        Some(300)
    } else if name.starts_with("mcp_") {
        Some(180)
    } else {
        None
    }
}

fn tool_aliases(name: &str, domain: &str) -> &'static [&'static str] {
    if let Some(def) = super::tool_spec(name) {
        return def.aliases;
    }
    match domain {
        "code" => &["code search", "compile", "test", "refactor"],
        "filesystem" => &["file", "directory", "edit file"],
        "web" => &["website", "browser", "research online"],
        "media" => &["image", "video", "svg", "diagram"],
        "document" => &["pdf", "docx", "xlsx", "document"],
        "memory" => &["remember", "recall", "knowledge graph"],
        "subagent" => &["delegate", "worker", "specialist"],
        _ => &[],
    }
}

fn tool_examples(name: &str, domain: &str) -> &'static [&'static str] {
    if let Some(def) = super::tool_spec(name) {
        return def.examples;
    }
    match domain {
        "code" => &["Analyze or modify source code"],
        "web" => &["Research a website or URL"],
        "media" => &["Create or transform visual media"],
        "document" => &["Read, convert, or edit documents"],
        "memory" => &["Store or retrieve durable facts"],
        _ => &[],
    }
}

fn tool_usage_hints(name: &str, domain: &str) -> (&'static str, &'static str) {
    if let Some(def) = super::tool_spec(name) {
        return (def.when_to_use, def.when_not_to_use);
    }
    match domain {
        "code" => (
            "Use for source-code analysis, build, test, or refactor tasks.",
            "Avoid for non-code document or media tasks.",
        ),
        "filesystem" => (
            "Use for project-local file and directory operations.",
            "Avoid for web or provider operations.",
        ),
        "web" => (
            "Use for URLs, browsers, crawling, and online research.",
            "Avoid for local-only codebase questions.",
        ),
        "media" => (
            "Use for image, video, SVG, Mermaid, and rendering tasks.",
            "Avoid for plain text or source-code edits.",
        ),
        "document" => (
            "Use for PDF, DOCX, XLSX, PPTX, and document conversion tasks.",
            "Avoid for source-code builds or shell commands.",
        ),
        "memory" => (
            "Use for durable facts, recall, graph memory, and knowledge retrieval.",
            "Avoid for transient one-turn calculations.",
        ),
        "subagent" => (
            "Use for delegated specialist tasks and parallel work.",
            "Avoid for simple single-step local tool calls.",
        ),
        "self_management" => (
            "Use for OpenZ diagnostics, config, sessions, and tool routing introspection.",
            "Avoid for user project modifications.",
        ),
        _ => ("", ""),
    }
}

impl ToolMetadata {
    pub fn infer(name: &str) -> Self {
        let canonical_name = canonical_tool_name(name);
        let name = canonical_name.as_str();
        let domain = infer_tool_domain(name);
        let writes_disk = tool_writes_disk(name);
        let spawns_process = matches!(name, "exec_command" | "python_sandbox")
            || name.contains("browser")
            || name.starts_with("cargo_")
            || name.starts_with("openmedia_video_")
            || name.starts_with("opendoc_convert")
            || name.starts_with("mcp_");
        let uses_network = tool_uses_network(name);
        let risk = super::tool_spec(name).map(|spec| spec.risk).unwrap_or_else(|| {
            if matches!(name, "exec_command" | "db_write")
                || writes_disk
                || name.contains("delete")
                || name.contains("remove")
                || name.contains("restore")
                || name.contains("clear")
            {
                ToolRisk::High
            } else if uses_network
                || spawns_process
                || name.contains("create")
                || name.contains("update")
            {
                ToolRisk::Medium
            } else {
                ToolRisk::Low
            }
        });
        let requires_approval = matches!(risk, ToolRisk::High);
        let priority = match domain {
            "subagent" => 100,
            "filesystem" | "shell" | "code" => 90,
            "self_management" => 85,
            "search" | "web" | "git" => 75,
            "cron" => 85,
            "memory" | "reasoning" | "context" => 65,
            "media" | "document" => 55,
            _ => 40,
        };
        let (when_to_use, when_not_to_use) = tool_usage_hints(name, domain);

        Self {
            presentation_name: super::presentation_name(name),
            domain,
            risk,
            uses_network,
            writes_disk,
            spawns_process,
            requires_approval,
            priority,
            aliases: tool_aliases(name, domain),
            examples: tool_examples(name, domain),
            when_to_use,
            when_not_to_use,
            recommended_timeout_secs: tool_recommended_timeout(name),
        }
    }

    /// Unknown tools remain behind an approval gate when no metadata signal
    /// identifies a safer capability class. Known domains and tools with
    /// explicit network/process/disk signals use their existing policy rules.
    pub fn requires_conservative_approval(&self) -> bool {
        self.domain == "general"
            && matches!(self.risk, ToolRisk::Low)
            && !self.uses_network
            && !self.writes_disk
            && !self.spawns_process
            && !self.requires_approval
    }

    /// Whether the shared risk metadata requires an interactive approval.
    /// Argument-sensitive exceptions are evaluated by the security guard
    /// before callers use this fallback.
    pub fn requires_risk_approval(&self) -> bool {
        matches!(self.risk, ToolRisk::High) || self.requires_approval
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "presentation_name": &self.presentation_name,
            "domain": self.domain,
            "risk": self.risk.as_str(),
            "uses_network": self.uses_network,
            "writes_disk": self.writes_disk,
            "spawns_process": self.spawns_process,
            "requires_approval": self.requires_approval,
            "priority": self.priority,
            "aliases": self.aliases,
            "examples": self.examples,
            "when_to_use": self.when_to_use,
            "when_not_to_use": self.when_not_to_use,
            "recommended_timeout_secs": self.recommended_timeout_secs,
        })
    }
}
