---
name: tools
description: "AI Agent Skill for OpenZ Tools — details how to implement, register, and configure tools, handle security approval, and manage large tool outputs."
metadata:
  version: 1.0.0
---

# OpenZ Tools Integration Guide 🔧🦀

This skill outlines how to implement the `Tool` trait, register tools in the registry, handle parameters/casing aliases, integrate with the `SecurityGuard`, and manage large outputs.

## 1. Implementing the `Tool` Trait

All native tools must implement the `Tool` trait from `src/tools/mod.rs`:

```rust
use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

#[async_trait]
pub trait Tool: Send + Sync {
    /// Unique name of the tool. Typically snake_case.
    fn name(&self) -> &str;

    /// Detailed description of the tool to help the LLM choose when and how to use it.
    fn description(&self) -> &str;

    /// The JSON Schema parameters defining required and optional arguments.
    fn parameters(&self) -> Value;

    /// Asynchronous execution callback.
    async fn call(&self, arguments: &Value) -> Result<Value>;
}
```

---

## 2. Registering a Tool

Register new tools in `ToolRegistry` (constructed in `src/cli/builder.rs` or `src/tools/mod.rs`):
- Use `registry.register(Arc::new(MyNewTool::new()))`.
- Custom subagents (from `~/.openz/subagents.json`) are dynamically loaded as tools (`DelegateProfileTool`) at LLM runtime via the registry's `.get()` method.

---

## 3. Important Gotchas & Guidelines

### Argument Naming and Aliases
Tool arguments do not follow a single unified convention. Some models supply `camelCase` while others supply `snake_case`.
- Handle casing differences gracefully in your `call()` implementation.
- Check `format_tool_args` in `src/agent/agent_loop.rs` to map your tool arguments to friendly display names and formatting strings for the user's progress spinner.

### Handling Large Outputs (>4,000 Characters)
If a tool returns output larger than 4,000 characters:
1. OpenZ automatically dumps the full output to `~/.openz/tool_outputs/<tool_name>_<uuid>.json`.
2. The output is compacted using the `context_compactor` (Z-Context / Headroom).
3. Only the compacted summary and file reference are sent to the LLM to prevent context pollution.

### Security and Sensitive Actions
Any tool that executes shell commands or writes files must be declared sensitive.
- Check and update `SecurityGuard::is_sensitive` inside `src/agent/security.rs` to intercept these tools.
- When intercepted, the channel prompt pauses and requests user approval before execution begins.

---

## 4. SearchXyz Integrated Tools 🔎

OpenZ natively integrates the `searchxyz` search suite for advanced keyless web search, scraping, document indexing, and Knowledge Graph management. These tools are prefix-registered under `searchxyz_`:

- `searchxyz_search_web`: Search the web (DuckDuckGo, Google, Bing, Brave, SearXng).
- `searchxyz_read_url`: Parse URLs, PDFs, YouTube transcripts, or Git repositories into clean Markdown.
- `searchxyz_search_and_read`: Combine web searching and crawling of top results in one call.
- `searchxyz_recall`: Semantic and keyword search over indexed documents.
- `searchxyz_list_sources`: List all indexed sources and cached pages.
- `searchxyz_deep_research`: Perform recursive multi-query crawls and compile a markdown report.
- `searchxyz_index_content`: Index custom text documents.
- `searchxyz_site_map`: Map website page trees and links.
- `searchxyz_index_relationship`: Insert node connections into the Knowledge Graph.
- `searchxyz_query_graph`: Query and traverse the local Knowledge Graph.
- `searchxyz_read_github_repo`: Clone and index codebases.
- `searchxyz_export_research`: Export local documents to a portable bundle.
- `searchxyz_import_research`: Load external document bundles.
- `searchxyz_delete_source`: Evict documents by URL or prefix.
- `searchxyz_clear_index`: Clear all documents and Graph data.

