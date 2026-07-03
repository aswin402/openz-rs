# AGENTS.md — opendoc-mcp

**Rust-native MCP server** for document CRUD, conversion, and intelligence. Exposes tools to AI agents via the Model Context Protocol (stdio). No external deps, no LibreOffice, no cloud APIs.

---

## Commands

```bash
cargo check --all-features   # Fast compile check (CI gate)
cargo test --all-features    # Run all unit + integration tests
cargo build --release        # Production binary
cargo clippy --all-features -- -D warnings  # Lint check (CI gate)
RUST_LOG=debug cargo run     # Run MCP server with debug logging
cargo run -- <args>          # Run CLI mode (e.g. `info path/to/doc.docx`)
```

---

## Architecture (IR-Centric)

The entire system revolves around an **Internal Representation (IR)**:

```
DOCX ──┐
PPTX ──┤
PDF  ──┤──▶  IR  ──▶  engine (search/replace/template/diff) ──▶  export
XLSX ──┤
HTML ──┤
MD/CSV─┘
```

- `src/ir/` — `Document`, `Paragraph`, `Table`, `Section`, `Image`, `Metadata`
- `src/handlers/mod.rs` — `load_to_ir(file_path)` detects format by extension, returns `Document`
- `src/engine/` — All editing (search, replace, template, diff, complexity) works on IR, so every operation works on **all formats**
- `src/converters/` — Cross-format conversion via IR (or direct crate APIs for DOCX→PDF etc.)
- `src/handlers/` — Format-specific import/export (`docx.rs`, `pdf.rs`, `pptx.rs`, `xlsx.rs`, `html.rs`, `md.rs`, `csv.rs`, `pdf_forms.rs`)

## Two Entry Points

`src/main.rs` determines mode based on `std::env::args().len() > 1`:

1. **MCP Server** (default, no args) — runs `OpendocServer` via rmcp stdio transport
2. **CLI** (args provided) — parses subcommands via clap (`convert`, `extract`, `batch`, `merge`, `validate`, `info`, `diff`, `formats`, `serve`)

## Key Patterns & Gotchas

### Tools Return `String` (JSON)
Every `#[tool]` function returns `String` (serialized JSON), not a structured type. Pattern:
```rust
// Success:
serde_json::json!({"success": true, "path": path}).to_string()
// Error:
serde_json::json!({"error": "message"}).to_string()
```

### Path Security
Every tool uses `validate_path!()` macro (defined in `server.rs`) which:
- Rejects null bytes
- Canonicalizes paths (blocks traversal)
- If `OPENDOC_ALLOWED_DIRS` env var is set, restricts access to those directories

### Feature Flags (Cargo.toml)
| Flag | Default | Adds |
|------|---------|------|
| `cli` | ✅ | clap (CLI subcommands) |
| `server` | ✅ | rmcp + tokio (MCP server) |
| `ocr` | ❌ | OCR (placeholder until v0.2.0) |
| `wasm` | ❌ | WASM target library compilation (builds with `--no-default-features` for edge/browser execution) |

### Error Handling Pattern
Handlers never panic. Every error is `serde_json::json!({"error": ...}).to_string()`.

### PPTX Image & PDF Conversion Limitations
- `add_slide_image` — placeholder, always returns `"success": false`
- `pptx_to_pdf` / `presentation_to_pdf` — placeholder, returns guidance
- `OCR` — returns setup instructions unless `--features ocr` is used

### Test Helpers
- `tests/common/mod.rs` provides `temp_path()` (unique temp paths) and fixture generators (`gen_txt`, `gen_csv`, `gen_xlsx`)
- Unit tests live inline in each `#[cfg(test)] mod tests`
- Integration tests in `tests/integration.rs` test the full `load_to_ir` → IR → engine pipeline
- Tests use atomic counters to avoid path collisions
- DOCX/PDF/PPTX integration tests are in-module (not in `tests/`) since they need crate-specific setup

### Engine Module
- `engine::edit_pipeline()` supports `EditOperation` enum for batch edits
- `engine::search` searches paragraphs, sections, tables, and raw text
- `engine::replace` and `engine::template` operate on IR in-memory — changes may not be persistable back to native format (return `"persisted": false` note)
- `engine::diff` uses LCS (longest common subsequence) — paragraph-level only, not character-level
- `engine::complexity` uses heuristics on IR (text density, image presence, table count)

### Module Structure (src/)
```
main.rs          — Binary entry, mode dispatch
lib.rs           — Module exports (pub mod ir, handlers, engine, ...)
server.rs        — #[tool] definitions, validate_path! macro, ServerHandler impl
cli.rs           — clap subcommands
ir/              — Document, Paragraph, Table, Section, Image, Alignment, Metadata
engine/          — search, replace, template, diff, complexity
handlers/        — docx/pptx/pdf/xlsx/html/md/csv/pdf_forms + load_to_ir()
converters/      — Cross-format conversion + generic IR→target export
batch/           — Rayon-parallel directory conversion
validators/      — Basic structure validation
ocr/             — Feature-gated OCR (placeholder)
security.rs      — Path validation (canonicalize + OPENDOC_ALLOWED_DIRS)
```

### Library Crate Dependencies
- **DOCX**: `rdocx` (create, open, edit, PDF/MD/HTML export)
- **PPTX**: `pptx` crate (create, open, edit, HTML export → converted to MD)
- **PDF**: `lopdf` (create, merge, extract text, replace text, AcroForm)
- **XLSX**: `calamine` (read) + `rust_xlsxwriter` (write for tests)
- **HTML**: `scraper` + `html5ever`
- **Markdown**: `pulldown-cmark` (parse) + `comrak` (available but unused in current code)

### Logging
Uses `tracing` + `tracing-subscriber` with env-filter. Default level is `info`. `RUST_LOG=debug` for detailed tracing.

### No State, No Config
The server is stateless. No configuration file. No database. No network calls. All state lives in the files being manipulated.
