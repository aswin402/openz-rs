//! CSV handler — converts CSV documents into IR using the csv crate.
//!
//! Each CSV file becomes a Section + Table in the IR (first row = headers).

use crate::ir::{Document, Section, Table};

/// Load a CSV file into the Internal Representation
pub fn to_ir(file_path: &str) -> Result<Document, String> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_path(file_path)
        .map_err(|e| format!("Failed to open CSV: {e}"))?;

    let mut doc = Document::new("csv");
    doc.path = Some(file_path.to_string());

    // Get headers
    let headers: Vec<String> = reader
        .headers()
        .map_err(|e| format!("Failed to read CSV headers: {e}"))?
        .iter()
        .map(|h| h.to_string())
        .collect();

    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut text_parts: Vec<String> = Vec::new();

    // Render header line
    if !headers.is_empty() {
        text_parts.push(format!("  {} |", headers.join(" | ")));
        text_parts.push(format!("  {} |", vec!["---"; headers.len()].join(" | ")));
    }

    // Read data rows
    for result in reader.records() {
        let record = result.map_err(|e| format!("Failed to read CSV record: {e}"))?;
        let row: Vec<String> = record.iter().map(|f| f.to_string()).collect();
        text_parts.push(format!("  {} |", row.join(" | ")));
        rows.push(row);
    }

    let section = Section {
        title: "CSV Data".to_string(),
        level: 0,
        index: 0,
        content: vec![],
    };

    let table = Table {
        headers,
        rows,
        caption: None,
    };

    doc.sections.push(section);
    doc.tables.push(table);
    doc.text = Some(text_parts.join("\n"));
    doc.metadata.page_count = Some(1);

    Ok(doc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static CSV_COUNTER: AtomicU32 = AtomicU32::new(0);

    fn unique_path(content: &str) -> String {
        let id = CSV_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir();
        let path = dir.join(format!("opendoc_csv_test_{id}.csv"));
        std::fs::write(&path, content).unwrap();
        path.to_str().unwrap().to_string()
    }

    #[test]
    fn test_csv_basic() {
        let csv = "name,age\nAlice,30\nBob,25\n";
        let path = unique_path(csv);
        let doc = to_ir(&path).unwrap();
        assert_eq!(doc.format, "csv");
        assert_eq!(doc.sections.len(), 1);
        assert_eq!(doc.tables.len(), 1);
        assert_eq!(doc.tables[0].headers, vec!["name", "age"]);
        assert_eq!(doc.tables[0].rows.len(), 2);
        assert_eq!(doc.tables[0].rows[0][0], "Alice");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_csv_text_representation() {
        let csv = "name,age\nAlice,30\n";
        let path = unique_path(csv);
        let doc = to_ir(&path).unwrap();
        let text = doc.text.unwrap_or_default();
        assert!(text.contains("name"));
        assert!(text.contains("Alice"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_csv_empty_headers_single_row() {
        let csv = "header\nvalue\n";
        let path = unique_path(csv);
        let doc = to_ir(&path).unwrap();
        assert_eq!(doc.tables[0].headers, vec!["header"]);
        assert_eq!(doc.tables[0].rows[0][0], "value");
        let _ = std::fs::remove_file(&path);
    }
}
