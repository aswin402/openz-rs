# MCP Refactor & Consolidation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor the opendoc-mcp server to optimize AI agent usability by consolidating 39 overlapping tools into ~20 tools, standardizing parameter interfaces, introducing structured error codes, and adding read-only MCP resources.

**Architecture:** Refactor `src/server.rs` to consolidate tool handlers, using `serde_json::Value` directly for structured objects to avoid double-stringification. Expose document read operations as MCP Resources instead of state-modifying Tools.

**Tech Stack:** Rust, `rmcp` crate, `serde_json`, `tokio`.

## Global Constraints

* Ensure all file operations route through the `validate_path!` macro from `src/security.rs`.
* All logging must write to `stderr` (e.g., via `tracing`), never to `stdout`.
* Compile and test the project with `cargo test --all-features` and `cargo check --all-features`.
* Ensure `cargo clippy --all-features -- -D warnings` passes before completing.

---

### Task 1: Consolidate Open/Read/Metadata Tools into `open_document`

**Files:**
- Modify: `src/server.rs`

**Interfaces:**
- Consumes: Existing tool handlers for `open_document`, `open_pptx`, `open_pdf`, `summarize_structure`, `document_statistics`, `extract_metadata`.
- Produces: A unified `open_document` tool.
  * Parameters: `file_path: String`, `detail_level: Option<String>` (enum: `summary`, `full`, `metadata_only`).
  * Returns: A JSON string containing the requested detail level.

- [ ] **Step 1: Write a failing integration test or check tool registrations**
  We will verify compilation fails or the consolidated schemas are parsed.
  Add a test in `src/server.rs` (under the test module if present) or inside `tests/integration.rs` testing consolidated options.
- [ ] **Step 2: Modify `src/server.rs` tool definitions**
  Remove `open_pptx`, `open_pdf`, `summarize_structure`, `document_statistics`, and `extract_metadata` from the macro router.
  Modify the `open_document` tool declaration:
  ```rust
  #[tool(
      name = "open_document",
      description = "Open any supported document (DOCX, PPTX, PDF, XLSX, HTML, MD, CSV, TXT) and return structured JSON layout or content. Set 'detail_level' to 'summary' for outline/toc only, 'metadata_only' for metadata, or 'full' for complete text and structure."
  )]
  async fn open_document(
      file_path: String,
      detail_level: Option<String>,
  ) -> Result<String, McpError> {
      // Implement merged logic checking detail_level and routing to existing handlers/IR extraction.
  }
  ```
- [ ] **Step 3: Run cargo check**
  Run: `cargo check --all-features`
  Expected: Successful compilation check.
- [ ] **Step 4: Verify test passes**
  Run: `cargo test --all-features`
  Expected: PASS.
- [ ] **Step 5: Commit changes**
  ```bash
  git add src/server.rs
  git commit -m "refactor: consolidate open, statistics, and metadata tools into open_document"
  ```

---

### Task 2: Consolidate Replace/Find Tools into `replace_text`

**Files:**
- Modify: `src/server.rs`

**Interfaces:**
- Consumes: `replace_text`, `docx_find_replace`, `pdf_replace_text`.
- Produces: A consolidated `replace_text` tool.
  * Parameters: `file_path: String`, `find: String`, `replace: String`.
  * Handles DOCX, PDF, and fallback formats in a single unified route.

- [ ] **Step 1: Write integration test case for format-specific replacement routing**
- [ ] **Step 2: Update `replace_text` in `src/server.rs`**
  Remove `docx_find_replace` and `pdf_replace_text`. Update `replace_text`:
  ```rust
  #[tool(
      name = "replace_text",
      description = "Find and replace text in any supported document. For DOCX and PDF, writes edits in-place to the file. For other formats, modifies the internal representation and returns whether changes were persisted."
  )]
  async fn replace_text(
      file_path: String,
      find: String,
      replace: String,
  ) -> Result<String, McpError> {
      // route based on file extension to docx/pdf handlers or fallback IR handlers
  }
  ```
- [ ] **Step 3: Run tests**
  Run: `cargo test --all-features`
  Expected: PASS.
- [ ] **Step 4: Commit**
  ```bash
  git add src/server.rs
  git commit -m "refactor: consolidate docx and pdf text replacement into replace_text"
  ```

---

### Task 3: Consolidate Convert Tools into `convert`

**Files:**
- Modify: `src/server.rs`

**Interfaces:**
- Consumes: `convert`, `docx_to_pdf`, `docx_to_markdown`, `pptx_to_markdown`, `export_to_xlsx`.
- Produces: Consolidated `convert` tool.
  * Parameters: `file_path: String`, `target_format: String`, `output_path: Option<String>`.

- [ ] **Step 1: Prepare test cases for unified conversion**
- [ ] **Step 2: Modify `convert` definition in `src/server.rs`**
  Remove the redundant conversion tools. Refactor `convert`:
  ```rust
  #[tool(
      name = "convert",
      description = "Convert a document from one format to another. Supported target formats: pdf, md, html, csv, txt, xlsx."
  )]
  async fn convert(
      file_path: String,
      target_format: String,
      output_path: Option<String>,
  ) -> Result<String, McpError> {
      // implement target_format matching and route to appropriate converter
  }
  ```
- [ ] **Step 3: Run check & test**
  Run: `cargo test --all-features`
  Expected: PASS.
- [ ] **Step 4: Commit**
  ```bash
  git add src/server.rs
  git commit -m "refactor: consolidate format conversion tools under unified convert tool"
  ```

---

### Task 4: Standardize Input Parameters (Accept Structured JSON)

**Files:**
- Modify: `src/server.rs`

**Interfaces:**
- Consumes: `fill_template` (`variables` string), `fill_pdf_form` (`values` string), `create_xlsx` (`sheets` string).
- Produces: Direct `serde_json::Value` (object/array maps) parameters instead of raw JSON strings.

- [ ] **Step 1: Modify signatures in `src/server.rs`**
  Update the tool parameters from `String` to `serde_json::Value`:
  ```rust
  // For fill_template:
  variables: serde_json::Value
  // For fill_pdf_form:
  values: serde_json::Value
  // For create_xlsx:
  sheets: serde_json::Value
  ```
- [ ] **Step 2: Update internal parsing logic**
  Replace double-serialization/deserialization logic (`serde_json::from_str(&variables)`) with direct usage of the `Value` structure.
- [ ] **Step 3: Verify tests and update test fixtures**
  Update any test cases in tests or server inline module to pass JSON structures instead of string-encoded JSON.
- [ ] **Step 4: Test and commit**
  Run: `cargo test --all-features`
  Expected: PASS.
  ```bash
  git add src/server.rs
  git commit -m "refactor: accept structured JSON values for template/form/sheet parameters"
  ```

---

### Task 5: Implement Structured Error Responses ("Errors as Instructions")

**Files:**
- Modify: `src/server.rs`

- [ ] **Step 1: Define structured error payload**
  Implement an internal helper function to generate standard structured JSON errors:
  ```rust
  fn structured_error(code: &str, msg: &str, category: &str, suggestion: &str) -> String {
      serde_json::json!({
          "error": msg,
          "error_code": code,
          "category": category,
          "suggestion": suggestion
      }).to_string()
  }
  ```
- [ ] **Step 2: Update error returns in `src/server.rs`**
  Modify tool handler returns to use the new structured error format.
- [ ] **Step 3: Verify and commit**
  Run: `cargo test`
  Expected: PASS.
  ```bash
  git add src/server.rs
  git commit -m "feat: implement structured error codes and instructions for AI recovery"
  ```

---

### Task 6: Expose Document Resources

**Files:**
- Modify: `src/server.rs`

- [ ] **Step 1: Define MCP Resources**
  Enable resources in the server initialization. Register routes:
  - `doc://{path}` -> read plain text of document
  - `doc://{path}/outline` -> read structured headings outline
- [ ] **Step 2: Test resource resolution**
- [ ] **Step 3: Commit**
  ```bash
  git add src/server.rs
  git commit -m "feat: add doc:// resources for read-only document inspection"
  ```
