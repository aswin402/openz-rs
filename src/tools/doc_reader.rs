use crate::tools::Tool;
use anyhow::{anyhow, Result};
use calamine::Reader;
use docx_rs::{
    read_docx, DocumentChild, ParagraphChild, RunChild, TableCellContent, TableChild, TableRowChild,
};
use serde_json::{json, Value};
use std::fs::File;
use std::io::Read;

pub struct DocReaderTool;

fn extract_docx_text(buf: &[u8]) -> Result<String> {
    let docx = read_docx(buf)?;
    let mut text = String::new();
    for child in &docx.document.children {
        extract_document_child(child, &mut text);
    }
    Ok(text)
}

fn extract_document_child(child: &DocumentChild, text: &mut String) {
    match child {
        DocumentChild::Paragraph(p) => {
            extract_paragraph(p, text);
        }
        DocumentChild::Table(t) => {
            extract_table(t, text, 0);
        }
        _ => {}
    }
}

fn extract_paragraph(p: &docx_rs::Paragraph, text: &mut String) {
    for p_child in &p.children {
        if let ParagraphChild::Run(r) = p_child {
            for r_child in &r.children {
                if let RunChild::Text(t) = r_child {
                    text.push_str(&t.text);
                }
            }
        }
    }
    text.push('\n');
}

fn extract_paragraph_inline(p: &docx_rs::Paragraph, text: &mut String) {
    for p_child in &p.children {
        if let ParagraphChild::Run(r) = p_child {
            for r_child in &r.children {
                if let RunChild::Text(t) = r_child {
                    text.push_str(&t.text);
                }
            }
        }
    }
}

fn extract_table(t: &docx_rs::Table, text: &mut String, depth: usize) {
    // Guard against deeply nested tables causing stack overflow
    const MAX_TABLE_DEPTH: usize = 20;
    if depth > MAX_TABLE_DEPTH {
        text.push_str("[...nested table truncated...]\n");
        return;
    }
    for row_child in &t.rows {
        match row_child {
            TableChild::TableRow(tr) => {
                for cell_child in &tr.cells {
                    match cell_child {
                        TableRowChild::TableCell(tc) => {
                            for content in &tc.children {
                                match content {
                                    TableCellContent::Paragraph(p) => {
                                        extract_paragraph_inline(p, text);
                                    }
                                    TableCellContent::Table(nested_t) => {
                                        extract_table(nested_t, text, depth + 1);
                                    }
                                    _ => {}
                                }
                            }
                            text.push('\t');
                        }
                    }
                }
                text.push('\n');
            }
        }
    }
}

fn document_text_needs_ocr(text: &str) -> bool {
    text.chars().filter(|ch| !ch.is_whitespace()).count() < 24
}

fn is_ocr_supported_extension(extension: Option<&str>) -> bool {
    matches!(
        extension.map(|ext| ext.to_ascii_lowercase()).as_deref(),
        Some("pdf" | "png" | "jpg" | "jpeg" | "bmp" | "tiff" | "tif")
    )
}

fn is_image_ocr_extension(extension: Option<&str>) -> bool {
    matches!(
        extension.map(|ext| ext.to_ascii_lowercase()).as_deref(),
        Some("png" | "jpg" | "jpeg" | "bmp" | "tiff" | "tif")
    )
}

fn ocr_text_from_result(result: &Value) -> Option<String> {
    if result.get("success").and_then(|value| value.as_bool()) != Some(true) {
        return None;
    }
    result
        .get("text")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(ToString::to_string)
}

fn run_opendoc_ocr(path: &std::path::Path, language: Option<&str>) -> Value {
    let response = crate::tools::opendoc::get_server().ocr_document(
        path.to_string_lossy().to_string(),
        language.map(ToString::to_string),
    );
    serde_json::from_str(&response).unwrap_or_else(|_| {
        json!({
            "success": false,
            "error": "Failed to parse OCR response",
            "raw": response
        })
    })
}

fn should_analyze_document_complexity(extension: Option<&str>, arguments: &Value) -> bool {
    let explicit = match arguments {
        Value::Object(map) => map
            .get("analyze_complexity")
            .or_else(|| map.get("analyzeComplexity"))
            .and_then(|value| match value {
                Value::Bool(b) => Some(*b),
                Value::String(s) => Some(s.eq_ignore_ascii_case("true") || s == "1"),
                Value::Number(n) => Some(n.as_i64() == Some(1)),
                _ => None,
            }),
        _ => None,
    };

    if let Some(enabled) = explicit {
        return enabled;
    }

    matches!(
        extension.map(|ext| ext.to_ascii_lowercase()).as_deref(),
        Some("pdf")
    )
}

fn run_opendoc_complexity_analysis(path: &std::path::Path) -> Value {
    let response = crate::tools::opendoc::get_server()
        .analyze_document_complexity(path.to_string_lossy().to_string());
    serde_json::from_str(&response).unwrap_or_else(|_| {
        json!({
            "success": false,
            "error": "Failed to parse document complexity response",
            "raw": response
        })
    })
}

#[async_trait::async_trait]
impl Tool for DocReaderTool {
    fn name(&self) -> &str {
        "read_doc"
    }

    fn description(&self) -> &str {
        "Read contents of a document file (PDF, Excel, DOCX Word document) and return its text content."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the document file (e.g. .pdf, .xlsx, .xls, .ods, .docx, .png, .jpg)."
                },
                "auto_ocr": {
                    "type": "boolean",
                    "description": "Automatically run OCR for scanned PDFs or image files when native text extraction is empty (default: true)."
                },
                "ocr_language": {
                    "type": "string",
                    "description": "Optional OCR language code for Tesseract, e.g. eng."
                },
                "analyze_complexity": {
                    "type": "boolean",
                    "description": "Automatically run document complexity analysis before PDF extraction to guide OCR/chunk/render decisions (default: true for PDFs)."
                }
            },
            "required": ["path"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let path_str = match arguments {
            Value::String(s) => s.trim().to_string(),
            Value::Object(map) => map
                .get("path")
                .or_else(|| map.get("file_path"))
                .or_else(|| map.get("filePath"))
                .or_else(|| map.get("file"))
                .or_else(|| map.get("document"))
                .or_else(|| map.get("doc"))
                .or_else(|| map.get("target"))
                .or_else(|| map.get("uri"))
                .or_else(|| map.get("url"))
                .and_then(|v| v.as_str())
                .map(|s| s.trim().to_string())
                .ok_or_else(|| anyhow!("Missing 'path' parameter"))?,
            _ => return Err(anyhow!("Missing 'path' parameter")),
        };

        let clean_path = path_str.strip_prefix("file://").unwrap_or(&path_str);
        let mut resolved_path = crate::config::loader::resolve_path(clean_path);
        if !resolved_path.exists() && resolved_path.extension().is_none() {
            for ext in &["pdf", "docx", "xlsx", "xls", "ods"] {
                let with_ext = resolved_path.with_extension(ext);
                if with_ext.exists() {
                    resolved_path = with_ext;
                    break;
                }
            }
        }
        if !resolved_path.exists() {
            return Err(anyhow!("File does not exist: {}", path_str));
        }

        // Guard against oversized files (50 MB limit)
        let metadata = std::fs::metadata(&resolved_path)?;
        const MAX_DOC_SIZE: u64 = 50 * 1024 * 1024;
        if metadata.len() > MAX_DOC_SIZE {
            return Err(anyhow!(
                "Document file too large ({} bytes, max {} bytes)",
                metadata.len(),
                MAX_DOC_SIZE
            ));
        }

        let extension = resolved_path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_lowercase());
        let auto_ocr = match arguments {
            Value::Object(map) => map
                .get("auto_ocr")
                .or_else(|| map.get("autoOcr"))
                .and_then(|v| match v {
                    Value::Bool(b) => Some(*b),
                    Value::String(s) => Some(s.eq_ignore_ascii_case("true") || s == "1"),
                    Value::Number(n) => Some(n.as_i64() == Some(1)),
                    _ => None,
                })
                .unwrap_or(true),
            _ => true,
        };
        let ocr_language = match arguments {
            Value::Object(map) => map
                .get("ocr_language")
                .or_else(|| map.get("ocrLanguage"))
                .or_else(|| map.get("language"))
                .or_else(|| map.get("lang"))
                .and_then(|value| value.as_str()),
            _ => None,
        };

        let mut complexity_analyzed = false;
        let mut complexity_result: Option<Value> = None;
        if should_analyze_document_complexity(extension.as_deref(), arguments) {
            complexity_analyzed = true;
            complexity_result = Some(run_opendoc_complexity_analysis(&resolved_path));
        }

        let mut extraction_method = "native";
        let mut ocr_attempted = false;
        let mut ocr_result: Option<Value> = None;

        let mut content = match extension.as_deref() {
            Some("pdf") => match pdf_extract::extract_text(&resolved_path) {
                Ok(text) => text,
                Err(err) if auto_ocr => {
                    ocr_result = Some(json!({
                        "native_extract_error": err.to_string()
                    }));
                    String::new()
                }
                Err(err) => return Err(err.into()),
            },
            Some("xlsx") | Some("xls") | Some("ods") => {
                let mut sheets = calamine::open_workbook_auto(&resolved_path)?;
                let mut text = String::new();
                for sheet_name in sheets.sheet_names().to_owned() {
                    if let Ok(range) = sheets.worksheet_range(&sheet_name) {
                        text.push_str(&format!("--- Sheet: {} ---\n", sheet_name));
                        for row in range.rows() {
                            let row_strs: Vec<String> =
                                row.iter().map(|cell| cell.to_string()).collect();
                            text.push_str(&row_strs.join("\t"));
                            text.push('\n');
                        }
                        text.push('\n');
                    }
                }
                text
            }
            Some("docx") => {
                let mut file = File::open(&resolved_path)?;
                let mut buf = Vec::new();
                file.read_to_end(&mut buf)?;
                extract_docx_text(&buf)?
            }
            ext if auto_ocr && is_image_ocr_extension(ext) => String::new(),
            _ => {
                return Err(anyhow!(
                    "Unsupported file extension. Supported formats: .pdf, .xlsx, .xls, .ods, .docx, .png, .jpg, .jpeg, .bmp, .tiff"
                ));
            }
        };

        if auto_ocr
            && is_ocr_supported_extension(extension.as_deref())
            && (is_image_ocr_extension(extension.as_deref()) || document_text_needs_ocr(&content))
        {
            ocr_attempted = true;
            let ocr = run_opendoc_ocr(&resolved_path, ocr_language);
            if let Some(ocr_text) = ocr_text_from_result(&ocr) {
                content = ocr_text;
                extraction_method = "ocr";
            }
            ocr_result = Some(match ocr_result.take() {
                Some(mut prior) => {
                    if let Some(obj) = prior.as_object_mut() {
                        obj.insert("ocr".to_string(), ocr);
                        prior
                    } else {
                        ocr
                    }
                }
                None => ocr,
            });
        }

        let _ = crate::tools::shared_memory::archive_research_entry(
            &path_str,
            &content,
            &format!("doc_reader: {}", path_str),
        )
        .await;

        Ok(json!({
            "status": if ocr_attempted && document_text_needs_ocr(&content) { "partial_success" } else { "success" },
            "content": content,
            "extraction_method": extraction_method,
            "complexity_analyzed": complexity_analyzed,
            "complexity_result": complexity_result,
            "ocr_attempted": ocr_attempted,
            "ocr_result": ocr_result
        }))
    }
}

#[cfg(test)]
#[path = "doc_reader_tests.rs"]
mod tests;
