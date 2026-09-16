# OpenZ — MCP Tools, SearchXyz, OpenDoc & Shared Memory Test Suite Decomposition (Phase 34 - Grand Finale)

## Goal
Decompose the final remaining embedded unit test suites across MCP tool wrappers (`tools::github_mcp`, `tools::docs_mcp`), SearchXyz deep research and graph indexing engines (`tools::searchxyz::mod`, `tools::searchxyz::graph`), OpenDoc document processor (`tools::opendoc::mod`), and Shared Memory embedding resolution (`tools::shared_memory::embeddings`) into dedicated sibling test modules. This achieves 100% test suite decomposition across the entire OpenZ codebase and bumps OpenZ to release `v0.0.182`.

---

## Target Subsystems & Metrics

| Module | Source File | Extracted Test File | Est. Test Lines |
|---|---|---|---|
| GitHub MCP Wrapper | `src/tools/github_mcp.rs` | `src/tools/github_mcp_tests.rs` | ~19 lines |
| Rust Docs MCP Wrapper | `src/tools/docs_mcp.rs` | `src/tools/docs_mcp_tests.rs` | ~10 lines |
| SearchXyz Root & Tool Metadata | `src/tools/searchxyz/mod.rs` | `src/tools/searchxyz/mod_tests.rs` | ~54 lines |
| SearchXyz GitHub Ingest Error Classifier | `src/tools/searchxyz/graph.rs` | `src/tools/searchxyz/graph_tests.rs` | ~35 lines |
| OpenDoc OCR & Digester Server | `src/tools/opendoc/mod.rs` | `src/tools/opendoc/mod_tests.rs` | ~22 lines |
| Shared Memory Cohere Embeddings | `src/tools/shared_memory/embeddings.rs` | `src/tools/shared_memory/embeddings_tests.rs` | ~44 lines |

---

## Detailed Task Breakdown

### Task 1: Extract `src/tools/github_mcp.rs` Test Suite
- **Source:** [`src/tools/github_mcp.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/github_mcp.rs)
- **Target:** [`src/tools/github_mcp_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/github_mcp_tests.rs)
- **Actions:**
  1. Create `src/tools/github_mcp_tests.rs` containing the unit test for server initialization and pull request creation dispatch.
  2. Replace inline `mod tests { ... }` in `src/tools/github_mcp.rs` with `#[cfg(test)] #[path = "github_mcp_tests.rs"] mod tests;`.
  3. Verify tests pass: `cargo test -p openz --lib tools::github_mcp -j 1`.
  4. Commit: `refactor(tools): extract github_mcp tests into dedicated sibling module`.

### Task 2: Extract `src/tools/docs_mcp.rs` Test Suite
- **Source:** [`src/tools/docs_mcp.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/docs_mcp.rs)
- **Target:** [`src/tools/docs_mcp_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/docs_mcp_tests.rs)
- **Actions:**
  1. Create `src/tools/docs_mcp_tests.rs` containing the unit test for docs server initialization and SQLite database connection verification.
  2. Replace inline `mod tests { ... }` in `src/tools/docs_mcp.rs` with `#[cfg(test)] #[path = "docs_mcp_tests.rs"] mod tests;`.
  3. Verify tests pass: `cargo test -p openz --lib tools::docs_mcp -j 1`.
  4. Commit: `refactor(tools): extract docs_mcp tests into dedicated sibling module`.

### Task 3: Extract `src/tools/searchxyz/mod.rs` Test Suite
- **Source:** [`src/tools/searchxyz/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/searchxyz/mod.rs)
- **Target:** [`src/tools/searchxyz/mod_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/searchxyz/mod_tests.rs)
- **Actions:**
  1. Create `src/tools/searchxyz/mod_tests.rs` containing unit tests for OpenZ embedded paths resolution and complete SearchXyz tool metadata names.
  2. Replace inline `mod tests { ... }` in `src/tools/searchxyz/mod.rs` with `#[cfg(test)] #[path = "mod_tests.rs"] mod tests;`.
  3. Verify tests pass: `cargo test -p openz --lib tools::searchxyz::tests -j 1`.
  4. Commit: `refactor(searchxyz): extract root tool metadata tests into dedicated sibling module`.

### Task 4: Extract `src/tools/searchxyz/graph.rs` Test Suite
- **Source:** [`src/tools/searchxyz/graph.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/searchxyz/graph.rs)
- **Target:** [`src/tools/searchxyz/graph_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/searchxyz/graph_tests.rs)
- **Actions:**
  1. Create `src/tools/searchxyz/graph_tests.rs` containing unit tests for GitHub repo file limit error classification, auto-retry defaults, and unrelated error handling.
  2. Replace inline `mod tests { ... }` in `src/tools/searchxyz/graph.rs` with `#[cfg(test)] #[path = "graph_tests.rs"] mod tests;`.
  3. Verify tests pass: `cargo test -p openz --lib tools::searchxyz::graph -j 1`.
  4. Commit: `refactor(searchxyz): extract graph error classification tests into dedicated sibling module`.

### Task 5: Extract `src/tools/opendoc/mod.rs` Test Suite
- **Source:** [`src/tools/opendoc/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/opendoc/mod.rs)
- **Target:** [`src/tools/opendoc/mod_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/opendoc/mod_tests.rs)
- **Actions:**
  1. Create `src/tools/opendoc/mod_tests.rs` containing unit tests for OpenDoc OCR availability check tool and server initialization.
  2. Replace inline `mod tests { ... }` in `src/tools/opendoc/mod.rs` with `#[cfg(test)] #[path = "mod_tests.rs"] mod tests;`.
  3. Verify tests pass: `cargo test -p openz --lib tools::opendoc::tests -j 1`.
  4. Commit: `refactor(opendoc): extract root ocr tests into dedicated sibling module`.

### Task 6: Extract `src/tools/shared_memory/embeddings.rs` Test Suite
- **Source:** [`src/tools/shared_memory/embeddings.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/shared_memory/embeddings.rs)
- **Target:** [`src/tools/shared_memory/embeddings_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/shared_memory/embeddings_tests.rs)
- **Actions:**
  1. Create `src/tools/shared_memory/embeddings_tests.rs` containing unit tests for Cohere embedding endpoint URL resolution and proxy base overrides.
  2. Replace inline `mod tests { ... }` in `src/tools/shared_memory/embeddings.rs` with `#[cfg(test)] #[path = "embeddings_tests.rs"] mod tests;`.
  3. Verify tests pass: `cargo test -p openz --lib tools::shared_memory::embeddings -j 1`.
  4. Commit: `refactor(shared_memory): extract embeddings url resolution tests into dedicated sibling module`.

### Task 7: Final Verification, Documentation & Release Bump (`v0.0.182`)
- **Actions:**
  1. Update `Cargo.toml`, `onpkg.json`, and `README.md` to `0.0.182`.
  2. Add release entry in `CHANGELOG.md` with **`Ideas`**, **`Inspirations`**, **`Sources & References`**, **`Subsystem Test Suite Extractions`**, and **`Verification`**.
  3. Add Section 4.46 in `recommendedfix.md` marking 100% embedded unit test suite decomposition across OpenZ complete.
  4. Verify the 260 native tools invariant: `cargo test -p openz --lib test_native_tool_registration_names -j 1`.
  5. Verify version sync: `cargo test -p openz --lib version_sync_tests -j 1`.
  6. Verify 0 clippy warnings: `cargo clippy -p openz --lib -j 2`.
  7. Commit: `chore(release): bump openz to v0.0.182 completing 100% test suite modularization across entire codebase`.
