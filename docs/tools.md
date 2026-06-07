# OpenZ Tools Registry & Native Tools 🔧🦀

This document outlines the `Tool` trait, the `ToolRegistry` structure, and the built-in native tools.

---

## 1. The `Tool` Trait

In `src/tools/mod.rs`, every tool must implement the `Tool` trait:

```rust
use anyhow::Result;

#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> serde_json::Value;
    async fn call(&self, arguments: &serde_json::Value) -> Result<serde_json::Value>;
}
```

---

## 2. Built-in Native Tools

* **`read_file`** (`src/tools/filesystem.rs`): Reads the full or partial contents of a file. Supports line ranges (1-indexed).
* **`write_file`** (`src/tools/filesystem.rs`): Writes text to a file, creating any parent folders automatically if they do not exist.
* **`list_dir`** (`src/tools/filesystem.rs`): Lists directory entries showing if they are folders and their sizes.
* **`exec_command`** (`src/tools/shell.rs`): Runs commands in `/bin/sh` (or `cmd.exe` on Windows) and returns stdout, stderr, and the status code.
* **`web_fetch`** (`src/tools/web.rs`): Downloads web pages and parses HTML tags into clean text using regex filters.
