# OpenZ — Logging & Interactive TUI Test Suite Decomposition (Phase 32)

## Goal
Decompose embedded unit test suites across the logging infrastructure (`logs::mod`, `logs::subscriber`, `logs::tui`), the Ratatui interactive modal layer (`channels::ratatui::modals`), and the interactive CLI provider configuration manager (`cli::configure`) into dedicated sibling test modules. This preserves internal module access while removing test suites from production modules, bumping OpenZ to `v0.0.180`.

---

## Target Subsystems & Metrics

| Module | Source File | Extracted Test File | Est. Test Lines |
|---|---|---|---|
| Logging Filters Root | `src/logs/mod.rs` | `src/logs/mod_tests.rs` | ~22 lines |
| Log Secret Scrubber | `src/logs/subscriber.rs` | `src/logs/subscriber_tests.rs` | ~19 lines |
| Log Viewer TUI & SQLite | `src/logs/tui.rs` | `src/logs/tui_tests.rs` | ~57 lines |
| Ratatui Popup Modals | `src/channels/ratatui/modals.rs` | `src/channels/ratatui/modals_tests.rs` | ~14 lines |
| CLI Interactive Configure | `src/cli/configure.rs` | `src/cli/configure_tests.rs` | ~42 lines |

---

## Detailed Task Breakdown

### Task 1: Extract `src/logs/mod.rs` Test Suite
- **Source:** [`src/logs/mod.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/logs/mod.rs)
- **Target:** [`src/logs/mod_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/logs/mod_tests.rs)
- **Actions:**
  1. Create `src/logs/mod_tests.rs` containing unit tests for `LogLevelFilter` and `SessionFilter` string option parsing.
  2. Replace inline `mod tests { ... }` in `src/logs/mod.rs` with `#[cfg(test)] #[path = "mod_tests.rs"] mod tests;`.
  3. Verify tests pass: `cargo test -p openz --lib logs::tests -j 1`.
  4. Commit: `refactor(logs): extract root filter tests into dedicated sibling module`.

### Task 2: Extract `src/logs/subscriber.rs` Test Suite
- **Source:** [`src/logs/subscriber.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/logs/subscriber.rs)
- **Target:** [`src/logs/subscriber_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/logs/subscriber_tests.rs)
- **Actions:**
  1. Create `src/logs/subscriber_tests.rs` containing unit tests for secret scrubbing within log messages.
  2. Replace inline `mod tests { ... }` in `src/logs/subscriber.rs` with `#[cfg(test)] #[path = "subscriber_tests.rs"] mod tests;`.
  3. Verify tests pass: `cargo test -p openz --lib logs::subscriber -j 1`.
  4. Commit: `refactor(logs): extract subscriber secret scrubbing tests into dedicated sibling module`.

### Task 3: Extract `src/logs/tui.rs` Test Suite
- **Source:** [`src/logs/tui.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/logs/tui.rs)
- **Target:** [`src/logs/tui_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/logs/tui_tests.rs)
- **Actions:**
  1. Create `src/logs/tui_tests.rs` containing unit tests for SQLite log tail queries with session and level filters.
  2. Replace inline `mod tests { ... }` in `src/logs/tui.rs` with `#[cfg(test)] #[path = "tui_tests.rs"] mod tests;`.
  3. Verify tests pass: `cargo test -p openz --lib logs::tui -j 1`.
  4. Commit: `refactor(logs): extract tui log query tests into dedicated sibling module`.

### Task 4: Extract `src/channels/ratatui/modals.rs` Test Suite
- **Source:** [`src/channels/ratatui/modals.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/ratatui/modals.rs)
- **Target:** [`src/channels/ratatui/modals_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/ratatui/modals_tests.rs)
- **Actions:**
  1. Create `src/channels/ratatui/modals_tests.rs` containing unit tests for modal centered rectangle coordinate and bound calculations.
  2. Replace inline `mod tests { ... }` in `src/channels/ratatui/modals.rs` with `#[cfg(test)] #[path = "modals_tests.rs"] mod tests;`.
  3. Verify tests pass: `cargo test -p openz --lib channels::ratatui::modals -j 1`.
  4. Commit: `refactor(ratatui): extract modals layout tests into dedicated sibling module`.

### Task 5: Extract `src/cli/configure.rs` Test Suite
- **Source:** [`src/cli/configure.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/configure.rs)
- **Target:** [`src/cli/configure_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/configure_tests.rs)
- **Actions:**
  1. Create `src/cli/configure_tests.rs` containing unit tests for provider key updates and built-in provider alias endpoint defaults.
  2. Replace inline `mod tests { ... }` in `src/cli/configure.rs` with `#[cfg(test)] #[path = "configure_tests.rs"] mod tests;`.
  3. Verify tests pass: `cargo test -p openz --lib cli::configure -j 1`.
  4. Commit: `refactor(cli): extract configure tests into dedicated sibling module`.

### Task 6: Final Verification, Documentation & Release Bump (`v0.0.180`)
- **Actions:**
  1. Update `Cargo.toml`, `onpkg.json`, and `README.md` to `0.0.180`.
  2. Add release entry in `CHANGELOG.md` with **`Ideas`**, **`Inspirations`**, **`Sources & References`**, **`Subsystem Test Suite Extractions`**, and **`Verification`**.
  3. Add Section 4.44 in `recommendedfix.md`.
  4. Verify the 260 native tools invariant: `cargo test -p openz --lib test_native_tool_registration_names -j 1`.
  5. Verify version sync: `cargo test -p openz --lib version_sync_tests -j 1`.
  6. Verify 0 clippy warnings: `cargo clippy -p openz --lib -j 2`.
  7. Commit: `chore(release): bump openz to v0.0.180 with decomposed logging, ratatui modals, and cli configure test suites`.
