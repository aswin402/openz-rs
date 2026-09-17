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

- [x] **Step 1: Write the failing test for CLI args parsing**

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

- [x] **Step 2: Run test to verify it fails**

Run: `cargo test -p openz --lib cli::headless_tests -j 1`  
Expected: FAIL compilation with unresolved `HeadlessArgs` or missing fields.

- [x] **Step 3: Implement `HeadlessArgs` in `src/cli/args.rs` and wire in `src/cli/mod.rs`**

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

- [x] **Step 4: Run test to verify it passes**

Run: `cargo test -p openz --lib cli::headless_tests -j 1`  
Expected: PASS (all 3 tests pass).

- [x] **Step 5: Commit**

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

- [x] **Step 1: Write tests for output serialization and security policy**
- [x] **Step 2: Run test to verify it fails**
- [x] **Step 3: Implement data models and security policy in `src/cli/headless.rs`**
- [x] **Step 4: Run test to verify it passes**
- [x] **Step 5: Commit**

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

- [x] **Step 1: Write integration test with `MockProvider`**
- [x] **Step 2: Run test to verify it fails**
- [x] **Step 3: Implement `execute_headless_turn` and `handle_headless` in `src/cli/headless.rs`**
- [x] **Step 4: Run test to verify it passes**
- [x] **Step 5: Commit**

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

- [x] **Step 1: Wire dispatch logic in `run_cli()`**
- [x] **Step 2: Verify type-check with job 1**
- [x] **Step 3: Test CLI dispatch unit tests**
- [x] **Step 4: Commit**

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

- [x] **Step 1: Run focused unit tests with job 1**
- [x] **Step 2: Verify the 260 native tools invariant**
- [x] **Step 3: Synchronous SemVer Bump to `v0.0.189`**
- [x] **Step 4: Run version sync unit test**
- [x] **Step 5: Final Commit**

```bash
git add Cargo.toml Cargo.lock onpkg.json README.md CHANGELOG.md
git commit -m "chore(release): bump openz to v0.0.189 with headless mode CLI"
```

---
