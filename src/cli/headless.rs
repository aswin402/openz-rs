use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeadlessRunOutput {
    pub status: String,
    pub content: String,
    pub session_id: String,
    pub tools_used: Vec<String>,
    pub tool_iterations: usize,
    pub duration_ms: u64,
    pub model: String,
    pub provider: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub exit_code: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HeadlessFormat {
    #[default]
    Text,
    Json,
    StreamJson,
}

impl HeadlessFormat {
    pub fn parse(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "json" => Self::Json,
            "stream-json" | "stream_json" | "ndjson" => Self::StreamJson,
            _ => Self::Text,
        }
    }
}

impl std::str::FromStr for HeadlessFormat {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::parse(s))
    }
}

impl std::fmt::Display for HeadlessFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Text => write!(f, "text"),
            Self::Json => write!(f, "json"),
            Self::StreamJson => write!(f, "stream-json"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct HeadlessSecurityPolicy {
    pub auto_approve_all: bool,
    pub allowed_tools: std::collections::HashSet<String>,
    pub denied_tool: std::sync::Arc<std::sync::Mutex<Option<String>>>,
}

impl HeadlessSecurityPolicy {
    pub fn new(auto_approve_all: bool, allowed_tools_csv: Option<&str>) -> Self {
        let mut allowed_tools = std::collections::HashSet::new();
        if let Some(csv) = allowed_tools_csv {
            for tool in csv.split(',') {
                let trimmed = tool.trim().to_lowercase();
                if !trimmed.is_empty() {
                    allowed_tools.insert(trimmed);
                }
            }
        }
        Self {
            auto_approve_all,
            allowed_tools,
            denied_tool: std::sync::Arc::new(std::sync::Mutex::new(None)),
        }
    }

    pub fn record_denial(&self, tool: &str) {
        if let Ok(mut guard) = self.denied_tool.lock() {
            *guard = Some(tool.to_string());
        }
    }

    pub fn last_denial(&self) -> Option<String> {
        self.denied_tool.lock().ok().and_then(|g| g.clone())
    }

    pub fn is_tool_permitted(&self, tool_name: &str, is_sensitive: bool) -> bool {
        if !is_sensitive {
            return true;
        }
        if self.auto_approve_all {
            return true;
        }
        self.allowed_tools
            .contains(&tool_name.trim().to_lowercase())
    }
}

tokio::task_local! {
    pub static CURRENT_HEADLESS_POLICY: HeadlessSecurityPolicy;
}

pub fn current_headless_policy() -> Option<HeadlessSecurityPolicy> {
    CURRENT_HEADLESS_POLICY.try_with(|p| p.clone()).ok()
}

pub fn resolve_session_key(
    explicit: Option<&str>,
    continue_session: bool,
    manager: Option<&crate::session::SessionManager>,
) -> String {
    if let Some(key) = explicit.map(str::trim).filter(|k| !k.is_empty()) {
        return key.to_string();
    }
    if continue_session {
        if let Some(mgr) = manager {
            if let Some(latest) = mgr.list_summaries().first() {
                return latest.key.clone();
            }
        }
    }
    format!(
        "cli:headless_{}_{}",
        chrono::Utc::now().format("%Y%m%d_%H%M%S"),
        &uuid::Uuid::new_v4().to_string()[..8]
    )
}

pub async fn resolve_prompt(
    prompt_arg: Option<&str>,
    prompt_flag: Option<&str>,
) -> anyhow::Result<String> {
    if let Some(arg) = prompt_arg.map(str::trim).filter(|s| !s.is_empty()) {
        if arg != "-" {
            return Ok(arg.to_string());
        }
    } else if let Some(flag) = prompt_flag.map(str::trim).filter(|s| !s.is_empty()) {
        if flag != "-" {
            return Ok(flag.to_string());
        }
    }

    use tokio::io::AsyncReadExt;
    let mut buffer = String::new();
    tokio::io::stdin().read_to_string(&mut buffer).await?;
    let trimmed = buffer.trim().to_string();
    if trimmed.is_empty() {
        anyhow::bail!("No prompt provided via positional argument, flag, or stdin");
    }
    Ok(trimmed)
}

pub fn format_output(output: &HeadlessRunOutput, format: HeadlessFormat) -> String {
    match format {
        HeadlessFormat::Text => {
            if output.status != "success" {
                match (&output.error, output.content.trim().is_empty()) {
                    (Some(err), false) => format!("{}\n\n{}", err, output.content),
                    (Some(err), true) => err.clone(),
                    (None, _) => output.content.clone(),
                }
            } else {
                output.content.clone()
            }
        }
        HeadlessFormat::Json => {
            serde_json::to_string_pretty(output).unwrap_or_else(|_| "{}".to_string())
        }
        HeadlessFormat::StreamJson => {
            // Emits completed HeadlessRunOutput as a single compact NDJSON line for MVP.
            // Full incremental token event streaming is planned for a future milestone.
            serde_json::to_string(output).unwrap_or_else(|_| "{}".to_string())
        }
    }
}

pub async fn execute_headless_turn(
    agent_loop: &crate::agent::AgentLoop,
    args: &crate::cli::args::HeadlessArgs,
    prompt: &str,
    session_key: &str,
) -> anyhow::Result<HeadlessRunOutput> {
    let start = std::time::Instant::now();
    let policy = HeadlessSecurityPolicy::new(args.yes, args.allowed_tools.as_deref());
    let timeout_secs = args
        .timeout
        .unwrap_or(agent_loop.config.agents.defaults.tool_timeout_secs.max(300));

    let run_fut = async {
        crate::agent::style::spinner::IS_SILENT
            .scope(true, async {
                CURRENT_HEADLESS_POLICY
                    .scope(policy.clone(), async {
                        agent_loop.run(prompt, session_key).await
                    })
                    .await
            })
            .await
    };

    let run_res = match tokio::time::timeout(
        std::time::Duration::from_secs(timeout_secs),
        run_fut,
    )
    .await
    {
        Ok(res) => res,
        Err(_) => {
            let duration_ms = start.elapsed().as_millis() as u64;
            return Ok(HeadlessRunOutput {
                status: "error".to_string(),
                content: String::new(),
                session_id: session_key.to_string(),
                tools_used: Vec::new(),
                tool_iterations: 0,
                duration_ms,
                model: agent_loop.config.agents.defaults.model.clone(),
                provider: agent_loop.config.agents.defaults.provider.clone(),
                error: Some(format!("Execution timed out after {}s", timeout_secs)),
                exit_code: 1,
            });
        }
    };

    let duration_ms = start.elapsed().as_millis() as u64;
    match run_res {
        Ok(run_result) => {
            let tools_used = run_result.tools_used;
            let tool_iterations = tools_used.len();
            let mut status = "success".to_string();
            let mut error = None;
            let mut exit_code = 0;

            if let Some(tool) = policy.last_denial() {
                status = "security_denied".to_string();
                error = Some(format!(
                    "Execution denied: Tool '{}' requires confirmation in headless mode. Run with -y/--yes or --allowed-tools to permit.",
                    tool
                ));
                exit_code = 2;
            }

            Ok(HeadlessRunOutput {
                status,
                content: run_result.content,
                session_id: session_key.to_string(),
                tools_used,
                tool_iterations,
                duration_ms,
                model: agent_loop.config.agents.defaults.model.clone(),
                provider: agent_loop.config.agents.defaults.provider.clone(),
                error,
                exit_code,
            })
        }
        Err(err) => {
            let err_msg = err.to_string();
            let mut exit_code = if err_msg.contains("denied")
                || err_msg.contains("security")
                || err_msg.contains("forbidden")
            {
                2
            } else {
                1
            };
            let mut status = "error".to_string();
            let mut error = Some(err_msg);

            if let Some(tool) = policy.last_denial() {
                status = "security_denied".to_string();
                error = Some(format!(
                    "Execution denied: Tool '{}' requires confirmation in headless mode. Run with -y/--yes or --allowed-tools to permit.",
                    tool
                ));
                exit_code = 2;
            }

            Ok(HeadlessRunOutput {
                status,
                content: String::new(),
                session_id: session_key.to_string(),
                tools_used: Vec::new(),
                tool_iterations: 0,
                duration_ms,
                model: agent_loop.config.agents.defaults.model.clone(),
                provider: agent_loop.config.agents.defaults.provider.clone(),
                error,
                exit_code,
            })
        }
    }
}

pub async fn handle_headless(args: crate::cli::args::HeadlessArgs) -> anyhow::Result<()> {
    crate::cli::set_silent_mode(true);
    std::env::set_var("OPENZ_SILENT", "1");

    let prompt = resolve_prompt(args.prompt.as_deref(), args.prompt_flag.as_deref()).await?;
    let format = HeadlessFormat::parse(&args.output_format);

    let mut config = crate::config::loader::load_config().unwrap_or_default();
    if let Some(ref m) = args.model {
        config.agents.defaults.model = m.clone();
    }
    if let Some(ref p) = args.provider {
        config.agents.defaults.provider = p.clone();
    }
    if let Some(iters) = args.max_iterations {
        config.agents.defaults.max_tool_iterations = iters;
    }
    if let Some(timeout) = args.timeout {
        config.agents.defaults.tool_timeout_secs = timeout;
    }

    let agent_loop = crate::cli::builder::build_agent_loop(config).await?;
    let session_key = resolve_session_key(
        args.session.as_deref(),
        args.r#continue,
        Some(&agent_loop.session_manager),
    );

    let output = execute_headless_turn(&agent_loop, &args, &prompt, &session_key).await?;
    let rendered = format_output(&output, format);

    if output.status == "success" {
        println!("{}", rendered);
        std::process::exit(0);
    } else {
        if format == HeadlessFormat::Text {
            eprintln!("{}", rendered);
        } else {
            println!("{}", rendered);
        }
        std::process::exit(output.exit_code);
    }
}
