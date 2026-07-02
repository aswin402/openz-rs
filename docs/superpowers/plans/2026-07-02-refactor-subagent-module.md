# Modularizing the Subagent Tools Module

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Modularize the monolithic 3000-line `src/tools/subagent.rs` file into a clean module folder `src/tools/subagent/` containing dedicated files for each tool and helper class.

**Architecture:** Extract each tool class and its specific helper functions into its own file, leaving `src/tools/subagent/mod.rs` to re-export them. `CancellationToken` and shared helper functions will reside in dedicated utility files within the module.

**Tech Stack:** Rust, standard workspace dependencies, tokio, serde_json.

## Global Constraints
- Do not modify how external modules consume `crate::tools::subagent`. By re-exporting all tools, structs, and tokens from `src/tools/subagent/mod.rs`, consumers like `src/cli.rs`, `src/agent/agent_loop.rs`, and `src/tools/mod.rs` can compile without any modification.
- Maintain exact signatures, names, and trait implementations for all tools.
- Maintain all existing comments and logic.

---

### Task 1: Create the Subagent Module Structure and Cancellation Token
Create the sub-module directory and place the `CancellationToken` definition there.

**Files:**
- Create: `src/tools/subagent/cancellation_token.rs`
- Create: `src/tools/subagent/mod.rs`

- [ ] **Step 1: Write `src/tools/subagent/cancellation_token.rs`**
  Write the full implementation of `CancellationToken` extracted from `src/tools/subagent.rs:2911-2968`.
  ```rust
  #[derive(Clone, Debug)]
  pub struct CancellationToken {
      cancelled: std::sync::Arc<std::sync::atomic::AtomicBool>,
      notify: std::sync::Arc<tokio::sync::Notify>,
  }

  impl Default for CancellationToken {
      fn default() -> Self {
          Self::new()
      }
  }

  impl CancellationToken {
      pub fn new() -> Self {
          let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
          let notify = std::sync::Arc::new(tokio::sync::Notify::new());

          let cancelled_clone = cancelled.clone();
          let notify_clone = notify.clone();
          tokio::spawn(async move {
              if let Some(mut rx) = crate::shutdown::receiver() {
                  if *rx.borrow() {
                      cancelled_clone.store(true, std::sync::atomic::Ordering::SeqCst);
                      notify_clone.notify_waiters();
                      return;
                  }
                  while rx.changed().await.is_ok() {
                      if *rx.borrow() {
                          cancelled_clone.store(true, std::sync::atomic::Ordering::SeqCst);
                          notify_clone.notify_waiters();
                          break;
                      }
                  }
              }
          });

          Self {
              cancelled,
              notify,
          }
      }

      pub fn cancel(&self) {
          self.cancelled.store(true, std::sync::atomic::Ordering::SeqCst);
          self.notify.notify_waiters();
      }

      pub fn is_cancelled(&self) -> bool {
          self.cancelled.load(std::sync::atomic::Ordering::SeqCst)
      }

      pub async fn wait_for_cancellation(&self) {
          if self.is_cancelled() {
              return;
          }
          self.notify.notified().await;
      }
  }
  ```

- [ ] **Step 2: Initialize `src/tools/subagent/mod.rs`**
  Declare the submodules, import files, and define task-local variable `DELEGATION_DEPTH` and shared helpers/imports:
  ```rust
  use std::sync::Arc;
  use crate::providers::LLMProvider;
  use crate::config::schema::Config;

  tokio::task_local! {
      pub static DELEGATION_DEPTH: usize;
  }

  pub mod cancellation_token;
  pub mod delegate_task;
  pub mod delegate_profile;
  pub mod evaluator_optimizer;
  pub mod optimize_profile;
  pub mod parallel_research;

  #[cfg(test)]
  mod tests;

  pub use cancellation_token::CancellationToken;
  pub use delegate_task::DelegateTaskTool;
  pub use delegate_profile::DelegateProfileTool;
  pub use evaluator_optimizer::EvaluatorOptimizerLoopTool;
  pub use optimize_profile::{OptimizeSubagentTool, CreateSubagentTool, DeleteSubagentTool};
  pub use parallel_research::ParallelResearchTool;

  // Shared utility function used across tools:
  pub fn build_provider_for_model(config: &Config, model: &str) -> anyhow::Result<Arc<dyn LLMProvider>> {
      let resolved = crate::providers::resolver::resolve_provider_full(config, model)?;
      Ok(resolved.instance)
  }
  ```

- [ ] **Step 3: Commit**
  ```bash
  git add src/tools/subagent/mod.rs src/tools/subagent/cancellation_token.rs
  git commit -m "refactor: initialize subagent module and cancellation_token"
  ```

---

### Task 2: Extract `DelegateTaskTool` and Workspace Helpers
Extract `DelegateTaskTool` and its specific workspace management functions (`create_isolated_workspace`, `WorktreeGuard`, `cleanup_isolated_workspace`, etc.) from `src/tools/subagent.rs` into `src/tools/subagent/delegate_task.rs`.

**Files:**
- Create: `src/tools/subagent/delegate_task.rs`

- [ ] **Step 1: Write `src/tools/subagent/delegate_task.rs`**
  Copy the implementation of `DelegateTaskTool` and all its helper functions. Include required imports:
  ```rust
  use crate::tools::Tool;
  use crate::tools::ToolRegistry;
  use crate::agent::style::*;
  use crate::agent::AgentLoop;
  use crate::config::schema::Config;
  use crate::providers::LLMProvider;
  use crate::session::SessionManager;
  use anyhow::{Result, anyhow};
  use std::sync::Arc;
  use serde_json::Value;
  use std::path::{Path, PathBuf};
  use std::fs;
  use super::{CancellationToken, DELEGATION_DEPTH, build_provider_for_model};

  pub struct DelegateTaskTool {
      pub config: Config,
      pub parent_provider: Arc<dyn LLMProvider>,
      pub session_manager: SessionManager,
      pub parent_tools: Vec<Arc<dyn Tool>>,
      pub cancellation_token: CancellationToken,
  }

  #[async_trait::async_trait]
  impl Tool for DelegateTaskTool {
      // ... Copy tool implementation from src/tools/subagent.rs:27-357 ...
  }

  pub fn ensure_markdown_images(text: &str) -> String {
      // ... Copy function from src/tools/subagent.rs:359-388 ...
  }

  pub struct WorktreeGuard {
      pub parent_dir: PathBuf,
      pub worktree_dir: PathBuf,
  }

  impl WorktreeGuard {
      // ... Copy implementation from src/tools/subagent.rs:1951-1973 ...
  }

  pub fn create_isolated_workspace(parent_dir: &Path) -> Result<PathBuf> {
      // ... Copy function from src/tools/subagent.rs:2142-2225 ...
  }

  pub fn copy_dir_recursive_filtered(src: &Path, dst: &Path) -> Result<()> {
      // ... Copy function from src/tools/subagent.rs:2227-2249 ...
  }

  pub fn cleanup_isolated_workspace(parent_dir: &Path, worktree_dir: &Path) {
      // ... Copy function from src/tools/subagent.rs:2251-2284 ...
  }

  pub fn sync_changes_back(src_dir: &Path, dst_dir: &Path) -> Result<()> {
      // ... Copy function from src/tools/subagent.rs:2286-2417 ...
  }
  ```

- [ ] **Step 2: Commit**
  ```bash
  git add src/tools/subagent/delegate_task.rs
  git commit -m "refactor: extract DelegateTaskTool and workspace helpers"
  ```

---

### Task 3: Extract `DelegateProfileTool` and Subagent Helpers
Extract `DelegateProfileTool` and its specific formatting/filtering functions from `src/tools/subagent.rs` into `src/tools/subagent/delegate_profile.rs`.

**Files:**
- Create: `src/tools/subagent/delegate_profile.rs`

- [ ] **Step 1: Write `src/tools/subagent/delegate_profile.rs`**
  Copy `DelegateProfileTool` and its helper functions (`format_subagent_name`, `filter_tools_for_subagent`):
  ```rust
  use crate::tools::Tool;
  use crate::tools::ToolRegistry;
  use crate::agent::style::*;
  use crate::agent::AgentLoop;
  use crate::config::schema::Config;
  use crate::providers::LLMProvider;
  use crate::session::SessionManager;
  use crate::subagents::SubagentProfile;
  use anyhow::{Result, anyhow};
  use std::sync::Arc;
  use serde_json::Value;
  use super::{CancellationToken, DELEGATION_DEPTH, build_provider_for_model};
  use super::delegate_task::{create_isolated_workspace, WorktreeGuard};

  pub struct DelegateProfileTool {
      pub config: Config,
      pub parent_provider: Arc<dyn LLMProvider>,
      pub session_manager: SessionManager,
      pub profile: SubagentProfile,
      pub parent_tools: Vec<Arc<dyn Tool>>,
      pub cancellation_token: CancellationToken,
  }

  #[async_trait::async_trait]
  impl Tool for DelegateProfileTool {
      // ... Copy tool implementation from src/tools/subagent.rs:403-1165 ...
  }

  pub fn format_subagent_name(name: &str) -> String {
      // ... Copy function from src/tools/subagent.rs:1655-1696 ...
  }

  pub fn filter_tools_for_subagent(subagent_name: &str, all_tools: &[Arc<dyn Tool>]) -> Vec<Arc<dyn Tool>> {
      // ... Copy function from src/tools/subagent.rs:2545-2656 ...
  }
  ```

- [ ] **Step 2: Commit**
  ```bash
  git add src/tools/subagent/delegate_profile.rs
  git commit -m "refactor: extract DelegateProfileTool and formatting helpers"
  ```

---

### Task 4: Extract `EvaluatorOptimizerLoopTool` and Schema Validation
Extract `EvaluatorOptimizerLoopTool` and `validate_schema` helper from `src/tools/subagent.rs` into `src/tools/subagent/evaluator_optimizer.rs`.

**Files:**
- Create: `src/tools/subagent/evaluator_optimizer.rs`

- [ ] **Step 1: Write `src/tools/subagent/evaluator_optimizer.rs`**
  Copy the struct and implementations:
  ```rust
  use crate::tools::Tool;
  use crate::tools::ToolRegistry;
  use crate::agent::style::*;
  use crate::agent::AgentLoop;
  use crate::config::schema::Config;
  use crate::providers::LLMProvider;
  use crate::session::SessionManager;
  use anyhow::{Result, anyhow};
  use std::sync::Arc;
  use serde_json::Value;
  use super::{CancellationToken, DELEGATION_DEPTH, build_provider_for_model};

  pub struct EvaluatorOptimizerLoopTool {
      pub config: Config,
      pub parent_provider: Arc<dyn LLMProvider>,
      pub session_manager: SessionManager,
      pub parent_tools: Vec<Arc<dyn Tool>>,
      pub cancellation_token: CancellationToken,
  }

  #[async_trait::async_trait]
  impl Tool for EvaluatorOptimizerLoopTool {
      // ... Copy tool implementation from src/tools/subagent.rs:1276-1472 ...
  }

  pub fn validate_schema(value: &Value, schema: &Value) -> Result<(), String> {
      // ... Copy function from src/tools/subagent.rs:2419-2535 ...
  }
  ```

- [ ] **Step 2: Commit**
  ```bash
  git add src/tools/subagent/evaluator_optimizer.rs
  git commit -m "refactor: extract EvaluatorOptimizerLoopTool"
  ```

---

### Task 5: Extract Profile Management Tools
Extract `OptimizeSubagentTool`, `CreateSubagentTool`, and `DeleteSubagentTool` from `src/tools/subagent.rs` into `src/tools/subagent/optimize_profile.rs`.

**Files:**
- Create: `src/tools/subagent/optimize_profile.rs`

- [ ] **Step 1: Write `src/tools/subagent/optimize_profile.rs`**
  Copy the profile management tools code.
  ```rust
  use crate::tools::Tool;
  use crate::config::schema::Config;
  use crate::providers::LLMProvider;
  use crate::session::SessionManager;
  use crate::subagents::SubagentProfile;
  use anyhow::{Result, anyhow};
  use std::sync::Arc;
  use serde_json::Value;
  use super::{CancellationToken, build_provider_for_model};

  pub struct OptimizeSubagentTool {
      pub config: Config,
      pub parent_provider: Arc<dyn LLMProvider>,
      pub session_manager: SessionManager,
      pub cancellation_token: CancellationToken,
  }

  #[async_trait::async_trait]
  impl Tool for OptimizeSubagentTool {
      // ... Copy tool implementation from src/tools/subagent.rs:1173-1263 ...
  }

  pub struct CreateSubagentTool {
      pub config: Config,
      pub parent_provider: Arc<dyn LLMProvider>,
      pub session_manager: SessionManager,
  }

  #[async_trait::async_trait]
  impl Tool for CreateSubagentTool {
      // ... Copy tool implementation from src/tools/subagent.rs:1479-1593 ...
  }

  pub struct DeleteSubagentTool;

  #[async_trait::async_trait]
  impl Tool for DeleteSubagentTool {
      // ... Copy tool implementation from src/tools/subagent.rs:1596-1653 ...
  }
  ```

- [ ] **Step 2: Commit**
  ```bash
  git add src/tools/subagent/optimize_profile.rs
  git commit -m "refactor: extract profile management tools"
  ```

---

### Task 6: Extract `ParallelResearchTool` and Utilities
Extract `ParallelResearchTool` and its specific helper functions from `src/tools/subagent.rs` into `src/tools/subagent/parallel_research.rs`.

**Files:**
- Create: `src/tools/subagent/parallel_research.rs`

- [ ] **Step 1: Write `src/tools/subagent/parallel_research.rs`**
  Copy the parallel research tool code and helpers:
  ```rust
  use crate::tools::Tool;
  use crate::agent::style::*;
  use crate::config::schema::Config;
  use crate::providers::LLMProvider;
  use crate::session::SessionManager;
  use anyhow::{Result, anyhow};
  use std::sync::Arc;
  use serde_json::Value;
  use super::{CancellationToken, DELEGATION_DEPTH, build_provider_for_model};

  pub struct ParallelResearchTool {
      pub config: Config,
      pub parent_provider: Arc<dyn LLMProvider>,
      pub session_manager: SessionManager,
      pub parent_tools: Vec<Arc<dyn Tool>>,
      pub cancellation_token: CancellationToken,
  }

  #[async_trait::async_trait]
  impl Tool for ParallelResearchTool {
      // ... Copy tool implementation from src/tools/subagent.rs:1705-1946 ...
  }

  pub fn get_status_from_goal(goal: &str) -> String {
      // ... Copy function from src/tools/subagent.rs:2970-2983 ...
  }

  pub fn get_heuristic_role(index: usize, goal: &str) -> String {
      // ... Copy function from src/tools/subagent.rs:2985-3005 ...
  }
  ```

- [ ] **Step 2: Commit**
  ```bash
  git add src/tools/subagent/parallel_research.rs
  git commit -m "refactor: extract ParallelResearchTool"
  ```

---

### Task 7: Extract Unit Tests
Extract all unit tests from `src/tools/subagent.rs` into `src/tools/subagent/tests.rs`.

**Files:**
- Create: `src/tools/subagent/tests.rs`

- [ ] **Step 1: Write `src/tools/subagent/tests.rs`**
  Copy tests and verify imports are present.
  ```rust
  use super::*;
  use crate::config::schema::Config;
  use crate::session::SessionManager;
  use crate::tools::Tool;
  use std::sync::Arc;
  use serde_json::Value;
  use super::evaluator_optimizer::validate_schema;

  // ... Copy unit tests from src/tools/subagent.rs:2659-2906 ...
  ```

- [ ] **Step 2: Commit**
  ```bash
  git add src/tools/subagent/tests.rs
  git commit -m "refactor: extract subagent unit tests"
  ```

---

### Task 8: Swap the Monolith with the New Module
Remove the monolithic `src/tools/subagent.rs` file. Because all sub-components are re-exported in `src/tools/subagent/mod.rs`, Rust compiler will naturally compile them without changes to consumers.

**Files:**
- Delete: `src/tools/subagent.rs`

- [ ] **Step 1: Remove `src/tools/subagent.rs`**
  ```bash
  git rm src/tools/subagent.rs
  ```

- [ ] **Step 2: Verify Compilation**
  Check that the project compiles correctly.
  Run: `cargo check`
  Expected: Compile successfully.

- [ ] **Step 3: Run Tests**
  Run: `cargo test --lib -- tools::subagent`
  Expected: All unit tests in the subagent module pass.

- [ ] **Step 4: Commit**
  ```bash
  git commit -m "refactor: replace subagent.rs monolith with modular subagent module"
  ```
