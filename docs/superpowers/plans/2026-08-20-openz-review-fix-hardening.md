# OpenZ Review Fix Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the concrete bugs and cleanup issues found in the v0.0.138 review: broken `Path` argument normalization, workspace-leaking Ratatui branch cache, stale subagent tool allowlists, committed session artifacts, README drift, and regenerated heavy build/cache directories.

**Architecture:** Codex acts as orchestrator/reviewer. Coding tasks are delegated one task at a time to the `gpt5.6 luna` subagent, with Codex reviewing each patch before the next task starts. Each task has its own focused test or low-resource static validation so the 1 GB RAM machine avoids full Cargo builds unless explicitly approved.

**Tech Stack:** Rust 2021, Tokio, serde_json, Ratatui, React/Vite docs surface, Git. Use `just test-one <test_name> openz` or `cargo test -p openz --lib <test_name> -j 2` only for focused tests.

## Global Constraints

- Codex is orchestrator: dispatch implementation to `gpt5.6 luna`, review diffs, control commits.
- Do not run full `cargo test`, full `cargo check`, full web build, or broad compiles on the laptop.
- Use only focused tests, `rustfmt`, `git diff --check`, `rg`, and static inspection unless user explicitly approves heavier checks.
- Preserve existing user changes; never delete tracked docs/artifacts without checking whether they are intentional.
- Use existing repo patterns; no broad refactors while fixing these review findings.
- Each task must end with a clean `git diff --check` and a small commit.

---

## File Structure

- `src/tools/mod.rs`
  - Fix global tool argument normalization for capitalized path-like keys.
  - Update tests that currently pin the broken no-`path` behavior.

- `src/channels/ratatui/app.rs`
  - Replace global branch cache with a workspace-keyed branch cache.
  - Add tests proving two workspaces do not share cached branch names.

- `src/tools/subagent/delegate_profile.rs`
  - Replace dead allowlist entries `crawl` and `obscura` with `crawl_website` and `obscura_browser`.

- `src/tools/subagent/parallel_research.rs`
  - Replace dead read-only tool entries `crawl` and `obscura` with `crawl_website` and `obscura_browser`.

- `src/tools/subagent/tests.rs`
  - Add a drift guard asserting subagent allowlisted tool names exist in the registered tool registry.

- `.gitignore`
  - Add `.zcode/` if plan/session artifacts are not intended source.

- `.zcode/plans/plan-sess_f73ed943-3561-4ba2-90e1-563c3df3d4cf.md`
  - Remove from git if `.zcode/` is confirmed as local agent state.

- `README.md`
  - Update `What's New In v0.0.115` to current `v0.0.138`, or rename to `Latest Highlights` to avoid version drift.

- `CHANGELOG.md`, `Cargo.toml`, `Cargo.lock`, `README.md`, `onpkg.json`
  - Bump final release version after fixes, only after implementation and focused verification.

---

### Task 1: Fix Capitalized Path Argument Normalization

**Worker:** `gpt5.6 luna`  
**Reviewer:** Codex orchestrator

**Files:**
- Modify: `src/tools/mod.rs:32-56`
- Modify: `src/tools/mod.rs:1703-1721`

**Interfaces:**
- Consumes: `normalize_tool_args(args: &serde_json::Value) -> serde_json::Value`
- Produces: normalized objects that preserve original keys and add compatibility aliases.

- [ ] **Step 1: Write the failing test**

Change the current test `normalize_tool_args_does_not_inject_filesystem_path_aliases` into this behavior test:

```rust
#[test]
fn normalize_tool_args_adds_filesystem_path_aliases_without_overwriting_explicit_path() {
    let normalized = normalize_tool_args(&serde_json::json!({
        "TargetFile": "src/main.rs",
        "filepath": "src/lib.rs",
        "file": "README.md",
        "Path": "Cargo.toml",
        "AbsolutePath": "/tmp/out.txt",
        "DirectoryPath": "src"
    }));

    assert_eq!(normalized["TargetFile"], "src/main.rs");
    assert_eq!(normalized["target_file"], "src/main.rs");
    assert_eq!(normalized["filepath"], "src/lib.rs");
    assert_eq!(normalized["file"], "README.md");
    assert_eq!(normalized["Path"], "Cargo.toml");
    assert_eq!(normalized["path"], "Cargo.toml");
    assert_eq!(normalized["absolute_path"], "/tmp/out.txt");
    assert_eq!(normalized["directory_path"], "src");

    let explicit = normalize_tool_args(&serde_json::json!({
        "path": "explicit.txt",
        "Path": "Cargo.toml"
    }));
    assert_eq!(explicit["path"], "explicit.txt");
    assert_eq!(explicit["Path"], "Cargo.toml");
}
```

- [ ] **Step 2: Run the focused test to verify it fails**

Run:

```bash
just test-one normalize_tool_args_adds_filesystem_path_aliases_without_overwriting_explicit_path openz
```

Expected before implementation: FAIL because `normalized["path"]` is missing.

- [ ] **Step 3: Implement minimal normalization**

Change the alias match in `normalize_tool_args`:

```rust
let alias = match k.as_str() {
    "Path" => "path".to_string(),
    "CommandLine" | "Command" | "command_line" => "command".to_string(),
    "Query" => "query".to_string(),
    "Url" | "UrlContent" => "url".to_string(),
    "Action" => "action".to_string(),
    "text" | "content_str" => "content".to_string(),
    "diff" => "patch".to_string(),
    "ImageName" | "OutputPath" => "output_path".to_string(),
    other => to_snake_case(other),
};
```

Keep the existing insertion rule:

```rust
if alias != *k && !new_map.contains_key(&alias) {
    new_map.insert(alias, normalized_value);
}
```

This preserves explicit `path` if the model already supplied one.

- [ ] **Step 4: Run focused test**

Run:

```bash
just test-one normalize_tool_args_adds_filesystem_path_aliases_without_overwriting_explicit_path openz
```

Expected: PASS.

- [ ] **Step 5: Low-resource checks**

Run:

```bash
rustfmt --edition 2021 src/tools/mod.rs
git diff --check
```

Expected: no output from `git diff --check`.

- [ ] **Step 6: Commit**

```bash
git add src/tools/mod.rs
git commit -m "fix: normalize capitalized path tool args"
```

---

### Task 2: Key Ratatui Git Branch Cache By Workspace

**Worker:** `gpt5.6 luna`  
**Reviewer:** Codex orchestrator

**Files:**
- Modify: `src/channels/ratatui/app.rs:1-11`
- Modify: `src/channels/ratatui/app.rs:341-384`
- Modify: `src/channels/ratatui/app.rs` tests module

**Interfaces:**
- Consumes: `RatatuiApp::get_git_branch(workspace: &Path) -> Option<String>`
- Produces: branch cache entries scoped by canonical workspace path.

- [ ] **Step 1: Write the failing unit test for cache isolation**

Add this test to the `#[cfg(test)]` module in `src/channels/ratatui/app.rs`:

```rust
#[test]
fn branch_cache_is_keyed_by_workspace() {
    let first = std::env::temp_dir().join(format!("openz_branch_a_{}", uuid::Uuid::new_v4()));
    let second = std::env::temp_dir().join(format!("openz_branch_b_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&first).unwrap();
    std::fs::create_dir_all(&second).unwrap();

    {
        let mut guard = super::BRANCH_CACHE.lock().unwrap();
        guard.clear();
        guard.insert(first.clone(), (Instant::now(), Some("main".to_string())));
        guard.insert(second.clone(), (Instant::now(), Some("feature".to_string())));
    }

    assert_eq!(RatatuiApp::get_git_branch(&first).as_deref(), Some("main"));
    assert_eq!(RatatuiApp::get_git_branch(&second).as_deref(), Some("feature"));

    let _ = std::fs::remove_dir_all(first);
    let _ = std::fs::remove_dir_all(second);
}
```

Expected initial compile/test failure: `BRANCH_CACHE` is not a map and has no `.clear()`/`.insert()`.

- [ ] **Step 2: Run focused test to verify it fails**

Run:

```bash
just test-one branch_cache_is_keyed_by_workspace openz
```

Expected: FAIL at compile/test due current cache type.

- [ ] **Step 3: Replace cache type**

Change imports:

```rust
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;
use std::time::{Duration, Instant};
```

Change statics:

```rust
static BRANCH_CACHE: Mutex<HashMap<PathBuf, (Instant, Option<String>)>> = Mutex::new(HashMap::new());
static IS_FETCHING_GIT: AtomicBool = AtomicBool::new(false);
```

- [ ] **Step 4: Key `get_git_branch` by canonical workspace**

Inside `get_git_branch`, compute a stable key:

```rust
let workspace_key = workspace
    .canonicalize()
    .unwrap_or_else(|_| workspace.to_path_buf());
```

Read cache by key:

```rust
if let Ok(guard) = BRANCH_CACHE.lock() {
    if let Some((last_check, branch)) = guard.get(&workspace_key) {
        cached_branch = branch.clone();
        if now.duration_since(*last_check) < Duration::from_secs(3) {
            needs_refresh = false;
        }
    }
}
```

In the background fetcher, store by the same key:

```rust
let cache_key = workspace_key.clone();
let ws = workspace_key.clone();
let fetcher = move || {
    let output = std::process::Command::new("git")
        .arg("rev-parse")
        .arg("--abbrev-ref")
        .arg("HEAD")
        .current_dir(&ws)
        .output();

    let branch = output.ok().and_then(|out| {
        if out.status.success() {
            let b = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !b.is_empty() { Some(b) } else { None }
        } else {
            None
        }
    });

    if let Ok(mut guard) = BRANCH_CACHE.lock() {
        guard.insert(cache_key, (Instant::now(), branch));
    }
    IS_FETCHING_GIT.store(false, std::sync::atomic::Ordering::SeqCst);
};
```

- [ ] **Step 5: Run focused test**

Run:

```bash
just test-one branch_cache_is_keyed_by_workspace openz
```

Expected: PASS.

- [ ] **Step 6: Low-resource checks**

Run:

```bash
rustfmt --edition 2021 src/channels/ratatui/app.rs
git diff --check
```

Expected: no output from `git diff --check`.

- [ ] **Step 7: Commit**

```bash
git add src/channels/ratatui/app.rs
git commit -m "fix: scope ratatui branch cache by workspace"
```

---

### Task 3: Fix Dead Subagent Tool Names And Add Drift Guard

**Worker:** `gpt5.6 luna`  
**Reviewer:** Codex orchestrator

**Files:**
- Modify: `src/tools/subagent/delegate_profile.rs:544-642`
- Modify: `src/tools/subagent/parallel_research.rs:26-44`
- Modify: `src/tools/subagent/tests.rs`

**Interfaces:**
- Consumes: registered native tool names from `ToolRegistry`.
- Produces: subagent allowlists that reference real tool names only.

- [ ] **Step 1: Write failing drift test**

Add a test in `src/tools/subagent/tests.rs`:

```rust
#[tokio::test]
async fn subagent_allowlisted_tools_exist_in_registry() {
    use crate::cli::tools::register_all_tools;
    use crate::config::schema::Config;
    use crate::providers::mock::MockProvider;
    use crate::session::SessionManager;
    use crate::tools::ToolRegistry;
    use std::sync::Arc;

    let registry = ToolRegistry::new();
    let config = Config::default();
    let provider = Arc::new(MockProvider::new());
    let sessions = SessionManager::new(std::env::temp_dir().join(format!(
        "openz_subagent_allowlist_{}",
        uuid::Uuid::new_v4()
    )));
    register_all_tools(&registry, &config, provider, sessions).unwrap();
    let registered = registry.tool_names();

    for tool in crate::tools::subagent::delegate_profile::all_static_subagent_allowlist_tools() {
        assert!(
            registered.contains(&tool.to_string()),
            "subagent allowlist references unregistered tool: {tool}"
        );
    }

    for tool in crate::tools::subagent::parallel_research::read_only_tool_names() {
        assert!(
            registered.contains(&tool.to_string()),
            "parallel_research read-only allowlist references unregistered tool: {tool}"
        );
    }
}
```

- [ ] **Step 2: Expose allowlist names for tests**

In `src/tools/subagent/parallel_research.rs`, add:

```rust
#[cfg(test)]
pub(crate) fn read_only_tool_names() -> &'static [&'static str] {
    READ_ONLY_TOOLS
}
```

In `src/tools/subagent/delegate_profile.rs`, add a helper near `filter_tools_for_subagent`:

```rust
#[cfg(test)]
pub(crate) fn all_static_subagent_allowlist_tools() -> Vec<&'static str> {
    let profiles: &[&str] = &[
        "planner",
        "researcher",
        "architect",
        "git_ops_agent",
        "ast_searcher",
        "database_specialist",
        "browser_operator",
        "dependency_manager",
        "frontend_architect",
        "docs_lookup_agent",
        "media_designer",
        "sop_designer",
        "api_integrator",
        "performance_tuner",
        "communication_manager",
        "document_compiler",
        "presentation_designer",
        "code_synthesizer",
        "summarizer_agent",
        "automation_agent",
        "coding_agent",
        "reviewer",
        "debugger",
        "test_engineer",
        "devops_agent",
        "refactor_agent",
        "memory_manager",
    ];

    let mut out = Vec::new();
    for profile in profiles {
        if let Some(tools) = static_allowlist_for_subagent(profile) {
            out.extend_from_slice(tools);
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}
```

To support this, extract the current `match subagent_name` from `filter_tools_for_subagent` into:

```rust
fn static_allowlist_for_subagent(subagent_name: &str) -> Option<&'static [&'static str]> {
    match subagent_name {
        // existing match arms here
        _ => None,
    }
}
```

Then `filter_tools_for_subagent` calls:

```rust
let allowed_names = static_allowlist_for_subagent(subagent_name);
```

- [ ] **Step 3: Run the focused test to verify it fails**

Run:

```bash
just test-one subagent_allowlisted_tools_exist_in_registry openz
```

Expected before fixing names: FAIL with unregistered `crawl` and `obscura`.

- [ ] **Step 4: Replace dead names**

In `delegate_profile.rs`, replace:

```rust
"crawl"
"obscura"
```

with:

```rust
"crawl_website"
"obscura_browser"
```

In `parallel_research.rs`, replace the same two names in `READ_ONLY_TOOLS`.

- [ ] **Step 5: Run focused test**

Run:

```bash
just test-one subagent_allowlisted_tools_exist_in_registry openz
```

Expected: PASS.

- [ ] **Step 6: Low-resource checks**

Run:

```bash
rustfmt --edition 2021 src/tools/subagent/delegate_profile.rs src/tools/subagent/parallel_research.rs src/tools/subagent/tests.rs
git diff --check
```

Expected: no output from `git diff --check`.

- [ ] **Step 7: Commit**

```bash
git add src/tools/subagent/delegate_profile.rs src/tools/subagent/parallel_research.rs src/tools/subagent/tests.rs
git commit -m "fix: sync subagent tool allowlists with registry"
```

---

### Task 4: Remove Tracked Session Artifact And Ignore Future `.zcode` State

**Worker:** `gpt5.6 luna`  
**Reviewer:** Codex orchestrator

**Files:**
- Modify: `.gitignore`
- Delete from git: `.zcode/plans/plan-sess_f73ed943-3561-4ba2-90e1-563c3df3d4cf.md`

**Interfaces:**
- Produces: repo no longer tracks session-specific `.zcode` plan state.

- [ ] **Step 1: Confirm `.zcode` is local state**

Run:

```bash
git ls-files .zcode
find .zcode -maxdepth 3 -type f -print
```

Expected current tracked file:

```text
.zcode/plans/plan-sess_f73ed943-3561-4ba2-90e1-563c3df3d4cf.md
```

- [ ] **Step 2: Add ignore rule**

Add this near other runtime/cache ignores in `.gitignore`:

```gitignore
.zcode/
```

- [ ] **Step 3: Remove tracked session file from git**

Run:

```bash
git rm --cached .zcode/plans/plan-sess_f73ed943-3561-4ba2-90e1-563c3df3d4cf.md
```

Then remove the local file only if Codex orchestrator confirms it is no longer needed:

```bash
rm .zcode/plans/plan-sess_f73ed943-3561-4ba2-90e1-563c3df3d4cf.md
```

- [ ] **Step 4: Verify ignore**

Run:

```bash
git status --short
git check-ignore -v .zcode/plans/example.md
```

Expected:

```text
D  .zcode/plans/plan-sess_f73ed943-3561-4ba2-90e1-563c3df3d4cf.md
.gitignore:<line>:.zcode/    .zcode/plans/example.md
```

- [ ] **Step 5: Commit**

```bash
git add .gitignore
git commit -m "chore: ignore local zcode session state"
```

---

### Task 5: Fix README Version Drift

**Worker:** `gpt5.6 luna`  
**Reviewer:** Codex orchestrator

**Files:**
- Modify: `README.md:59-70`

**Interfaces:**
- Produces: README copy that does not become stale on every release.

- [ ] **Step 1: Replace hardcoded heading**

Change:

```markdown
## 🆕 What's New In `v0.0.115`
```

to:

```markdown
## 🆕 Latest Highlights
```

- [ ] **Step 2: Ensure badge version still matches Cargo**

Run:

```bash
rg -n "version-|Version |What's New In|Latest Highlights" README.md Cargo.toml onpkg.json
```

Expected:

```text
README.md:12:...version-0.0.138...
README.md:59:## 🆕 Latest Highlights
Cargo.toml:3:version = "0.0.138"
onpkg.json:141:    "version": "0.0.138",
```

- [ ] **Step 3: Commit**

```bash
git add README.md
git commit -m "docs: remove stale whats-new version heading"
```

---

### Task 6: Clean Regenerated Heavy Local Artifacts

**Worker:** Codex orchestrator, not Luna  
**Reason:** This is destructive local cleanup, not coding.

**Files/Dirs:**
- Remove ignored local dirs only: `target`, `web/node_modules`, `web/dist`, `.fastembed_cache`, `.crush`, `logs`

- [ ] **Step 1: Show size before deletion**

Run:

```bash
du -sh . target web/node_modules web/dist .fastembed_cache .crush logs 2>/dev/null
```

Expected: current repo may be very large because `target` was regenerated.

- [ ] **Step 2: Remove ignored generated dirs**

Run only after user approval if needed:

```bash
rm -rf target web/node_modules web/dist .fastembed_cache .crush logs
```

- [ ] **Step 3: Verify git clean**

Run:

```bash
git status --short --branch
du -sh .
```

Expected:

```text
## main...origin/main
```

and repo size substantially reduced.

---

### Task 7: Version, Changelog, Final Focused Verification, Push

**Worker:** Codex orchestrator  
**Reviewer:** Codex final review

**Files:**
- Modify: `CHANGELOG.md`
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `README.md`
- Modify: `onpkg.json`

- [ ] **Step 1: Run low-resource verification**

Run only focused tests touched by this plan:

```bash
just test-one normalize_tool_args_adds_filesystem_path_aliases_without_overwriting_explicit_path openz
just test-one branch_cache_is_keyed_by_workspace openz
just test-one subagent_allowlisted_tools_exist_in_registry openz
```

If the machine lags, stop after the failing command and report which test remains.

- [ ] **Step 2: Static checks**

Run:

```bash
git diff --check
rg -n "version-|Version |Latest Release|\"version\"" README.md CHANGELOG.md Cargo.toml Cargo.lock onpkg.json
```

- [ ] **Step 3: Bump version**

If current version is `0.0.138`, bump to `0.0.139` in:

```text
Cargo.toml
Cargo.lock
README.md
onpkg.json
CHANGELOG.md
```

Top changelog entry:

```markdown
### v0.0.139 (Latest Release)
**Review Fix Hardening:**
- **Fix:** Normalized capitalized filesystem `Path` tool arguments to `path` while preserving explicit native keys.
- **Fix:** Scoped Ratatui git branch cache by workspace so parallel repo TUIs do not share stale branch names.
- **Fix:** Replaced stale subagent allowlist names with registered `crawl_website` and `obscura_browser` tools and added a registry drift guard.
- **Docs:** Removed stale README "What's New" version heading and ignored local `.zcode` session state.
- **Chore:** Bumped version to `v0.0.139`.
```

Also remove `(Latest Release)` from `v0.0.138`.

- [ ] **Step 4: Final status**

Run:

```bash
git status --short --branch
git diff --stat
```

- [ ] **Step 5: Commit and push**

Run:

```bash
git add CHANGELOG.md Cargo.toml Cargo.lock README.md onpkg.json .gitignore src/tools/mod.rs src/channels/ratatui/app.rs src/tools/subagent/delegate_profile.rs src/tools/subagent/parallel_research.rs src/tools/subagent/tests.rs
git commit -m "fix: harden review-found tool and tui regressions"
git push origin main
```

Expected:

```text
main -> main
```

---

## Orchestrator Todo

- [ ] Dispatch Task 1 to `gpt5.6 luna`.
- [ ] Review Task 1 diff and focused test result.
- [ ] Dispatch Task 2 to `gpt5.6 luna`.
- [ ] Review Task 2 diff and focused test result.
- [ ] Dispatch Task 3 to `gpt5.6 luna`.
- [ ] Review Task 3 diff and focused test result.
- [ ] Dispatch Task 4 to `gpt5.6 luna` only if user confirms `.zcode/` is local state; otherwise keep file tracked.
- [ ] Review Task 4 diff.
- [ ] Dispatch Task 5 to `gpt5.6 luna`.
- [ ] Review Task 5 diff.
- [ ] Run Task 6 cleanup directly as Codex orchestrator after approval.
- [ ] Run Task 7 final verification, version bump, changelog update, commit, and push.

## Review Checklist

- [ ] `Path` args work for filesystem tools without overwriting explicit `path`.
- [ ] Ratatui branch cache cannot leak branch names across OpenZ and NexaDesk sessions.
- [ ] Every subagent allowlisted tool exists in `ToolRegistry::tool_names()`.
- [ ] `.zcode/` local session state is ignored or intentionally documented as source.
- [ ] README has no stale release-heading version.
- [ ] Heavy generated dirs are removed or intentionally left for active development.
- [ ] `CHANGELOG.md`, `Cargo.toml`, `Cargo.lock`, `README.md`, and `onpkg.json` versions match.
