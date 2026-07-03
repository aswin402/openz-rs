//! HTML handler — converts HTML documents into IR using scraper.
//!
//! Extracts text from body, heading tags become sections, standard text
//! becomes paragraphs.

use crate::ir::{Document, Paragraph, Section};
use scraper::{Html, Selector};

/// Load an HTML file into the Internal Representation
pub fn to_ir(file_path: &str) -> Result<Document, String> {
    let content = std::fs::read_to_string(file_path)
        .map_err(|e| format!("Failed to read HTML file: {e}"))?;

    html_to_ir(&content, Some(file_path))
}

/// Convert HTML string content into IR
pub fn html_to_ir(html: &str, path: Option<&str>) -> Result<Document, String> {
    let mut doc = Document::new("html");
    if let Some(p) = path {
        doc.path = Some(p.to_string());
    }

    let document = Html::parse_document(html);

    // Try to get <title>
    let title_sel = Selector::parse("title").map_err(|e| format!("Selector error: {e}"))?;
    if let Some(title_el) = document.select(&title_sel).next() {
        let t = title_el.text().collect::<String>().trim().to_string();
        if !t.is_empty() {
            doc.metadata.title = Some(t);
        }
    }

    // Collect text snippets
    let mut text_parts: Vec<String> = Vec::new();

    // Process heading elements as sections in document order
    let heading_sel = Selector::parse("h1, h2, h3, h4, h5, h6")
        .map_err(|e| format!("Selector error: {e}"))?;
    for el in document.select(&heading_sel) {
        let tag_name = el.value().name();
        let level: u32 = tag_name[1..].parse().unwrap_or(1);
        let title = el.text().collect::<String>().trim().to_string();
        if !title.is_empty() {
            text_parts.push(format!("{}: {}", "#".repeat(level as usize), title));
            doc.sections.push(Section {
                title,
                level,
                index: doc.sections.len(),
                content: vec![],
            });
        }
    }

    // Process paragraph elements as paragraphs
    let p_sel = Selector::parse("p, div, span, li")
        .map_err(|e| format!("Selector error: {e}"))?;
    for el in document.select(&p_sel) {
        let text = el.text().collect::<String>().trim().to_string();
        if !text.is_empty() {
            // Check if this is a heading tag
            let tag_name = el.value().name();
            if tag_name.starts_with('h') && tag_name.len() == 2
                && tag_name.as_bytes()[1].is_ascii_digit()
            {
                continue; // already added as section
            }

            doc.paragraphs.push(Paragraph {
                text: text.clone(),
                is_heading: false,
                ..Paragraph::default()
            });
            text_parts.push(text);
        }
    }

    doc.text = Some(text_parts.join("\n"));
    doc.metadata.page_count = Some(1);

    Ok(doc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_html_basic() {
        let html = "<html><head><title>Test</title></head><body><p>Hello World</p></body></html>";
        let doc = html_to_ir(html, None).unwrap();
        assert_eq!(doc.format, "html");
        assert_eq!(doc.metadata.title, Some("Test".to_string()));
        assert!(doc.text.as_deref().unwrap_or("").contains("Hello World"));
    }

    #[test]
    fn test_html_headings_as_sections() {
        let html = "<html><body><h1>Title</h1><p>Content</p><h2>Sub</h2><p>More</p></body></html>";
        let doc = html_to_ir(html, None).unwrap();
        assert_eq!(doc.sections.len(), 2);
        assert_eq!(doc.sections[0].title, "Title");
        assert_eq!(doc.sections[0].level, 1);
        assert_eq!(doc.sections[1].title, "Sub");
        assert_eq!(doc.sections[1].level, 2);
    }

    #[test]
    fn test_html_headings_order() {
        let html = "<html><body><h1>First H1</h1><h2>Then H2</h2><h1>Then H1 Again</h1></body></html>";
        let doc = html_to_ir(html, None).unwrap();
        assert_eq!(doc.sections.len(), 3);
        assert_eq!(doc.sections[0].title, "First H1");
        assert_eq!(doc.sections[1].title, "Then H2");
        assert_eq!(doc.sections[2].title, "Then H1 Again");
    }

    #[test]
    fn test_html_empty() {
        let doc = html_to_ir("<html></html>", None).unwrap();
        assert_eq!(doc.format, "html");
        assert!(doc.paragraphs.is_empty());
    }

    #[test]
    fn test_html_text_content() {
        let html = "<html><body><p>First</p><p>Second</p></body></html>";
        let doc = html_to_ir(html, None).unwrap();
        let text = doc.text.unwrap_or_default();
        assert!(text.contains("First"));
        assert!(text.contains("Second"));
    }
}
