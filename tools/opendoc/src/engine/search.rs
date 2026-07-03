use crate::ir::Document;

/// Search document content for a pattern
pub fn search_document(doc: &Document, query: &str, use_regex: bool) -> Vec<SearchResult> {
    let mut results = Vec::new();

    let pattern: Box<dyn Fn(&str) -> bool> = if use_regex {
        match regex::RegexBuilder::new(query).size_limit(1_000_000).build() {
            Ok(re) => Box::new(move |text: &str| re.is_match(text)),
            Err(_) => return results,
        }
    } else {
        let q = query.to_lowercase();
        Box::new(move |text: &str| text.to_lowercase().contains(&q))
    };

    // 1. Search raw text
    if let Some(ref raw_text) = doc.text {
        for (i, line) in raw_text.lines().enumerate() {
            if pattern(line) {
                results.push(SearchResult {
                    paragraph_index: None,
                    text: line.to_string(),
                    is_heading: false,
                    match_type: "raw_text".to_string(),
                    location: format!("line {}", i + 1),
                });
            }
        }
    }

    // 2. Search paragraphs
    for (i, p) in doc.paragraphs.iter().enumerate() {
        if pattern(&p.text) {
            results.push(SearchResult {
                paragraph_index: Some(i),
                text: p.text.clone(),
                is_heading: p.is_heading,
                match_type: if p.is_heading { "heading".to_string() } else { "paragraph".to_string() },
                location: if p.is_heading { format!("heading {}", p.heading_level) } else { format!("paragraph {}", i + 1) },
            });
        }
    }

    // 3. Search sections
    for (i, sec) in doc.sections.iter().enumerate() {
        if pattern(&sec.title) {
            results.push(SearchResult {
                paragraph_index: None,
                text: sec.title.clone(),
                is_heading: true,
                match_type: "section_title".to_string(),
                location: format!("section {} ('{}')", i + 1, sec.title),
            });
        }
    }

    // 4. Search tables (headers and rows)
    for (t_idx, table) in doc.tables.iter().enumerate() {
        if let Some(ref cap) = table.caption {
            if pattern(cap) {
                results.push(SearchResult {
                    paragraph_index: None,
                    text: cap.clone(),
                    is_heading: false,
                    match_type: "table_caption".to_string(),
                    location: format!("table {} caption", t_idx + 1),
                });
            }
        }
        for (h_idx, header) in table.headers.iter().enumerate() {
            if pattern(header) {
                results.push(SearchResult {
                    paragraph_index: None,
                    text: header.clone(),
                    is_heading: false,
                    match_type: "table_header".to_string(),
                    location: format!("table {}, header column {}", t_idx + 1, h_idx + 1),
                });
            }
        }
        for (r_idx, row) in table.rows.iter().enumerate() {
            for (c_idx, cell) in row.iter().enumerate() {
                if pattern(cell) {
                    results.push(SearchResult {
                        paragraph_index: None,
                        text: cell.clone(),
                        is_heading: false,
                        match_type: "table_cell".to_string(),
                        location: format!("table {}, row {}, column {}", t_idx + 1, r_idx + 1, c_idx + 1),
                    });
                }
            }
        }
    }

    results
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paragraph_index: Option<usize>,
    pub text: String,
    pub is_heading: bool,
    pub match_type: String,
    pub location: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::Document;

    #[test]
    fn test_search_case_insensitive() {
        let mut doc = Document::new("txt");
        doc.paragraphs.push(crate::ir::Paragraph::new("Hello World"));
        let results = search_document(&doc, "hello", false);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_search_regex() {
        let mut doc = Document::new("txt");
        doc.paragraphs.push(crate::ir::Paragraph::new("Call 555-1234"));
        let results = search_document(&doc, r"\d{3}-\d{4}", true);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_search_multiple_paragraphs() {
        let mut doc = Document::new("txt");
        doc.paragraphs.push(crate::ir::Paragraph::new("First"));
        doc.paragraphs.push(crate::ir::Paragraph::new("Second"));
        doc.paragraphs.push(crate::ir::Paragraph::new("First again"));
        let results = search_document(&doc, "First", false);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_search_empty_doc() {
        let doc = Document::new("txt");
        let results = search_document(&doc, "anything", false);
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_table_and_section() {
        let mut doc = Document::new("docx");
        doc.sections.push(crate::ir::Section {
            title: "Introduction Section".to_string(),
            level: 1,
            index: 0,
            content: Vec::new(),
        });
        
        let table = crate::ir::Table::new(
            vec!["Header A".to_string(), "Header B".to_string()],
            vec![vec!["Value A".to_string(), "Target Cell".to_string()]],
        );
        doc.tables.push(table);
        
        // 1. Search section title
        let results_sec = search_document(&doc, "Introduction", false);
        assert_eq!(results_sec.len(), 1);
        assert_eq!(results_sec[0].match_type, "section_title");
        assert_eq!(results_sec[0].text, "Introduction Section");

        // 2. Search table header
        let results_hdr = search_document(&doc, "Header B", false);
        assert_eq!(results_hdr.len(), 1);
        assert_eq!(results_hdr[0].match_type, "table_header");

        // 3. Search table cell
        let results_cell = search_document(&doc, "Target", false);
        assert_eq!(results_cell.len(), 1);
        assert_eq!(results_cell[0].match_type, "table_cell");
        assert_eq!(results_cell[0].text, "Target Cell");
    }
}
