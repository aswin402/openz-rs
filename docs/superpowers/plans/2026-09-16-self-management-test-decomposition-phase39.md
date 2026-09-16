# Self-Management Test Suite Decomposition — Execution Plan (Phase 39)

> **Milestone:** Phase 39  
> **Target Release:** `v0.0.187`  
> **Scope:** Full decomposition and deletion of monolithic [`src/tools/self_management/tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/self_management/tests.rs) (682 lines) into 8 dedicated 1:1 sibling test files.  

---

## 1. Goal & Architecture Rationale

The `src/tools/self_management/` directory implements OpenZ's self-management, configuration, diagnostics, discovery, and maintenance tools. Currently, all unit tests are concentrated in a single 682-line monolithic file ([`tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/self_management/tests.rs)).

Following the architectural pattern established in Phases 36–38 (subagents, memory_extra):
- Each domain source file will declare its own sibling test module (`#[cfg(test)] #[path = "..._tests.rs"] mod tests;`).
- Monolithic `tests.rs` will be deleted entirely.
- Exact 260 registered native tools invariant and 0 clippy warnings must be maintained.
- All tests executed with `CARGO_INCREMENTAL=0` and `-j 1`.

---

## 2. Decomposition Mapping

| Sibling Test Module | Source Module | Tests Extracted | Focus |
|---|---|:---:|---|
| [`catalog_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/self_management/catalog_tests.rs) | [`catalog.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/self_management/catalog.rs) | 2 | `ToolCatalogTool` metadata exposure, domain filtering, example formats, and resource policy checks |
| [`inventory_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/self_management/inventory_tests.rs) | [`inventory.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/self_management/inventory.rs) | 1 | `OpenZInventoryTool` runtime identity, model capabilities, guidance prompt |
| [`diagnostics_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/self_management/diagnostics_tests.rs) | [`diagnostics.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/self_management/diagnostics.rs) | 4 | `DiagnoseSystemTool` system info & DB checks, `DiagnoseToolTool` execution, mock argument normalization |
| [`scope_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/self_management/scope_tests.rs) | [`scope.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/self_management/scope.rs) | 2 | `RequestToolScopeTool` structured requests, `OptimizeToolScopeTool` prefix filtering and restoration |
| [`skills_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/self_management/skills_tests.rs) | [`skills.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/self_management/skills.rs) | 1 | `CurateSkillTool` CRUD lifecycle (add, list, delete) |
| [`config_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/self_management/config_tests.rs) | [`config.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/self_management/config.rs) | 2 | `ManageConfigTool` view/update/credential storage, `redact_secrets` nested redaction |
| [`sessions_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/self_management/sessions_tests.rs) | [`sessions.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/self_management/sessions.rs) | 1 | `ManageSessionsTool` lifecycle (list, archive, delete, prune) with `TestEnvLock` |
| [`backups_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/self_management/backups_tests.rs) | [`backups.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/self_management/backups.rs) | 1 | `ManageBackupsTool` lifecycle (create, list, restore, delete) |
| **Total** | — | **14** | **All 8 source modules with 1:1 sibling tests** |

---

## 3. Step-by-Step Tasks

- [ ] **Task 1: Extract Catalog & Inventory Tests**
  - Create `src/tools/self_management/catalog_tests.rs` (2 tests) & declare in `catalog.rs`.
  - Create `src/tools/self_management/inventory_tests.rs` (1 test) & declare in `inventory.rs`.
  - Verify with `CARGO_INCREMENTAL=0 cargo test -p openz --lib tools::self_management::catalog -j 1` and `inventory`.
  - Commit.

- [ ] **Task 2: Extract Diagnostics & Scope Tests**
  - Create `src/tools/self_management/diagnostics_tests.rs` (4 tests) & declare in `diagnostics.rs`.
  - Create `src/tools/self_management/scope_tests.rs` (2 tests) & declare in `scope.rs`.
  - Verify with `CARGO_INCREMENTAL=0 cargo test -p openz --lib tools::self_management::diagnostics -j 1` and `scope`.
  - Commit.

- [ ] **Task 3: Extract Skills & Config Tests**
  - Create `src/tools/self_management/skills_tests.rs` (1 test) & declare in `skills.rs`.
  - Create `src/tools/self_management/config_tests.rs` (2 tests) & declare in `config.rs`.
  - Verify with `CARGO_INCREMENTAL=0 cargo test -p openz --lib tools::self_management::skills -j 1` and `config`.
  - Commit.

- [ ] **Task 4: Extract Sessions & Backups Tests, Delete Monolith `tests.rs`**
  - Create `src/tools/self_management/sessions_tests.rs` (1 test with `TestEnvLock`) & declare in `sessions.rs`.
  - Create `src/tools/self_management/backups_tests.rs` (1 test) & declare in `backups.rs`.
  - Remove `mod tests;` from `src/tools/self_management/mod.rs` and delete `src/tools/self_management/tests.rs`.
  - Verify all 14 tests pass: `CARGO_INCREMENTAL=0 cargo test -p openz --lib tools::self_management -j 1 -- --test-threads=1`.
  - Commit.

- [ ] **Task 5: Invariant Verification & Linter Check**
  - 260 registered native tools: `CARGO_INCREMENTAL=0 cargo test -p openz --lib test_native_tool_registration_names -j 1`.
  - 0 clippy warnings: `cargo clippy -p openz --lib -j 2`.

- [ ] **Task 6: Release Bump (`v0.0.187`), Documentation & Artifact**
  - Increment package version to `0.0.187` in `Cargo.toml`, `onpkg.json`, and `README.md`.
  - Update `CHANGELOG.md` with release notes for `v0.0.187`.
  - Update `recommendedfix.md` with Section 4.51 and summary table.
  - Verify version sync test: `CARGO_INCREMENTAL=0 cargo test -p openz --lib release_version_surfaces_match_cargo_package_version -j 1`.
  - Generate Phase 39 summary artifact.
  - Commit release changes.
