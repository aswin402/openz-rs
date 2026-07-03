//! Cross-format document conversion.
//!
//! Converts between formats using the IR as intermediary:
//!   DOCX → IR → PDF
//!   PPTX → IR → Markdown
//!   etc.

pub mod transmutation;
pub mod render;

use crate::ir::Document;
use std::path::Path;

/// Convert a document from one format to another
pub fn convert(
    source: &str,
    target_format: &str,
    output: &str,
) -> Result<ConversionResult, ConversionError> {
    convert_with_password(source, target_format, output, None)
}

/// Convert a document from one format to another with an optional password
pub fn convert_with_password(
    source: &str,
    target_format: &str,
    output: &str,
    password: Option<&str>,
) -> Result<ConversionResult, ConversionError> {
    let source_path = Path::new(source);
    if !source_path.exists() {
        return Err(ConversionError::FileNotFound(source.to_string()));
    }

    let ext = source_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    // Route to specific converter based on source + target
    match (ext.as_str(), target_format) {
        // DOCX → anything
        ("docx", "pdf") => docx_to_pdf(source, output),
        ("docx", "md" | "markdown") => docx_to_markdown(source, output),
        ("docx", "html") => docx_to_html(source, output),

        // PPTX → anything
        ("pptx", "md" | "markdown") => pptx_to_markdown(source, output),
        ("pptx", "pdf") => pptx_to_pdf(source, output),

        // PDF → anything
        ("pdf", "txt" | "text") => pdf_to_text_with_password(source, output, password),
        ("pdf", "md" | "markdown") => pdf_to_markdown_with_password(source, output, password),

        // XLSX → anything
        ("xlsx", "csv") => xlsx_to_csv(source, output),

        // Generic: load as IR → export
        _ => {
            let doc = crate::handlers::load_to_ir_with_password(source, password)
                .map_err(|e| ConversionError::ConversionFailed(e.to_string()))?;
            export(&doc, target_format, output)
        }
    }
}

fn docx_to_pdf(source: &str, output: &str) -> Result<ConversionResult, ConversionError> {
    let doc = rdocx::Document::open(source)
        .map_err(|e| ConversionError::ConversionFailed(e.to_string()))?;
    let pdf_bytes = doc
        .to_pdf()
        .map_err(|e| ConversionError::ConversionFailed(e.to_string()))?;
    std::fs::write(output, &pdf_bytes)
        .map_err(|e| ConversionError::IoError(e.to_string()))?;
    Ok(ConversionResult {
        source: source.to_string(),
        output: output.to_string(),
        source_format: "docx".to_string(),
        target_format: "pdf".to_string(),
        size_bytes: pdf_bytes.len(),
    })
}

fn docx_to_markdown(source: &str, output: &str) -> Result<ConversionResult, ConversionError> {
    let doc = rdocx::Document::open(source)
        .map_err(|e| ConversionError::ConversionFailed(e.to_string()))?;
    let md = doc.to_markdown();
    std::fs::write(output, &md)
        .map_err(|e| ConversionError::IoError(e.to_string()))?;
    Ok(ConversionResult {
        source: source.to_string(),
        output: output.to_string(),
        source_format: "docx".to_string(),
        target_format: "markdown".to_string(),
        size_bytes: md.len(),
    })
}

fn docx_to_html(source: &str, output: &str) -> Result<ConversionResult, ConversionError> {
    let doc = rdocx::Document::open(source)
        .map_err(|e| ConversionError::ConversionFailed(e.to_string()))?;
    let html = doc.to_html();
    std::fs::write(output, &html)
        .map_err(|e| ConversionError::IoError(e.to_string()))?;
    Ok(ConversionResult {
        source: source.to_string(),
        output: output.to_string(),
        source_format: "docx".to_string(),
        target_format: "html".to_string(),
        size_bytes: html.len(),
    })
}

fn pptx_to_markdown(source: &str, output: &str) -> Result<ConversionResult, ConversionError> {
    let md = crate::handlers::pptx::to_markdown(source);
    let json: serde_json::Value = serde_json::from_str(&md)
        .map_err(|e| ConversionError::ConversionFailed(e.to_string()))?;
    
    if let Some(err) = json.get("error") {
        return Err(ConversionError::ConversionFailed(err.to_string()));
    }
    
    let md_text = json.get("markdown")
        .and_then(|v| v.as_str())
        .unwrap_or("");
        
    std::fs::write(output, md_text)
        .map_err(|e| ConversionError::IoError(e.to_string()))?;
        
    Ok(ConversionResult {
        source: source.to_string(),
        output: output.to_string(),
        source_format: "pptx".to_string(),
        target_format: "markdown".to_string(),
        size_bytes: md_text.len(),
    })
}

/// Convert PPTX to PDF by extracting text and creating a PDF page per slide
fn pptx_to_pdf(source: &str, output: &str) -> Result<ConversionResult, ConversionError> {
    // Load PPTX via the pptx handler
    let doc = crate::handlers::pptx::to_ir(source)
        .map_err(|e| ConversionError::ConversionFailed(e.to_string()))?;
    
    let text: Vec<&str> = doc.paragraphs.iter().map(|p| p.text.as_str()).collect();
    
    // Create PDF using lopdf (one page per ~40 lines of content)
    let mut pdf = lopdf::Document::new();
    
    let font_id = pdf.add_object(lopdf::Object::Dictionary(lopdf::Dictionary::from_iter([
        (b"Type".to_vec(), lopdf::Object::Name(b"Font".to_vec())),
        (b"Subtype".to_vec(), lopdf::Object::Name(b"Type1".to_vec())),
        (b"BaseFont".to_vec(), lopdf::Object::Name(b"Helvetica".to_vec())),
    ])));
    
    let mut page_ids = Vec::new();
    let pages_id = pdf.new_object_id();
    let lines: Vec<String> = text.iter().map(|s| {
        s.replace('\\', "\\\\")
            .replace('(', "\\(")
            .replace(')', "\\)")
    }).collect();
    
    let chunks = lines.chunks(40);
    for chunk in chunks {
        let mut content_parts = vec!["BT /F1 12 Tf 50 700 Td".to_string()];
        for (i, line) in chunk.iter().enumerate() {
            if i > 0 {
                content_parts.push(format!("0 -15 Td ({}) Tj", line));
            } else {
                content_parts.push(format!("({}) Tj", line));
            }
        }
        content_parts.push("ET".to_string());
        let content_str = content_parts.join(" ");
        
        let content_id = pdf.add_object(lopdf::Object::Stream(lopdf::Stream {
            dict: lopdf::Dictionary::new(),
            content: content_str.into_bytes(),
            allows_compression: true,
            start_position: None,
        }));
        
        let page_id = pdf.new_object_id();
        let page = lopdf::Object::Dictionary(lopdf::Dictionary::from_iter([
            (b"Type".to_vec(), lopdf::Object::Name(b"Page".to_vec())),
            (b"Parent".to_vec(), lopdf::Object::Reference(pages_id)),
            (b"Contents".to_vec(), lopdf::Object::Reference(content_id)),
            (b"Resources".to_vec(), lopdf::Object::Dictionary(lopdf::Dictionary::from_iter([
                (b"Font".to_vec(), lopdf::Object::Dictionary(lopdf::Dictionary::from_iter([
                    (b"F1".to_vec(), lopdf::Object::Reference(font_id)),
                ]))),
            ]))),
            (b"MediaBox".to_vec(), lopdf::Object::Array(vec![
                lopdf::Object::Integer(0), lopdf::Object::Integer(0),
                lopdf::Object::Integer(612), lopdf::Object::Integer(792),
            ])),
        ]));
        pdf.objects.insert(page_id, page);
        page_ids.push(page_id);
    }
    
    if page_ids.is_empty() {
        let content_id = pdf.add_object(lopdf::Object::Stream(lopdf::Stream {
            dict: lopdf::Dictionary::new(),
            content: b"BT /F1 12 Tf 50 700 Td (Empty slide) Tj ET".to_vec(),
            allows_compression: true,
            start_position: None,
        }));
        let page_id = pdf.new_object_id();
        let page = lopdf::Object::Dictionary(lopdf::Dictionary::from_iter([
            (b"Type".to_vec(), lopdf::Object::Name(b"Page".to_vec())),
            (b"Parent".to_vec(), lopdf::Object::Reference(pages_id)),
            (b"Contents".to_vec(), lopdf::Object::Reference(content_id)),
            (b"Resources".to_vec(), lopdf::Object::Dictionary(lopdf::Dictionary::from_iter([
                (b"Font".to_vec(), lopdf::Object::Dictionary(lopdf::Dictionary::from_iter([
                    (b"F1".to_vec(), lopdf::Object::Reference(font_id)),
                ]))),
            ]))),
            (b"MediaBox".to_vec(), lopdf::Object::Array(vec![
                lopdf::Object::Integer(0), lopdf::Object::Integer(0),
                lopdf::Object::Integer(612), lopdf::Object::Integer(792),
            ])),
        ]));
        pdf.objects.insert(page_id, page);
        page_ids.push(page_id);
    }
    
    let pages = lopdf::Object::Dictionary(lopdf::Dictionary::from_iter([
        (b"Type".to_vec(), lopdf::Object::Name(b"Pages".to_vec())),
        (b"Kids".to_vec(), lopdf::Object::Array(
            page_ids.iter().map(|id| lopdf::Object::Reference(*id)).collect()
        )),
        (b"Count".to_vec(), lopdf::Object::Integer(page_ids.len() as i64)),
    ]));
    pdf.objects.insert(pages_id, pages);
    
    let catalog_id = pdf.new_object_id();
    let catalog = lopdf::Object::Dictionary(lopdf::Dictionary::from_iter([
        (b"Type".to_vec(), lopdf::Object::Name(b"Catalog".to_vec())),
        (b"Pages".to_vec(), lopdf::Object::Reference(pages_id)),
    ]));
    pdf.objects.insert(catalog_id, catalog);
    pdf.trailer.set("Root", lopdf::Object::Reference(catalog_id));
    pdf.max_id = catalog_id.0;
    
    pdf.save(output)
        .map_err(|e| ConversionError::ConversionFailed(e.to_string()))?;
    
    let size = std::fs::metadata(output)
        .map(|m| m.len() as usize)
        .unwrap_or(0);
    
    Ok(ConversionResult {
        source: source.to_string(),
        output: output.to_string(),
        source_format: "pptx".to_string(),
        target_format: "pdf".to_string(),
        size_bytes: size,
    })
}


fn pdf_to_text_with_password(source: &str, output: &str, password: Option<&str>) -> Result<ConversionResult, ConversionError> {
    let mut doc = lopdf::Document::load(source)
        .map_err(|e| ConversionError::ConversionFailed(e.to_string()))?;
    if doc.is_encrypted() {
        let pass = password.unwrap_or("");
        doc.decrypt(pass.as_bytes())
            .map_err(|e| ConversionError::ConversionFailed(format!("Failed to decrypt PDF: {}", e)))?;
    }
    let pages: Vec<u32> = doc.get_pages().keys().copied().collect();
    let text = doc
        .extract_text(&pages)
        .map_err(|e| ConversionError::ConversionFailed(e.to_string()))?;
    std::fs::write(output, &text)
        .map_err(|e| ConversionError::IoError(e.to_string()))?;
    Ok(ConversionResult {
        source: source.to_string(),
        output: output.to_string(),
        source_format: "pdf".to_string(),
        target_format: "text".to_string(),
        size_bytes: text.len(),
    })
}


fn pdf_to_markdown_with_password(source: &str, output: &str, password: Option<&str>) -> Result<ConversionResult, ConversionError> {
    let mut doc = lopdf::Document::load(source)
        .map_err(|e| ConversionError::ConversionFailed(e.to_string()))?;
    if doc.is_encrypted() {
        let pass = password.unwrap_or("");
        doc.decrypt(pass.as_bytes())
            .map_err(|e| ConversionError::ConversionFailed(format!("Failed to decrypt PDF: {}", e)))?;
    }
    let pages: Vec<u32> = doc.get_pages().keys().copied().collect();
    let text = doc
        .extract_text(&pages)
        .map_err(|e| ConversionError::ConversionFailed(e.to_string()))?;
    let md = format!("# Extracted PDF\n\n{}", text);
    std::fs::write(output, &md)
        .map_err(|e| ConversionError::IoError(e.to_string()))?;
    Ok(ConversionResult {
        source: source.to_string(),
        output: output.to_string(),
        source_format: "pdf".to_string(),
        target_format: "markdown".to_string(),
        size_bytes: md.len(),
    })
}

fn xlsx_to_csv(source: &str, output: &str) -> Result<ConversionResult, ConversionError> {
    use calamine::{open_workbook, Reader, Xlsx};
    use std::fs::File;
    use std::io::BufReader;
    let mut workbook: Xlsx<BufReader<File>> = open_workbook(source)
        .map_err(|e: calamine::XlsxError| ConversionError::ConversionFailed(e.to_string()))?;

    let mut csv_output = String::new();
    if let Some(Ok(range)) = workbook.worksheet_range_at(0) {
        for row in range.rows() {
            let line: Vec<String> = row
                .iter()
                .map(|cell| match cell {
                    calamine::Data::String(s) => s.clone(),
                    calamine::Data::Float(f) => f.to_string(),
                    calamine::Data::Int(i) => i.to_string(),
                    calamine::Data::Bool(b) => b.to_string(),
                    _ => String::new(),
                })
                .collect();
            csv_output.push_str(&line.join(","));
            csv_output.push('\n');
        }
    }

    std::fs::write(output, &csv_output)
        .map_err(|e| ConversionError::IoError(e.to_string()))?;
    Ok(ConversionResult {
        source: source.to_string(),
        output: output.to_string(),
        source_format: "xlsx".to_string(),
        target_format: "csv".to_string(),
        size_bytes: csv_output.len(),
    })
}

fn escape_html(s: &str) -> String {
    let mut escaped = String::new();
    for c in s.chars() {
        match c {
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '&' => escaped.push_str("&amp;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(c),
        }
    }
    escaped
}

fn escape_csv(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        let escaped = s.replace('"', "\"\"");
        format!("\"{}\"", escaped)
    } else {
        s.to_string()
    }
}

/// Export a parsed IR document to a target format.
/// Supported formats: json, txt, md, html, csv, xlsx, docx.
pub fn export(doc: &Document, target_format: &str, output: &str) -> Result<ConversionResult, ConversionError> {
    match target_format {
        "json" => {
            let json = serde_json::to_string_pretty(doc)
                .map_err(|e| ConversionError::ConversionFailed(e.to_string()))?;
            std::fs::write(output, &json)
                .map_err(|e| ConversionError::IoError(e.to_string()))?;
            Ok(ConversionResult {
                source: doc.path.clone().unwrap_or_default(),
                output: output.to_string(),
                source_format: doc.format.clone(),
                target_format: "json".to_string(),
                size_bytes: json.len(),
            })
        }
        "txt" | "text" => {
            let content = if let Some(ref raw) = doc.text {
                raw.clone()
            } else {
                let p_texts: Vec<String> = doc.paragraphs.iter().map(|p| p.text.clone()).collect();
                p_texts.join("\n")
            };
            std::fs::write(output, &content)
                .map_err(|e| ConversionError::IoError(e.to_string()))?;
            Ok(ConversionResult {
                source: doc.path.clone().unwrap_or_default(),
                output: output.to_string(),
                source_format: doc.format.clone(),
                target_format: target_format.to_string(),
                size_bytes: content.len(),
            })
        }
        "md" | "markdown" => {
            let md = doc.to_markdown();
            std::fs::write(output, &md)
                .map_err(|e| ConversionError::IoError(e.to_string()))?;
            Ok(ConversionResult {
                source: doc.path.clone().unwrap_or_default(),
                output: output.to_string(),
                source_format: doc.format.clone(),
                target_format: target_format.to_string(),
                size_bytes: md.len(),
            })
        }
        "html" => {
            let mut html = String::new();
            html.push_str("<!DOCTYPE html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n</head>\n<body>\n");
            if !doc.paragraphs.is_empty() || !doc.tables.is_empty() {
                for p in &doc.paragraphs {
                    if p.is_heading {
                        let tag = format!("h{}", p.heading_level.clamp(1, 6));
                        html.push_str(&format!("<{}>{}</{}>\n", tag, escape_html(&p.text), tag));
                    } else {
                        html.push_str(&format!("<p>{}</p>\n", escape_html(&p.text)));
                    }
                }
                for table in &doc.tables {
                    html.push_str("<table>\n");
                    if let Some(ref cap) = table.caption {
                        html.push_str(&format!("<caption>{}</caption>\n", escape_html(cap)));
                    }
                    if !table.headers.is_empty() {
                        html.push_str("  <thead>\n    <tr>\n");
                        for h in &table.headers {
                            html.push_str(&format!("      <th>{}</th>\n", escape_html(h)));
                        }
                        html.push_str("    </tr>\n  </thead>\n");
                    }
                    if !table.rows.is_empty() {
                        html.push_str("  <tbody>\n");
                        for row in &table.rows {
                            html.push_str("    <tr>\n");
                            for cell in row {
                                html.push_str(&format!("      <td>{}</td>\n", escape_html(cell)));
                            }
                            html.push_str("    </tr>\n");
                        }
                        html.push_str("  </tbody>\n");
                    }
                    html.push_str("</table>\n");
                }
            } else if let Some(ref raw) = doc.text {
                html.push_str(&format!("<pre>{}</pre>\n", escape_html(raw)));
            }
            html.push_str("</body>\n</html>");
            std::fs::write(output, &html)
                .map_err(|e| ConversionError::IoError(e.to_string()))?;
            Ok(ConversionResult {
                source: doc.path.clone().unwrap_or_default(),
                output: output.to_string(),
                source_format: doc.format.clone(),
                target_format: target_format.to_string(),
                size_bytes: html.len(),
            })
        }
        "csv" => {
            let mut csv_str = String::new();
            if !doc.tables.is_empty() {
                let table = &doc.tables[0];
                if !table.headers.is_empty() {
                    let escaped_hdrs: Vec<String> = table.headers.iter().map(|h| escape_csv(h)).collect();
                    csv_str.push_str(&escaped_hdrs.join(","));
                    csv_str.push('\n');
                }
                for row in &table.rows {
                    let escaped_cells: Vec<String> = row.iter().map(|c| escape_csv(c)).collect();
                    csv_str.push_str(&escaped_cells.join(","));
                    csv_str.push('\n');
                }
            } else {
                for p in &doc.paragraphs {
                    csv_str.push_str(&escape_csv(&p.text));
                    csv_str.push('\n');
                }
            }
            std::fs::write(output, &csv_str)
                .map_err(|e| ConversionError::IoError(e.to_string()))?;
            Ok(ConversionResult {
                source: doc.path.clone().unwrap_or_default(),
                output: output.to_string(),
                source_format: doc.format.clone(),
                target_format: target_format.to_string(),
                size_bytes: csv_str.len(),
            })
        }
        "xlsx" => {
            crate::handlers::xlsx::from_ir(doc, output)
                .map_err(ConversionError::ConversionFailed)?;
            let size = std::fs::metadata(output)
                .map(|m| m.len() as usize)
                .unwrap_or(0);
            Ok(ConversionResult {
                source: doc.path.clone().unwrap_or_default(),
                output: output.to_string(),
                source_format: doc.format.clone(),
                target_format: "xlsx".to_string(),
                size_bytes: size,
            })
        }
        "docx" => {
            let mut docx_doc = rdocx::Document::new();
            if let Some(ref raw) = doc.text {
                for line in raw.lines() {
                    let mut p = docx_doc.add_paragraph("");
                    p.add_run(line);
                }
            } else {
                for p in &doc.paragraphs {
                    let mut docx_p = docx_doc.add_paragraph("");
                    let mut run = docx_p.add_run(&p.text);
                    if p.is_heading {
                        run = run.bold(true).size(24.0);
                    } else {
                        if p.bold { run = run.bold(true); }
                        if p.italic { run = run.italic(true); }
                    }
                }
                for table in &doc.tables {
                    let rows = table.rows.len() + if table.headers.is_empty() { 0 } else { 1 };
                    let cols = if !table.headers.is_empty() {
                        table.headers.len()
                    } else if !table.rows.is_empty() {
                        table.rows[0].len()
                    } else {
                        0
                    };
                    if rows > 0 && cols > 0 {
                        let mut docx_table = docx_doc.add_table(rows, cols);
                        let mut row_offset = 0;
                        if !table.headers.is_empty() {
                            for (c_idx, h) in table.headers.iter().enumerate() {
                                if let Some(mut cell) = docx_table.cell(0, c_idx) {
                                    cell.set_text(h);
                                }
                            }
                            row_offset = 1;
                        }
                        for (r_idx, row) in table.rows.iter().enumerate() {
                            for (c_idx, cell_val) in row.iter().enumerate() {
                                if c_idx < cols {
                                    if let Some(mut cell) = docx_table.cell(r_idx + row_offset, c_idx) {
                                        cell.set_text(cell_val);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            docx_doc.save(output)
                .map_err(|e| ConversionError::ConversionFailed(e.to_string()))?;
            let size = std::fs::metadata(output)
                .map(|m| m.len() as usize)
                .unwrap_or(0);
            Ok(ConversionResult {
                source: doc.path.clone().unwrap_or_default(),
                output: output.to_string(),
                source_format: doc.format.clone(),
                target_format: target_format.to_string(),
                size_bytes: size,
            })
        }
        _ => Err(ConversionError::UnsupportedConversion(
            doc.format.clone(),
            target_format.to_string(),
        )),
    }
}

/// Result of a successful document conversion.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ConversionResult {
    pub source: String,
    pub output: String,
    pub source_format: String,
    pub target_format: String,
    pub size_bytes: usize,
}

/// Errors that can occur during document conversion.
#[derive(Debug, thiserror::Error)]
pub enum ConversionError {
    #[error("File not found: {0}")]
    FileNotFound(String),
    #[error("Conversion failed: {0}")]
    ConversionFailed(String),
    #[error("IO error: {0}")]
    IoError(String),
    #[error("Unsupported conversion: {0} → {1}")]
    UnsupportedConversion(String, String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{Document as IrDocument, Paragraph as IrParagraph, Table as IrTable, Section as IrSection};
    use std::fs;

    #[test]
    fn test_convert_file_not_found() {
        let res = convert("/nonexistent/file/path.docx", "pdf", "/tmp/out.pdf");
        assert!(matches!(res, Err(ConversionError::FileNotFound(_))));
    }

    #[test]
    fn test_unsupported_conversion() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_unsupported.txt");
        fs::write(&path, "content").unwrap();
        
        let res = convert(path.to_str().unwrap(), "unsupported_format", "/tmp/out.pdf");
        assert!(res.is_err());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_export_formats() {
        let dir = std::env::temp_dir();
        let mut doc = IrDocument::new("txt");
        doc.path = Some(dir.join("source_export.txt").to_str().unwrap().to_string());
        
        let p_text = IrParagraph::new("Hello paragraph");
        doc.paragraphs.push(p_text);

        let mut p_heading = IrParagraph::new("My Section");
        p_heading.is_heading = true;
        p_heading.heading_level = 2;
        doc.paragraphs.push(p_heading);

        doc.sections.push(IrSection {
            title: "My Section".to_string(),
            level: 2,
            index: 0,
            content: Vec::new(),
        });
        doc.tables.push(IrTable::new(
            vec!["Header1".to_string(), "Header2".to_string()],
            vec![vec!["Value1".to_string(), "Value2".to_string()]]
        ));

        // Test export to JSON
        let path_json = dir.join("out_export.json");
        let res = export(&doc, "json", path_json.to_str().unwrap());
        assert!(res.is_ok());
        let content = fs::read_to_string(&path_json).unwrap();
        assert!(content.contains("Hello paragraph"));
        assert!(content.contains("Header1"));
        let _ = fs::remove_file(path_json);

        // Test export to TXT
        let path_txt = dir.join("out_export.txt");
        let res = export(&doc, "txt", path_txt.to_str().unwrap());
        assert!(res.is_ok());
        let content = fs::read_to_string(&path_txt).unwrap();
        assert!(content.contains("Hello paragraph"));
        let _ = fs::remove_file(path_txt);

        // Test export to Markdown
        let path_md = dir.join("out_export.md");
        let res = export(&doc, "md", path_md.to_str().unwrap());
        assert!(res.is_ok());
        let content = fs::read_to_string(&path_md).unwrap();
        assert!(content.contains("# My Section"));
        assert!(content.contains("| Header1 | Header2 |"));
        let _ = fs::remove_file(path_md);

        // Test export to HTML
        let path_html = dir.join("out_export.html");
        let res = export(&doc, "html", path_html.to_str().unwrap());
        assert!(res.is_ok());
        let content = fs::read_to_string(&path_html).unwrap();
        assert!(content.contains("<h2>My Section</h2>"));
        assert!(content.contains("<th>Header1</th>"));
        let _ = fs::remove_file(path_html);

        // Test export to CSV
        let path_csv = dir.join("out_export.csv");
        let res = export(&doc, "csv", path_csv.to_str().unwrap());
        assert!(res.is_ok());
        let content = fs::read_to_string(&path_csv).unwrap();
        assert!(content.contains("Header1,Header2"));
        assert!(content.contains("Value1,Value2"));
        let _ = fs::remove_file(path_csv);

        // Test export to XLSX
        let path_xlsx = dir.join("out_export.xlsx");
        let res = export(&doc, "xlsx", path_xlsx.to_str().unwrap());
        assert!(res.is_ok());
        assert!(path_xlsx.exists());
        let _ = fs::remove_file(path_xlsx);

        // Test export to DOCX
        let path_docx = dir.join("out_export.docx");
        let res = export(&doc, "docx", path_docx.to_str().unwrap());
        assert!(res.is_ok());
        assert!(path_docx.exists());
        let _ = fs::remove_file(path_docx);
    }
}
