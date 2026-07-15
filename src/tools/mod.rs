use crate::config::schema::Config;
use crate::providers::LLMProvider;
use crate::session::SessionManager;
use crate::tools::subagent::CancellationToken;
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

pub const MIN_TOOL_TIMEOUT_SECS: u64 = 5;
pub const MAX_TOOL_TIMEOUT_SECS: u64 = 1_800;

pub fn clamp_tool_timeout_secs(timeout_secs: u64) -> u64 {
    timeout_secs.clamp(MIN_TOOL_TIMEOUT_SECS, MAX_TOOL_TIMEOUT_SECS)
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolMetadata {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolRisk {
    Low,
    Medium,
    High,
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

impl ToolMetadata {
    pub fn infer(name: &str) -> Self {
        let domain = infer_tool_domain(name);
        let writes_disk = tool_writes_disk(name);
        let spawns_process = matches!(name, "exec_command" | "python_sandbox")
            || name.contains("browser")
            || name.starts_with("cargo_")
            || name.starts_with("openmedia_video_")
            || name.starts_with("opendoc_convert")
            || name.starts_with("mcp_");
        let uses_network = tool_uses_network(name);
        let risk = if matches!(name, "exec_command" | "db_write")
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
        };
        let requires_approval = matches!(risk, ToolRisk::High);
        let priority = match domain {
            "subagent" => 100,
            "filesystem" | "shell" | "code" => 90,
            "self_management" => 85,
            "search" | "web" | "git" => 75,
            "memory" | "reasoning" | "context" => 65,
            "media" | "document" => 55,
            _ => 40,
        };
        let (when_to_use, when_not_to_use) = tool_usage_hints(name, domain);

        Self {
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

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
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

fn infer_tool_domain(name: &str) -> &'static str {
    if matches!(
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
    matches!(
        name,
        "web_fetch" | "web_search" | "crawl_site" | "social_search" | "check_port"
    ) || name.starts_with("searchxyz")
        || name.starts_with("github")
        || name.starts_with("docs_install")
        || name.contains("browser")
        || name.contains("download")
        || name.contains("mcp")
}

fn tool_recommended_timeout(name: &str) -> Option<u64> {
    // Long-running tools get recommended timeouts so the orchestrator doesn't need
    // to explicitly set _timeout_secs on every call. These are hints; the config
    // default or a caller-supplied _timeout_secs still take precedence.
    match name {
        // Subagent delegation — full LLM loop with tool execution
        "delegate_task" | "parallel_research" | "evaluator_optimizer_loop" => Some(600),

        // Browser automation — CDP sessions with page load + interaction
        name if name.contains("browser") || name.contains("obscura") => Some(600),

        // Web crawling — multi-page spider
        "crawl_site" => Some(600),

        // Video generation — Chromium rendering or Wavyte API
        "html_video" | "generate_video" => Some(900),

        // Image generation — HTML/CSS/SVG render to PNG
        "generate_image" | "svg_animator" => Some(300),

        // Semantic search — indexing codebase
        "semantic_search" => Some(300),

        // MCP tools — external process communication
        name if name.starts_with("mcp_") => Some(180),

        // Shell commands — potentially long running
        "exec_command" | "python_sandbox" => Some(180),

        // HTML rendering via Mermaid
        "mermaid" => Some(300),

        // Document conversion
        name if name.starts_with("opendoc_") => Some(300),

        _ => None,
    }
}

fn tool_aliases(name: &str, domain: &str) -> &'static [&'static str] {
    match name {
        "cargo_manager" => &[
            "cargo test",
            "cargo check",
            "cargo build",
            "clippy",
            "rust tests",
        ],
        "exec_command" => &["shell command", "terminal", "bash", "run command"],
        "read_file" => &["open file", "inspect file", "view file"],
        "grep_search" => &["search code", "find text", "ripgrep"],
        "web_fetch" => &["fetch url", "read webpage", "download page"],
        "web_search" => &["internet search", "search web", "lookup online"],
        "delegate_task" => &["subagent", "delegate", "specialist agent"],
        "git_manager" => &["git status", "git diff", "git commit", "git log"],
        "tool_catalog" => &["list tools", "tool help", "available tools"],
        _ => match domain {
            "code" => &["code search", "compile", "test", "refactor"],
            "filesystem" => &["file", "directory", "edit file"],
            "web" => &["website", "browser", "research online"],
            "media" => &["image", "video", "svg", "diagram"],
            "document" => &["pdf", "docx", "xlsx", "document"],
            "memory" => &["remember", "recall", "knowledge graph"],
            "subagent" => &["delegate", "worker", "specialist"],
            _ => &[],
        },
    }
}

fn tool_examples(name: &str, domain: &str) -> &'static [&'static str] {
    match name {
        "cargo_manager" => &["Run cargo test --lib", "Run cargo check after Rust edits"],
        "exec_command" => &[
            "Run ls to inspect generated files",
            "Run a safe project-local command",
        ],
        "read_file" => &["Read src/main.rs before editing", "Inspect a config file"],
        "grep_search" => &["Find all uses of a function", "Search for a symbol in src"],
        "web_fetch" => &["Fetch a documentation URL", "Read one webpage"],
        "web_search" => &[
            "Search current public documentation",
            "Look up recent release info",
        ],
        "delegate_task" => &[
            "Ask a reviewer subagent to inspect changes",
            "Route image analysis to a vision subagent",
        ],
        "git_manager" => &["Check git status", "Review a diff before commit"],
        "tool_catalog" => &[
            "List tools for a website research task",
            "Explain why tools were hidden",
        ],
        _ => match domain {
            "code" => &["Analyze or modify source code"],
            "web" => &["Research a website or URL"],
            "media" => &["Create or transform visual media"],
            "document" => &["Read, convert, or edit documents"],
            "memory" => &["Store or retrieve durable facts"],
            _ => &[],
        },
    }
}

fn tool_usage_hints(name: &str, domain: &str) -> (&'static str, &'static str) {
    match name {
        "cargo_manager" => (
            "Use for Rust cargo build, check, test, clippy, and compiler-fix workflows.",
            "Avoid when only reading files or searching source text.",
        ),
        "exec_command" => (
            "Use for shell commands that cannot be handled by a safer native tool.",
            "Avoid for file reads, code search, or destructive commands without approval.",
        ),
        "read_file" => (
            "Use to inspect known text files before editing or explaining code.",
            "Avoid for broad searches; use grep or find tools instead.",
        ),
        "grep_search" => (
            "Use to find symbols, text, TODOs, and call sites across a project.",
            "Avoid when the exact file is already known and only needs reading.",
        ),
        "web_fetch" => (
            "Use to read a specific URL supplied by the user or found by search.",
            "Avoid for open-ended research; search first.",
        ),
        "web_search" => (
            "Use when current or external web information is required.",
            "Avoid when the answer is fully available from local project files.",
        ),
        "delegate_task" => (
            "Use for independent specialist work, reviews, research, or multimodal routing.",
            "Avoid for simple direct actions the orchestrator can complete itself.",
        ),
        "git_manager" => (
            "Use for git status, diffs, commit history, and repository state checks.",
            "Avoid for GitHub API operations; use GitHub tools for remote provider actions.",
        ),
        "tool_catalog" => (
            "Use to inspect available tools, routing decisions, and hidden tool reasons.",
            "Avoid when the correct tool is already obvious and exposed.",
        ),
        _ => match domain {
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
        },
    }
}

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

fn is_core_tool(name: &str) -> bool {
    matches!(
        name,
        "tool_catalog"
            | "optimize_tool_scope"
            | "diagnose_tool"
            | "delegate_task"
            | "send_remote_input"
            | "read_file"
            | "find_files"
            | "grep_search"
    )
}

fn tool_allowed_by_filter(name: &str, filter: Option<&Vec<String>>) -> bool {
    if let Some(prefixes) = filter {
        is_core_tool(name) || prefixes.iter().any(|prefix| name.starts_with(prefix))
    } else {
        true
    }
}

fn select_domains_for_prompt(prompt: &str) -> std::collections::BTreeSet<&'static str> {
    let lower = prompt.to_lowercase();
    let mut domains = std::collections::BTreeSet::new();
    domains.insert("self_management");
    domains.insert("filesystem");
    domains.insert("subagent");

    if contains_any(
        &lower,
        &[
            "cargo", "rust", "test", "build", "compile", "compiler", "error", "code", "function",
            "module", "refactor", "lint", "clippy",
        ],
    ) {
        domains.insert("code");
        domains.insert("shell");
        domains.insert("git");
    }
    if contains_any(
        &lower,
        &[
            "website", "web", "url", "browser", "page", "crawl", "fetch", "search", "internet",
            "research", "http", "https",
        ],
    ) {
        domains.insert("web");
    }
    if contains_any(
        &lower,
        &[
            "image",
            "photo",
            "picture",
            "screenshot",
            "svg",
            "video",
            "media",
            "mermaid",
            "diagram",
            "render",
        ],
    ) {
        domains.insert("media");
        domains.insert("document");
    }
    if contains_any(
        &lower,
        &[
            "pdf",
            "docx",
            "xlsx",
            "pptx",
            "document",
            "spreadsheet",
            "archive",
        ],
    ) {
        domains.insert("document");
    }
    if contains_any(
        &lower,
        &[
            "git", "commit", "push", "pull", "pr", "github", "branch", "diff",
        ],
    ) {
        domains.insert("git");
        domains.insert("code");
    }
    if contains_any(
        &lower,
        &["memory", "remember", "recall", "fact", "knowledge", "graph"],
    ) {
        domains.insert("memory");
    }
    if contains_any(&lower, &["think", "reason", "plan", "analyze", "breakdown"]) {
        domains.insert("reasoning");
        domains.insert("context");
    }
    if contains_any(
        &lower,
        &["terminal", "shell", "command", "bash", "process", "port"],
    ) {
        domains.insert("shell");
    }
    if contains_any(&lower, &["mcp", "server", "gateway", "bridge"]) {
        domains.insert("mcp");
    }

    domains
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

fn tool_selection_score(
    name: &str,
    metadata: &ToolMetadata,
    selected_domains: &std::collections::BTreeSet<&'static str>,
) -> i32 {
    let mut score = metadata.priority as i32;
    if is_core_tool(name) {
        score += 1_000;
    }
    if selected_domains.contains(metadata.domain) {
        score += 500;
    }
    score -= match metadata.risk {
        ToolRisk::Low => 0,
        ToolRisk::Medium => 10,
        ToolRisk::High => 25,
    };
    if metadata.requires_approval {
        score -= 10;
    }
    score
}

fn tool_selection_reasons(
    name: &str,
    metadata: &ToolMetadata,
    selected_domains: &std::collections::BTreeSet<&'static str>,
) -> Vec<&'static str> {
    let mut reasons = Vec::new();
    if is_core_tool(name) {
        reasons.push("core_tool");
    }
    if selected_domains.contains(metadata.domain) {
        reasons.push("prompt_domain");
    }
    match metadata.risk {
        ToolRisk::Low => reasons.push("low_risk"),
        ToolRisk::Medium => reasons.push("medium_risk_penalty"),
        ToolRisk::High => reasons.push("high_risk_penalty"),
    }
    if metadata.requires_approval {
        reasons.push("requires_approval");
    }
    reasons
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
    route_cache: Arc<std::sync::Mutex<Option<(ToolRouteCacheKey, ToolRouteAnalysis)>>>,
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
            route_cache: Arc::new(std::sync::Mutex::new(None)),
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
            route_cache: Arc::new(std::sync::Mutex::new(None)),
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

    pub fn register(&self, tool: Arc<dyn Tool>) {
        self.write_tools().insert(tool.name().to_string(), tool);
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

    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        let filter = self.filter_scope.lock().ok().and_then(|g| g.clone());
        if let Some(ref prefixes) = filter {
            if name != "delegate_task"
                && name != "send_remote_input"
                && name != "optimize_tool_scope"
                && !prefixes.iter().any(|prefix| name.starts_with(prefix))
            {
                return None;
            }
        }

        // 1. If name is "delegate_task", override and inject parent tools dynamically
        if name == "delegate_task" {
            let (config, provider, session_manager) = self.context.as_ref()?;
            let mut parent_tools = Vec::new();
            for tool in self.read_tools().values() {
                if tool.name() != "delegate_task"
                    && tool.name() != "parallel_research"
                    && tool.name() != "send_remote_input"
                {
                    parent_tools.push(tool.clone());
                }
            }
            return Some(Arc::new(crate::tools::subagent::DelegateTaskTool {
                config: config.clone(),
                parent_provider: provider.clone(),
                session_manager: session_manager.clone(),
                parent_tools,
                cancellation_token: CancellationToken::new(),
            }));
        }

        // 1b. If name is "parallel_research", override and inject parent tools dynamically
        if name == "parallel_research" {
            let (config, provider, session_manager) = self.context.as_ref()?;
            let mut parent_tools = Vec::new();
            for tool in self.read_tools().values() {
                if tool.name() != "delegate_task"
                    && tool.name() != "parallel_research"
                    && tool.name() != "send_remote_input"
                {
                    parent_tools.push(tool.clone());
                }
            }
            return Some(Arc::new(crate::tools::subagent::ParallelResearchTool {
                config: config.clone(),
                parent_provider: provider.clone(),
                session_manager: session_manager.clone(),
                parent_tools,
                cancellation_token: CancellationToken::new(),
            }));
        }

        // 1c. If name is "evaluator_optimizer_loop", override and inject parent tools dynamically
        if name == "evaluator_optimizer_loop" {
            let (config, provider, session_manager) = self.context.as_ref()?;
            let mut parent_tools = Vec::new();
            for tool in self.read_tools().values() {
                if tool.name() != "delegate_task"
                    && tool.name() != "parallel_research"
                    && tool.name() != "evaluator_optimizer_loop"
                    && tool.name() != "send_remote_input"
                {
                    parent_tools.push(tool.clone());
                }
            }
            return Some(Arc::new(
                crate::tools::subagent::EvaluatorOptimizerLoopTool {
                    config: config.clone(),
                    parent_provider: provider.clone(),
                    session_manager: session_manager.clone(),
                    parent_tools,
                    cancellation_token: CancellationToken::new(),
                },
            ));
        }

        // 2. Check static tools
        if let Some(tool) = self.read_tools().get(name) {
            return Some(tool.clone());
        }

        // 3. If not found, check if it matches a custom subagent profile dynamically
        let (config, provider, session_manager) = self.context.as_ref()?;
        let active_subagent = crate::tools::subagent::ACTIVE_SUBAGENT
            .try_with(|s| s.clone())
            .unwrap_or_default();
        if !active_subagent.is_empty() && name == active_subagent {
            return None;
        }
        let profiles = crate::subagents::load_profiles().ok()?;
        let profile = profiles.into_iter().find(|p| p.name == name)?;

        let mut parent_tools = Vec::new();
        for tool in self.read_tools().values() {
            if tool.name() != "delegate_task"
                && tool.name() != "parallel_research"
                && tool.name() != "send_remote_input"
            {
                parent_tools.push(tool.clone());
            }
        }

        Some(Arc::new(crate::tools::subagent::DelegateProfileTool {
            config: config.clone(),
            parent_provider: provider.clone(),
            session_manager: session_manager.clone(),
            profile,
            parent_tools,
            cancellation_token: CancellationToken::new(),
        }))
    }

    pub fn get_static_tools(&self) -> Vec<Arc<dyn Tool>> {
        let filter = self.filter_scope.lock().ok().and_then(|g| g.clone());
        self.read_tools()
            .values()
            .filter(|t| {
                if let Some(ref prefixes) = filter {
                    let name = t.name();
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
        select_domains_for_prompt(prompt)
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

        let selected_domains_set = select_domains_for_prompt(prompt);
        let selected_domains: Vec<String> = selected_domains_set
            .iter()
            .map(|domain| (*domain).to_string())
            .collect();
        let static_names: HashSet<String> = static_tool_names.into_iter().collect();
        let reserved_subagents = self
            .dynamic_subagent_tools(filter.as_ref(), &static_names)
            .len()
            .min(128);
        let static_limit = 128usize.saturating_sub(reserved_subagents);

        let mut entries: Vec<ToolRouteEntry> = static_tools
            .values()
            .filter(|tool| tool_allowed_by_filter(tool.name(), filter.as_ref()))
            .map(|tool| {
                let metadata = tool.metadata();
                let selected_score =
                    tool_selection_score(tool.name(), &metadata, &selected_domains_set);
                let matched_prompt_domain = selected_domains_set.contains(metadata.domain);
                let selection_reason =
                    tool_selection_reasons(tool.name(), &metadata, &selected_domains_set)
                        .into_iter()
                        .map(str::to_string)
                        .collect();
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

        let selected_count = entries.len().min(static_limit);
        let dropped_count = entries.len().saturating_sub(selected_count);
        for (idx, entry) in entries.iter_mut().enumerate() {
            if idx < selected_count {
                entry.exposed_to_model = true;
            } else {
                entry.hidden_reason = Some("api_limit");
            }
        }

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

    pub fn to_openai_format_for_prompt(&self, prompt: &str) -> Vec<serde_json::Value> {
        let filter = self.filter_scope.lock().ok().and_then(|g| g.clone());
        let static_tools = self.read_tools();
        let static_names: HashSet<String> = static_tools.keys().cloned().collect();
        drop(static_tools);
        let mut subagent_tools = self.dynamic_subagent_tools(filter.as_ref(), &static_names);
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
            .filter(|entry| entry.exposed_to_model)
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
pub mod browser_common;
pub mod cargo_manager;
pub mod clipboard;
pub mod compiler_auto_heal;
pub mod crawl;
pub mod cron;
pub mod db_inspector;
pub mod doc_reader;
pub mod docs_mcp;
pub mod filesystem;
pub mod firefox;
pub mod git_manager;
pub mod github;
pub mod github_mcp;
pub mod graph_memory;
pub mod grep;
pub mod gsd_browser;
pub mod headroom;
pub mod html_video;
pub mod image_generator;
pub mod js_format;
pub mod mcp;
pub mod mcp_manager;
pub mod memory_extra;
pub mod mermaid;
pub mod network;
pub mod notes;
pub mod obscura;
pub mod onpkg;
pub mod open;
pub mod opendoc;
pub mod openmedia;
pub mod outline;
pub mod remote;
pub mod resource_policy;
pub mod rust_docs;
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
pub mod template_compiler;
pub mod video;
pub mod wasm_sandbox;
pub mod watcher;
pub mod web;
pub mod web_search;
