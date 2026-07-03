# Architecture Document — opendoc-mcp

**Version:** 0.0.1
**Status:** Active
**Last Updated:** 2026-06-28

---

## 1. System Overview

`opendoc-mcp` is an MCP (Model Context Protocol) server that exposes document manipulation capabilities as tools that AI assistants can call. It follows a **modular handler architecture** where each document format is implemented as an independent module behind a unified MCP interface.

### Core Design Principles

1. **Single Binary** — No runtime dependencies. Compile once, run anywhere.
2. **Zero-Copy Where Possible** — Stream data rather than loading entire documents into memory.
3. **Fail Fast** — Validate inputs early, return structured error JSON.
4. **Format Isolation** — Each format handler is independent; adding a new format means adding one file.
5. **MCP-First** — Every capability is exposed as a tool; no hidden APIs.

---

## 2. Architecture Diagram

```
┌══════════════════════════════════════════════════════════════┐
║                      MCP Host (Client)                       ║
║  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────────┐ ║
║  │  Claude   │  │  Cursor   │  │ VS Code  │  │ Custom Agent │ ║
║  │  Desktop  │  │           │  │ (Cline)  │  │              │ ║
║  └─────┬─────┘  └────┬──────┘  └────┬─────┘  └──────┬───────┘ ║
║        │              │              │                │         ║
║        └──────────────┴──────────────┴────────────────┘         ║
║                          │ JSON-RPC 2.0 over stdio              ║
╚══════════════════════════╪═══════════════════════════════════════╝
                           │
┌──────────────────────────▼──────────────────────────────────────┐
│                    opendoc-mcp Server                           │
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │                    Transport Layer                        │  │
│  │              stdin/stdout (JSON-RPC 2.0)                  │  │
│  │         rmcp::transport::stdio — MCP Protocol             │  │
│  └────────────────────────┬─────────────────────────────────┘  │
│                           │                                     │
│  ┌────────────────────────▼─────────────────────────────────┐  │
│  │                   Server Layer (server.rs)                │  │
│  │                                                          │  │
│  │  ┌────────────────────────────────────────────────────┐  │  │
│  │  │           OpendocServer (struct)                   │  │  │
│  │  │                                                    │  │  │
│  │  │  #[tool(description="...")]                        │  │  │
│  │  │  fn create_document(...)  → docx::create_document  │  │  │
│  │  │  fn open_document(...)    → docx::open_document    │  │  │
│  │  │  fn add_paragraph(...)    → docx::add_paragraph    │  │  │
│  │  │  fn add_table(...)        → docx::add_table        │  │  │
│  │  │  fn create_pdf(...)       → pdf::create_pdf        │  │  │
│  │  │  fn merge_pdfs(...)       → pdf::merge_pdfs        │  │  │
│  │  │  ... (18+ tools)                                   │  │  │
│  │  └────────────────────────────────────────────────────┘  │  │
│  └────────────────────────┬─────────────────────────────────┘  │
│                           │                                     │
│  ┌────────────────────────▼─────────────────────────────────┐  │
│  │                 Handler Layer (handlers/)                 │  │
│  │                                                          │  │
│  │  ┌──────────────┐  ┌──────────────┐  ┌────────────────┐  │  │
│  │  │   docx.rs    │  │   pptx.rs    │  │    pdf.rs      │  │  │
│  │  │              │  │              │  │                │  │  │
│  │  │ • rdocx      │  │ • pptx       │  │ • lopdf        │  │  │
│  │  │ • create     │  │ • create     │  │ • create       │  │  │
│  │  │ • open       │  │ • open       │  │ • open         │  │  │
│  │  │ • edit       │  │ • edit       │  │ • merge        │  │  │
│  │  │ • convert    │  │ • convert    │  │ • extract      │  │  │
│  │  └──────────────┘  └──────────────┘  └────────────────┘  │  │
│  │                                                          │  │
│  │  (Future handlers)                                       │  │
│  │  ┌──────────────┐  ┌──────────────┐  ┌────────────────┐  │  │
│  │  │   xlsx.rs    │  │   html.rs    │  │    md.rs       │  │  │
│  │  │   (v0.1.0)   │  │   (v0.1.0)   │  │   (v0.1.0)    │  │  │
│  │  └──────────────┘  └──────────────┘  └────────────────┘  │  │
│  └──────────────────────────────────────────────────────────┘  │
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │                 Core Dependencies                         │  │
│  │                                                          │  │
│  │  rmcp (MCP SDK)  → Protocol, transport, tool macros      │  │
│  │  tokio           → Async runtime                         │  │
│  │  serde/serde_json → Structured JSON I/O                  │  │
│  │  anyhow/thiserror → Error handling                       │  │
│  │  tracing         → Structured logging                    │  │
│  └──────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────────┘
```

---

## 3. Module Structure

```
src/
├── main.rs              # Binary entry point
│                        # - Initializes tracing/logging
│                        # - Creates OpendocServer
│                        # - Calls server.run()
│
├── lib.rs               # Library root
│                        # - pub mod ir (Internal Representation)
│                        # - pub mod handlers
│                        # - pub mod engine
│                        # - pub mod converters
│                        # - pub mod validators
│                        # - pub mod batch
│                        # - pub mod ocr
│                        # - pub mod server
│                        # - pub mod cli
│                        # - pub mod types
│
├── server.rs            # MCP Server implementation
│                        # - OpendocServer struct
│                        # - #[tool] attribute macros
│                        # - ServerHandler impl (get_info)
│                        # - new() and run() methods
│
├── ir/                  # Internal Representation
│   ├── mod.rs           # Re-exports
│   ├── document.rs      # Document, Section structs
│   ├── elements.rs      # Paragraph, Table, Image, etc.
│   └── metadata.rs      # Metadata (title, author, form_fields, etc.)
│
├── engine/              # IR-based operations
│   ├── mod.rs           # edit_pipeline, EditOperation
│   ├── search.rs        # Keyword/regex search
│   ├── replace.rs       # Find & replace
│   ├── template.rs      # {{placeholder}} filling
│   ├── diff.rs          # Document comparison
│   └── complexity.rs    # Complexity analysis, OCR detection
│
├── converters/          # Cross-format conversion
│   ├── mod.rs           # convert(), xlsx_to_csv, etc.
│   └── transmutation.rs # Advanced format mapping
│
├── validators/          # Document validation
│
├── batch/               # Parallel batch processing
│
├── ocr/                 # OCR pipeline (feature-gated)
│   └── mod.rs           # ocr_document, is_ocr_available
│
├── types.rs             # Re-exports (rmcp::*)
│
└── handlers/            # Document format handlers
    ├── mod.rs           # load_to_ir() entry point
    ├── docx.rs          # rdocx: create, open, edit, convert
    ├── pdf.rs           # lopdf: create, open, merge, extract
    ├── pdf_forms.rs     # lopdf: AcroForm list + fill
    ├── pptx.rs          # pptx: create, open, edit, convert
    └── xlsx.rs          # calamine: read → IR (tables + sections)
```

---

---

## 6. New Feature Modules

### 6.1 PDF Forms (`handlers/pdf_forms.rs`)

Uses lopdf directly to read/write AcroForm fields:
- `list_form_fields()` — Walks the AcroForm tree, returns all fields with types/values/flags
- `fill_form_fields()` — Sets /V and /AS entries on field dictionaries
- No new dependencies (uses existing lopdf)
- Supports TextField (Tx), CheckBox/Radio (Btn), Choice (Ch), Signature (Sig)

### 6.2 Complexity Analysis (`engine/complexity.rs`)

Pure heuristic analysis on the IR:
- Detects scanned PDFs (no text + images present)
- Measures text density (chars/page)
- Assigns Simple/Moderate/Complex/Scanned level
- Recommends pipeline (text/spatial/ocr)
- Zero new dependencies

### 6.3 OCR Pipeline (`ocr/mod.rs`)

Feature-gated behind `--features ocr`:
- When disabled: returns helpful error with setup instructions
- When enabled: planned pipeline using pdfium-render + tesseract (v0.2.0)
- `OcrConfig` struct for language, DPI, and preprocessing options

---

## 7. Feature Flags

| Flag | Default | Purpose | Dependencies Added |
|------|---------|---------|--------------------|
| `cli` | ✅ | CLI subcommands | clap |
| `server` | ✅ | MCP server | rmcp, tokio |
| `ocr` | ❌ | OCR engine | pdfium-render, tesseract (future) |
| `wasm` | ❌ | WASM target | wasm-pack (future) |

---

## 8. Data Flow

### 4.1 Tool Invocation Flow

```
┌────────┐    JSON-RPC Request     ┌────────┐    Function Call    ┌─────────┐
│  MCP   │ ───────────────────────►│ Server │ ──────────────────►│ Handler │
│ Client │                         │ Layer  │                    │ Module  │
│        │◄─────────────────────── │        │◄────────────────── │         │
└────────┘    JSON-RPC Response    └────────┘    Result JSON     └─────────┘

Example (create_document):

1. Client sends:
   {
     "jsonrpc": "2.0",
     "method": "tools/call",
     "params": {
       "name": "create_document",
       "arguments": {
         "file_path": "/tmp/report.docx",
         "title": "Q4 Report"
       }
     }
   }

2. Server dispatches to docx::create_document("/tmp/report.docx", Some("Q4 Report"))

3. Handler creates document via rdocx, saves to file

4. Server returns:
   {
     "jsonrpc": "2.0",
     "result": {
       "content": [{
         "type": "text",
         "text": "{\n  \"success\": true,\n  \"path\": \"/tmp/report.docx\",\n  \"format\": \"docx\"\n}"
       }]
     }
   }
```

### 4.2 Error Handling Flow

```
Handler Function
      │
      ├── Ok(value) ──► serde_json::json!(value).to_string() ──► Success Response
      │
      └── Err(e) ──► format!("{{\"error\":\"{e}\"}}") ──► Error JSON Response
                           │
                           └── All errors are stringified into JSON
                               with an "error" key. The MCP protocol
                               wraps this in its own error envelope
                               for transport-level failures.
```

---

## 9. Component Details

### 9.1 Transport Layer (`rmcp`)

- **Protocol:** JSON-RPC 2.0 over stdio
- **Transport:** Standard input/output (stdin/stdout)
- **Framing:** Newline-delimited JSON messages
- **Capability negotiation:** Automatic on connection

The `rmcp` crate handles all MCP protocol details:
- Lifecycle management (initialize, ping, shutdown)
- Tool discovery (`tools/list`)
- Tool execution (`tools/call`)
- Error formatting and protocol-level error codes

### 9.2 Server Layer (`server.rs`)

The `OpendocServer` struct uses `rmcp`'s `#[tool]` attribute macro to register tools:

```rust
#[derive(Debug, Clone, Default)]
pub struct OpendocServer;

#[tool(tool_box)]
impl OpendocServer {
    #[tool(description = "Create a new DOCX document...")]
    fn create_document(
        &self,
        #[tool(param)]
        #[schemars(description = "File path...")]
        file_path: String,
        #[tool(param)]
        #[schemars(description = "Optional title...")]
        title: Option<String>,
    ) -> String {
        docx::create_document(&file_path, title.as_deref())
    }
    // ... more tools
}
```

**Key pattern:** All tools return `String` (JSON). This keeps the server layer thin — it's just a router.

### 9.3 Handler Layer (`handlers/`)

Each handler module follows a consistent pattern:

```rust
// 1. Error adapter function (private)
fn format_result<T: Serialize>(result: Result<T, Error>) -> String {
    match result {
        Ok(val) => serde_json::to_string_pretty(&val).unwrap_or_default(),
        Err(e) => format!("{{\"error\":\"{e}\"}}"),
    }
}

// 2. Public functions called by server layer
pub fn create_document(file_path: &str, title: Option<&str>) -> String { ... }
pub fn open_document(file_path: &str) -> String { ... }
```

**Handler responsibilities:**
- Open/save document files
- Execute requested operation
- Return JSON string (success or error)

### 9.4 Type System (`types.rs`)

Currently re-exports `rmcp::*` for convenience. In future versions, this module will contain shared types:
- `DocumentMetadata` — Common metadata struct
- `ConversionOptions` — Shared conversion configuration
- `ToolResult<T>` — Unified result type

---

## 10. Dependency Graph

```
opendoc-mcp
├── rmcp (MCP SDK) [optional, server feature]
│   ├── tokio (async runtime)
│   └── serde_json (JSON handling)
├── rdocx (DOCX handler)
│   ├── zip (OOXML packaging)
│   └── quick-xml (XML parsing)
├── pptx (PPTX handler)
├── ppt-rs (PPTX ECMA-376)
├── lopdf (PDF handler)
│   └── pdf_forms (AcroForm)
├── calamine (XLSX read → IR)
├── rust_xlsxwriter (XLSX write)
├── csv (CSV)
├── regex (find/replace)
├── comrak + pulldown-cmark (Markdown)
├── html5ever + scraper (HTML)
├── image (image processing)
├── rayon (parallel batch)
├── serde / serde_json (serialization)
├── clap [optional, cli feature]
├── anyhow / thiserror (error handling)
└── tracing / tracing-subscriber (logging)
```

### Dependency Requirements

| Crate | Version | Purpose | Alternative |
|-------|---------|---------|-------------|
| `rmcp` | 0.1 | MCP protocol | None (only Rust MCP SDK) |
| `tokio` | 1 | Async runtime | smol, async-std |
| `rdocx` | 0.1 | DOCX read/write/convert | docx-rs, docx_rust |
| `pptx` | 0.1 | PPTX read/write | Custom OPC |
| `lopdf` | 0.31 | PDF read/write/merge + AcroForm | pdf.rs, printpdf |
| `calamine` | 0.24 | XLSX read (pure Rust) | xlsx |
| `rust_xlsxwriter` | 0.68 | XLSX write | xlsx |
| `clap` [opt] | 4 | CLI arg parser | None |
| `serde` | 1 | Serialization | None |
| `anyhow` | 1 | Error handling | eyre |
| `tracing` | 0.1 | Logging | log |

---

## 11. Security Architecture

### 7.1 Threat Model

| Threat | Impact | Mitigation |
|--------|--------|------------|
| Path traversal | Read/write outside allowed dirs | Validate all paths with `canonicalize()` |
| Large file DoS | Memory exhaustion | Stream processing, size limits |
| Malformed document | Crash/panic | Defensive parsing, `Result`-based error handling |
| Shell injection | Arbitrary command execution | No subprocess calls, no shell commands |
| Sensitive data leak | Document data exposed | No telemetry, no network calls |

### 7.2 Security Boundaries

```
┌─────────────────────────────────────────────┐
│            MCP Host Process                  │
│  (Claude Desktop / VS Code / etc.)           │
│                                              │
│  ┌──────────────────────────────────────┐   │
│  │       opendoc-mcp (subprocess)        │   │
│  │                                       │   │
│  │  • No network access                  │   │
│  │  • No shell access                    │   │
│  │  • Only reads/writes to paths passed  │   │
│  │    as tool arguments                  │   │
│  │  • All I/O through stdio JSON-RPC     │   │
│  └──────────────────────────────────────┘   │
│                                              │
│  ┌──────────────────────────────────────┐   │
│  │         Filesystem                    │   │
│  │  Documents are read/written here      │   │
│  └──────────────────────────────────────┘   │
└─────────────────────────────────────────────┘
```

---

## 12. Performance Design

### 8.1 Startup Optimization

- **Lazy loading:** Handler crates are loaded at compile-time (no runtime discovery)
- **No initialization:** Server is ready immediately — no DB connections, no network calls
- **Minimal imports:** Only necessary dependencies are compiled

### 8.2 Runtime Efficiency

- **Direct memory mapping:** Use `mmap` for large file reads where applicable
- **Streaming writes:** `lopdf` and `rdocx` support streaming save
- **No cloning:** Prefer references over owned data in hot paths
- **Arena allocation:** Avoid repeated allocations in loops

### 8.3 Benchmark Targets

```
Operation                   Target      Current (v0.0.1)
───────────────             ──────      ─────────────────
Binary size (stripped)      < 5 MB      ~4.2 MB
Startup to ready            < 5 ms      ~3 ms
DOCX create (1 para)        < 2 ms      ~1.5 ms
DOCX open (10 pages)        < 5 ms      ~3 ms
PDF create (1 page)         < 3 ms      ~2 ms
PDF merge (5 files)         < 10 ms     ~8 ms
DOCX → PDF (10 pages)       < 50 ms     ~30 ms
Memory (idle)               < 5 MB      ~3.5 MB
```

---

## 13. Future Architecture

### 13.1 v0.2.0 — OCR & Advanced PDF

- Real OCR implementation behind `ocr` feature flag
- PDF form field creation (not just fill)
- PDF/A validation
- Image extraction from documents

### 13.2 v0.3.0 — WASM & Enterprise

```
                     ┌─────────────────────┐
                     │   BatchProcessor    │
                     │  ┌───────────────┐  │
                     │  │ • Directory   │  │
                     │  │ • Recursive   │  │
                     │  │ • Filter      │  │
                     │  └───────────────┘  │
                     └─────────────────────┘
                     ┌─────────────────────┐
                     │   TextChunker       │
                     │  ┌───────────────┐  │
                     │  │ • By heading  │  │
                     │  │ • By tokens   │  │
                     │  │ • By size     │  │
                     │  └───────────────┘  │
                     └─────────────────────┘
```

### 13.3 v1.0.0 — Architecture Modularization

```
                    ┌──────────────────────┐
                    │   opendoc-mcp-core   │ (no_std + wasm compatible)
                    │                      │
                    │  ┌────────────────┐  │
                    │  │ • All handlers │  │
                    │  │ • No I/O      │  │
                    │  │ • Pure data   │  │
                    │  └────────────────┘  │
                    └──────────────────────┘
                              │
              ┌───────────────┴───────────────┐
              │                               │
    ┌─────────▼─────────┐          ┌──────────▼──────────┐
    │ opendoc-mcp-server │          │ opendoc-mcp-wasm    │
    │ (native binary)    │          │ (browser/edge)      │
    │ stdio transport    │          │ Streamable HTTP     │
    └───────────────────┘          └─────────────────────┘
```

---

## 14. Design Decisions

| Decision | Rationale |
|----------|-----------|
| **Return `String` (JSON) from tools** | Simplifies protocol layer; MCP content is always text |
| **Each handler is independent** | Adding a format = adding one file, no changes to server.rs |
| **rdocx over docx-rs** | rdocx has built-in PDF/HTML/MD conversion, larger API surface |
| **lopdf over printpdf** | lopdf supports reading/editing/merging, not just creation |
| **calamine over xlsx** | Pure Rust, no C binding, reads both .xlsx and .xls |
| **Regex for find/replace** | More powerful than plain text; agents can use regex patterns |
| **No async in handlers** | File I/O is fast enough; async adds complexity without benefit |
| **IR-centric architecture** | One document model for all formats — add format = add importer |
| **Feature-gated heavy deps** | OCR deps (pdfium, tesseract) only compile when requested |
| **No configuration file** | CLI arguments only; keeps the server stateless and simple |
