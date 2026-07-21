# Tool Metadata Refactor Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Eliminate match-statement bloat and duplicate static arrays in `src/tools/mod.rs` by allowing each tool struct to define its own metadata via a simplified builder pattern on the `Tool` trait.

**Architecture:** 
- Enhance `ToolMetadata` with builder helper methods: `new()`, `with_risk()`, `with_network()`, `with_writes_disk()`, `with_spawns_process()`, `with_aliases()`, `with_examples()`, `with_usage()`, and `with_timeout()`.
- Rewrite `ToolMetadata::infer(name)` using generic string patterns (heuristics) for fallback/MCP tools, completely removing `infer_tool_domain`, `tool_writes_disk`, `tool_uses_network`, `tool_recommended_timeout`, `tool_aliases`, `tool_examples`, and `tool_usage_hints` matching code.
- Implement explicit overrides of `fn metadata(&self) -> ToolMetadata` for core native tools to ensure exact parity with legacy definitions.

**Tech Stack:** Rust

---

### Task 1: Add ToolMetadata Builder Pattern and Simplified Inference Heuristics

**Files:**
- Modify: `src/tools/mod.rs`

**Step 1: Write the builder methods and replace ToolMetadata::infer & match statement functions**

Modify `src/tools/mod.rs` to add the builders, update `ToolMetadata::infer()`, and delete lines 179 to 663 (everything related to `StaticToolDef`, `STATIC_TOOL_DEFS`, and the match functions).

The new `ToolMetadata` implementation should look like this:

```rust
impl ToolMetadata {
    pub fn new(domain: &'static str) -> Self {
        let priority = match domain {
            "subagent" => 100,
            "filesystem" | "shell" | "code" => 90,
            "self_management" => 85,
            "search" | "web" | "git" => 75,
            "memory" | "reasoning" | "context" => 65,
            "media" | "document" => 55,
            _ => 40,
        };
        Self {
            domain,
            risk: ToolRisk::Low,
            uses_network: false,
            writes_disk: false,
            spawns_process: false,
            requires_approval: false,
            priority,
            aliases: &[],
            examples: &[],
            when_to_use: "",
            when_not_to_use: "",
            recommended_timeout_secs: None,
        }
    }

    pub fn with_risk(mut self, risk: ToolRisk) -> Self {
        self.risk = risk;
        self.requires_approval = matches!(risk, ToolRisk::High);
        self
    }

    pub fn with_network(mut self, uses_network: bool) -> Self {
        self.uses_network = uses_network;
        self
    }

    pub fn with_writes_disk(mut self, writes_disk: bool) -> Self {
        self.writes_disk = writes_disk;
        self
    }

    pub fn with_spawns_process(mut self, spawns_process: bool) -> Self {
        self.spawns_process = spawns_process;
        self
    }

    pub fn with_aliases(mut self, aliases: &'static [&'static str]) -> Self {
        self.aliases = aliases;
        self
    }

    pub fn with_examples(mut self, examples: &'static [&'static str]) -> Self {
        self.examples = examples;
        self
    }

    pub fn with_usage(mut self, when_to_use: &'static str, when_not_to_use: &'static str) -> Self {
        self.when_to_use = when_to_use;
        self.when_not_to_use = when_not_to_use;
        self
    }

    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.recommended_timeout_secs = Some(secs);
        self
    }

    pub fn infer(name: &str) -> Self {
        let domain = if name.contains("subagent") || name == "delegate_task" || name == "parallel_research" || name == "evaluator_optimizer_loop" {
            "subagent"
        } else if name.contains("file") || name.contains("dir") || name == "replace_lines" {
            "filesystem"
        } else if name.contains("exec") || name.contains("sandbox") || name.contains("shell") {
            "shell"
        } else if name.starts_with("git") || name.starts_with("github") {
            "git"
        } else if name.contains("cargo") || name.contains("compiler") || name.contains("grep") || name.contains("outline") || name.contains("ast_grep") || name.contains("rust_docs") {
            "code"
        } else if name.contains("web") || name.contains("browser") || name.contains("crawl") || name.starts_with("searchxyz") || name.contains("social_search") {
            "web"
        } else if name.contains("memory") || name.contains("entities") || name.contains("relations") || name.contains("observations") || name.contains("graph") || name.contains("recall") {
            "memory"
        } else if name.contains("headroom") || name.contains("compress") || name.contains("cache") || name.contains("scope_context") {
            "context"
        } else if name.contains("thinking") || name.contains("reasoning") {
            "reasoning"
        } else if name.contains("doc") || name.contains("pdf") || name.contains("xlsx") || name.contains("docx") || name.contains("pptx") {
            "document"
        } else if name.contains("media") || name.contains("image") || name.contains("video") || name.contains("svg") || name.contains("mermaid") {
            "media"
        } else if name.contains("config") || name.contains("diagnose") || name.contains("session") || name.contains("backup") || name.contains("server") || name.contains("job") || name.contains("whitelist") {
            "self_management"
        } else if name.contains("mcp") {
            "mcp"
        } else {
            "general"
        };

        let writes_disk = name.contains("write")
            || name.contains("patch")
            || name.contains("create")
            || name.contains("update")
            || name.contains("delete")
            || name.contains("remove")
            || name.contains("clear")
            || name.contains("import")
            || name.contains("download");

        let uses_network = name.contains("web")
            || name.contains("search")
            || name.contains("fetch")
            || name.contains("crawl")
            || name.starts_with("git")
            || name.contains("mcp");

        let spawns_process = name.contains("exec")
            || name.contains("sandbox")
            || name.contains("browser")
            || name.starts_with("cargo")
            || name.contains("video")
            || name.contains("mcp");

        let risk = if name.contains("exec") || name.contains("db_write") || writes_disk || name.contains("delete") || name.contains("remove") || name.contains("clear") {
            ToolRisk::High
        } else if uses_network || spawns_process || name.contains("create") || name.contains("update") {
            ToolRisk::Medium
        } else {
            ToolRisk::Low
        };

        let requires_approval = matches!(risk, ToolRisk::High);

        let priority = match domain {
            "subagent" => 100,
            "filesystem" | "shell" | "code" => 90,
            "self_management" => 85,
            "search" | "web" | "git" => 75,
            "memory" | "reasoning" | "context" => 65,
            "media" | "document" => 55,
            _ => 40,
        };

        let recommended_timeout_secs = if domain == "subagent" || name.contains("browser") || name.contains("obscura") || name == "crawl_site" {
            Some(600)
        } else if name == "html_video" || name == "generate_video" {
            Some(900)
        } else if name == "generate_image" || name == "svg_animator" || name == "semantic_search" || name == "mermaid" || name.starts_with("opendoc_") {
            Some(300)
        } else if name.starts_with("mcp_") {
            Some(180)
        } else if name.contains("exec") || name.contains("sandbox") {
            Some(180)
        } else {
            None
        };

        Self {
            domain,
            risk,
            uses_network,
            writes_disk,
            spawns_process,
            requires_approval,
            priority,
            aliases: &[],
            examples: &[],
            when_to_use: "",
            when_not_to_use: "",
            recommended_timeout_secs,
        }
    }
}
```

---

### Task 2: Override `metadata()` for Key Native Tools

**Files:**
- Modify: `src/tools/network.rs`, `src/tools/cargo_manager.rs`, `src/tools/shell.rs`, `src/tools/filesystem.rs`, `src/tools/web.rs`, `src/tools/db_inspector.rs`

Override the `metadata()` trait method for specific tools to match their configurations:

**Step 1: CheckPortTool (`network.rs`)**
```rust
    fn metadata(&self) -> crate::tools::ToolMetadata {
        crate::tools::ToolMetadata::new("self_management")
            .with_risk(crate::tools::ToolRisk::Medium)
            .with_network(true)
            .with_aliases(&["check port", "port list", "port check"])
            .with_examples(&["Check if port 8765 is listening"])
            .with_usage("Use to check if a local port is free or listening.", "Avoid for external network scans.")
    }
```

**Step 2: CargoManagerTool (`cargo_manager.rs`)**
```rust
    fn metadata(&self) -> crate::tools::ToolMetadata {
        crate::tools::ToolMetadata::new("code")
            .with_aliases(&["cargo test", "cargo check", "cargo build", "clippy", "rust tests"])
            .with_examples(&["Run cargo test --lib", "Run cargo check after Rust edits"])
            .with_usage("Use for Rust cargo build, check, test, clippy, and compiler-fix workflows.", "Avoid when only reading files or searching source text.")
    }
```

**Step 3: ExecCommandTool (`shell.rs`)**
```rust
    fn metadata(&self) -> crate::tools::ToolMetadata {
        crate::tools::ToolMetadata::new("shell")
            .with_risk(crate::tools::ToolRisk::High)
            .with_spawns_process(true)
            .with_timeout(180)
            .with_aliases(&["shell command", "terminal", "bash", "run command"])
            .with_examples(&["Run ls to inspect generated files", "Run a safe project-local command"])
            .with_usage("Use for shell commands that cannot be handled by a safer native tool.", "Avoid for file reads, code search, or destructive commands without approval.")
    }
```

**Step 4: ReadFileTool (`filesystem.rs`)**
```rust
    fn metadata(&self) -> crate::tools::ToolMetadata {
        crate::tools::ToolMetadata::new("filesystem")
            .with_aliases(&["open file", "inspect file", "view file"])
            .with_examples(&["Read src/main.rs before editing", "Inspect a config file"])
            .with_usage("Use to inspect known text files before editing or explaining code.", "Avoid for broad searches; use grep or find tools instead.")
    }
```

**Step 5: WriteFileTool (`filesystem.rs`)**
```rust
    fn metadata(&self) -> crate::tools::ToolMetadata {
        crate::tools::ToolMetadata::new("filesystem")
            .with_risk(crate::tools::ToolRisk::High)
            .with_writes_disk(true)
            .with_usage("Use to create new files in the workspace.", "Avoid when appending or updating existing files.")
    }
```

**Step 6: WebFetchTool (`web.rs`)**
```rust
    fn metadata(&self) -> crate::tools::ToolMetadata {
        crate::tools::ToolMetadata::new("web")
            .with_risk(crate::tools::ToolRisk::Medium)
            .with_network(true)
            .with_aliases(&["fetch url", "scrape website", "download web page"])
            .with_examples(&["Fetch API documentation URL", "Scrape text content from a web page"])
            .with_usage("Use to fetch and parse raw text/markdown content from a URL.", "Avoid for general web searches; use web_search instead.")
    }
```

**Step 7: DbWriteTool (`db_inspector.rs`)**
```rust
    fn metadata(&self) -> crate::tools::ToolMetadata {
        crate::tools::ToolMetadata::new("filesystem")
            .with_risk(crate::tools::ToolRisk::High)
            .with_writes_disk(true)
            .with_usage("Use to run insert/update/delete database write statements.", "Avoid for read-only database queries.")
    }
```

**Step 8: ManageWhitelistTool (`manage_whitelist.rs`)**
```rust
    fn metadata(&self) -> crate::tools::ToolMetadata {
        crate::tools::ToolMetadata::new("self_management")
            .with_risk(crate::tools::ToolRisk::High)
            .with_writes_disk(true)
            .with_usage("Use to add, remove, or list whitelisted paths and command prefixes.", "Avoid for normal filesystem or workspace actions.")
    }
```

---

### Task 3: Verify and Build

**Step 1: Check and test**
Run: `cargo check`
Run: `cargo test`

**Step 2: Commit**
```bash
git add src/
git commit -m "refactor: tool metadata match statements to trait overrides"
```
