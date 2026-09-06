# Codebase Hardening and Quality Improvements Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Resolve the four critical issues and hardening opportunities identified in the codebase audit: (1) dual-dispatch WebSocket tool progress with `activity_notice` for full WebUI compatibility, (2) migrate standalone integration tests in `tests/agent_loop.rs` into an in-crate test module to respect build-cache and linking rules, (3) parameterize the browser Chrome DevTools Protocol (CDP) port and harden process killing, and (4) add `--clean-target` to `openz doctor` to prevent future disk exhaustion and laptop freezes.

**Architecture:** 
- **Tool Progress Streaming:** Enhance `send_progress_update()` in `src/agent/agent_loop/tool_execution.rs` to publish both `tool_progress` and `activity_notice` events.
- **In-Crate Integration Testing:** Relocate `tests/agent_loop.rs` to `src/agent/agent_loop/integration_tests.rs` behind `#[cfg(test)]`, eliminating the duplicate external binary target while preserving 100% test coverage.
- **Configurable CDP Port:** Extend `BrowserConfig` in `src/config/schema.rs` with `cdp_port: u16` (default 9222) and update browser tools to use dynamic port resolution instead of hardcoded values and blind `fuser -k` killing.
- **Doctor Target Purge:** Add `--clean-target` to `openz doctor` in `src/cli/args.rs` and `src/cli/doctor.rs` to inspect and purge overgrown `target/` directories.

**Tech Stack:** Rust 2021 edition, Tokio async runtime, Axum/Tungstenite WebSocket protocols, Clap CLI, Serde JSON.

## Global Constraints
- Always cap Cargo compilation and testing with `-j 2` (`cargo test -p openz --lib <test_name> -j 2`).
- Never create standalone integration test binaries in `tests/*.rs`. All tests must live in-crate under `src/` (`--lib`).
- Maintain the exact 128 registered native tools invariant in `src/cli/builder.rs` (verified by `test_native_tool_registration_names`).
- Zero compiler or clippy warnings across `openz` and all workspace crates (`cargo clippy --workspace --lib -j 2`).
- All file and symbol references must use clickable `file://` markdown links.

---

### Task 1: Dual-Dispatch Progress Updates & WebUI Correlation

**Files:**
- Modify: `src/agent/agent_loop/tool_execution.rs:254-269`
- Modify: `src/channels/websocket/tests.rs`

**Interfaces:**
- Consumes: `crate::channels::websocket::publish_activity_notice(session_key, kind, title, detail)`
- Produces: Dual WebSocket event dispatch on progress updates for backwards & forwards WebUI compatibility.

- [ ] **Step 1: Write unit test verifying dual-dispatch progress update**

In `src/channels/websocket/tests.rs`, add a test verifying that progress events emit valid JSON payloads for both `tool_progress` and `activity_notice`.

```rust
#[test]
fn test_tool_progress_and_activity_notice_payload_validity() {
    let chat_id = "test-chat-123";
    let turn_id = "turn-456";
    let tool_call_id = "call-789";
    let tool_name = "test_tool";
    let progress_text = "Processing chunk 1/3...";

    let tp = crate::channels::websocket::protocol::tool_progress(
        chat_id,
        turn_id,
        tool_call_id,
        tool_name,
        progress_text,
    );
    let an = crate::channels::websocket::protocol::activity_notice(
        chat_id,
        "progress",
        "Tool Progress",
        progress_text,
        1700000000,
    );

    assert_eq!(tp["type"], "tool_progress");
    assert_eq!(tp["payload"]["chat_id"], chat_id);
    assert_eq!(tp["payload"]["content"], progress_text);

    assert_eq!(an["type"], "activity_notice");
    assert_eq!(an["payload"]["kind"], "progress");
    assert_eq!(an["payload"]["detail"], progress_text);
}
```

- [ ] **Step 2: Run test to verify it passes**

Run: `cargo test -p openz --lib test_tool_progress_and_activity_notice_payload_validity -j 2`
Expected: PASS

- [ ] **Step 3: Update `send_progress_update()` to dispatch both events**

In `src/agent/agent_loop/tool_execution.rs:254-269`, update `send_progress_update`:

```rust
pub(crate) async fn send_progress_update(session_key: &str, text: &str) {
    let actual_session = crate::agent::style::spinner::get_current_session_key()
        .unwrap_or_else(|| session_key.to_string());
    if let Some(chat_id) = crate::channels::websocket::ws_chat_id(&actual_session)
        .or_else(|| crate::channels::websocket::ws_chat_id(session_key))
    {
        // 1. Emit typed tool_progress event for modern WebUI clients
        crate::channels::websocket::publish_ws_event(
            crate::channels::websocket::protocol::tool_progress(
                chat_id,
                crate::agent::agent_loop::current_turn_id(),
                "",
                "",
                text,
            ),
        );
        // 2. Dual-dispatch activity_notice so all WebUI clients immediately display progress
        crate::channels::websocket::publish_activity_notice(
            &actual_session,
            "progress",
            "Progress",
            text,
        );
    }
    // ... external channel notification blocks (Telegram, Discord, WhatsApp)
```

- [ ] **Step 4: Verify test suite and clippy**

Run: `cargo test -p openz --lib websocket_protocol -j 2`
Run: `cargo clippy -p openz --lib -j 2`
Expected: 0 warnings, 0 errors.

- [ ] **Step 5: Commit**

```bash
git add src/agent/agent_loop/tool_execution.rs src/channels/websocket/tests.rs
git commit -m "fix(websocket): dual-dispatch activity notice with tool progress for WebUI compatibility"
```

---

### Task 2: Relocate Standalone Integration Tests to In-Crate Module

**Files:**
- Create: `src/agent/agent_loop/integration_tests.rs`
- Modify: `src/agent/agent_loop/mod.rs`
- Delete: `tests/agent_loop.rs`
- Modify: `recommendedfix.md`

**Interfaces:**
- Consumes: `AgentLoop`, `TestMockProvider`, mock tools from inside crate.
- Produces: In-crate test module under `src/` eliminating standalone binary target.

- [ ] **Step 1: Create in-crate module `src/agent/agent_loop/integration_tests.rs`**

Move the contents of `tests/agent_loop.rs` into `src/agent/agent_loop/integration_tests.rs`. Adjust paths from `openz::` to `crate::`:

```rust
use crate::agent::AgentLoop;
use crate::config::schema::Config;
use crate::providers::{GenerationSettings, LLMProvider, LLMResponse, ToolCallRequest};
use crate::session::Message;
use crate::session::SessionManager;
use crate::tools::{Tool, ToolRegistry};
use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

// ... Include TestMockProvider, CalculatorTool, FlakyHttpTool, and the 3 test functions:
// - test_agent_loop_tool_execution_pipeline
// - test_agent_loop_session_overrides_pipeline
// - test_tool_retry_on_transient_error
```

- [ ] **Step 2: Register module in `src/agent/agent_loop/mod.rs`**

In `src/agent/agent_loop/mod.rs`, add under `#[cfg(test)]`:
```rust
#[cfg(test)]
mod integration_tests;
```

- [ ] **Step 3: Remove standalone test file `tests/agent_loop.rs`**

Delete `tests/agent_loop.rs`.

- [ ] **Step 4: Run relocated in-crate tests**

Run: `cargo test -p openz --lib agent_loop::integration_tests -j 2`
Expected: All 3 integration tests PASS as part of the library crate.

- [ ] **Step 5: Update `recommendedfix.md` §5.1**

Update `recommendedfix.md` to document that `tests/agent_loop.rs` has been migrated to in-crate tests under `src/agent/agent_loop/integration_tests.rs`.

- [ ] **Step 6: Commit**

```bash
git add src/agent/agent_loop/integration_tests.rs src/agent/agent_loop/mod.rs tests/agent_loop.rs recommendedfix.md
git commit -m "refactor(tests): migrate standalone agent_loop integration test to in-crate module"
```

---

### Task 3: Parameterize Browser CDP Port & Safe Process Management

**Files:**
- Modify: `src/config/schema.rs:98-115`
- Modify: `src/tools/browser/common.rs:16-115`
- Modify: `src/tools/html_video.rs:230-260`
- Modify: `src/tools/browser/obscura.rs:205-230`
- Modify: `src/tools/image_generator.rs:335-360`

**Interfaces:**
- Consumes: `BrowserConfig::cdp_port`
- Produces: `pub fn browser_cdp_port() -> u16` helper and targeted process killing.

- [ ] **Step 1: Write test for `BrowserConfig::cdp_port` default and deserialization**

In `src/config/schema.rs`, add a test in `mod tests`:
```rust
#[test]
fn browser_config_includes_configurable_cdp_port() {
    let cfg = Config::default();
    assert_eq!(cfg.browser.cdp_port, 9222);

    let json = serde_json::json!({
        "browser": {
            "cdpPort": 9225
        }
    });
    let parsed: Config = serde_json::from_value(json).unwrap();
    assert_eq!(parsed.browser.cdp_port, 9225);
}
```

- [ ] **Step 2: Add `cdp_port` to `BrowserConfig`**

In `src/config/schema.rs:98-115`:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BrowserConfig {
    #[serde(alias = "firefox_webdriver_port")]
    pub firefox_webdriver_port: u16,
    #[serde(alias = "firefox_attach_port")]
    pub firefox_attach_port: u16,
    #[serde(alias = "cdp_port", alias = "chrome_cdp_port")]
    pub cdp_port: u16,
}

impl Default for BrowserConfig {
    fn default() -> Self {
        Self {
            firefox_webdriver_port: 4444,
            firefox_attach_port: 4445,
            cdp_port: 9222,
        }
    }
}
```

- [ ] **Step 3: Add `browser_cdp_port()` accessor and safe process kill in `src/tools/browser/common.rs`**

```rust
pub fn browser_cdp_port() -> u16 {
    crate::config::loader::load_config()
        .map(|c| c.browser.cdp_port)
        .unwrap_or(9222)
}

pub fn kill_browser_on_port(port: u16) {
    #[cfg(unix)]
    {
        // Target only known headless browser binaries on the port to avoid killing unrelated developer tools
        let cmd = format!(
            "for pid in $(lsof -t -i:{port} 2>/dev/null); do \
                cmd=$(ps -p $pid -o comm= 2>/dev/null); \
                case \"$cmd\" in *chrome*|*chromium*|*obscura*) kill -9 $pid 2>/dev/null ;; esac; \
            done"
        );
        let _ = Command::new("sh")
            .arg("-c")
            .arg(&cmd)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }
    #[cfg(windows)]
    {
        let cmd = format!(
            "for /f \"tokens=5\" %a in ('netstat -aon ^| findstr {port}') do taskkill /F /PID %a"
        );
        let _ = Command::new("cmd")
            .arg("/C")
            .arg(&cmd)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }
}

pub fn kill_browser_on_port_9222() {
    kill_browser_on_port(browser_cdp_port());
}
```

- [ ] **Step 4: Update `html_video.rs`, `obscura.rs`, and `image_generator.rs` to use dynamic port**

Replace static `"http://127.0.0.1:9222..."` with `format!("http://127.0.0.1:{}/...", browser_cdp_port())`.

- [ ] **Step 5: Run tests and clippy**

Run: `cargo test -p openz --lib browser_config -j 2`
Run: `cargo clippy -p openz --lib -j 2`
Expected: 0 warnings, 0 errors.

- [ ] **Step 6: Commit**

```bash
git add src/config/schema.rs src/tools/browser/common.rs src/tools/html_video.rs src/tools/browser/obscura.rs src/tools/image_generator.rs
git commit -m "feat(browser): parameterize CDP port and harden headless process termination"
```

---

### Task 4: Build Cache Cleanup Flag in `openz doctor` (`--clean-target`)

**Files:**
- Modify: `src/cli/args.rs:37-40`
- Modify: `src/cli/mod.rs:131-133`
- Modify: `src/cli/doctor.rs:403-502`

**Interfaces:**
- Consumes: `--clean-target` flag from CLI args.
- Produces: Safe cleanup of `target/` build directory with reclaimed space reporting.

- [ ] **Step 1: Write test for doctor clean target helper**

In `src/cli/doctor.rs`, add a unit test in `mod tests` verifying target clean logic on a temporary directory:
```rust
#[test]
fn test_clean_target_directory() -> Result<()> {
    let temp_dir = std::env::temp_dir().join(format!("openz_clean_test_{}", uuid::Uuid::new_v4()));
    let target = temp_dir.join("target");
    std::fs::create_dir_all(&target)?;
    std::fs::write(target.join("dummy.bin"), vec![0u8; 1024 * 1024])?;

    assert!(target.exists());
    let reclaimed = clean_target_cache_at(&target)?;
    assert!(reclaimed >= 1024 * 1024);
    assert!(!target.exists());
    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(())
}
```

- [ ] **Step 2: Add `clean_target_cache_at()` helper in `src/cli/doctor.rs`**

```rust
pub fn clean_target_cache_at(target_path: &Path) -> Result<u64> {
    if !target_path.exists() {
        return Ok(0);
    }
    let size = path_size_bytes(target_path).unwrap_or(0);
    std::fs::remove_dir_all(target_path)?;
    Ok(size)
}
```

- [ ] **Step 3: Update `Doctor` CLI argument in `src/cli/args.rs`**

```rust
    Doctor {
        #[arg(long)]
        scrub_secrets: bool,
        #[arg(long)]
        clean_target: bool,
    },
```

- [ ] **Step 4: Wire `--clean-target` in `src/cli/mod.rs` and `src/cli/doctor.rs`**

In `src/cli/mod.rs`:
```rust
Some(Command::Doctor { scrub_secrets, clean_target }) => {
    doctor::handle_doctor(scrub_secrets, clean_target).await?;
}
```

In `src/cli/doctor.rs`:
Update `handle_doctor(scrub_secrets: bool, clean_target: bool)` to invoke `clean_target_cache_at(&cwd.join("target"))` when `--clean-target` is passed, printing the reclaimed space before running the disk report.

- [ ] **Step 5: Run tests and verify CLI help**

Run: `cargo test -p openz --lib test_clean_target_directory -j 2`
Run: `cargo run -- doctor --help`
Expected: `--clean-target` flag appears in help text and test passes.

- [ ] **Step 6: Commit**

```bash
git add src/cli/args.rs src/cli/mod.rs src/cli/doctor.rs
git commit -m "feat(doctor): add --clean-target flag for easy build-cache purging"
```

---

### Task 5: End-to-End Invariant Verification & Roadmap Update

**Files:**
- Modify: `recommendedfix.md`
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Run full library test suite sequential check**

Run: `cargo test -p openz --lib test_native_tool_registration_names -j 2`
Run: `cargo test -p openz --lib websocket_protocol_events_serialize_safely -j 2`
Run: `cargo test -p openz --lib agent_loop::integration_tests -j 2`
Run: `cargo clippy --workspace --lib -j 2`
Expected: All tests PASS, exactly 128 native tools registered, 0 clippy warnings workspace-wide.

- [ ] **Step 2: Update documentation and bump changelog**

Record `v0.0.148` in `Cargo.toml` and `CHANGELOG.md`.

- [ ] **Step 3: Commit and push**

```bash
git add Cargo.toml CHANGELOG.md recommendedfix.md
git commit -m "chore(release): bump openz to v0.0.148 with hardening and cleanup fixes"
git push origin main
```
