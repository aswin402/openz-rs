# OpenZ 🦊 `v0.0.53`

<p align="center">
  <img src="assets/logo.png" width="200" alt="OpenZ Logo">
</p>

OpenZ is a high-performance personal AI agent framework built in Rust by **Aswin**. It combines an interactive terminal agent, background channels, native tools, memory, SearchXyz research, Headroom compression, OpenMedia generation, OpenDoc document automation, dynamic subagents, and MCP integration in one local-first binary.

**Repository:** [github.com/aswin402/openz-rs](https://github.com/aswin402/openz-rs)

OpenZ was rebranded from `nanobot` and is inspired by Zeroclaw, Nanobot, hermes-agent, loops!, DOX, Headroom, OpenMemory, SearchXyz-style research systems, OpenMedia, OpenDoc, and Rust-native MCP tooling.

---

## What Changed In `v0.0.53`

- **TUI formatting fix:** preserved markdown/newlines while stripping provider `<think>...</think>` leaks, restoring readable multi-line answers in the terminal.
- **Canonical session command:** replaced duplicate `/new` command with `/new-session` across TUI and Telegram command menus.
- **Selectable session resume:** Telegram `/resume` now shows inline buttons for previous sessions plus a `Continue current session` option. TUI uses `/history` as the single interactive session restore command.
- **Model reliability hardening:** risky/unknown/free/small models are warned about, smoke-tested in the background, and tracked in `~/.openz/model_registry.json`.
- **Weak-model context hardening:** pinned identity/persona memory and recent-session context are injected so small models behave better on identity and “what were we discussing” questions.
- **Knowledge and workflow memory:** saved source bookmarks, research briefs, and reusable workflow cards help repeated research/tasks reuse known links, paths, repos, and successful procedures. Inspect them with `/sources <query>` and `/workflows <query>`.

---

## Core Capabilities

### Agent Runtime

- TUI chat loop with slash commands, raw-mode-safe rendering, session persistence, streaming support, and automatic continuation when a provider stops because of output length.
- Multi-channel operation through terminal, WebSocket/WebUI gateway, Telegram, Discord, WhatsApp, and Email.
- OpenAI-compatible local gateway endpoint through `openz gateway`.
- Background self-improvement that can update memories and skills from completed conversations.
- Session integrity via hash-chain verification and the `/audit` command.

### Tools And Automation

OpenZ registers native tools directly in Rust. The major tool families are:

- **Files, shell, code, and git:** read/write/patch/list/find files, line replacement, grep, AST search, code outline, git operations, cargo operations, DB inspection/write, Rust docs, template compilation, WASM and Python sandbox execution.
- **Research and web:** web fetch/search, SearchXyz web/research/cache/graph tools, GitHub repo ingestion, site maps, crawlers, social search, browser automation via GSD/Obscura/Firefox, and vector semantic search.
- **Memory:** cognitive memory, graph memory, working memory, episodic reflections, shared memory, semantic facts, hybrid FTS/vector search, conflict handling, fact extraction, stale/deletion handling, code graph indexing, and memory stats.
- **Headroom compression:** content/file/directory/diff/schema compression, signature-only code compression, CCR cache, FTS cache search, cache import/export, token estimates, stats, usage analytics, and bounded run-and-compress.
- **Subagents:** `delegate_task`, dynamic subagent profiles, `parallel_research`, evaluator/optimizer loops, subagent creation/deletion/optimization, cancellation propagation, and bounded dynamic timeouts.
- **OpenMedia:** SVG/image/chart/icon/video generation, video templates, animated SVG timelines, Lottie conversion, filters, resize/crop/convert/batch processing, quality scoring, and prompt refinement.
- **OpenDoc:** read/search/convert documents, DOCX/PPTX/XLSX/PDF creation and editing, PDF splitting/merging/forms/tables, OCR checks, and archive digests.
- **MCP:** CRUD MCP server configuration, stdio client support, and a gRPC-to-stdio bridge for MCP transport.

### Memory System

`v0.0.50` and `v0.0.51` made memory a first-class OpenZ subsystem:

- `MemoryCoordinator` coordinates semantic, graph, recall, deletion, and stats paths.
- Hybrid search combines FTS5 and deterministic vector embeddings with reciprocal-rank fusion.
- `forget_memory` purges or tombstones across semantic metadata, FTS rows, graph, shared/cognitive memory, research, sessions, and skills-derived facts.
- Prompt memory is query-aware, stale-fact aware, deduplicated, and top-30 budgeted.
- Fact extraction supports multi-word entities, profile facts, chained clauses, `built_with`, `lives_in`, and `prefers` relations.
- Regression coverage includes stale facts, contradictions, deletion, recall relevance, poisoning attempts, prompt budgeting, embeddings, and codebase indexing.

### Safety And Resource Controls

- SecurityGuard intercepts destructive shell commands, privilege escalation, process control, network transfer commands, and risky file writes before execution.
- Optional Linux seccomp BPF sandboxing is available for subprocesses when enabled in config.
- High-risk tools use resource policy checks and approval gates.
- Long-running tools use recommended timeouts and bounded overrides instead of hardcoded 120s limits.
- SearchXyz destructive operations require explicit confirmation, and web/repo ingestion supports output and repository limits.
- Install/update scripts back up global OpenZ data, relocate stray runtime DB files, repair corrupt Cargo registry source unpacks, warn on huge build caches, and can clean `target/` explicitly.

---

## Runtime Commands

| Command | Purpose |
|---|---|
| `openz onboard` | First-time provider setup wizard. |
| `openz configure` | Configure providers, channels, gateway, sandbox, and preferences. |
| `openz agent` | Start the terminal TUI agent. |
| `openz gateway` | Start WebSocket/WebUI gateway and local API endpoint. |
| `openz telegram` | Start Telegram bot listener. |
| `openz discord` | Start Discord gateway listener. |
| `openz whatsapp` | Start WhatsApp webhook receiver. |
| `openz email` | Start Email IMAP/SMTP client. |
| `openz subagent` | Manage subagent profiles. |
| `openz sop list|instances|trigger|resume` | Manage SOP workflow instances. |
| `openz mcp-bridge --port <N> -- <cmd> [args...]` | Bridge gRPC/TCP to stdio MCP. |
| `openz logs [--tail N] [--session S] [--level L]` | View structured logs. |
| `openz changelog` | Print footprint specs and release history. |
| `openz streaming` | Toggle response streaming. |
| `openz doctor` | Check runtime DB placement, archive stale graph branches, and report disk/cache pressure. |

---

## Install And Update

### Install

```bash
./localinstall.sh
```

### Update

```bash
./localupdate.sh
```

### Balanced and low-resource builds

Recommended for most laptops when normal update lags the machine:

```bash
./localupdate.sh --balanced
```

Balanced mode caps Cargo to 2 jobs by default, uses the `release-balanced` profile, skips the duplicate pre-install `cargo check`, and avoids ThinLTO linker spikes. It is usually much faster than `--low-resource` while using less RAM/CPU than the full release path.

For a little more speed on stronger machines:

```bash
OPENZ_BUILD_JOBS=3 ./localupdate.sh --balanced
```

Use minimum-resource mode only when the machine is still lagging or swapping:

```bash
./localupdate.sh --low-resource
```

### Reclaim Cargo build-cache space

Repeated local builds and tests can make `target/` very large. Normal install/update runs warn when `target/` is over 20 GiB. To remove rebuildable Cargo artifacts before compiling:

```bash
./localupdate.sh --clean-target
```

or:

```bash
./localinstall.sh --clean-target
```

This is equivalent to `cargo clean` before the build. It does not delete `~/.openz` runtime data, sessions, memories, or config.

---

## Configuration And Data Locations

- Main config: `~/.openz/config.json` or `$OPENZ_CONFIG_DIR/config.json`
- Sessions: `~/.openz/sessions/`
- Memory databases: `~/.openz/memory.db`, `~/.openz/graph_memory.db`, and related SQLite files
- Tool outputs: `~/.openz/tool_outputs/`
- Traces/logs: `~/.openz/traces/`, `~/.openz/openz.log`
- Subagents: `~/.openz/subagents.json`
- Skills: `~/.openz/skills/` plus SQLite-backed skill/memory storage

Key provider environment variables include `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, `DEEPSEEK_API_KEY`, `GROQ_API_KEY`, `OPENROUTER_API_KEY`, `MISTRAL_API_KEY`, `OPENCODE_ZEN_API_KEY`, `GOOGLE_AI_STUDIO_API_KEY`, `TELEGRAM_BOT_TOKEN`, `DISCORD_BOT_TOKEN`, and WhatsApp credentials.

---

## Performance And Footprint

These values are intentionally measured/qualified rather than hard-coded marketing claims:

| Area | Current OpenZ Behavior |
|---|---|
| Binary / install size | Install/update scripts print the exact installed binary size. A recent measured dev install was about 124 MB; exact size depends on compiled heavy stacks such as ONNX embeddings, browser/media tooling, and document processing. |
| Idle RAM | About 15-30 MB in cloud/API mode when local embedding models are not loaded. |
| Active RAM | About 30-80 MB typical; 200 MB+ when local ONNX embeddings are loaded. |
| CPU | Near 0% while idle; Tokio async runtime does work only when active. |
| Startup | Core CLI paths are millisecond-scale; full TUI startup depends on config, DB checks, enabled channels, MCP/tool setup, and provider checks. |
| Build cache | Cargo `target/` can grow into tens of GiB during development. Use `--clean-target` when disk pressure appears. |

---

## Development Commands

| Command | Purpose |
|---|---|
| `cargo check` | Fast type check. |
| `cargo test --lib -- --test-threads=1` | Run the library test suite deterministically. |
| `cargo fmt --check` | Verify Rust formatting. |
| `cargo clippy` | Lint. |
| `cargo build --release` | Build optimized release binary. |
| `cargo clean` | Remove rebuildable Cargo artifacts under `target/`. |

The project has no Makefile. The local install/update scripts wrap the common build/install flow and add OpenZ-specific checks.

---

## Project Map

```text
openz/
├── Cargo.toml              # Rust package metadata and dependencies
├── build.rs                # tonic-build proto compilation
├── CHANGELOG.md            # release history and specs
├── localinstall.sh         # global install helper
├── localupdate.sh          # update helper with backup, checks, and install
├── src/
│   ├── main.rs             # dotenv init, Tokio runtime, CLI dispatch
│   ├── cli/                # clap commands, configure UI, logs, doctor, tool registration
│   ├── config/             # schema, provider defaults, loader, migrations
│   ├── providers/          # OpenAI-compatible and Anthropic provider clients
│   ├── agent/              # agent loop, prompt build, security, skills, TUI style
│   ├── channels/           # CLI, WebSocket, Telegram, Discord, WhatsApp, Email
│   ├── tools/              # native tool implementations
│   ├── cron/               # scheduler
│   ├── sop/                # stateful SOP workflow engine
│   └── subagents/          # profile definitions and manager
├── docs/                   # architecture and subsystem docs
└── assets/                 # logo and bundled assets
```

---

## Documentation

- [Architecture](docs/architecture.md)
- [Security Guard & Permissions](docs/security.md)
- [Channels & Configuration](docs/channels.md)
- [Self-Improvement & Memory](docs/self_improvement.md)
- [Model Context Protocol](docs/mcps.md)
- [Tools Registry](docs/tools.md)
- [ZeroClaw Gap Analysis & Roadmap](docs/zeroclaw_research.md)
- [Changelog](CHANGELOG.md)

---

## Notes For Operators

- Use `openz doctor` if runtime DB files appear in a project directory or disk usage looks wrong. It preserves data, relocates stray artifacts under `~/.openz`, and reports oversized caches such as `target/`, `~/.openz`, SearchXyz, and Cargo cache.
- Use `openz logs --tail 100` when debugging channel or provider issues.
- Use `openz changelog` to see the version shipped in the installed binary and current measured binary size.
- Use `./localupdate.sh --clean-target` after heavy development sessions if disk space drops unexpectedly.
- Keep secrets in environment variables or `~/.openz/config.json`; do not commit local config, sessions, or runtime DBs.
