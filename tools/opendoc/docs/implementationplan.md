# Implementation Plan — opendoc-mcp

**Version:** 0.0.10-dev
**Status:** Active
**Last Updated:** 2026-07-03

---

## 1. Overview

This document outlines the phased implementation plan for `opendoc-mcp`, tracking progress from the current v0.0.10-dev through v1.0.0.

### Current Status (v0.0.10)

| Component | Status | Notes |
|-----------|--------|-------|
| MCP Server framework | ✅ Complete | rmcp SDK, stdio transport, tool registration, doc:// resources |
| DOCX handler | ✅ Complete | Create, open, edit, convert, and custom layout/styles |
| PPTX handler | ✅ Complete | Create, open, edit, convert, image embedding, slide backgrounds/fonts |
| PDF handler | ✅ Complete | Create, open, merge, extract, replace, AcroForm fill |
| XLSX handler | ✅ Complete | read via calamine, write via rust_xlsxwriter, cell updates |
| HTML handler | ✅ Complete | read/write via scraper + html5ever |
| Markdown handler | ✅ Complete | read/write via pulldown-cmark |
| CSV handler | ✅ Complete | read/write |
| IR (Internal Representation) | ✅ Complete | Document/Paragraph/Table/Section/Image/Metadata pipeline |
| Engine | ✅ Complete | LCS diff, regex search, template filling, complexity heuristics |
| Batch processor | ✅ Complete | Rayon-parallel directory conversion |
| CLI | ✅ Complete | clap subcommands |
| OCR pipeline | ✅ Complete | Poppler pdftoppm + Tesseract CLI integration |

---

## 2. Phase 1 to 4: Completed Milestones

### Phase 1: Core Polish & Formatting (v0.0.2 - v0.0.4) ✅
- [x] Multi-page PDF creation with layout flows and page breaks
- [x] Template engine with support for nested objects and loops
- [x] DOCX image insertion via rdocx
- [x] rust-toolchain.toml for MSRV pinning (1.75.0)

### Phase 2: RAG & Chunking (v0.0.5) ✅
- [x] Text chunking with configurable strategies (heading, fixed, token)
- [x] PDF split and merge enhancements
- [x] Password decryption support for PDFs

### Phase 3: Spreadsheet Editing & Diffing (v0.0.6 - v0.0.7) ✅
- [x] Spreadsheet Write, Update, & Edit Support (XLSX)
- [x] Visual Document Diffing (HTML/Markdown highlights with word-level edits)

### Phase 4: Formatting & OCR (v0.0.8 - v0.0.10) ✅
- [x] Enhanced DOCX/PPTX styling and layout options
- [x] Scanned PDF OCR integration (Tesseract CLI + pdftoppm)

---

## 3. Phase 5: Agent-Optimized Intelligence (v0.1.0) 🔜

**Focus:** Multi-modal visual reasoning, archive pipelines, structured metadata extraction, and WebAssembly compilation.

| ID | Task | Priority | Est. Effort | Status | Description |
|----|------|----------|-------------|--------|-------------|
| 5.1 | Document screenshots for visual reasoning | High | 2 days | ✅ | Render document pages to PNG/JPEG image paths so multimodal agents can view charts/tables. |
| 5.2 | Recursive ZIP/Archive extraction | Medium | 1 day | ✅ | Process mixed archive files recursively and output clean Markdown digests. |
| 5.3 | Structured Domain Template extraction | High | 3 days | ✅ | Extract structured entities using pre-defined legal, financial, and spatio-temporal templates. |
| 5.4 | WASM Target support | Medium | 3 days | ✅ | Configure compilation for browser/edge execution (`wasm32-unknown-unknown`). |

### Deliverables
- [x] Tool to render PDF/DOCX/PPTX pages to images for agent vision
- [x] Tool to extract, parse, and summarize mixed files inside `.zip` archives
- [x] Rule-based template extraction engine (`src/engine/extract.rs`) for finance and contracts
- [x] WASM target build and compilation instructions

---

## 4. Phase 6: Enterprise Hardening (v1.0.0) 🔜

**Focus:** Production readiness, digital signatures, HTTP transport, and enterprise security.

| ID | Task | Priority | Est. Effort | Status |
|----|------|----------|-------------|--------|
| 6.1 | Cryptographic digital signatures (PDF) | High | 3 days | ❌ |
| 6.2 | Security audit and sandbox validation | High | 5 days | ❌ |
| 6.3 | Streamable HTTP transport and streaming comparison | Medium | 3 days | ❌ |
| 6.4 | PDF/A standard compliance validator | Low | 2 days | ✅ |
