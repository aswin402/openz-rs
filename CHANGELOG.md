# OpenZ Changelog & System Specifications 🦊⚡

Welcome to OpenZ! This document provides an official record of the framework's architecture, hardware footprint, system capabilities, Model Context Protocol (MCP) integrations, native tools, and version releases.

---

## 📊 System Specifications & Hardware Footprint

| Category | Specification | Detail |
| :--- | :--- | :--- |
| **ROM (Binary Size)** | **~10 MB - 15 MB** | Statically compiled release binary, optimized for fast deployment. |
| **RAM Footprint (Cloud)** | **~15 MB - 30 MB** | Memory consumption when using Cloud-First Embeddings and remote LLM APIs. |
| **RAM Footprint (Local)** | **~200 MB+** | Memory consumption when loading local ONNX vector embeddings (`AllMiniLML6V2`) into RAM. |
| **CPU Utilization** | **0% Idle CPU** | Event-driven architecture using Tokio thread pools ensures zero CPU waste when inactive. |
| **Startup Speed** | **< 5 ms** | Boot-to-prompt initialization speed (excluding network API latency). |
| **Inspired By** | **hermes-agent**, **Zeroclaw**, **Nanobot**, **loops!**, **DOX**, **codegraph**, **tantivy**, **lancedb**, **surrealdb**, **petgraph**, **sentrux**, **tree-sitter-graph**, **mistral.rs**, **agentgateway**, **cowork-forge**, **openhuman**, **mcp-rust-sdk**, **wasserstein-agents**, **gsd-browser**, **chromewright**, **sediment**, **ClawDB**, **ferres-db**, **native-devtools-mcp**, **tokio-cron-scheduler**, **grpc-rust**, **mcp-searxng**, **searxng-mcp**, **opendocswork-mcp**, **slack-mcp-server**, **task-master**, **langgraph**, **crawl4ai**, **websurfx**, **headroom**, **rust-mcp-filesystem**, **novada-mcp**, **obscura**, **crawlee**, **katana**, **librefang**, **openmetadata**, **youtube-transcript-api**, **semble**, **deep-research**, **ocrs**, **agent-skills**, **superpowers**, **OpenMemory**, **SkillSpector**, **OpenHands**, **deer-flow**, **multica**, **ast-grep**, **caveman**, **graphify**, **notify**, **mcp-everything**, **mcp-memory**, **mcp-sequentialthinking**, **mcp-git**, **mcp-fetch**, **mcp-time**, and **openfang** | Synthesizes loops, workflows, code graphs, search indexers, serverless vector DBs, gateways, multi-agent teams, desktop memory, MCP SDKs, browser CDP, office parsers, stateful agent graphs, web crawlers, context scoping, native filesystems, anti-bot stealth scrapers, sandboxed agent operating systems, metadata context layers, lightweight media scrapers, token-saving hybrid search, local deep research, Rust-native machine learning OCR, lifecycle-based engineering skills, structured branch-driven workflows, self-hosted hierarchical memory engines, skill security scanning, autonomous developer agents, agent workspace management, syntax-aware structural code searches, terseness prompt optimizations, codebase knowledge graphs, filesystem file watchers, MCP reference specifications, native Git integration, and WASM execution sandboxes. |

---

## 💡 Architectural Inspirations & Design Similarities

OpenZ synthesizes patterns from several state-of-the-art developer tools to keep its footprint ultra-lightweight:

### 1. [`codegraph`](https://github.com/suatkocar/codegraph) & [`codegraph-rust`](https://github.com/Jakedismo/codegraph-rust) (Code Relationship Mapping)
*   **The Concept:** Map code structures (imports, functions, structs, classes) to represent relationships and dependencies.
*   **In OpenZ:** We employ `code_outline` (`src/tools/outline.rs`) and `ast_grep` to build structural syntax indexes. Furthermore, the `memory` MCP server (`openmemory_rs`) compiles entity-relationship graphs of the codebase so OpenZ can query dependencies and connections (e.g. "what implements this trait?") semantically without loading whole files.

### 2. [`quickwit-oss/tantivy`](https://github.com/quickwit-oss/tantivy) (High-Performance Local Indexing)
*   **The Concept:** A fast, low-memory local text indexer written in pure Rust.
*   **In OpenZ:** OpenZ prioritizes 100% Rust-native, local-first search (via optimized ripgrep wrappers and SQLite indexers) over heavy external databases (such as Elasticsearch or cloud indices). This matches `tantivy`'s philosophy: keeping search local, using zero-cost abstractions, and delivering instant boot times (<5ms) and tiny ROM/RAM footprint.

### 3. [`lancedb/lancedb`](https://github.com/lancedb/lancedb) (Serverless Vector Database)
*   **The Concept:** Serverless, disk-backed, local-first vector database storing embeddings and metadata co-located on the user's disk.
*   **In OpenZ:** OpenZ's semantic memory and research archives (`src/tools/shared_memory.rs`, `src/tools/semantic_search.rs`) implement file-backed vector search. It executes ONNX embedding models completely locally using the `fastembed` library and stores metadata in a local SQLite database (`~/.openz/memory.db`), enabling offline semantic search and zero-latency lookups without remote server overhead.

### 4. [`surrealdb/surrealdb`](https://github.com/surrealdb/surrealdb) (Embedded Multi-Model Database)
*   **The Concept:** Embedded, serverless, multi-model (document, graph, relational) database engine in Rust.
*   **In OpenZ:** OpenZ's memory system combines structured document logs, relational columns (SQLite), and entity-relationship links. The `memory` MCP (`openmemory_rs`) mimics this multi-model philosophy, executing embedded document and graph queries co-located on disk (`~/.openz/memory.db`) without requiring external database servers.

### 5. [`petgraph/petgraph`](https://github.com/petgraph/petgraph) (Graph Structures & DAG Workflows)
*   **The Concept:** Standard Rust graph representation, manipulation, and traversal library.
*   **In OpenZ:** The **SOP Workflow Engine** (`src/sop/`) represents tasks and workflows as Directed Acyclic Graphs (DAGs) and executes independent steps in parallel. It uses topological sorting and performs graph dependency cycle detection on startup. Additionally, `openmemory_rs` utilizes graph traversals (BFS/DFS) to explore code relationships.

### 6. [`sentrux/sentrux`](https://github.com/sentrux/sentrux) (Architectural Sensors & Quality Gates)
*   **The Concept:** Real-time codebase quality sensing, dependency analysis, and quality gates to prevent code decay.
*   **In OpenZ:** The built-in SOP workflow templates (such as `ship-pr-until-green` and `pre-commit-guard`) act as automated quality gates. They verify tests, compile workspaces, auto-heal syntax/borrow-checker errors in a closed loop via `CompilerAutoHealTool`, and automatically roll back code corruption using checkpointed Zenflow snapshots.

### 7. [`tree-sitter/tree-sitter-graph`](https://github.com/tree-sitter/tree-sitter-graph) (Syntactic-to-Semantic Graph Mapping)
*   **The Concept:** Constructing arbitrary graph structures directly from AST parsing syntax trees.
*   **In OpenZ:** We leverage `ast_grep` (built on `tree-sitter`) and `code_outline` to parse files structurally. The `openmemory_rs` MCP server transforms these syntax trees into relational code graphs, mapping callers, interfaces, and implementations dynamically.

### 8. [`EricLBuehler/mistral.rs`](https://github.com/EricLBuehler/mistral.rs) (Local LLM & Embedding Inference)
*   **The Concept:** Fast, local LLM and embedding inference engine written in Rust.
*   **In OpenZ:** Although we interface with cloud providers, OpenZ supports 100% private, offline executions by routing to local LLM providers (e.g. Ollama via our OpenAI API compatibility) and running vector embeddings locally on CPU/GPU via ONNX and the `fastembed` library.

### 9. [`agentgateway/agentgateway`](https://github.com/agentgateway/agentgateway) (Unified Agent Router & Traffic Plane)
*   **The Concept:** Rust-native, high-performance API gateway and traffic proxy routing agentic traffic, bridging MCP servers, and enforcing AI prompt security.
*   **In OpenZ:** The WebSocket Gateway (`openz gateway`) operates as a single-user agent traffic router. It hosts an OpenAI-compatible Completions API (`/v1/chat/completions`) that handles multi-model provider routing, translates JSON-RPC requests over an in-process gRPC Tonic bridge for stdio MCP servers, tracks token billing/usage, and logs security checks.

### 10. [`sopaco/cowork-forge`](https://github.com/sopaco/cowork-forge) (Multi-Agent Workspaces & Actor-Critic Verification)
*   **The Concept:** Automating software development by organizing specialized virtual developer teams (PMs, Architects, Engineers) and verifying code quality using Actor-Critic reviews.
*   **In OpenZ:** OpenZ implements this collaborative multi-agent structure inside `src/subagents/` with dedicated profiles (`planner`, `architect`, `reviewer`, `test_engineer`). It coordinates these subagents inside stateful loops (such as the `EvaluatorOptimizerLoopTool` and `CompilerAutoHealTool`) which act as Actor-Critic loops to review, compile, lint, and repair code iterations recursively.

### 11. [`tinyhumansai/openhuman`](https://github.com/tinyhumansai/openhuman) (Desktop-First Memory Curation)
*   **The Concept:** Desktop-first, offline-first personal AI assistant focused on memory synthesis, data integration, and private local tools.
*   **In OpenZ:** Designed as a desktop-first, highly private developer workspace assistant. Its self-improvement curate loop asynchronously reads chat logs and synthesizes raw logs into clean, editable memory facts and SQLite skills, keeping prompt contexts compact while enabling full user privacy.

### 12. [`modelcontextprotocol/rust-sdk`](https://github.com/modelcontextprotocol/rust-sdk) (Official MCP Specifications)
*   **The Concept:** Standardized JSON-RPC protocol specifications for tools/resources/prompts sharing.
*   **In OpenZ:** The client implementation (`src/tools/mcp.rs`) complies with the JSON-RPC handshake, `tools/list` schema queries, and `tools/call` executions defined in the official MCP specifications, making OpenZ fully extensible with any standard MCP tool server.

### 13. [`wasserstein-agents`](https://crates.io/crates/wasserstein-agents) (Optimal Multi-Agent Task Distribution)
*   **The Concept:** Mathematics-based optimal transport, Wasserstein distance computations, and coordinate routing of multi-agent distributions.
*   **In OpenZ:** In terms of operational coordination, OpenZ runs specialized subagents concurrently via `ParallelResearchTool` and `EvaluatorOptimizerLoopTool`. It allocates tasks dynamically to isolated sub-workspaces, preventing overlapping work and optimizing the computational transport plan of multi-agent systems.

### 14. [`gsd-browser`](https://opengsd.net/products/gsd-browser) & [`bnomei/chromewright`](https://github.com/bnomei/chromewright) (CDP-Based Browser Automation)
*   **The Concept:** Controlling web browsers natively over Chrome DevTools Protocol (CDP) WebSocket endpoints and Playwright automation.
*   **In OpenZ:** We natively register `GsdBrowserTool` (Playwright-based automation) and `ObscuraBrowserTool` (pure CDP-based WebSocket automation). This mirrors `chromewright`'s design, giving the agent direct control over CDP without needing heavy Playwright browser compilation, enabling extremely fast, lightweight web navigations.

### 15. [`rendro/sediment`](https://github.com/rendro/sediment) & [`ClawDB`](https://github.com/Claw-DB/ClawDB) (Local-First Semantic Memory & Gateways)
*   **The Concept:** Rust-based single binary local-first MCP semantic memory systems with graphs, decay, and multi-channel messaging integrations.
*   **In OpenZ:** OpenZ natively supports `sediment` as a pre-configured MCP tool server. Additionally, our native memory consolidation and multi-channel chat gateway listeners (WhatsApp, Telegram, Discord, Email, WebSockets) share this identical architectural philosophy: keeping all semantic indexing local (`~/.openz/memory.db`), managing conversational histories securely, and acting as a personal self-hosted gateway.

### 16. [`ferres-db/ferres-db`](https://github.com/ferres-db/ferres-db) (High-Performance Vector Search Engines)
*   **The Concept:** Self-hosted vector databases featuring low latency and robust write-ahead log (WAL) persistence in Rust.
*   **In OpenZ:** Our local vector database wrappers and SQLite schema implement WAL persistence for memory items. Using local embedding models, OpenZ achieves sub-millisecond local semantic lookups, matching FerresDB's goal of fast, reliable, co-located vector search.

### 17. [`sh3ll3x3c/native-devtools-mcp`](https://github.com/sh3ll3x3c/native-devtools-mcp) (Native Debugging & Computer Use)
*   **The Concept:** MCP server giving AI agents direct control over native desktop applications, browsers (via CDP), and system devtools.
*   **In OpenZ:** OpenZ integrates native tools such as `SystemInfoTool` and CDP-based `ObscuraBrowserTool` alongside our sandboxed `exec_command` shell execution. This maps directly to the native desktop and browser CDP debugging control model.

### 18. [`mvniekerk/tokio-cron-scheduler`](https://github.com/mvniekerk/tokio-cron-scheduler) (Asynchronous Job Schedulers)
*   **The Concept:** Asynchronous task scheduling loop written in Rust using Tokio for managing cron jobs.
*   **In OpenZ:** OpenZ incorporates a fully native cron scheduling architecture (`src/cron/` and `src/tools/cron.rs`). It uses standard cron formats and duration intervals to dispatch background agent routines asynchronously, ensuring non-blocking execution on active user channels.

### 19. [`grpc/grpc-rust`](https://github.com/grpc/grpc-rust) (gRPC Communication Channels)
*   **The Concept:** A high-performance Rust gRPC implementation for client-server protocol execution.
*   **In OpenZ:** We leverage `tonic` (the modern hyper-based gRPC implementation in Rust) to build the in-process MCP bridge (`src/tools/mcp.rs`). It encapsulates standard stdio MCP JSON-RPC protocols inside structured gRPC channels, providing robust and noise-isolated tool execution APIs.

### 20. [`ihor-sokoliuk/mcp-searxng`](https://github.com/ihor-sokoliuk/mcp-searxng) & [`varlabz/searxng-mcp`](https://github.com/varlabz/searxng-mcp) (Private Federated Search APIs)
*   **The Concept:** High-performance Model Context Protocol (MCP) servers for SearXNG engines, enabling private, structured web search queries and response parsing.
*   **In OpenZ:** OpenZ's modular MCP integration allows users to easily register local or private search aggregators (such as `mcp-searxng` or `searxng-mcp` via `manage_mcp` configurations), enabling highly private web fetching and query capabilities co-located on your network.

### 21. [`aimino-tech/opendocswork-mcp`](https://github.com/aimino-tech/opendocswork-mcp) (Document Extraction & Office MCPs)
*   **The Concept:** An MCP server designed to extract and parse tables, text, and metadata from `.docx`, `.xlsx`, and `.pptx` files.
*   **In OpenZ:** OpenZ natively pre-configures and supports the `office` tool powered by the `opendocswork-mcp` binary compiled locally. We also register `DocReaderTool` (`src/tools/doc_reader.rs`) which uses Rust document libraries to read PDF, spreadsheet, and text files.

### 22. [`slack-samples/bolt-js-slack-mcp-server`](https://github.com/slack-samples/bolt-js-slack-mcp-server) (Collaborative Slack MCP Bridge)
*   **The Concept:** Slack integration via MCP client structures, enabling agents to parse messages and manage workspace channels.
*   **In OpenZ:** We natively build messaging gateway adapters (TUI, WebSocket gateway, WhatsApp, Discord, Telegram, and Email IMAP/SMTP). Additionally, users can bridge Slack using `manage_mcp` to connect `bolt-js-slack-mcp-server` in-process, allowing OpenZ to monitor channels and coordinate tasks.

### 23. [`eyaltoledano/claude-task-master`](https://github.com/eyaltoledano/claude-task-master) (AI Task Execution & Complexity Scoring)
*   **The Concept:** Structured task parsing from PRD documents, complexity scoring, and test-driven autopilot compilation checks.
*   **In OpenZ:** Our **SOP Workflow Engine** compiles structural task steps into Directed Acyclic Graphs (DAGs). It runs TDD autopilots (like `ship-pr-until-green` and `pre-commit-guard`) that check builds, capture stderr compile blocks, and automatically repair code in a loop via `CompilerAutoHealTool` until all checks are verified.

### 24. [`langchain-ai/langgraph`](https://github.com/langchain-ai/langgraph) (Stateful Agentic Graph Loops)
*   **The Concept:** Modeling multi-actor agent loops as stateful graphs (nodes, edges, cycles) with structured memory persistence.
*   **In OpenZ:** OpenZ's core chat runtime runs on a **Stateful TurnState machine** (Restore → Compact → Command → Build → Run → Save → Respond → Done) designed as a cyclic state graph. Our SOP workflow engine executes tasks concurrently as Directed Acyclic Graphs (DAGs) and records execution instance states locally on disk.

### 25. [`unclecode/crawl4ai`](https://github.com/unclecode/crawl4ai) (LLM-Friendly Scrapers & Crawlers)
*   **The Concept:** Web crawlers built to compile dynamic web pages and output token-efficient clean Markdown formats optimized for LLM consumption.
*   **In OpenZ:** OpenZ features `web_fetch` (scrapes pages and converts them to clean structured markdown) and `crawl_website` (`CrawlSiteTool` using `spider-rs` for concurrent multi-threaded crawling). This aligns with `crawl4ai`'s goal: stripping HTML DOM paths into compact markdown nodes to optimize prompt context token budgets.

### 26. [`neon-mmd/websurfx`](https://github.com/neon-mmd/websurfx) (Rust Meta-Search Engines)
*   **The Concept:** High-performance, privacy-respecting, and secure search aggregators built natively in Rust.
*   **In OpenZ:** We share Websurfx's design criteria: using Rust's concurrency and memory safety to write extremely fast, low-overhead search tools (`web_search` and scrapers) that run entirely locally and aggregate information privately.

### 27. [`chopratejas/headroom`](https://github.com/chopratejas/headroom) (Context Compression & Scope Management)
*   **The Concept:** Walking directory paths to resolve local guidelines (`AGENTS.md`) and compressing logs to respect token limits.
*   **In OpenZ:** We register the `headroom-mcp` server as a default local tool, running the `scope_context` command before file edits to compile folder-specific `AGENTS.md` guidelines, and we compress tool outputs >4000 characters using context compactor states (`src/agent/context_compactor.rs`) to prevent prompt token drift.

### 28. [`rust-mcp-stack/rust-mcp-filesystem`](https://github.com/rust-mcp-stack/rust-mcp-filesystem) (Native Rust MCP Filesystems)
*   **The Concept:** Safe, standard filesystem manipulation tools implemented as MCP servers in Rust.
*   **In OpenZ:** Instead of spawning external Node.js/Python server binaries for file checks, OpenZ implements native Rust filesystem tools (`read_file`, `write_file`, `list_dir`, `patch_file`) directly in-process (`src/tools/filesystem.rs`), delivering sub-millisecond, zero-overhead file operations.

### 29. [`NovadaLabs/novada-mcp`](https://github.com/NovadaLabs/novada-mcp) (Unified Scraper & Research MCP)
*   **The Concept:** A unified MCP server offering web search, browser automation, anti-bot handling, residential proxy integration, and autonomous multi-source research.
*   **In OpenZ:** We share this vision of a multi-purpose research toolkit. OpenZ packs native search (`web_search`), concurrent crawling (`crawl_website`), and browser automation (`gsd_browser`, `obscura_browser`), acting as an all-in-one local equivalent to Novada MCP's unified scraper/researcher interface without requiring third-party SaaS API keys.

### 30. [`h4ckf0r0day/obscura`](https://github.com/h4ckf0r0day/obscura) (Stealth CDP-Based Headless Browser)
*   **The Concept:** A lightweight, dependency-free Rust-based headless browser engine consuming minimal RAM (~30MB) and offering built-in fingerprint randomization and ad/tracker blocking via Chrome DevTools Protocol.
*   **In OpenZ:** The native `ObscuraBrowserTool` (`src/tools/obscura.rs`) integrates directly with the Obscura client. By leveraging its headless CDP controls, OpenZ performs fast, stealthy, and low-resource web navigation and DOM rendering without the heavy RAM overhead of traditional Chromium/Playwright bundles.

### 31. [`apify/crawlee`](https://github.com/apify/crawlee) (Reliable Browser & Request Crawling)
*   **The Concept:** An open-source web scraping and browser automation library featuring automated proxy rotation, session handling, and robust HTML/DOM extraction pipelines.
*   **In OpenZ:** OpenZ's web fetching and crawler pipelines (`web_fetch`, `crawl_website`) mimic Crawlee's robust request loop. We handle dynamic JS rendering via CDP, fall back to high-performance raw response parsing, and clean raw HTML into token-efficient Markdown structures optimized for LLMs.

### 32. [`projectdiscovery/katana`](https://github.com/projectdiscovery/katana) (Security-First High-Speed Spidering)
*   **The Concept:** Next-generation, fast web crawling and endpoint discovery spidering tool supporting standard and headless browser modes for SPA discovery.
*   **In OpenZ:** Our concurrent `CrawlSiteTool` (powered by `spider-rs`) adopts Katana's dual-mode speed: it uses raw request spidering for sub-millisecond static page traversal and switches to headless browser rendering for dynamic paths, building a comprehensive page/endpoint index of targets rapidly.

### 33. [`librefang/librefang`](https://github.com/librefang/librefang) (Rust Agent Operating System)
*   **The Concept:** An open-source, Rust-native agent operating system managing processes, isolation, scheduling, and Merkle audit trails.
*   **In OpenZ:** We share this systems-centric design philosophy. OpenZ functions as a lightweight agent runtime that manages task execution loops, applies process sandboxing (seccomp BPF filters), and builds cryptographic Merkle hash-chain ledgers to track actions securely and transparently.

### 34. [`open-metadata/openmetadata`](https://github.com/open-metadata/openmetadata) (Unified Metadata Context Layer)
*   **The Concept:** A centralized open-source data discovery, cataloging, and collaboration platform providing a shared context layer for humans and AI.
*   **In OpenZ:** OpenZ implements local context discovery and metadata co-location. It maps code outlines structurally and resolves folder-specific instructions dynamically, acting as an in-process, developer-first metadata layer.

### 35. [`jdepoix/youtube-transcript-api`](https://github.com/jdepoix/youtube-transcript-api) (Lightweight Captions Extractor)
*   **The Concept:** A fast, dependency-free scraper that fetches video transcripts and subtitles without requiring Google API keys or headless browsers.
*   **In OpenZ:** We prioritize lightweight, zero-key scraping alternatives. Just as the YouTube transcript API avoids heavy Selenium stacks, OpenZ's native `DocReaderTool` and `web_fetch` scraper extract structured content directly with minimal overhead.

### 36. [`minishlab/semble`](https://github.com/minishlab/semble) (Token-Saving Hybrid Code Search)
*   **The Concept:** A fast CPU-only code search library combining semantic embeddings and BM25 lexical search to retrieve precise code chunks instead of reading entire files.
*   **In OpenZ:** OpenZ implements local code search (`grep_search`, `ast_grep`) and local vector-based semantic search (`FastEmbed`). This matches Semble's core mission: indexing structures locally and scope-limiting code retrievals to prevent prompt token drift.

### 37. [`u14app/deep-research`](https://github.com/u14app/deep-research) (Private Deep Research Report Engine)
*   **The Concept:** An open-source tool designed to generate in-depth, privacy-focused research reports using LLMs, local storage, and MCP interfaces.
*   **In OpenZ:** We share the goal of privacy-focused deep research. OpenZ coordinates specialized subagents (such as in `ParallelResearchTool`) to perform concurrent multi-source queries, scraping, and synthesis, storing all research data locally without external SaaS trackers.

### 38. [`robertknight/ocrs`](https://github.com/robertknight/ocrs) (Rust-Native OCR Engine)
*   **The Concept:** A modern, native Rust OCR library using neural networks and the RTen runtime to extract text from images.
*   **In OpenZ:** Just as ocrs replaces external binary dependencies with pure-Rust machine learning models, OpenZ prioritizes Rust-native local engines (like ONNX models via fastembed) for private, lightweight in-process metadata extraction.

### 39. [`addyosmani/agent-skills`](https://github.com/addyosmani/agent-skills) (Production-Grade Engineering Workflows)
*   **The Concept:** A collection of structured, lifecycle-based skills (Define, Plan, Build, Verify, Ship) with standardized slash commands designed to enforce rigorous engineering habits in AI coding agents.
*   **In OpenZ:** The GSD workflow system implements this lifecycle directly. We use specs, plans, compiler check loops, and self-healing tools to enforce TDD and prevent lazy code edits.

### 40. [`obra/superpowers`](https://github.com/obra/superpowers) (7-Stage Disciplined Engineering Methodology)
*   **The Concept:** A modular agentic skills framework guiding AI assistants through isolated feature branches, TDD, code review, and branch merges to ensure code quality.
*   **In OpenZ:** OpenZ's workflow loops mirror these 7 stages. We execute parallel tasks via topological DAG sorting, verify output builds natively, auto-heal errors, and use cryptographic Merkle ledger transitions to secure updates.

### 41. [`CaviraOSS/OpenMemory`](https://github.com/CaviraOSS/OpenMemory) (Self-Hosted Hierarchical Memory Engine)
*   **The Concept:** An open-source, self-hosted memory engine providing long-term contextual memory using a Hierarchical Memory Decomposition architecture.
*   **In OpenZ:** We share this local-first structured memory philosophy. OpenZ utilizes a multi-tier memory architecture co-locating episodic facts (in session metadata) and procedural instructions/skills (within a local SQLite database), enabling humans and agents to access structured context securely.

### 42. [`NVIDIA/SkillSpector`](https://github.com/NVIDIA/SkillSpector) (AI Agent Skill Security Evaluator)
*   **The Concept:** An open-source security tool that scans AI capabilities and skills for vulnerabilities, malicious logic, and data exfiltration vectors.
*   **In OpenZ:** Security and safety are core constraints. OpenZ's native `SecurityGuard` acts as an active active verification gate, intercepting destructive tools, network requests, and out-of-workspace commands to validate compliance before code changes are made.

### 43. [`OpenHands/OpenHands`](https://github.com/OpenHands/OpenHands) (Model-Agnostic AI Developer Harness)
*   **The Concept:** An open-source generalist agent platform that automates software engineering tasks within secure sandboxed environments.
*   **In OpenZ:** OpenZ implements this fully developer-centric execution paradigm. It combines multi-agent TDD loops with direct workspace sandboxing, letting developers run workflows securely on local systems or cloud channels.

### 44. [`bytedance/deer-flow`](https://github.com/bytedance/deer-flow) (Long-Horizon Multi-Agent Task Harness)
*   **The Concept:** A SuperAgent harness designed for autonomous task planning, multi-agent decomposition, and isolated tool execution.
*   **In OpenZ:** OpenZ adopts a similar multi-agent orchestration pattern. It breaks complex objectives into subtasks, delegates them to specialized profiles (like planner, reviewer, etc.) inside isolated sub-workspaces, loads modular `skills/*.md` on demand, and maintains cross-session memory.

### 45. [`multica-ai/multica`](https://github.com/multica-ai/multica) (Autonomous Team & Agent Workspace)
*   **The Concept:** An open-source managed platform designed to orchestrate and manage AI coding agents as if they were real team members, using a local daemon to route tasks.
*   **In OpenZ:** We share this focus on developer agent teams. The local WebSocket gateway (`openz gateway`) acts as a single-user agent router, hosting completions APIs, managing multiple model provider sessions, and routing tool tasks dynamically.

### 46. [`ast-grep/ast-grep`](https://github.com/ast-grep/ast-grep) (AST-Based Structural Code Search & Replace)
*   **The Concept:** A high-performance command-line tool written in Rust for syntax-aware code search, linting, and rewriting using abstract syntax trees.
*   **In OpenZ:** OpenZ integrates `ast_grep` natively as a core tool (`src/tools/ast_grep.rs`). This allows OpenZ to perform structure-aware refactoring, syntax pattern matches, and precise edits without relying on raw regex or simple string searches.

### 47. [`juliusbrussee/caveman`](https://github.com/juliusbrussee/caveman) (Terseness-Driven Token Compression)
*   **The Concept:** An AI coding skill designed to reduce token usage by forcing models to communicate in an ultra-compressed "caveman-like" style.
*   **In OpenZ:** OpenZ natively integrates this via the `caveman_mode` setting (ON by default). This injects a specific system prompt instruction that strips pleasantries and filler words, reducing token overhead while maintaining complete technical accuracy.

### 48. [`safishamsi/graphify`](https://github.com/safishamsi/graphify) (Codebase-to-Knowledge-Graph Builder)
*   **The Concept:** An agentic skill that processes codebases and document folders to compile queryable entity-relationship knowledge graphs.
*   **In OpenZ:** OpenZ incorporates this capability. It pre-configures a local `graphify` skill and bridges it with the `openmemory_rs` MCP server, mapping structural code imports and file hierarchies into queryable graph nodes (JSON/HTML outputs).

### 49. [`notify-rs/notify`](https://github.com/notify-rs/notify) (Cross-Platform File Watching)
*   **The Concept:** A standard cross-platform file system monitoring library in Rust that watches files/directories for modifications, creations, and deletions.
*   **In OpenZ:** Utilized directly by the `FileWatcherTool` (`src/tools/watcher.rs`) to track workspace folder changes and trigger automated compilation/test suites when source code changes.

### 50. [`modelcontextprotocol/servers/src/everything`](https://github.com/modelcontextprotocol/servers/tree/main/src/everything) (MCP Everything Reference)
*   **The Concept:** A reference Model Context Protocol server demonstrating resources, prompts, and tools implementation specifications.
*   **In OpenZ:** OpenZ's modular MCP client (`src/tools/mcp.rs`) supports all standard JSON-RPC capability sets shown in the `everything` reference, allowing the agent to dynamically inspect tools and compile prompts.

### 51. [`modelcontextprotocol/servers/src/memory`](https://github.com/modelcontextprotocol/servers/tree/main/src/memory) (MCP Graph-Based Semantic Memory)
*   **The Concept:** An MCP server that maintains persistent semantic entity-relationship graphs.
*   **In OpenZ:** We leverage this exact pattern to build entity-relation indices. OpenZ interfaces with the `openmemory_rs` MCP server to maintain knowledge graphs and execute semantic context traversals.

### 52. [`modelcontextprotocol/servers/src/sequentialthinking`](https://github.com/modelcontextprotocol/servers/tree/main/src/sequentialthinking) (MCP Sequential Planning)
*   **The Concept:** An MCP server designed to support step-by-step reasoning and logical progression before code execution.
*   **In OpenZ:** OpenZ pre-configures and defaults to `mcp-server-sequential-thinking` (compiled locally) to help the model plan structural edits and reason systematically in complex files.

### 53. [`modelcontextprotocol/servers/src/git`](https://github.com/modelcontextprotocol/servers/tree/main/src/git) (MCP Git Integration)
*   **The Concept:** An MCP server exposing git repository tools (status, diff, log, commit).
*   **In OpenZ:** OpenZ implements a native Rust-based `GitManagerTool` (`src/tools/git_manager.rs`) to track repository status, commits, and diffs directly in-process without requiring external daemon executions.

### 54. [`modelcontextprotocol/servers/src/fetch`](https://github.com/modelcontextprotocol/servers/tree/main/src/fetch) (MCP Web Fetching & Markdown Converter)
*   **The Concept:** An MCP server that fetches pages and parses them into clean Markdown.
*   **In OpenZ:** We implement this capability directly in-process via our native `web_fetch` scraper (utilizing the `scraper` and `html2md` crates), delivering sub-millisecond page fetches and cleaner token-saving formatting.

### 55. [`modelcontextprotocol/servers/src/time`](https://github.com/modelcontextprotocol/servers/tree/main/src/time) (MCP Time & Date Context)
*   **The Concept:** An MCP server providing local time and timezone transformations.
*   **In OpenZ:** OpenZ uses the native `chrono` library to manage date and time boundaries inside scheduled cron tasks and logs, rendering time contexts locally.

### 56. [`nousresearch/hermes-agent`](https://github.com/nousresearch/hermes-agent) (Self-Evolution & Skill Optimization)
*   **The Concept:** An autonomous agent platform designed for long-term memory, self-learning, asynchronous subagents, and self-evolution to optimize its own skills over time.
*   **In OpenZ:** OpenZ incorporates a closed-loop self-improvement curator that asynchronously reviews conversation histories, extracts factual memories, and updates procedural skill guidelines dynamically inside `~/.openz/skills/*.md`.

### 57. [`HKUDS/nanobot`](https://github.com/HKUDS/nanobot) (Lightweight AI Developer Core)
*   **The Concept:** An ultra-lightweight, personal AI developer assistant written in Rust supporting multi-platform integrations, MCP server bridging, and cron task scheduling.
*   **In OpenZ:** OpenZ is the direct successor and rebrand of `nanobot`, preserving its clean event-driven agent loop while expanding it with robust multi-channel adapters, gRPC bridges, and SQLite database storage.

### 58. [`zeroclaw-labs/zeroclaw`](https://github.com/zeroclaw-labs/zeroclaw) (Secure Zero-Overhead Runtimes)
*   **The Concept:** A high-performance, zero-overhead autonomous AI agent runtime written entirely in Rust, prioritizing secure local executions and systems-level efficiency.
*   **In OpenZ:** We target an identical lightweight systems footprint and execution speed. OpenZ's `SecurityGuard` permissions and BPF-based seccomp sandboxing align directly with `zeroclaw`'s secure, deny-by-default runtime execution.

### 59. [`RightNow-AI/openfang`](https://github.com/RightNow-AI/openfang) (Modular Agent Operating Systems)
*   **The Concept:** An Agent Operating System in Rust using modular capability packages ("Hands"), WASM engines, and cryptographic Merkle audit trails.
*   **In OpenZ:** OpenZ implements modular, pre-configured capability sets, supports WASM execution sandboxes (`wasm_sandbox`), and utilizes a cryptographic Merkle Hash-Chain ledger to track and audit all actions securely.

### 60. [`agent0ai/dox`](https://github.com/agent0ai/dox) (Hierarchical Context Resolution)
*   **The Concept:** A token-efficient codebase context framework that establishes a hierarchical tree of `AGENTS.md` files (from project-level down to folder-specific instructions) so AI agents can navigate directories dynamically.
*   **In OpenZ:** We natively support this hierarchical folder rules resolution. By utilizing the `headroom-mcp` (`scope_context`) server, OpenZ automatically traverses directory structures to parse local `AGENTS.md` context layers and scope-limits code files during workspace edits.

### 61. [`loops!`](https://github.com/agent-skills) (Iterative Loop Engineering)
*   **The Concept:** Designing autonomous AI workflows as persistent feedback loops (Action → Observation → Decision/Refinement → Repeat) that iteratively test and self-heal code execution rather than single-shot prompts.
*   **In OpenZ:** The Stateful TurnState machine and the **SOP Workflow Engine** run on this exact looping model. Workflows (like `ship-pr-until-green`) compile, execute, read compiler stderr/stdout logs, and feed errors back to the LLM to refine and heal code iterations recursively.

---

## ⚡ Key Features & Subsystems

### 1. Memory & Skill Self-Improvement 🧠
*   **Dual-Tier Memory System:**
    *   *Tier 1 (Factual Memory):* Captures user preferences, persona, and session facts inside the session JSON (`session.metadata["memory"]`).
    *   *Tier 2 (Procedural Skills):* Stores recipes, conventions, and troubleshooting guidelines inside a local SQLite database (`~/.openz/memory.db`).
*   **Closed-Loop Background Curator:** Asynchronously reviews conversation history after each turn (using `tokio::spawn`), compiles new guidelines, and isolates skills by subagent profile (e.g. `profile = 'planner'`).
*   **Curator Throttling:** Avoids redundant LLM review calls on simple queries by throttling runs (requiring >4000 tokens of context or a tool call that modifies files/executes commands/uses the web).
*   **Stale Skills Archival Throttling:** Throttles background database cleaning checks (`archive_stale_skills`) to run at most once every 24 hours.

### 2. Native Compiler Auto-Healing 🛠️
*   **Self-Healing Loop (`compiler_auto_heal`):** Native reflection tool that executes build/compile commands (e.g., `cargo check` or `npm run build`), captures compiler errors (stderr/stdout), feeds them back to the LLM, and refines edits in an iterative loop (up to 5 iterations) until compilation succeeds.

### 3. Stateful SOP Workflow Engine (loops!-inspired) 📋
*   **DAG Execution:** Executes multi-step Directed Acyclic Graph (DAG) procedures in parallel using Tokio.
*   **Built-in SOPs:**
    *   `ship-pr-until-green`: Feature implementation, PR creation, CI verification loop, and self-healing.
    *   `pre-commit-guard`: Pre-commit hook configuration and workspace validation.

---

## 🔌 Model Context Protocol (MCP) Integration & Servers

OpenZ communicates with external tool servers using the Model Context Protocol (MCP) to extend its capabilities. It supports two primary communication architectures:
1.  **Stdio JSON-RPC:** Spawns external processes with standard pipe redirection (`stdin`/`stdout`).
2.  **Unified gRPC (Tonic):** To prevent third-party logging output ("stdio pollution") from breaking the JSON-RPC parser, OpenZ runs an automatic in-process bridge. It maps stdio-based servers to an ephemeral gRPC port on localhost, automatically filtering out non-JSON log lines.

### Pre-Configured Rust-Native MCP Servers
OpenZ prioritizes high-performance, cargo-installed Rust MCP servers located in `~/.cargo/bin/` (or resolved via the `AI_AGENT_TOOLS_BASE` workspace env var):
*   **`headroom`** (`headroom-mcp`): Implements `scope_context` to scan directory trees for `AGENTS.md` guidelines and inject local rules, preventing context drift. Also compresses long tool outputs.
*   **`office`** (`opendocswork-mcp`): Direct text, table, and structure extractor for `.docx`, `.xlsx`, and `.pptx` documents.
*   **`sequential-thinking`** (`mcp-server-sequential-thinking`): Reasoning server allowing the model to perform sequential, multi-step structured thinking before executing changes.
*   **`memory`** (`openmemory_rs`): Persistent semantic entity-relationship graph database for storing knowledge graphs.

### MCP Management:
*   **Dynamic configuration:** The agent manages server registrations via the `manage_mcp` tool.
*   **`mcps_manager` subagent:** A protected subagent profile equipped to download, compile, configure, and install MCP servers on demand.

---

## 🔧 Core Tools Registry & Usages

OpenZ exposes a robust, local tool set categorized below:

### 1. Filesystem & Repository Analysis
*   `read_file` / `write_file` / `patch_file`: Reads, writes, or patches target text files recursively.
*   `find_files`: Searches for files matching glob patterns with size and time filtering.
*   `replace_lines`: Replaces exact line sequences within a file (surgical line-level edits).
*   `zenflow_edit`: Multi-file structural editing with smart context matching (requires git repository).
*   `list_dir`: Lists directory contents including sizes and subfolders.
*   `grep_search`: Highly optimized ripgrep wrapper for locating patterns across codebases.
*   `code_outline`: Generates class, struct, function, and interface outline trees (Rust, Python, Go, JS/TS).
*   `ast_grep`: Executes structural AST searches (e.g. matching syntax patterns).
*   `index_codebase`: Indexes codebase structure into a structured JSON summary.
*   `git_manager`: Executes git operations (status, diff, log, commits).
*   `db_inspector` / `db_write`: Secure SQLite database reader and query writer.
*   `doc_reader`: Extracts text from PDF, DOCX, and XLSX files.
*   `rust_docs`: Queries Rust documentation from docs.rs for crate API references.
*   `compile_template`: Compiles Handlebars/Mustache templates with provided context data.

### 2. Sandbox & Compilation
*   `exec_command`: Runs sandboxed shell commands using a Linux BPF seccomp sandbox filter (if enabled).
*   `python_sandbox`: Executes Python scripts in an isolated subprocess with resource limits.
*   `wasm_execute`: Executes WebAssembly (`.wasm`) binaries inside a secure, sandboxed `wasmtime` engine.
*   `cargo_manager`: Runs compilation and testing (`cargo check`, `cargo build`, `cargo test`).
*   `js_format`: Fast JavaScript/TypeScript syntax formatting.
*   `compiler_auto_heal`: Automatically diagnoses and fixes compilation errors.

### 3. Web Search, Scraping & Social
*   `web_search`: Conducts Tavily web queries to return search results.
*   `web_fetch`: Scrapes HTML pages and converts them to formatted markdown.
*   `social_search`: Searches Hacker News, Reddit, and other social platforms for content.
*   `crawl_website`: Performs multi-threaded async site spidering via `spider-rs`.
*   `gsd_browser`: Direct headless Chrome automation (Playwright-based).
*   `obscura_browser` / `firefox_browser`: CDP-based headless browser controls.
*   `semantic_search`: Performs vector-based semantic search across a codebase using embeddings.

### 4. Job Scheduling & Cron
*   `schedule_job`: Registers recurring background cron tasks or one-time timers.
*   `list_jobs` / `remove_job`: Lists or deletes registered jobs.
*   `file_watcher`: Watches local folders to trigger scripts/commands when files change.

### 5. Memory & Knowledge
*   `store_memory`: Stores structured observations, decisions, or facts in the agent's long-term memory.
*   `recall_memory`: Retrieves stored memories by query context.
*   `clear_memory`: Clears all entries from the agent's memory store.
*   `archive_research`: Archives research findings into persistent storage.
*   `search_research`: Searches archived research content.
*   `index_notes`: Indexes and searches local markdown notes.

### 6. Graphics & Visuals
*   `render_mermaid`: Renders 23+ diagram formats directly to SVG.
*   `generate_video`: Compiles JSON timeline descriptions to MP4 files via `wavyte`.
*   `generate_image`: Generates PNG images programmatically from HTML/CSS/SVG or URL.
*   `html_to_video`: Renders timeline-based MP4 videos from HTML/CSS animation templates.
*   `create_animated_svg`: Creates animated SVG files from motion descriptions.

### 7. Subagents, Messaging & Workflows
*   `delegate_task`: Runs isolated subtasks in a separate subagent context.
*   `parallel_research`: Runs multiple research subtasks in parallel and merges results.
*   `evaluator_optimizer_loop`: Iteratively generates and evaluates responses until quality criteria met.
*   `optimize_subagent`: Refines a subagent's system prompt using AI based on feedback.
*   `create_subagent` / `delete_subagent`: Dynamically creates or removes custom subagent profiles.
*   `trigger_sop`: Instantiates stateful workflows (SOPs).
*   `send_remote_input`: Forwards commands to other active agent sessions.
*   `onpkg`: Integrates with the `onpkg` package and stack manager.

---

## 🎮 Basic Usage & Console Commands

### Running Channels
*   **Terminal TUI:** `openz agent` (launches interactive terminal prompt).
*   **WebSocket gateway:** `openz gateway` (launches static Web UI server & completions API).
*   **Telegram bot:** `openz telegram` (polls configured bot token).
*   **Discord bot:** `openz discord` (connects as a Discord gateway bot).
*   **WhatsApp API:** `openz whatsapp` (spawns an Axum webhook receiver on port 8090).

### TUI Terminal Slash Commands
Inside `openz agent`, the user can issue direct slash commands:
*   `/memory` / `/memory add <fact>` / `/memory clear`: Manage Tier-1 facts.
*   `/skills` / `/skill view <name>` / `/skill add <name>` / `/skill delete <name>`: Manage Tier-2 database skills.
*   `/sop list` / `/sop instances` / `/sop trigger <id>`: Manage SOP workflow loops.
*   `/audit`: Verifies the cryptographic Merkle hash-ledger integrity and lists recent transactions.
*   `/clear`: Resets active conversation context window history.
*   `/status`: Lists loaded MCP servers, active session information, and resource use.

---

## 📅 Version Release History

### v0.0.42 (Latest Release)
*   **Feature: Prompt-aware tool router (HIGH)**:
    *   Added tool metadata for domains, risk, resource usage, aliases, examples, and use/avoid guidance so OpenZ can expose the most relevant tools per prompt.
    *   Tool descriptions sent to providers now include compact routing hints to improve tool choice without inflating the full system prompt.
    *   Added optional TUI router visibility through `showToolRouterStatus` for debugging which tools were selected and why.
*   **Feature: Native tool catalog and observability (HIGH)**:
    *   Added the `tool_catalog` native tool so agents can inspect available tools, domains, aliases, risk levels, examples, and current router/resource status.
    *   Extended config management support for router and resource-policy settings.
*   **Guardrail: Tool resource policy (HIGH)**:
    *   Added disk-space, network-tool, expensive-tool, and concurrent process-tool checks before native tool execution.
    *   Added configurable `minFreeDiskGb`, `allowNetworkTools`, `maxConcurrentProcessTools`, and `warnBeforeExpensiveTools` settings.
    *   Reused existing approval flow for expensive tools while avoiding duplicate approval prompts after the security guard has already asked.
*   **Tests:** Added targeted coverage for tool routing metadata, API truncation behavior, tool catalog output, self-management config updates, and resource-policy decisions.
*   **Chore:** Bumped version to `v0.0.42`.

### v0.0.41 (Previous Release)
*   **Fix: OpenZ worktree disk quota guard (CRITICAL)**:
    *   Added size-aware cleanup for `~/.openz/worktrees/openz_worktree_*` to prevent stale subagent worktrees from consuming large disk space.
    *   Worktree cleanup now enforces max age, max count, max total bytes, and a minimum free-space safety margin.
    *   Added tests to verify old worktrees are removed, non-OpenZ directories are preserved, and oldest worktrees are pruned first when the quota is exceeded.
*   **Fix: Ctrl+C / exit shutdown hang (HIGH)**:
    *   Distinguishes idle-prompt Ctrl+C from active-turn cancellation so idle Ctrl+C triggers app shutdown instead of only sending a turn-cancel signal.
    *   Bounds gateway shutdown and outbound offline-notification HTTP calls with short timeouts so OpenZ cannot sit indefinitely after `Goodbye!` / `Shutting down gateways...`.
*   **Feature: Subagent lifecycle status model (HIGH)**:
    *   Added structured subagent lifecycle states for queued, running, fallback, cancelling, cancelled, timed out, failed, and completed states.
    *   Subagent JSON tool results now include lifecycle metadata so TUI and future monitoring can render clear status instead of relying on raw strings.
*   **Chore: Version drift guard**:
    *   Added a test that verifies Cargo, README, ONPKG, Markdown changelog, and CLI changelog version surfaces stay synchronized.
*   **Chore:** Bumped version to `v0.0.41`.

### v0.0.40 (Previous Release)
*   **Fix: Subagent cancellation caused invisible input / frozen prompt text (CRITICAL)**:
    *   **Root cause:** When a user cancelled a turn (using `Esc` or `Ctrl+C`), the outer future (`run_fut`) was aborted immediately. However, because the future wrapping the subagent spinner (`with_spinner`) was dropped mid-execution, the clean-up block that popped the spinner from the `ACTIVE_SPINNERS` stack was bypassed. This left the spinner on the stack indefinitely.
    *   **Consequence:** The TUI print shims (`tui_print_fn` and `tui_println_fn`) constantly saw an active spinner and printed `\r\x1b[2K` on every print, immediately clearing the prompt and the user's typed input characters as they typed them.
    *   **Fix:** Introduced `SpinnerGuard`, a struct implementing the Rust `Drop` trait. Because `drop()` is guaranteed to execute when the guard goes out of scope (even if the future is dropped or cancelled), the spinner is now always reliably popped from the coordination stack.
*   **Fix: TUI spinner overlapping/misaligned rendering during cancel and thinking states (HIGH)**:
    *   **Root cause:** Direct raw `print!` statements were used to render the turn cancellation warning (`▲ Turn cancelled by user.`) and the model's thinking duration badges (`L ● Thought for Xs`). Because they bypassed the global `STDOUT_MUTEX` lock and active spinner checks, the background spinner thread wrote progress characters (e.g. `⠋`) side-by-side on the same line.
    *   **Fix:** Converted all raw warning and thought-duration prints in `run.rs` to `crate::tui_println!`. They now correctly lock the stdout mutex and clear any active spinner line before rendering.
*   **Chore:** Bumped version to `v0.0.40`.

### v0.0.39 (Previous Release)
*   **Fix: Esc / Ctrl+C cancellation was unreliable during subagent execution (CRITICAL)**:
    *   **Root cause:** Crossterm raw mode converts `Ctrl+C` from a signal (`SIGINT`) into a key event, bypassing the OS signal handler. The keyboard listener was running inside `tokio::task::spawn` with blocking `crossterm::event::poll/read` calls, starving the Tokio runtime and preventing the cancellation future from making progress.
    *   **Fix:** Moved keyboard input polling to a dedicated **OS thread** (`std::thread::spawn`) instead of a Tokio task. The OS thread polls crossterm events every 100ms without blocking the async runtime, and signals cancellation via the existing `CLI_CANCEL_TX` watch channel.
    *   **Fix:** Added a `std::sync::mpsc` channel for the main task to signal the keyboard thread to stop cleanly after the agent turn completes, preventing leaked threads.
*   **Fix: LLM streaming loop was not cancellation-aware (CRITICAL)**:
    *   The `while let Some(chunk) = stream.next().await` loop in `run.rs` would block indefinitely on slow LLM responses with no cancellation check.
    *   **Fix:** Replaced with a `tokio::select!` loop that races each `stream.next()` against a **cancel-safe** `watch::Receiver::changed()` listener (not `Notify`, which loses permits when dropped in `select!`).
*   **Fix: Tool execution (subagent delegation) was not cancellable (HIGH)**:
    *   Tool calls (especially `delegate_task`, `parallel_research`) ran behind `tokio::time::timeout` but had no way to be interrupted by user input.
    *   **Fix:** Wrapped tool execution in `tokio::select!` racing the timeout+execution against the `CLI_CANCEL_TX` watch channel. Cancellation and timeout errors both propagate to the turn-level `CancellationToken` to break the agent loop immediately.
    *   Flattened the match arms from `Ok(Ok(res)) / Ok(Err(e)) / Err(timeout)` to `Ok(res) / Err(e)` since timeout is now folded into the `Err` variant.
*   **Fix: Subagent tool spinners were silently suppressed (MEDIUM)**:
    *   `with_spinner()` in `spinner.rs` returned early (no visual feedback) when `DELEGATION_DEPTH > 0`, making subagent tool execution appear frozen.
    *   **Fix:** Spinners now render at all delegation depths with tree-prefix indentation (using `get_tree_prefix()`), giving visual feedback during subagent work.
*   **Fix: `openz logs` missed subagent log lines when filtering by session (HIGH)**:
    *   Nested tracing spans produce lines like `turn{session=cli:abc}:turn{session=subagent:vision:123}: ...` with multiple `session=` values. The old `extract_session_from_line` only found the FIRST `session=` (the parent CLI session), so filtering by the subagent session would miss these lines.
    *   **Fix:** Replaced `extract_session_from_line` with `extract_all_sessions_from_line` that collects ALL `session=` values from the entire line. Updated `session_matches` to check if ANY of the line's sessions match the filter. The session badge now displays the LAST (most specific) session.
*   **Fix: Non-streaming LLM call path was completely un-cancellable (CRITICAL)**:
    *   When `streaming = false`, `chat_with_fallback()` calls `active_provider.chat()` wrapped in `with_spinner()` — neither races against any cancel signal. The code blocks on the HTTP response indefinitely, ignoring Esc/Ctrl+C entirely.
    *   **Fix:** Wrapped the non-streaming `chat_with_fallback` call in `tokio::select!` racing against the `CLI_CANCEL_TX` watch channel. On cancel, sets `turn_cancel.cancel()` and returns `TurnState::Save` immediately.
*   **Fix: `chat_with_fallback` retry loop ignored cancellation (HIGH)**:
    *   When the primary provider fails, the fallback loop iterates through up to 3 fallback models + retries the original — none checked for cancellation. With network issues, this means 4+ HTTP timeouts before cancel takes effect.
    *   **Fix:** Added cancel-before-attempt checks at each stage: before each fallback model attempt, and before the final retry of the original provider. Returns `Err("Cancelled by user")` immediately.
*   **Fix: Auto-continuation loop ignored cancellation (MEDIUM)**:
    *   When `finish_reason == "length"`, up to 3 additional `chat_with_fallback` calls run with no cancel check, compounding the non-streaming blocking bug.
    *   **Fix:** Added `turn_cancel.is_cancelled()` check at the start of each continuation iteration.
*   **Fix: Multi-tool-call batch didn't break on cancel (MEDIUM)**:
    *   When the LLM returns multiple tool calls in one response, the `for call in resp.tool_calls` loop continues after the first tool is cancelled, starting and immediately cancelling each subsequent tool (unnecessary overhead + error accumulation).
    *   **Fix:** Added `turn_cancel.is_cancelled()` check at the start of each tool call iteration to break immediately.
*   **Fix: Vision subagent model routing and fallback isolation (CRITICAL)**:
    *   Dynamic subagent tools are now reserved inside the 128-tool OpenAI-compatible payload limit, preventing `vision_agent` from being described in prompts but missing from executable tool schemas.
    *   Subagent runtime config now preserves the selected profile model/provider/fallbacks across per-turn config reloads, so `vision_agent` no longer reverts internally to the main orchestrator model.
    *   Profile subagents no longer inherit the main agent's global fallback models; each profile owns its configured primary/fallback chain.
    *   OpenRouter-style `*:free` model IDs (for example `google/gemma-4-31b-it:free` and `nvidia/...:free`) route to OpenRouter instead of being sent to OpenAI or stripped incorrectly.
*   **Fix: Delegate-task cancellation and false interrupt reporting (HIGH)**:
    *   Active agent turns now keep crossterm raw mode enabled while the keyboard cancellation watcher is running, so Esc/Ctrl+C are delivered during long `delegate_task` and subagent calls.
    *   Tool/subagent timeouts no longer set the same turn-cancel flag used by real user interrupts, preventing misleading `Turn cancelled by user` messages when the provider timed out or rate-limited.
    *   Generic delegated workers no longer receive recursive `delegate_task` registration by default, preventing runaway nested delegation loops.
*   **Chore:** Bumped version to `v0.0.39`.

### v0.0.38 (Previous Release)
*   **Fix: Live `openz logs` streaming was unusably delayed (CRITICAL)**:
    *   Linked `CancellationToken` to `CLI_CANCEL_TX` (per-turn Ctrl+C signal) in addition to the existing `SHUTDOWN_TX` (global SIGTERM) listener. Previously, pressing Ctrl+C during a subagent task only cancelled the outer CLI loop but left subagent background tasks running.
    *   Added `biased;` to all 8 `tokio::select!` cancellation branches in `delegate_task.rs`, `delegate_profile.rs`, and `parallel_research.rs` — ensures cancellation is checked before running the child agent.
    *   Fixed `CancellationToken` background task unconditionally firing when `shutdown::receiver()` returned `None` — now only cancels if a signal actually fired.
    *   Fixed `parallel_research.rs` spawned tasks: added `tokio::select!` with `biased;` cancellation to race `join_all` against `wait_for_cancellation()`, so the parent returns immediately on Ctrl+C instead of waiting for spawned tasks.
*   **Fix: Multi-workspace session conflicts (HIGH)**:
    *   Replaced the hardcoded `"cli:direct"` session key with a workspace-unique key derived from the current working directory hash (`cli:{cwd_hash}`).
    *   This allows multiple `openz agent` instances to run simultaneously in different project directories without lock contention.
    *   Remote input (Telegram, watcher, `send_remote_input`) still works via a `"cli:direct"` global inbox fallback in the CLI input loop.
    *   Added `get_cli_session_key()` to `config/loader.rs`. Updated `channels/cli/mod.rs`, `cli/agent.rs`, `channels/cli/input.rs`, `agent/security.rs`, and `agent/agent_loop/mod.rs`.
*   **Fix: Background channel `eprintln!` calls corrupt TUI (MEDIUM)**:
    *   Replaced 15 `eprintln!()` calls in Discord, Telegram, Email, agent subroutines, and config loader with `tracing::error!()` or `tui_println!()` to prevent terminal corruption when running background channels concurrently.
*   **Refactor: Eliminated 30 `#[allow(unused_macros)]` boilerplate blocks (LOW)**:
    *   Defined shared `#[macro_export]` versions of `println!`, `print!`, `eprintln!`, `eprint!` in `agent/style/mod.rs`.
    *   All CLI modules now use `use crate::{println, ...};` instead of redefining the same 3-4 macros locally (11 files, ~270 lines removed).
*   **Refactor: Deduplicated provider config resolution (MEDIUM)**:
    *   Added `Config::resolve_provider_config()` in `config/schema.rs` as the single source of truth for provider API key + base URL resolution.
    *   Removed ~500 lines of identical 17-way match arms from `providers/resolver.rs`, `channels/mod.rs`, and `cli/builder.rs`.
*   **Chore: `.gitignore` cleanup, removed old branding**:
    *   Added `/memory.db-shm` and `/memory.db-wal` to `.gitignore`.
    *   Removed stale `nanobotdocs/` directory (old branding) and duplicate `assets/logo1.png`.
    *   Bumped version to `v0.0.38`.

### v0.0.37 (Previous Release)
*   **Feature: Bot Detection Bypass (Stealth) for SearchXyz (HIGH)**:
    *   Implemented dynamic **Rotating Proxies** inside DuckDuckGo and Google search backends.
    *   Implemented **Headless Browser Fallback** (`js-rendering` feature utilizing `chromiumoxide`) to bypass Cloudflare captchas and rate-limiting blocks automatically on Raw HTTP request failure.
    *   Wired up the `Crawler` pool of client proxies and headless browser instances directly into backends initialization in `tools/searchxyz/src/bin/searchxyz.rs` and `src/tools/searchxyz/mod.rs`.
*   **Feature: Structured Error Self-Healing Recovery ("Errors as Instructions") (HIGH)**:
    *   Upgraded the tool execution loop in [run.rs](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/run.rs) to natively parse and forward structured JSON error responses returned by tools.
    *   Merges local `self_healing_suggestion` recovery hints seamlessly into structured tool errors if missing, allowing tools to define their own explicit semantic healing protocols.
*   **Feature: TUI Formatting, Casing, and Duplicate Redundant Suffix/Prefix Fixes (MEDIUM)**:
    *   Resolved argument casing parameter mismatches (`query`/`Query` and `path`/`Path`) across all native tools (`grep_search`, `read_file`, `view_file`, `write_file`, `list_dir`, and `doc_reader`) to ensure file names and query details print successfully in the terminal.
    *   Fixed TUI friendly name duplicate suffixing (e.g. `● Grep Search Search` deduplicated to `● Grep Search`).
*   **Feature: Subagent Visual Image Delegation & Instant TUI Cancellation (HIGH)**:
    *   Implemented standard, non-blocking **asynchronous SIGINT / Ctrl+C cancellation** inside [cli/mod.rs](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/cli/mod.rs#L741-L770), replacing the blocking background event polling thread and preventing lock contention over standard input.
    *   Implemented **Automatic Image Path Scanning & Markdown Linking** in `DelegateTaskTool` and `DelegateProfileTool` to resolve subagent blindness. Subagents now automatically receive image attachments parsed from paths in their goal or context text, enabling seamless visual task delegation to vision models like Gemini 2.5 Flash on the very first turn.
*   **Documentation & Guidelines (MEDIUM)**:
    *   Authored a workspace-level procedural tools selection, safety, and error recovery guide in [skills/tool_usage_guide.md](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/skills/tool_usage_guide.md) and [onpkg_docs/tool_usage_guide.md](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/onpkg_docs/tool_usage_guide.md).
    *   Bumped project and Cargo workspace package version to `v0.0.37`.

### v0.0.36 (Previous Release)
*   **Feature: System Diagnostics, Session Management, and Safe Test Suites (HIGH)**:
    *   Implemented `DiagnoseSystemTool` (`diagnose_system`) to profile OpenZ storage directories (`sessions/`, `tool_outputs/`, `traces/`, and `skills/`), check SQLite database connectability, size, and integrity (`PRAGMA integrity_check;`) for all 5 databases, and test LLM provider endpoint request latency.
    *   Implemented `ManageSessionsTool` (`manage_sessions`) to list active session files with message counts/sizes, prune old temporary tool outputs to prevent disk exhaustion, archive sessions, or permanently delete session files.
    *   Created a safe test execution script `run_tests_safely.sh` that compiles with restricted CPU cores and runs tests sequentially by module to prevent memory exhaustion and laptop crashes on developer machines.
    *   Cleaned up corrupted Cargo dependency directory caches.
    *   Bumped framework version to `v0.0.36` across Cargo, README, and `onpkg.json`.

### v0.0.35 (Previous Release)
*   **Feature: Real-Time Configuration Dynamic Reloading & Management (HIGH)**:
    *   Dynamic reload of configuration (`config.json`) at the start of each turn loop iteration inside `AgentLoop::run_inner` synced via `TurnContext`.
    *   Implemented `ManageConfigTool` (`manage_config`) to view the active configuration with recursive secret key redaction (`api_key`, `bot_token`, `verify_token`, `password`, `secret`) and update agent hyper-parameters (`model`, `provider`, `max_tokens`, `temperature`, `caveman_mode`, `tool_timeout_secs`, `streaming`, `max_tool_iterations`).
    *   Added dedicated unit tests verifying that configuration viewing, secret redaction, and hyper-parameter updates behave correctly.
    *   Bumped framework version to `v0.0.35` across Cargo, README, and `onpkg.json`.

### v0.0.34 (Previous Release)
*   **Feature: Self-Management & Self-Healing Toolkit (HIGH)**:
    *   Implemented `DiagnoseToolTool` (`diagnose_tool`) to allow the agent to test any native tool in the registry, check schema validity, measure execution latency, and capture standard errors.
    *   Implemented `CurateSkillTool` (`curate_skill`) to allow the agent to CRUD its own guidelines dynamically in the SQLite database skills store.
    *   Implemented `OptimizeToolScopeTool` (`optimize_tool_scope`) to allow the agent to filter the set of active tool schemas exposed to the LLM by prefix, saving prompt tokens and avoiding hallucinations.
    *   Exposed dynamic prefix filtering in `ToolRegistry` across `get`, `get_static_tools`, and `to_openai_format` methods.
    *   Added dedicated unit tests verifying that all three self-management tools and scope filters behave correctly.
    *   Bumped framework version to `v0.0.34` across Cargo, README, and `onpkg.json`.

### v0.0.33 (Previous Release)
*   **Feature: Native Integration of GitHub & Docs MCP Servers (HIGH)**:
    *   Ported the entire `openz_github_mcp` and `openz_docs_mcp` packages directly into the workspace under `tools/openz-github` and `tools/openz-docs`.
    *   Converted both codebases to standard Cargo library targets (`lib.rs`) and removed standalone executable entry points.
    *   Exposed 3 GitHub management tools under the `github_` prefix (e.g. `github_create_pull_request`, `github_search_issues`, `github_get_issue_comments`) using re-exported `octocrab` client integration.
    *   Exposed 6 local and remote documentation search/caching tools under the `docs_` prefix (e.g. `docs_list_docsets`, `docs_install_docset`, `docs_search_docs`, `docs_read_doc_page`, `docs_search_rust_crate`, `docs_read_rust_docs`).
    *   Registered all 9 new tools natively inside `build_agent_loop` in [src/cli/builder.rs](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/builder.rs).
    *   Wrote unit tests verifying client and database schema initialization.
*   **Prompting & Version Update**:
    *   Added `github_` and `docs_` tool prefixes to the system guidelines prompt list in [src/agent/agent_loop/build.rs](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/build.rs).
    *   Bumped framework version to `v0.0.33` across Cargo, README, and `onpkg.json`.

### v0.0.32 (Previous Release)
*   **Feature: Integrated OpenDoc-MCP Native Tool Port (HIGH)**:
    *   Ported the entire `opendoc-mcp` codebase into the workspace under [tools/opendoc/](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/tools/opendoc) to enable off-grid document intelligence.
    *   Upgraded the workspace rust toolchain to `stable` to cleanly resolve dependencies (like edition 2024 dependencies).
    *   Updated the visibility of all 36 document creation, extraction, diffing, and templating methods inside [tools/opendoc/src/server.rs](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/tools/opendoc/src/server.rs) to `pub fn`.
    *   Wrapped all 36 tools natively under the `opendoc_` prefix in [src/tools/opendoc/mod.rs](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/opendoc/mod.rs).
    *   Registered the tools inside `build_agent_loop` in [src/cli/builder.rs](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/builder.rs).
    *   Wrote unit tests verifying initialization of the opendoc server and basic OCR availability check.
*   **Refactor: SearchXyz Modularization (MEDIUM)**:
    *   Split the `searchxyz` monolithic wrapper into separate submodules (`web`, `index`, `graph`) under [tools/searchxyz/](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/tools/searchxyz) to follow standard codebase modularization guidelines.
*   **Prompting & Documentation Update**:
    *   Added all `opendoc_` tools to the agent's core system prompt guidelines in [src/agent/agent_loop/build.rs](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/build.rs).
    *   Bumped the project version to `v0.0.32`.

### v0.0.31 (Previous Release)
*   **Feature: Integrated OpenMedia-RS Crate Workspace (HIGH)**:
    *   Migrated all 8 crates of `openmedia-rs` directly into the `openz` workspace under [tools/openmedia](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/tools/openmedia), making `openz` fully portable and self-contained.
    *   Exposed all 37 OpenMedia tools natively under the `openmedia_` prefix, including `openmedia_create_chart` (charts), `openmedia_video_create` (DSL video composition), `openmedia_animate_svg` (SMIL layout animations), and `openmedia_improve_score_image` (aesthetics prompt alignment).
    *   Constructed a thread-safe static `OnceLock` singleton wrapper in [src/tools/openmedia/mod.rs](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/openmedia/mod.rs) for in-memory execution without JSON-RPC stdio overhead.
    *   Used `schemars::schema_for!` to automatically generate and validate parameters JSON schemas from Rust structs.
*   **Workspace Manifest Consolidation**:
    *   Added workspace members and workspace dependencies tables to `openz` root `Cargo.toml`.
    *   Registered and validated the `openmedia-core` and `openmedia-mcp` crates via standard Cargo workspace dependency inheritance (`workspace = true`).
*   **Testing & Prompting Integration**:
    *   Added `openmedia_` tools to the agent's core system prompt template guidelines in [src/agent/agent_loop/build.rs](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/build.rs).
    *   Wrote `test_openmedia_server_ping` unit tests verifying singleton server status.
    *   Bumped version to `v0.0.31`. All 204 tests passing cleanly.

### v0.0.30 (Previous Release)
*   **Feature: Integrated SearchXyz Tool Suite (HIGH)**:
    *   Fully integrated the `searchxyz` crate into the Cargo workspace.
    *   Wrapped all 15 tools from `searchxyz` as native OpenZ tools, prefix-registered under `searchxyz_`:
        *   `searchxyz_search_web`: Web search dispatcher.
        *   `searchxyz_read_url`: Document/media/git repo parser.
        *   `searchxyz_search_and_read`: Multi-step web search and crawl.
        *   `searchxyz_recall`: Semantic & keyword lookup.
        *   `searchxyz_list_sources`: Local document source lister.
        *   `searchxyz_deep_research`: Recursive multi-query crawler & report compiler.
        *   `searchxyz_index_content`: Custom text manual indexer.
        *   `searchxyz_site_map`: Fast sitemap/tree crawl discovery.
        *   `searchxyz_index_relationship`: Knowledge Graph node insertion.
        *   `searchxyz_query_graph`: Graph query & traversal.
        *   `searchxyz_read_github_repo`: Repository codebase cloner & indexer.
        *   `searchxyz_export_research`: Portable JSON metrics/doc exporter.
        *   `searchxyz_import_research`: JSON bundle importer.
        *   `searchxyz_delete_source`: URL-based source eviction.
        *   `searchxyz_clear_index`: Clear all documents and Graph memory.
*   **Search Integration (HIGH)**:
    *   Integrated `SearchXyz`'s federated `SearchDispatcher` into `WebSearchTool` (`web_search`) as the primary web search engine.
    *   Preserved original search providers (Tavily, Exa, DuckDuckGo scraper, Mojeek scraper) as robust automatic fallbacks in case `SearchXyz` encounters network errors or is unconfigured.
*   **Maintenance & Testing**:
    *   Added `rmcp` and `schemars` dependencies to the workspace root `Cargo.toml`.
    *   Wiped local package/registry cache to resolve resolving and download contentions.
    *   Implemented `test_searchxyz_tools_metadata` unit tests verifying wrapper registry.
    *   Bumped version to `v0.0.30`. All 202 native tests and 38 integrated `searchxyz` tests passing.

### v0.0.29 (Latest Release)
*   **Security: SSRF & timing attack mitigations (HIGH)**:
    *   Implemented constant-time WhatsApp HMAC signature validation in `src/channels/whatsapp.rs` to protect webhook endpoints from timing attacks.
    *   Added WebSocket frame size and message limits (16MB) in `src/channels/websocket.rs` to prevent DoS attacks.
    *   Implemented HTTP chunked response body limits (10MB) in `src/tools/web.rs` to block memory exhaustion.
    *   Introduced IP pinning for SSRF validation in `src/tools/web.rs` to eliminate the DNS-rebinding TOCTOU race window.
*   **Database: Hardened shared memory and graph databases (HIGH)**:
    *   Replaced per-call connection patterns in `shared_memory` with a thread-safe singleton connection using WAL mode and a 5s busy timeout.
    *   Unified branch and main schema DDLs in `graph_memory` to eliminate schema corruption.
    *   Established deadlock-free lock ordering (`db_static()` lock before `BRANCH_MUTEX`).
    *   Optimized queries with `LIMIT` clauses and capped the O(n²) consolidation to the newest 200 entries.
    *   Added eviction limits to in-memory fallback stores to prevent memory leaks.
*   **Reliability & Channel Enhancements (HIGH)**:
    *   Registered global panic hook to restore raw terminal mode and exit alternate screens cleanly.
    *   Redacted Discord bot tokens in error logs.
    *   Added concurrency semaphores to WhatsApp webhook axum handlers.
    *   Updated atomic ordering to prevent race conditions in Discord gateway heartbeat routines.
*   **Performance: Async I/O and System Prompt budgets (HIGH)**:
    *   Migrated hot-path filesystem operations in `run.rs` and `session.rs` to async `tokio::fs` or `spawn_blocking`.
    *   Optimized save frequency to write session incrementally every 5 iterations.
    *   Added character/token budget caps (32k) for system prompts to prevent context token overflow.
    *   Implemented path traversal constraints in filesystem tools to prevent unauthorized file access.
*   **Maintenance: Code Cleanups**:
    *   Removed `AgentError` dead code and resolved 13 unused compiler warnings across the repository.
    *   Bumped version to v0.0.29. All 201 unit tests passing cleanly.

### v0.0.28
*   **Refactor: Codebase Modularization (MEGA):** Modularized all remaining monolithic files into cleanly structured, package-based submodules:
    *   Split CLI raw terminal input/render/channel loop (`src/channels/cli.rs`) into [src/channels/cli/](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/channels/cli/).
    *   Split core agent loop state machine (`src/agent/agent_loop.rs`) into [src/agent/agent_loop/](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop/).
    *   Split CLI subcommands & configuration menus (`src/cli.rs`) into [src/cli/](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/cli/).
    *   Split memory extra tools (`src/tools/memory_extra.rs`) into [src/tools/memory_extra/](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/memory_extra/).
    *   Split headroom compression tools (`src/tools/headroom.rs`) into [src/tools/headroom/](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/headroom/).
    *   Split shared memory tools (`src/tools/shared_memory.rs`) into [src/tools/shared_memory/](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/shared_memory/).
    *   Split sequential thinking tools (`src/tools/sequential_thinking.rs`) into [src/tools/sequential_thinking/](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/sequential_thinking/).
    *   Split graph memory tools (`src/tools/graph_memory.rs`) into [src/tools/graph_memory/](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/graph_memory/).
*   **Docs: Repository Architecture Update (MEDIUM):** Updated architecture and tools documentation to match modular packages layout.
*   **Maintenance: Version Bump:** Bumped to v0.0.28. All 200 unit tests passing sequentially.

### v0.0.27
*   **Feat: Cross-Session Memory Persistence (HIGH):** Implemented automatic persistence of user/project facts and observations. The background self-improvement curator now parses extracted facts from markdown and stores them permanently into the SQLite database. Added automatic retrieval in `TurnState::Build` that queries all active semantic facts and graph node observations across past sessions and injects them dynamically into the system prompt. Enabled fact-sharing keyword checks to trigger curators even on simple/short turns.
*   **Fix: Test Isolation & Test Path Caching (MEDIUM):** Isolated `cargo test` database paths to temporary files (`openz_test_graph_memory_<uuid>.db`) and cached the path via a `OnceLock` within the test process. Corrected mathematical expectation in `test_text_similarity` to match Jaccard word-overlap math. All 198 tests now compile and pass cleanly.
*   **Refactor: MCP-to-Native Tool Port — Sequential Thinking, Memory, Headroom (MEGA):** Ported all 67 tools from 3 external MCP servers to native Rust implementations across 4 new files (`sequential_thinking.rs`: 5 tools, `headroom.rs`: 19 tools, `graph_memory.rs`: 12 tools, `memory_extra.rs`: 31 tools). Eliminates external binary spawns, JSON-RPC overhead, and stdio polling for these servers. Compilation is clean (0 new warnings), 198/198 tests pass.
*   **Fix: Dual SQLite Connection Elimination (HIGH):** Both `graph_memory.rs` and `memory_extra.rs` now share a single `OnceLock<Mutex<Connection>>` via `pub(crate) with_db()`. Removed ~170 lines of duplicated DB infrastructure from `memory_extra.rs` (`db_static()`, `init_db()`, `get_db_path()`, `with_db()`, `scope_from_args`). All table DDL merged into `graph_memory::init_db()` — eliminates `SQLITE_BUSY` errors from concurrent connections.
*   **Fix: Name Collision Resolution (MEDIUM):** Renamed `ast_grep::IndexCodebaseTool` to `AstGrepIndexCodebaseTool` (tool name: `ast_grep_index_codebase`) to avoid collision with `memory_extra::IndexCodebaseTool`.
*   **Config: MCP Server Pruning (MEDIUM):** Removed 5 MCP servers from `~/.openz/config.json` and `config/schema.rs` defaults: `sequential-thinking`, `memory`, `headroom`, `database`, `context-bus`. `database-mcp` was duplicated by native `DbInspectorTool`/`DbWriteTool`; `context-bus-mcp` had no native equivalent and was removed at user request.
*   **Maintenance: Version Bump:** Bumped to v0.0.27. All 67 native tools registered with zero orphans, zero name collisions.

### v0.0.26
*   **Feat: Official Repository Awareness (HIGH):** Updated core system prompt guidelines in `src/agent/agent_loop.rs` to make the agent explicitly aware of its official GitHub repository and source code at `https://github.com/aswin402/openz-rs` for advanced self-querying.
*   **Feat: Indented and Aligned Monologue Formatting (MEDIUM):** Redesigned thought/reasoning blocks in the TUI to wrap paragraphs dynamically according to the active terminal width. The tree connector (`  L `) is printed only on the very first line of a thought, and subsequent paragraphs/wrapped lines are space-padded to align neatly under the start.
*   **Style: Custom Color System Update (MEDIUM):** Updated global theme colors in `src/agent/style/colors.rs`: AURA_PURPLE is set to `#6F00FF`, AURA_GREEN to `#00FF00`, and error/fail reds to `#FF0000`. Original `EMERALD_GREEN` was restored.
*   **Fix: Duplicated Tool Name Display (MEDIUM):** Introduced a clean extraction parser `clean_tool_args_msg` that prevents friendly tool names from duplicating start message outputs (e.g. converting `● Web Search WebSearch` to just `● Web Search`).
*   **Maintenance: Version Bump:** Bumped to v0.0.26.

### v0.0.25
*   **Feat: Structured Live Log Visualizer (HIGH):** Redesigned the terminal-based log follow screen in `openz logs` ([`src/logs.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/logs.rs)) to output high-fidelity trace representations with customized semantic icons, colors, and bold labels representing different workflow events:
    * 👤 `[USER]` — Human prompt message (Cyan).
    * 🧠 `[THINKING]` — Model reasoning/thought tokens (Orange).
    * 📡 `[LLM CALL]` — Outgoing model invocation requests (Slate).
    * 🤖 `[RESPONSE]` — Model completions output (White).
    * 🛠️ `[TOOL START]` / `[TOOL DONE]` / `[TOOL FAIL]` — Full tool lifecycle tracking with clean arguments parsing and return statuses (Gold/Green/Rose).
    * 🤖 `[SUBAGENT START]` / `[SUBAGENT DONE]` / `[SUBAGENT FAIL]` — Correlated trace tracking of child agent delegations (Purple/Green/Rose).
    * 🛡️ `[BLOCKED]` — Commands intercepted by the SecurityGuard or user denials (Gold).
    * 🧹 `[CURATOR]` — Progress logs for the background self-improvement curator (Purple).
    * 💾 `[SAVED]` / `🗜️ [COMPACT]` — History compaction and database transaction saving (Green/Slate).
*   **Reliability: Comprehensive Execution Tracing (HIGH):** Instrumented the core `AgentLoop` state machine ([`src/agent/agent_loop.rs`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/agent/agent_loop.rs)) with detailed tracing statements covering context compaction, LLM completions, tool executions, security approvals, and background curator tasks to ensure all developer actions are fully visible in the live logs stream.
*   **Maintenance: Version Bump:** Bumped to v0.0.25.

### v0.0.24
*   **Fix: Tokio TcpListener from_std Panic in gRPC MCP Bridge (HIGH):** Resolved a Tokio runtime panic during stdio-based MCP bridge startup by explicitly invoking `port_guard.set_nonblocking(true)?` before converting the standard socket to a Tokio listener via `TcpListener::from_std`.
*   **Fix: Direct gRPC Server Startup Connection (HIGH):** Replaced the static 500ms sleep and single connect attempt for direct gRPC servers (such as `openmemory_rs` on port 50051) with a robust 20-attempt retry loop (sleeping 150ms between retries, up to 3 seconds total) to prevent early `Connection refused (os error 111)` failures on heavier startup routines.
*   **Maintenance: Compiled Workspace Binaries:** Recompiled and resolved path routing for local workspace subprojects `openmemory_rs`, `mcp-server-sequential-thinking`, and `context-bus-mcp` to ensure all 10 enabled MCP servers initialize successfully.
*   **Maintenance: Version Bump:** Bumped to v0.0.24. All 125 tests passing sequentially.

### v0.0.23
*   **Fix: LLM Parameter Mapping for Non-OpenAI Reasoning Models (HIGH):** Modified request payload logic to exclude non-OpenAI models like DeepSeek V4/R1, QwQ, etc. from `max_completion_tokens` parameter routing. They are now queried using standard `temperature` and `max_tokens` parameters, which prevents completion token starvation and resolves early truncations / cutoffs on OpenAI-compatible gateways (like OpenCode Zen).
*   **Feat: CLI Response Streaming Toggle Wizard (MEDIUM):** Implemented a new global CLI subcommand `openz streaming` that runs an interactive terminal menu/wizard. This allows users to easily enable or disable response streaming globally for default agent configurations without manually editing files.
*   **Maintenance: Version Bump:** Bumped to v0.0.23. All 124 tests passing sequentially.

### v0.0.22
*   **MCP Server Health Monitoring (HIGH):** Implemented a background monitoring task (`start_mcp_health_checks`) running every 30 seconds to monitor spawned MCP servers via lightweight `"tools/list"` ping calls. Seamlessly handles connection drop detection, invalidates stale connections, performs background auto-reconnections, and emits warning/recovery notifications across CLI and WebSocket channels.
*   **Git Integration Tool (HIGH):** Created a native `git_provider` tool in `src/tools/github.rs` to interact with GitHub and GitLab API endpoints natively using `reqwest`. Supports creating pull requests (`create_pr`), listing issues (`list_issues`), searching repository code (`search_code`), and fetching PR/MR diff contents (`get_pr_diff`) without calling external shell processes.
*   **Security: Git Base URL SSRF Hardening (HIGH):** Hardened the `git_provider` tool against Server-Side Request Forgery (SSRF) by validating `api_base` values against standard IP checking rules and applying an IP-restricted redirect client policy.
*   **Maintenance: Version Bump:** Bumped to v0.0.22. All 124 tests passing sequentially.

### v0.0.21
*   **Reliability: Tool Timeout Enforcement (HIGH):** Wrapped all tool calls in `tokio::time::timeout` using the configured `tool_timeout_secs` to prevent infinite hangs from unresponsive tools or external subprocesses.
*   **Resilience: Graceful Shutdown Coordination (HIGH):** Implemented a global shutdown token and registered SIGTERM/SIGINT signal handlers in `main.rs`. Seamlessly integrated the sync terminal raw-mode input loop in `CliChannel` to exit raw mode cleanly on shutdown, avoiding terminal formatting issues.
*   **Safety: Cross-Process Session File Locking (HIGH):** Added file-based advisory locking using the `fs2` crate in `SessionManager`. Prevents multiple concurrent `openz` processes from using the same session file and causing data corruption.
*   **Performance: Real-time Response Streaming (HIGH):** Added `chat_stream` support to `LLMProvider` and implemented SSE chunk parsing in `OpenAIProvider`. Wired up response streaming directly in `AgentLoop`'s Run state for CLI and WebSocket channels, yielding text delta-by-delta while maintaining full tool-call accumulation capability.
*   **Maintenance: Version Bump:** Bumped to v0.0.21. All 121 tests passing sequentially, 0 clippy warnings.

### v0.0.20
*   **Security: Default-Deny Gateway Auth (CRITICAL):** Changed authorization to default-reject gateway requests if `OPENZ_GATEWAY_TOKEN` is unset or empty. Added timing-attack protection by hashing tokens with SHA-256 before comparing them in constant-time.
*   **Security: SSRF Redirect & IPv6 Hardening (CRITICAL):** Configured `WebFetchTool`'s reqwest client with a custom redirect policy validating every hop against `validate_url_sync`. Hardened IPv6 network detection to block loopback, unspecified, multicast, Unique Local Addresses (ULA), link-local unicast, and IPv4-mapped IPv6 addresses.
*   **Security: SQL Injection CLI Spawning Elimination (CRITICAL):** Replaced all subprocess shell-outs to the `sqlite3` CLI process in `DbInspectorTool` and `DbWriteTool` with an in-process integration using the `rusqlite` crate, completely eliminating shell/argument injections and CLI dot-command executions.
*   **Maintenance: Version Bump:** Bumped to v0.0.20. All 121 tests passing, 0 clippy warnings.

### v0.0.19
*   **Security: Subagent Loopback Isolation:** Excluded `SendRemoteInputTool` (`send_remote_input`) from dynamically constructed subagent tool lists (`delegate_task`, `parallel_research`, `evaluator_optimizer_loop`, and custom profiles) to prevent loopback command/prompt injection from nested child loops.
*   **Refactor: Subagent Tool Filtering:** Added a secondary restriction in `filter_tools_for_subagent` to strip out `send_remote_input` from all allowed profile lists.
*   **Maintenance: Version Bump:** Bumped to v0.0.19. All 118 tests passing, 0 clippy warnings.

### v0.0.18
*   **Bugfix: Discord Sequence & Heartbeat Tracking:** Renamed `_s` to `s` in `GatewayMessage` to correctly deserialize Discord's sequence numbers, and populated the sequence tracker inside the background heartbeat payload to prevent prolonged session disconnections.
*   **Refactor: Raw-Mode Output & Custom Error Stream:** Added raw-mode compatible `tui_eprintln!` and `tui_eprint!` helpers to prevent line formatting corruption when writing to `stderr`. Updated gateway shutdown logs to use `tui_println!`.
*   **Refactor: Regex Pre-compilation & Caching:** Swapped inline Regex compilations inside terminal formatting functions with static precompiled `OnceLock` instances.
*   **Bugfix: Vision Model Matching & Coverage:** Explicitly whitelisted `gpt-4-turbo` and `gpt-4-vision-preview` in `model_supports_vision()`.
*   **Refactor: Tool Registry Determinism & Profile Cache:** Added alphabetical sorting by function name to the registered tools list to stabilize the system prompt. Added a thread-safe modification time (mtime) cache to `load_profiles()` to optimize dynamic subagent resolution checks.
*   **Bugfix: Subagent Task Scoping:** Scoped `ACTIVE_WORKSPACE` and `DELEGATION_DEPTH` task-local variables inside `ParallelResearchTool` spawned tasks.
*   **Refactor: Logging Comment Strip Protection:** Prevented stripping URLs (`http://`, `https://`) in context compaction by ignoring double slashes preceded by colons.
*   **Security: Hardened Session Hash Chains:** Checked for hash presence inside `verify_hash_chain()` to block tampering via stripping the message validation hashes.
*   **Bugfix: Cargo.toml Inline Table Dependency Parser:** Fixed parsing of `[dependencies.foo]` tables in `onpkg` package scanner to ignore subproperties like `version` or `features`.
*   **Maintenance: Version Bump:** Bumped to v0.0.18. All 118 tests passing, 0 clippy warnings.

### v0.0.17
*   **Refactor: MCP Dual Cache Consolidation:** Removed `LAZY_MCP_CLIENTS` cache, consolidated to single `SPAWNED_MCP_CLIENTS`. `LazyMcpToolWrapper::call()` now delegates to `McpClient::spawn()` which handles both fast and slow paths. Eliminates first-call cache miss.
*   **Cleanup: Dead Code Removal:** Removed `McpClientType::Stdio` variant and all associated match arms (~60 lines of unreachable code). All spawns use gRPC exclusively.
*   **Bugfix: find_free_port TOCTOU Race:** `find_free_port()` now returns a bound `TcpListener` guard. The listener is passed to `run_mcp_bridge()` and only dropped right before `tonic::Server::serve()` binds, shrinking the race window from ~100ms to <1µs.
*   **Bugfix: Bridge Child Process Monitoring:** Added `child_exit` monitor task in `tokio::select!` inside `run_mcp_bridge()`. If the stdio child crashes, the gRPC bridge shuts down instead of returning stale errors.
*   **Bugfix: Stderr Reader Cancellation:** Reader and stderr forwarding tasks are now aborted via `.abort()` on bridge shutdown, preventing orphaned tasks.
*   **Test: MCP Unit Tests:** Added 4 tests for `find_free_port()` (race-free listener binding behavior) and `McpClient::invalidate()` (cache entry lifecycle).
*   **Maintenance: Version Bump:** Bumped to v0.0.17. All 118 tests passing, 0 clippy warnings.

### v0.0.16
*   **Security: SSRF DNS Rebinding Defense (CRITICAL):** Replaced string-only URL validation in `web_fetch` with DNS resolution checks. After validating URL syntax and hostname patterns, resolves the hostname to IP addresses via `tokio::task::spawn_blocking` + `ToSocketAddrs`, then verifies all resolved IPs are safe (not private, loopback, link-local, unspecified, broadcast, or multicast). Prevents DNS rebinding attacks where a malicious DNS server returns a private IP after initial validation.
*   **Security: SSRF Protection for Crawler (CRITICAL):** Added the same `validate_url()` and `is_safe_ip()` functions to `CrawlSiteTool` in `crawl.rs`. Blocks crawling of internal/private endpoints before `Website::new()` is called.
*   **Security: Port Scanner Restriction (CRITICAL):** Added localhost-only restriction to `CheckPortTool` in `network.rs`. Only allows `127.0.0.1`, `localhost`, `::1`, `[::1]`. If a non-allowed host is given, resolves it and checks if any resolved IP is loopback. Prevents internal network enumeration.
*   **Security: Command Injection in xdg-open (CRITICAL):** Added shell metacharacter validation for URLs in `open.rs`. Blocks `;`, `|`, `&`, `$`, `` ` ``, `\n` characters. Separated URL and file path handling paths, both using `tokio::task::spawn_blocking`.
*   **Security: SQL Injection Enhancement (HIGH):** Expanded SQL injection defense in `db_inspector.rs` with Unicode confusable normalization (zero-width chars, fullwidth digits), additional blocked keywords (`UNION`, `EXCEPT`, `INTERSECT`, `LOAD`, `OVERWRITE`, `CALL`, `EXECUTE`, `HAVING`, `GROUPBY`, `ORDERBY`), semicolon blocking (allows trailing `;` only), and SQL comment blocking (`--`, `/*`).
*   **Bugfix: File Size Guard (HIGH):** Added 50MB file size limit on `ReadFileTool` in `filesystem.rs`. Returns error with guidance to use line ranges for large files.
*   **Bugfix: DOCX Size Limit + Table Recursion (HIGH):** Added 50MB limit on DOCX file reads in `doc_reader.rs`. Added depth parameter to `extract_table()` with `MAX_TABLE_DEPTH = 20` guard to prevent stack overflow from deeply nested tables.
*   **Bugfix: Blocking I/O in ast_grep (MEDIUM):** Wrapped `Command::output()` in `tokio::task::spawn_blocking` in `ast_grep.rs` to prevent blocking the tokio runtime thread pool. Also fixed clippy redundant closure warning.
*   **Bugfix: Blocking I/O in git_manager (MEDIUM):** Wrapped `cmd.output()` in `tokio::task::spawn_blocking` in `git_manager.rs` to prevent blocking the tokio runtime thread pool.
*   **Bugfix: Blocking I/O in system_info (MEDIUM):** Wrapped all 7+ `Command::output()` calls in `tokio::task::spawn_blocking` in `system_info.rs` to prevent blocking the tokio runtime thread pool.
*   **Bugfix: Cron Serialization Panic (MEDIUM):** Changed `.unwrap()` to `.filter_map(|j| serde_json::to_value(j).ok())` in `cron.rs` to prevent panics on serialization failures.
*   **Bugfix: Outline String Slicing Panic (MEDIUM):** Added bounds checking on all 4 visitor methods in `outline.rs` (`visit_function`, `visit_class`, `visit_ts_interface_declaration`, `visit_ts_type_alias_declaration`). Changed `self.source_text[..start]` to safe conditional with length check.
*   **Bugfix: Batch Insert Performance (MEDIUM):** Wrapped the INSERT loop in `notes.rs` in a SQLite transaction (`BEGIN TRANSACTION` / `COMMIT`) for batch insert performance.
*   **Bugfix: LLM Code Backup (MEDIUM):** Added `.bak` backup creation before writing LLM-generated code in `cargo_manager.rs`. On write failure, restores from backup.
*   **Security: Chrome Flags Hardening (CRITICAL):** Removed `--disable-web-security` and `--allow-file-access-from-files` from `image_generator.rs` and `html_video.rs` for consistency with `obscura.rs`.
*   **Security: API Key Redaction (HIGH):** Removed raw tool call argument logging from `openai.rs` to prevent API key leakage in debug logs.
*   **Security: MCP Server Removal Confirmation (HIGH):** Added `confirm` parameter requirement for `manage_mcp` remove action to prevent accidental bulk deletion.
*   **Bugfix: Dead Social Search Backends (HIGH):** Replaced dead Nitter and Invidious instances in `social_search.rs`. Twitter search now returns clear error. YouTube search uses direct scraping fallback. Added error propagation in `search_all`.
*   **Bugfix: Reddit Rate-Limit Handling (HIGH):** Added retry logic with exponential backoff for HTTP 429 responses in Reddit search.
*   **Bugfix: Shared Memory DB Corruption Recovery (HIGH):** Added `PRAGMA journal_mode=WAL`, integrity check, and automatic recovery (rename corrupt DB, recreate) in `shared_memory.rs`.
*   **Bugfix: Cron Job Timing (MEDIUM):** Fixed `last_run` being set before execution — now set after completion. `next_run` calculated from actual completion time.
*   **Bugfix: MCP Stale Client Recovery (MEDIUM):** Added `clear_memory_mcp_client()` and retry logic in `LazyMcpToolWrapper::call()` to reconnect when MCP server crashes.
*   **Bugfix: Obscura Tab Leak (MEDIUM):** Restructured `call()` to ensure tab is always closed via scope guard pattern, even on error.
*   **Bugfix: Obscura CDP Timeout (MEDIUM):** Added 30-second timeout to `send_cdp_cmd()` to prevent infinite hang on browser crash.
*   **Bugfix: Crawl Empty Results (MEDIUM):** Added error when crawl returns zero results instead of silently returning empty array.
*   **Bugfix: DDG Search Fallback Logging (LOW):** Added warning log when DuckDuckGo scraping fails before falling back to Mojeek.
*   **Maintenance: Version Bump:** Bumped to v0.0.16. All 114 tests passing, 0 clippy warnings.

### v0.0.15
*   **Security: SQL Injection Defense (CRITICAL):** Replaced trivially-bypassable keyword blocklist in `DbInspectorTool` with comprehensive SQL injection defense: normalized whitespace removal, blocklist of dangerous SQL keywords (INSERT, UPDATE, DELETE, DROP, ALTER, CREATE, ATTACH, DETACH, PRAGMA, etc.), blocklist of sqlite3 dot-commands (.shell, .import, .output, .read, .system), and whitelist requiring queries start with SELECT or EXPLAIN.
*   **Security: Shell Command Allowlist (CRITICAL):** Added compile command allowlist validation to `CompilerAutoHealTool` (cargo, rustc, gcc, clang, make, npm, python, etc.), enforced `max_iterations` cap of 5, added backup file creation before AI-generated overwrites.
*   **Security: SSRF Prevention (CRITICAL):** Added `validate_url()` to `web_fetch` blocking localhost, loopback, cloud metadata endpoints (169.254.169.254), private/reserved IP ranges, and non-HTTP schemes. Restricted `rust_docs` `sub_path` to only `https://docs.rs/` or `https://crates.io/` URLs.
*   **Security: WhatsApp Webhook Signature Verification (CRITICAL):** Added HMAC-SHA256 signature verification using `X-Hub-Signature-256` header. Reads `WHATSAPP_APP_SECRET` env var; returns 403 on invalid signatures when configured.
*   **Security: CORS Hardening (CRITICAL):** Replaced `allow_origin(Any)` in WebSocket gateway with explicit localhost origins (localhost, 127.0.0.1 on ports 3000/8765). Restricted methods to GET, POST, OPTIONS.
*   **Security: Hardcoded Path Removal (CRITICAL):** Replaced hardcoded `AI_AGENT_TOOLS_BASE` and `PARENT_WORKSPACE_TARGET` constants with functions reading from `AI_AGENT_TOOLS_BASE` and `OPENZ_WORKSPACE_TARGET` env vars, falling back to `dirs::home_dir()`-based defaults.
*   **Security: Browser Flags Hardening (CRITICAL):** Removed `--allow-file-access-from-files` and `--disable-web-security` Chrome flags from `ObscuraBrowserTool`.
*   **Security: JS Injection Fix (CRITICAL):** Replaced naive selector escaping in `GenerateImageTool` with comprehensive escaping for `\`, `"`, `'`, `\n`, `\r`. Restructured JS to pass selector as function argument.
*   **Security: Unsafe `env::set_var` (CRITICAL):** Wrapped `set_var("OPENZ_SILENT")` in `unsafe` block with explanation (safe because it runs before spawning threads).
*   **Security: IMAP TLS (CRITICAL):** Restored `imap::ClientBuilder::new().connect()` (the `imap` crate with `rustls-tls` feature handles TLS automatically).
*   **Bugfix: UTF-8 Panics (HIGH):** Fixed 4+ byte-slicing panic locations: `social_search.rs` (selftext and YouTube snippet), `agent_loop.rs` (tool args and message truncation), `menu.rs` (display title) — all now use `.chars().count()` and `.chars().take().collect()`.
*   **Bugfix: Unbounded Disk Usage (HIGH):** Added `cleanup_old_files()` to `AgentLoop` that deletes files older than 7 days in `~/.openz/traces/` and `~/.openz/tool_outputs/`. Called at start of each turn.
*   **Bugfix: Mutex Poisoning Panics (HIGH):** Changed all 4 `.lock().unwrap()` in `watcher.rs` to `.lock().unwrap_or_else(|e| e.into_inner())` to recover from poisoned mutexes.
*   **Bugfix: NaN Panic (HIGH):** Changed `.partial_cmp().unwrap()` to `.unwrap_or(Ordering::Equal)` in `semantic_search.rs`.
*   **Bugfix: SOP Crash (HIGH):** Replaced `.expect()` with proper error propagation in `sop/engine.rs` for malformed context steps.
*   **Bugfix: Template Recursion Stack Overflow (HIGH):** Added `MAX_TEMPLATE_DEPTH = 10` limit to `template_compiler.rs` recursive rendering.
*   **Bugfix: Security Bypass in Loose Mode (HIGH):** Pipe-to-shell blocking (`| sh`, `| bash`, `| python`) now enforced in ALL modes, not just strict mode.
*   **Bugfix: Activity File Race Condition (HIGH):** Replaced direct `fs::write` with atomic write (temp file + rename) in `activity.rs` to prevent partial reads from concurrent sessions.
*   **Bugfix: Ollama Double-Spawn Race (HIGH):** Combined port check and child guard check into single lock scope in `ollama_manager.rs` to eliminate TOCTOU window.
*   **Bugfix: Silent Error Swallowing (MEDIUM):** Added `eprintln!` for directory creation failures in `main.rs`, replaced nested `if let Ok` with `match` blocks logging via `tracing::warn!` in `activity.rs`.
*   **Bugfix: Vision Model False Positives (MEDIUM):** Changed `m.contains("o1")` to `m.starts_with("o1")` and `m.contains("o3")` to `m.starts_with("o3")` in `model_supports_vision()`.
*   **Bugfix: MockProvider Atomic Race (MEDIUM):** Replaced load-then-store with `fetch_update` using `Ordering::SeqCst` for thread-safe error injection counter.
*   **Bugfix: Unbounded Crawl Parameters (MEDIUM):** Added `.min(1000)` to limit, `.min(10)` to depth, `.max(50)` to delay in `CrawlSiteTool`.
*   **Bugfix: Lost Trailing Newline (MEDIUM):** `ReplaceLinesTool` now preserves trailing newline when original content had one.
*   **Bugfix: Empty API Key Warning (MEDIUM):** Added `tracing::warn!` when API key resolution returns empty string for a provider.
*   **Bugfix: HTTP Client Reuse (MEDIUM):** Replaced per-call `reqwest::Client` creation in multimodal parsing with a shared static client via `OnceLock`.
*   **Bugfix: Regex Recompilation (MEDIUM):** Replaced per-call `Regex::new()` in `context_compactor.rs`, `cli.rs`, and `web.rs` with static `OnceLock<Regex>` patterns.
*   **Bugfix: SVG Attribute Injection (MEDIUM):** Applied `escape_xml()` to all attribute values in `SvgElement::to_svg_string()`.
*   **Bugfix: CSS Template Injection (MEDIUM):** Added backslash escaping to CSS injection in `GenerateImageTool` to prevent template literal breakout.
*   **Bugfix: Python Execution Timeout (MEDIUM):** Added 60-second `tokio::time::timeout` to `PythonSandboxTool` to prevent infinite hangs.
*   **Bugfix: Port Allocation Warning (MEDIUM):** Added `tracing::warn!` when `find_free_port()` exhausts 100 attempts.
*   **Bugfix: Cron Scheduler Shutdown (LOW):** `start_scheduler()` now returns `JoinHandle<()>` for graceful shutdown.
*   **Bugfix: Discord Infinite Reconnect (LOW):** Added `MAX_RETRIES = 10` cap with attempt count in error messages.
*   **Bugfix: WASM Exit Code (LOW):** Extracts real WASI exit code via `I32Exit` downcast instead of hardcoding `1`.
*   **Bugfix: Empty Embeddings (LOW):** Skips DB insert when embedding generation fails (empty vec) in research archive.
*   **Bugfix: Subagent Empty Fallback (LOW):** Filters empty strings from fallback model list before padding.
*   **Bugfix: Biased Random Selection (LOW):** `select_random_message` now uses all 16 UUID bytes for unbiased selection instead of just byte 0.
*   **Code Quality: Clippy Cleanup:** Fixed all 112 clippy warnings across the codebase (strip_prefix, matches! macro, while let loops, static regex, too_many_arguments suppression, etc.).
*   **Maintenance: Version Bump:** Bumped to v0.0.15. All 114 tests passing.

### v0.0.14
*   **Feature: Incremental Session Saving:** Upgraded `AgentLoop` to save the active conversation session (`cli_direct.json`) incrementally to disk: (1) immediately upon receiving a user prompt in `Restore` state, and (2) at the end of each successful turn iteration inside the run loop. This ensures that even if an execution is interrupted via `Esc` or `Ctrl+C` midway, the prompt, thoughts, and intermediate tool outputs are fully persisted on disk. When restarted, typing "continue" allows the agent to resume execution with complete context.
*   **Feature: Resumed Session History Visualization:** Implemented `print_session_history` helper in the CLI channel (`cli.rs`) to format and render previous messages, assistant thoughts, and tool executions. This automatically displays the loaded session's history upon startup/resume or when switching sessions via the `/history` command menu, resolving the visual blank-screen confusion.

### v0.0.13
*   **Bugfix: ANSI Code Log File Pollution:** Configured separate registry layers for the file writer and standard error in `tracing-subscriber` setup inside `main.rs`. This prevents ANSI escape codes from being written to the log file `openz.log`, which was causing level and target parsing failures in the log viewer (`openz logs`) when background/server subcommands were run in a terminal.
*   **Bugfix: Consistent Log Path Resolution:** Updated `default_log_path()` in `logs.rs` to resolve relative to `crate::config::config_dir()` instead of hardcoding `~/.openz/openz.log`. This ensures path alignment whenever `OPENZ_CONFIG_DIR` is customized.
*   **Feature: Real-Time Stream Default:** Changed the default value of the `--tail` parameter from `200` to `0` lines for `openz logs` and channel logs subcommands. This allows `openz logs` to start tailing immediately from the current file end (showing only live logs one by one as they happen, like a Hono server) while still supporting historical inspection via manual `--tail N`.
*   **Bugfix: Backtrace Pruning Regex Correction:** Fixed a typo in `context_compactor.rs` where the backtrace regex pattern had a double caret `^^` instead of a single caret `^`, enabling correct frame pruning.

### v0.0.12
*   **Feature: High-Fidelity HTML-to-Image Generation:** Rewrote `GenerateImageTool` (`generate_image`) to render and capture complex HTML, CSS grid/flex layouts, Tailwind CDN styles, web fonts, and custom SVGs using a local headless Chrome/Chromium instance via CDP at high-DPI Retina resolution (`device_scale_factor: 2.0`). Supports custom CSS injections and element-specific crops (`selector`).
*   **Feature: Remotion-Equivalent Video rendering:** Added `HtmlToVideoTool` (`html_to_video`) to load custom HTML/CSS timelines, tick frames programmatically via JS, capture snapshots via CDP, and stitch frames into final MP4 files using FFmpeg.
*   **Feature: Asynchronous Command Interruption:** Upgraded shell command execution (`ExecCommandTool`, `PythonSandboxTool`) and cargo compilations to asynchronous processes (`tokio::process::Command`) with `.kill_on_drop(true)`, enabling instant child process termination when an agent turn is interrupted via `Esc` or `Ctrl+C`.
*   **Bugfix: Modern Chrome CDP Verb Compatibility:** Patched all browser engines (`image_generator.rs`, `obscura.rs`, `html_video.rs`) to use a cascading `PUT` request with `GET` fallback on the `/json/new` endpoint, resolving `405 Method Not Allowed` failures enforced by modern Chrome (149+).
*   **Bugfix: Headless Browser & Video Generator UTF-8 Charset Encoding:** Added custom middleware to force the `text/html; charset=utf-8` header on the local static file servers in both `HtmlToVideoTool` (`html_to_video`) and `GenerateImageTool` (`generate_image`). This ensures all emojis, icons, and special UTF-8 characters (like middle dots) render correctly without text corruption (such as `ðŸ”—`, `âŒ¨ï¸`) in output images and videos.
*   **Bugfix: Infinite Argument-Correction Loop:** Patched raw newline handling and root parameter fallbacks inside `extract_tool_call` to prevent infinite tool calling errors.
*   **Bugfix: Response Continuation Tool-Calling & Loop Detection:** Disabled tool definitions during response continuation to prevent models from generating malformed tool calls, and enabled re-parsing of fallback tool calls on completed accumulated responses. Fixed a bug in `count_previous_tool_calls` and prompt history construction where OpenAI-style nested tool calls (nested under `function`) bypassed loop detection and context formatting.
*   **Configuration: Local-First Embeddings:** Locked configuration files to `"embeddings": { "mode": "local" }` to ensure vector lookups run entirely offline via FastEmbed and avoid remote cloud connection calls.
*   **Feature: Raw SVG & Global Styling in SVG Animator:** Enhanced `SvgAnimatorTool` (`create_animated_svg`) by adding support for the `raw_svg` parameter (allows direct raw code injection or partial code wrapped automatically inside an SVG envelope). Enabled common styling attributes (`class`, `style`, `transform`, `filter`, `clip_path`, `mask`) globally on all shapes, and implemented attribute deduplication/overwriting in `SvgElement` construction.
*   **Documentation: Workspace Alignment:** Documented the new visual schemas in `AGENTS.md` and active agent skill instructions under `onpkg_docs/image_generator.md`.
*   **Bugfix: Real-time Log Streaming & Buffering:** Rewrote the file tailing logic inside the logs viewer (`openz logs`) using periodic file reopenings to handle file rotation/inode recreation reliably on all platforms, alongside Unix inode checks to detect recreated files. Implemented a trailing buffer to slice and print only complete lines. Fixed a seek-index calculation bug where the file offset pointer was advanced by the processed buffer offset instead of the raw read bytes size, preventing duplicate reads and out-of-sync pointer resets. This completely resolves terminal truncations, duplicate entries, lockups, and output delays. Additionally, updated log initialization in `main.rs` to stream logs live to `stderr` with ANSI colors during background server subcommands (e.g. `gateway`, `telegram`, `discord`, `whatsapp`), making server operation visible in real-time.
*   **Bugfix: Headless Browser Local File Sandboxing (Snap/Flatpak Compatibility):** Replaced sandboxed `file://` URLs in `generate_image` and `html_to_video` with a dynamically spawned, temporary, local Axum web server on a random free port (e.g. `http://127.0.0.1:PORT/file`). Added `--allow-file-access-from-files` and `--disable-web-security` flags. Resolved relative path bug where parent directories resolved to empty string `""` by enforcing absolute paths, preventing "This site can't be reached" (isolated `/tmp` and local file access blockages) on Ubuntu and other systems running Chrome via Snaps or Flatpaks. Also added a configurable `load_delay_ms` parameter (default `1500` ms) to the `html_to_video` tool to allow custom timing configuration for heavy pages to fully load/mount JS bundle animations before frame screenshots are captured.
*   **Prompt: Agent System Prompt Guidelines:** Made the OpenZ agent system prompt aware of its creator (Aswin), inspirations, specifications, features, and `changelog` command.
*   **Documentation: README.md Updates:** Updated README.md documentation for the `changelog` command.
*   **Maintenance: Version Bump:** Staged and committed all outstanding code changes and version bump to GitHub.

### v0.0.11
*   **Feature: Changelog Subcommand:** Added `openz changelog` command to display features, specifications, and version history.
*   **Feature: Changelog File:** Added `CHANGELOG.md` in the project root.
*   **Optimization: Curator Throttling:** Implemented context length and tool-use checks inside the background curator to prevent unnecessary API expenses.
*   **Optimization: Stale Skills Archival:** Throttled the skills database archiver to a 24-hour interval via persistent JSON timestamps.
*   **Feature: Cloud-First Embeddings:** Integrated remote vector embedding fallbacks and a `"cloud_only"` low-RAM mode to skip downloading local ONNX weights.
*   **Feature: Compiler Auto-Healing:** Added the `CompilerAutoHealTool` to automate syntax and compilation repair loops in Rust/JS.
*   **Maintenance: Startup Cleanup:** Automated git worktree and temporary workspace pruning on startup.
*   **Optimization: Low-Resource Build Mode:** Added a `--low-resource` (or `--low-mem`) flag to `localinstall.sh` and `localupdate.sh` to restrict parallel compiler jobs and codegen-units, preventing high memory and CPU utilization during installation/updates.
*   **Optimization: Cargo.toml Release Profile:** Configured custom release build settings (`codegen-units = 1`, `lto = "thin"`, `strip = true`, `debug = false`) to natively reduce peak RAM usage and compiler threads during production builds, reducing final ROM footprint.

### v0.0.10
*   **Feature: SQLite Memory Layer:** Shifted skills and long-term facts to a SQLite database (`~/.openz/memory.db`) with auto-migration.
*   **Feature: Code Semantic Search:** Embedded structural search using `ast_grep` and fast vector indexing.
*   **Feature: subagents Registration:** Registered specialized subagent profiles as dynamic LLM tools.
*   **Feature: `mermaid_designer`:** Added a dedicated subagent for generating SVG flowcharts.

### v0.0.9
*   **Feature: Cryptographic Ledger:** Added SHA-256 Merkle chain hash ledger for auditing agent loops (`/audit` command).
*   **Feature: WhatsApp Channel:** Built Axum webhook channel for WhatsApp API integration.
*   **Feature: Auto-Continuation:** Stitched assistant messages seamlessly when hitting token limits (`finish_reason = "length"`).

### v0.0.8
*   **Feature: Email Channel:** Added pure Rust IMAP/SMTP email client.
*   **Feature: Discord Channel:** Added Discord Gateway WebSockets channel support.

### v0.0.7
*   **Feature: Telegram Bot:** Added Telegram polling channel.
*   **Feature: WebSocket Gateway:** Built static UI server and local OpenAI endpoint.

### v0.0.1 - v0.0.6
*   **Core Foundation:** Initial Clap CLI parser, sandboxed execution, filesystem tools, and basic Anthropic/OpenAI provider trait routing.
