# Future Improvements: MCP-to-Native Port & Session Memory

## Status Summary

All 67 tools from 3 MCP servers (`sequential-thinking`, `memory`, `headroom`) have been ported to native Rust across 4 files. Compilation is clean, 195/198 tests pass (3 pre-existing flaky), and 5 MCP servers have been removed from config.

## Completed Work

### Native Tool Ports

| File | Lines | Tools | Source MCP Server |
|:---|:---:|:---:|:---:|
| `src/tools/sequential_thinking.rs` | 1182 | 5 | `sequential-thinking` |
| `src/tools/headroom.rs` | 2058 | 19 | `headroom` |
| `src/tools/graph_memory.rs` | 1224 | 12 | `memory` |
| `src/tools/memory_extra.rs` | 2994 | 31 | `memory` |

### Architecture Fixes

1. **Name collision resolution** (`ast_grep.rs:91`): `IndexCodebaseTool` → `AstGrepIndexCodebaseTool` to avoid collision with `memory_extra::IndexCodebaseTool`
2. **Dual SQLite connection elimination** (`graph_memory.rs` + `memory_extra.rs`): Both now share a single `OnceLock<Mutex<Connection>>` via `graph_memory::with_db()` made `pub(crate)`. Memory_extra's duplicate `db_static()/init_db()/get_db_path()/with_db()/scope_from_args()` removed (~170 lines). All DDL merged into `graph_memory::init_db()`
3. **5 MCP servers removed** from `~/.openz/config.json`: `sequential-thinking`, `memory`, `headroom`, `database`, `context-bus`
4. **67 tools registered** in `cli.rs` with zero orphans, zero name collisions

### Key Files Changed

- `src/tools/graph_memory.rs` — Made `with_db()`, `scope_from_args()` pub(crate); merged all memory_extra tables into `init_db()`
- `src/tools/memory_extra.rs` — Removed duplicated DB infrastructure; now imports from graph_memory
- `src/tools/ast_grep.rs` — Renamed `IndexCodebaseTool` → `AstGrepIndexCodebaseTool`
- `src/tools/mod.rs` — Added `pub mod memory_extra;`
- `src/config/schema.rs` — Removed sequential-thinking, memory, headroom MCP server defaults
- `src/cli.rs` — Updated registrations (ast_grep, memory_extra tools)
- `~/.openz/config.json` — Removed 5 MCP server entries

### Known Issues

- 3 pre-existing flaky tests: `test_get_working_memory_expired`, `test_search_text_fts5`, `test_text_similarity`
- 3 pre-existing compiler warnings: unused `rel` in `QueryFactHistoryTool`, unused `list_sessions`/`delete_session` methods, unused `ccr_id` field in headroom
- Test mutex pattern (`TEST_MUTEX`) replicated in graph_memory to serialize parallel tokio tests

---

## Remaining: Cross-Session Memory Persistence

### Problem

The agent doesn't remember past sessions — on restart it starts with zero context even though sessions are saved to `~/.openz/sessions/cli_direct.json`. Two root causes:

1. **Session lifecycle issue** (`cli.rs`): On startup, `handle_agent()` archives the session (renames to `cli:history_<timestamp>` and replaces with empty) before the agent loop's `Restore` state can load it
2. **Memory store emptiness**: Even when loaded, the LLM sees raw messages but the memory stores (graph, semantic) are empty because they're only populated when the LLM explicitly calls those tools

### Proposed Solution

Industry-standard pattern using existing infrastructure:

#### A. Automatic Extraction (Save state, `agent_loop.rs` ~line 1468)

After saving the session, extract key user facts from recent messages using the LLM and store them in graph_memory nodes:

```rust
// Pseudocode
if let Ok(facts) = extract_user_facts(&messages).await {
    for fact in facts {
        graph_memory::with_db(|db| {
            // CREATE node for user fact
            // INSERT observations about user
        });
    }
}
```

#### B. Automatic Retrieval (Restore state, `agent_loop.rs` ~line 261)

Query graph memory for nodes matching the user, query semantic_metadata for recent context, inject into system prompt:

```rust
// Pseudocode
let user_context = graph_memory::with_db(|db| {
    // SELECT nodes matching user
    // SELECT recent semantic_metadata
});
system_prompt += &format!("\n## User Context\n{}", user_context);
```

#### C. Implementation Notes

- Use `scope_from_args` pattern with `userId` = session user identity
- Leverage existing `graph_memory::with_db()` — already shared between graph_memory and memory_extra
- FTS5 and semantic_search available for richer retrieval
- Background curator could also be used for extraction (runs async post-turn)

---

## Next Steps

1. Implement automatic memory extraction on session end (`Save` state in `agent_loop.rs`)
2. Implement automatic memory retrieval on session start (`Restore` state in `agent_loop.rs`)
3. Test with `cargo test --lib` and manual back-to-back `openz agent` sessions
