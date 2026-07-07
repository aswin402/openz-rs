use crate::engine::{diff, replace, search, template};
use crate::handlers::{self, docx, pdf, pdf_forms, pptx, xlsx};
use rmcp::{
    model::{ServerCapabilities, ServerInfo},
    serve_server, tool,
    transport::stdio,
    ServerHandler,
};

macro_rules! validate_path {
    ($path:expr) => {
        match crate::security::validate_path(&$path) {
            Ok(p) => p.to_string_lossy().into_owned(),
            Err(e) => return serde_json::json!({"error": format!("Security error: {e}")}).to_string(),
        }
    };
}

#[derive(Debug, Clone, Default)]
pub struct OpendocServer;

// ──────────────────────────────────────────────
//  DOCUMENT INTELLIGENCE TOOLS
// ──────────────────────────────────────────────

#[tool(tool_box)]
impl OpendocServer {
    // ═══════════════════════════════════════════
    //  FORMAT-AGNOSTIC IR TOOLS
    // ═══════════════════════════════════════════

    #[tool(
        name = "open_document",
        description = "Open any supported document (DOCX, PPTX, PDF, XLSX, HTML, MD, CSV, TXT) and return structured JSON layout or content. Set 'detail_level' to 'summary' for outline and metadata only, 'metadata_only' for metadata block only, or 'full' for complete paragraphs, tables, images, metadata, and outline."
    )]
    pub fn open_document(
        &self,
        #[tool(param)]
        #[schemars(description = "File path to the document")]
        file_path: String,
        #[tool(param)]
        #[schemars(
            description = "Level of detail to return: 'summary' (metadata + outline + counts), 'metadata_only' (metadata only), or 'full' (all content including paragraphs and tables)"
        )]
        detail_level: Option<String>,
        #[tool(param)]
        #[schemars(description = "Optional password for encrypted documents")]
        password: Option<String>,
    ) -> String {
        let file_path = validate_path!(file_path);
        let detail = detail_level.unwrap_or_else(|| "full".to_string());
        match handlers::load_to_ir_with_password(&file_path, password.as_deref()) {
            Ok(ir) => match detail.as_str() {
                "metadata_only" => serde_json::to_string_pretty(&serde_json::json!({
                    "success": true,
                    "path": file_path,
                    "format": ir.format,
                    "metadata": ir.metadata,
                }))
                .unwrap_or_default(),
                "summary" => serde_json::to_string_pretty(&serde_json::json!({
                    "success": true,
                    "path": file_path,
                    "format": ir.format,
                    "paragraphs": ir.paragraphs.len(),
                    "tables": ir.tables.len(),
                    "images": ir.images.len(),
                    "sections": ir.sections.len(),
                    "estimated_tokens": ir.estimate_tokens(),
                    "metadata": ir.metadata,
                    "outline": ir.outline(),
                }))
                .unwrap_or_default(),
                _ => serde_json::to_string_pretty(&serde_json::json!({
                    "success": true,
                    "path": file_path,
                    "format": ir.format,
                    "paragraphs": ir.paragraphs,
                    "tables": ir.tables,
                    "images": ir.images,
                    "sections": ir.sections,
                    "estimated_tokens": ir.estimate_tokens(),
                    "metadata": ir.metadata,
                    "outline": ir.outline(),
                }))
                .unwrap_or_default(),
            },
            Err(e) => map_load_error(&e),
        }
    }

    #[tool(description = "Read the full text content of any document (plain text extraction)")]
    pub fn read_document_text(
        &self,
        #[tool(param)]
        #[schemars(description = "File path to the document")]
        file_path: String,
        #[tool(param)]
        #[schemars(description = "Optional password for encrypted documents")]
        password: Option<String>,
    ) -> String {
        let file_path = validate_path!(file_path);
        match handlers::load_to_ir_with_password(&file_path, password.as_deref()) {
            Ok(ir) => {
                let text: Vec<String> = ir.paragraphs.iter().map(|p| p.text.clone()).collect();
                let content = text.join("\n");
                serde_json::json!({
                    "success": true,
                    "text": content,
                    "char_count": content.len(),
                    "est_tokens": ir.estimate_tokens(),
                })
                .to_string()
            }
            Err(e) => map_load_error(&e),
        }
    }

    #[tool(description = "Search for text in a document using keyword or regex")]
    pub fn search_document(
        &self,
        #[tool(param)]
        #[schemars(description = "File path to the document")]
        file_path: String,
        #[tool(param)]
        #[schemars(description = "Search query (plain text or regex pattern)")]
        query: String,
        #[tool(param)]
        #[schemars(description = "If true, treat query as a regex pattern")]
        use_regex: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Optional password for encrypted documents")]
        password: Option<String>,
    ) -> String {
        let file_path = validate_path!(file_path);
        match handlers::load_to_ir_with_password(&file_path, password.as_deref()) {
            Ok(ir) => {
                let results = search::search_document(&ir, &query, use_regex.unwrap_or(false));
                serde_json::json!({
                    "success": true,
                    "query": query,
                    "matches": results.len(),
                    "results": results,
                })
                .to_string()
            }
            Err(e) => map_load_error(&e),
        }
    }

    #[tool(description = "Find and replace text in any document (supports regex)")]
    pub fn replace_text(
        &self,
        #[tool(param)]
        #[schemars(description = "File path to the document")]
        file_path: String,
        #[tool(param)]
        #[schemars(description = "Text or pattern to find")]
        find: String,
        #[tool(param)]
        #[schemars(description = "Replacement text")]
        replace: String,
        #[tool(param)]
        #[schemars(description = "Optional password for encrypted documents")]
        password: Option<String>,
    ) -> String {
        let file_path = validate_path!(file_path);
        let path = std::path::Path::new(&file_path);
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        match ext.as_str() {
            "docx" => {
                if password.is_some() {
                    return serde_json::json!({"error": "Password decryption for Office documents (.docx) is not supported under offline mode due to missing office_crypto library. Encrypted PDFs are fully supported."}).to_string();
                }
                handlers::docx::find_replace_text(&file_path, &find, &replace)
            }
            "pdf" => handlers::pdf::replace_text_with_password(
                &file_path,
                &find,
                &replace,
                password.as_deref(),
            ),
            "txt" | "text" => match std::fs::read_to_string(&file_path) {
                Ok(content) => {
                    let re = match regex::RegexBuilder::new(&find)
                        .size_limit(1_000_000)
                        .build()
                    {
                        Ok(r) => r,
                        Err(e) => {
                            return serde_json::json!({"error": format!("invalid regex: {e}")})
                                .to_string()
                        }
                    };
                    let new_content = re.replace_all(&content, &replace).to_string();
                    let count = re.find_iter(&content).count();
                    match std::fs::write(&file_path, new_content) {
                        Ok(_) => serde_json::json!({
                            "success": true,
                            "replacements": count,
                            "persisted": true,
                        })
                        .to_string(),
                        Err(e) => {
                            serde_json::json!({"error": format!("Failed to write file: {e}")})
                                .to_string()
                        }
                    }
                }
                Err(e) => {
                    serde_json::json!({"error": format!("Failed to read file: {e}")}).to_string()
                }
            },
            _ => {
                match handlers::load_to_ir_with_password(&file_path, password.as_deref()) {
                    Ok(mut ir) => {
                        let count = replace::replace_text(&mut ir, &find, &replace);
                        // Try to persist via generic export for writable formats
                        let writable = [
                            "md", "markdown", "html", "csv", "txt", "text", "json", "docx",
                        ];
                        if writable.contains(&ext.as_str()) && count > 0 {
                            match crate::converters::export(&ir, &ext, &file_path) {
                                Ok(result) => serde_json::json!({
                                    "success": true,
                                    "replacements": count,
                                    "persisted": true,
                                    "output_size": result.size_bytes,
                                })
                                .to_string(),
                                Err(e) => serde_json::json!({
                                    "success": true,
                                    "replacements": count,
                                    "persisted": false,
                                    "note": format!("IR modified but failed to save: {e}"),
                                })
                                .to_string(),
                            }
                        } else {
                            serde_json::json!({
                                "success": true,
                                "replacements": count,
                                "persisted": false,
                                "note": format!("Replace operated on extracted text in-memory. Saving back to format '{ext}' is not supported yet.")
                            }).to_string()
                        }
                    }
                    Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
                }
            }
        }
    }

    #[tool(description = "Compare two documents and return a structured diff")]
    pub fn diff_documents(
        &self,
        #[tool(param)]
        #[schemars(description = "First document path")]
        file_a: String,
        #[tool(param)]
        #[schemars(description = "Second document path")]
        file_b: String,
    ) -> String {
        let file_a = validate_path!(file_a);
        let file_b = validate_path!(file_b);
        let (doc_a, doc_b) = match (handlers::load_to_ir(&file_a), handlers::load_to_ir(&file_b)) {
            (Ok(a), Ok(b)) => (a, b),
            (Err(e), _) => {
                return serde_json::json!({"error": format!("Failed to load file_a: {e}")})
                    .to_string()
            }
            (_, Err(e)) => {
                return serde_json::json!({"error": format!("Failed to load file_b: {e}")})
                    .to_string()
            }
        };
        let result = diff::diff_documents(&doc_a, &doc_b);
        serde_json::to_string_pretty(&result).unwrap_or_default()
    }

    #[tool(
        description = "Compare two documents and return a visual difference report as HTML or Markdown"
    )]
    pub fn diff_documents_visual(
        &self,
        #[tool(param)]
        #[schemars(description = "First document path")]
        file_a: String,
        #[tool(param)]
        #[schemars(description = "Second document path")]
        file_b: String,
        #[tool(param)]
        #[schemars(description = "Output format: html or markdown (default: html)")]
        format: Option<String>,
    ) -> String {
        let file_a = validate_path!(file_a);
        let file_b = validate_path!(file_b);
        let (doc_a, doc_b) = match (handlers::load_to_ir(&file_a), handlers::load_to_ir(&file_b)) {
            (Ok(a), Ok(b)) => (a, b),
            (Err(e), _) => {
                return serde_json::json!({"error": format!("Failed to load file_a: {e}")})
                    .to_string()
            }
            (_, Err(e)) => {
                return serde_json::json!({"error": format!("Failed to load file_b: {e}")})
                    .to_string()
            }
        };
        let is_html = format.map(|f| f.to_lowercase() == "html").unwrap_or(true);
        diff::render_diff_visual(&doc_a, &doc_b, is_html)
    }

    #[tool(description = "Chunk document for RAG embedding pipelines with configurable strategies")]
    pub fn chunk_for_embedding(
        &self,
        #[tool(param)]
        #[schemars(description = "File path to the document")]
        file_path: String,
        #[tool(param)]
        #[schemars(description = "Strategy: fixed, heading, recursive, page (default: fixed)")]
        strategy: Option<String>,
        #[tool(param)]
        #[schemars(description = "Maximum tokens per chunk (default 512)")]
        max_tokens: Option<usize>,
        #[tool(param)]
        #[schemars(description = "Token overlap between consecutive chunks (default: 50)")]
        overlap: Option<usize>,
    ) -> String {
        let file_path = validate_path!(file_path);
        match handlers::load_to_ir(&file_path) {
            Ok(ir) => {
                let chunking_strategy = match strategy {
                    Some(s) => match s.parse::<crate::engine::chunk::ChunkingStrategy>() {
                        Ok(st) => st,
                        Err(e) => return serde_json::json!({"error": e}).to_string(),
                    },
                    None => crate::engine::chunk::ChunkingStrategy::Fixed,
                };
                let max_tok = max_tokens.unwrap_or(512);
                let over = overlap.unwrap_or(50);

                let chunks = ir.chunk_with_strategy(chunking_strategy, max_tok, over);
                serde_json::json!({
                    "success": true,
                    "strategy": format!("{:?}", chunking_strategy).to_lowercase(),
                    "chunk_count": chunks.len(),
                    "chunks": chunks,
                })
                .to_string()
            }
            Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
        }
    }

    #[tool(description = "Fill {{placeholders}} in a document with values")]
    pub fn fill_template(
        &self,
        #[tool(param)]
        #[schemars(description = "File path to the template document")]
        file_path: String,
        #[tool(param)]
        #[schemars(description = "JSON object of key-value pairs to fill")]
        variables: serde_json::Value,
        #[tool(param)]
        #[schemars(description = "Optional password for encrypted documents")]
        password: Option<String>,
    ) -> String {
        let file_path = validate_path!(file_path);
        let vars: Vec<(String, String)> = if let serde_json::Value::Object(map) = variables {
            map.into_iter()
                .map(|(k, v)| {
                    let val_str = match v {
                        serde_json::Value::String(s) => s,
                        serde_json::Value::Null => "".to_string(),
                        other => other.to_string(),
                    };
                    (k, val_str)
                })
                .collect()
        } else {
            return serde_json::json!({"error": "variables must be a JSON object"}).to_string();
        };

        let path = std::path::Path::new(&file_path);
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        match ext.as_str() {
            "docx" => {
                let mut doc = match rdocx::Document::open(&file_path) {
                    Ok(d) => d,
                    Err(e) => return serde_json::json!({"error": e.to_string()}).to_string(),
                };
                let map: std::collections::HashMap<&str, &str> =
                    vars.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
                let count = doc.replace_all(&map);
                match doc.save(&file_path) {
                    Ok(_) => serde_json::json!({
                        "success": true,
                        "replacements": count,
                        "persisted": true,
                    })
                    .to_string(),
                    Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
                }
            }
            "txt" | "text" => match std::fs::read_to_string(&file_path) {
                Ok(content) => {
                    let mut new_content = content;
                    let mut count = 0;
                    for (key, value) in &vars {
                        let placeholder = format!("{{{{{}}}}}", key);
                        if new_content.contains(&placeholder) {
                            let occurrences = new_content.matches(&placeholder).count();
                            new_content = new_content.replace(&placeholder, value);
                            count += occurrences;
                        }
                    }
                    match std::fs::write(&file_path, new_content) {
                        Ok(_) => serde_json::json!({
                            "success": true,
                            "replacements": count,
                            "persisted": true,
                        })
                        .to_string(),
                        Err(e) => {
                            serde_json::json!({"error": format!("Failed to write file: {e}")})
                                .to_string()
                        }
                    }
                }
                Err(e) => {
                    serde_json::json!({"error": format!("Failed to read file: {e}")}).to_string()
                }
            },
            _ => {
                match handlers::load_to_ir_with_password(&file_path, password.as_deref()) {
                    Ok(mut ir) => {
                        let count = template::fill_template(&mut ir, &vars);
                        // Try to persist via generic export for writable formats
                        let writable = [
                            "md", "markdown", "html", "csv", "txt", "text", "json", "docx",
                        ];
                        if writable.contains(&ext.as_str()) && count > 0 {
                            match crate::converters::export(&ir, &ext, &file_path) {
                                Ok(result) => serde_json::json!({
                                    "success": true,
                                    "replacements": count,
                                    "persisted": true,
                                    "output_size": result.size_bytes,
                                })
                                .to_string(),
                                Err(e) => serde_json::json!({
                                    "success": true,
                                    "replacements": count,
                                    "persisted": false,
                                    "note": format!("IR modified but failed to save: {e}"),
                                })
                                .to_string(),
                            }
                        } else {
                            serde_json::json!({
                                "success": true,
                                "replacements": count,
                                "persisted": false,
                                "note": format!("Template filled in-memory. Saving back to format '{ext}' is not supported yet.")
                            }).to_string()
                        }
                    }
                    Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
                }
            }
        }
    }

    #[tool(description = "Validate document structure and integrity")]
    pub fn validate_document(
        &self,
        #[tool(param)]
        #[schemars(description = "File path to the document")]
        file_path: String,
        #[tool(param)]
        #[schemars(description = "Optional password for encrypted documents")]
        password: Option<String>,
    ) -> String {
        let file_path = validate_path!(file_path);
        match handlers::load_to_ir_with_password(&file_path, password.as_deref()) {
            Ok(ir) => {
                let result = crate::validators::validate_document(&ir);
                serde_json::to_string_pretty(&result).unwrap_or_default()
            }
            Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
        }
    }

    #[tool(
        description = "Validate PDF/A standard compliance (encryption, embedded fonts, forbidden actions, XMP metadata)"
    )]
    pub fn validate_pdf_a_compliance(
        &self,
        #[tool(param)]
        #[schemars(description = "File path to the PDF document")]
        file_path: String,
    ) -> String {
        let file_path = validate_path!(file_path);

        let path = std::path::Path::new(&file_path);
        if path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_lowercase())
            != Some("pdf".to_string())
        {
            return serde_json::json!({"error": "Only PDF files are supported for PDF/A validation"}).to_string();
        }

        match crate::validators::pdf_a::validate_pdf_a(path) {
            Ok(result) => serde_json::to_string_pretty(&result).unwrap_or_default(),
            Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
        }
    }

    // ═══════════════════════════════════════════
    //  CONVERSION TOOLS
    // ═══════════════════════════════════════════

    #[tool(description = "Convert a document from one format to another")]
    pub fn convert(
        &self,
        #[tool(param)]
        #[schemars(description = "Source file path")]
        source: String,
        #[tool(param)]
        #[schemars(description = "Target format (pdf, md, html, csv, txt, json)")]
        target_format: String,
        #[tool(param)]
        #[schemars(description = "Output file path")]
        output: String,
        #[tool(param)]
        #[schemars(description = "Optional password for encrypted source documents")]
        password: Option<String>,
    ) -> String {
        let source = validate_path!(source);
        let output = validate_path!(output);
        match crate::converters::convert_with_password(
            &source,
            &target_format,
            &output,
            password.as_deref(),
        ) {
            Ok(result) => serde_json::to_string_pretty(&result).unwrap_or_default(),
            Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
        }
    }

    #[tool(
        description = "Extract embedded images from a DOCX or PPTX document and save them to a directory"
    )]
    pub fn extract_images(
        &self,
        #[tool(param)]
        #[schemars(description = "File path to the DOCX or PPTX document")]
        file_path: String,
        #[tool(param)]
        #[schemars(description = "Directory where the extracted images should be saved")]
        output_dir: String,
    ) -> String {
        let file_path = validate_path!(file_path);
        let output_dir = validate_path!(output_dir);
        match handlers::extract_images_from_zip(&file_path, &output_dir) {
            Ok(images) => serde_json::json!({
                "success": true,
                "extracted_count": images.len(),
                "images": images
            })
            .to_string(),
            Err(e) => serde_json::json!({"error": e}).to_string(),
        }
    }

    #[tool(
        description = "Split a PDF document into a subset of pages specified by a start and end page (1-based, inclusive)"
    )]
    pub fn split_pdf(
        &self,
        #[tool(param)]
        #[schemars(description = "File path to the PDF document to split")]
        file_path: String,
        #[tool(param)]
        #[schemars(description = "File path to save the split PDF")]
        output_path: String,
        #[tool(param)]
        #[schemars(description = "Start page number (1-based, inclusive)")]
        start_page: u32,
        #[tool(param)]
        #[schemars(description = "End page number (1-based, inclusive)")]
        end_page: u32,
        #[tool(param)]
        #[schemars(description = "Optional password for encrypted documents")]
        password: Option<String>,
    ) -> String {
        let file_path = validate_path!(file_path);
        let output_path = validate_path!(output_path);
        match handlers::pdf::split_pdf_with_password(
            &file_path,
            &output_path,
            start_page,
            end_page,
            password.as_deref(),
        ) {
            Ok(_) => serde_json::json!({
                "success": true,
                "split_path": output_path,
                "pages_kept": (end_page + 1 - start_page)
            })
            .to_string(),
            Err(e) => serde_json::json!({"error": e}).to_string(),
        }
    }

    #[tool(description = "Create an HTML document from text content")]
    pub fn create_html(
        &self,
        #[tool(param)]
        #[schemars(description = "File path to save the HTML")]
        file_path: String,
        #[tool(param)]
        #[schemars(description = "Body text content (paragraphs separated by newlines)")]
        body: String,
        #[tool(param)]
        #[schemars(description = "Optional HTML title")]
        title: Option<String>,
    ) -> String {
        let file_path = validate_path!(file_path);
        let title = title.unwrap_or_else(|| "Document".to_string());
        let escaped_body = body
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");
        let paragraphs: String = escaped_body
            .split('\n')
            .filter(|l| !l.is_empty())
            .map(|p| format!("    <p>{}</p>\n", p.trim()))
            .collect();
        let html = format!(
            "<!DOCTYPE html>\n\
            <html lang=\"en\">\n\
            <head>\n\
            <meta charset=\"utf-8\">\n\
            <title>{}</title>\n\
            </head>\n\
            <body>\n\
            {}\
            </body>\n\
            </html>\n",
            title, paragraphs
        );
        match std::fs::write(&file_path, &html) {
            Ok(_) => serde_json::json!({
                "success": true,
                "path": file_path,
                "format": "html",
                "size_bytes": html.len(),
            })
            .to_string(),
            Err(e) => {
                serde_json::json!({"error": format!("Failed to write HTML: {e}")}).to_string()
            }
        }
    }

    // ═══════════════════════════════════════════
    //  BATCH TOOLS
    // ═══════════════════════════════════════════

    #[tool(
        description = "Batch convert all files in a directory matching a pattern with advanced options"
    )]
    pub fn batch_convert(
        &self,
        #[tool(param)]
        #[schemars(description = "Input directory path")]
        input_dir: String,
        #[tool(param)]
        #[schemars(description = "File pattern (e.g., *.docx, *.pdf)")]
        pattern: String,
        #[tool(param)]
        #[schemars(description = "Target format")]
        target_format: String,
        #[tool(param)]
        #[schemars(description = "Output directory")]
        output_dir: String,
        #[tool(param)]
        #[schemars(description = "If true, walk directories recursively")]
        recursive: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Optional password for encrypted source documents")]
        password: Option<String>,
        #[tool(param)]
        #[schemars(description = "Optional concurrency thread limit")]
        concurrency: Option<usize>,
    ) -> String {
        let input_dir = validate_path!(input_dir);
        let output_dir = validate_path!(output_dir);
        let results = crate::batch::batch_convert_extended(
            &input_dir,
            &pattern,
            &target_format,
            &output_dir,
            recursive.unwrap_or(false),
            password.as_deref(),
            concurrency,
        );
        serde_json::to_string_pretty(&results).unwrap_or_default()
    }

    // ═══════════════════════════════════════════
    //  DOCX TOOLS
    // ═══════════════════════════════════════════

    #[tool(
        description = "Create a new DOCX document. Professional layout: Page size must be US Letter (12240x15840 DXA) or A4, margins 1 inch (1440 DXA). Use Georgia/Cambria for headings, Calibri/Arial for body text. Left-align paragraphs; center only titles. Include a Table of Contents (TOC) using HeadingLevels."
    )]
    pub fn create_docx(
        &self,
        #[tool(param)]
        #[schemars(description = "File path to save the document")]
        file_path: String,
        #[tool(param)]
        #[schemars(description = "Optional document title (placed centered on page 1)")]
        title: Option<String>,
    ) -> String {
        let file_path = validate_path!(file_path);
        docx::create_document(&file_path, title.as_deref())
    }

    #[tool(
        description = "Add a paragraph with formatting and layout to a DOCX. Keep layout clean; use Georgia for headings and Calibri/Arial for body text. Never manually insert Unicode bullets."
    )]
    pub fn docx_add_paragraph(
        &self,
        #[tool(param)]
        #[schemars(description = "File path to the document")]
        file_path: String,
        #[tool(param)]
        #[schemars(
            description = "Text content. Never insert manual bullet character lists; use standard paragraph lines."
        )]
        text: String,
        #[tool(param)]
        #[schemars(description = "Optional bold. Use for titles and table headers.")]
        bold: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Optional italic. Use for captions, quotes, or sub-details.")]
        italic: Option<bool>,
        #[tool(param)]
        #[schemars(
            description = "Optional underline. WARNING: Avoid underlining titles (use whitespace or color instead)."
        )]
        underline: Option<bool>,
        #[tool(param)]
        #[schemars(
            description = "Optional font size in points (e.g. 24-28 for titles, 14-16 for subheadings, 11-12 for body)."
        )]
        font_size: Option<f32>,
        #[tool(param)]
        #[schemars(
            description = "Optional font family name. Pair Georgia/Cambria (headings) with Arial/Calibri (body)."
        )]
        font_family: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "Optional font color (Hex RGB, e.g. F7931A for Bitcoin Gold, 1E2761 for Navy)."
        )]
        color: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "Optional font highlight color (Hex RGB, e.g. FFFF00 for yellow)."
        )]
        highlight: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "Optional alignment: left, center, right, justify. Left-align body paragraphs; center titles."
        )]
        alignment: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "Optional shading fill color (Hex RGB) to highlight whole paragraph block."
        )]
        shading: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "Optional line spacing. Recommended: 1.15 to 1.3 for optimal readability."
        )]
        line_spacing: Option<f64>,
        #[tool(param)]
        #[schemars(description = "Optional keep with next paragraph to prevent orphaned headers.")]
        keep_with_next: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Optional keep lines together inside a single page.")]
        keep_together: Option<bool>,
        #[tool(param)]
        #[schemars(
            description = "Optional page break before paragraph. Use to structure exactly 10-page chapters."
        )]
        page_break_before: Option<bool>,
    ) -> String {
        let file_path = validate_path!(file_path);
        docx::add_paragraph(
            &file_path,
            &text,
            bold,
            italic,
            underline,
            font_size,
            font_family,
            color,
            highlight,
            alignment,
            shading,
            line_spacing,
            keep_with_next,
            keep_together,
            page_break_before,
        )
    }

    #[tool(
        description = "Add a table to a DOCX document with premium styling. Tables must fit clean column boundaries; use alternating rows and clear header shading."
    )]
    pub fn docx_add_table(
        &self,
        #[tool(param)]
        #[schemars(description = "File path to the document")]
        file_path: String,
        #[tool(param)]
        #[schemars(
            description = "Headers (JSON array of strings). E.g. ['Item', 'Quantity', 'Cost']."
        )]
        headers: Vec<String>,
        #[tool(param)]
        #[schemars(
            description = "Data rows (JSON array of arrays of strings). Must match header length."
        )]
        data: Vec<Vec<String>>,
        #[tool(param)]
        #[schemars(
            description = "Optional table width percentage (0 to 100). Default is 100.0 (full width of text margins)."
        )]
        width_pct: Option<f64>,
        #[tool(param)]
        #[schemars(
            description = "Optional table alignment: left, center, right, justify. Center is highly recommended for structured reports."
        )]
        alignment: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "Optional table border style. Recommended: 'single' for professional clean lines."
        )]
        border_style: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "Optional border size in eighths of a pt. Recommended: 4 to 8 for subtle borders."
        )]
        border_size: Option<u32>,
        #[tool(param)]
        #[schemars(
            description = "Optional border color (Hex RGB, e.g. CCCCCC for subtle light grey, avoid solid black)."
        )]
        border_color: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "Optional shading header color (Hex RGB, e.g. 1E2761 for deep navy, F7931A for gold)."
        )]
        shading_header: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "Optional shading data cell color (Hex RGB, e.g. F5F5F5 for zebra alternating rows)."
        )]
        shading_data: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "Optional prevent page breaks inside rows (default: true). Ensures clean layout rendering."
        )]
        cant_split: Option<bool>,
    ) -> String {
        let file_path = validate_path!(file_path);
        docx::add_table(
            &file_path,
            &headers,
            &data,
            width_pct,
            alignment,
            border_style,
            border_size,
            border_color,
            shading_header,
            shading_data,
            cant_split,
        )
    }

    #[tool(description = "Add an image with size options to a DOCX document")]
    pub fn docx_add_image(
        &self,
        #[tool(param)]
        #[schemars(description = "File path to the document")]
        file_path: String,
        #[tool(param)]
        #[schemars(description = "File path to the image to insert")]
        image_path: String,
        #[tool(param)]
        #[schemars(description = "Optional image width in inches (default: 2.0)")]
        width_inches: Option<f64>,
        #[tool(param)]
        #[schemars(description = "Optional image height in inches (default: 1.5)")]
        height_inches: Option<f64>,
    ) -> String {
        let file_path = validate_path!(file_path);
        let image_path = validate_path!(image_path);
        docx::add_image(&file_path, &image_path, width_inches, height_inches)
    }

    // ═══════════════════════════════════════════
    //  PPTX TOOLS
    // ═══════════════════════════════════════════

    #[tool(
        description = "Create a new PowerPoint presentation. Design rules: Choose a bold, topic-informed color palette (e.g. Midnight Executive, Forest & Moss, Coral Energy). Maintain 60-30-10 dominance. Leave breathing room—don't fill every inch. Use Georgia/Calibri/Segoe UI. Left-align body text."
    )]
    pub fn create_pptx(
        &self,
        #[tool(param)]
        #[schemars(description = "File path to save the presentation")]
        file_path: String,
        #[tool(param)]
        #[schemars(description = "Optional title (rendered on title slide)")]
        title: Option<String>,
    ) -> String {
        let file_path = validate_path!(file_path);
        pptx::create_presentation(&file_path, title.as_deref())
    }

    #[tool(
        description = "Add a slide with title, optional body bullet points, and custom layout styling. Enforce 60-30-10 color rule. Use dark/light sandwich structures. Never use accent lines under titles."
    )]
    pub fn pptx_add_slide(
        &self,
        #[tool(param)]
        #[schemars(description = "File path to the presentation")]
        file_path: String,
        #[tool(param)]
        #[schemars(description = "Slide title. Use concise, active titles (36-44pt).")]
        title: String,
        #[tool(param)]
        #[schemars(
            description = "Optional bullet point content (JSON array of strings). E.g. ['Point 1', 'Point 2']."
        )]
        body: Option<Vec<String>>,
        #[tool(param)]
        #[schemars(
            description = "Optional slide background color (Hex RGB, e.g. 1A1A2E for deep space navy dark-mode, FFFFFF for clean light mode)."
        )]
        bg_color: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "Optional font size for body points. Recommended: 14-16pt for optimal visibility."
        )]
        font_size: Option<f32>,
        #[tool(param)]
        #[schemars(
            description = "Optional font color (Hex RGB, e.g. FFFFFF for dark slides, 333333 for light slides. Ensure contrast!)."
        )]
        font_color: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "Optional font family name. Pair Georgia/Cambria (title) with Arial/Calibri/Segoe UI (body)."
        )]
        font_family: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "Optional alignment: left, center, right, justify. Left-align body bullet text; center title slides."
        )]
        alignment: Option<String>,
    ) -> String {
        let file_path = validate_path!(file_path);
        pptx::add_slide(
            &file_path,
            &title,
            body.as_deref(),
            bg_color,
            font_size,
            font_color,
            font_family,
            alignment,
        )
    }

    // ═══════════════════════════════════════════
    //  XLSX TOOLS
    // ═══════════════════════════════════════════

    #[tool(description = "Create a new XLSX workbook with sheets, headers, and data")]
    pub fn create_xlsx(
        &self,
        #[tool(param)]
        #[schemars(description = "File path to save the XLSX")]
        file_path: String,
        #[tool(param)]
        #[schemars(
            description = "JSON array of sheets: [{\"name\": \"Sheet1\", \"headers\": [\"A\",\"B\"], \"data\": [[\"1\",\"2\"]]}]"
        )]
        sheets: serde_json::Value,
    ) -> String {
        let file_path = validate_path!(file_path);
        let sheets: Vec<xlsx::XlsxSheet> = match serde_json::from_value(sheets) {
            Ok(s) => s,
            Err(e) => {
                return serde_json::json!({"error": format!("Invalid JSON structure: {e}")})
                    .to_string()
            }
        };
        xlsx::create_xlsx(&file_path, &sheets)
    }

    #[tool(
        description = "Edit an existing XLSX spreadsheet by applying sheet additions and cell updates"
    )]
    pub fn edit_xlsx(
        &self,
        #[tool(param)]
        #[schemars(description = "File path of the existing XLSX to edit")]
        file_path: String,
        #[tool(param)]
        #[schemars(description = "Optional JSON array of sheet names to add: [\"Summary\"]")]
        add_sheets: Option<serde_json::Value>,
        #[tool(param)]
        #[schemars(
            description = "Optional JSON array of cell updates: [{\"sheet_name\": \"Sheet1\", \"row\": 1, \"col\": 1, \"value\": \"31\"}]"
        )]
        cell_updates: Option<serde_json::Value>,
    ) -> String {
        let file_path = validate_path!(file_path);

        let parsed_add_sheets: Option<Vec<String>> =
            add_sheets.and_then(|v| serde_json::from_value(v).ok());
        let parsed_cell_updates: Option<Vec<xlsx::XlsxCellOperation>> =
            cell_updates.and_then(|v| serde_json::from_value(v).ok());

        let request = xlsx::XlsxEditRequest {
            file_path,
            cell_updates: parsed_cell_updates,
            add_sheets: parsed_add_sheets,
        };

        match xlsx::edit_xlsx(&request) {
            Ok(json_res) => json_res,
            Err(e) => serde_json::json!({"error": e}).to_string(),
        }
    }

    // ═══════════════════════════════════════════
    //  PDF TOOLS
    // ═══════════════════════════════════════════

    #[tool(description = "Create a simple PDF with text (auto word-wrap, multi-page)")]
    pub fn create_pdf(
        &self,
        #[tool(param)]
        #[schemars(description = "File path to save the PDF")]
        file_path: String,
        #[tool(param)]
        #[schemars(description = "Text content")]
        text: String,
        #[tool(param)]
        #[schemars(description = "Optional author name")]
        author: Option<String>,
    ) -> String {
        let file_path = validate_path!(file_path);
        pdf::create_pdf(&file_path, &text, author.as_deref())
    }

    #[tool(
        description = "Create a PDF with premium layout control: title page, page numbers, margins, font size. Use explicit Form Feed page breaks ('\\x0c' or '\\f') to split pages. Never use unicode subscript/superscripts."
    )]
    #[allow(clippy::too_many_arguments)]
    pub fn create_formatted_pdf(
        &self,
        #[tool(param)]
        #[schemars(description = "File path to save the PDF")]
        file_path: String,
        #[tool(param)]
        #[schemars(
            description = "Text content. Separate chapters/pages using Form Feed '\\x0c' or '\\f' characters."
        )]
        text: String,
        #[tool(param)]
        #[schemars(description = "Optional document title (rendered centered on page 1).")]
        title: Option<String>,
        #[tool(param)]
        #[schemars(description = "Optional author name (printed on page 1 below title).")]
        author: Option<String>,
        #[tool(param)]
        #[schemars(description = "Whether to show page numbers in footer (default: false).")]
        page_numbers: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Font size in points (default: 12).")]
        font_size: Option<f64>,
        #[tool(param)]
        #[schemars(description = "Top margin in points (default: 72 = 1 inch).")]
        margin_top: Option<f64>,
        #[tool(param)]
        #[schemars(description = "Bottom margin in points (default: 72 = 1 inch).")]
        margin_bottom: Option<f64>,
        #[tool(param)]
        #[schemars(description = "Left margin in points (default: 72 = 1 inch).")]
        margin_left: Option<f64>,
        #[tool(param)]
        #[schemars(description = "Right margin in points (default: 72 = 1 inch).")]
        margin_right: Option<f64>,
    ) -> String {
        let file_path = validate_path!(file_path);
        let config = pdf::PdfLayoutConfig {
            title,
            author,
            page_numbers: page_numbers.unwrap_or(false),
            font_size: font_size.unwrap_or(12.0),
            margin_top: margin_top.unwrap_or(72.0),
            margin_bottom: margin_bottom.unwrap_or(72.0),
            margin_left: margin_left.unwrap_or(72.0),
            margin_right: margin_right.unwrap_or(72.0),
            ..Default::default()
        };
        pdf::create_formatted_pdf(&file_path, &text, &config)
    }

    #[tool(description = "Merge multiple PDFs into one file")]
    pub fn merge_pdfs(
        &self,
        #[tool(param)]
        #[schemars(description = "List of PDF file paths to merge")]
        sources: Vec<String>,
        #[tool(param)]
        #[schemars(description = "Output PDF file path")]
        output: String,
    ) -> String {
        let output = validate_path!(output);
        let sources: Vec<String> = match sources
            .iter()
            .map(|s| crate::security::validate_path(s))
            .collect::<Result<Vec<_>, _>>()
        {
            Ok(paths) => paths
                .into_iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect(),
            Err(e) => {
                return serde_json::json!({"error": format!("Security error: {e}")}).to_string()
            }
        };
        pdf::merge_pdfs(&sources, &output)
    }

    #[tool(description = "Extract text from a PDF (optionally from a specific page)")]
    pub fn extract_pdf_text(
        &self,
        #[tool(param)]
        #[schemars(description = "File path to the PDF")]
        file_path: String,
        #[tool(param)]
        #[schemars(description = "Optional page number (0-based)")]
        page: Option<u32>,
    ) -> String {
        let file_path = validate_path!(file_path);
        pdf::extract_text(&file_path, page)
    }

    #[tool(description = "List all PDF form fields (AcroForm) with types, values, and flags")]
    pub fn list_pdf_fields(
        &self,
        #[tool(param)]
        #[schemars(description = "File path to the PDF")]
        file_path: String,
    ) -> String {
        let file_path = validate_path!(file_path);
        match pdf_forms::list_form_fields(&file_path) {
            Ok(fields) => serde_json::json!({
                "success": true,
                "field_count": fields.len(),
                "fields": fields,
            })
            .to_string(),
            Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
        }
    }

    #[tool(
        description = "Fill PDF form fields with values (supports AcroForm text, checkbox, and choice fields)"
    )]
    pub fn fill_pdf_form(
        &self,
        #[tool(param)]
        #[schemars(description = "File path to the PDF")]
        file_path: String,
        #[tool(param)]
        #[schemars(description = "JSON object mapping field names to values")]
        values: serde_json::Value,
    ) -> String {
        let file_path = validate_path!(file_path);
        let vals: Vec<(String, String)> = if let serde_json::Value::Object(map) = values {
            map.into_iter()
                .map(|(k, v)| {
                    let val_str = match v {
                        serde_json::Value::String(s) => s,
                        serde_json::Value::Null => "".to_string(),
                        other => other.to_string(),
                    };
                    (k, val_str)
                })
                .collect()
        } else {
            return serde_json::json!({"error": "values must be a JSON object"}).to_string();
        };

        match pdf_forms::fill_form_fields(&file_path, &vals) {
            Ok(count) => serde_json::json!({
                "success": true,
                "filled": count,
            })
            .to_string(),
            Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
        }
    }

    // ═══════════════════════════════════════════
    //  METADATA TOOLS
    // ═══════════════════════════════════════════

    #[tool(description = "Find all tables in a document and return them as structured JSON")]
    pub fn find_tables(
        &self,
        #[tool(param)]
        #[schemars(description = "File path to the document")]
        file_path: String,
    ) -> String {
        let file_path = validate_path!(file_path);
        match handlers::load_to_ir(&file_path) {
            Ok(ir) => serde_json::json!({
                "success": true,
                "table_count": ir.tables.len(),
                "tables": ir.tables,
            })
            .to_string(),
            Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
        }
    }

    #[tool(
        description = "Analyze document complexity: detect scanned PDFs, text density, OCR requirements"
    )]
    pub fn analyze_document_complexity(
        &self,
        #[tool(param)]
        #[schemars(description = "File path to the document")]
        file_path: String,
    ) -> String {
        let file_path = validate_path!(file_path);
        match handlers::load_to_ir(&file_path) {
            Ok(ir) => {
                let report = crate::engine::complexity::analyze_complexity(&ir);
                serde_json::to_string_pretty(&report).unwrap_or_default()
            }
            Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
        }
    }

    #[tool(
        description = "Run OCR on a scanned PDF or image-based document (requires --features ocr)"
    )]
    pub fn ocr_document(
        &self,
        #[tool(param)]
        #[schemars(description = "File path to the document")]
        file_path: String,
        #[tool(param)]
        #[schemars(
            description = "Language code (default: eng). See tesseract docs for supported languages."
        )]
        language: Option<String>,
    ) -> String {
        let file_path = validate_path!(file_path);
        match crate::ocr::ocr_document(&file_path, language.as_deref()) {
            Ok(ir) => serde_json::json!({
                "success": true,
                "format": ir.format,
                "paragraphs": ir.paragraphs.len(),
                "text": ir.text,
            })
            .to_string(),
            Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
        }
    }

    #[tool(description = "Check if OCR engine is available on this system")]
    pub fn check_ocr_available(&self) -> String {
        serde_json::json!({
            "available": crate::ocr::is_ocr_available(),
            "languages": crate::ocr::available_languages(),
        })
        .to_string()
    }

    #[tool(
        description = "Render pages of a PDF, image, or office document (DOCX, PPTX, XLSX) into PNG screenshots for visual reasoning"
    )]
    pub fn render_document_pages(
        &self,
        #[tool(param)]
        #[schemars(description = "File path to the document")]
        file_path: String,
        #[tool(param)]
        #[schemars(description = "Target directory to save the rendered PNG files")]
        output_dir: String,
        #[tool(param)]
        #[schemars(description = "Optional resolution DPI (default: 150)")]
        dpi: Option<u32>,
        #[tool(param)]
        #[schemars(
            description = "Optional list of 1-based page numbers to render (renders all pages if omitted)"
        )]
        pages: Option<Vec<u32>>,
    ) -> String {
        let file_path = validate_path!(file_path);
        let output_dir = validate_path!(output_dir);

        match crate::converters::render::render_document_pages(&file_path, &output_dir, dpi, pages)
        {
            Ok(files) => serde_json::json!({
                "success": true,
                "rendered_files": files,
            })
            .to_string(),
            Err(e) => serde_json::json!({"error": e}).to_string(),
        }
    }

    #[tool(
        description = "Recursively unpack a ZIP archive (including nested archives), parse all supported documents inside it, and compile a single structured Markdown digest report (digest.md)"
    )]
    pub fn extract_archive_digest(
        &self,
        #[tool(param)]
        #[schemars(description = "Absolute path to the ZIP archive")]
        archive_path: String,
        #[tool(param)]
        #[schemars(
            description = "Optional destination directory. If not specified, a temporary directory will be created."
        )]
        output_dir: Option<String>,
    ) -> String {
        let archive_path = validate_path!(archive_path);
        let output_dir_val = output_dir.map(|p| validate_path!(p));

        match crate::batch::archive::process_archive_digest(
            &archive_path,
            output_dir_val.as_deref(),
        ) {
            Ok(res) => serde_json::to_string(&res).unwrap_or_default(),
            Err(e) => serde_json::json!({"error": e}).to_string(),
        }
    }

    #[tool(
        description = "Extract structured domain entities (legal, financial, timeline) from a document using pre-defined rules and heuristics"
    )]
    pub fn extract_structured_metadata(
        &self,
        #[tool(param)]
        #[schemars(description = "File path to the document")]
        file_path: String,
        #[tool(param)]
        #[schemars(
            description = "The target domain template: 'legal', 'financial', or 'timeline'"
        )]
        template_type: String,
    ) -> String {
        let file_path = validate_path!(file_path);

        let doc = match crate::handlers::load_to_ir(&file_path) {
            Ok(d) => d,
            Err(e) => return serde_json::json!({"error": e.to_string()}).to_string(),
        };

        match template_type.to_lowercase().as_str() {
            "legal" => {
                let res = crate::engine::extract::extract_legal(&doc);
                serde_json::to_string_pretty(&res).unwrap_or_default()
            }
            "financial" => {
                let res = crate::engine::extract::extract_financial(&doc);
                serde_json::to_string_pretty(&res).unwrap_or_default()
            }
            "timeline" => {
                let res = crate::engine::extract::extract_timeline(&doc);
                serde_json::to_string_pretty(&res).unwrap_or_default()
            }
            other => serde_json::json!({
                "error": format!("Unsupported template type: '{}'. Supported types: 'legal', 'financial', 'timeline'.", other)
            }).to_string(),
        }
    }

    // ═══════════════════════════════════════════
    //  UTILITY TOOLS
    // ═══════════════════════════════════════════

    #[tool(description = "List all available tools and their descriptions")]
    pub fn list_capabilities(&self) -> String {
        serde_json::to_string_pretty(&serde_json::json!({
            "server": "opendoc-mcp",
            "version": env!("CARGO_PKG_VERSION"),
            "description": "Rust-native Document Intelligence Engine for AI Agents",
            "formats": ["docx", "pptx", "pdf", "xlsx", "html", "md", "csv", "txt"],
            "tool_categories": {
                "document_intelligence": ["open_document", "read_document_text", "search_document", "replace_text", "diff_documents", "diff_documents_visual", "chunk_for_embedding", "fill_template", "validate_document", "validate_pdf_a_compliance"],
                "conversion": ["convert", "create_html"],
                "batch": ["batch_convert", "extract_archive_digest"],
                "docx": ["create_docx", "docx_add_paragraph", "docx_add_table", "docx_add_image"],
                "pptx": ["create_pptx", "pptx_add_slide"],
                "xlsx": ["create_xlsx", "edit_xlsx"],
                "pdf": ["create_pdf", "merge_pdfs", "extract_pdf_text", "list_pdf_fields", "fill_pdf_form"],
                "metadata": ["find_tables", "analyze_document_complexity", "extract_structured_metadata"],
                "ai_features": ["ocr_document", "check_ocr_available", "render_document_pages"],
                "utility": ["list_capabilities"]
            }
        }))
        .unwrap_or_default()
    }
}

#[tool(tool_box)]
impl ServerHandler for OpendocServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "Opendoc MCP Server — Document Intelligence Engine for AI Agents.\n\n\
                Supports DOCX, PPTX, PDF, XLSX, HTML, MD, CSV, TXT.\n\n\
                KEY CAPABILITIES:\n\
                • Open any document → structured JSON with outline, metadata, content\n\
                • Search, replace, diff, template fill, chunk for RAG\n\
                • Cross-format conversion (DOCX↔PDF, DOCX→MD, PDF→TXT, etc.)\n\
                • PDF form field listing and filling (AcroForm)\n\
                • Document complexity analysis (scanned PDF detection, OCR needs)\n\
                • OCR for scanned documents (with --features ocr)\n\
                • Batch processing entire directories\n\
                • Document validation and statistics\n\n\
                All operations work on a unified Internal Representation (IR),\n\
                meaning most tools work on ALL supported formats.\n\n\
                Use `list_capabilities` to see all available tools."
                    .into(),
            ),
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
            ..Default::default()
        }
    }

    fn list_resources(
        &self,
        _request: rmcp::model::PaginatedRequestParam,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> impl std::future::Future<Output = Result<rmcp::model::ListResourcesResult, rmcp::Error>>
           + Send
           + '_ {
        std::future::ready(Ok(rmcp::model::ListResourcesResult::default()))
    }

    fn read_resource(
        &self,
        request: rmcp::model::ReadResourceRequestParam,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> impl std::future::Future<Output = Result<rmcp::model::ReadResourceResult, rmcp::Error>>
           + Send
           + '_ {
        let uri = request.uri.clone();
        let fut = async move {
            if !uri.starts_with("doc://") {
                return Err(rmcp::Error::invalid_request(
                    format!("Invalid resource URI protocol: {}", uri),
                    None,
                ));
            }

            let path_part = &uri["doc://".len()..];
            let (file_path, is_outline) = if let Some(stripped) = path_part.strip_suffix("/outline")
            {
                (stripped, true)
            } else {
                (path_part, false)
            };

            let validated_path = crate::security::validate_path(file_path).map_err(|e| {
                rmcp::Error::invalid_request(format!("Security validation error: {}", e), None)
            })?;
            let path_str = validated_path.to_string_lossy().into_owned();

            let ir = handlers::load_to_ir(&path_str)
                .map_err(|e| rmcp::Error::resource_not_found(format!("Load error: {}", e), None))?;

            let text_content = if is_outline {
                serde_json::to_string_pretty(&ir.outline()).unwrap_or_default()
            } else {
                let text: Vec<String> = ir.paragraphs.iter().map(|p| p.text.clone()).collect();
                text.join("\n")
            };

            Ok(rmcp::model::ReadResourceResult {
                contents: vec![rmcp::model::ResourceContents::text(
                    text_content,
                    request.uri.clone(),
                )],
            })
        };
        fut
    }
}

impl OpendocServer {
    pub fn new() -> Self {
        Self
    }

    pub async fn run(self) -> anyhow::Result<()> {
        tracing::info!("Starting Opendoc Document Intelligence Server via stdio...");
        serve_server(self, stdio()).await?;
        Ok(())
    }
}

fn structured_error(code: &str, msg: &str, category: &str, suggestion: &str) -> String {
    serde_json::json!({
        "error": msg,
        "error_code": code,
        "category": category,
        "suggestion": suggestion
    })
    .to_string()
}

fn map_load_error(e: &handlers::LoadError) -> String {
    match e {
        handlers::LoadError::UnsupportedFormat(ext) => structured_error(
            "UNSUPPORTED_FORMAT",
            &format!("Unsupported file extension: '{ext}'"),
            "validation",
            "Ensure the file has a supported extension: docx, pptx, pdf, xlsx, html, md, csv, txt."
        ),
        handlers::LoadError::IoError(msg) => structured_error(
            "FILE_IO_ERROR",
            &format!("Failed to access file: {msg}"),
            "io",
            "Verify the file exists at the specified path and that the server has read permissions."
        ),
        handlers::LoadError::ParseError(msg) => structured_error(
            "PARSE_ERROR",
            &format!("Failed to parse document: {msg}"),
            "format",
            "The file may be corrupt or its structure does not match the expected format."
        ),
    }
}
