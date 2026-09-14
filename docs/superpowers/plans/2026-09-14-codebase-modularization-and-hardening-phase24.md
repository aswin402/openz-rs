# Codebase Modularization & Channels/Config Test Suite Decomposition (Phase 24)

**Goal:** Extract embedded test suites from [`src/config/path_policy.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/config/path_policy.rs) (65 lines), [`src/channels/discord.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/discord.rs) (47 lines), [`src/channels/telegram/commands.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/telegram/commands.rs) (47 lines), [`src/channels/ratatui/markdown.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/ratatui/markdown.rs) (41 lines), and [`src/config/provider_catalog.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/config/provider_catalog.rs) (36 lines) into dedicated test modules (~236 lines total). Preserves 100% backward compatibility, exact 260 registered native tools invariant, zero clippy warnings, and bumps release to `v0.0.172`.

## Global Constraints
- For testing, use `-j 1` (`cargo test -p openz --lib <test_name> -j 1`) to preserve laptop responsiveness and eliminate swap thrashing.
- For building / clippy, compile 2 at a time (`-j 2` via `.cargo/config.toml`).
- Use hardened `[profile.dev]` and `[profile.test]` profiles (`debug = 0`, `codegen-units = 2`).
- Never create standalone integration test targets in `tests/*.rs` (all tests must remain in-crate under `src/` via `--lib`).
- Strictly preserve the exact 260 native registered tools invariant in [`src/cli/builder.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/builder.rs).
- Zero compiler or clippy warnings across the workspace.
- 100% clickable markdown links with `file://` scheme.
- Follow Release & Changelog Protocol: explicit Ideas, Inspirations, and Sources in [`CHANGELOG.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/CHANGELOG.md).

---

### Task 1: Extract Embedded Unit Tests from `src/config/path_policy.rs`
**Files:**
- Create: [`src/config/path_policy_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/config/path_policy_tests.rs)
- Modify: [`src/config/path_policy.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/config/path_policy.rs)

- [x] **Step 1:** Extract lines 187–251 of `src/config/path_policy.rs` into `src/config/path_policy_tests.rs`.
- [x] **Step 2:** In `src/config/path_policy.rs`, link via `#[cfg(test)] #[path = "path_policy_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib config::path_policy -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(config): extract embedded test suite into config/path_policy_tests.rs"`.

---

### Task 2: Extract Embedded Unit Tests from `src/channels/discord.rs`
**Files:**
- Create: [`src/channels/discord_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/discord_tests.rs)
- Modify: [`src/channels/discord.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/discord.rs)

- [x] **Step 1:** Extract lines 403–449 of `src/channels/discord.rs` into `src/channels/discord_tests.rs`.
- [x] **Step 2:** In `src/channels/discord.rs`, link via `#[cfg(test)] #[path = "discord_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib channels::discord -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(channels): extract embedded test suite into channels/discord_tests.rs"`.

---

### Task 3: Extract Embedded Unit Tests from `src/channels/telegram/commands.rs`
**Files:**
- Create: [`src/channels/telegram/commands_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/telegram/commands_tests.rs)
- Modify: [`src/channels/telegram/commands.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/telegram/commands.rs)

- [x] **Step 1:** Extract lines 474–520 of `src/channels/telegram/commands.rs` into `src/channels/telegram/commands_tests.rs`.
- [x] **Step 2:** In `src/channels/telegram/commands.rs`, link via `#[cfg(test)] #[path = "commands_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib channels::telegram::commands -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(telegram): extract embedded test suite into channels/telegram/commands_tests.rs"`.

---

### Task 4: Extract Embedded Unit Tests from `src/channels/ratatui/markdown.rs`
**Files:**
- Create: [`src/channels/ratatui/markdown_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/ratatui/markdown_tests.rs)
- Modify: [`src/channels/ratatui/markdown.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/ratatui/markdown.rs)

- [x] **Step 1:** Extract lines 214–254 of `src/channels/ratatui/markdown.rs` into `src/channels/ratatui/markdown_tests.rs`.
- [x] **Step 2:** In `src/channels/ratatui/markdown.rs`, link via `#[cfg(test)] #[path = "markdown_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib channels::ratatui::markdown -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(ratatui): extract embedded test suite into channels/ratatui/markdown_tests.rs"`.

---

### Task 5: Extract Embedded Unit Tests from `src/config/provider_catalog.rs`
**Files:**
- Create: [`src/config/provider_catalog_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/config/provider_catalog_tests.rs)
- Modify: [`src/config/provider_catalog.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/config/provider_catalog.rs)

- [x] **Step 1:** Extract lines 298–333 of `src/config/provider_catalog.rs` into `src/config/provider_catalog_tests.rs`.
- [x] **Step 2:** In `src/config/provider_catalog.rs`, link via `#[cfg(test)] #[path = "provider_catalog_tests.rs"] mod tests;`.
- [x] **Step 3:** Run `cargo test -p openz --lib config::provider_catalog -j 1` and verify all tests pass.
- [x] **Step 4:** Run `cargo clippy -p openz --lib -j 2` to ensure 0 warnings.
- [x] **Step 5:** Commit: `git commit -am "refactor(config): extract embedded test suite into config/provider_catalog_tests.rs"`.

---

### Task 6: Invariant Verification, Documentation & Release Bump (`v0.0.172`)
**Files:**
- Modify: [`Cargo.toml`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/Cargo.toml)
- Modify: [`onpkg.json`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/onpkg.json)
- Modify: [`README.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/README.md)
- Modify: [`CHANGELOG.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/CHANGELOG.md)
- Modify: [`recommendedfix.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/recommendedfix.md)

- [x] **Step 1:** Verify 260 native tools invariant: `cargo test -p openz --lib test_native_tool_registration_names -j 1`.
- [x] **Step 2:** Bump version to `0.0.172` in `Cargo.toml`, `onpkg.json`, and `README.md`.
- [x] **Step 3:** Add release notes for `v0.0.172` in `CHANGELOG.md` documenting Ideas, Inspirations, Sources, Details & Metrics, and Verification; update `recommendedfix.md`.
- [x] **Step 4:** Run `cargo test -p openz --lib version_sync_tests -j 1`.
- [x] **Step 5:** Run `cargo clippy -p openz --lib -j 2`.
- [x] **Step 6:** Commit: `git commit -am "chore(release): bump openz to v0.0.172 with decomposed channels and config test suites"`.
