# procedural_tool_guidelines

## Core Guidelines for Native Tool Execution, Error Recovery, and Fallbacks in OpenZ

This guide teaches you how to select, execute, and troubleshoot native tools in OpenZ to ensure high reliability, fast response times, and zero token waste.

---

### 1. File Access & Modification (Filesystem)
* **Tool Selection:**
  * **Reading:** Use `read_file` or `view_file`. For viewing structural files, use `code_outline` or `ast_grep`.
  * **Modifying:** ALWAYS prefer `patch_file` (unified diff format) or `replace_lines` (specific range) over rewriting the entire file using `write_file`. Rewriting large files consumes massive token budget and is error-prone.
* **Error Recovery & Path Safety:**
  * **Error: Permission Denied / Traversal Blocked:** Path traversal protection restricts file operations strictly within the repository workspace and `~/.openz/`. Ensure all paths are relative to these roots.
  * **Error: File Already Exists (on write):** Set the `Overwrite` flag to `true` if you explicitly intend to replace a file.

---

### 2. Sandbox, Shell, & Command Execution
* **Tool Selection:**
  * Use `exec_command` to execute shell processes (e.g. `cargo check`, compile, test).
* **Timeout & Resource Limits:**
  * **Timeout:** Shell commands are capped at a **120-second timeout**. If a compilation or script runs long, break it into smaller steps or use `--low-resource` flags.
  * **Resource exhaustion:** Under low memory/CPU systems, set `export CARGO_BUILD_JOBS=1` and `export RUSTFLAGS="-C codegen-units=1"` to prevent system freeze.
* **Worst-Case Scenario (Sandbox Blocks):**
  * On Linux, `exec_command` may enforce a **seccomp BPF sandbox** restricting network and system access.
  * If a command returns `EPERM` or is unexpectedly killed during valid builds/tool chains, check if sandbox is enabled in `~/.openz/config.json` (`enableSandbox`). If so, advise the user to run `openz configure` to disable sandbox temporarily.

---

### 3. Web Search & Scraping (SearchXyz)
* **Tool Selection:**
  * Use `web_search` as the entrypoint. It prioritizes local `searchxyz` and falls back to external APIs (Tavily/Exa) only if first-party methods fail.
* **Anti-Bot-Detection & Fallbacks:**
  * `searchxyz` DuckDuckGo/Google backends support **rotating proxies** and **headless Chromium rendering** via `chromiumoxide`.
  * If a query returns 0 results or gets blocked, the engine automatically attempts headless browser retrieval.
* **Worst-Case Scenario:**
  * If a website has extreme anti-scraping protections (e.g., Cloudflare Under Attack mode), headless rendering might fail. In this case, fallback to:
    1. Downstream paid APIs (Tavily/Exa).
    2. Scraping text-only caches or alternate search engines (Brave, Bing).

---

### 4. Database Access (DbInspector / DbWrite)
* **Tool Selection:**
  * Use `db_inspector` and `db_write` to interact with SQLite databases. These run in-process using Rust's `rusqlite` crate—**do not spawn `sqlite3` CLI processes**.
* **Memory & Performance Safeguards:**
  * **Capping Rows:** Unbounded SELECT statements can consume high memory. ALWAYS include `LIMIT` clauses on database queries.
  * **Deadlock Prevention:** To avoid lock conflicts, wrap DELETE+INSERT pairs or bulk insertions in SQL transactions (`BEGIN TRANSACTION; ... COMMIT;`).

---

### 5. Large Tool Outputs & Compaction
* **Context Budget:**
  * The compiled system prompt budget is capped at **32K characters**.
* **Aggressive Truncation:**
  * Outputs exceeding 4,000 characters are automatically saved to `~/.openz/tool_outputs/` and compacted inline.
  * If you need the full uncompressed payload, retrieve it using `retrieve_original` with the given file/CCR reference ID.
