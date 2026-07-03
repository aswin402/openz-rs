//! Integration tests for opendoc-mcp.
//!
//! Tests the full load_to_ir -> IR -> output pipeline for each format.

mod common;

use opendoc_mcp::handlers::load_to_ir;
use opendoc_mcp::engine::{complexity, replace, search};
use common::*;

// ──────────────────────────────────────────────
//  TXT → IR
// ──────────────────────────────────────────────

#[test]
fn test_txt_to_ir() {
    let path = temp_path("txt");
    gen_txt(&path);

    let doc = load_to_ir(path.to_str().unwrap()).unwrap();
    assert_eq!(doc.format, "txt");
    assert!(doc.text.unwrap_or_default().contains("Hello World"));
}

// ──────────────────────────────────────────────
//  CSV → IR
// ──────────────────────────────────────────────

#[test]
fn test_csv_to_ir() {
    let path = temp_path("csv");
    gen_csv(&path);

    let doc = load_to_ir(path.to_str().unwrap()).unwrap();
    assert_eq!(doc.format, "csv");
    assert_eq!(doc.tables.len(), 1);
    assert_eq!(doc.tables[0].headers, vec!["name", "age", "city"]);
    assert_eq!(doc.tables[0].rows.len(), 2);
}

// ──────────────────────────────────────────────
//  XLSX → IR
// ──────────────────────────────────────────────

#[test]
fn test_xlsx_to_ir() {
    let path = temp_path("xlsx");
    gen_xlsx(&path);

    let doc = load_to_ir(path.to_str().unwrap()).unwrap();
    assert_eq!(doc.format, "xlsx");
    assert_eq!(doc.sections.len(), 1);
    assert_eq!(doc.tables.len(), 1);

    let table = &doc.tables[0];
    assert_eq!(table.headers, vec!["Name", "Age"]);
    assert_eq!(table.rows.len(), 2);
    assert_eq!(table.rows[0][0], "Alice");
}

#[test]
fn test_xlsx_text_representation() {
    let path = temp_path("xlsx");
    gen_xlsx(&path);

    let doc = load_to_ir(path.to_str().unwrap()).unwrap();
    let text = doc.text.unwrap_or_default();
    assert!(text.contains("Name"));
    assert!(text.contains("Alice"));
    assert!(text.contains("30"));
}

// ──────────────────────────────────────────────
//  HTML → IR
// ──────────────────────────────────────────────

#[test]
fn test_html_to_ir() {
    let path = temp_path("html");
    std::fs::write(&path, "<html><body><h1>Title</h1><p>Hello World</p></body></html>").unwrap();

    let doc = load_to_ir(path.to_str().unwrap()).unwrap();
    assert_eq!(doc.format, "html");
    assert_eq!(doc.sections.len(), 1);
    assert_eq!(doc.sections[0].title, "Title");
}

#[test]
fn test_html_text_content() {
    let path = temp_path("html");
    std::fs::write(&path, "<html><body><p>Para 1</p><p>Para 2</p></body></html>").unwrap();

    let doc = load_to_ir(path.to_str().unwrap()).unwrap();
    let text = doc.text.unwrap_or_default();
    assert!(text.contains("Para 1"));
    assert!(text.contains("Para 2"));
}

// ──────────────────────────────────────────────
//  MD → IR
// ──────────────────────────────────────────────

#[test]
fn test_md_to_ir() {
    let path = temp_path("md");
    std::fs::write(&path, "# Title\n\nHello World").unwrap();

    let doc = load_to_ir(path.to_str().unwrap()).unwrap();
    assert_eq!(doc.format, "markdown");
    assert_eq!(doc.sections.len(), 1);
    assert_eq!(doc.sections[0].title, "Title");
}

#[test]
fn test_md_text_content() {
    let path = temp_path("md");
    std::fs::write(&path, "Line 1\n\nLine 2").unwrap();

    let doc = load_to_ir(path.to_str().unwrap()).unwrap();
    let text = doc.text.unwrap_or_default();
    assert!(text.contains("Line 1"));
    assert!(text.contains("Line 2"));
}

// ──────────────────────────────────────────────
//  Unsupported format
// ──────────────────────────────────────────────

#[test]
fn test_unsupported_format() {
    let path = temp_path("xyz");
    std::fs::write(&path, "garbage").unwrap();

    let result = load_to_ir(path.to_str().unwrap());
    assert!(result.is_err());
}

// ──────────────────────────────────────────────
//  Complexity analysis
// ──────────────────────────────────────────────

#[test]
fn test_complexity_scanned() {
    let doc = opendoc_mcp::ir::Document::new("pdf");
    // Empty doc with no text → scanned
    let report = complexity::analyze_complexity(&doc);
    assert!(report.needs_ocr);
    assert!(matches!(report.estimated_complexity, complexity::Complexity::Scanned));
}

#[test]
fn test_complexity_simple() {
    let mut doc = opendoc_mcp::ir::Document::new("txt");
    doc.paragraphs.push(opendoc_mcp::ir::Paragraph::new("Hello world"));
    doc.metadata.page_count = Some(1);

    let report = complexity::analyze_complexity(&doc);
    assert!(!report.needs_ocr);
    assert!(matches!(report.estimated_complexity, complexity::Complexity::Simple));
    assert_eq!(report.recommended_pipeline, "text");
}

// ──────────────────────────────────────────────
//  Search & Replace
// ──────────────────────────────────────────────

#[test]
fn test_search_found() {
    let mut doc = opendoc_mcp::ir::Document::new("txt");
    doc.paragraphs.push(opendoc_mcp::ir::Paragraph::new("The quick brown fox"));

    let results = search::search_document(&doc, "fox", false);
    assert_eq!(results.len(), 1);
    assert!(results[0].text.contains("fox"));
}

#[test]
fn test_search_not_found() {
    let mut doc = opendoc_mcp::ir::Document::new("txt");
    doc.paragraphs.push(opendoc_mcp::ir::Paragraph::new("Hello world"));

    let results = search::search_document(&doc, "foobar", false);
    assert!(results.is_empty());
}

#[test]
fn test_replace_text() {
    let mut doc = opendoc_mcp::ir::Document::new("txt");
    doc.paragraphs.push(opendoc_mcp::ir::Paragraph::new("Hello World"));

    let count = replace::replace_text(&mut doc, "World", "Rust");
    assert_eq!(count, 1);
    assert_eq!(doc.paragraphs[0].text, "Hello Rust");
}

#[test]
fn test_replace_regex() {
    let mut doc = opendoc_mcp::ir::Document::new("txt");
    doc.paragraphs.push(opendoc_mcp::ir::Paragraph::new("Hello 123 World"));

    let count = replace::replace_text(&mut doc, r"\d+", "NUM");
    assert_eq!(count, 1);
    assert_eq!(doc.paragraphs[0].text, "Hello NUM World");
}

// ──────────────────────────────────────────────
//  Format detection
// ──────────────────────────────────────────────

#[test]
fn test_format_detection_xlsx_extension() {
    let path = temp_path("xlsx");
    gen_xlsx(&path);

    let doc = load_to_ir(path.to_str().unwrap()).unwrap();
    assert_eq!(doc.format, "xlsx");
}

#[test]
fn test_format_detection_txt_extension() {
    let path = temp_path("txt");
    gen_txt(&path);

    let doc = load_to_ir(path.to_str().unwrap()).unwrap();
    assert_eq!(doc.format, "txt");
}

// ──────────────────────────────────────────────
//  Edge cases
// ──────────────────────────────────────────────

#[test]
fn test_empty_txt() {
    let path = temp_path("txt");
    std::fs::write(&path, "").unwrap();

    let doc = load_to_ir(path.to_str().unwrap()).unwrap();
    assert_eq!(doc.text.unwrap_or_default(), "");
}

#[test]
fn test_file_not_found() {
    let result = load_to_ir("/nonexistent/path/file.docx");
    assert!(result.is_err());
}

#[test]
fn test_conversion_bidirectional() {
    let path_csv = temp_path("csv");
    gen_csv(&path_csv);

    let path_md = temp_path("md");
    
    // Convert CSV -> MD
    let res = opendoc_mcp::converters::convert(
        path_csv.to_str().unwrap(),
        "md",
        path_md.to_str().unwrap(),
    );
    assert!(res.is_ok());

    // Read the MD file
    let md_content = std::fs::read_to_string(&path_md).unwrap();
    assert!(md_content.contains("| name | age | city |"));
    assert!(md_content.contains("| Alice | 30 | NYC |"));

    // Convert MD -> HTML
    let path_html = temp_path("html");
    let res2 = opendoc_mcp::converters::convert(
        path_md.to_str().unwrap(),
        "html",
        path_html.to_str().unwrap(),
    );
    assert!(res2.is_ok());

    let html_content = std::fs::read_to_string(&path_html).unwrap();
    assert!(html_content.contains("<th>name</th>"));
    assert!(html_content.contains("<td>Alice</td>"));

    let _ = std::fs::remove_file(path_csv);
    let _ = std::fs::remove_file(path_md);
    let _ = std::fs::remove_file(path_html);
}
