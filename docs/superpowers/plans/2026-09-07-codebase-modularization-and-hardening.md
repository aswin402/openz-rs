# Codebase Modularization & Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Modularize the 1,636-line `logs.rs` god-file into focused submodules, unify secret detection and regex scrubbing in `src/core/secrets.rs`, harden cross-platform shell argument quoting and atomic model preference saving, and verify all invariants across the workspace.

**Architecture:** 
1. Provide `quote_shell_arg` in `src/core/process.rs` for Windows (`cmd.exe`) and Unix (`sh`) command lines, eliminate duplicated detached process construction in `src/tools/shell.rs`, and make `save_model_prefs_at` resilient to write/rename failures with `sync_all` and automatic `.tmp` cleanup.
2. Decompose `src/logs.rs` into `src/logs/` submodules (`storage.rs`, `subscriber.rs`, `query.rs`, `tui.rs`, and `mod.rs`), preserving 100% backward compatibility for all existing callers of `crate::logs::*` and `openz::logs::*`.
3. Centralize secret pattern matching (Telegram bot tokens, OpenAI/OpenRouter keys, partial tokens) and string redaction in `src/core/secrets.rs`, eliminating duplicated regex loops in `src/cli/doctor.rs` and `src/logs/`.
4. Document the native tool taxonomy and enforce tool module integrity.
5. Bump release to `v0.0.150` across all version surfaces (`Cargo.toml`, `README.md`, `onpkg.json`, `CHANGELOG.md`) and verify zero compiler/clippy warnings.

**Tech Stack:** Rust (edition 2021), Tokio, Tracing, Tracing-Subscriber, Rusqlite, Serde, Regex, Crossterm.

## Global Constraints

- Always cap Cargo compilation and testing with `-j 2` (e.g. `cargo test -p openz --lib <test_name> -j 2`, `cargo clippy -p openz --lib -j 2`).
- Never create standalone integration test targets in `tests/*.rs` (all tests must remain in-crate under `src/` via `--lib`).
- Maintain the exact native registered tools invariant in `src/cli/builder.rs` (`cargo test -p openz --lib test_native_tool_registration_names -j 2`).
- Zero compiler or clippy warnings across the workspace (`cargo clippy -p openz --lib -j 2`).
- All file paths and code symbols must use clickable `file://` markdown links.

---

### Task 1: Cross-Platform Shell Quoting, Process Deduplication & Model Preferences Hardening

**Files:**
- Modify: [`src/core/process.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/process.rs)
- Modify: [`src/tools/shell.rs:591-605`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/shell.rs#L591-L605)
- Modify: [`src/tools/filesystem.rs:543-557`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/filesystem.rs#L543-L557)
- Modify: [`src/providers/model_prefs.rs:41-50`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/model_prefs.rs#L41-L50)
- Test: [`src/core/process.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/process.rs)
- Test: [`src/providers/model_prefs.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/model_prefs.rs)

**Interfaces:**
- Consumes: [`crate::config::loader::set_command_cwd`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/config/loader.rs)
- Produces:
  - `pub fn quote_shell_arg(arg: &str) -> String` in [`src/core/process.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/process.rs)
  - Atomic persistence with `.tmp` purge on error in [`src/providers/model_prefs.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/model_prefs.rs)

- [x] **Step 1: Write failing unit tests in `src/core/process.rs` and `src/providers/model_prefs.rs`**

In [`src/core/process.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/process.rs):
```rust
    #[test]
    fn test_quote_shell_arg_handles_spaces_and_quotes() {
        let arg = "hello world's \"test\"";
        let quoted = quote_shell_arg(arg);
        if cfg!(target_os = "windows") {
            assert!(quoted.starts_with('"') && quoted.ends_with('"'));
            assert!(quoted.contains("\"\"test\"\""));
        } else {
            assert!(quoted.starts_with('\'') && quoted.ends_with('\''));
            assert!(quoted.contains("'\\''"));
        }
    }
```

In [`src/providers/model_prefs.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/model_prefs.rs):
```rust
    #[test]
    fn test_save_model_prefs_cleans_up_on_failure() {
        let guard = TempDirGuard::new("model-prefs-err-test");
        // Create an un-writable path to provoke an error or test invalid target
        let file_as_dir = guard.path().join("file_blocking_dir");
        std::fs::write(&file_as_dir, "blocking").unwrap();
        // Trying to save into a path where directory creation fails
        let invalid_dir = file_as_dir.join("sub");
        let res = save_model_prefs_at(&invalid_dir, &ModelPrefs::default());
        assert!(res.is_err());
        // Verify no leftover .tmp files were created in parent
        let entries = std::fs::read_dir(guard.path()).unwrap();
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            assert!(!name.contains(".tmp."), "leftover temp file found: {name}");
        }
    }
```

- [x] **Step 2: Run tests to verify failure**

Run: `cargo test -p openz --lib test_quote_shell_arg -j 2`
Expected: FAIL (unresolved `quote_shell_arg`)

- [x] **Step 3: Implement `quote_shell_arg`, update shell and filesystem tools, and harden `model_prefs` saving**

In [`src/core/process.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/process.rs):
```rust
/// Properly quote a shell argument for the host shell.
/// Uses double-quotes with internal `"` escaped as `""` on Windows `cmd.exe`.
/// Uses single-quotes with internal `'` escaped as `'\''` on Unix POSIX `sh`.
pub fn quote_shell_arg(arg: &str) -> String {
    if cfg!(target_os = "windows") {
        let mut s = String::with_capacity(arg.len() + 2);
        s.push('"');
        for c in arg.chars() {
            if c == '"' {
                s.push_str("\"\"");
            } else {
                s.push(c);
            }
        }
        s.push('"');
        s
    } else {
        let mut s = String::with_capacity(arg.len() + 2);
        s.push('\'');
        for c in arg.chars() {
            if c == '\'' {
                s.push_str("'\\''");
            } else {
                s.push(c);
            }
        }
        s.push('\'');
        s
    }
}
```

In [`src/tools/shell.rs:591-602`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/shell.rs#L591-L602):
Replace manual `cmd` and `sh` building with:
```rust
fn spawn_detached_command(command_line: &str, kind: DetachedCommandKind) -> Result<()> {
    let mut cmd = crate::core::process::host_shell_command(command_line);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
```

In [`src/tools/filesystem.rs:543-557`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/filesystem.rs#L543-L557):
Replace hardcoded single-quote escaping in `ZenflowEditTool` with:
```rust
        if in_git {
            let escaped_path = crate::core::process::quote_shell_arg(&path.to_string_lossy());
            let _ = run_cmd(format!("git add -- {}", escaped_path)).await;
```

In [`src/providers/model_prefs.rs:41-50`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/model_prefs.rs#L41-L50):
```rust
pub fn save_model_prefs_at(dir: &std::path::Path, prefs: &ModelPrefs) -> anyhow::Result<()> {
    let path = model_prefs_path_at(dir);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temp_file = path.with_extension(format!("tmp.{}", uuid::Uuid::new_v4()));
    let write_res = (|| -> anyhow::Result<()> {
        let mut file = std::fs::File::create(&temp_file)?;
        let json = serde_json::to_string_pretty(prefs)?;
        use std::io::Write;
        file.write_all(json.as_bytes())?;
        file.sync_all()?;
        Ok(())
    })();

    if let Err(e) = write_res {
        let _ = std::fs::remove_file(&temp_file);
        return Err(e);
    }

    if let Err(e) = std::fs::rename(&temp_file, &path) {
        let _ = std::fs::remove_file(&temp_file);
        return Err(e.into());
    }

    Ok(())
}
```

- [x] **Step 4: Run tests to verify they pass**

Run:
```bash
cargo test -p openz --lib core::process -j 2
cargo test -p openz --lib providers::model_prefs -j 2
cargo test -p openz --lib test_native_tool_registration_names -j 2
```
Expected: PASS (all tests pass)

- [x] **Step 5: Run clippy and commit**

Run: `cargo clippy -p openz --lib -j 2`
Expected: 0 warnings

```bash
git add src/core/process.rs src/tools/shell.rs src/tools/filesystem.rs src/providers/model_prefs.rs
git commit -m "fix(core): add quote_shell_arg, deduplicate host shell spawning, and harden model_prefs save"
```

---

### Task 2: Modularize `src/logs.rs` into `src/logs/` Domain Submodules

**Files:**
- Create: `src/logs/mod.rs`
- Create: `src/logs/storage.rs`
- Create: `src/logs/subscriber.rs`
- Create: `src/logs/query.rs`
- Create: `src/logs/tui.rs`
- Remove: `src/logs.rs`
- Test: `src/logs/mod.rs`

**Interfaces:**
- Consumes: `rusqlite`, `tracing`, `tracing_subscriber`, `crossterm`, `crate::config::loader::data_dir()`
- Produces:
  - `pub struct LogEntry`
  - `pub static LOG_TX`
  - `pub struct SqliteLogLayer`
  - `pub struct SecretScrubWriter<W>`
  - `pub fn default_db_path() -> PathBuf`
  - `pub fn default_log_path() -> PathBuf`
  - `pub async fn init_db_writer(...)`
  - `pub enum SessionFilter`
  - `pub enum LogLevelFilter`
  - `pub struct RunningSession`
  - `pub fn get_running_sessions() -> Result<Vec<RunningSession>>`
  - `pub fn get_latest_session_id() -> Option<String>`
  - `pub fn detect_active_session() -> Option<String>`
  - `pub fn print_row(...)`
  - `pub async fn run_logs_viewer(...)`
  - `pub fn print_session_recent_logs(...)`

- [x] **Step 1: Create `src/logs/storage.rs`**

Extract database migration, storage path resolution, and batch write loop from `src/logs.rs:93-154`:
- `LogEntry` definition
- `default_db_path()` and `default_log_path()`
- `init_db_writer(mut rx: tokio::sync::mpsc::UnboundedReceiver<LogEntry>)`
- Table initialization: `CREATE TABLE IF NOT EXISTS logs (...)` and indexes.

- [x] **Step 2: Create `src/logs/subscriber.rs`**

Extract tracing layer and scrub writer from `src/logs.rs:25-92, 155-221`:
- `LOG_TX: OnceLock<tokio::sync::mpsc::UnboundedSender<LogEntry>>`
- `SqliteLogLayer` implementing `tracing_subscriber::Layer`
- `EventFieldVisitor`
- `SecretScrubWriter<W>` implementing `std::io::Write`
- Redaction functions `initialize_secret_redaction` and `redact_sensitive_text`

- [x] **Step 3: Create `src/logs/query.rs`**

Extract session querying and filtering from `src/logs.rs:222-290, 1479-1636`:
- `SessionFilter` enum with parser methods
- `LogLevelFilter` enum with parsing and comparison
- `RunningSession` struct
- `get_running_sessions() -> Result<Vec<RunningSession>>`
- `get_latest_session_id() -> Option<String>`
- `detect_active_session() -> Option<String>`

- [x] **Step 4: Create `src/logs/tui.rs`**

Extract terminal formatting and ANSI rendering from `src/logs.rs:1140-1478`:
- `print_row(...)`
- `run_logs_viewer(...)`
- `print_session_recent_logs(...)`
- ANSI styling constants and crossterm layout helpers

- [x] **Step 5: Create `src/logs/mod.rs` with re-exports and remove `src/logs.rs`**

In `src/logs/mod.rs`:
```rust
pub mod query;
pub mod storage;
pub mod subscriber;
pub mod tui;

pub use query::{detect_active_session, get_latest_session_id, get_running_sessions, LogLevelFilter, RunningSession, SessionFilter};
pub use storage::{default_db_path, default_log_path, init_db_writer, LogEntry};
pub use subscriber::{initialize_secret_redaction, redact_sensitive_text, SecretScrubWriter, SqliteLogLayer, LOG_TX};
pub use tui::{print_row, print_session_recent_logs, run_logs_viewer};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_level_filter_parsing() {
        assert_eq!(LogLevelFilter::from_opt(Some("info")), LogLevelFilter::Info);
        assert_eq!(LogLevelFilter::from_opt(Some("warn")), LogLevelFilter::Warn);
        assert_eq!(LogLevelFilter::from_opt(Some("error")), LogLevelFilter::Error);
        assert_eq!(LogLevelFilter::from_opt(None), LogLevelFilter::All);
    }

    #[test]
    fn test_session_filter_parsing() {
        assert_eq!(SessionFilter::from_opt(Some("all")), SessionFilter::All);
        assert_eq!(SessionFilter::from_opt(None), SessionFilter::All);
        assert_eq!(SessionFilter::from_opt(Some("session-123")), SessionFilter::Only("session-123".to_string()));
    }
}
```

Remove `src/logs.rs`.

- [x] **Step 6: Run tests to verify modularized logs**

Run:
```bash
cargo test -p openz --lib logs -j 2
cargo test -p openz --lib cli::logs -j 2
cargo test -p openz --lib test_native_tool_registration_names -j 2
```
Expected: PASS

- [x] **Step 7: Run clippy and commit**

Run: `cargo clippy -p openz --lib -j 2`
Expected: 0 warnings

```bash
git add src/logs/
git rm src/logs.rs
git commit -m "refactor(logs): decompose 1600-line logs.rs into dedicated src/logs/ submodules"
```

---

### Task 3: Unify Secret Redaction & Regex Pattern Scrubbing in `src/core/secrets.rs`

**Files:**
- Modify: [`src/core/secrets.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/secrets.rs)
- Modify: [`src/cli/doctor.rs:92-136`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/doctor.rs#L92-L136)
- Modify: [`src/logs/subscriber.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/logs/subscriber.rs)
- Test: [`src/core/secrets.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/secrets.rs)
- Test: [`src/cli/doctor.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/doctor.rs)

**Interfaces:**
- Consumes: `regex::Regex`, `serde_json::Value`
- Produces:
  - `pub fn secret_patterns() -> &'static [regex::Regex]`
  - `pub fn scrub_secret_text(text: &str) -> (String, usize)`
  - `pub fn redact_text_with_secrets(text: &str, secrets: &[String]) -> String`
  - `pub fn collect_all_environment_and_config_secrets(config: &serde_json::Value) -> Vec<String>`

- [x] **Step 1: Write failing unit test in `src/core/secrets.rs`**

```rust
    #[test]
    fn test_scrub_secret_text_patterns() {
        let input = "sk-12345678901234567890 and bot 12345678:abcdefghijklmnopqrst";
        let (scrubbed, count) = scrub_secret_text(input);
        assert_eq!(count, 2);
        assert!(!scrubbed.contains("sk-12345678901234567890"));
        assert!(!scrubbed.contains("12345678:abcdefghijklmnopqrst"));
        assert!(scrubbed.contains("[REDACTED_SECRET]"));
    }

    #[test]
    fn test_redact_text_with_secrets_replaces_known_keys() {
        let secrets = vec!["supersecretpass123".to_string()];
        let redacted = redact_text_with_secrets("My password is supersecretpass123!", &secrets);
        assert_eq!(redacted, "My password is [REDACTED_SECRET]!");
    }
```

- [x] **Step 2: Run test to verify failure**

Run: `cargo test -p openz --lib test_scrub_secret_text_patterns -j 2`
Expected: FAIL (`scrub_secret_text` not found)

- [x] **Step 3: Implement centralized scrubbing in `src/core/secrets.rs`**

In [`src/core/secrets.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/secrets.rs):
```rust
use std::sync::LazyLock;

static SECRET_PATTERNS: LazyLock<Vec<regex::Regex>> = LazyLock::new(|| {
    vec![
        regex::Regex::new(r"\d{8,}:[A-Za-z0-9_-]{16,}\b").expect("valid telegram token regex"),
        regex::Regex::new(r"\bsk-[A-Za-z0-9_-]{16,}\b").expect("valid sk token regex"),
        regex::Regex::new(r"\d{8,}:[A-Za-z0-9_-]{3,}\.\.\.")
            .expect("valid partial telegram token regex"),
        regex::Regex::new(r"\bsk-[A-Za-z0-9_-]{4,}\.\.\.").expect("valid partial sk token regex"),
    ]
});

/// Return the compiled regular expressions matching well-known API keys and credentials.
pub fn secret_patterns() -> &'static [regex::Regex] {
    &SECRET_PATTERNS
}

/// Scrub text by replacing matches of known secret token regex patterns with `[REDACTED_SECRET]`.
/// Returns the sanitized string and the total number of replacements performed.
pub fn scrub_secret_text(text: &str) -> (String, usize) {
    let mut scrubbed = text.to_string();
    let mut replacements = 0usize;
    for pattern in secret_patterns() {
        let count = pattern.find_iter(&scrubbed).count();
        if count > 0 {
            scrubbed = pattern
                .replace_all(&scrubbed, "[REDACTED_SECRET]")
                .into_owned();
            replacements = replacements.saturating_add(count);
        }
    }
    (scrubbed, replacements)
}

/// Redact occurrences of specific known secret values from a text string.
pub fn redact_text_with_secrets(text: &str, secrets: &[String]) -> String {
    secrets.iter().fold(text.to_string(), |result, secret| {
        result.replace(secret, "[REDACTED_SECRET]")
    })
}

/// Collect credentials from both config and environment variables.
pub fn collect_all_environment_and_config_secrets(config: &serde_json::Value) -> Vec<String> {
    let mut secrets = Vec::new();
    collect_secret_values(config, &mut secrets);
    for (key, value) in std::env::vars() {
        if is_secret_key(&key) && value.trim().len() >= 8 {
            secrets.push(value.trim().to_string());
        }
    }
    secrets.sort_by_key(|v| std::cmp::Reverse(v.len()));
    secrets.dedup();
    secrets
}
```

- [x] **Step 4: Update `src/cli/doctor.rs` and `src/logs/subscriber.rs` to consume `src/core/secrets.rs`**

In [`src/cli/doctor.rs:92-118`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/doctor.rs#L92-L118):
Replace local `secret_patterns()` and `scrub_secret_text()` with calls to `crate::core::secrets::scrub_secret_text`:
```rust
fn scrub_secret_text(text: &str) -> SecretScrubResult {
    let (text, replacements) = crate::core::secrets::scrub_secret_text(text);
    SecretScrubResult {
        text,
        replacements,
    }
}
```

In [`src/logs/subscriber.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/logs/subscriber.rs):
Update `initialize_secret_redaction` and `redact_text_with_secrets` to delegate to `crate::core::secrets`.

- [x] **Step 5: Run tests and clippy**

Run:
```bash
cargo test -p openz --lib core::secrets -j 2
cargo test -p openz --lib cli::doctor -j 2
cargo test -p openz --lib test_native_tool_registration_names -j 2
cargo clippy -p openz --lib -j 2
```
Expected: PASS and 0 warnings

- [x] **Step 6: Commit**

```bash
git add src/core/secrets.rs src/cli/doctor.rs src/logs/
git commit -m "refactor(secrets): centralize secret regex patterns and scrubbing into core::secrets"
```

---

### Task 4: Clarify and Document Native Tool Namespace & Architecture

**Files:**
- Create: [`src/tools/README.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/README.md)
- Modify: [`src/tools/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/mod.rs)
- Test: [`src/cli/builder.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/builder.rs)

**Interfaces:**
- Consumes: Tool domain organization across registration layers
- Produces: Architectural clarity documentation, invariant documentation

- [x] **Step 1: Create `src/tools/README.md` documenting the tool subsystem architecture**

Document:
1. Core Tool Subsystem Engine: `arguments.rs`, `metadata.rs`, `defs.rs`, `registry.rs`, `routing.rs`, `resource_policy.rs`, `scope_engine.rs`.
2. Grouped Subsystem Folders:
   - `browser/`: Headless browser control, CDP, Firefox, Obscura
   - `graph_memory/`: Entity/relation/observation graph memory & database branching
   - `headroom/`: Context token compaction, cache alignment, CCR retrieval
   - `memory_extra/`: Working memory, episodic recall, smart storage
   - `opendoc/`: Document parsing & MCP server
   - `openmedia/`: Media manipulation, animations, images, video
   - `searchxyz/`: Multi-engine web search integration
   - `self_management/`: Autonomous diagnostics, session management, backups
   - `sequential_thinking/`: Dynamic step-by-step reasoning tools
   - `shared_memory/`: Team-shared persistent knowledge memory
   - `subagent/`: Delegation, subagent profiles, cancellation tokens
3. Loose Native Tools: Filesystem, shell, code analysis, network, task management.
4. Registration modules: `src/cli/tool_registration/{core, integrations, media, memory}.rs`.

- [x] **Step 2: Add architectural guard test in `src/cli/builder.rs`**

Verify that all native tool registrations maintain exact category invariants and no orphan tools exist.

- [x] **Step 3: Run test and clippy**

Run:
```bash
cargo test -p openz --lib test_native_tool_registration_names -j 2
cargo clippy -p openz --lib -j 2
```
Expected: PASS, 0 warnings

- [x] **Step 4: Commit**

```bash
git add src/tools/README.md src/tools/mod.rs src/cli/builder.rs
git commit -m "docs(tools): document tool subsystem domain taxonomy and architectural guidelines"
```

---

### Task 5: Invariant Verification, Multi-Surface Version Sync & Release Bump (`v0.0.150`)

**Files:**
- Modify: [`Cargo.toml`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/Cargo.toml)
- Modify: [`README.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/README.md)
- Modify: [`onpkg.json`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/onpkg.json)
- Modify: [`CHANGELOG.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/CHANGELOG.md)
- Modify: [`recommendedfix.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/recommendedfix.md)
- Test: [`src/lib.rs:version_sync_tests`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/lib.rs)

- [x] **Step 1: Bump version in `Cargo.toml`, `onpkg.json`, and `README.md`**

In [`Cargo.toml`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/Cargo.toml):
```toml
version = "0.0.150"
```

In [`onpkg.json`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/onpkg.json):
```json
  "project": {
    "name": "openz",
    "version": "0.0.150",
```

In [`README.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/README.md):
Update version badge to `0.0.150`.

- [x] **Step 2: Add release notes to `CHANGELOG.md`**

Add top-level release block:
```markdown
### v0.0.150 (Latest Release)
- **Modularized Logging Architecture**: Decomposed the 1,636-line `src/logs.rs` god-file into `src/logs/` submodules (`storage.rs`, `subscriber.rs`, `query.rs`, `tui.rs`, `mod.rs`) while strictly maintaining 100% backward compatibility for all callers.
- **Centralized Secret Scrubbing**: Consolidated token regex pattern matching (Telegram, OpenAI `sk-...`, partial tokens) and string redaction into [`src/core/secrets.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/secrets.rs), eliminating duplication in `doctor` and logging.
- **Cross-Platform Shell Quoting**: Introduced `quote_shell_arg` in [`src/core/process.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/process.rs) with proper Windows double-quote escaping and Unix single-quote escaping, resolving Windows git-add quoting failures.
- **Hardened Model Preferences Persistence**: Enhanced `save_model_prefs_at` with file `sync_all` and automatic `.tmp` file cleanup upon any write/rename error.
- **Tool Taxonomy Documentation**: Formalized native tool subsystem taxonomy in `src/tools/README.md`.
```

- [x] **Step 3: Update `recommendedfix.md`**

Document all completed architectural hardening items.

- [x] **Step 4: Verify version synchronization and registration invariant**

Run:
```bash
cargo test -p openz --lib version_sync_tests -j 2
cargo test -p openz --lib test_native_tool_registration_names -j 2
cargo clippy -p openz --lib -j 2
```
Expected: PASS across all checks with 0 warnings.

- [x] **Step 5: Commit release bump**

```bash
git add Cargo.toml README.md onpkg.json CHANGELOG.md recommendedfix.md docs/superpowers/plans/2026-09-07-codebase-modularization-and-hardening.md
git commit -m "chore(release): bump openz to v0.0.150 with modular logs and unified secrets"
```
