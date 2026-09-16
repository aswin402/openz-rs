# OpenZ Headless Mode CLI Implementation Plan 🦊⚡

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a headless, non-interactive CLI execution mode for OpenZ supporting dual invocation (`openz -p "..."` and `openz run "..."`), stdin piping, configurable output formatting (`text`, `json`, `stream-json`), strict security policy with `--yes` override, and isolated ephemeral sessions.

**Architecture:** A dedicated, decoupled execution module [`src/cli/headless.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/headless.rs) with 1:1 colocated sibling tests [`src/cli/headless_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/headless_tests.rs). Integrates with `AgentLoop` under `IS_SILENT` scope, cleanly isolated from terminal raw mode and crossterm TUI loops.

**Tech Stack:** Rust 2021, `clap` v4 (derive), `tokio` (async stdin & runtime), `serde` / `serde_json`, `uuid` v4, `chrono`.

## Global Constraints

- **Strict Resource Capping:** Never run workspace-wide builds/checks/tests.
  - Tests & checks: strictly capped at **1 parallel job** (`-j 1`).
  - Compiles: strictly capped at **2 parallel jobs** (`-j 2`).
- **Tool Invariant:** The exact **260 registered native tools invariant** must never drift.
- **Compiler Warnings:** Maintain 0 clippy warnings across the workspace.
- **SemVer Protocol:** Increment version by `+0.0.1` (`0.0.188` → `0.0.189`) synchronously across `Cargo.toml`, `onpkg.json`, `README.md`, and record in `CHANGELOG.md`.

---

### Task 1: CLI Arguments & Headless Data Models

**Files:**
- Modify: [`src/cli/args.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/args.rs)
- Create: [`src/cli/headless_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/headless_tests.rs)
- Modify: [`src/cli/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/mod.rs)

**Interfaces:**
- Produces:
  - `pub struct HeadlessArgs`
  - `pub struct HeadlessRunOutput`
  - `Command::Run(HeadlessArgs)` (aliased to `"exec"`)
  - Top-level `CliArgs` flags: `prompt`, `output_format`, `yes`

- [ ] **Step 1: Write the failing test for CLI args parsing**

Create `src/cli/headless_tests.rs`:
```rust
use crate::cli::args::{CliArgs, Command, HeadlessArgs};
use clap::Parser;

#[test]
fn test_headless_top_level_flag_parsing() {
    let args = CliArgs::try_parse_from(["openz", "-p", "inspect codebase", "--output-format", "json", "-y"])
        .expect("should parse top-level -p flag");
    assert_eq!(args.prompt.as_deref(), Some("inspect codebase"));
    assert_eq!(args.output_format.as_deref(), Some("json"));
    assert!(args.yes);
}

#[test]
fn test_headless_run_subcommand_parsing() {
    let args = CliArgs::try_parse_from([
        "openz",
        "run",
        "analyze diff",
        "--output-format",
        "text",
        "--allowed-tools",
        "read_file,grep_search",
        "--session",
        "test-session-123",
        "--model",
        "anthropic/claude-3-5-sonnet",
    ])
    .expect("should parse run subcommand");

    match args.command {
        Some(Command::Run(headless)) => {
            assert_eq!(headless.prompt.as_deref(), Some("analyze diff"));
            assert_eq!(headless.output_format, "text");
            assert_eq!(headless.allowed_tools.as_deref(), Some("read_file,grep_search"));
            assert_eq!(headless.session.as_deref(), Some("test-session-123"));
            assert_eq!(headless.model.as_deref(), Some("anthropic/claude-3-5-sonnet"));
        }
        other => panic!("expected Command::Run, got {:?}", other),
    }
}

#[test]
fn test_headless_exec_subcommand_alias() {
    let args = CliArgs::try_parse_from(["openz", "exec", "quick check"])
        .expect("should parse exec alias");
    assert!(matches!(args.command, Some(Command::Run(_))));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p openz --lib cli::headless_tests -j 1`  
Expected: FAIL compilation with unresolved `HeadlessArgs` or missing fields.

- [ ] **Step 3: Implement `HeadlessArgs` in `src/cli/args.rs` and wire in `src/cli/mod.rs`**

Update `src/cli/args.rs`:
```rust
#[derive(Parser, Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct HeadlessArgs {
    #[arg(index = 1)]
    pub prompt: Option<String>,

    #[arg(short = 'p', long = "prompt")]
    pub prompt_flag: Option<String>,

    #[arg(long, default_value = "text")]
    pub output_format: String,

    #[arg(short = 'y', long = "yes")]
    pub yes: bool,

    #[arg(long)]
    pub allowed_tools: Option<String>,

    #[arg(long)]
    pub session: Option<String>,

    #[arg(long)]
    pub r#continue: bool,

    #[arg(short = 'm', long = "model")]
    pub model: Option<String>,

    #[arg(long)]
    pub provider: Option<String>,

    #[arg(long)]
    pub max_iterations: Option<usize>,

    #[arg(long)]
    pub timeout: Option<u64>,
}
```

Update `CliArgs` and `Command` in `src/cli/args.rs`:
```rust
#[derive(Parser)]
#[command(name = "openz", version = env!("CARGO_PKG_VERSION"), about = "OpenZ - Rebranded Ultra-Lightweight Personal AI Agent")]
pub struct CliArgs {
    #[arg(short = 'p', long = "prompt", global = true)]
    pub prompt: Option<String>,

    #[arg(long, global = true)]
    pub output_format: Option<String>,

    #[arg(short = 'y', long = "yes", global = true)]
    pub yes: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

pub enum Command {
    // ...
    #[command(alias = "exec")]
    Run(HeadlessArgs),
}
```

Register test module in `src/cli/mod.rs`:
```rust
pub mod headless;
#[cfg(test)]
mod headless_tests;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p openz --lib cli::headless_tests -j 1`  
Expected: PASS (all 3 tests pass).

- [ ] **Step 5: Commit**

```bash
git add src/cli/args.rs src/cli/mod.rs src/cli/headless_tests.rs
git commit -m "feat(cli): add headless CLI arguments and command definitions"
```

---

### Task 2: Headless Output Models & Security Policy

**Files:**
- Create: [`src/cli/headless.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/headless.rs)
- Modify: [`src/cli/headless_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/headless_tests.rs)

**Interfaces:**
- Produces:
  - `pub struct HeadlessRunOutput`
  - `pub enum HeadlessFormat { Text, Json, StreamJson }`
  - `pub struct HeadlessSecurityPolicy`
  - `pub fn resolve_prompt(...)`
  - `pub fn resolve_session_key(...)`

- [ ] **Step 1: Write tests for output serialization and security policy**

Add to `src/cli/headless_tests.rs`:
```rust
use crate::cli::headless::{
    resolve_session_key, HeadlessRunOutput, HeadlessSecurityPolicy,
};

#[test]
fn test_headless_run_output_json_serialization() {
    let output = HeadlessRunOutput {
        status: "success".to_string(),
        content: "Operation completed successfully".to_string(),
        session_id: "cli:headless_test".to_string(),
        tools_used: vec!["read_file".to_string(), "grep_search".to_string()],
        tool_iterations: 2,
        duration_ms: 450,
        model: "anthropic/claude-3-5-sonnet".to_string(),
        provider: "anthropic".to_string(),
        error: None,
        exit_code: 0,
    };

    let json_str = serde_json::to_string_pretty(&output).expect("must serialize");
    let parsed: serde_json::Value = serde_json::from_str(&json_str).expect("must parse");
    assert_eq!(parsed["status"], "success");
    assert_eq!(parsed["exit_code"], 0);
    assert_eq!(parsed["tools_used"].as_array().unwrap().len(), 2);
}

#[test]
fn test_headless_security_policy_evaluation() {
    // 1. Strict mode without --yes or allowed tools
    let policy = HeadlessSecurityPolicy::new(false, None);
    assert!(policy.is_tool_permitted("read_file", false));
    assert!(!policy.is_tool_permitted("exec_command", true));
    assert!(!policy.is_tool_permitted("write_file", true));

    // 2. Auto-approved with --yes
    let policy_yes = HeadlessSecurityPolicy::new(true, None);
    assert!(policy_yes.is_tool_permitted("read_file", false));
    assert!(policy_yes.is_tool_permitted("exec_command", true));
    assert!(policy_yes.is_tool_permitted("write_file", true));

    // 3. Allowed tools list
    let policy_allowed = HeadlessSecurityPolicy::new(false, Some("read_file,exec_command"));
    assert!(policy_allowed.is_tool_permitted("exec_command", true));
    assert!(!policy_allowed.is_tool_permitted("write_file", true));
}

#[test]
fn test_headless_session_key_resolution() {
    let ephemeral = resolve_session_key(None, false, None);
    assert!(ephemeral.starts_with("cli:headless_"));

    let custom = resolve_session_key(Some("custom-thread-key"), false, None);
    assert_eq!(custom, "custom-thread-key");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p openz --lib cli::headless_tests -j 1`  
Expected: FAIL compilation with missing types in `headless`.

- [ ] **Step 3: Implement data models and security policy in `src/cli/headless.rs`**

Create `src/cli/headless.rs`:
```rust
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeadlessFormat {
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

#[derive(Debug, Clone)]
pub struct HeadlessSecurityPolicy {
    pub auto_approve_all: bool,
    pub allowed_tools: std::collections::HashSet<String>,
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
        }
    }

    pub fn is_tool_permitted(&self, tool_name: &str, is_sensitive: bool) -> bool {
        if !is_sensitive {
            return true;
        }
        if self.auto_approve_all {
            return true;
        }
        self.allowed_tools.contains(&tool_name.to_lowercase())
    }
}

pub fn resolve_session_key(
    explicit: Option<&str>,
    _r#continue: bool,
    _manager: Option<&crate::session::SessionManager>,
) -> String {
    if let Some(key) = explicit.map(str::trim).filter(|k| !k.is_empty()) {
        return key.to_string();
    }
    format!(
        "cli:headless_{}_{}",
        chrono::Utc::now().format("%Y%m%d_%H%M%S"),
        &uuid::Uuid::new_v4().to_string()[..8]
    )
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p openz --lib cli::headless_tests -j 1`  
Expected: PASS (all tests pass).

- [ ] **Step 5: Commit**

```bash
git add src/cli/headless.rs src/cli/headless_tests.rs
git commit -m "feat(headless): implement output contracts, security policy, and session resolution"
```

---

### Task 3: Headless Execution Engine (`handle_headless`)

**Files:**
- Modify: [`src/cli/headless.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/headless.rs)
- Modify: [`src/cli/headless_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/headless_tests.rs)

**Interfaces:**
- Produces:
  - `pub async fn handle_headless(args: HeadlessArgs) -> anyhow::Result<()>`
  - Non-interactive security hook integration
  - Exit code management (`std::process::exit` or result return)

- [ ] **Step 1: Write integration test with `MockProvider`**

Add to `src/cli/headless_tests.rs`:
```rust
use crate::cli::args::HeadlessArgs;
use crate::cli::headless::execute_headless_turn;
use crate::config::schema::Config;
use crate::providers::mock::MockProvider;
use crate::session::SessionManager;
use crate::tools::ToolRegistry;
use std::sync::Arc;

#[tokio::test]
async fn test_execute_headless_turn_with_mock_provider() {
    let mut config = Config::default();
    config.agents.defaults.model = "mock-model".to_string();
    config.agents.defaults.provider = "mock".to_string();

    let provider = Arc::new(MockProvider::new());
    provider.add_response("Headless execution was successful.");

    let registry = ToolRegistry::new();
    let temp_dir = std::env::temp_dir().join(format!("openz-headless-test-{}", uuid::Uuid::new_v4()));
    let session_manager = SessionManager::new(temp_dir);

    let agent_loop = crate::agent::AgentLoop::new(
        config.clone(),
        provider.clone(),
        registry,
        session_manager,
    );

    let args = HeadlessArgs {
        prompt: Some("test prompt".to_string()),
        output_format: "json".to_string(),
        yes: true,
        ..Default::default()
    };

    let output = execute_headless_turn(&agent_loop, &args, "test prompt", "cli:headless_test_run")
        .await
        .expect("turn should succeed");

    assert_eq!(output.status, "success");
    assert_eq!(output.exit_code, 0);
    assert!(output.content.contains("Headless execution was successful"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p openz --lib cli::headless_tests -j 1`  
Expected: FAIL with `execute_headless_turn` not defined.

- [ ] **Step 3: Implement `execute_headless_turn` and `handle_headless` in `src/cli/headless.rs`**

Add to `src/cli/headless.rs`:
- Asynchronous prompt resolution (reading `stdin` if `-` or empty in pipe).
- Building `AgentLoop` with overrides.
- Executing under `IS_SILENT.scope(true, ...)`.
- Formatting `stdout` according to `HeadlessFormat` (Text, JSON, StreamJson).
- Setting exit code (`0`, `1`, `2`).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p openz --lib cli::headless_tests -j 1`  
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/cli/headless.rs src/cli/headless_tests.rs
git commit -m "feat(headless): implement headless runner execution engine and output formatting"
```

---

### Task 4: CLI Dispatch Routing in `src/cli/mod.rs`

**Files:**
- Modify: [`src/cli/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/mod.rs)
- Modify: [`src/cli/args.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/args.rs)

**Interfaces:**
- Consumes:
  - `handle_headless(HeadlessArgs)`
- Produces:
  - CLI top-level and subcommand routing for `openz -p ...` and `openz run ...`

- [ ] **Step 1: Wire dispatch logic in `run_cli()`**

In `src/cli/mod.rs`:
```rust
// 1. Top-level -p flag check:
if let Some(prompt) = args.prompt {
    let headless_args = args::HeadlessArgs {
        prompt: Some(prompt),
        output_format: args.output_format.unwrap_or_else(|| "text".to_string()),
        yes: args.yes,
        ..Default::default()
    };
    return headless::handle_headless(headless_args).await;
}

// 2. Command match:
match args.command {
    Some(Command::Run(headless_args)) => {
        return headless::handle_headless(headless_args).await;
    }
    // ... existing commands ...
}
```

- [ ] **Step 2: Verify type-check with job 1**

Run: `cargo check -p openz -j 1`  
Expected: Clean compilation with 0 warnings.

- [ ] **Step 3: Test CLI dispatch unit tests**

Run: `cargo test -p openz --lib cli::headless_tests -j 1`  
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add src/cli/mod.rs src/cli/args.rs
git commit -m "feat(cli): wire headless execution dispatch in run_cli"
```

---

### Task 5: End-to-End Verification, SemVer Bump & Documentation

**Files:**
- Modify: [`Cargo.toml`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/Cargo.toml)
- Modify: [`onpkg.json`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/onpkg.json)
- Modify: [`README.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/README.md)
- Modify: [`CHANGELOG.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/CHANGELOG.md)

- [ ] **Step 1: Run focused unit tests with job 1**

Run: `cargo test -p openz --lib cli::headless_tests -j 1`  
Expected: All headless tests pass.

- [ ] **Step 2: Verify the 260 native tools invariant**

Run: `cargo test -p openz --lib test_native_tool_registration_names -j 1`  
Expected: PASS — exactly 260 tools, zero drift.

- [ ] **Step 3: Synchronous SemVer Bump to `v0.0.189`**

Synchronously update version to `0.0.189`:
- `Cargo.toml`: `version = "0.0.189"`
- `onpkg.json`: `"version": "0.0.189"`
- `README.md`: version badges and headers
- `CHANGELOG.md`: add release entry with Ideas, Inspirations, Sources & References, Details & Metrics, Verification.

- [ ] **Step 4: Run version sync unit test**

Run: `cargo test -p openz --lib version_sync_tests -j 1`  
Expected: PASS.

- [ ] **Step 5: Final Commit**

```bash
git add Cargo.toml onpkg.json README.md CHANGELOG.md
git commit -m "chore(release): bump openz to v0.0.189 with headless mode CLI"
```

---
