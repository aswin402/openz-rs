# Tool Selection, Safety, and Error Recovery Guide

This guide details the conventions, boundaries, and error recovery protocols for all native tools in OpenZ. Follow these instructions to optimize tool execution, avoid infinite loops, and handle errors robustly.

---

## 1. Filesystem Operations
* **Prefer Incremental Edits:** Always prefer using `patch_file` or `replace_lines` instead of rewriting entire files with `write_file`. This conserves token usage and prevents overwriting parallel changes.
* **Path Constraints:** All file/directory operations are limited to the workspace folder and `~/.openz/`. Any path outside these roots will trigger a traversal prevention error.

---

## 2. Command & Process Execution (`exec_command`)
* **Resource Limits:** Restrict execution cores and memory on memory-constrained systems (e.g. `export CARGO_BUILD_JOBS=1`).
* **Timeout Limits:** Shell commands have a strict 120-second timeout. Break long script executions into separate smaller commands.
* **Sandbox EPERM:** On Linux, a BPF seccomp filter blocks dangerous system calls (network, mounting, etc.). If a compiler tool or command returns `EPERM`, disable `enableSandbox` temporarily via `openz configure`.

---

## 3. Web Fetching & Scraping (`searchxyz`)
* **Bypass Protections:** DuckDuckGo and Google search backends support rotating proxy pools and headless Chromium loading (`js-rendering`) natively.
* **Scraping Fallbacks:** If a raw HTTP scrape fails, the engine automatically triggers the headless Chromium fallback. If first-party search remains blocked, fallback to paid search APIs (Tavily/Exa).

---

## 4. SQLite Database Inspector & Writer
* **In-Process Connections:** Execute SQL directly using native `rusqlite` inspector tools. Do not spawn the `sqlite3` CLI tool.
* **Resource Capping:** Always add `LIMIT` clauses to SELECT queries to prevent memory bloating.
* **Concurrency:** Wrap bulk writes, deletes, and updates inside SQL transactions (`BEGIN TRANSACTION; ... COMMIT;`) to prevent deadlocks and locking bottlenecks.
