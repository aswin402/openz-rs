# OpenZ 🦊 `v0.0.114`

OpenZ is a local-first AI agent runtime built in Rust. It combines an interactive terminal agent, native tools, durable memory, browser-backed research, document/media automation, background channels, and isolated subagents in one fast desktop-oriented binary.

Repository: <https://github.com/aswin402/openz-rs>

OpenZ was rebranded from `nanobot` and is maintained by Aswin.

---

## Why OpenZ

Most agent frameworks require users to know the right tool, backend, or workflow switch. OpenZ is designed to make those choices automatically:

- If native web search is blocked, OpenZ falls back to local browser discovery.
- If a fetched page is only a JavaScript shell, OpenZ retries with browser rendering.
- If a generated file should be shown, OpenZ opens it automatically.
- If opening fails, OpenZ asks the local device inventory for viewer/app suggestions.
- If a document has no extractable text, OpenZ attempts OCR automatically.
- If an edit starts, OpenZ scopes project context before modifying files.
- If a saved tool output is requested, OpenZ retrieves the original instead of re-truncating it.

The goal is simple: users describe outcomes; OpenZ handles tool orchestration.

---

## Core Capabilities

| Area | What OpenZ Provides |
|---|---|
| Agent runtime | Terminal TUI, tool-calling loop, streaming/non-streaming provider support, cancellation, session persistence |
| Web and research | SearchXyz native search, browser fallback, direct URL fetch, cache revalidation, source ledger, research briefs |
| Browser automation | GSD browser, Firefox/WebDriver, Obscura/CDP rendering, browser preflight diagnostics |
| Memory | Cognitive memory, semantic recall, working memory, knowledge graph entities/relations, shared team memory |
| Code tools | AST grep, code outline, grep, Rust docs lookup, cargo manager, compiler auto-heal |
| Filesystem | Read, write, patch, replace lines, find files, list directories, transactional Zenflow edits |
| Documents | PDF/DOCX/XLSX reading, OCR fallback, document complexity analysis, OpenDoc integrations |
| Media | Image, SVG, animation, and video generation/conversion pipelines through OpenMedia tools |
| Subagents | Researcher/reviewer/developer-style delegation with isolated project workspaces or scratch workspaces |
| Channels | CLI/TUI, WebSocket gateway/WebUI, Telegram, Discord, WhatsApp, Email |
| Automation | SOP workflows, cron jobs, background self-improvement, device inventory, secret scrub doctor |
| Extensibility | MCP client support, MCP bridge, local tool registry, dynamic subagent tools |

---

## What Changed In `v0.0.114`

`v0.0.114` focuses on reliability, privacy, and automatic tool routing.

### Privacy and trace handling

- Reasoning/trace output is private by default.
- `/tui trace full|compact|off` controls trace visibility.
- Long tool outputs are stored as exact raw files and can be restored with `retrieve_original`.
- Saved `~/.openz/tool_outputs/` reads are automatically rewritten to `retrieve_original`.

### Web and research reliability

- `web_search` now uses a local-first cascade: native SearchXyz, native rescue, then provider-free browser discovery.
- Search failures classify blocked, rate-limited, timeout, and no-result cases.
- Search diagnostics are appended automatically unless `diagnose_on_failure=false` is set.
- Research-style browser searches automatically read top discovered pages.
- `web_fetch` retries JavaScript shell pages through GSD → Firefox → Obscura rendering.
- Fresh/current/latest requests automatically force `cache_mode=revalidate` unless explicitly overridden.
- `searchxyz_read_github_repo` auto-expands small `max_files` mismatches.

### Documents and artifacts

- `read_doc` automatically OCRs scanned PDFs and supported image files when native extraction is empty.
- PDF complexity analysis runs by default before extraction/OCR.
- Generated local artifacts auto-open when the user asks to show/open/view/play/display the result.
- Failed `open_path` calls automatically query `device_inventory suggest` for matching local viewers/apps.

### Subagents and safety

- Subagents launched from unsafe non-project directories such as `/home/aswin` now use scratch workspaces under `~/.openz/worktrees` instead of writing directly in the active workspace.
- Git project workspaces still use isolated worktrees with normal sync-back behavior.
- `parallel_research` returns completed work as `partial_success` when another branch times out.
- Marketplace prompts now clarify buyer-side vs seller-side agent marketplaces only when genuinely ambiguous.

### Editing and project context

- First file edit per target automatically runs `scope_context` so project instructions are loaded before modification.
- CLI cancellation handling is more robust for Esc, Ctrl+C, Ctrl+D, ETX, and EOT input paths.
- `openz doctor --scrub-secrets` can scrub historical secrets from sessions, traces, tool outputs, and runtime data.

---

## Installation

### Build from source

```bash
git clone https://github.com/aswin402/openz-rs.git
cd openz-rs
cargo build --release
```

The release binary is written to:

```bash
target/release/openz
```

### Install locally

```bash
cargo install --path .
```

If the Cargo target directory grows too large, use the repository helper:

```bash
./localinstall.sh --clean-target
```

### Optional OCR support

OCR support requires the `ocr` feature and system OCR dependencies such as Tesseract/Poppler.

```bash
cargo build --release --features ocr
```

---

## Quick Start

```bash
openz onboard
openz configure
openz agent
```

Recommended launch pattern for project work:

```bash
cd /path/to/your/project
openz agent
```

If OpenZ is launched from a home/root/runtime directory, subagents automatically use a scratch workspace instead of writing into that unsafe directory.

---

## Common Commands

| Command | Purpose |
|---|---|
| `openz agent` | Start the interactive terminal agent |
| `openz configure` | Configure providers, channels, gateway, and defaults |
| `openz onboard` | First-time provider setup wizard |
| `openz gateway` | Start WebSocket gateway and WebUI |
| `openz telegram` | Start Telegram polling channel |
| `openz discord` | Start Discord gateway channel |
| `openz whatsapp` | Start WhatsApp webhook receiver |
| `openz subagent` | Manage subagent profiles |
| `openz sop list` | List SOP workflow definitions |
| `openz sop trigger <id>` | Trigger a SOP workflow |
| `openz logs` | View structured runtime logs |
| `openz doctor` | Inspect runtime health and storage layout |
| `openz doctor --scrub-secrets` | Redact historical leaked secrets in runtime files |
| `openz changelog` | View release history and system specifications |

---

## Configuration

OpenZ stores runtime configuration under:

```text
~/.openz/config.json
```

Override the config/runtime directory with:

```bash
export OPENZ_CONFIG_DIR=/path/to/openz-runtime
```

Common provider/channel environment variables:

```bash
OPENAI_API_KEY
ANTHROPIC_API_KEY
OPENROUTER_API_KEY
GROQ_API_KEY
MISTRAL_API_KEY
GOOGLE_AI_STUDIO_API_KEY
TELEGRAM_BOT_TOKEN
DISCORD_BOT_TOKEN
WHATSAPP_API_KEY
```

Environment keys override config-file keys at provider resolution time.

---

## Web Search Model

SearchXyz is OpenZ's local research/search subsystem. It is not a full independent global search index like Google. It combines:

- local indexed sources and research briefs,
- direct URL reading,
- scraper-backed native search,
- browser-backed discovery,
- optional external/private backends such as SearXNG or Brave.

OpenZ does not require Brave or SearXNG for normal operation. They improve live discovery reliability, but `v0.0.114` adds a provider-free browser fallback so OpenZ can continue when scraper backends are blocked.

---

## Architecture

```text
User channel
  └─ AgentLoop
      ├─ restore session
      ├─ compact long context
      ├─ apply slash commands
      ├─ build prompt from config, memory, skills, activity
      ├─ execute model/tool loop
      │   ├─ SecurityGuard approvals
      │   ├─ automatic tool routing and fallbacks
      │   ├─ tool output compression and retrieval refs
      │   └─ source ledger / research capture
      ├─ save transcript
      └─ return response
```

Main implementation areas:

| Path | Responsibility |
|---|---|
| `src/agent/` | Agent loop, security, prompt construction, context compaction, events |
| `src/channels/` | CLI, WebSocket, Telegram, Discord, WhatsApp, Email |
| `src/tools/` | Native tool implementations |
| `src/tools/searchxyz/` | Local/native/browser-backed search and research indexing |
| `src/tools/subagent/` | Delegation, isolated workspaces, parallel research |
| `src/config/` | Config schema, loading, runtime path resolution |
| `src/sop/` | Stateful workflow engine |
| `tools/openmedia/` | Media generation pipeline crates |
| `tools/opendoc/` | Document/OCR integration crate |

---

## Safety Model

OpenZ is designed for local-first operation, but it can still perform powerful actions. Safety controls include:

- approval gates for high-risk tools,
- destructive command detection,
- optional Linux seccomp sandboxing for subprocesses,
- runtime path separation under `~/.openz`,
- subagent workspace isolation,
- secret redaction in config/log views,
- historical secret scrubbing via doctor,
- explicit network/resource policy metadata for tools.

No safety layer replaces user review. Treat shell execution, file writes, and external channel actions as privileged operations.

---

## Development

```bash
cargo check
cargo test
cargo test <test_name>
cargo clippy
cargo fmt
```

Useful targeted checks:

```bash
cargo test version_sync_tests::release_version_surfaces_match_cargo_package_version --lib
cargo test cli::tools::tests::register_all_tools_includes_expected_domains_without_duplicates --lib
cargo test agent::agent_loop::run::auto_tool_arg_tests:: --lib -- --test-threads=1
cargo test tools::web_search::tests:: --lib -- --test-threads=1
```

There is no Makefile and no GitHub Actions CI config currently checked in.

---

## Project Status

OpenZ is under active development. The current focus is reducing explicit tool-name prompting by moving common behavior into automatic routing, fallback, and recovery paths.

Current priorities:

1. Make provider-free web discovery more reliable without requiring Brave or SearXNG.
2. Continue converting manual tool instructions into automatic agent-loop behavior.
3. Keep trace/reasoning private by default across all public channels.
4. Improve browser lifecycle recovery and diagnostics.
5. Keep memory, source ledger, and research capture precise enough to avoid stale answers.

---

## License

See the repository license files and Cargo metadata for the current license terms.
