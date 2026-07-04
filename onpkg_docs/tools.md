---
name: tools
description: "AI Agent Skill for OpenZ Tools — details how to implement, register, and configure tools, handle security approval, and manage large tool outputs."
metadata:
  version: 1.0.0
---

# OpenZ Tools Integration Guide 🔧🦀

This skill outlines how to implement the `Tool` trait, register tools in the registry, handle parameters/casing aliases, integrate with the `SecurityGuard`, and manage large outputs.

## 1. Implementing the `Tool` Trait

All native tools must implement the `Tool` trait from `src/tools/mod.rs`:

```rust
use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

#[async_trait]
pub trait Tool: Send + Sync {
    /// Unique name of the tool. Typically snake_case.
    fn name(&self) -> &str;

    /// Detailed description of the tool to help the LLM choose when and how to use it.
    fn description(&self) -> &str;

    /// The JSON Schema parameters defining required and optional arguments.
    fn parameters(&self) -> Value;

    /// Asynchronous execution callback.
    async fn call(&self, arguments: &Value) -> Result<Value>;
}
```

---

## 2. Registering a Tool

Register new tools in `ToolRegistry` (constructed in `src/cli/builder.rs` or `src/tools/mod.rs`):
- Use `registry.register(Arc::new(MyNewTool::new()))`.
- Custom subagents (from `~/.openz/subagents.json`) are dynamically loaded as tools (`DelegateProfileTool`) at LLM runtime via the registry's `.get()` method.

---

## 3. Important Gotchas & Guidelines

### Argument Naming and Aliases
Tool arguments do not follow a single unified convention. Some models supply `camelCase` while others supply `snake_case`.
- Handle casing differences gracefully in your `call()` implementation.
- Check `format_tool_args` in `src/agent/agent_loop.rs` to map your tool arguments to friendly display names and formatting strings for the user's progress spinner.

### Handling Large Outputs (>4,000 Characters)
If a tool returns output larger than 4,000 characters:
1. OpenZ automatically dumps the full output to `~/.openz/tool_outputs/<tool_name>_<uuid>.json`.
2. The output is compacted using the `context_compactor` (Z-Context / Headroom).
3. Only the compacted summary and file reference are sent to the LLM to prevent context pollution.

### Security and Sensitive Actions
Any tool that executes shell commands or writes files must be declared sensitive.
- Check and update `SecurityGuard::is_sensitive` inside `src/agent/security.rs` to intercept these tools.
- When intercepted, the channel prompt pauses and requests user approval before execution begins.

---

## 4. SearchXyz Integrated Tools 🔎

OpenZ natively integrates the `searchxyz` search suite for advanced keyless web search, scraping, document indexing, and Knowledge Graph management. These tools are prefix-registered under `searchxyz_`:

- `searchxyz_search_web`: Search the web (DuckDuckGo, Google, Bing, Brave, SearXng).
- `searchxyz_read_url`: Parse URLs, PDFs, YouTube transcripts, or Git repositories into clean Markdown.
- `searchxyz_search_and_read`: Combine web searching and crawling of top results in one call.
- `searchxyz_recall`: Semantic and keyword search over indexed documents.
- `searchxyz_list_sources`: List all indexed sources and cached pages.
- `searchxyz_deep_research`: Perform recursive multi-query crawls and compile a markdown report.
- `searchxyz_index_content`: Index custom text documents.
- `searchxyz_site_map`: Map website page trees and links.
- `searchxyz_index_relationship`: Insert node connections into the Knowledge Graph.
- `searchxyz_query_graph`: Query and traverse the local Knowledge Graph.
- `searchxyz_read_github_repo`: Clone and index codebases.
- `searchxyz_export_research`: Export local documents to a portable bundle.
- `searchxyz_import_research`: Load external document bundles.
- `searchxyz_delete_source`: Evict documents by URL or prefix.
- `searchxyz_clear_index`: Clear all documents and Graph data.

---

## 5. GitHub Integrated Tools 🦊

OpenZ natively integrates GitHub repository, issue, and pull request management tools under the `github_` prefix:

- `github_create_pull_request`: Create pull requests in any GitHub repository.
- `github_search_issues`: Search GitHub issues and pull requests using advanced search queries.
- `github_get_issue_comments`: Retrieve comments for specific issues/PRs.

---

## 6. Local and Crates Documentation Tools 📚

OpenZ natively integrates documentation set downloader, indexing, crates.io query, and docs.rs scraper tools under the `docs_` prefix:

- `docs_list_docsets`: List all locally installed docsets in the SQLite store.
- `docs_install_docset`: Download and cache documentation sets from DevDocs.io.
- `docs_search_docs`: Search entries (articles, methods, classes) in locally installed docsets.
- `docs_read_doc_page`: Fetch and render specific documentation pages in clean Markdown format.
- `docs_search_rust_crate`: Search crates.io for package descriptions and versions.
- `docs_read_rust_docs`: Fetch and parse HTML documentation pages from docs.rs dynamically.

---

## 7. OpenDoc Integrated Document Intelligence Tools 📄

OpenZ natively integrates document rendering, text extraction, visual and text diffs, templates, and office document generation (DOCX, XLSX, PPTX, PDF) under the `opendoc_` prefix:

- `opendoc_open_document`: Resolve and load documents from disk or URL.
- `opendoc_read_document_text`: Retrieve full plaintext from PDF/DOCX/XLSX/PPTX.
- `opendoc_search_document`: Run substring/regex searches over document text.
- `opendoc_replace_text`: Run search-and-replace queries over document text.
- `opendoc_diff_documents`: Generate unified diff blocks comparing two document structures.
- `opendoc_diff_documents_visual`: Compare visual changes between two PDF documents.
- `opendoc_chunk_for_embedding`: Extract text segments optimized for vector embeddings.
- `opendoc_fill_template`: Substitute templated fields in structured documents.
- `opendoc_validate_document`: Assert schema constraints on document metadata/content.
- `opendoc_validate_pdf_a_compliance`: Verify PDF/A archive quality standards.
- `opendoc_extract_structured_metadata`: Retrieve properties, authors, and keywords from office documents.
- `opendoc_convert`: Convert documents between file formats (e.g. DOCX -> HTML, PDF -> MD).
- `opendoc_extract_images`: Extract image attachments from document archives.
- `opendoc_split_pdf`: Split PDF archives by page indices.
- `opendoc_create_html`: Generate custom HTML pages.
- `opendoc_batch_convert`: Run batch conversions on list of documents.
- `opendoc_create_docx`: Scaffold empty DOCX document.
- `opendoc_docx_add_paragraph`: Add paragraphs to DOCX files.
- `opendoc_docx_add_table`: Add tables to DOCX files.
- `opendoc_docx_add_image`: Embed images into DOCX files.
- `opendoc_create_pptx`: Scaffold empty PPTX presentation.
- `opendoc_pptx_add_slide`: Add slide layouts to PPTX presentations.
- `opendoc_create_xlsx`: Scaffold empty XLSX sheet.
- `opendoc_edit_xlsx`: Modify cell values and formats in XLSX sheets.
- `opendoc_create_pdf`: Scaffold empty PDF document.
- `opendoc_create_formatted_pdf`: Compile PDFs with custom layouts, fonts, and images.
- `opendoc_merge_pdfs`: Combine multiple PDFs into one unified file.
- `opendoc_extract_pdf_text`: Extract plaintext from PDF documents.
- `opendoc_list_pdf_fields`: List interactive form fields in PDFs.
- `opendoc_fill_pdf_form`: Populate form fields in interactive PDFs.
- `opendoc_find_tables`: Locate and extract data tables from PDF documents.
- `opendoc_analyze_document_complexity`: Analyze document page count, word count, and element densities.
- `opendoc_ocr_document`: Run OCR engine to extract text from scanned images/PDFs.
- `opendoc_check_ocr_available`: Check if OCR engine is available locally.
- `opendoc_render_document_pages`: Render document pages to high-resolution images.
- `opendoc_extract_archive_digest`: Get metadata summary digest of a ZIP/tar archive.

---

## 8. Self-Management & Self-Healing Tools 🛠️

OpenZ natively integrates tools that allow the agent to test, debug, and optimize its own execution parameters and database guidelines:

- `diagnose_tool`: Test, profile, and validate parameters against JSON schemas for any registered native tool.
- `curate_skill`: List, add, update, or delete procedural skills and guidelines dynamically in the SQLite database store.
- `optimize_tool_scope`: Restrict or reset the set of active tool prefixes exposed to the agent loop to minimize prompt size and prevent tool hallucinations.
- `manage_config`: View active configuration (with automatic secret key redaction) or update agent hyper-parameters (such as model, provider, temperature, max_tokens, caveman_mode, tool_timeout_secs, streaming, max_tool_iterations) in real-time.
- `diagnose_system`: Retrieve comprehensive OpenZ system diagnostics including directory file sizes (sessions, traces, outputs) and SQLite database health checks.
- `manage_sessions`: List active session files, archive session histories, delete sessions, or prune temporary tool outputs to prevent disk space exhaustion.



