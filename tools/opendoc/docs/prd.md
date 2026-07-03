# Product Requirements Document — opendoc-mcp

**Version:** 0.0.1
**Status:** Draft / Pre-Alpha
**Last Updated:** 2026-06-28

---

## 1. Executive Summary

`opendoc-mcp` is a high-performance, pure-Rust implementation of a Model Context Protocol (MCP) server that provides AI agents with comprehensive document CRUD (Create, Read, Update, Delete) operations. It supports DOCX, PPTX, and PDF formats with plans for XLSX, HTML, and Markdown.

The core value proposition: **a single, lightweight, zero-dependency binary that any MCP-compatible AI assistant can use to manipulate documents** — eliminating the need for cloud APIs, LibreOffice installations, or heavy language runtimes.

---

## 2. Problem Statement

### The Gap

AI agents (Claude, ChatGPT, Cursor, VS Code Copilot) are increasingly used for document-heavy workflows:
- Generating reports and proposals
- Extracting data from invoices and contracts
- Converting between document formats
- Batch-processing document collections
- Preparing documents for RAG (Retrieval-Augmented Generation)

However, existing solutions have critical limitations:

| Solution | Problem |
|----------|---------|
| **Foxit MCP Server** | Cloud-only, PDF-focused, proprietary, requires API key |
| **Nutrient/PSPDFKit MCP** | Enterprise pricing, cloud dependency, PDF-focused |
| **Carbone MCP** | Template-only, requires Carbone cloud API |
| **Vulcan File Ops** | Node.js (heavy), TypeScript, not format-native |
| **Office-PowerPoint-MCP** | Python, PPTX-only |
| **MCP Libre** | Requires LibreOffice installed (200MB+ external dep) |
| **Python-docx / python-pptx scripts** | Ad-hoc, no MCP protocol, not standardized |

### The Opportunity

**No pure-Rust MCP server for document CRUD exists.** Rust offers:
- Single binary deployment (~5MB vs 50MB+ for Node.js)
- Zero runtime dependencies (no Python, no Node, no JVM)
- Predictable performance (no GC pauses)
- Memory safety without garbage collection
- Cross-compilation to any target (including WASM for browsers)

---

## 3. Target Audience

### Primary: AI Agents / LLM Applications
- **Claude Desktop** — Generate/edit reports, extract data from documents
- **Cursor / VS Code (Cline)** — Edit documentation, process files during coding
- **Custom AI agents** — Batch document processing, RAG pipeline preparation
- **ChatGPT with MCP support** — Office document manipulation

### Secondary: Developers & Power Users
- **DevOps engineers** — Automate document generation in CI/CD
- **Content creators** — Batch convert, format, and manage documents
- **Data scientists** — Prepare document corpora for ML/NLP pipelines

---

## 4. User Personas

### Persona 1: "Alex" — AI Agent
- An LLM running in Claude Desktop
- Needs to create a DOCX report from conversation, then convert it to PDF
- Requirements: fast, reliable, no external config

### Persona 2: "Priya" — Developer
- Building a RAG pipeline that processes thousands of documents
- Needs to extract text from DOCX, PPTX, and PDF files
- Requirements: batch processing, consistent output, streaming for large files

### Persona 3: "Marcus" — Enterprise Architect
- Evaluating MCP servers for company-wide AI deployment
- Needs security, auditability, minimal attack surface
- Requirements: no network calls, sandboxed file access, single binary

---

## 5. Functional Requirements

### F-1: Document Creation
| ID | Requirement | Priority | Phase |
|----|-------------|----------|-------|
| F-1.1 | Create DOCX documents with title, paragraphs, tables | P0 | v0.0.1 |
| F-1.2 | Create PPTX presentations with slides, titles, body text | P0 | v0.0.1 |
| F-1.3 | Create PDF documents with text content | P0 | v0.0.1 |
| F-1.4 | Create XLSX spreadsheets with sheets, rows, cells | P1 | v0.1.0 |
| F-1.5 | Create HTML documents with structure | P1 | v0.1.0 |
| F-1.6 | Create Markdown documents | P1 | v0.1.0 |

### F-2: Document Reading
| ID | Requirement | Priority | Phase |
|----|-------------|----------|-------|
| F-2.1 | Open DOCX and return metadata (paragraphs, tables, author) | P0 | v0.0.1 |
| F-2.2 | Open PPTX and return metadata (slide count) | P0 | v0.0.1 |
| F-2.3 | Open PDF and return metadata (pages, encryption, version) | P0 | v0.0.1 |
| F-2.4 | Extract text from PDF (full or specific page) | P0 | v0.0.1 |
| F-2.5 | Extract text from DOCX paragraphs | P1 | v0.1.0 |
| F-2.6 | Extract images from documents | P2 | v0.2.0 |
| F-2.7 | Extract tables from documents | P2 | v0.2.0 |

### F-3: Document Editing
| ID | Requirement | Priority | Phase |
|----|-------------|----------|-------|
| F-3.1 | Add formatted paragraphs to DOCX (bold, italic, font size) | P0 | v0.0.1 |
| F-3.2 | Add tables to DOCX with headers and data | P0 | v0.0.1 |
| F-3.3 | Add slides to PPTX with title and body | P0 | v0.0.1 |
| F-3.4 | Find and replace text in DOCX (regex) | P0 | v0.0.1 |
| F-3.5 | Find and replace text in PDF | P0 | v0.0.1 |
| F-3.6 | Add images to DOCX | P1 | v0.1.0 |
| F-3.7 | Add images to PPTX slides | P1 | v0.1.0 |
| F-3.8 | Insert page breaks, headers, footers | P2 | v0.2.0 |

### F-4: Document Conversion
| ID | Requirement | Priority | Phase |
|----|-------------|----------|-------|
| F-4.1 | Convert DOCX to PDF | P0 | v0.0.1 |
| F-4.2 | Convert DOCX to Markdown | P0 | v0.0.1 |
| F-4.3 | Convert PPTX to Markdown | P0 | v0.0.1 |
| F-4.4 | Convert PPTX to PDF | P1 | v0.1.0 |
| F-4.5 | Convert DOCX to HTML | P1 | v0.1.0 |
| F-4.6 | Convert any document to Markdown (for RAG) | P1 | v0.1.0 |
| F-4.7 | Batch convert entire directories | P2 | v0.2.0 |

### F-5: Document Management
| ID | Requirement | Priority | Phase |
|----|-------------|----------|-------|
| F-5.1 | Merge multiple PDFs into one | P0 | v0.0.1 |
| F-5.2 | Split PDF by page range | P2 | v0.2.0 |
| F-5.3 | Compare two documents (diff) | P2 | v0.2.0 |
| F-5.4 | Password-protect documents | P2 | v0.2.0 |
| F-5.5 | Batch rename/move/copy documents | P2 | v0.2.0 |

### F-6: AI-Agent-Specific Features
| ID | Requirement | Priority | Phase |
|----|-------------|----------|-------|
| F-6.1 | Return all results as structured JSON | P0 | v0.0.1 |
| F-6.2 | Descriptive tool names and parameter schemas | P0 | v0.0.1 |
| F-6.3 | Template-based document generation (JSON → DOCX) | P1 | v0.1.0 |
| F-6.4 | Text chunking for RAG pipelines | P2 | v0.2.0 |
| F-6.5 | Streaming output for large documents | P2 | v0.2.0 |
| F-6.6 | Document summarization metadata | P2 | v0.2.0 |

---

## 6. Non-Functional Requirements

### NFR-1: Performance
| Metric | Target | Method |
|--------|--------|--------|
| Binary size | < 10 MB (stripped release) | `strip`, `lto` |
| Startup time | < 50 ms | `time` |
| Memory (idle) | < 10 MB RSS | `heaptrack` / `valgrind` |
| DOCX create (1 page) | < 10 ms | criterion benchmark |
| PDF merge (10 files) | < 20 ms | criterion benchmark |
| DOCX → PDF (10 pages) | < 100 ms | criterion benchmark |

### NFR-2: Reliability
- **Zero panics in normal operation** — All errors handled via `Result` types
- **File-system validation** — Check paths exist, are writable before operations
- **Graceful degradation** — Partial results returned on multi-file operations

### NFR-3: Security
- **No network access** — All operations are local filesystem only
- **No external processes** — No subprocess calls to LibreOffice or other tools
- **Path traversal protection** — Validate all file paths against allowed directories
- **No telemetry** — Zero data collection, no analytics

### NFR-4: Compatibility
- **MCP Specification** — Fully compliant with MCP 2025-06-18 spec
- **Transport** — stdio (primary), Streamable HTTP (future)
- **Platforms** — Linux (primary), macOS, Windows
- **Rust version** — MSRV 1.75+

### NFR-5: Maintainability
- **Modular architecture** — Each format handler is an independent module
- **Comprehensive tests** — Unit tests for every handler function
- **Documentation** — Every public function documented with doc comments

---

## 7. Feature Roadmap

```
v0.0.1 (Current)         v0.1.0                  v0.2.0                  v1.0.0
─────────────────    ─────────────────    ─────────────────    ─────────────────
DOCX CRUD ✔️          XLSX support ✔️       Image extraction ✔️    Streaming ✔️
PPTX CRUD ✔️          HTML read/write ✔️    Document diff ✔️      WASM target ✔️
PDF CRUD ✔️           Template engine ✔️    Password/encrypt ✔️   Digital sigs ✔️
Basic convert ✔️      anytomd-rs integ.✔️   Text chunking ✔️      PDF/A validation ✔️
JSON output ✔️        office2pdf integ.✔️   Batch operations ✔️   Enterprise auth ✔️
```

---

## 8. Success Metrics

| Metric | Current (v0.0.1) | Target (v1.0.0) |
|--------|-------------------|------------------|
| Supported formats | 3 (DOCX, PPTX, PDF) | 7+ (add XLSX, HTML, MD, ODT) |
| Tool count | 18 | 40+ |
| GitHub stars | — | 500+ |
| Downloads (cargo) | — | 10,000+ |
| Issues resolved | — | 90% closure rate |
| Avg startup time | < 10 ms | < 5 ms |
| Memory (idle) | ~4 MB | < 3 MB |

---

## 9. Constraints & Assumptions

### Constraints
1. **Pure Rust only** — No C bindings, no FFI to system libraries
2. **No cloud APIs** — All processing is local
3. **No LibreOffice** — Must work without any external office suite
4. **minimal dependencies** — Prefer pure-Rust crates over wrapped C libraries

### Assumptions
1. AI agents communicate via MCP stdio transport
2. Files are accessible on the local filesystem
3. Users have basic familiarity with MCP configuration
4. Most documents are < 100 MB (optimized for typical office docs)

---

## 10. Glossary

| Term | Definition |
|------|------------|
| **MCP** | Model Context Protocol — open standard for AI-tool communication |
| **CRUD** | Create, Read, Update, Delete |
| **DOCX** | Office Open XML Word document format |
| **PPTX** | Office Open XML PowerPoint format |
| **XLSX** | Office Open XML Excel format |
| **RAG** | Retrieval-Augmented Generation |
| **stdio** | Standard input/output transport for MCP |
| **rmcp** | Rust MCP SDK — the crate implementing MCP for Rust |
| **WASM** | WebAssembly — compile target for browser/edge execution |
