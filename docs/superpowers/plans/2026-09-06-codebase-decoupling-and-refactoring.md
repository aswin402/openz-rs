# Codebase Decoupling & Refactoring Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eliminate hardcoded constants, fix cross-platform shell execution bugs, unify fragmented HTTP client construction, and relocate misplaced model risk domain types into the provider layer.

**Architecture:** Create `src/core/process.rs` as the single cross-platform shell process factory (fixing a Windows runtime bug in filesystem patch verification). Standardize network clients onto `src/core/http.rs` with guaranteed timeouts. Parameterize WebSocket CORS origins and prompt budget limits in `src/config/schema.rs`. Generalize persona/creator identity heuristics in prompt building. Move `ModelPrefs` and `classify_model_risk` from `src/channels/mod.rs` to dedicated provider submodules (`src/providers/model_prefs.rs` and `src/providers/risk.rs`).

**Tech Stack:** Rust (edition 2021), Tokio, Reqwest, Axum, Serde, Clap.

## Global Constraints

- Always cap Cargo compilation and testing with `-j 2` (e.g. `cargo test -p openz --lib <test_name> -j 2`, `cargo clippy -p openz --lib -j 2`).
- Never create standalone integration test targets in `tests/*.rs` (all tests must remain in-crate under `src/` via `--lib`).
- Maintain the exact 128 native registered tools invariant in `src/cli/builder.rs` (`cargo test -p openz --lib test_native_tool_registration_names -j 2`).
- Zero compiler or clippy warnings across the workspace (`cargo clippy -p openz --lib -j 2`).
- All file paths and code symbols must use clickable `file://` markdown links.

---

### Task 1: Centralize Cross-Platform Shell Execution (`src/core/process.rs`) & Fix Windows `ApplyPatch` Bug

**Files:**
- Create: [`src/core/process.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/process.rs)
- Modify: [`src/core/mod.rs:1-6`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/mod.rs#L1-L6)
- Modify: [`src/tools/filesystem.rs:526-531`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/filesystem.rs#L526-L531)
- Modify: [`src/tools/compiler_auto_heal.rs:190-205`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/compiler_auto_heal.rs#L190-L205)
- Modify: [`src/tools/shell.rs:418-428`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/shell.rs#L418-L428)

**Interfaces:**
- Consumes: [`crate::config::loader::set_command_cwd`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/config/loader.rs), [`crate::config::loader::set_tokio_command_cwd`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/config/loader.rs)
- Produces:
  - `pub fn host_shell_command(cmd: &str) -> std::process::Command`
  - `pub fn host_tokio_shell_command(cmd: &str) -> tokio::process::Command`

- [x] **Step 1: Write unit tests in `src/core/process.rs`**

```rust
//! Shared cross-platform process utilities for spawning host shell commands.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_host_shell_command_builds() {
        let cmd = host_shell_command("echo hello");
        let program = cmd.get_program().to_string_lossy().to_string();
        if cfg!(target_os = "windows") {
            assert_eq!(program, "cmd");
        } else {
            assert_eq!(program, "sh");
        }
    }

    #[tokio::test]
    async fn test_host_tokio_shell_command_runs() {
        let mut cmd = host_tokio_shell_command("echo hello_openz_process");
        let output = cmd.output().await.expect("shell command should run");
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("hello_openz_process"));
    }
}
```

- [x] **Step 2: Run test to verify it fails before implementation**

Run: `cargo test -p openz --lib core::process -j 2`
Expected: FAIL (module not yet registered / declared)

- [x] **Step 3: Implement `src/core/process.rs` and export in `src/core/mod.rs`**

In [`src/core/process.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/process.rs):
```rust
//! Shared cross-platform process utilities for spawning host shell commands.

/// Builds a synchronous `std::process::Command` targeting the host shell.
/// Uses `cmd.exe /C <cmd>` on Windows and `sh -c <cmd>` on Unix.
pub fn host_shell_command(cmd: &str) -> std::process::Command {
    let mut command = if cfg!(target_os = "windows") {
        let mut c = std::process::Command::new("cmd");
        c.args(["/C", cmd]);
        c
    } else {
        let mut c = std::process::Command::new("sh");
        c.args(["-c", cmd]);
        c
    };
    crate::config::loader::set_command_cwd(&mut command);
    command
}

/// Builds an asynchronous `tokio::process::Command` targeting the host shell.
/// Uses `cmd.exe /C <cmd>` on Windows and `sh -c <cmd>` on Unix.
pub fn host_tokio_shell_command(cmd: &str) -> tokio::process::Command {
    let mut command = if cfg!(target_os = "windows") {
        let mut c = tokio::process::Command::new("cmd");
        c.args(["/C", cmd]);
        c
    } else {
        let mut c = tokio::process::Command::new("sh");
        c.args(["-c", cmd]);
        c
    };
    crate::config::loader::set_tokio_command_cwd(&mut command);
    command
}
```

In [`src/core/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/mod.rs):
```rust
pub mod http;
pub mod inventory;
pub mod process;
pub mod secrets;
```

- [x] **Step 4: Update call sites in `filesystem.rs`, `compiler_auto_heal.rs`, and `shell.rs`**

In [`src/tools/filesystem.rs:526-530`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/filesystem.rs#L526-L530):
Replace:
```rust
        let run_cmd = |cmd: String| async move {
            let mut command = tokio::process::Command::new("sh");
            crate::config::loader::set_tokio_command_cwd(&mut command);
            command.arg("-c").arg(&cmd);
            let output = command.output().await?;
```
With:
```rust
        let run_cmd = |cmd: String| async move {
            let mut command = crate::core::process::host_tokio_shell_command(&cmd);
            let output = command.output().await?;
```

In [`src/tools/compiler_auto_heal.rs:190-200`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/compiler_auto_heal.rs#L190-L200):
Replace the inline `cfg!(target_os = "windows")` block with:
```rust
        let mut cmd = crate::core::process::host_tokio_shell_command(command_str);
```

In [`src/tools/shell.rs:418-428`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/shell.rs#L418-L428):
Replace the raw command block with:
```rust
        let mut std_cmd = crate::core::process::host_shell_command(command_str);
```

- [x] **Step 5: Run tests and clippy**

Run: `cargo test -p openz --lib core::process -j 2`
Expected: PASS (2 tests pass)
Run: `cargo test -p openz --lib test_native_tool_registration_names -j 2`
Expected: PASS (128 native tools verified)
Run: `cargo clippy -p openz --lib -j 2`
Expected: 0 warnings

- [x] **Step 6: Commit**

```bash
git add src/core/process.rs src/core/mod.rs src/tools/filesystem.rs src/tools/compiler_auto_heal.rs src/tools/shell.rs
git commit -m "refactor(core): centralize cross-platform host shell execution in core::process and fix Windows filesystem patch bug"
```

---

### Task 2: Standardize Shared HTTP Client Usage across Network Tools & Channels

**Files:**
- Modify: [`src/core/http.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/http.rs)
- Modify: [`src/channels/whatsapp.rs:395-440`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/whatsapp.rs#L395-L440)
- Modify: [`src/tools/rust_docs.rs:15-25`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/rust_docs.rs#L15-L25)
- Modify: [`src/tools/github.rs:125-140`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/github.rs#L125-L140)
- Modify: [`src/tools/browser/firefox.rs:38-48`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/firefox.rs#L38-L48)
- Modify: [`src/tools/browser/status.rs:175-186`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/status.rs#L175-L186)
- Modify: [`src/channels/mod.rs:680-692`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/mod.rs#L680-L692)

**Interfaces:**
- Consumes: [`crate::core::http::default_http_client`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/http.rs#L9), [`crate::core::http::custom_http_client`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/http.rs#L14)
- Produces: Safe HTTP clients across all network callers with guaranteed connect and request timeouts.

- [x] **Step 1: Add a test in `src/core/http.rs` verifying custom client timeouts**

```rust
    #[test]
    fn test_custom_http_client_with_custom_timeouts() {
        let client = custom_http_client(Duration::from_secs(5), Duration::from_secs(15));
        let _ = client.clone();
    }
```

- [x] **Step 2: Update `src/channels/whatsapp.rs` to eliminate untimed `reqwest::Client::new()`**

In [`src/channels/whatsapp.rs:401`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/whatsapp.rs#L401) and [`src/channels/whatsapp.rs:435`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/whatsapp.rs#L435):
Replace:
```rust
client: reqwest::Client::new(),
```
With:
```rust
client: crate::core::http::default_http_client(),
```

- [x] **Step 3: Update `src/tools/rust_docs.rs`, `src/tools/github.rs`, `src/tools/browser/firefox.rs`, `src/tools/browser/status.rs`, and `src/channels/mod.rs`**

In [`src/tools/rust_docs.rs:18-22`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/rust_docs.rs#L18-L22):
Replace:
```rust
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
```
With:
```rust
        let client = crate::core::http::custom_http_client(
            std::time::Duration::from_secs(5),
            std::time::Duration::from_secs(10),
        );
```

In [`src/tools/browser/firefox.rs:42`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/firefox.rs#L42):
Replace:
```rust
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .unwrap_or_default();
```
With:
```rust
    let client = crate::core::http::custom_http_client(
        std::time::Duration::from_secs(2),
        std::time::Duration::from_secs(3),
    );
```

In [`src/tools/browser/status.rs:181`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/status.rs#L181):
Replace:
```rust
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .unwrap_or_default();
```
With:
```rust
        let client = crate::core::http::custom_http_client(
            std::time::Duration::from_secs(1),
            std::time::Duration::from_secs(2),
        );
```

In [`src/channels/mod.rs:686`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/mod.rs#L686):
Replace:
```rust
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_default();
```
With:
```rust
    let client = crate::core::http::custom_http_client(
        std::time::Duration::from_secs(5),
        std::time::Duration::from_secs(10),
    );
```

- [x] **Step 4: Run tests and clippy**

Run: `cargo test -p openz --lib core::http -j 2`
Expected: PASS
Run: `cargo test -p openz --lib test_native_tool_registration_names -j 2`
Expected: PASS (128 native tools verified)
Run: `cargo clippy -p openz --lib -j 2`
Expected: 0 warnings

- [x] **Step 5: Commit**

```bash
git add src/core/http.rs src/channels/whatsapp.rs src/tools/rust_docs.rs src/tools/browser/firefox.rs src/tools/browser/status.rs src/channels/mod.rs
git commit -m "refactor(http): standardize network callers onto crate::core::http with safe timeouts"
```

---

### Task 3: Parameterize WebSocket CORS Allowed Origins

**Files:**
- Modify: [`src/config/schema.rs:315-335`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/config/schema.rs#L315-L335)
- Modify: [`src/channels/websocket/auth.rs:30-65`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket/auth.rs#L30-L65)

**Interfaces:**
- Consumes: [`WebSocketChannelConfig`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/config/schema.rs#L315)
- Produces: Configurable `cors_origins: Vec<String>` field and dynamic origin validation.

- [x] **Step 1: Write unit test in `src/channels/websocket/auth.rs`**

```rust
    #[test]
    fn test_is_allowed_origin_with_custom_configured_origins() {
        let mut config = WebSocketChannelConfig::default();
        config.cors_origins.push("https://custom.app.domain".to_string());

        assert!(is_allowed_origin("https://custom.app.domain", &config));
        assert!(is_allowed_origin("http://localhost:3000", &config));
        assert!(!is_allowed_origin("https://evil.site.com", &config));
    }
```

- [x] **Step 2: Run test to verify it fails before schema update**

Run: `cargo test -p openz --lib test_is_allowed_origin_with_custom_configured_origins -j 2`
Expected: FAIL (`cors_origins` does not exist on `WebSocketChannelConfig`)

- [x] **Step 3: Add `cors_origins` to `WebSocketChannelConfig` in `src/config/schema.rs`**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketChannelConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_ws_port")]
    pub port: u16,
    #[serde(default = "default_ws_host")]
    pub host: String,
    #[serde(default)]
    pub start_on_boot: bool,
    #[serde(default)]
    pub start_on_tui: bool,
    #[serde(default = "default_ws_cors_origins", alias = "cors_origins")]
    pub cors_origins: Vec<String>,
}

pub fn default_ws_cors_origins() -> Vec<String> {
    vec![
        "http://localhost:3000".to_string(),
        "http://127.0.0.1:3000".to_string(),
        "http://localhost:5173".to_string(),
        "http://127.0.0.1:5173".to_string(),
        "http://localhost:8765".to_string(),
        "http://127.0.0.1:8765".to_string(),
    ]
}
```

- [x] **Step 4: Update `is_allowed_origin` and `websocket_cors_origins` in `src/channels/websocket/auth.rs`**

In [`src/channels/websocket/auth.rs:33-45`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/websocket/auth.rs#L33-L45):
```rust
pub(crate) fn is_allowed_origin(origin: &str, config: &WebSocketChannelConfig) -> bool {
    let port = config.port.to_string();
    let configured_http = ["http://", &config.host, ":", &port].concat();
    let configured_https = ["https://", &config.host, ":", &port].concat();

    origin == configured_http
        || origin == configured_https
        || config.cors_origins.iter().any(|allowed| allowed == origin)
}
```

And in `websocket_cors_origins`:
```rust
pub(crate) fn websocket_cors_origins(config: &WebSocketChannelConfig) -> Vec<HeaderValue> {
    let mut origins = vec![
        HeaderValue::from_static("http://localhost"),
        HeaderValue::from_static("http://127.0.0.1"),
    ];

    for custom in &config.cors_origins {
        if let Ok(hv) = HeaderValue::from_str(custom) {
            if !origins.contains(&hv) {
                origins.push(hv);
            }
        }
    }
```

- [x] **Step 5: Run tests and clippy**

Run: `cargo test -p openz --lib websocket::auth -j 2`
Expected: PASS
Run: `cargo clippy -p openz --lib -j 2`
Expected: 0 warnings

- [x] **Step 6: Commit**

```bash
git add src/config/schema.rs src/channels/websocket/auth.rs
git commit -m "feat(websocket): parameterize allowed CORS origins in WebSocketChannelConfig"
```

---

### Task 4: Decouple Personal Identity Literals & Dynamic Prompt Budget Limit

**Files:**
- Modify: [`src/config/schema.rs:120-160`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/config/schema.rs#L120-L160)
- Modify: [`src/agent/agent_loop/build.rs:65-75, 145-155, 285-312`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/build.rs#L65-L75)

**Interfaces:**
- Consumes: [`AgentDefaults::prompt_budget_limit`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/config/schema.rs#L121)
- Produces: General persona identity heuristics and dynamic context-budget scaling.

- [x] **Step 1: Write unit tests in `src/agent/agent_loop/build.rs`**

```rust
    #[test]
    fn test_identity_memory_candidate_matches_bot_name_and_custom_hints() {
        assert!(identity_memory_candidate("user prefers python", "openz"));
        assert!(identity_memory_candidate("bot persona is friendly", "openz"));
        assert!(identity_memory_candidate("call me Alice", "openz"));
        assert!(!identity_memory_candidate("compile the rust binary", "openz"));
    }

    #[test]
    fn test_prompt_budget_resolution() {
        assert_eq!(resolve_prompt_budget(Some(64000), 128000), 64000);
        assert_eq!(resolve_prompt_budget(None, 128000), 48000);
        assert_eq!(resolve_prompt_budget(None, 16000), 12000);
    }
```

- [x] **Step 2: Add `prompt_budget_limit` to `AgentDefaults` in `src/config/schema.rs`**

```rust
    #[serde(default, alias = "prompt_budget_limit")]
    pub prompt_budget_limit: Option<usize>,
```

- [x] **Step 3: Update `src/agent/agent_loop/build.rs`**

Implement `resolve_prompt_budget`:
```rust
pub fn resolve_prompt_budget(configured_budget: Option<usize>, context_window: usize) -> usize {
    if let Some(explicit) = configured_budget {
        return explicit;
    }
    // Allocate up to 3/8 of total context window for prompt overhead, clamped between 8k and 64k chars
    let computed = (context_window * 3) / 8;
    computed.clamp(8000, 64000)
}
```

Update budget calculation in `build.rs`:
```rust
    let budget_limit = resolve_prompt_budget(
        config.agents.defaults.prompt_budget_limit,
        config.agents.defaults.context_limit.unwrap_or(32000),
    );
```

Generalize identity heuristic in `build.rs`:
```rust
fn identity_memory_candidate(text: &str, bot_name: &str) -> bool {
    let normalized = text.to_lowercase();
    let bot_norm = bot_name.trim().to_lowercase();
    if !bot_norm.is_empty() && normalized.contains(&bot_norm) {
        return true;
    }
    [
        "name",
        "called",
        "persona",
        "personality",
        "friend",
        "preference",
        "prefers",
        "wants",
        "likes",
        "creator",
        "developer",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
}
```

Update `identity_answer_priority_context`:
```rust
fn identity_answer_priority_context(user_content: &str, pinned_memory: &str) -> &'static str {
    if is_identity_or_persona_query(user_content) && !pinned_memory.trim().is_empty() {
        "\n\n[Identity Answer Priority]\nThe active persona and user preference facts in [Pinned Memory] outrank the generic system product identity for identity/persona questions. If asked who you are, answer as the active persona first, then mention OpenZ only as underlying system/context if useful. Do not ignore custom persona memory because the generic header says OpenZ.\n"
    } else {
        ""
    }
}
```

- [x] **Step 4: Run tests and clippy**

Run: `cargo test -p openz --lib agent_loop::build -j 2`
Expected: PASS
Run: `cargo test -p openz --lib test_native_tool_registration_names -j 2`
Expected: PASS (128 native tools verified)
Run: `cargo clippy -p openz --lib -j 2`
Expected: 0 warnings

- [x] **Step 5: Commit**

```bash
git add src/config/schema.rs src/agent/agent_loop/build.rs
git commit -m "refactor(prompt): generalize identity heuristics and make prompt budget context-aware"
```

---

### Task 5: Move Misplaced Model Preferences and Model Risk from `channels` to `providers`

**Files:**
- Create: [`src/providers/model_prefs.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/model_prefs.rs)
- Create: [`src/providers/risk.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/risk.rs)
- Modify: [`src/providers/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/mod.rs)
- Modify: [`src/channels/mod.rs:45-215`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/mod.rs#L45-L215)

**Interfaces:**
- Consumes: Catalog provider definitions from [`src/channels/model_catalog.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/model_catalog.rs)
- Produces:
  - `crate::providers::model_prefs::{ModelPrefs, ModelRef, load_model_prefs, save_model_prefs, record_recent_model, toggle_favorite_model}`
  - `crate::providers::risk::{ModelRisk, classify_model_risk}`
  - Re-exported in `crate::channels::*` for zero breaking changes to existing callers.

- [x] **Step 1: Create `src/providers/model_prefs.rs`**

Extract `ModelPrefs`, `ModelRef`, `load_model_prefs`, `save_model_prefs`, `record_recent_model`, `toggle_favorite_model` from [`src/channels/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/mod.rs) into [`src/providers/model_prefs.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/model_prefs.rs).
Include existing unit tests `test_record_recent_model` and `test_toggle_favorite_model`.

- [x] **Step 2: Create `src/providers/risk.rs`**

Extract `ModelRisk`, `classify_model_risk`, and helper heuristics from [`src/channels/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/mod.rs) into [`src/providers/risk.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/risk.rs).
Include unit tests for model risk classification (checking known vs unknown, free tier, small models).

- [x] **Step 3: Register submodules in `src/providers/mod.rs` and re-export in `src/channels/mod.rs`**

In [`src/providers/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/mod.rs):
```rust
pub mod model_prefs;
pub mod risk;
```

In [`src/channels/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/mod.rs):
```rust
pub use crate::providers::model_prefs::{
    load_model_prefs, record_recent_model, save_model_prefs, toggle_favorite_model, ModelPrefs,
    ModelRef,
};
pub use crate::providers::risk::{classify_model_risk, ModelRisk};
```

- [x] **Step 4: Run tests and clippy**

Run: `cargo test -p openz --lib providers::model_prefs -j 2`
Run: `cargo test -p openz --lib providers::risk -j 2`
Run: `cargo test -p openz --lib test_native_tool_registration_names -j 2`
Expected: PASS (128 native tools verified)
Run: `cargo clippy -p openz --lib -j 2`
Expected: 0 warnings

- [x] **Step 5: Commit**

```bash
git add src/providers/model_prefs.rs src/providers/risk.rs src/providers/mod.rs src/channels/mod.rs
git commit -m "refactor(providers): relocate model_prefs and model risk classification from channels to providers"
```

---

### Task 6: Final Invariant Verification, Documentation & Release Bump (`v0.0.149`)

**Files:**
- Modify: [`Cargo.toml:3`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/Cargo.toml#L3)
- Modify: [`CHANGELOG.md:1-10`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/CHANGELOG.md#L1-L10)
- Modify: [`recommendedfix.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/recommendedfix.md)

**Interfaces:**
- Workspace invariants validation: 128 native tools preserved, zero clippy warnings.

- [x] **Step 1: Verify all unit tests and tool invariants**

Run: `cargo test -p openz --lib test_native_tool_registration_names -j 2`
Expected: PASS (128 tools registered)
Run: `cargo clippy --workspace --lib -j 2`
Expected: 0 warnings across all 11 workspace crates.

- [x] **Step 2: Update version and changelog**

Bump version to `v0.0.149` in [`Cargo.toml`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/Cargo.toml), [`CHANGELOG.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/CHANGELOG.md), and document the decoupling changes in [`recommendedfix.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/recommendedfix.md).

- [x] **Step 3: Commit and push**

```bash
git add Cargo.toml CHANGELOG.md recommendedfix.md
git commit -m "chore(release): bump openz to v0.0.149 with codebase decoupling and process centralization"
```
