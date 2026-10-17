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
    extract_paragraph_inline(p, text);
    text.push('\n');
}

fn extract_paragraph_inline(p: &docx_rs::Paragraph, text: &mut String) {
    for p_child in &p.children {
        match p_child {
            ParagraphChild::Run(r) => {
                for r_child in &r.children {
                    match r_child {
                        RunChild::Text(t) => text.push_str(&t.text),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
}

fn extract_table(t: &docx_rs::Table, text: &mut String, depth: usize) {
    const MAX_TABLE_DEPTH: usize = 20;
    match depth > MAX_TABLE_DEPTH {
        true => {
            text.push_str("[...nested table truncated...]\n");
            return;
        }
        false => {}
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
    let is_ok = result.get("success").and_then(|value| value.as_bool()) == Some(true);
    is_ok
        .then(|| {
            result
                .get("text")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .map(ToString::to_string)
        })
        .flatten()
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

    explicit.unwrap_or_else(|| {
        matches!(
            extension.map(|ext| ext.to_ascii_lowercase()).as_deref(),
            Some("pdf")
        )
    })
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
        "Read contents of a document file (PDF, DOCX, Excel, PPTX, CSV, HTML, Markdown, Plain Text) and return its text content."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the document file (e.g. .pdf, .docx, .xlsx, .pptx, .csv, .html, .md, .txt, .png, .jpg)."
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
        let base_path = crate::config::loader::resolve_path(clean_path);
        let resolved_path = match !base_path.exists() && base_path.extension().is_none() {
            true => [
                "pdf", "docx", "xlsx", "xls", "ods", "xlsb", "pptx", "ppt", "csv", "html", "md",
                "txt",
            ]
            .iter()
            .map(|ext| base_path.with_extension(ext))
            .find(|candidate| candidate.exists())
            .unwrap_or(base_path),
            false => base_path,
        };
        match resolved_path.exists() {
            true => {}
            false => return Err(anyhow!("File does not exist: {}", path_str)),
        }

        let metadata = std::fs::metadata(&resolved_path)?;
        const MAX_DOC_SIZE: u64 = 50 * 1024 * 1024;
        match metadata.len() > MAX_DOC_SIZE {
            true => {
                return Err(anyhow!(
                    "Document file too large ({} bytes, max {} bytes)",
                    metadata.len(),
                    MAX_DOC_SIZE
                ));
            }
            false => {}
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

        let complexity_analyzed =
            should_analyze_document_complexity(extension.as_deref(), arguments);
        let complexity_result = complexity_analyzed
            .then(|| run_opendoc_complexity_analysis(&resolved_path));

        let mut extraction_method = "native";
        let mut ocr_attempted = false;
        let mut ocr_result: Option<Value> = None;

        let mut content = match extension.as_deref() {
            Some("pdf") => match pdf_extract::extract_text(&resolved_path) {
                Ok(text) => text,
                Err(err) => match auto_ocr {
                    true => {
                        ocr_result = Some(json!({
                            "native_extract_error": err.to_string()
                        }));
                        String::new()
                    }
                    false => return Err(err.into()),
                },
            },
            Some("xlsx") | Some("xls") | Some("ods") | Some("xlsb") => {
                let mut sheets = calamine::open_workbook_auto(&resolved_path)?;
                let mut text = String::new();
                for sheet_name in sheets.sheet_names().to_owned() {
                    match sheets.worksheet_range(&sheet_name) {
                        Ok(range) => {
                            text.push_str(&format!("--- Sheet: {} ---\n", sheet_name));
                            for row in range.rows() {
                                let row_strs: Vec<String> =
                                    row.iter().map(|cell| cell.to_string()).collect();
                                text.push_str(&row_strs.join("\t"));
                                text.push('\n');
                            }
                            text.push('\n');
                        }
                        Err(_) => {}
                    }
                }
                text
            }
            Some("docx") => {
                let mut file = File::open(&resolved_path)?;
                let mut buf = Vec::new();
                file.read_to_end(&mut buf)?;
                match extract_docx_text(&buf).ok().filter(|t| !t.trim().is_empty()) {
                    Some(text) => text,
                    None => {
                        let resp = crate::tools::opendoc::get_server().read_document_text(
                            resolved_path.to_string_lossy().to_string(),
                            None,
                        );
                        serde_json::from_str::<Value>(&resp)
                            .ok()
                            .and_then(|v| {
                                v.get("text")
                                    .and_then(|t| t.as_str())
                                    .map(ToString::to_string)
                            })
                            .unwrap_or_default()
                    }
                }
            }
            Some("pptx" | "ppt" | "csv" | "html" | "htm" | "md" | "markdown" | "txt" | "text") => {
                let resp = crate::tools::opendoc::get_server().read_document_text(
                    resolved_path.to_string_lossy().to_string(),
                    None,
                );
                match serde_json::from_str::<Value>(&resp) {
                    Ok(v) => match (
                        v.get("text").and_then(|t| t.as_str()),
                        v.get("error").and_then(|e| e.as_str()),
                    ) {
                        (Some(text), _) => text.to_string(),
                        (None, Some(err)) => return Err(anyhow!("Failed to read document: {err}")),
                        (None, None) => resp,
                    },
                    Err(_) => resp,
                }
            }
            ext => match auto_ocr && is_image_ocr_extension(ext) {
                true => String::new(),
                false => {
                    return Err(anyhow!(
                        "Unsupported file extension. Supported formats: .pdf, .docx, .xlsx, .xls, .ods, .pptx, .ppt, .csv, .html, .md, .txt, .png, .jpg, .jpeg, .bmp, .tiff"
                    ));
                }
            },
        };

        let should_run_ocr = auto_ocr
            && is_ocr_supported_extension(extension.as_deref())
            && (is_image_ocr_extension(extension.as_deref())
                || document_text_needs_ocr(&content));
        match should_run_ocr {
            true => {
                ocr_attempted = true;
                let ocr = run_opendoc_ocr(&resolved_path, ocr_language);
                let _ = ocr_text_from_result(&ocr).map(|ocr_text| {
                    content = ocr_text;
                    extraction_method = "ocr";
                });
                ocr_result = Some(match ocr_result.take() {
                    Some(mut prior) => match prior.as_object_mut() {
                        Some(obj) => {
                            obj.insert("ocr".to_string(), ocr);
                            prior
                        }
                        None => ocr,
                    },
                    None => ocr,
                });
            }
            false => {}
        }

        let _ = crate::tools::shared_memory::archive_research_entry(
            &path_str,
            &content,
            &format!("doc_reader: {}", path_str),
        )
        .await;

        let status = match ocr_attempted && document_text_needs_ocr(&content) {
            true => "partial_success",
            false => "success",
        };

        Ok(json!({
            "status": status,
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
