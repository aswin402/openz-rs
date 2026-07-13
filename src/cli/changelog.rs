use crate::agent::style::*;
use crate::println;
use anyhow::Result;

pub async fn handle_changelog() -> Result<()> {
    println!(
        "{purple}=== OpenZ System Specifications & Changelog ==={reset}\n",
        purple = AURA_PURPLE,
        reset = COLOR_RESET
    );

    println!(
        "{bold}📊 Hardware Footprint & Specifications:{reset}",
        bold = COLOR_BOLD,
        reset = COLOR_RESET
    );
    println!(
        "  {blue}• ROM (Binary Size):{reset}   ~10 MB - 15 MB (optimized Rust binary)",
        blue = AURA_BLUE,
        reset = COLOR_RESET
    );
    println!("  {blue}• RAM (Cloud Mode):{reset}    ~15 MB - 30 MB (remote vector embeddings & LLM APIs)", blue = AURA_BLUE, reset = COLOR_RESET);
    println!(
        "  {blue}• RAM (Local Mode):{reset}    ~200 MB+ (local ONNX embedding model loaded)",
        blue = AURA_BLUE,
        reset = COLOR_RESET
    );
    println!("  {blue}• CPU Footprint:{reset}       0% when idle (Tokio async event-driven architecture)", blue = AURA_BLUE, reset = COLOR_RESET);
    println!(
        "  {blue}• Startup Speed:{reset}       < 5 ms boot-to-prompt speed",
        blue = AURA_BLUE,
        reset = COLOR_RESET
    );
    println!("  {blue}• Inspired By:{reset}         hermes-agent, Zeroclaw, Nanobot, loops!, DOX, codegraph, tantivy, lancedb,", blue = AURA_BLUE, reset = COLOR_RESET);
    println!("                         surrealdb, petgraph, sentrux, tree-sitter-graph, mistral.rs, agentgateway,");
    println!("                         cowork-forge, openhuman, mcp-rust-sdk, wasserstein-agents, gsd-browser,");
    println!(
        "                         chromewright, sediment, ClawDB, ferres-db, native-devtools-mcp,"
    );
    println!("                         tokio-cron-scheduler, grpc-rust, mcp-searxng, searxng-mcp, opendocswork-mcp,");
    println!("                         slack-mcp-server, task-master, langgraph, crawl4ai, websurfx, headroom,");
    println!("                         rust-mcp-filesystem, novada-mcp, obscura, crawlee, katana, librefang,");
    println!("                         openmetadata, youtube-transcript-api, semble, deep-research, ocrs,");
    println!("                         agent-skills, superpowers, OpenMemory, SkillSpector, OpenHands, deer-flow,");
    println!("                         multica, ast-grep, caveman, graphify, notify, mcp-everything, mcp-memory,");
    println!(
        "                         mcp-sequentialthinking, mcp-git, mcp-fetch, mcp-time, openfang\n"
    );

    println!(
        "{bold}⚡ Key Capabilities & Subsystems:{reset}",
        bold = COLOR_BOLD,
        reset = COLOR_RESET
    );
    println!(
        "  {gold}1. Memory & Skill Self-Improvement Curator{reset}",
        gold = AURA_GOLD,
        reset = COLOR_RESET
    );
    println!(
        "     Asynchronously analyzes conversations to extract Tier 1 memory facts and Tier 2"
    );
    println!("     procedural skills (stored in a SQLite database). Throttled to avoid wasteful LLM calls");
    println!("     on simple turns and limit stale skill clean-ups to once every 24 hours.");
    println!(
        "  {gold}2. Native Compiler Auto-Healing{reset}",
        gold = AURA_GOLD,
        reset = COLOR_RESET
    );
    println!(
        "     `compiler_auto_heal` tool compiles code natively, reads stderr compiler errors,"
    );
    println!(
        "     and prompts the LLM to fix syntax or borrow checker issues in a loop until green."
    );
    println!(
        "  {gold}3. Stateful SOP Workflow Engine{reset}",
        gold = AURA_GOLD,
        reset = COLOR_RESET
    );
    println!("     Executes multi-step Directed Acyclic Graph (DAG) procedures like `ship-pr-until-green`.");
    println!(
        "  {gold}4. Pluggable Channel Adapters{reset}",
        gold = AURA_GOLD,
        reset = COLOR_RESET
    );
    println!("     Operates concurrently via Console TUI, WebSocket, Telegram, Discord, WhatsApp, and Email.");
    println!(
        "  {gold}5. Security Guard & Subprocess BPF Sandbox{reset}",
        gold = AURA_GOLD,
        reset = COLOR_RESET
    );
    println!("     Intercepts destructive commands and sandboxes subprocesses using seccomp BPF filters.");
    println!(
        "  {gold}6. Startup Resource Clean-up{reset}",
        gold = AURA_GOLD,
        reset = COLOR_RESET
    );
    println!("     Auto-prunes stale git worktrees and temporary workspaces to keep disk ROM footprint low.\n");

    println!(
        "{bold}🔌 Model Context Protocol (MCP) Integration:{reset}",
        bold = COLOR_BOLD,
        reset = COLOR_RESET
    );
    println!("  OpenZ integrates with MCP servers using Stdio JSON-RPC or an in-process gRPC Tonic bridge.");
    println!("  {blue}• office:{reset}            Extracts text structures/tables from `.docx`, `.xlsx`, and `.pptx`.", blue = AURA_BLUE, reset = COLOR_RESET);
    println!(
        "  {blue}• spreadsheet:{reset}       Reads/writes Excel files via Apache POI.",
        blue = AURA_BLUE,
        reset = COLOR_RESET
    );
    println!(
        "  {blue}• just:{reset}              Runs Justfile task definitions.",
        blue = AURA_BLUE,
        reset = COLOR_RESET
    );
    println!(
        "  {blue}• docs:{reset}              Queries OpenZ documentation.",
        blue = AURA_BLUE,
        reset = COLOR_RESET
    );
    println!(
        "  {blue}• github:{reset}            GitHub integration (PRs, issues, code search).",
        blue = AURA_BLUE,
        reset = COLOR_RESET
    );
    println!(
        "  {blue}• database:{reset}          SQLite knowledge graph database.",
        blue = AURA_BLUE,
        reset = COLOR_RESET
    );
    println!(
        "  {blue}• browser:{reset}           Playwright-based browser automation.",
        blue = AURA_BLUE,
        reset = COLOR_RESET
    );
    println!(
        "  {blue}• sediment:{reset}          Codebase search and indexing.",
        blue = AURA_BLUE,
        reset = COLOR_RESET
    );

    println!(
        "{bold}🔧 Core Native Tools & Usages:{reset}",
        bold = COLOR_BOLD,
        reset = COLOR_RESET
    );
    println!("  {gold}• Sequential Thinking:{reset}  `sequentialthinking` (plan & reason), `analyze_graph` (graph analysis), `export_session` (thought serialization)", gold = AURA_GOLD, reset = COLOR_RESET);
    println!("  {gold}• Knowledge Graph Memory:{reset} `create_entities`, `read_graph`, `search_nodes`, `add_observations`, `create_relations`", gold = AURA_GOLD, reset = COLOR_RESET);
    println!("  {gold}• Context Compression (Headroom):{reset} `scope_context` (compile AGENTS.md rules), `compress_content` (register CCR), `retrieve_original` (get full content)", gold = AURA_GOLD, reset = COLOR_RESET);
    println!("  {gold}• Memory Extra:{reset}         `set_working_memory`, `smart_store`, `extract_and_store_facts`, `query_fact_history`", gold = AURA_GOLD, reset = COLOR_RESET);
    println!("  {gold}• Filesystem:{reset}         `read_file`, `write_file`, `patch_file`, `list_dir`, `grep_search`, `code_outline`", gold = AURA_GOLD, reset = COLOR_RESET);
    println!("  {gold}• Browsing & Web:{reset}     `web_search` (Tavily), `web_fetch`, `crawl_website` (spider-rs), `gsd_browser` (Playwright)", gold = AURA_GOLD, reset = COLOR_RESET);
    println!("  {gold}• Graphics & Video:{reset}   `generate_mermaid` (SVG renderer), `generate_video` (wavyte), `image_generator` (PNG)", gold = AURA_GOLD, reset = COLOR_RESET);
    println!("  {gold}• Task & Automation:{reset}  `delegate_task` (isolated subagent), `trigger_sop` (workflow engine), `schedule_job` (cron)", gold = AURA_GOLD, reset = COLOR_RESET);
    println!("  {gold}• Shell & Code:{reset}       `exec_command` (sandboxed), `wasm_sandbox` (wasmtime), `cargo_manager`, `js_format`\n", gold = AURA_GOLD, reset = COLOR_RESET);

    println!(
        "{bold}📅 Version Release History:{reset}",
        bold = COLOR_BOLD,
        reset = COLOR_RESET
    );

    println!(
        "  {green}[v{}] - Current Release{reset}",
        env!("CARGO_PKG_VERSION"),
        green = AURA_GREEN,
        reset = COLOR_RESET
    );
    println!(
        "    • Hardened SearchXyz with max_chars output budgets and truncation metadata."
    );
    println!(
        "    • Added GitHub repo ingest limits: max_files, max_total_bytes, git_timeout_secs."
    );
    println!("    • Persisted SearchXyz graph/cache updates and required confirm=true for destructive tools.");

    println!(
        "  {slate}[v0.0.46]{reset}",
        slate = AURA_SLATE,
        reset = COLOR_RESET
    );
    println!(
        "    • Hardened openmedia_create_svg with examples, aliases, and output_path copy support."
    );
    println!(
        "    • Added OpenMedia SVG line/text alignment attributes for cleaner logo generation."
    );
    println!("    • Added OpenMedia SVG-specific self-healing hints for schema mistakes.");

    println!(
        "  {slate}[v0.0.40]{reset}",
        slate = AURA_SLATE,
        reset = COLOR_RESET
    );
    println!("    • Fixed subagent model routing, fallback isolation, and OpenRouter free-model handling.");
    println!(
        "    • Improved CLI cancellation for long-running LLM, tool, and delegated subagent turns."
    );
    println!(
        "    • Split agent loop and tool registration internals into smaller maintainable units."
    );

    println!(
        "  {slate}[v0.0.39]{reset}",
        slate = AURA_SLATE,
        reset = COLOR_RESET
    );
    println!("    • Reserved dynamic subagent tools inside provider API tool limits.");
    println!("    • Prevented profile subagents from inheriting unrelated global fallback models.");

    println!(
        "  {slate}[v0.0.14]{reset}",
        slate = AURA_SLATE,
        reset = COLOR_RESET
    );
    println!("    • Implemented incremental session saving to disk to prevent data loss on early command cancellation.");
    println!("    • Added print_session_history to render previous messages and tool runs when starting/switching sessions.");

    println!(
        "  {slate}[v0.0.13]{reset}",
        slate = AURA_SLATE,
        reset = COLOR_RESET
    );
    println!("    • Configured separate tracing-subscriber layers to prevent ANSI escape code log pollution.");
    println!("    • Aligned default log path resolution with OPENZ_CONFIG_DIR customization.");
    println!(
        "    • Changed logs tail default value to 0 to only show real-time stream logs by default."
    );
    println!("    • Corrected double caret typo in context compactor backtrace regex.");

    println!(
        "  {slate}[v0.0.12]{reset}",
        slate = AURA_SLATE,
        reset = COLOR_RESET
    );
    println!("    • Made the OpenZ agent system prompt aware of its creator (Aswin), inspirations, specifications, features, and `changelog` command.");
    println!("    • Updated README.md documentation for the `changelog` command.");
    println!("    • Staged and committed all outstanding code changes and version bump to GitHub.");

    println!(
        "  {slate}[v0.0.11]{reset}",
        slate = AURA_SLATE,
        reset = COLOR_RESET
    );
    println!("    • Added `openz changelog` command and root `CHANGELOG.md` file.");
    println!(
        "    • Implemented Curator and Archival Throttling (reducing context & API token usage)."
    );
    println!("    • Added Cloud-First Embeddings with remote prioritize and a `cloud_only` low-RAM mode.");
    println!("    • Added native compiler auto-healing (`CompilerAutoHealTool`).");
    println!("    • Added automatic workspace clean-up to purge stale git worktrees on boot.");
    println!("    • Added `--low-resource` flag to build/update scripts to throttle memory & CPU.");
    println!("    • Configured Cargo.toml release profile (codegen-units, LTO, stripping) to natively limit compilation RAM.");

    println!(
        "  {slate}[v0.0.10]{reset}",
        slate = AURA_SLATE,
        reset = COLOR_RESET
    );
    println!("    • SQLite backend database migration (`~/.openz/memory.db`).");
    println!("    • Structural repository semantic indexing using `ast_grep` & vector embeddings.");
    println!("    • Added `mermaid_designer` subagent to generate SVG flowcharts.");

    println!(
        "  {slate}[v0.0.9]{reset}",
        slate = AURA_SLATE,
        reset = COLOR_RESET
    );
    println!("    • Cryptographic Merkle Hash-Chain ledger (`/audit` command).");
    println!("    • WhatsApp Axum webhook receiver channel adapter.");
    println!("    • Dynamic assistant auto-continuation for response truncation.");

    println!("\n{slate}For the full changelog details, please refer to: {reset}{bold}CHANGELOG.md{reset}\n", slate = AURA_SLATE, reset = COLOR_RESET, bold = COLOR_BOLD);

    Ok(())
}
