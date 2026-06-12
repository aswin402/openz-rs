use crate::tools::Tool;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use calamine::Reader;
use docx_rs::{
    read_docx, DocumentChild, ParagraphChild, RunChild,
    TableChild, TableRowChild, TableCellContent
};

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
            extract_table(t, text);
        }
        _ => {}
    }
}

fn extract_paragraph(p: &docx_rs::Paragraph, text: &mut String) {
    for p_child in &p.children {
        match p_child {
            ParagraphChild::Run(r) => {
                for r_child in &r.children {
                    match r_child {
                        RunChild::Text(t) => {
                            text.push_str(&t.text);
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    text.push('\n');
}

fn extract_paragraph_inline(p: &docx_rs::Paragraph, text: &mut String) {
    for p_child in &p.children {
        match p_child {
            ParagraphChild::Run(r) => {
                for r_child in &r.children {
                    match r_child {
                        RunChild::Text(t) => {
                            text.push_str(&t.text);
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
}

fn extract_table(t: &docx_rs::Table, text: &mut String) {
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
                                        extract_table(nested_t, text);
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
                    "description": "Path to the document file (e.g. .pdf, .xlsx, .xls, .ods, .docx)."
                }
            },
            "required": ["path"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let path_str = arguments.get("path").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'path' parameter"))?;
        
        let path = Path::new(path_str);
        if !path.exists() {
            return Err(anyhow!("File does not exist: {}", path_str));
        }

        let extension = path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_lowercase());

        let content = match extension.as_deref() {
            Some("pdf") => {
                pdf_extract::extract_text(path)?
            }
            Some("xlsx") | Some("xls") | Some("ods") => {
                let mut sheets = calamine::open_workbook_auto(path)?;
                let mut text = String::new();
                for sheet_name in sheets.sheet_names().to_owned() {
                    if let Ok(range) = sheets.worksheet_range(&sheet_name) {
                        text.push_str(&format!("--- Sheet: {} ---\n", sheet_name));
                        for row in range.rows() {
                            let row_strs: Vec<String> = row.iter()
                                .map(|cell| cell.to_string())
                                .collect();
                            text.push_str(&row_strs.join("\t"));
                            text.push('\n');
                        }
                        text.push('\n');
                    }
                }
                text
            }
            Some("docx") => {
                let mut file = File::open(path)?;
                let mut buf = Vec::new();
                file.read_to_end(&mut buf)?;
                extract_docx_text(&buf)?
            }
            _ => {
                return Err(anyhow!("Unsupported file extension. Supported formats: .pdf, .xlsx, .xls, .ods, .docx"));
            }
        };

        Ok(json!({
            "status": "success",
            "content": content
        }))
    }
}
