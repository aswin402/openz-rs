# opendoc-mcp Progress To-Do List

Tasks tracked in [Implementation Plan](docs/implementationplan.md).

## ✅ v0.0.2 — Complete

- [x] Consolidate Open/Read/Metadata Tools
- [x] Consolidate Replace/Find Tools
- [x] Consolidate Conversion Tools
- [x] Standardize Input Parameters (Structured JSON)
- [x] Structured Error Responses
- [x] Expose Document Resources
- [x] Clean up & Documentation Update
- [x] rust-toolchain.toml for MSRV pinning (1.75.0)
- [x] Real PPTX image embedding (OPC/ZIP manipulation)
- [x] Real PPTX→PDF conversion (delegate to converter)
- [x] Criterion benchmark suite
- [x] Doc comments on all public functions
- [x] Changelog updated for v0.0.2

## ✅ v0.0.3 — Complete

- [x] Multi-page PDF creation with layout (auto word-wrap, page breaks, margins, page numbers, title page)
- [x] Enhanced template engine (nested objects, loops, conditionals)
- [x] DOCX image insertion (binary embedding via rdocx)
- [x] Expanded test coverage (14 new unit tests for template engine, 5 new layout PDF tests, 1 new DOCX image test, all passing)
- [x] Version incremented and codebase validated offline
- [x] Changelog updated for v0.0.3

## ✅ v0.0.4 — Complete

- [x] Text chunking strategies (heading, token count, byte size) for RAG input
- [x] Image extraction out of DOCX and PPTX files
- [x] PDF split by page range
- [x] Password and encryption support for PDF and Office files

## ✅ v0.0.5 — Complete

- [x] Batch operations conversion CLI/MCP tool extensions
- [x] Direct conversion performance profiling and benchmarking

## ✅ v0.0.6 — Complete

- [x] Spreadsheet Write, Update, & Edit Support (XLSX)

## ✅ v0.0.7 — Complete

- [x] Visual Document Diffing (HTML/Markdown highlights)

## ✅ v0.0.8 — Complete

- [x] Enhanced DOCX/PPTX styling and layout options

## ✅ v0.0.9 — Complete

- [x] Scanned PDF OCR integration

## 🔜 v0.1.0 — Planned

- [ ] WASM target support
- [ ] Cryptographic digital signatures & PDF signing
- [ ] Streaming document comparison and layout rendering
