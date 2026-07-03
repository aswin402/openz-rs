//! Markdown handler — converts Markdown documents into IR using pulldown-cmark.
//!
//! Heading elements become sections; paragraphs, lists, and code blocks
//! become paragraphs in the IR.

use crate::ir::{Document, Paragraph, Section};
use pulldown_cmark::{Event, Tag, TagEnd};

/// Load a Markdown file into the Internal Representation
pub fn to_ir(file_path: &str) -> Result<Document, String> {
    let content = std::fs::read_to_string(file_path)
        .map_err(|e| format!("Failed to read Markdown file: {e}"))?;

    md_to_ir(&content, Some(file_path))
}

/// Convert Markdown string content into IR
pub fn md_to_ir(md: &str, path: Option<&str>) -> Result<Document, String> {
    let mut doc = Document::new("markdown");
    if let Some(p) = path {
        doc.path = Some(p.to_string());
    }

    let mut options = pulldown_cmark::Options::empty();
    options.insert(pulldown_cmark::Options::ENABLE_TABLES);
    let parser = pulldown_cmark::Parser::new_ext(md, options);

    let mut text_parts: Vec<String> = Vec::new();
    let mut current_text = String::new();
    let mut current_heading_level: Option<u32> = None;

    let mut in_table = false;
    let mut current_row = Vec::new();
    let mut current_headers = Vec::new();
    let mut current_rows = Vec::new();

    fn flush_para(
        doc: &mut Document,
        text_parts: &mut Vec<String>,
        current_text: &mut String,
        heading_level: &mut Option<u32>,
    ) {
        let text = current_text.trim().to_string();
        if text.is_empty() {
            return;
        }

        if let Some(level) = *heading_level {
            doc.sections.push(Section {
                title: text.clone(),
                level,
                index: doc.sections.len(),
                content: vec![],
            });
            text_parts.push(format!("{} {}", "#".repeat(level as usize), text));
        } else {
            doc.paragraphs.push(Paragraph::new(&text));
            text_parts.push(text);
        }

        current_text.clear();
        *heading_level = None;
    }

    for event in parser {
        match event {
            Event::Start(Tag::Table(_)) => {
                flush_para(&mut doc, &mut text_parts, &mut current_text, &mut current_heading_level);
                in_table = true;
                current_headers.clear();
                current_rows.clear();
            }
            Event::End(TagEnd::Table) => {
                in_table = false;
                let table = crate::ir::Table {
                    headers: current_headers.clone(),
                    rows: current_rows.clone(),
                    caption: None,
                };
                doc.tables.push(table);
            }
            Event::Start(Tag::TableHead) => {
                current_row.clear();
            }
            Event::End(TagEnd::TableHead) => {
                current_headers = current_row.clone();
                current_row.clear();
            }
            Event::Start(Tag::TableRow) => {
                current_row.clear();
            }
            Event::End(TagEnd::TableRow) => {
                current_rows.push(current_row.clone());
            }
            Event::Start(Tag::TableCell) => {
                current_text.clear();
            }
            Event::End(TagEnd::TableCell) => {
                current_row.push(current_text.trim().to_string());
                current_text.clear();
            }
            Event::Start(Tag::Heading { level, .. }) => {
                flush_para(&mut doc, &mut text_parts, &mut current_text, &mut current_heading_level);
                current_heading_level = Some(level as u32);
            }
            Event::End(TagEnd::Heading(..)) => {
                flush_para(&mut doc, &mut text_parts, &mut current_text, &mut current_heading_level);
            }
            Event::Start(Tag::Paragraph) => {
                if !in_table {
                    flush_para(&mut doc, &mut text_parts, &mut current_text, &mut current_heading_level);
                }
            }
            Event::End(TagEnd::Paragraph) => {
                if !in_table {
                    flush_para(&mut doc, &mut text_parts, &mut current_text, &mut current_heading_level);
                }
            }
            Event::Text(text) => {
                current_text.push_str(&text);
            }
            Event::SoftBreak | Event::HardBreak => {
                current_text.push(' ');
            }
            Event::Code(text) => {
                current_text.push_str(&text);
            }
            _ => {}
        }
    }

    flush_para(&mut doc, &mut text_parts, &mut current_text, &mut current_heading_level);

    doc.text = Some(text_parts.join("\n"));
    doc.metadata.page_count = Some(1);

    Ok(doc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_md_basic() {
        let md = "# Title\n\nHello World";
        let doc = md_to_ir(md, None).unwrap();
        assert_eq!(doc.format, "markdown");
        assert_eq!(doc.sections.len(), 1);
        assert_eq!(doc.sections[0].title, "Title");
    }

    #[test]
    fn test_md_paragraphs() {
        let md = "First paragraph.\n\nSecond paragraph.";
        let doc = md_to_ir(md, None).unwrap();
        assert!(doc.paragraphs.iter().any(|p| p.text.contains("First")));
        assert!(doc.paragraphs.iter().any(|p| p.text.contains("Second")));
    }

    #[test]
    fn test_md_multiple_headings() {
        let md = "# H1\n\nContent\n\n## H2\n\nMore";
        let doc = md_to_ir(md, None).unwrap();
        assert_eq!(doc.sections.len(), 2);
        assert_eq!(doc.sections[0].title, "H1");
        assert_eq!(doc.sections[0].level, 1);
        assert_eq!(doc.sections[1].title, "H2");
        assert_eq!(doc.sections[1].level, 2);
    }

    #[test]
    fn test_md_table() {
        let md = "| name | age |\n| --- | --- |\n| Alice | 30 |";
        let doc = md_to_ir(md, None).unwrap();
        assert_eq!(doc.tables.len(), 1);
        assert_eq!(doc.tables[0].headers, vec!["name", "age"]);
        assert_eq!(doc.tables[0].rows.len(), 1);
        assert_eq!(doc.tables[0].rows[0], vec!["Alice", "30"]);
    }

    #[test]
    fn test_md_empty() {
        let doc = md_to_ir("", None).unwrap();
        assert_eq!(doc.format, "markdown");
        assert!(doc.paragraphs.is_empty());
        assert!(doc.sections.is_empty());
    }

    #[test]
    fn test_md_text_content() {
        let md = "# Doc\n\nSome **bold** text.";
        let doc = md_to_ir(md, None).unwrap();
        let text = doc.text.unwrap_or_default();
        assert!(text.contains("Doc"));
        assert!(text.contains("Some bold text"));
    }
}
