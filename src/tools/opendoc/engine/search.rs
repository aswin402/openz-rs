use crate::tools::opendoc::ir::Document;

fn push_if_match(
    results: &mut Vec<SearchResult>,
    matched: bool,
    paragraph_index: Option<usize>,
    text: &str,
    is_heading: bool,
    match_type: &str,
    location: String,
) {
    match matched {
        true => results.push(SearchResult {
            paragraph_index,
            text: text.to_string(),
            is_heading,
            match_type: match_type.to_string(),
            location,
        }),
        false => {}
    }
}

/// Search document content for a pattern
pub fn search_document(doc: &Document, query: &str, use_search_pattern: bool) -> Vec<SearchResult> {
    let mut results = Vec::new();

    let pattern: Box<dyn Fn(&str) -> bool> = match use_search_pattern {
        true => {
            match super::replace::is_pattern_match(query, "") {
                Ok(_) => {}
                Err(_) => return results,
            }
            let q = query.to_string();
            Box::new(move |text: &str| {
                super::replace::is_pattern_match(&q, text).unwrap_or(false)
            })
        }
        false => {
            let q = query.to_lowercase();
            Box::new(move |text: &str| text.to_lowercase().contains(&q))
        }
    };

    // 1. Search raw text
    match &doc.text {
        Some(raw_text) => {
            for (i, line) in raw_text.lines().enumerate() {
                push_if_match(
                    &mut results,
                    pattern(line),
                    None,
                    line,
                    false,
                    "raw_text",
                    format!("line {}", i + 1),
                );
            }
        }
        None => {}
    }

    // 2. Search paragraphs
    for (i, p) in doc.paragraphs.iter().enumerate() {
        let (match_type, location) = match p.is_heading {
            true => ("heading", format!("heading {}", p.heading_level)),
            false => ("paragraph", format!("paragraph {}", i + 1)),
        };
        push_if_match(
            &mut results,
            pattern(&p.text),
            Some(i),
            &p.text,
            p.is_heading,
            match_type,
            location,
        );
    }

    // 3. Search sections
    for (i, sec) in doc.sections.iter().enumerate() {
        push_if_match(
            &mut results,
            pattern(&sec.title),
            None,
            &sec.title,
            true,
            "section_title",
            format!("section {} ('{}')", i + 1, sec.title),
        );
    }

    // 4. Search tables (headers and rows)
    for (t_idx, table) in doc.tables.iter().enumerate() {
        match &table.caption {
            Some(cap) => push_if_match(
                &mut results,
                pattern(cap),
                None,
                cap,
                false,
                "table_caption",
                format!("table {} caption", t_idx + 1),
            ),
            None => {}
        }
        for (h_idx, header) in table.headers.iter().enumerate() {
            push_if_match(
                &mut results,
                pattern(header),
                None,
                header,
                false,
                "table_header",
                format!("table {}, header column {}", t_idx + 1, h_idx + 1),
            );
        }
        for (r_idx, row) in table.rows.iter().enumerate() {
            for (c_idx, cell) in row.iter().enumerate() {
                push_if_match(
                    &mut results,
                    pattern(cell),
                    None,
                    cell,
                    false,
                    "table_cell",
                    format!(
                        "table {}, row {}, column {}",
                        t_idx + 1,
                        r_idx + 1,
                        c_idx + 1
                    ),
                );
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
    use crate::tools::opendoc::ir::Document;

    #[test]
    fn test_search_case_insensitive() {
        let mut doc = Document::new("txt");
        doc.paragraphs
            .push(crate::tools::opendoc::ir::Paragraph::new("Hello World"));
        let results = search_document(&doc, "hello", false);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_search_regex() {
        let mut doc = Document::new("txt");
        doc.paragraphs
            .push(crate::tools::opendoc::ir::Paragraph::new("Call 555-1234"));
        let results = search_document(&doc, r"\d{3}-\d{4}", true);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_search_multiple_paragraphs() {
        let mut doc = Document::new("txt");
        doc.paragraphs.push(crate::tools::opendoc::ir::Paragraph::new("First"));
        doc.paragraphs.push(crate::tools::opendoc::ir::Paragraph::new("Second"));
        doc.paragraphs
            .push(crate::tools::opendoc::ir::Paragraph::new("First again"));
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
        doc.sections.push(crate::tools::opendoc::ir::Section {
            title: "Introduction Section".to_string(),
            level: 1,
            index: 0,
            content: Vec::new(),
        });

        let table = crate::tools::opendoc::ir::Table::new(
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
