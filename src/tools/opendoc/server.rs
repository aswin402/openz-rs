use crate::tools::opendoc::engine::{diff, replace, search, template};
use crate::tools::opendoc::handlers::{self, docx, pdf, pdf_forms, pptx, xlsx};

macro_rules! validate_path {
    ($path:expr) => {
        match crate::tools::opendoc::security::validate_path(&$path) {
            Ok(p) => p.to_string_lossy().into_owned(),
            Err(e) => return serde_json::json!({"error": format!("Security error: {e}")}).to_string(),
        }
    };
}

fn normalize_json_param(val: serde_json::Value) -> serde_json::Value {
    match val {
        serde_json::Value::String(s) => {
            let trimmed = s.trim();
            if (trimmed.starts_with('{') && trimmed.ends_with('}'))
                || (trimmed.starts_with('[') && trimmed.ends_with(']'))
            {
                serde_json::from_str(trimmed).unwrap_or(serde_json::Value::String(s))
            } else {
                serde_json::Value::String(s)
            }
        }
        other => other,
    }
}

#[derive(Debug, Clone, Default)]
pub struct OpendocServer;

// ──────────────────────────────────────────────
//  DOCUMENT INTELLIGENCE TOOLS
// ──────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
impl OpendocServer {
    // ═══════════════════════════════════════════
    //  FORMAT-AGNOSTIC IR TOOLS
    // ═══════════════════════════════════════════

        pub fn open_document(
        &self,
                        file_path: String,
                        detail_level: Option<String>,
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

        pub fn read_document_text(
        &self,
                        file_path: String,
                        password: Option<String>,
    ) -> String {
        let file_path = validate_path!(file_path);
        match handlers::load_to_ir_with_password(&file_path, password.as_deref()) {
            Ok(ir) => {
                let content = if !ir.paragraphs.is_empty() {
                    let text: Vec<String> = ir.paragraphs.iter().map(|p| p.text.clone()).collect();
                    text.join("\n")
                } else {
                    ir.text.clone().unwrap_or_default()
                };
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

        pub fn search_document(
        &self,
                        file_path: String,
                        query: String,
                        use_regex: Option<bool>,
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

        pub fn replace_text(
        &self,
                        file_path: String,
                        find: String,
                        replace: String,
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
                            match crate::tools::opendoc::converters::export(&ir, &ext, &file_path) {
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

        pub fn diff_documents(
        &self,
                        file_a: String,
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

        pub fn diff_documents_visual(
        &self,
                        file_a: String,
                        file_b: String,
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

        pub fn chunk_for_embedding(
        &self,
                        file_path: String,
                        strategy: Option<String>,
                        max_tokens: Option<usize>,
                        overlap: Option<usize>,
    ) -> String {
        let file_path = validate_path!(file_path);
        match handlers::load_to_ir(&file_path) {
            Ok(ir) => {
                let chunking_strategy = match strategy {
                    Some(s) => match s.parse::<crate::tools::opendoc::engine::chunk::ChunkingStrategy>() {
                        Ok(st) => st,
                        Err(e) => return serde_json::json!({"error": e}).to_string(),
                    },
                    None => crate::tools::opendoc::engine::chunk::ChunkingStrategy::Fixed,
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

        pub fn fill_template(
        &self,
                        file_path: String,
                        variables: serde_json::Value,
                        password: Option<String>,
    ) -> String {
        let file_path = validate_path!(file_path);
        let variables = normalize_json_param(variables);
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
                            match crate::tools::opendoc::converters::export(&ir, &ext, &file_path) {
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

        pub fn validate_document(
        &self,
                        file_path: String,
                        password: Option<String>,
    ) -> String {
        let file_path = validate_path!(file_path);
        match handlers::load_to_ir_with_password(&file_path, password.as_deref()) {
            Ok(ir) => {
                let result = crate::tools::opendoc::validators::validate_document(&ir);
                serde_json::to_string_pretty(&result).unwrap_or_default()
            }
            Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
        }
    }

        pub fn validate_pdf_a_compliance(
        &self,
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

        match crate::tools::opendoc::validators::pdf_a::validate_pdf_a(path) {
            Ok(result) => serde_json::to_string_pretty(&result).unwrap_or_default(),
            Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
        }
    }

    // ═══════════════════════════════════════════
    //  CONVERSION TOOLS
    // ═══════════════════════════════════════════

        pub fn convert(
        &self,
                        source: String,
                        target_format: String,
                        output: String,
                        password: Option<String>,
    ) -> String {
        let source = validate_path!(source);
        let output = validate_path!(output);
        match crate::tools::opendoc::converters::convert_with_password(
            &source,
            &target_format,
            &output,
            password.as_deref(),
        ) {
            Ok(result) => serde_json::to_string_pretty(&result).unwrap_or_default(),
            Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
        }
    }

        pub fn extract_images(
        &self,
                        file_path: String,
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

        pub fn split_pdf(
        &self,
                        file_path: String,
                        output_path: String,
                        start_page: u32,
                        end_page: u32,
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

        pub fn create_html(
        &self,
                        file_path: String,
                        body: String,
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

        pub fn batch_convert(
        &self,
                        input_dir: String,
                        pattern: String,
                        target_format: String,
                        output_dir: String,
                        recursive: Option<bool>,
                        password: Option<String>,
                        concurrency: Option<usize>,
    ) -> String {
        let input_dir = validate_path!(input_dir);
        let output_dir = validate_path!(output_dir);
        let results = crate::tools::opendoc::batch::batch_convert_extended(
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

        pub fn create_docx(
        &self,
                        file_path: String,
                        title: Option<String>,
    ) -> String {
        let file_path = validate_path!(file_path);
        docx::create_document(&file_path, title.as_deref())
    }

        pub fn docx_add_paragraph(
        &self,
                        file_path: String,
                        text: String,
                        bold: Option<bool>,
                        italic: Option<bool>,
                        underline: Option<bool>,
                        font_size: Option<f32>,
                        font_family: Option<String>,
                        color: Option<String>,
                        highlight: Option<String>,
                        alignment: Option<String>,
                        shading: Option<String>,
                        line_spacing: Option<f64>,
                        keep_with_next: Option<bool>,
                        keep_together: Option<bool>,
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

        pub fn docx_add_table(
        &self,
                        file_path: String,
                        headers: Vec<String>,
                        data: Vec<Vec<String>>,
                        width_pct: Option<f64>,
                        alignment: Option<String>,
                        border_style: Option<String>,
                        border_size: Option<u32>,
                        border_color: Option<String>,
                        shading_header: Option<String>,
                        shading_data: Option<String>,
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

        pub fn docx_add_image(
        &self,
                        file_path: String,
                        image_path: String,
                        width_inches: Option<f64>,
                        height_inches: Option<f64>,
    ) -> String {
        let file_path = validate_path!(file_path);
        let image_path = validate_path!(image_path);
        docx::add_image(&file_path, &image_path, width_inches, height_inches)
    }

    // ═══════════════════════════════════════════
    //  PPTX TOOLS
    // ═══════════════════════════════════════════

        pub fn create_pptx(
        &self,
                        file_path: String,
                        title: Option<String>,
    ) -> String {
        let file_path = validate_path!(file_path);
        pptx::create_presentation(&file_path, title.as_deref())
    }

        pub fn pptx_add_slide(
        &self,
                        file_path: String,
                        title: String,
                        body: Option<Vec<String>>,
                        bg_color: Option<String>,
                        font_size: Option<f32>,
                        font_color: Option<String>,
                        font_family: Option<String>,
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

        pub fn create_xlsx(
        &self,
                        file_path: String,
                        sheets: serde_json::Value,
    ) -> String {
        let file_path = validate_path!(file_path);
        let sheets = normalize_json_param(sheets);
        let sheets: Vec<xlsx::XlsxSheet> = match serde_json::from_value(sheets) {
            Ok(s) => s,
            Err(e) => {
                return serde_json::json!({"error": format!("Invalid JSON structure: {e}")})
                    .to_string()
            }
        };
        xlsx::create_xlsx(&file_path, &sheets)
    }

        pub fn edit_xlsx(
        &self,
                        file_path: String,
                        add_sheets: Option<serde_json::Value>,
                        cell_updates: Option<serde_json::Value>,
    ) -> String {
        let file_path = validate_path!(file_path);

        let parsed_add_sheets: Option<Vec<String>> = add_sheets
            .map(normalize_json_param)
            .and_then(|v| serde_json::from_value(v).ok());
        let parsed_cell_updates: Option<Vec<xlsx::XlsxCellOperation>> = cell_updates
            .map(normalize_json_param)
            .and_then(|v| serde_json::from_value(v).ok());

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

        pub fn create_pdf(
        &self,
                        file_path: String,
                        text: String,
                        author: Option<String>,
    ) -> String {
        let file_path = validate_path!(file_path);
        pdf::create_pdf(&file_path, &text, author.as_deref())
    }

        #[allow(clippy::too_many_arguments)]
    pub fn create_formatted_pdf(
        &self,
                        file_path: String,
                        text: String,
                        title: Option<String>,
                        author: Option<String>,
                        page_numbers: Option<bool>,
                        font_size: Option<f64>,
                        margin_top: Option<f64>,
                        margin_bottom: Option<f64>,
                        margin_left: Option<f64>,
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

        pub fn merge_pdfs(
        &self,
                        sources: Vec<String>,
                        output: String,
    ) -> String {
        let output = validate_path!(output);
        let sources: Vec<String> = match sources
            .iter()
            .map(|s| crate::tools::opendoc::security::validate_path(s))
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

        pub fn extract_pdf_text(
        &self,
                        file_path: String,
                        page: Option<u32>,
    ) -> String {
        let file_path = validate_path!(file_path);
        pdf::extract_text(&file_path, page)
    }

        pub fn list_pdf_fields(
        &self,
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

        pub fn fill_pdf_form(
        &self,
                        file_path: String,
                        values: serde_json::Value,
    ) -> String {
        let file_path = validate_path!(file_path);
        let values = normalize_json_param(values);
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

        pub fn find_tables(
        &self,
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

        pub fn analyze_document_complexity(
        &self,
                        file_path: String,
    ) -> String {
        let file_path = validate_path!(file_path);
        match handlers::load_to_ir(&file_path) {
            Ok(ir) => {
                let report = crate::tools::opendoc::engine::complexity::analyze_complexity(&ir);
                serde_json::to_string_pretty(&report).unwrap_or_default()
            }
            Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
        }
    }

        pub fn ocr_document(
        &self,
                        file_path: String,
                        language: Option<String>,
    ) -> String {
        let file_path = validate_path!(file_path);
        match crate::tools::opendoc::ocr::ocr_document(&file_path, language.as_deref()) {
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

        pub fn check_ocr_available(&self) -> String {
        serde_json::json!({
            "available": crate::tools::opendoc::ocr::is_ocr_available(),
            "languages": crate::tools::opendoc::ocr::available_languages(),
        })
        .to_string()
    }

        pub fn render_document_pages(
        &self,
                        file_path: String,
                        output_dir: String,
                        dpi: Option<u32>,
                        pages: Option<Vec<u32>>,
    ) -> String {
        let file_path = validate_path!(file_path);
        let output_dir = validate_path!(output_dir);

        match crate::tools::opendoc::converters::render::render_document_pages(&file_path, &output_dir, dpi, pages)
        {
            Ok(files) => serde_json::json!({
                "success": true,
                "rendered_files": files,
            })
            .to_string(),
            Err(e) => serde_json::json!({"error": e}).to_string(),
        }
    }

        pub fn extract_archive_digest(
        &self,
                        archive_path: String,
                        output_dir: Option<String>,
    ) -> String {
        let archive_path = validate_path!(archive_path);
        let output_dir_val = output_dir.map(|p| validate_path!(p));

        match crate::tools::opendoc::batch::archive::process_archive_digest(
            &archive_path,
            output_dir_val.as_deref(),
        ) {
            Ok(res) => serde_json::to_string(&res).unwrap_or_default(),
            Err(e) => serde_json::json!({"error": e}).to_string(),
        }
    }

        pub fn extract_structured_metadata(
        &self,
                        file_path: String,
                        template_type: String,
    ) -> String {
        let file_path = validate_path!(file_path);

        let doc = match crate::tools::opendoc::handlers::load_to_ir(&file_path) {
            Ok(d) => d,
            Err(e) => return serde_json::json!({"error": e.to_string()}).to_string(),
        };

        match template_type.to_lowercase().as_str() {
            "legal" => {
                let res = crate::tools::opendoc::engine::extract::extract_legal(&doc);
                serde_json::to_string_pretty(&res).unwrap_or_default()
            }
            "financial" => {
                let res = crate::tools::opendoc::engine::extract::extract_financial(&doc);
                serde_json::to_string_pretty(&res).unwrap_or_default()
            }
            "timeline" => {
                let res = crate::tools::opendoc::engine::extract::extract_timeline(&doc);
                serde_json::to_string_pretty(&res).unwrap_or_default()
            }
            "general" | "" => {
                let legal = crate::tools::opendoc::engine::extract::extract_legal(&doc);
                let financial = crate::tools::opendoc::engine::extract::extract_financial(&doc);
                let timeline = crate::tools::opendoc::engine::extract::extract_timeline(&doc);
                serde_json::to_string_pretty(&serde_json::json!({
                    "legal": legal,
                    "financial": financial,
                    "timeline": timeline,
                }))
                .unwrap_or_default()
            }
            other => serde_json::json!({
                "error": format!("Unsupported template type: '{}'. Supported types: 'legal', 'financial', 'timeline', 'general'.", other)
            }).to_string(),
        }
    }

    // ═══════════════════════════════════════════
    //  UTILITY TOOLS
    // ═══════════════════════════════════════════

        pub fn list_capabilities(&self) -> String {
        serde_json::to_string_pretty(&serde_json::json!({
            "server": "opendoc",
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



impl OpendocServer {
    pub fn new() -> Self {
        Self
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
