# OpenZ Autonomous 24/7 Agent Architecture Specification
**Document Version:** 1.0.0  
**Target Runtime:** OpenZ v0.0.247+  
**Target Environment:** Linux / Unix High-Availability Unattended Daemon  
**Author:** Engineering Lead / Manager (Antigravity & OpenZ Autonomous Operations)  

---

## 1. Executive Summary & Autonomous Operations Philosophy

OpenZ is an async, monolithic, zero-IPC personal AI agent framework implemented natively in Rust. Unlike legacy agent architectures that orchestrate disparate Python scripts, Node.js daemons, and detached Model Context Protocol (MCP) child processes over JSON-RPC sockets, OpenZ compiles all 260 tools directly into a single unified binary.

Operating an autonomous agent **24/7 in production** requires meeting five critical invariants:
1. **Zero-IPC Memory Footprint & Determinism:** Native in-process tool execution eliminating process churn, IPC serialization bottlenecks, and orphaned zombie processes.
2. **Crash-Resistant State Persistence:** Write-Ahead-Log (WAL) backed session history under `~/.openz/sessions/` preserving turn continuity across machine reboots.
3. **Bounded Context Windows:** Dynamic headroom compaction compressing session history whenever message counts cross thresholds (default 120 turns) using Z-Context compression.
4. **Defense-in-Depth Execution Sandboxing:** Strict two-layer security consisting of `SecurityGuard` permission policies and Linux BPF seccomp syscall filters for subprocesses.
5. **Continuous Self-Improvement & Chronos Scheduling:** Autonomous background curation synthesizing durable skills into `~/.openz/skills/` and cron-based autonomous execution routines.

```
+-----------------------------------------------------------------------------------+
|                           OpenZ 24/7 Autonomous Daemon                            |
|                                                                                   |
|  +--------------------+    +--------------------+    +-------------------------+  |
|  |   Ingress Vector   |    |    Chronos Loop    |    |   Autonomous Webhooks   |  |
|  | CLI / WS / TG / DC |    | Scheduled Triggers |    |  WhatsApp / Rest Hooks  |  |
|  +---------+----------+    +---------+----------+    +------------+------------+  |
|            |                         |                            |               |
|            +-------------------------+----------------------------+               |
|                                      |                                            |
|                                      v                                            |
|                  +---------------------------------------+                        |
|                  |      TurnState Machine (5-Stage)      |                        |
|                  |   Restore -> Compact -> Run -> Save   |                        |
|                  +-------------------+-------------------+                        |
|                                      |                                            |
|             +------------------------+------------------------+                   |
|             |                                                 |                   |
|             v                                                 v                   |
|   +-------------------+                             +-------------------+         |
|   | 260 Native Tools  |                             | In-Process Vector |         |
|   | (Zero-IPC Engine) |                             | & Tantivy Memory  |         |
|   +---------+---------+                             +---------+---------+         |
|             |                                                 |                   |
|             +------------------------+------------------------+                   |
|                                      |                                            |
|                                      v                                            |
|                  +---------------------------------------+                        |
|                  |     Async Background Self-Curator     |                        |
|                  | Skills Synthesis & WAL Synchronization|                        |
|                  +---------------------------------------+                        |
+-----------------------------------------------------------------------------------+
```

---

## 2. Core 5-Stage Autonomous Loop

The autonomous loop executes through an atomic state machine defined in `src/agent/agent_loop/mod.rs`:

$$\text{Restore} \longrightarrow \text{Compact} \longrightarrow \text{Command} \longrightarrow \text{Build} \longrightarrow \text{Run} \longrightarrow \text{Save} \longrightarrow \text{Curate}$$

### Stage 1: Sense & Restore
- Loads durable session state from `~/.openz/sessions/<session_key>.json`.
- Ingests environmental telemetry: active working directory, git state, system load, time, and external webhook/channel events.
- Evaluates recent file modifications using native watcher caches.

### Stage 2: Context Compaction & Budgeting
- Monitors conversation token and message bounds.
- If total turns $> 120$ messages, automatically activates context compaction.
- Preserves recent user instructions while compressing intermediate tool outputs into durable references (`~/.openz/tool_outputs/<uuid>.json`).
- Traverses backwards to anchor on the nearest user message, preventing orphaned assistant/tool turns from invalidating LLM provider schema requirements.

### Stage 3: Dynamic System Prompt Assembly
- Compiles the system prompt dynamically from:
  1. Base Persona & Caveman terseness directive (default active).
  2. Durable User Preferences (`~/.openz/USER.md`).
  3. Knowledge Graph Memory excerpts from SQLite / Tantivy inverted index.
  4. Active Skill Manifests from `~/.openz/skills/*.md`.
  5. Global Activity Trace log (`~/.openz/activity.json`).

### Stage 4: Act & Execute (Native Tool Pipeline)
- Dispatches prompt to the primary provider (with auto-fallback failover across configured providers).
- Handles tool call invocations via `ToolRegistry`.
- Validates each call through `SecurityGuard` permissions and Seccomp sandboxing.
- Executes natively in-process (or in isolated child processes for shell commands) with standard 300-second timeouts.

### Stage 5: Save & Self-Improvement Curation
- Writes turn results to disk with atomic file renaming.
- Spawns an asynchronous background task (`tokio::spawn`) running the **Curator**:
  - Analyzes the turn for new recurring patterns or explicit user instructions.
  - Automatically synthesizes reusable skills into `~/.openz/skills/<skill_name>.md`.
  - Updates durable user facts in `~/.openz/USER.md`.

---

## 3. Subsystem Breakdown: 260 Native Tools Matrix

OpenZ provides 260 native tools categorized into specialized subsystems:

| Subsystem | Tool Count | Primary Responsibilities | Architecture & Engine |
|---|---|---|---|
| **Headless CLI & Core** | 12 | Batch execution, streaming, piped stdin, command routing | `clap`, `tokio::task_local` |
| **Filesystem & Patching** | 18 | `read_file`, `write_file`, `patch_file`, `replace_lines`, directory traversal | Async Tokio FS, atomic temp-file replace |
| **Shell & Execution** | 6 | `exec_command`, process monitoring, signal dispatching | Linux BPF seccomp sandbox, `PR_SET_NO_NEW_PRIVS` |
| **SearchXyz (Web & KG)** | 28 | `searchxyz_search_web`, `searchxyz_read_url`, `searchxyz_index_content`, `searchxyz_query_graph` | In-process Tantivy full-text index & SQLite KG |
| **OpenDoc Intelligence** | 32 | `.docx`, `.xlsx`, `.pptx`, `.pdf`, `.csv` document creation, parsing, table insertion | In-process Calamine, RDocx, fast-excel |
| **OpenMedia Graphics** | 31 | SVG layout, Mermaid diagrams, charts, icons, PNG rasterization | `resvg`, `tiny_skia`, system font loader |
| **Wavyte Video Engine** | 14 | Programmatic MP4 video generation, motion paths, timeline tweening | Software CPU renderer + hardware h264 pipeline |
| **Graph & Working Memory** | 42 | Entity extraction, graph neighbor queries, CCR context retrieval, working memory slots | Native SQLite + FastEmbed 384d vectors |
| **SOP & Orchestration** | 24 | Standard Operating Procedure workflows, multi-agent delegator, review loops | Stateful JSON instances, asyncDAG scheduler |
| **Chronos & Automation** | 16 | `schedule_job`, `list_jobs`, `remove_job`, cron expressions, interval timers | Tokio async interval daemon, durable JSON storage |
| **Self-Management** | 37 | Diagnostics, doctor, backup, rollback, tool catalog, inventory | In-process introspection & configuration APIs |

---

## 4. Resilience, Fault Tolerance & Self-Healing

Running 24/7 without human intervention demands resilient fault handling:

### 1. Multi-Provider Failover Cascading
If the primary provider hits HTTP 429 (rate limited), HTTP 500 (upstream outage), or timeout:
```
Primary (e.g. MiniMax) ---> Fallback 1 (Groq) ---> Fallback 2 (Mistral) ---> Fallback 3 (OpenRouter)
```
The router preserves conversation messages across attempts without losing context.

### 2. Context Protection & Tool Output Headroom
When tool outputs exceed 4,000 characters:
1. Raw payload is flushed to `~/.openz/tool_outputs/<tool>_<uuid>.json`.
2. A compressed summary is generated via Headroom CCR compactor.
3. The model receives a compact representation with a reference handle (`retrieve_original`), protecting context tokens from sudden bloat.

### 3. Crash Recovery via WAL Sessions
Sessions are appended atomically. If power fails or the process is killed mid-turn:
- The previous turn remains clean and valid.
- Upon restart, `openz` resumes from the last completed user/assistant exchange.
- Broken JSON states are automatically archived with timestamped suffixes (`~/.openz/sessions/<key>.corrupt.<ts>`).

---

## 5. 24/7 Production Deployment Guide

### Systemd Service Configuration
To run OpenZ as an autonomous system daemon:

```ini
[Unit]
Description=OpenZ 24/7 Autonomous Agent Daemon
After=network.target network-online.target
Wants=network-online.target

[Service]
Type=simple
User=aswin
WorkingDirectory=/home/aswin
Environment="HOME=/home/aswin"
Environment="PATH=/home/aswin/.cargo/bin:/usr/local/bin:/usr/bin:/bin"
ExecStart=/home/aswin/.cargo/bin/openz gateway --port 8765 --host 127.0.0.1
Restart=always
RestartSec=5s
LimitNOFILE=65535
LimitNPROC=4096
CPUQuota=200%
MemoryMax=4G

[Install]
WantedBy=multi-user.target
```

### Automated Monitoring & Health Routine
- Run the heartbeat audit script `autonomous_agent_ops/scripts/heartbeat_audit.sh` every 5 minutes via cron or OpenZ native `schedule_job`:
  ```bash
  openz run -p "Schedule job '*/5 * * * *' to execute autonomous_agent_ops/scripts/heartbeat_audit.sh" -y
  ```
- Inspect live logs anytime via:
  ```bash
  openz logs --tail 100 --level info
  ```
