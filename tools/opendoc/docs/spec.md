# Technical Specification — opendoc-mcp

**Version:** 0.0.1
**Status:** Draft
**Last Updated:** 2026-06-28

**Supersedes:** N/A
**Applies to:** opendoc-mcp v0.0.1
**Target audience:** Developers, contributors, AI agent integrators

---

## 1. Introduction

`opendoc-mcp` is a Model Context Protocol (MCP) server that exposes document manipulation capabilities as callable tools for AI assistants. This specification defines the exact tool signatures, parameter schemas, return values, error handling, and behavioral contracts.

---

## 2. MCP Protocol Compliance

### 2.1 Version

Implements MCP specification **2025-06-18** ([spec](https://modelcontextprotocol.io/specification/2025-06-18)).

### 2.2 Transport

- **Primary:** stdio (standard input/output)
- **Future:** Streamable HTTP (planned for v0.3.0)

### 2.3 Server Capabilities

```json
{
  "capabilities": {
    "tools": {}
  },
  "serverInfo": {
    "name": "opendoc-mcp",
    "version": "0.0.1"
  }
}
```

### 2.4 Supported Primitives

| Primitive | Status | Notes |
|-----------|--------|-------|
| Tools | ✅ | All document operations as callable tools |
| Resources | ❌ | Future: expose documents as readable resources |
| Prompts | ❌ | Future: document template prompts |

---

## 3. Tool Specifications

### 3.1 DOCX Tools

#### 3.1.1 `create_document`

Create a new DOCX document and save to a file path.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `file_path` | `string` | ✅ | File path to save the document (e.g., `/path/to/output.docx`) |
| `title` | `string` | ❌ | Optional title for the document |

**Returns:**
```json
{
  "success": true,
  "path": "/path/to/output.docx",
  "format": "docx"
}
```

**Errors:**
- File path is invalid or unwritable
- `rdocx` internal error

---

#### 3.1.2 `open_document`

Open an existing DOCX document and return its metadata.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `file_path` | `string` | ✅ | File path to the existing document |

**Returns:**
```json
{
  "path": "/path/to/doc.docx",
  "paragraphs": 12,
  "tables": 2,
  "content_items": 14,
  "title": "Document Title",
  "author": "Opendoc MCP"
}
```

**Errors:**
- File not found
- File is not a valid DOCX

---

#### 3.1.3 `add_paragraph`

Add a paragraph with text to a DOCX document.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `file_path` | `string` | ✅ | File path to the document |
| `text` | `string` | ✅ | Text content of the paragraph |
| `bold` | `boolean` | ❌ | Optional bold formatting |
| `italic` | `boolean` | ❌ | Optional italic formatting |
| `font_size` | `number` | ❌ | Optional font size in points (e.g., 12) |

**Returns:**
```json
{
  "success": true,
  "path": "/path/to/doc.docx",
  "text_length": 42
}
```

**Notes:**
- Paragraph is appended to the end of the document
- `font_size` is specified in points (half-points internally)

---

#### 3.1.4 `add_table`

Add a table to a DOCX document.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `file_path` | `string` | ✅ | File path to the document |
| `headers` | `string[]` | ✅ | Headers for the table as a JSON array of strings |
| `data` | `string[][]` | ✅ | Table data as a JSON array of arrays of strings |

**Returns:**
```json
{
  "success": true,
  "rows": 5,
  "cols": 3
}
```

**Notes:**
- First row is always the header row
- Number of columns = max(len(headers), max(len(row) for row in data))
- Empty cells are left blank
- Table is appended to the end of the document

---

#### 3.1.5 `find_replace_text`

Find and replace text in a DOCX document using regex.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `file_path` | `string` | ✅ | File path to the document |
| `find` | `string` | ✅ | Text or regex pattern to find |
| `replace` | `string` | ✅ | Replacement text |

**Returns:**
```json
{
  "success": true,
  "replacements": 3
}
```

**Notes:**
- The `find` parameter is interpreted as a Rust regex
- For literal string matching, escape special regex characters
- Operates on paragraph text content
- Document is saved in-place

---

#### 3.1.6 `document_to_pdf`

Convert a DOCX document to PDF.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `source` | `string` | ✅ | Source DOCX file path |
| `output` | `string` | ✅ | Output PDF file path |

**Returns:**
```json
{
  "success": true,
  "source": "/path/to/input.docx",
  "output": "/path/to/output.pdf",
  "size_bytes": 12345
}
```

**Notes:**
- Uses `rdocx` built-in PDF layout engine
- Font subsetting is applied
- Output has selectable text (not scanned)

---

#### 3.1.7 `document_to_markdown`

Convert a DOCX document to Markdown.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `source` | `string` | ✅ | Source DOCX file path |
| `output` | `string` | ✅ | Output Markdown file path |

**Returns:**
```json
{
  "success": true,
  "source": "/path/to/input.docx",
  "output": "/path/to/output.md",
  "size_bytes": 567
}
```

---

### 3.2 PPTX Tools

#### 3.2.1 `create_presentation`

Create a new PowerPoint presentation and save to a file path.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `file_path` | `string` | ✅ | File path to save the presentation |
| `title` | `string` | ❌ | Optional title for the presentation |

**Returns:**
```json
{
  "success": true,
  "path": "/path/to/output.pptx",
  "format": "pptx"
}
```

---

#### 3.2.2 `open_presentation`

Open an existing presentation and return its metadata.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `file_path` | `string` | ✅ | File path to the existing presentation |

**Returns:**
```json
{
  "path": "/path/to/presentation.pptx",
  "slides": 5,
  "format": "pptx"
}
```

---

#### 3.2.3 `add_slide`

Add a slide to a presentation with a title and optional body text.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `file_path` | `string` | ✅ | File path to the presentation |
| `title` | `string` | ✅ | Slide title |
| `body` | `string[]` | ❌ | Optional body content (bullet points as JSON array) |

**Returns:**
```json
{
  "success": true,
  "slide_number": 3,
  "title": "Slide Title"
}
```

**Notes:**
- If `body` is provided, creates a title + content slide layout
- If `body` is omitted, creates a title-only slide
- Slide number is 1-based

---

#### 3.2.4 `add_slide_image`

Add an image to a slide in a presentation.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `file_path` | `string` | ✅ | File path to the presentation |
| `slide_number` | `integer` | ✅ | Slide number (1-based) |
| `image_path` | `string` | ✅ | Path to the image file |

**Returns:**
```json
{
  "success": false,
  "note": "Image embedding on slides is available in the extended API...",
  "slide": 1,
  "image": "/path/to/image.png"
}
```

**Note:** Full image embedding is a placeholder in v0.0.1 — will be implemented in v0.1.0.

---

#### 3.2.5 `presentation_to_pdf`

Convert a presentation to PDF.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `source` | `string` | ✅ | Source PPTX file path |
| `output` | `string` | ✅ | Output PDF file path |

**Returns:**
```json
{
  "success": false,
  "note": "PPTX to PDF conversion requires the office2pdf crate...",
  "source": "/path/to/input.pptx",
  "alternative": "Use the document_to_pdf tool after converting PPTX to DOCX..."
}
```

**Note:** PPTX→PDF conversion is a placeholder in v0.0.1. Will use `office2pdf` crate in v0.1.0.

---

#### 3.2.6 `presentation_to_markdown`

Export presentation to Markdown text.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `source` | `string` | ✅ | Source PPTX file path |

**Returns:**
```json
{
  "success": true,
  "slides": 5,
  "markdown": "# Title\n\n## Slide 1\n\nContent..."
}
```

---

### 3.3 PDF Tools

#### 3.3.1 `create_pdf`

Create a simple PDF with text content.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `file_path` | `string` | ✅ | File path to save the PDF |
| `text` | `string` | ✅ | Text content for the PDF |
| `author` | `string` | ❌ | Optional author metadata |

**Returns:**
```json
{
  "success": true,
  "path": "/path/to/output.pdf",
  "format": "pdf",
  "pages": 1
}
```

**Notes:**
- Creates a single-page PDF with Helvetica font
- Text is placed at position (100, 700) in PDF units
- Special characters (backslash, parens) are automatically escaped
- Page size is US Letter (612 × 792 points)

---

#### 3.3.2 `open_pdf`

Open a PDF and extract its metadata and text content.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `file_path` | `string` | ✅ | File path to the PDF |

**Returns:**
```json
{
  "path": "/path/to/doc.pdf",
  "pages": 10,
  "encrypted": false,
  "version": "1.7"
}
```

---

#### 3.3.3 `merge_pdfs`

Merge multiple PDF files into one.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `sources` | `string[]` | ✅ | List of source PDF file paths to merge |
| `output` | `string` | ✅ | Output PDF file path |

**Returns:**
```json
{
  "success": true,
  "sources": ["/path/to/a.pdf", "/path/to/b.pdf"],
  "output": "/path/to/merged.pdf"
}
```

**Notes:**
- Files are merged in the order specified
- Object IDs are renumbered to avoid conflicts
- Page tree count is updated

**Errors:**
- No source files provided
- Any source file is unreadable or invalid

---

#### 3.3.4 `extract_pdf_text`

Extract text from a PDF file.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `file_path` | `string` | ✅ | File path to the PDF |
| `page` | `integer` | ❌ | Optional page number (0-based) to extract from specific page |

**Returns:**
```json
{
  "success": true,
  "text": "Extracted text content...",
  "pages": 10
}
```

**Notes:**
- If `page` is omitted, text from all pages is extracted
- If `page` is specified, only that page is extracted (0-based index)
- Returns error if page number is out of range

---

#### 3.3.5 `pdf_replace_text`

Replace text in a PDF document.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `file_path` | `string` | ✅ | File path to the PDF |
| `find` | `string` | ✅ | Text to find |
| `replace` | `string` | ✅ | Replacement text |

**Returns:**
```json
{
  "success": true,
  "pages_modified": 3,
  "find": "old text",
  "replace": "new text"
}
```

**Notes:**
- Operates on content streams of each page
- Document is saved in-place
- `pages_modified` counts pages where at least one replacement occurred

---

#### 3.3.6 `list_pdf_fields`

List all PDF form fields (AcroForm) with types, values, and flags.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `file_path` | `string` | ✅ | File path to the PDF |

**Returns:**
```json
{
  "success": true,
  "field_count": 5,
  "fields": [
    {
      "name": "form1.name",
      "partial_name": "name",
      "field_type": "Tx",
      "field_type_name": "Text",
      "value": null,
      "default_value": null,
      "page": null,
      "is_readonly": false,
      "is_required": true,
      "options": []
    }
  ]
}
```

**Errors:**
- PDF has no AcroForm dictionary
- PDF is encrypted or corrupt

---

#### 3.3.7 `fill_pdf_form`

Fill PDF form fields with values (supports AcroForm text, checkbox, and choice fields).

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `file_path` | `string` | ✅ | File path to the PDF |
| `values` | `object` | ✅ | JSON object mapping field names to values |

**Returns:**
```json
{
  "success": true,
  "filled": 3
}
```

**Notes:**
- Field names must match the fully qualified AcroForm field path
- Text fields accept any string
- Button fields accept the export value (e.g., "Yes", "On")
- Document is saved in-place

---

### 3.4 Document Intelligence Tools

#### 3.4.1 `analyze_document_complexity`

Analyze document complexity: detect scanned PDFs, measure text density, determine OCR requirements.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `file_path` | `string` | ✅ | File path to the document |

**Returns:**
```json
{
  "needs_ocr": false,
  "reasons": [],
  "text_density": 2500.0,
  "estimated_complexity": "Moderate",
  "page_count": 5,
  "has_tables": true,
  "has_images": false,
  "has_mixed_languages": false,
  "recommended_pipeline": "text"
}
```

**Complexity Levels:**
- `Simple` — Plain text, single column (use "text" pipeline)
- `Moderate` — Tables, lists, some formatting (use "spatial" pipeline)
- `Complex` — Multi-column, dense tables, mixed content (use "spatial" pipeline)
- `Scanned` — Image-based, needs OCR (use "ocr" pipeline)

---

#### 3.4.2 `ocr_document`

Run OCR on a scanned PDF or image-based document.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `file_path` | `string` | ✅ | File path to the document |
| `language` | `string` | ❌ | Language code (default: "eng") |

**Returns:**
```json
{
  "success": true,
  "format": "pdf",
  "paragraphs": 12,
  "text": "Extracted text from OCR..."
}
```

**Notes:**
- Requires `--features ocr` at build time
- System packages: libtesseract-dev (Ubuntu) or tesseract (macOS)
- Without the feature flag, returns a setup guide

---

#### 3.4.3 `check_ocr_available`

Check if the OCR engine is available on the current system.

**Parameters:** None

**Returns:**
```json
{
  "available": false,
  "languages": ["eng"]
}
```

**Notes:**
- `available` is always `false` until the real OCR engine is implemented in v0.2.0
- `languages` lists tesseract language packs detected on the system

---

### 3.5 Utility Tools

#### 3.5.1 `list_capabilities`

List all available tools and their descriptions.

**Parameters:** None

**Returns:**
```json
{
  "server": "opendoc-mcp",
  "version": "0.0.1",
  "description": "Rust-native Document Intelligence Engine for AI Agents",
  "formats": ["docx", "pptx", "pdf", "xlsx", "html", "md", "csv", "txt"],
  "tool_categories": {
    "document_intelligence": ["open_document", "read_document_text", ...],
    "conversion": ["convert", "to_markdown", ...],
    "pdf": ["create_pdf", "open_pdf", "merge_pdfs", "extract_pdf_text", "pdf_replace_text", "list_pdf_fields", "fill_pdf_form"],
    "ai_features": ["ocr_document", "check_ocr_available"],
    "metadata": ["extract_metadata", "find_tables", "document_statistics", "analyze_document_complexity"]
  }
}
```

---

## 4. Error Handling

### 4.1 Error Response Format

All errors are returned as JSON strings with an `"error"` key:

```json
{
  "error": "Descriptive error message"
}
```

### 4.2 Error Categories

| Category | Example | HTTP Equivalent |
|----------|---------|-----------------|
| **File not found** | `"error":"No such file or directory"` | 404 |
| **Invalid format** | `"error":"Not a valid PDF file"` | 400 |
| **Permission denied** | `"error":"Permission denied"` | 403 |
| **Parse error** | `"error":"Invalid XML in DOCX"` | 422 |
| **IO error** | `"error":"io: disk full"` | 500 |
| **Validation error** | `"error":"page number out of range"` | 400 |

### 4.3 Error Handling Strategy

```rust
// Pattern used throughout handlers:
fn some_operation(path: &str) -> String {
    let doc = match open_file(path) {
        Ok(d) => d,
        Err(e) => return format!("{{\"error\":\"{e}\"}}"),
    };
    // ... perform operation ...
    match doc.save(path) {
        Ok(_) => success_json(),
        Err(e) => format!("{{\"error\":\"io: {e}\"}}"),
    }
}
```

All errors are surfaced to the AI agent as structured text, allowing the agent to:
1. Retry with corrected parameters
2. Report the error to the user
3. Attempt alternative approaches

---

## 5. Configuration

### 5.1 Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `RUST_LOG` | `info` | Logging level (trace, debug, info, warn, error) |

### 5.2 No Configuration File

`opendoc-mcp` intentionally has no configuration file. It is stateless and stateless by design:
- All state is in the files being manipulated
- All parameters are passed as tool arguments
- There are no secrets, API keys, or connection strings

This simplifies deployment and aligns with the MCP philosophy of lightweight tools.

---

## 6. File System Access Patterns

### 6.1 Path Validation

Paths provided by the AI agent are used **as-is** with no directory restrictions in v0.0.1. The user's MCP host configuration is responsible for access control.

### 6.2 File Modes

| Operation | Mode |
|-----------|------|
| Read existing file | Read-only open |
| Create new file | Write, create if not exists |
| Save edited file | Write, overwrite existing |
| Merge files | Read all sources, write output |

### 6.3 Temporary Files

- No temporary files are created during operations
- All operations are direct read/write to the specified paths

---

## 7. Thread Safety

- `OpendocServer` implements `Clone + Send + Sync`
- No mutable shared state — each tool call is independent
- `rmcp` SDK handles concurrent requests via async tasks
- Handler functions are pure: input → output with no side effects beyond file I/O

---

## 8. Logging and Observability

### 8.1 Log Levels

| Level | Usage |
|-------|-------|
| `error` | Tool call failures, file system errors |
| `warn` | Deprecated parameters, non-fatal issues |
| `info` | Server start/stop, tool invocation summary |
| `debug` | Tool parameters, handler entry/exit |
| `trace` | Detailed internal state, serialization |

### 8.2 Log Format

Default format:
```
2026-06-28T10:30:00.123Z  INFO opendoc_mcp::server: Starting Opendoc MCP Server via stdio...
2026-06-28T10:30:01.456Z DEBUG opendoc_mcp::handlers::docx: Creating document at /tmp/report.docx
```

---

## 9. Testing Specification

### 9.1 Unit Tests

Every handler function has unit tests:
- Success path with valid inputs
- Error path with invalid file paths
- Error path with corrupt files
- Edge cases (empty documents, very large text, special characters)

### 9.2 Integration Tests

- Full tool invocation via MCP protocol
- End-to-end: create → edit → read → convert workflow
- Cross-format: DOCX → PDF content fidelity check
- File system: permissions, disk full, concurrent access

### 9.3 Benchmark Tests

```rust
// Example criterion benchmark
fn bench_create_document(c: &mut Criterion) {
    c.bench_function("create_document_1_paragraph", |b| {
        b.iter(|| {
            let path = tempfile::NamedTempFile::new().unwrap();
            docx::create_document(path.path().to_str().unwrap(), Some("Test"));
        })
    });
}
```

---

## 10. Version Compatibility

### 10.1 Semantic Versioning

This project follows **Semantic Versioning 2.0.0**:

- **MAJOR** (1.x.x): Breaking changes to tool signatures or MCP protocol
- **MINOR** (x.1.x): New tools, new format support, new features
- **PATCH** (x.x.1): Bug fixes, performance improvements, non-breaking changes

### 10.2 Backward Compatibility

- Existing tools will not be removed without a major version bump
- Parameters may be added (not removed) in minor versions
- Return JSON may gain new fields in minor versions (never remove fields)

---

## 11. Rust API Reference (for library consumers)

`opendoc-mcp` can also be used as a Rust library:

```rust
use opendoc_mcp::handlers::docx;

fn main() {
    let result = docx::create_document("/tmp/test.docx", Some("Hello"));
    println!("{}", result); // {"success": true, ...}

    let md = docx::to_markdown("/tmp/test.docx", "/tmp/test.md");
    println!("{}", md);
}
```

### Public API Surface

| Module | Visibility | Description |
|--------|------------|-------------|
| `opendoc_mcp::server::OpendocServer` | Public | MCP server implementation |
| `opendoc_mcp::handlers::docx` | Public | DOCX operations |
| `opendoc_mcp::handlers::pdf` | Public | PDF operations |
| `opendoc_mcp::handlers::pptx` | Public | PPTX operations |
| `opendoc_mcp::handlers::xlsx` | Public | XLSX read → IR |
| `opendoc_mcp::handlers::pdf_forms` | Public | PDF AcroForm listing/filling |
| `opendoc_mcp::engine::complexity` | Public | Document complexity analysis |
| `opendoc_mcp::ocr` | Public | OCR pipeline (feature-gated) |
| `opendoc_mcp::ir` | Public | Internal Document Representation |
| `opendoc_mcp::types` | Public | Re-exports |
