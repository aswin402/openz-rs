# OpenZ Native Configuration Manager Specification & Design

This document details the architectural design and implementation plan for the native `manage_config` tool in OpenZ.

---

## 1. Objective

Allow the agent and subagents to programmatically inspect the active settings of the framework (with credentials/secrets redacted) and modify default hyperparameters (like model prefixes, temperature, max tokens, and caveman mode). Changes must be immediately applied to the running execution loop as well as persisted to disk.

---

## 2. Requirements & Constraints

1. **Secrets Security (Redaction)**: Under no circumstances should raw API keys, Discord/Telegram bot tokens, or webhook verification credentials be exposed to the LLM. Any view/read operation must redact all sensitive keys containing `"api_key"`, `"bot_token"`, `"verify_token"`, or `"password"` to `"********"`.
2. **Credential Modification Block**: Editing API keys or bot tokens via this tool must be blocked. Hyperparameter modifications must only target the harmless `agents.defaults` variables.
3. **Runtime Syncing**: When settings are updated, the main agent loop must instantly sync the modifications in memory so they affect the current turn and generation.
4. **Error Resiliency**: If invalid parameters (e.g. out of bound temperature or invalid model format) are passed, the update should fail gracefully with descriptive validation errors.

---

## 3. Architecture & Data Flow

### 3.1 Data Flow Diagrams

```
[Agent LLM Loop] ──(Calls tool)──> [ManageConfigTool]
                                           │
                                           ├─ View Action: 
                                           │   ├─ Load config.json
                                           │   ├─ Recursively redact secrets
                                           │   └─ Return redacted JSON
                                           │
                                           └─ Update Action:
                                               ├─ Validate inputs
                                               ├─ Load config.json
                                               ├─ Update defaults field
                                               ├─ Save to disk
                                               └─ Return success
```

### 3.2 Turn Loop Update Syncing
To achieve real-time synchronization, the turn state machine loop will reload config at the beginning of each turn iteration:

```rust
// Inside src/agent/agent_loop/mod.rs (run_inner)
if let Ok(latest_config) = crate::config::loader::load_config() {
    self.config = latest_config.clone();
    if let Some(ref mut ctx) = self.tools.context {
        ctx.0 = latest_config;
    }
}
```

---

## 4. Testing Strategy

1. **Unit Tests**:
   - Verify secrets redaction correctly blanks out `api_key` and channel bot tokens.
   - Verify updating hyperparameters correctly modifies and saves `config.json`.
   - Verify invalid hyperparameters fail validation.
2. **Integration Tests**:
   - Verify registry context is correctly updated when config changes on disk.
