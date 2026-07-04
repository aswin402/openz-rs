# Native Configuration Manager Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the `manage_config` native tool and turn loop config reloading to allow OpenZ to dynamically view and update its hyperparameter settings in a safe and secure manner.

**Architecture:** Create a `ManageConfigTool` tool in `self_management.rs` with view/update actions, filter out credential keys for security, and add automatic config reloading inside the `AgentLoop::run_inner` TurnState loop to enable real-time synchronization.

**Tech Stack:** Rust, serde_json, anyhow.

## Global Constraints
- Under no circumstances should credentials (API keys, bot tokens, verify tokens) be returned in raw form or modified by this tool.
- Hyperparameter changes must only affect non-sensitive fields under `agents.defaults`.
- Real-time turn-loop synchronization must be safe from file read failures.

---

### Task 1: Real-time Configuration reloading in Turn Loop

**Files:**
- Modify: [src/agent/agent_loop/mod.rs](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/mod.rs)

**Interfaces:**
- Consumes: [crate::config::loader::load_config](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/config/loader.rs)
- Produces: Dynamically reloaded `self.config` and `self.tools.context.0` at the start of each iteration in `run_inner`.

- [ ] **Step 1: Implement the config reload logic**
  At the beginning of the `while state != TurnState::Done` loop in `run_inner` (approx line 278), reload config from disk if successful and apply it:
  ```rust
  if let Ok(latest_config) = crate::config::loader::load_config() {
      self.config = latest_config.clone();
      if let Some(ref mut ctx) = self.tools.context {
          ctx.0 = latest_config;
      }
  }
  ```
- [ ] **Step 2: Run cargo check to verify compilation**
- [ ] **Step 3: Commit changes**
  `git commit -m "feat: reload configuration on each loop turn iteration"`

---

### Task 2: Implement ManageConfigTool

**Files:**
- Modify: [src/tools/self_management.rs](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/self_management.rs)
- Modify: [src/cli/builder.rs](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/builder.rs)

**Interfaces:**
- Produces: `ManageConfigTool` (`manage_config`) implementing `Tool`.

- [ ] **Step 1: Implement redaction helper function**
  Implement a recursive redaction helper that clones the config JSON and replaces values of keys containing `"api_key"`, `"bot_token"`, `"verify_token"`, `"password"`, `"secret"` with `"********"`.
- [ ] **Step 2: Implement ManageConfigTool tool struct**
  Create `ManageConfigTool` implementing `Tool`.
  - For action `"view"`, serialize active configuration, run the redaction helper, and return the redacted JSON.
  - For action `"update"`, load `config.json`, validate the input updates (only update `defaults` keys: `model`, `provider`, `max_tokens`, `temperature`, `caveman_mode`, `tool_timeout_secs`, `streaming`, `max_tool_iterations`), apply them, and call `save_config`.
- [ ] **Step 3: Register ManageConfigTool in the builder**
  Register `std::sync::Arc::new(crate::tools::self_management::ManageConfigTool)` in [src/cli/builder.rs](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/builder.rs).
- [ ] **Step 4: Write unit tests in self_management.rs**
  Verify the following scenarios:
  - "view" correctly redacts sensitive fields like `api_key` or `bot_token`.
  - "update" correctly saves new default temperature, model, etc., to disk.
- [ ] **Step 5: Verify tests compile and run**
  `cargo test --lib -- tools::self_management::tests`
- [ ] **Step 6: Commit changes**
  `git commit -m "feat: implement native manage_config tool with secrets redaction"`

---

### Task 3: Integration Verification & Docs

**Files:**
- Modify: [onpkg.json](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/onpkg.json)
- Modify: [onpkg_docs/tools.md](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/onpkg_docs/tools.md)

- [ ] **Step 1: Update onpkg.json and tools documentation**
  - Add `manage_config` to `self_management` tools listing in `onpkg.json`.
  - Document `manage_config` in `onpkg_docs/tools.md`.
- [ ] **Step 2: Run all workspace tests to verify compatibility**
  `cargo test`
- [ ] **Step 3: Commit and push changes**
  `git commit -m "docs: document manage_config tool in skill guide and package configuration" && git push origin main`
