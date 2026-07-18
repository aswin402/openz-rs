# Tool Selection, Safety, and Error Recovery Guide

This guide details the conventions, boundaries, and error recovery protocols for all native tools in OpenZ. Follow these instructions to optimize tool execution, avoid infinite loops, and handle errors robustly.

---

## 1. Filesystem Operations
* **Prefer Incremental Edits:** Always prefer using `patch_file` or `replace_lines` instead of rewriting entire files with `write_file`. This conserves token usage and prevents overwriting parallel changes.
* **Path Constraints:** All file/directory operations are limited to the workspace folder and `~/.openz/`. Any path outside these roots will trigger a traversal prevention error.

---

## 2. Command & Process Execution (`exec_command`)
* **Resource Limits:** Restrict execution cores and memory on memory-constrained systems (e.g. `CARGO_BUILD_JOBS=1 cargo test --lib`).
* **Timeout Limits:** The default tool timeout is 300 seconds unless the user config overrides it. Preserve custom timeout values and split long work into smaller verified steps when possible.
* **Managed Servers:** Dev-server commands such as `npm run dev`, `bun run dev`, `npx vite`, and `python -m http.server` are registered as background servers. Use `manage_servers` automatically to list or stop OpenZ-launched servers when the preview or verification task is finished.
* **Detached GUI Apps:** Browser, viewer, editor, and media-player launches should be treated as complete when the command reports a visible app launch. Do not retry alternate viewers unless the user says the launch failed.
* **Sandbox EPERM:** On Linux, optional BPF seccomp filtering can block commands that need broader system calls. If compiler/browser tooling returns `EPERM`, inspect `enableSandbox` in config and use the least permissive mode that still works.

---

## 3. Web Fetching & Scraping (`searchxyz`)
* **Bypass Protections:** DuckDuckGo and Google search backends support rotating proxy pools and headless Chromium loading (`js-rendering`) natively.
* **Scraping Fallbacks:** If a raw HTTP scrape fails, the engine automatically triggers the headless Chromium fallback. If first-party search remains blocked, fallback to paid search APIs (Tavily/Exa).

---

## 4. SQLite Database Inspector & Writer
* **In-Process Connections:** Execute SQL directly using native `rusqlite` inspector tools. Do not spawn the `sqlite3` CLI tool.
* **Resource Capping:** Always add `LIMIT` clauses to SELECT queries to prevent memory bloating.
* **Concurrency:** Wrap bulk writes, deletes, and updates inside SQL transactions (`BEGIN TRANSACTION; ... COMMIT;`) to prevent deadlocks and locking bottlenecks.
