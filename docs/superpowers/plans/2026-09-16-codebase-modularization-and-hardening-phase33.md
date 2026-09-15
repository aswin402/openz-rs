# OpenZ — Core Engine & Provider Vision Test Suite Decomposition (Phase 33)

## Goal
Decompose embedded unit test suites across the provider vision heuristics (`providers::mod`), tool scope policy engine (`tools::scope_engine`), onpkg template manager tool (`tools::onpkg`), headless HTML animation video rendering plan (`tools::html_video`), and social media search integrations (`tools::social_search`) into dedicated sibling test modules. This preserves internal visibility, cleans module implementations, and bumps OpenZ to `v0.0.181`.

---

## Target Subsystems & Metrics

| Module | Source File | Extracted Test File | Est. Test Lines |
|---|---|---|---|
| Provider Vision Model Heuristics | `src/providers/mod.rs` | `src/providers/mod_tests.rs` | ~66 lines |
| Tool Scope Intent Engine | `src/tools/scope_engine.rs` | `src/tools/scope_engine_tests.rs` | ~44 lines |
| Onpkg Package Manifest Manager | `src/tools/onpkg.rs` | `src/tools/onpkg_tests.rs` | ~26 lines |
| HTML Video Render Planner | `src/tools/html_video.rs` | `src/tools/html_video_tests.rs` | ~20 lines |
| Social Media Search Tool | `src/tools/social_search.rs` | `src/tools/social_search_tests.rs` | ~28 lines |

---

## Detailed Task Breakdown

### Task 1: Extract `src/providers/mod.rs` Test Suite
- **Source:** [`src/providers/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/mod.rs)
- **Target:** [`src/providers/mod_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/mod_tests.rs)
- **Actions:**
  1. Create `src/providers/mod_tests.rs` containing the unit tests for `model_supports_vision` across local MIVI, OpenAI, Anthropic, Google, Meta Llama, Mistral, and other multimodal providers.
  2. Replace inline `mod tests { ... }` in `src/providers/mod.rs` with `#[cfg(test)] #[path = "mod_tests.rs"] mod tests;`.
  3. Verify tests pass: `cargo test -p openz --lib providers::tests -j 1`.
  4. Commit: `refactor(providers): extract vision model support tests into dedicated sibling module`.

### Task 2: Extract `src/tools/scope_engine.rs` Test Suite
- **Source:** [`src/tools/scope_engine.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/scope_engine.rs)
- **Target:** [`src/tools/scope_engine_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/scope_engine_tests.rs)
- **Actions:**
  1. Create `src/tools/scope_engine_tests.rs` containing unit tests for local repo read tool pack, external research tool pack, and direct answer tool pack scoping rules.
  2. Replace inline `mod tests { ... }` in `src/tools/scope_engine.rs` with `#[cfg(test)] #[path = "scope_engine_tests.rs"] mod tests;`.
  3. Verify tests pass: `cargo test -p openz --lib tools::scope_engine -j 1`.
  4. Commit: `refactor(tools): extract scope engine tests into dedicated sibling module`.

### Task 3: Extract `src/tools/onpkg.rs` Test Suite
- **Source:** [`src/tools/onpkg.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/onpkg.rs)
- **Target:** [`src/tools/onpkg_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/onpkg_tests.rs)
- **Actions:**
  1. Create `src/tools/onpkg_tests.rs` containing unit tests for onpkg doctor tool action and manifest synchronization.
  2. Replace inline `mod tests { ... }` in `src/tools/onpkg.rs` with `#[cfg(test)] #[path = "onpkg_tests.rs"] mod tests;`.
  3. Verify tests pass: `cargo test -p openz --lib tools::onpkg -j 1`.
  4. Commit: `refactor(tools): extract onpkg tests into dedicated sibling module`.

### Task 4: Extract `src/tools/html_video.rs` Test Suite
- **Source:** [`src/tools/html_video.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/html_video.rs)
- **Target:** [`src/tools/html_video_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/html_video_tests.rs)
- **Actions:**
  1. Create `src/tools/html_video_tests.rs` containing unit tests for HTML video render planner duration, frame limit, and segment guidance rules.
  2. Replace inline `mod tests { ... }` in `src/tools/html_video.rs` with `#[cfg(test)] #[path = "html_video_tests.rs"] mod tests;`.
  3. Verify tests pass: `cargo test -p openz --lib tools::html_video -j 1`.
  4. Commit: `refactor(tools): extract html_video render plan tests into dedicated sibling module`.

### Task 5: Extract `src/tools/social_search.rs` Test Suite
- **Source:** [`src/tools/social_search.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/social_search.rs)
- **Target:** [`src/tools/social_search_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/social_search_tests.rs)
- **Actions:**
  1. Create `src/tools/social_search_tests.rs` containing unit tests for Hacker News and Polymarket search response validation.
  2. Replace inline `mod tests { ... }` in `src/tools/social_search.rs` with `#[cfg(test)] #[path = "social_search_tests.rs"] mod tests;`.
  3. Verify tests pass: `cargo test -p openz --lib tools::social_search -j 1`.
  4. Commit: `refactor(tools): extract social search tests into dedicated sibling module`.

### Task 6: Final Verification, Documentation & Release Bump (`v0.0.181`)
- **Actions:**
  1. Update `Cargo.toml`, `onpkg.json`, and `README.md` to `0.0.181`.
  2. Add release entry in `CHANGELOG.md` with **`Ideas`**, **`Inspirations`**, **`Sources & References`**, **`Subsystem Test Suite Extractions`**, and **`Verification`**.
  3. Add Section 4.45 in `recommendedfix.md`.
  4. Verify the 260 native tools invariant: `cargo test -p openz --lib test_native_tool_registration_names -j 1`.
  5. Verify version sync: `cargo test -p openz --lib version_sync_tests -j 1`.
  6. Verify 0 clippy warnings: `cargo clippy -p openz --lib -j 2`.
  7. Commit: `chore(release): bump openz to v0.0.181 with decomposed provider vision and core tools test suites`.
