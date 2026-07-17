# Knowledge and Workflow Memory Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give OpenZ durable source memory and reusable workflow memory so repeated research/tasks reuse known links, paths, repos, and successful tool procedures instead of starting from zero.

**Architecture:** Extend the existing `~/.openz/memory.db` shared-memory layer with typed `source_bookmarks`, `research_briefs`, `workflow_cards`, and `workflow_runs` tables. Expose CRUD through native tools, retrieve relevant sources/workflows during prompt build, and show minimal TUI status lines when sources or workflows are matched/created.

**Tech Stack:** Rust, rusqlite, serde_json, existing `Tool` trait, existing `shared_memory` module, existing TUI style/color helpers, existing session/self-improvement curator.

## Global Constraints

- Use the existing SQLite DB path from `crate::tools::shared_memory::db::get_sqlite_db_path()`.
- Do not create a separate database unless the shared-memory DB cannot support the feature.
- Secrets must be redacted before storing workflows.
- Research answers must prefer fresh official/canonical sources but refresh stale data before answering volatile questions.
- Workflow reuse must be opt-in by confidence: use only active workflows with successful prior runs or user-approved creation.
- Keep TUI status minimal and theme-consistent.
- Avoid hardcoded workflows such as Telegram screenshots; store them as data.

---

## File Structure

- Modify `src/tools/shared_memory/db.rs`: add schema tables and indexes.
- Create `src/tools/shared_memory/knowledge.rs`: source bookmark and research brief CRUD/search helpers and tools.
- Create `src/tools/shared_memory/workflows.rs`: workflow card CRUD/search/run tracking helpers and tools.
- Modify `src/tools/shared_memory/mod.rs`: export new tools and helpers.
- Modify `src/cli/tools.rs`: register new native tools.
- Modify `src/cli/builder.rs`: update native registration tests and expected counts.
- Modify `src/agent/agent_loop/build.rs`: inject relevant source/workflow context into system prompt with a tight budget.
- Modify `src/agent/agent_loop/save.rs`: add workflow-mining prompt rules to the existing curator so successful multi-tool trajectories produce workflow cards through the new tool, not only freeform skills.
- Modify `src/agent/agent_loop/tool_execution.rs`: print compact TUI status for knowledge/workflow tool success.
- Modify `src/channels/cli/render.rs` and `src/channels/telegram.rs`: add slash command visibility for `/workflows` and `/sources` if implemented as channel commands.
- Modify `CHANGELOG.md` and `README.md`: document the feature.

---

### Task 1: Add Persistent Tables

**Files:**
- Modify: `src/tools/shared_memory/db.rs`
- Test: existing shared-memory tests plus new tests in `src/tools/shared_memory/knowledge.rs` and `src/tools/shared_memory/workflows.rs`

**Interfaces:**
- Produces DB tables:
  - `source_bookmarks(id, label, kind, uri, aliases, summary, trust_score, last_checked, stale_after_secs, created_at, updated_at, use_count)`
  - `research_briefs(id, topic, summary, source_ids, confidence, stale_after_secs, created_at, updated_at, use_count)`
  - `workflow_cards(id, name, triggers, summary, steps_json, preconditions, verification, risk, status, success_count, failure_count, last_used, created_at, updated_at)`
  - `workflow_runs(id, workflow_id, session_key, task, success, error, timestamp)`

- [ ] **Step 1: Add schema SQL**

Add these statements inside `create_schema(conn: &Connection)`:

```rust
conn.execute(
    "CREATE TABLE IF NOT EXISTS source_bookmarks (
        id TEXT PRIMARY KEY,
        label TEXT NOT NULL,
        kind TEXT NOT NULL,
        uri TEXT NOT NULL,
        aliases TEXT NOT NULL,
        summary TEXT NOT NULL,
        trust_score REAL NOT NULL DEFAULT 0.5,
        last_checked TEXT,
        stale_after_secs INTEGER NOT NULL DEFAULT 604800,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL,
        use_count INTEGER NOT NULL DEFAULT 0,
        UNIQUE(uri)
    )",
    [],
)?;
conn.execute(
    "CREATE INDEX IF NOT EXISTS idx_source_bookmarks_label ON source_bookmarks(label)",
    [],
)?;
conn.execute(
    "CREATE TABLE IF NOT EXISTS research_briefs (
        id TEXT PRIMARY KEY,
        topic TEXT NOT NULL UNIQUE,
        summary TEXT NOT NULL,
        source_ids TEXT NOT NULL,
        confidence REAL NOT NULL DEFAULT 0.5,
        stale_after_secs INTEGER NOT NULL DEFAULT 86400,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL,
        use_count INTEGER NOT NULL DEFAULT 0
    )",
    [],
)?;
conn.execute(
    "CREATE TABLE IF NOT EXISTS workflow_cards (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL UNIQUE,
        triggers TEXT NOT NULL,
        summary TEXT NOT NULL,
        steps_json TEXT NOT NULL,
        preconditions TEXT NOT NULL,
        verification TEXT NOT NULL,
        risk TEXT NOT NULL DEFAULT 'normal',
        status TEXT NOT NULL DEFAULT 'draft',
        success_count INTEGER NOT NULL DEFAULT 0,
        failure_count INTEGER NOT NULL DEFAULT 0,
        last_used TEXT,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
    )",
    [],
)?;
conn.execute(
    "CREATE TABLE IF NOT EXISTS workflow_runs (
        id TEXT PRIMARY KEY,
        workflow_id TEXT NOT NULL,
        session_key TEXT NOT NULL,
        task TEXT NOT NULL,
        success INTEGER NOT NULL,
        error TEXT,
        timestamp TEXT NOT NULL,
        FOREIGN KEY(workflow_id) REFERENCES workflow_cards(id)
    )",
    [],
)?;
```

- [ ] **Step 2: Run compile check**

Run: `cargo check`
Expected: PASS.

---

### Task 2: Source and Research Brief CRUD Tool

**Files:**
- Create: `src/tools/shared_memory/knowledge.rs`
- Modify: `src/tools/shared_memory/mod.rs`
- Modify: `src/cli/tools.rs`
- Modify: `src/cli/builder.rs`

**Interfaces:**
- Produces tools:
  - `knowledge_source` with actions `add`, `search`, `get`, `update`, `delete`, `mark_checked`.
  - `research_brief` with actions `save`, `search`, `get`, `delete`, `mark_used`.

- [ ] **Step 1: Write tests for source CRUD**

Add a test that:
1. Adds label `Hermes Agent`, kind `docs`, uri `https://hermes-agent.nousresearch.com/docs/`, aliases `["hermes", "nous hermes"]`.
2. Searches `whats hermes`.
3. Asserts one result contains the URI.
4. Deletes it.

Run: `cargo test --lib knowledge_source -- --test-threads=1`
Expected: FAIL until implementation exists.

- [ ] **Step 2: Implement helper functions and tool struct**

Implement:

```rust
pub async fn add_source_bookmark(args: SourceBookmarkInput) -> Result<SourceBookmark>;
pub async fn search_source_bookmarks(query: &str, limit: usize) -> Result<Vec<SourceBookmark>>;
pub struct KnowledgeSourceTool;
```

Search should score label, aliases, uri, and summary with simple lowercase containment first. Do not require embeddings in Task 2.

- [ ] **Step 3: Register tool**

Add exports in `shared_memory/mod.rs` and register in `src/cli/tools.rs` near existing shared memory tools.

- [ ] **Step 4: Update registration test**

Add `knowledge_source` and `research_brief` to the `shared_names` array and adjust expected shared count.

- [ ] **Step 5: Run tests**

Run:

```bash
cargo test --lib knowledge_source -- --test-threads=1
cargo test --lib test_native_tool_registration_names -- --test-threads=1
cargo check
```

Expected: PASS.

---

### Task 3: Workflow Card CRUD and Run Tracking

**Files:**
- Create: `src/tools/shared_memory/workflows.rs`
- Modify: `src/tools/shared_memory/mod.rs`
- Modify: `src/cli/tools.rs`
- Modify: `src/cli/builder.rs`

**Interfaces:**
- Produces tool `workflow_memory` with actions `add`, `search`, `get`, `update`, `delete`, `record_run`, `activate`, `deactivate`.
- Produces helper `search_active_workflows(query: &str, limit: usize) -> Result<Vec<WorkflowCard>>`.

- [ ] **Step 1: Write tests**

Add tests that:
1. Add workflow `screenshot_active_window_to_telegram` with trigger `send screenshot to telegram` and status `active`.
2. Search `take screenshot and send it to telegram`.
3. Assert the workflow is returned.
4. Record success.
5. Assert `success_count == 1`.

Run: `cargo test --lib workflow_memory -- --test-threads=1`
Expected: FAIL until implementation exists.

- [ ] **Step 2: Implement tool**

`workflow_memory` parameters must accept:

```json
{
  "action": "add|search|get|update|delete|record_run|activate|deactivate",
  "name": "workflow_name",
  "query": "user task text",
  "triggers": ["phrase one"],
  "summary": "what this workflow does",
  "steps": [{"tool":"exec_command","args":{"cmd":"..."},"note":"..."}],
  "preconditions": ["telegram configured"],
  "verification": ["Telegram API returns ok=true"],
  "risk": "normal|high",
  "status": "draft|active|disabled",
  "success": true,
  "error": "optional error text",
  "session_key": "optional session"
}
```

- [ ] **Step 3: Redact secrets before storage**

Add a helper that replaces fields containing `token`, `secret`, `password`, `api_key`, `authorization` with `********` inside `steps_json`.

- [ ] **Step 4: Register and test**

Run:

```bash
cargo test --lib workflow_memory -- --test-threads=1
cargo test --lib test_native_tool_registration_names -- --test-threads=1
cargo check
```

Expected: PASS.

---

### Task 4: Prompt Retrieval Injection

**Files:**
- Modify: `src/agent/agent_loop/build.rs`
- Test: add tests in existing build tests module.

**Interfaces:**
- Consumes `search_source_bookmarks`, `search_active_workflows`.
- Produces prompt sections:
  - `Relevant saved sources and paths:`
  - `Relevant reusable workflows:`

- [ ] **Step 1: Add tests**

Add a test that stores one source and one workflow, calls retrieval helpers with a matching user prompt, and asserts returned text contains source URI and workflow name.

- [ ] **Step 2: Implement budgeted retrieval**

Add:

```rust
async fn retrieve_source_context(user_content: &str) -> String;
async fn retrieve_workflow_context(user_content: &str) -> String;
```

Each returns at most 4 entries and 3000 chars. It must not panic if DB is missing/corrupt.

- [ ] **Step 3: Inject into base prompt**

Include source/workflow context before `skills_part`, so even weak models see it.

- [ ] **Step 4: Test**

Run:

```bash
cargo test --lib retrieve_source_context -- --test-threads=1
cargo check
```

Expected: PASS.

---

### Task 5: Automatic Learning Hooks

**Files:**
- Modify: `src/agent/agent_loop/save.rs`
- Modify: `src/agent/agent_loop/tool_execution.rs`

**Interfaces:**
- Curator can output workflow card JSON and source bookmark JSON.
- Tool execution renderer prints minimal status when `knowledge_source`, `research_brief`, or `workflow_memory` succeeds.

- [ ] **Step 1: Update curator prompt**

Extend the existing self-improvement prompt with:

```text
If the recent task used 3+ tools successfully and represents a reusable procedure, create a workflow card using workflow_memory action=add. Store as draft unless the user explicitly said to remember/use next time, then status=active. Redact secrets. Include preconditions and verification.
If the recent task discovered useful links, repos, docs, local paths, or canonical sources, create source bookmarks using knowledge_source action=add.
```

- [ ] **Step 2: Add TUI summaries**

In `format_tool_outcome_summary`, render:
- `◇ Source saved: <label>` for `knowledge_source add`
- `◇ Sources matched: <n>` for `knowledge_source search`
- `◇ Workflow saved: <name>` for `workflow_memory add`
- `◇ Workflow matched: <name>` for `workflow_memory search`
- `◇ Workflow updated: success <count>` for `workflow_memory record_run`

- [ ] **Step 3: Test output summaries**

Add tests for `format_tool_outcome_summary` if existing tests are present; otherwise test the helper directly.

---

### Task 6: Channel Commands and Docs

**Files:**
- Modify: `src/channels/cli/render.rs`
- Modify: `src/channels/cli/mod.rs`
- Modify: `src/channels/telegram.rs`
- Modify: `README.md`
- Modify: `CHANGELOG.md`

**Interfaces:**
- CLI commands:
  - `/sources <query>` lists source bookmarks.
  - `/workflows <query>` lists workflows.
- Telegram commands:
  - `/sources <query>` returns matching source labels/URIs.
  - `/workflows <query>` returns matching workflow names/status.

- [ ] **Step 1: Add command visibility**

Add `/sources` and `/workflows` to TUI slash menu and Telegram command registration.

- [ ] **Step 2: Add simple handlers**

Handlers call existing search helpers and print concise results. No edit/delete over Telegram in this task; CRUD stays tool-only for safety.

- [ ] **Step 3: Update docs**

Document the feature in README and CHANGELOG.

- [ ] **Step 4: Final verification**

Run:

```bash
cargo fmt
cargo test --lib shared_memory -- --test-threads=1
cargo test --lib test_native_tool_registration_names -- --test-threads=1
cargo check
git diff --check
```

Expected: PASS.

---

## Self-Review

- Spec coverage: Covers source memory, research cache, workflow memory, CRUD tools, automatic learning, TUI visibility, and safety rules.
- Placeholder scan: No placeholders remain; every task has concrete files, interfaces, and verification commands.
- Type consistency: `knowledge_source`, `research_brief`, and `workflow_memory` are consistently named across schema, tools, registration, prompt injection, and channel commands.
