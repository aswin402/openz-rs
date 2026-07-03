//! Shared test helpers.
//!
//! Generates temporary test files for each format we can reliably create.
//! Complex formats (PDF, PPTX, DOCX) are tested via in-module unit tests or skipped.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// Get a unique temp file path with the given extension
pub fn temp_path(ext: &str) -> PathBuf {
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let mut p = std::env::temp_dir();
    p.push(format!("opendoc_test_{id}.{ext}"));
    p
}

/// Generate a minimal TXT fixture
pub fn gen_txt(path: &Path) {
    std::fs::write(path, "Hello World\nThis is line 2.\n").unwrap();
}

/// Generate a minimal CSV fixture
pub fn gen_csv(path: &Path) {
    std::fs::write(path, "name,age,city\nAlice,30,NYC\nBob,25,SF\n").unwrap();
}

/// Generate a minimal XLSX fixture
pub fn gen_xlsx(path: &Path) {
    use rust_xlsxwriter::*;
    let mut workbook = Workbook::new();
    let sheet = workbook.add_worksheet();
    sheet.write_string(0, 0, "Name").unwrap();
    sheet.write_string(0, 1, "Age").unwrap();
    sheet.write_string(1, 0, "Alice").unwrap();
    sheet.write_number(1, 1, 30.0).unwrap();
    sheet.write_string(2, 0, "Bob").unwrap();
    sheet.write_number(2, 1, 25.0).unwrap();
    workbook.save(path).unwrap();
}
