use crate::ir::Document;

/// Replace text in a document
pub fn replace_text(doc: &mut Document, find: &str, replace: &str) -> usize {
    let re = match regex::RegexBuilder::new(find).size_limit(1_000_000).build() {
        Ok(r) => r,
        Err(_) => return 0,
    };

    let mut count = 0;
    for p in &mut doc.paragraphs {
        let new_text = re.replace_all(&p.text, replace).to_string();
        if new_text != p.text {
            count += 1;
            p.text = new_text;
        }
    }

    // Also replace in section titles
    for section in &mut doc.sections {
        let new_title = re.replace_all(&section.title, replace).to_string();
        if new_title != section.title {
            count += 1;
            section.title = new_title;
        }
    }

    // Also replace in tables
    for table in &mut doc.tables {
        for header in &mut table.headers {
            let new = re.replace_all(header, replace).to_string();
            if new != *header {
                count += 1;
                *header = new;
            }
        }
        for row in &mut table.rows {
            for cell in row {
                let new = re.replace_all(cell, replace).to_string();
                if new != *cell {
                    count += 1;
                    *cell = new;
                }
            }
        }
    }

    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{Paragraph, Section, Table};

    #[test]
    fn test_replace_plain_text() {
        let mut doc = Document::new("txt");
        doc.paragraphs.push(Paragraph::new("Hello World"));
        let count = replace_text(&mut doc, "World", "Rust");
        assert_eq!(count, 1);
        assert_eq!(doc.paragraphs[0].text, "Hello Rust");
    }

    #[test]
    fn test_replace_regex() {
        let mut doc = Document::new("txt");
        doc.paragraphs.push(Paragraph::new("Hello 123 World 456"));
        let count = replace_text(&mut doc, r"\d+", "NUM");
        assert_eq!(count, 1);
        assert_eq!(doc.paragraphs[0].text, "Hello NUM World NUM");
    }

    #[test]
    fn test_replace_multiple_paragraphs() {
        let mut doc = Document::new("txt");
        doc.paragraphs.push(Paragraph::new("foo bar"));
        doc.paragraphs.push(Paragraph::new("baz bar"));
        let count = replace_text(&mut doc, "bar", "qux");
        assert_eq!(count, 2);
        assert_eq!(doc.paragraphs[0].text, "foo qux");
        assert_eq!(doc.paragraphs[1].text, "baz qux");
    }

    #[test]
    fn test_replace_no_match() {
        let mut doc = Document::new("txt");
        doc.paragraphs.push(Paragraph::new("Hello World"));
        let count = replace_text(&mut doc, "Nonexistent", "Rust");
        assert_eq!(count, 0);
        assert_eq!(doc.paragraphs[0].text, "Hello World");
    }

    #[test]
    fn test_replace_in_section_title() {
        let mut doc = Document::new("txt");
        doc.sections.push(Section {
            title: "Introduction".to_string(),
            level: 1,
            index: 0,
            content: vec![],
        });
        let count = replace_text(&mut doc, "Intro", "Conclusion");
        assert_eq!(count, 1);
        assert_eq!(doc.sections[0].title, "Conclusionduction");
    }

    #[test]
    fn test_replace_in_table() {
        let mut doc = Document::new("csv");
        doc.tables.push(Table {
            headers: vec!["Name".to_string(), "City".to_string()],
            rows: vec![
                vec!["Alice".to_string(), "NYC".to_string()],
            ],
            caption: None,
        });
        let count = replace_text(&mut doc, "NYC", "LA");
        assert_eq!(count, 1);
        assert_eq!(doc.tables[0].rows[0][1], "LA");
    }

    #[test]
    fn test_replace_invalid_regex_returns_zero() {
        let mut doc = Document::new("txt");
        doc.paragraphs.push(Paragraph::new("Hello"));
        let count = replace_text(&mut doc, "[invalid", "x");
        assert_eq!(count, 0);
    }
}
