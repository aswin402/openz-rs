# OpenZ Model Context Protocol (MCP) Integration 🔌🦀

This document describes how `openz` communicates with stdio-based and gRPC-based Model Context Protocol (MCP) servers and registers their tools dynamically.

---

## 1. Stdio-Based JSON-RPC Protocol

The `McpClient` in `src/tools/mcp.rs` spawns an external server command in a subprocess with standard pipes:
1. **Handshake:** Sends an `initialize` JSON-RPC request to set up capabilities.
2. **Setup Notification:** Dispatches `notifications/initialized` to signal readiness.
3. **Tool Listing:** Queries `tools/list` to retrieve all available tools supported by the server.
4. **Execution:** Delegates tool calls to the server via `tools/call`.

---

## 2. Dynamic Tool Wrapping

Tools returned by the MCP server are converted into native `openz` tools using `McpToolWrapper`:

```rust
pub struct McpToolWrapper {
    pub client: McpClient,
    pub name: String,
    pub description: String,
    pub parameters: Value,
}
```

Since `McpToolWrapper` implements the `Tool` trait, the agent can call external tools exactly like native Rust functions.

---

## 3. Native Tools vs MCP Defaults

OpenZ now ships many former MCP capabilities as native Rust tools registered directly in the tool registry:
*   **Headroom/context compression**: `scope_context`, `compress_content`, `retrieve_original`, `compress_file`, `compress_diff`, `compress_url`, and related CCR/cache tools are native.
*   **Sequential thinking**: `sequentialthinking`, `analyze_graph`, `export_session`, `summarize_reasoning`, and `reasoning_templates` are native.
*   **Memory graph and extended memory**: graph nodes/relations, working memory, semantic facts, fact history, workflow memory, research briefs, and knowledge sources are native SQLite-backed tools.

MCP remains supported for external servers. Default MCP entries focus on tools that are still external or optional, while native replacements should not be duplicated as default MCP servers.

---

## 4. MCP Management System

OpenZ provides a dual-layer management system for MCP configurations:
1.  **`manage_mcp` tool**: Allows the agent to list, add, remove, enable, or disable server definitions inside `~/.openz/config.json` dynamically.
2.  **`mcps_manager` subagent**: A protected subagent designed to inspect runtimes (Rust/Node/Python), resolve dependencies, and automatically install/setup new MCP servers on demand.

---

## 5. Unified gRPC/Tonic Transport & Bridge

To solve stdio pollution (where random logging/stderr/stdout outputs from third-party libraries break the JSON-RPC parser), OpenZ unifies all workspace MCP server communication over **gRPC using Tonic**:

### In-Process gRPC Bridge
For servers that natively only support stdio transport (e.g. `database-mcp`, `chromewright`, `opendocswork-mcp`), OpenZ runs an automatic in-process bridge:
1. **Dynamic Port Allocation**: Automatically allocates an ephemeral TCP port on localhost.
2. **Subprocess Management**: Spawns the stdio target server process with private pipes.
3. **Robust Noise Filtering**: Reads lines from the subprocess's stdout and filters out any non-JSON logs (logging pollution) before sending correct responses back over the gRPC Tonic channel.
4. **Timeouts & Safety**: Includes connection timeouts and EOF checks to prevent busy loops and indefinite connection hangs.
