# Codebase Hardening and Deduplication Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eliminate copy-pasted boilerplate (SQLite PRAGMAs, process spawning, HTTP client instantiation), decouple hardcoded URLs and magic numbers, optimize hot-path regexes with `LazyLock`, and sanitize test temp directories without altering runtime behavior.

**Architecture:** Extend canonical helpers in [`src/core/`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/mod.rs) (`core::sqlite`, `core::process`, `core::http`), delegate database initialization across 7 modules to `core::sqlite`, parameterize Cohere embedding endpoints, and hoist hot regex patterns into zero-cost `std::sync::LazyLock` statics.

**Tech Stack:** Rust (edition 2021), rusqlite, reqwest, regex, tokio.

## Global Constraints
- Always cap Cargo compilation and testing with `-j 2` (`cargo test -p openz --lib <test_name> -j 2`, `cargo clippy -p openz --lib -j 2`).
- Never create standalone integration test targets in `tests/*.rs` (all tests must remain in-crate under `src/` via `--lib`).
- Strictly preserve the exact 128 native registered tools invariant in [`src/cli/builder.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/builder.rs).
- Zero compiler or clippy warnings across the workspace.
- 100% clickable markdown links with `file://` scheme.

---

### Task 1: Centralize SQLite PRAGMAs in `src/core/sqlite.rs` and Update Database Callers
**Files:**
- Create: [`src/core/sqlite.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/sqlite.rs)
- Modify: [`src/core/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/core/mod.rs)
- Modify: [`src/agent/skills.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/skills.rs)
- Modify: [`src/tools/semantic_search.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/semantic_search.rs)
- Modify: [`src/tools/graph_memory/branch.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/graph_memory/branch.rs)
- Modify: [`src/tools/graph_memory/db.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/graph_memory/db.rs)
- Modify: [`src/tools/shared_memory/db.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/shared_memory/db.rs)
- Modify: [`src/tools/sequential_thinking/store.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/sequential_thinking/store.rs)
- Modify: [`src/tools/headroom/cache.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/headroom/cache.rs)

**Interfaces:**
- Produces:
  - `pub const STANDARD_SQLITE_PRAGMAS: &str`
  - `pub fn apply_standard_pragmas(conn: &rusqlite::Connection) -> rusqlite::Result<()>`
  - `pub fn apply_standard_pragmas_with_extra(conn: &rusqlite::Connection, extra_sql: &str) -> rusqlite::Result<()>`

- [x] **Step 1:** Create `src/core/sqlite.rs` with tests and register `pub mod sqlite;` in `src/core/mod.rs`.
- [x] **Step 2:** Replace copy-pasted PRAGMA execution batches in `skills.rs`, `semantic_search.rs`, `graph_memory/branch.rs`, `graph_memory/db.rs`, `shared_memory/db.rs`, `sequential_thinking/store.rs`, and `headroom/cache.rs`.
- [x] **Step 3:** Run `cargo test -p openz --lib core::sqlite -j 2` and verify all touched tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2`.
- [x] **Step 5:** Commit changes.

---

### Task 2: Standardize Process Killer & HTTP Client Usage
**Files:**
- Modify: [`src/tools/browser/common.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/browser/common.rs)
- Modify: [`src/tools/shared_memory/db.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/shared_memory/db.rs)
- Modify: [`src/tools/searchxyz/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/searchxyz/mod.rs)

**Interfaces:**
- Consumes:
  - `crate::core::process::host_shell_command`
  - `crate::core::http::default_http_client`

- [x] **Step 1:** In `kill_browser_on_port`, replace custom `Command::new("sh")` and `Command::new("cmd")` blocks with `crate::core::process::host_shell_command(&cmd)`.
- [x] **Step 2:** In `src/tools/shared_memory/db.rs`, route `pub fn get_shared_client()` to `crate::core::http::default_http_client()`.
- [x] **Step 3:** In `src/tools/searchxyz/mod.rs:84`, replace fallback with `crate::core::http::default_http_client().clone()`.
- [x] **Step 4:** Run `cargo test -p openz --lib tools::browser -j 2` and `cargo test -p openz --lib tools::shared_memory -j 2`.
- [x] **Step 5:** Run `cargo clippy -p openz --lib -j 2`.
- [x] **Step 6:** Commit changes.

---

### Task 3: Cohere Base URL Parameterization & Prompt Cutoff Constant
**Files:**
- Modify: [`src/tools/shared_memory/embeddings.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/shared_memory/embeddings.rs)
- Modify: [`src/agent/agent_loop/build.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/build.rs)

- [x] **Step 1:** In `src/tools/shared_memory/embeddings.rs:162, 400`, respect `cohere_config.api_base`.
- [x] **Step 2:** In `src/agent/agent_loop/build.rs:589`, extract `3000` to `const MAX_SOURCE_CONTEXT_CHARS: usize = 3000;`.
- [x] **Step 3:** Add test in `embeddings.rs` validating custom Cohere URL derivation.
- [x] **Step 4:** Run `cargo test -p openz --lib agent::agent_loop -j 2` and `cargo test -p openz --lib tools::shared_memory -j 2`.
- [x] **Step 5:** Run `cargo clippy -p openz --lib -j 2`.
- [x] **Step 6:** Commit changes.

---

### Task 4: Compile Hot Regexes via `LazyLock`
**Files:**
- Modify: [`src/tools/outline.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/outline.rs)
- Modify: [`src/tools/memory_extra/codebase.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/memory_extra/codebase.rs)

- [x] **Step 1:** In `src/tools/outline.rs`, hoist regexes into static `LazyLock<Regex>`.
- [x] **Step 2:** In `src/tools/memory_extra/codebase.rs`, hoist all 10 language syntax regexes into static `LazyLock<Regex>`.
- [x] **Step 3:** Run `cargo test -p openz --lib tools::outline -j 2` and `cargo test -p openz --lib tools::memory_extra::codebase -j 2`.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2`.
- [x] **Step 5:** Commit changes.

---

### Task 5: Sanitize Test Temp Directories
**Files:**
- Modify: [`src/cli/tools.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/tools.rs)
- Modify: [`src/cli/builder.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/builder.rs)
- Modify: [`src/tools/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/mod.rs)

- [x] **Step 1:** Replace static `/tmp/...` paths with unique UUID-based temp dirs under `std::env::temp_dir()`.
- [x] **Step 2:** Verify 128 registered native tools invariant: `cargo test -p openz --lib test_native_tool_registration_names -j 2`.
- [x] **Step 3:** Run `cargo clippy -p openz --lib -j 2`.
- [x] **Step 4:** Commit changes.
