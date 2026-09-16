# OpenZ — Subagent Profile Tool Filtering, Policy & Orchestration Modularization (Phase 35)

## Goal
Modularize subagent profile tool filtering, static capability allowlists, workspace isolation requirements, and model fallback resolution from `src/tools/subagent/delegate_profile.rs` into a dedicated domain module `src/tools/subagent/allowlist.rs` with sibling unit test suite `src/tools/subagent/allowlist_tests.rs`. This reduces `delegate_profile.rs` by ~54% (from 595 lines down to ~270 lines), decouples subagent policy and tool routing rules, and bumps OpenZ to release `v0.0.183`.

---

## Target Subsystems & Metrics

| Module | Source File | Extracted Domain / Test Files | Est. Lines | Expected Reduction |
|---|---|---|---|---|
| Subagent Tool Allowlist & Policy | `src/tools/subagent/delegate_profile.rs` | `src/tools/subagent/allowlist.rs` & `allowlist_tests.rs` | ~250 lines logic + ~80 lines tests | `delegate_profile.rs`: 595 → ~270 lines (~54% reduction) |

---

## Detailed Task Breakdown

### Task 1: Create `src/tools/subagent/allowlist.rs` and `allowlist_tests.rs`
- **Source:** [`src/tools/subagent/delegate_profile.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/delegate_profile.rs)
- **Target:** [`src/tools/subagent/allowlist.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/allowlist.rs) and [`src/tools/subagent/allowlist_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/allowlist_tests.rs)
- **Actions:**
  1. Implement `static_allowlist_for_subagent(subagent_name: &str) -> Option<&'static [&'static str]>`.
  2. Implement `all_static_subagent_allowlist_tools() -> Vec<&'static str>`.
  3. Implement `filter_tools_for_subagent(subagent_name: &str, all_tools: &[Arc<dyn Tool>]) -> Vec<Arc<dyn Tool>>`.
  4. Implement `profile_needs_workspace(profile_name: &str) -> bool`.
  5. Implement `delegate_profile_models_to_try(config: &Config, profile: &SubagentProfile) -> Vec<String>`.
  6. Add unit tests in `src/tools/subagent/allowlist_tests.rs` covering tool allowlisting, `send_remote_input` exclusion, workspace policy detection, and model fallback ordering.
  7. Verify: `cargo test -p openz --lib tools::subagent::allowlist -j 1`.

### Task 2: Register `allowlist` in `src/tools/subagent/mod.rs`
- **Source:** [`src/tools/subagent/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/mod.rs)
- **Actions:**
  1. Add `pub mod allowlist;` and `pub use allowlist::*;`.
  2. Ensure all re-exports maintain 100% backward compatibility.

### Task 3: Refactor `src/tools/subagent/delegate_profile.rs`
- **Source:** [`src/tools/subagent/delegate_profile.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/subagent/delegate_profile.rs)
- **Actions:**
  1. Replace inline allowlist and tool filtering functions with `pub use super::allowlist::*;`.
  2. Consume `profile_needs_workspace(&self.profile.name)` and `delegate_profile_models_to_try(&self.config, &self.profile)`.
  3. Verify `cargo test -p openz --lib tools::subagent -j 1`.
  4. Commit: `refactor(subagent): decompose delegate_profile tool allowlist and policy into dedicated allowlist module`.

### Task 4: Invariant & Linter Verification
- **Actions:**
  1. Verify 260 registered native tools invariant: `cargo test -p openz --lib test_native_tool_registration_names -j 1`.
  2. Verify 0 clippy warnings: `cargo clippy -p openz --lib -j 2`.

### Task 5: Final Documentation, Version Sync & Release Bump (`v0.0.183`)
- **Actions:**
  1. Update `Cargo.toml`, `onpkg.json`, and `README.md` to `0.0.183`.
  2. Add release entry in `CHANGELOG.md` with **`Ideas`**, **`Inspirations`**, **`Sources & References`**, **`Subsystem Modularization Details`**, and **`Verification`**.
  3. Add Section 4.47 in `recommendedfix.md` documenting subagent profile tool filtering and policy modularization.
  4. Verify version sync: `cargo test -p openz --lib version_sync_tests -j 1`.
  5. Create Phase 35 artifact in the brain directory.
  6. Commit: `chore(release): bump openz to v0.0.183 with subagent profile allowlist and policy modularization`.
