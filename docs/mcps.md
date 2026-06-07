# OpenZ Model Context Protocol (MCP) Integration 🔌🦀

This document describes how `openz` communicates with stdio-based MCP servers and registers their tools dynamically.

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

Since `McpToolWrapper` implements the `Tool` trait, the agent can call external tools exactly like native rust functions.
