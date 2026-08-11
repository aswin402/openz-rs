# OpenZ Security Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the highest-risk audit findings without running full workspace builds or broad test suites.

**Architecture:** Reuse the existing Rust modules and local safety patterns. Move shared path validation into `config::loader`, keep gateway/auth changes in `channels::websocket`, and keep rollback behavior local to `tools::filesystem`.

**Tech Stack:** Rust 2021, Tokio, Axum, rusqlite, existing `just test-one <name> openz` low-resource test recipe.

## Global Constraints

- Do not run full `cargo check`, build, compile, clippy, or test unless explicitly requested.
- Use only targeted test commands through `just test-one <test_name> openz`.
- Keep Cargo parallelism capped at 2 jobs through the Justfile.
- Do not revert unrelated user work.
- Add regression tests before production changes.

---

### Task 1: Remove Destructive `zenflow_edit` Rollback

**Files:**
- Modify: `src/tools/filesystem.rs`

**Interfaces:**
- Consumes: existing `ZenflowEditTool::call`.
- Produces: rollback that restores only the edited file and does not execute `git reset --hard`.

- [x] **Step 1: Write the failing test**

Add a test in `src/tools/filesystem.rs` test module:

```rust
#[test]
fn zenflow_edit_source_does_not_use_hard_reset() {
    let source = std::fs::read_to_string("src/tools/filesystem.rs").unwrap();
    assert!(
        !source.contains("git reset --hard HEAD~1"),
        "zenflow_edit must not use destructive worktree-wide rollback"
    );
}
```

- [x] **Step 2: Run test to verify it fails**

Run: `just test-one zenflow_edit_source_does_not_use_hard_reset openz`

Expected: FAIL because the source still contains `git reset --hard HEAD~1`.

- [x] **Step 3: Implement minimal fix**

Replace the rollback branch with file-level restore:

```rust
if let Some(orig) = original_content {
    fs::write(&path, orig)?;
} else {
    let _ = fs::remove_file(&path);
}
if committed {
    let _ = run_cmd("git reset --mixed HEAD~1".to_string()).await;
}
```

Ensure `original_content` is always captured before writing.

- [x] **Step 4: Run targeted test**

Run: `just test-one zenflow_edit_source_does_not_use_hard_reset openz`

Expected: PASS.

### Task 2: Redact WebUI Approval Arguments

**Files:**
- Modify: `src/agent/security.rs`

**Interfaces:**
- Consumes: `SecurityGuard::redacted_value`.
- Produces: WebSocket `security_request.arguments` with secret fields masked.

- [x] **Step 1: Write the failing test**

Add a test in `src/agent/security.rs`:

```rust
#[test]
fn redacted_approval_arguments_mask_nested_secrets() {
    let args = json!({
        "action": "set_credential",
        "provider": { "api_key": "sk-live", "token": "tok-live" },
        "safe": "visible"
    });
    let redacted = SecurityGuard::redacted_approval_arguments(&args);
    assert_eq!(redacted["provider"]["api_key"], "********");
    assert_eq!(redacted["provider"]["token"], "********");
    assert_eq!(redacted["safe"], "visible");
}
```

- [x] **Step 2: Run test to verify it fails**

Run: `just test-one redacted_approval_arguments_mask_nested_secrets openz`

Expected: FAIL because `redacted_approval_arguments` does not exist.

- [x] **Step 3: Implement minimal fix**

Expose a small wrapper:

```rust
pub fn redacted_approval_arguments(arguments: &Value) -> Value {
    Self::redacted_value(arguments)
}
```

Change the WebSocket approval event to:

```rust
"arguments": SecurityGuard::redacted_approval_arguments(arguments),
```

- [x] **Step 4: Run targeted test**

Run: `just test-one redacted_approval_arguments_mask_nested_secrets openz`

Expected: PASS.

### Task 3: Create Config Temp Files With Private Permissions

**Files:**
- Modify: `src/config/loader.rs`

**Interfaces:**
- Consumes: existing `save_config`.
- Produces: temp and final config files with mode `0600` on Unix.

- [x] **Step 1: Write the failing test**

Add a Unix-only test:

```rust
#[cfg(unix)]
#[tokio::test]
async fn save_config_temp_write_uses_private_final_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let dir = std::env::temp_dir().join(format!("openz_cfg_perm_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    super::CONFIG_DIR_OVERRIDE
        .scope(dir.clone(), async {
            super::save_config(&crate::config::schema::Config::default()).unwrap();
        })
        .await;
    let mode = std::fs::metadata(dir.join("config.json"))
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600);
    let _ = std::fs::remove_dir_all(&dir);
}
```

- [x] **Step 2: Run test**

Run: `just test-one save_config_temp_write_uses_private_final_permissions openz`

Expected: PASS or expose permission issue depending on platform umask. If it already passes, still implement private temp-file creation to remove the race.

- [x] **Step 3: Implement minimal fix**

On Unix, create the temp file with `OpenOptionsExt::mode(0o600)`, write through the file handle, sync, then rename. Keep non-Unix behavior unchanged.

- [x] **Step 4: Run targeted test**

Run: `just test-one save_config_temp_write_uses_private_final_permissions openz`

Expected: PASS.

### Task 4: Share Safe Path Validation With DB Tools

**Files:**
- Modify: `src/config/loader.rs`
- Modify: `src/tools/filesystem.rs`
- Modify: `src/tools/db_inspector.rs`

**Interfaces:**
- Produces: `pub fn verify_safe_path(path: &Path) -> Result<()>` in `config::loader`.
- Consumes: same function from filesystem and database tools.

- [x] **Step 1: Write failing DB path test**

Add a test in `src/tools/db_inspector.rs`:

```rust
#[tokio::test]
async fn db_inspector_blocks_outside_safe_paths() {
    let tool = DbInspectorTool;
    let res = tool
        .call(&json!({
            "db_path": "/etc/passwd",
            "action": "schema"
        }))
        .await;
    assert!(res.unwrap_err().to_string().contains("Path traversal prevention"));
}
```

- [x] **Step 2: Run test to verify it fails**

Run: `just test-one db_inspector_blocks_outside_safe_paths openz`

Expected: FAIL because DB tools currently do not verify safe paths.

- [x] **Step 3: Implement minimal fix**

Move `verify_safe_path` from `src/tools/filesystem.rs` into `src/config/loader.rs` as public. Replace filesystem local calls with `crate::config::loader::verify_safe_path`, and call it in both `DbInspectorTool` and `DbWriteTool` after `resolve_path`.

- [x] **Step 4: Run targeted tests**

Run: `just test-one db_inspector_blocks_outside_safe_paths openz`

Expected: PASS.

### Task 5: Stop Following Symlinks In `find_files`

**Files:**
- Modify: `src/tools/filesystem.rs`

**Interfaces:**
- Consumes: `FindFilesTool::run_fd`.
- Produces: `fd` search results that do not cross symlink boundaries.

- [x] **Step 1: Write failing test**

Add a test that creates a workspace symlink to an outside temp directory and asserts outside files are not returned.

- [x] **Step 2: Run test to verify it fails**

Run: `just test-one find_files_does_not_follow_symlinked_dirs openz`

Expected: FAIL while `fd -L` follows symlinks.

- [x] **Step 3: Implement minimal fix**

Remove `cmd.arg("-L");` from `FindFilesTool::run_fd`.

- [x] **Step 4: Run targeted test**

Run: `just test-one find_files_does_not_follow_symlinked_dirs openz`

Expected: PASS.

### Task 6: Gateway Auth Guard For Non-Loopback Hosts

**Files:**
- Modify: `src/channels/websocket.rs`

**Interfaces:**
- Consumes: `WebSocketChannelConfig.host`.
- Produces: startup refusal when host is non-loopback and `OPENZ_GATEWAY_TOKEN` is unset.

- [x] **Step 1: Write focused unit tests**

Create tests for `gateway_token_required_for_host("127.0.0.1") == false` and `gateway_token_required_for_host("0.0.0.0") == true`.

- [x] **Step 2: Run test to verify it fails**

Run: `just test-one gateway_token_required_for_host openz`

Expected: FAIL because helper does not exist.

- [x] **Step 3: Implement minimal fix**

Add helper and call it in `WsGateway::start` before binding. Return an error if a non-loopback host is configured without `OPENZ_GATEWAY_TOKEN`.

- [x] **Step 4: Run targeted test**

Run: `just test-one gateway_token_required_for_host openz`

Expected: PASS.

### Task 7: Require Gateway Token For Sensitive WebSocket Config Updates

**Files:**
- Modify: `src/channels/websocket.rs`

**Interfaces:**
- Consumes: WebSocket `set_config` envelope.
- Produces: rejection event before mutation when credential/security/workspace/whitelist fields are changed and `OPENZ_GATEWAY_TOKEN` is unset.

- [x] **Step 1: Write focused unit tests**

Create tests for provider API keys, channel tokens, security mode, workspace, whitelist-like keys, masked placeholders, and benign preference updates.

- [x] **Step 2: Run test to verify it fails**

Run: `just test-one config_update_requires_gateway_token_for_provider_api_key openz`

Expected: FAIL because `config_update_requires_gateway_token` does not exist.

- [x] **Step 3: Implement minimal fix**

Add `config_update_requires_gateway_token(envelope)` and call it at the start of the `set_config` branch. If the update is sensitive and no gateway token is configured, emit `config_update_rejected` without echoing submitted values and skip mutation/save.

- [x] **Step 4: Run targeted tests and low-resource check**

Run:
- `just test-one config_update_requires_gateway_token_for_provider_api_key openz`
- `just test-one config_update_requires_gateway_token_for_security_mode_and_workspace openz`
- `just test-one config_update_requires_gateway_token_for_channel_tokens openz`
- `just test-one config_update_allows_benign_preferences_without_gateway_token openz`
- `just check openz`

Expected: PASS.

## Tracking Todo

- [x] Task 1: Remove destructive `zenflow_edit` rollback.
- [x] Task 2: Redact WebUI approval arguments.
- [x] Task 3: Private config temp-file creation.
- [x] Task 4: DB tools safe-path enforcement.
- [x] Task 5: Stop symlink-following in `find_files`.
- [x] Task 6: Require gateway token for non-loopback binds.
- [x] Task 7: Require gateway token for sensitive WebSocket config updates.

## Self-Review

- Spec coverage: covers bugs, hardcoded/security defaults, and improvement items surfaced in the audit.
- Placeholder scan: no task has open-ended implementation text except Task 5 symlink setup, which is intentionally test-specific and constrained to one behavior.
- Type consistency: function names are consistent across tasks.
