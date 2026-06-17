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

## 3. Rust-Native Defaults

By default, OpenZ is pre-configured to utilize high-performance, lightweight Rust-native MCP servers resolved at runtime from `~/.cargo/bin/`, bypassing Node.js/npx runtimes completely:
*   **`office`**: compiled local-first parser (`opendocswork-mcp` under `~/.cargo/bin/`) to extract text and tables from `.docx`, `.xlsx`, and `.pptx` documents.
*   **`headroom`**: context compression and directory scoping server (`headroom-mcp` under `~/.cargo/bin/`) to optimize context windows dynamically. Employs `scope_context` to compile hierarchical `AGENTS.md` instructions and `compress_content` / `retrieve_original` for token budget optimizations.
*   **`sequential-thinking`**: sequential reasoning server (`mcp-server-sequential-thinking` under `~/.cargo/bin/`) to plan and double check execution paths.
*   **`memory`**: memory graph database (`openmemory_rs` under `~/.cargo/bin/`) for storing semantic entity relationships.

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
