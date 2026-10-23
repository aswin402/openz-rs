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
        match allowed_tools_csv {
            Some(csv) => {
                for tool in csv.split(',') {
                    let trimmed = tool.trim().to_lowercase();
                    match trimmed.is_empty() {
                        true => {}
                        false => {
                            allowed_tools.insert(trimmed);
                        }
                    }
                }
            }
            None => {}
        }
        Self {
            auto_approve_all,
            allowed_tools,
            denied_tool: std::sync::Arc::new(std::sync::Mutex::new(None)),
        }
    }

    pub fn record_denial(&self, tool: &str) {
        match self.denied_tool.lock() {
            Ok(mut guard) => {
                *guard = Some(tool.to_string());
            }
            Err(_) => {}
        }
    }

    pub fn last_denial(&self) -> Option<String> {
        self.denied_tool.lock().ok().and_then(|g| g.clone())
    }

    pub fn is_tool_permitted(&self, tool_name: &str, is_sensitive: bool) -> bool {
        match (is_sensitive, self.auto_approve_all) {
            (false, _) | (true, true) => true,
            (true, false) => self.allowed_tools.contains(&tool_name.trim().to_lowercase()),
        }
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
    match explicit.map(str::trim).filter(|k| !k.is_empty()) {
        Some(key) => return key.to_string(),
        None => {}
    }
    match (
        continue_session,
        manager.and_then(|mgr| mgr.list_summaries().into_iter().next()),
    ) {
        (true, Some(latest)) => latest.key,
        _ => format!(
            "cli:headless_{}_{}",
            chrono::Utc::now().format("%Y%m%d_%H%M%S"),
            crate::core::short_uuid()
        ),
    }
}

fn non_stdin_prompt(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|s| !s.is_empty() && *s != "-")
        .map(str::to_string)
}

pub async fn resolve_prompt(
    prompt_arg: Option<&str>,
    prompt_flag: Option<&str>,
) -> anyhow::Result<String> {
    match non_stdin_prompt(prompt_arg).or_else(|| non_stdin_prompt(prompt_flag)) {
        Some(prompt) => return Ok(prompt),
        None => {}
    }

    use tokio::io::AsyncReadExt;
    let mut buffer = String::new();
    tokio::io::stdin().read_to_string(&mut buffer).await?;
    let trimmed = buffer.trim().to_string();
    match trimmed.is_empty() {
        true => anyhow::bail!("No prompt provided via positional argument, flag, or stdin"),
        false => Ok(trimmed),
    }
}

pub fn format_output(output: &HeadlessRunOutput, format: HeadlessFormat) -> String {
    match format {
        HeadlessFormat::Text => match output.status.as_str() {
            "success" => output.content.clone(),
            _ => match (&output.error, output.content.trim().is_empty()) {
                (Some(err), false) => format!("{}\n\n{}", err, output.content),
                (Some(err), true) => err.clone(),
                (None, _) => output.content.clone(),
            },
        },
        HeadlessFormat::Json => {
            serde_json::to_string_pretty(output).unwrap_or_else(|_| "{}".to_string())
        }
        HeadlessFormat::StreamJson => {
            serde_json::to_string(output).unwrap_or_else(|_| "{}".to_string())
        }
    }
}

fn classify_headless_denial(
    policy: &HeadlessSecurityPolicy,
    default_status: &str,
    default_error: Option<String>,
    default_exit_code: i32,
) -> (String, Option<String>, i32) {
    match policy.last_denial() {
        Some(tool) => (
            "security_denied".to_string(),
            Some(format!(
                "Execution denied: Tool '{}' requires confirmation in headless mode. Run with -y/--yes or --allowed-tools to permit.",
                tool
            )),
            2,
        ),
        None => (default_status.to_string(), default_error, default_exit_code),
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
            let (status, error, exit_code) =
                classify_headless_denial(&policy, "success", None, 0);

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
            let default_exit_code = match err_msg.contains("denied")
                || err_msg.contains("security")
                || err_msg.contains("forbidden")
            {
                true => 2,
                false => 1,
            };
            let (status, error, exit_code) =
                classify_headless_denial(&policy, "error", Some(err_msg), default_exit_code);

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
    match args.model {
        Some(ref m) => config.agents.defaults.model = m.clone(),
        None => {}
    }
    match args.provider {
        Some(ref p) => config.agents.defaults.provider = p.clone(),
        None => {}
    }
    match args.max_iterations {
        Some(iters) => config.agents.defaults.max_tool_iterations = iters,
        None => {}
    }
    match args.timeout {
        Some(timeout) => config.agents.defaults.tool_timeout_secs = timeout,
        None => {}
    }

    let agent_loop = crate::cli::builder::build_agent_loop(config).await?;
    let session_key = resolve_session_key(
        args.session.as_deref(),
        args.r#continue,
        Some(&agent_loop.session_manager),
    );

    let output = execute_headless_turn(&agent_loop, &args, &prompt, &session_key).await?;
    let rendered = format_output(&output, format);

    match args.wait_curator {
        true => {
            crate::agent::agent_loop::save::wait_for_curator(std::time::Duration::from_secs(30))
                .await;
        }
        false => {}
    }

    match (output.status.as_str(), format) {
        ("success", _) => {
            println!("{}", rendered);
            std::process::exit(0);
        }
        (_, HeadlessFormat::Text) => {
            eprintln!("{}", rendered);
            std::process::exit(output.exit_code);
        }
        (_, _) => {
            println!("{}", rendered);
            std::process::exit(output.exit_code);
        }
    }
}
