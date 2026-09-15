# OpenZ — Provider Reliability & Telegram Channel Concurrency Test Suite Decomposition (Phase 31)

## Goal
Decompose embedded unit test suites across provider reliability infrastructure (`providers::circuit_breaker`, `providers::transport`, `providers::risk`) and Telegram channel concurrency/state utilities (`channels::telegram::lock`, `channels::telegram::state`) into dedicated sibling test modules. This isolates test code from operational production code while preserving full internal visibility and bumping OpenZ to `v0.0.179`.

---

## Target Subsystems & Metrics

| Module | Source File | Extracted Test File | Est. Test Lines |
|---|---|---|---|
| Provider Circuit Breaker | `src/providers/circuit_breaker.rs` | `src/providers/circuit_breaker_tests.rs` | ~72 lines |
| Provider Transport Defaults | `src/providers/transport.rs` | `src/providers/transport_tests.rs` | ~36 lines |
| Provider Model Risk Classifier | `src/providers/risk.rs` | `src/providers/risk_tests.rs` | ~50 lines |
| Telegram Channel Polling Lock | `src/channels/telegram/lock.rs` | `src/channels/telegram/lock_tests.rs` | ~42 lines |
| Telegram Channel Session State | `src/channels/telegram/state.rs` | `src/channels/telegram/state_tests.rs` | ~38 lines |

---

## Detailed Task Breakdown

### Task 1: Extract `src/providers/circuit_breaker.rs` Test Suite
- **Source:** [`src/providers/circuit_breaker.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/circuit_breaker.rs)
- **Target:** [`src/providers/circuit_breaker_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/circuit_breaker_tests.rs)
- **Actions:**
  1. Create `src/providers/circuit_breaker_tests.rs` containing the unit tests for backoff duration calculations, HTTP status retryability evaluation, circuit breaker state transitions, automatic reset on success, and manual reset.
  2. Replace inline `mod tests { ... }` in `circuit_breaker.rs` with `#[cfg(test)] #[path = "circuit_breaker_tests.rs"] mod tests;`.
  3. Verify tests pass: `cargo test -p openz --lib providers::circuit_breaker -j 1`.
  4. Commit: `refactor(providers): extract circuit_breaker tests into dedicated sibling module`.

### Task 2: Extract `src/providers/transport.rs` Test Suite
- **Source:** [`src/providers/transport.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/transport.rs)
- **Target:** [`src/providers/transport_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/transport_tests.rs)
- **Actions:**
  1. Create `src/providers/transport_tests.rs` containing unit tests for provider HTTP configuration timeout defaults and chat/messages endpoint resolution.
  2. Replace inline `mod tests { ... }` in `transport.rs` with `#[cfg(test)] #[path = "transport_tests.rs"] mod tests;`.
  3. Verify tests pass: `cargo test -p openz --lib providers::transport -j 1`.
  4. Commit: `refactor(providers): extract transport tests into dedicated sibling module`.

### Task 3: Extract `src/providers/risk.rs` Test Suite
- **Source:** [`src/providers/risk.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/risk.rs)
- **Target:** [`src/providers/risk_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/providers/risk_tests.rs)
- **Actions:**
  1. Create `src/providers/risk_tests.rs` containing unit tests for unknown free model flagging, curated strong default model approval, small model warnings, and experimental model detection.
  2. Replace inline `mod tests { ... }` in `risk.rs` with `#[cfg(test)] #[path = "risk_tests.rs"] mod tests;`.
  3. Verify tests pass: `cargo test -p openz --lib providers::risk -j 1`.
  4. Commit: `refactor(providers): extract risk tests into dedicated sibling module`.

### Task 4: Extract `src/channels/telegram/lock.rs` Test Suite
- **Source:** [`src/channels/telegram/lock.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/telegram/lock.rs)
- **Target:** [`src/channels/telegram/lock_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/telegram/lock_tests.rs)
- **Actions:**
  1. Create `src/channels/telegram/lock_tests.rs` containing unit tests for bot token redaction in lock paths, duplicate poller locking rejection, and reusable lock file acquisition.
  2. Replace inline `mod tests { ... }` in `lock.rs` with `#[cfg(test)] #[path = "lock_tests.rs"] mod tests;`.
  3. Verify tests pass: `cargo test -p openz --lib channels::telegram::lock -j 1`.
  4. Commit: `refactor(telegram): extract lock tests into dedicated sibling module`.

### Task 5: Extract `src/channels/telegram/state.rs` Test Suite
- **Source:** [`src/channels/telegram/state.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/telegram/state.rs)
- **Target:** [`src/channels/telegram/state_tests.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/telegram/state_tests.rs)
- **Actions:**
  1. Create `src/channels/telegram/state_tests.rs` containing unit tests for remote session selection round-tripping and button label preview formatting.
  2. Replace inline `mod tests { ... }` in `state.rs` with `#[cfg(test)] #[path = "state_tests.rs"] mod tests;`.
  3. Verify tests pass: `cargo test -p openz --lib channels::telegram::state -j 1`.
  4. Commit: `refactor(telegram): extract state tests into dedicated sibling module`.

### Task 6: Final Verification, Documentation & Release Bump (`v0.0.179`)
- **Actions:**
  1. Update `Cargo.toml`, `onpkg.json`, and `README.md` to `0.0.179`.
  2. Add release entry in `CHANGELOG.md` with **`Ideas`**, **`Inspirations`**, **`Sources & References`**, **`Subsystem Test Suite Extractions`**, and **`Verification`**.
  3. Add Section 4.43 in `recommendedfix.md`.
  4. Verify the 260 native tools invariant: `cargo test -p openz --lib test_native_tool_registration_names -j 1`.
  5. Verify version sync: `cargo test -p openz --lib version_sync_tests -j 1`.
  6. Verify 0 clippy warnings: `cargo clippy -p openz --lib -j 2`.
  7. Commit: `chore(release): bump openz to v0.0.179 with decomposed provider reliability and telegram concurrency test suites`.
