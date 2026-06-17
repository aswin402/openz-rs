# OpenZ — Agent Guide

High-performance, async personal AI agent framework built in Rust. Rebranded from `nanobot`.

---

## Commands

| Command | Purpose |
|---|---|
| `cargo build --release` | Production build (~10-15MB binary) |
| `cargo build` | Debug build |
| `cargo run -- <subcommand>` | Run from source |
| `cargo test` | Run all unit tests (28 files with `#[cfg(test)]`) |
| `cargo test <test_name>` | Run specific test |
| `cargo check` | Fast type-check without codegen |
| `cargo clippy` | Lint |
| `cargo install --path .` | Install globally (see `localinstall.sh`) |

No Makefile; no CI config (GitHub Actions, etc.) present.

### Runtime subcommands (`openz <subcommand>`)

| Subcommand | Description |
|---|---|
| `onboard` | First-time LLM provider setup wizard |
| `configure` | Full config UI: providers, gateway, Telegram, Discord, WhatsApp |
| `agent` | Terminal TUI chat (also auto-starts configured background channels) |
| `gateway` | WebSocket + WebUI server (port 8765 default) |
| `telegram` | Telegram bot polling listener |
| `discord` | Discord bot gateway listener |
| `whatsapp` | WhatsApp API webhook receiver (Axum, port 8090) |
| `subagent` | TUI manager for subagent profiles |
| `sop list \| instances \| trigger \| resume` | SOP workflow engine |
| `mcp-bridge --port <N> -- <cmd> [args...]` | gRPC-to-stdio MCP bridge |
| `changelog` | View OpenZ hardware footprint specifications and version release history |

---

## Project Map

```
openz/
├── Cargo.toml              # ~55 deps. workspace root
├── build.rs                # Compiles proto/mcp.proto via tonic-build
├── CHANGELOG.md            # OpenZ specs, features, versions, and architectures
├── onpkg.json              # ONPKG meta: scripts, agent instructions
├── src/
│   ├── main.rs             # dotenv init, tokio runtime, dispatches to cli::run_cli
│   ├── cli.rs              # Clap CLI parsing, AgentLoop construction, tool registration, channel auto-start
│   ├── session.rs          # Session/Msg types, JSON persistence ~/.openz/sessions/
│   │
│   ├── config/
│   │   ├── schema.rs       # Config struct, defaults (13 providers, 4 channels, MCP servers)
│   │   └── loader.rs       # resolve_path(), load/save ~/.openz/config.json
│   │
│   ├── providers/
│   │   ├── mod.rs          # LLMProvider trait, ContentPart, LLMResponse, model_supports_vision()
│   │   ├── openai.rs       # OpenAI-compatible (DeepSeek, Groq, Ollama, etc.)
│   │   └── anthropic.rs    # Anthropic Messages API
│   │   # Both implement the same trait; no other provider files needed
│   │
│   ├── agent/
│   │   ├── agent_loop.rs   # Core TurnState state machine (Restore → Compact → Command → Build → Run → Save → Respond → Done)
│   │   ├── skills.rs       # Load/save/delete skills from ~/.openz/skills/*.md
│   │   ├── activity.rs     # Global activity tracking (~/.openz/activity.json)
│   │   ├── security.rs     # SecurityGuard: intercepts destructive/privileged/network commands
│   │   ├── context_compactor.rs  # Tool output compression (Z-Context / Headroom port)
│   │   └── style/          # TUI: colors.rs, icons.rs, spinner.rs, menu.rs, tui_println! macro
│   │
│   ├── channels/           # Channel trait implementations
│   │   ├── mod.rs          # Channel trait, shutdown logic, fetch_provider_models()
│   │   ├── cli.rs          # TUI terminal with crossterm raw-mode, slash commands, clipboard paste
│   │   ├── websocket.rs    # Axum WS + static file WebUI server
│   │   ├── telegram.rs     # Polling loop + inline callback approval buttons
│   │   ├── discord.rs      # WebSocket gateway client
│   │   └── whatsapp.rs     # Axum webhook receiver for WhatsApp Business API
│   │
│   ├── tools/              # All native tools implement the Tool trait
│   │   ├── mod.rs          # Tool trait, ToolRegistry (static tools + dynamic subagent resolution)
│   │   ├── filesystem.rs   # read, write, list, find, patch, replace_lines
│   │   ├── shell.rs        # exec_command (sandboxed subprocess)
│   │   ├── web.rs          # web_fetch (DOM scraper with scraper crate)
│   │   ├── mcp.rs          # MCP stdio client, gRPC bridge, McpToolWrapper
│   │   ├── mcp_manager.rs  # manage_mcp tool (CRUD MCP server configs)
│   │   ├── subagent.rs     # delegate_task, OptimizeSubagentTool, Create/DeleteSubagentTool
│   │   ├── grep.rs         # grep_search
│   │   ├── ast_grep.rs     # AST structural search
│   │   ├── git_manager.rs  # git status/diff/commits
│   │   ├── cargo_manager.rs# cargo build/check/test
│   │   ├── outline.rs      # code_outline (structural file parsing)
│   │   ├── db_inspector.rs # SQLite reader + writer
│   │   ├── web_search.rs   # Tavily web search
│   │   ├── gsd_browser.rs  # GSD browser automation (Playwright wrapper)
│   │   ├── cron.rs         # Schedule/List/RemoveJob tools
│   │   ├── remote.rs       # send_remote_input (cross-channel prompt forwarding)
│   │   ├── clipboard.rs    # system clipboard get/set
│   │   ├── open.rs         # open files/URLs in default app
│   │   ├── watcher.rs      # file_watcher (background file change monitor)
│   │   ├── image_generator.rs  # GenerateImageTool (draw shapes/text to PNG)
│   │   ├── crawl.rs        # CrawlSiteTool (spider-rs multi-threaded crawler)
│   │   ├── obscura.rs      # ObscuraBrowserTool (headless browser via CDP)
│   │   ├── doc_reader.rs   # Read PDF, DOCX, XLSX files
│   │   ├── wasm_sandbox.rs # Execute WASM modules
│   │   ├── semantic_search.rs # FastEmbed vector search
│   │   ├── rust_docs.rs    # Query rustdoc documentation
│   │   ├── system_info.rs  # OS/hardware info
│   │   ├── network.rs      # check_port
│   │   └── onpkg.rs        # onpkg package/template manager
│   │
│   ├── cron/               # Scheduler loop (cron syntax + duration)
│   ├── sop/                # Stateful SOP workflow engine (persisted JSON instances)
│   └── subagents/          # SubagentProfile definitions (~/.openz/subagents.json)
│       └── mod.rs          # 15+ default profiles (planner, researcher, reviewer, etc.)
```

---

## Architecture & Data Flow

```
CLI → clap parse → channel (cli/ws/tg/dc/wa) → AgentLoop::run()
  │
  ├─ Restore: load session from ~/.openz/sessions/<key>.json
  ├─ Compact: if > max_messages (120), LLM-summarize old history + consolidate memory
  ├─ Command: check for /slash commands
  ├─ Build: construct system prompt (base + summary + memory + skills + activity + caveman mode)
  ├─ Run: LLM chat loop with tool execution (max 200 iterations)
  │   ├─ Auto-continuation: if finish_reason="length", re-prompt to continue (up to 3 retries)
  │   ├─ SecurityGuard: intercept sensitive exec_command/write_file calls → TUI or Telegram approval
  │   └─ Tool output >4000 chars → saved to ~/.openz/tool_outputs/ + compressed inline
  ├─ Save: persist session to disk
  ├─ Respond: return content to channel
  └─ Background: spawn self-improvement curator (memory + skills review)
```

### Key traits

- **`LLMProvider`** (`providers/mod.rs:104`): `chat(&self, system_prompt, messages, tools, settings) -> Result<LLMResponse>`. Two implementations: `OpenAIProvider` (also handles DeepSeek, Groq, Ollama, OpenRouter, etc.) and `AnthropicProvider`.
- **`Tool`** (`tools/mod.rs:9`): `name() -> &str`, `description() -> &str`, `parameters() -> Value`, `call(&self, args) -> Result<Value>`.
- **`Channel`** (`channels/mod.rs:3`): `name() -> &'static str`, `start() -> Result<()>`.

### TurnState machine (`agent/agent_loop.rs:13`)

```
Restore → Compact → Command → Build → Run → Save → Respond → Done
```

Each state is a branch in a `while state != TurnState::Done` loop inside `run_inner()`.

---

## Critical Gotchas & Non-Obvious Patterns

### Provider routing is keyword-based, not config-based
The `build_agent_loop` function in `cli.rs:270-337` uses **model name prefixes** to route providers, not the `provider` field. A model named `anthropic/claude-3-5-sonnet` routes to Anthropic, while `gpt-4o` routes to OpenAI. The `provider` field is only used when auto-detection fails. Subagent model selection follows the same pattern with `provider/` prefixes.

### 13 providers, only 2 provider files
All OpenAI-compatible providers (DeepSeek, Groq, Ollama, OpenRouter, MiniMax, Mistral, z.ai, NVIDIA, OpenCode Zen, Cerebras, Google AI Studio) share `OpenAIProvider` in `openai.rs`. Only Anthropic has a separate implementation in `anthropic.rs`.

### Auto-provider resolution is cascading
When `provider = "auto"`, the system checks model prefix keywords first, then performs a cascading key-availability check: if model contains "claude" but no Anthropic key → check OpenCode Zen → check OpenRouter → fall back to Anthropic anyway.

### Caveman mode is ON by default
`caveman_mode` defaults to `true` in `config/schema.rs:78-80`. This injects a terseness instruction into the system prompt that strips articles, filler, pleasantries. Turn it off in config if you want verbose responses.

### Tool argument naming is inconsistent
Tool call arguments have **no single naming convention**. Some tools use `serde` rename (e.g., `command_line` → `CommandLine`), others direct field names, `snake_case`, `camelCase`. The `format_tool_args` function in `agent_loop.rs:998` handles ~20 specific tool names with multiple alias support. When adding a new tool, check both conventions.

### Console output must use tui_println! macro
In the CLI channel, raw mode (crossterm) is active. Use the `tui_println!` macro from `agent/style/mod.rs` which translates `\n` to `\r\n`. Direct `println!` causes diagonal alignment issues.

### Sub-agent CancellationToken lifecycle
All sub-agent tools (`DelegateTaskTool`, `DelegateProfileTool`, `ParallelResearchTool`, `EvaluatorOptimizerLoopTool`) hold a `cancellation_token: CancellationToken` field. The token is defined in `src/tools/subagent.rs` using `Arc<AtomicBool>` + `Arc<tokio::sync::Notify>`. Each `call()` method wraps sub-agent `run()` with `tokio::select! { biased; _ = cancellation_token.wait_for_cancellation() => { error } }` so cancellation terminates sub-agents immediately rather than waiting for timeout. Nested sub-agents inherit the parent's token via `.clone()`. Top-level tools in `cli.rs` and `ToolRegistry::get()` create fresh tokens with `CancellationToken::new()`.

### seccomp BPF sandbox for exec_command
The `ExecCommandTool` in `src/tools/shell.rs` applies a BPF seccomp filter before executing shell commands. The sandbox runs inside `pre_exec` (after fork, before exec) and is Linux-only. Key components:

- **`apply_seccomp_guard()`** (`shell.rs:12`): Sets `PR_SET_NO_NEW_PRIVS` (blocks setuid in child), `PR_SET_PDEATHSIG` (SIGKILL if parent dies), resource limits (RLIMIT_CPU=30/60s, RLIMIT_AS=256/512MB, RLIMIT_FSIZE=10MB), then installs the BPF filter.
- **`allowlist_seccomp_filter()`** (`shell.rs:52`): Linear-scan BPF filter with architecture check (x86_64 + aarch64) and ~110 allowed syscalls covering basic I/O, filesystem, memory, process, and signal operations. Denies networking, module loading, ptrace, mount, and other dangerous syscalls.
- **Filter installation**: Uses `syscall(SYS_seccomp, SECCOMP_SET_MODE_FILTER, 0, &prog)` directly instead of `prctl(PR_SET_SECCOMP, ...)` to avoid a kernel quirk where prctl-based filter installation kills execve with SIGKILL on some x86_64 builds.
- **`sandbox_command()`** (`shell.rs:291`): Wraps the seccomp setup in `cmd.pre_exec()`. Errors are logged but non-fatal — a failed seccomp install still allows the command to run. Only applied if `enable_sandbox` is set to `true` in the configuration.
- **`enable_sandbox` Toggle**: Can be enabled/disabled via `openz configure` (or `enableSandbox` in `~/.openz/config.json`). When set to `false` (default), subprocesses bypass the seccomp filter to prevent blocking compiler and browser tools (e.g., `gsd_browser`, `chromewright`), while command execution safety is still verified by `SecurityGuard` permissions.
- **Fallback**: On non-Linux platforms, `sandbox_command()` is a no-op, so `exec_command` also works on macOS/Windows.

### Self-improvement runs async in background
After every non-slash-command turn, `tokio::spawn` runs a background curator that calls the LLM to review the conversation, update session memory, and create/update skills in `~/.openz/skills/`. This can cause race conditions if multiple channels hit the same session simultaneously — the curator reloads the session from disk to mitigate this.

### SecurityGuard intercepts before tool execution
The guard intercepts in `agent_loop.rs:635` inside the Run state, **after** the LLM has already consumed tokens to produce the tool call. There's no pre-flight check that prevents the LLM from generating the tool call. Denial just returns `{"error": "Execution denied by user."}` to the LLM.

### Tool output truncation is aggressive
Tool outputs >4000 characters are: (1) written to `~/.openz/tool_outputs/<name>_<uuid>.json`, (2) passed through the context compactor (Headroom port / Z-Context), and (3) only the compressed version + file reference is injected into the message list.

### Session consolidation ensures no orphaned messages
When truncating session history (Compact state at `agent_loop.rs:107-217`), the code scans backward from the truncation point to find the nearest "user" message. This prevents orphaned "tool" or "assistant" messages from causing API errors.

### MCP binary resolution uses AI_AGENT_TOOLS_BASE
`Config::default()` in `config/schema.rs` resolves MCP binary paths via `resolve_mcp_bin(binary, subproject)` with a three-stage priority:
1. **`{AI_AGENT_TOOLS_BASE}/{subproject}/target/release/{binary}`** — local project build (e.g. `sequentialthinking_rs/target/release/mcp-server-sequential-thinking`)
2. **`~/.cargo/bin/{binary}`** — cargo-installed binary
3. **bare `{binary}` name** — fall back to `$PATH`

`AI_AGENT_TOOLS_BASE` is the constant `"/home/aswin/programming/vscode/myProjects/ai_agent_tools"` defined at the top of `config/schema.rs`. Special cases: `sequential-thinking` → `sequentialthinking_rs` project, `memory` → `memory_rs` project (binary: `openmemory_rs`). All other servers (`chromewright`, `database-mcp`, `headroom-mcp`, `just-mcp`, `opendocswork-mcp`, `openz-docs-mcp`, `openz-github-mcp`, `sediment`, `spreadsheet-mcp`) are cargo-installed and resolve via stage 2.

### Subagents = tools at the LLM level
Custom subagent profiles from `~/.openz/subagents.json` are dynamically registered as tools in `ToolRegistry::to_openai_format()` (`tools/mod.rs:100-129`). When the LLM "calls" a subagent name as a tool, `ToolRegistry::get()` (`tools/mod.rs:63-82`) matches it and returns a `DelegateProfileTool`.

### gRPC proto lives inside the repo
`build.rs` references `proto/mcp.proto` relative to the project root. The proto file is included in the repo at `proto/mcp.proto`.

---

## Testing Patterns

- 28 test modules across the source tree
- Tests use `#[cfg(test)]` and `#[test]` (standard Rust)
- Tool tests typically construct inputs as `serde_json::json!({...})` and call the tool's `.call()` method
- Provider tests mock `reqwest` responses or test `model_supports_vision()` helper
- Security tests validate `is_sensitive()` against `exec_command` and `write_file` calls
- Skills tests create, verify, and clean up temporary skill files
- No integration tests or test harness beyond `cargo test`

---

## Config

Located at `~/.openz/config.json` (or `$OPENZ_CONFIG_DIR/config.json`). Structure (`config/schema.rs:225`):

| Section | Key fields |
|---|---|
| `providers` | 13 optional `{api_key, api_base, api_type}` configs |
| `agents.defaults` | model, provider, max_tokens: 4096, temperature: 0.1, max_messages: 120, max_tool_iterations: 200, caveman_mode: true |
| `channels` | websocket {port: 8765, host: 127.0.0.1}, telegram, discord, whatsapp |
| `mcp_servers` | Map of name → `{command, args, enabled}` |

Key resolution order: config.providers.X.api_key → `PROVIDER_API_KEY` env var. API keys from environment variables override config values at resolution time (`cli.rs:342-457`).

---

## Key Environment Variables

- `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `DEEPSEEK_API_KEY`, `GROQ_API_KEY`, `OPENROUTER_API_KEY`, `MINIMAX_API_KEY`, `MISTRAL_API_KEY`, `Z_AI_API_KEY`, `NVIDIA_API_KEY`, `OPENCODE_ZEN_API_KEY`, `CEREBRES_API_KEY`, `GOOGLE_AI_STUDIO_API_KEY`
- `TELEGRAM_BOT_TOKEN`, `DISCORD_BOT_TOKEN`, `WHATSAPP_API_KEY`, `WHATSAPP_PHONE_NUMBER_ID`
- `OPENZ_CONFIG_DIR` — override `~/.openz` config path
- `OPENZ_SILENT` — suppress MCP server setup logs (set automatically for background channels)
