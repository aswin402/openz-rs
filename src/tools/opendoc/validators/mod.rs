//! Document validation (PDF/A compliance, structure checks, etc.)

pub mod pdf_a;

use crate::tools::opendoc::ir::Document;

#[derive(Debug, Clone, serde::Serialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub checks: Vec<ValidationCheck>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ValidationCheck {
    pub name: String,
    pub passed: bool,
    pub message: String,
}

/// Basic document structure validation
pub fn validate_document(doc: &Document) -> ValidationResult {
    let mut checks = Vec::new();

    // Check for empty documents
    let has_content = !doc.paragraphs.is_empty() || doc.text.is_some();
    checks.push(ValidationCheck {
        name: "non_empty".to_string(),
        passed: has_content,
        message: match has_content {
            false => "Document has no content".to_string(),
            true => "Document has content".to_string(),
        },
    });

    // Check for oversized content
    let total_chars: usize = doc.paragraphs.iter().map(|p| p.text.len()).sum();
    checks.push(ValidationCheck {
        name: "size_check".to_string(),
        passed: total_chars < 10_000_000, // 10M chars ~ 10MB
        message: format!("Total content size: {} chars", total_chars),
    });

    ValidationResult {
        valid: checks.iter().all(|c| c.passed),
        checks,
    }
}
