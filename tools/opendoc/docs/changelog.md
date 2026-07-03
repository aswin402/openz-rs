# Changelog — opendoc-mcp

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [0.0.11] — 2026-07-03

### Added

#### Document Screenshots Rendering (Task 5.1)
- Implemented multi-format document page screenshots rendering (`render_document_pages` tool).
- Supports PDF rendering directly to PNGs via Poppler's `pdftoppm`.
- Supports DOCX, PPTX, XLSX, and HTML rendering to page-by-page PNGs by converting to a temporary PDF via headless LibreOffice `soffice` first.
- Added comprehensive unit tests and automated DPI/page filters.

#### Recursive ZIP Archive Extraction & Markdown Digesting (Task 5.2)
- Implemented recursive ZIP archive extraction (`extract_archive_digest` tool and CLI subcommand `digest`).
- Recursively unpacks nested zip files, creating structured directory paths.
- Scans all files and parses compatible document formats (PDF, DOCX, PPTX, XLSX, HTML, MD, CSV, TXT) into a single Markdown digest report (`digest.md`) summarizing the archive's structured contents.
- Added unit tests for recursive extraction lifecycle.

## [0.0.10] — 2026-07-02

### Added

#### Scanned PDF OCR Integration
- Implemented robust offline OCR using system-installed Tesseract CLI and Poppler's `pdftoppm` rendering engine.
- Supports PDF rendering to 150 DPI PNG images, sorting and routing individual pages to Tesseract, and stitching extracted text back into document structure.
- Supports direct standalone image file OCR (PNG, JPG, BMP, TIFF) using Tesseract.
- Added dynamic check for system-level OCR tools via `is_ocr_available()`.
- Exposed Tesseract available languages list using `tesseract --list-langs` output.
- Added comprehensive unit tests (`test_ocr_image_lifecycle`) with uncompressed 1x1 BMP data validation.

## [0.0.9] — 2026-07-02

### Added

#### Enhanced DOCX/PPTX Styling and Layout Options
- Added complete paragraph and run style controls in DOCX editor (underline, font family, color, highlighting, paragraph alignment, shading/fill color, line spacing, and page break properties).
- Added table styling in DOCX editor (custom table width percentage, alignment, border style/size/color, header shading, cell shading, and row break prevention).
- Added slide styling in PPTX editor (slide background solid fill, custom run size, font color, font family, and text alignment).
- Exposed all style parameters in the MCP server tools `docx_add_paragraph`, `docx_add_table`, and `pptx_add_slide`.
- Added comprehensive unit tests for DOCX styling (`test_docx_enhanced_styling`) and PPTX styling (`test_pptx_styling_lifecycle`).

## [0.0.8] — 2026-07-02

### Added

#### Visual Document Diffing (HTML/Markdown highlights)
- Implemented `render_diff_visual` in `src/engine/diff.rs` to compute paragraph-level comparisons and return highlighted reports.
- Supports both styled HTML format and standard Markdown format.
- Implemented character/word-level diffing inside modified paragraphs (adjacent additions and removals) to highlight the specific words altered.
- Registered the new `diff_documents_visual` tool to the MCP server under the `"document_intelligence"` category.
- Added comprehensive visual diff unit tests for both Markdown word-level diffs and HTML container markup.

## [0.0.7] — 2026-07-02

### Added

#### Spreadsheet Write, Update, & Edit Support (XLSX)
- Implemented `edit_xlsx` function in `src/handlers/xlsx.rs` to support editing existing Excel workbooks.
- Supports adding new worksheets.
- Supports applying cell updates across multiple worksheets including writing formulas, floating numbers, booleans, and strings.
- Automatically handles out-of-bounds cell coordinates by dynamically resizing the spreadsheet grid.
- Exposed the new `edit_xlsx` tool to the MCP server with capabilities registration.
- Added comprehensive unit test `test_edit_xlsx` validating sheet additions, cell updates, grid resizing, and formula parsing.

## [0.0.6] — 2026-07-02

### Added

#### Recursive & Extended Batch Operations
- Implemented recursive directory walking for the batch converter.
- Recreates directory subdirectory structures relative to the input folder in the output folder.
- Integrated optional password decryption into batch conversion.
- Added concurrency thread limit control for batch processing.
- Exposed these new parameters to both the `batch_convert` MCP tool and the `batch` CLI command.

#### Performance Profiling & Benchmarking
- Added Criterion benchmarks for PDF text extraction (`bench_pdf_text_extraction`), template engine filling (`bench_template_rendering`), PDF layout compilation (`bench_pdf_layout_creation`), and DOCX ZIP image extraction (`bench_docx_image_extraction`).
- Verified all benchmarks compile and execute successfully.

## [0.0.5] — 2026-07-02

### Added

#### Password & Encryption Support (PDF & Office Documents)
- Added password decryption and loading support for PDFs using `lopdf`'s native decryption capabilities.
- Implemented `load_to_ir_with_password` and `replace_text_with_password` in PDF handler.
- Configured MCP tools (`open_document`, `read_document_text`, `search_document`, `replace_text`, `fill_template`, `validate_document`, `convert`) to support optional `password` parameter.
- Handled offline office encryption restrictions with clear, user-friendly descriptive errors.

#### Zip-based Image Extraction (DOCX / PPTX)
- Created `extract_images_from_zip` function in `src/handlers/mod.rs` using the `zip` crate.
- Added `extract_images` tool to MCP server, extracting embedded pictures from DOCX/PPTX media folders to target output directory.

#### PDF Split by Page Range
- Implemented `split_pdf` and `split_pdf_with_password` in `src/handlers/pdf.rs`.
- Added `split_pdf` tool to MCP server to extract a subset of pages into a separate document (1-based, inclusive).

### Fixed

#### PDF Content Stream Serialization
- Fixed page content stream missing `/Length` property in stream dictionaries. Page stream text is now fully compliant and parseable by standard PDF text extraction.
- Wrapped plain-text content streams inside `BT` (Begin Text) and `ET` (End Text) operators for standard text extraction compatibility.
- Added unit tests for split PDF, PDF encryption detection, image extraction, and password-protected PDF loading. All 100 tests pass successfully.

## [0.0.4] — 2026-07-02

### Added

#### Strategy-Driven Text Chunking (RAG Support)
- Created `src/engine/chunk.rs` implementing multiple configurable strategies for text chunking:
  - `fixed` (sliding window of paragraphs with configurable token overlap).
  - `heading` (headings/section boundary-based chunking with recursive splits for oversized segments).
  - `recursive` (langchain-style recursive splitting by paragraph, newline, sentence, and word boundaries).
  - `page` (slide or sheet boundary-based chunking).
- Exposed the `strategy` and `overlap` configuration parameters to the `chunk_for_embedding` MCP server tool.
- Added extensive unit tests covering all chunking strategies.

## [0.0.3] — 2026-07-02

### Added

#### DOCX Image Insertion
- Created `docx_add_image` tool/function in `src/handlers/docx.rs` and registered it to the MCP server.
- Supports binary image embedding into paragraphs of existing DOCX files, with custom width and height parameters in inches.
- Updated `to_ir` to parse and extract image dimensions and metadata from DOCX files.
- Added comprehensive unit tests validating image insertion and IR metadata extraction.

#### PDF Rendering with Layout Control
- Created `create_formatted_pdf` tool/function in `src/handlers/pdf.rs` allowing layout customisations.
- Configurable settings via `PdfLayoutConfig`: custom document title (rendered centered on page 1), author metadata, page numbering toggle, margins (top/bottom/left/right), font size.
- Automatic word-wrap and multi-page text flow.
- Support for explicit page breaks using Form Feed (`\x0c` / `\f`).
- Added 5 new PDF layout unit tests (`test_create_pdf_empty_text`, `test_formatted_pdf_with_title`, `test_formatted_pdf_page_numbers`, `test_formatted_pdf_explicit_page_break`, `test_formatted_pdf_word_wrap`).

### Fixed

#### Template Engine
- Fixed off-by-two searching index bug in `find_matching_close_in` that bypassed brace openers for closing tags.
- Fixed Rust `format!` brace escaping bug for section tags (`format!("{{/{}}}")` was evaluated as `"{/name}"` instead of `"{{/name}}"`).
- Fixed evaluation counts in `expand_conditional`, `expand_each`, and `expand_section` loops where successful blocks with zero variable replacements inside were not counted as a replacement.
- Fixed `resolve_path` for looping over lists of scalars, where the wrapper object itself was returned instead of lookup key `"this"` or `"."`.
- Removed redundant `let context` declaration in `expand_each` and unused mutable `count` compiler warnings.

#### Tests & Verification
- Resolved 14 failing unit tests in the template engine suite. All 91 tests (71 unit + 20 integration) are now passing successfully.

## [0.0.2] — 2026-06-30

### Added

#### IR Architecture (Internal Representation)
- Universal `Document` model with `Paragraph`, `Table`, `Section`, `Image`, `Metadata`
- All handlers implement `to_ir()` for unified format-agnostic processing
- Engine module: `search` (regex/text), `replace`, `template`, `diff` (LCS), `complexity` (heuristics)
- Batch processor with rayon for parallel directory conversion
- Validators module for document structure checks
- `doc://` MCP resources for read-only document access

#### New Format Support
- **XLSX** — Read via `calamine`, write via `rust_xlsxwriter`, convert to/from IR
- **HTML** — Read/write via `scraper` + `html5ever`
- **Markdown** — Read/write via `pulldown-cmark`, export from IR
- **CSV** — Read/write via native Rust (included in IR pipeline)
- **TXT** — Read/write (plain text fallback)
- **PDF Forms** — List fields and fill AcroForm values

#### Expanded Conversion Pipeline
- Cross-format conversion via `converters::convert()` (DOCX→PDF, DOCX→MD, DOCX→HTML, PPTX→MD, PPTX→PDF, PDF→TXT, PDF→MD, XLSX→CSV)
- Generic IR→target export (JSON, TXT, MD, HTML, CSV, XLSX, DOCX)
- Real PPTX→PDF conversion (text extracted via IR, rendered via lopdf)
- Real PPTX image embedding (OPC/ZIP package manipulation with PNG, JPEG, GIF, BMP, TIFF, SVG)

#### Tool Consolidation
- Consolidated 39 tools into ~20 unified MCP tools
- `open_document` — Single tool for all formats (detail_level: full/summary/metadata_only)
- `replace_text` — Unified find/replace across DOCX, PDF, and IR
- `convert` — Unified conversion with `target_format` parameter
- Structured JSON params (`serde_json::Value`) for `fill_template`, `fill_pdf_form`, `create_xlsx`
- Structured error responses with `error_code`, `category`, and `suggestion`
- `list_capabilities` updated with all formats and tools

#### Infrastructure
- CLI subcommands via clap (`convert`, `extract`, `batch`, `merge`, `validate`, `info`, `diff`, `formats`, `serve`)
- Security module with `validate_path!()` macro and `OPENDOC_ALLOWED_DIRS` sandbox
- GitHub Actions CI (Linux: build + test + clippy)
- `rust-toolchain.toml` for MSRV pinning (1.75.0)
- AGENTS.md with full architecture documentation
- Criterion benchmark suite (3 benchmarks: DOCX→IR, TXT→IR, search)

### Documentation
- Doc comments on all public functions across all modules
- Updated README.md with current tool list and format support
- Updated architecture.md with IR-centric design
- Updated spec.md reflecting consolidated tools

### Fixed
- PPTX `to_pdf()` placeholder replaced with real converter delegation
- PPTX `add_slide_image()` placeholder replaced with real OPC image embedding

## [0.0.1] — 2026-06-28

### Added

#### DOCX Support
- `create_document` — Create new Word documents with optional title
- `open_document` — Read document metadata (paragraphs, tables, content count, author)
- `add_paragraph` — Append formatted text with bold, italic, and font size options
- `add_table` — Insert tables with headers and data rows
- `find_replace_text` — Regex-powered find and replace in document content
- `document_to_pdf` — Convert DOCX to PDF with selectable text (via `rdocx` layout engine)
- `document_to_markdown` — Convert DOCX to Markdown format

#### PPTX Support
- `create_presentation` — Create new PowerPoint files
- `open_presentation` — Read presentation metadata (slide count)
- `add_slide` — Add slides with title and optional body bullet points
- `add_slide_image` — Reference images on slides (basic, full embedding pending)
- `presentation_to_markdown` — Export slide content as Markdown
- `presentation_to_pdf` — Placeholder (will use `office2pdf` in v0.1.0)

#### PDF Support
- `create_pdf` — Generate single-page PDFs with Helvetica font
- `open_pdf` — Read PDF metadata (page count, encryption status, version)
- `merge_pdfs` — Combine multiple PDFs into one with automatic object renumbering
- `extract_pdf_text` — Extract text from full document or specific page
- `pdf_replace_text` — Find and replace text in PDF content streams

#### Utility
- `list_capabilities` — List all available tools, formats, and server version
- Server information with instructions for AI agents

#### Infrastructure
- MCP protocol compliance via `rmcp` SDK (spec 2025-06-18)
- stdio transport for zero-configuration setup
- Structured logging with `tracing` + `tracing-subscriber` (level controlled via `RUST_LOG`)
- JSON-RPC 2.0 compliant tool invocation
- All responses returned as structured JSON for easy AI agent consumption
- Modular handler architecture — each format is an independent module

### Technical Details

- **Runtime:** Tokio async (single-threaded for stdio transport)
- **DOCX engine:** `rdocx` 0.1 (pure Rust, no C dependencies)
- **PPTX engine:** `pptx` 0.1 (pure Rust OPC XML manipulation)
- **PDF engine:** `lopdf` 0.31 (pure Rust PDF read/write/merge)
- **Binary size:** ~4.2 MB (stripped release build)
- **Startup time:** ~3 ms to first tool invocation
- **Memory (idle):** ~3.5 MB RSS

### Known Limitations

- PPTX image embedding returns a guidance message (not yet implemented)
- PPTX→PDF conversion returns a guidance message (requires `office2pdf` crate)
- PDF creation is single-page only with fixed positioning
- No XLSX support yet (planned for v0.1.0)
- No HTML support yet (planned for v0.1.0)
- No native Markdown read/write yet (planned for v0.1.0)
- No unit tests yet (planned for v0.0.2)
- No CI pipeline yet (planned for v0.0.2)

---

## [Unreleased]

### Planned for v0.0.2

- [ ] Unit tests for all handler functions
- [ ] Integration tests for server tool dispatch
- [ ] PPTX image embedding (full binary image insertion)
- [ ] PPTX→PDF conversion via `office2pdf`
- [ ] GitHub Actions CI (Linux, macOS, Windows)
- [ ] Doc comments on all public functions
- [ ] Benchmark suite with criterion
- [ ] rust-toolchain.toml for MSRV pinning

### Planned for v0.1.0

- [ ] XLSX support (create, read, edit via `rust_xlsxwriter` + `calamine`)
- [ ] HTML read/write support
- [ ] Markdown read/write support (as native format)
- [ ] Template-based document generation (JSON + template → document)
- [ ] `anytomd-rs` integration for unified document→Markdown
- [ ] `office2pdf` integration for real office→PDF conversion

### Planned for v0.2.0

- [ ] Text chunking for RAG (by heading, token count, byte size)
- [ ] Batch directory processing (convert, extract, transform)
- [ ] Document diff / comparison
- [ ] Image extraction from DOCX/PPTX
- [ ] PDF split by page range
- [ ] Password/encryption support
- [ ] Streaming output for large documents

### Planned for v1.0.0

- [ ] Security audit
- [ ] Path traversal protection / sandboxed directories
- [ ] WASM compilation target
- [ ] Streamable HTTP transport
- [ ] Digital signatures (PDF)
- [ ] PDF/A validation
- [ ] Official documentation site

---

## Archive

*No prior versions. This is the initial release.*
