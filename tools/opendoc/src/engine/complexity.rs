//! Document complexity detection.
//!
//! Analyzes a document and provides signals about its complexity:
//! - Whether OCR is needed (scanned PDFs, embedded images)
//! - Text density
//! - Layout complexity
//! - Recommended parser pipeline

use crate::ir::Document;

/// Complexity analysis results
#[derive(Debug, Clone, serde::Serialize)]
pub struct ComplexityReport {
    pub needs_ocr: bool,
    pub reasons: Vec<String>,
    pub text_density: f64,              // chars per page (0 if no pages)
    pub estimated_complexity: Complexity,
    pub page_count: u32,
    pub has_tables: bool,
    pub has_images: bool,
    pub has_mixed_languages: bool,
    pub recommended_pipeline: String,
}

/// Complexity level
#[derive(Debug, Clone, serde::Serialize)]
pub enum Complexity {
    Simple,     // Plain text, single column
    Moderate,   // Tables, lists, some formatting
    Complex,    // Multi-column, dense tables, mixed content
    Scanned,    // Image-based, needs OCR
}

impl Complexity {
    /// Check if this complexity is at least the given level
    pub fn is_at_least(&self, other: &Complexity) -> bool {
        self.level() >= other.level()
    }

    fn level(&self) -> u8 {
        match self {
            Complexity::Simple => 0,
            Complexity::Moderate => 1,
            Complexity::Complex => 2,
            Complexity::Scanned => 3,
        }
    }
}

/// Analyze document complexity
pub fn analyze_complexity(doc: &Document) -> ComplexityReport {
    let mut reasons = Vec::new();
    let mut is_scanned = false;

    // Count chars
    let total_chars: usize = doc.paragraphs.iter().map(|p| p.text.len()).sum();
    let page_count = doc.metadata.page_count.unwrap_or(1).max(1);
    let text_density = total_chars as f64 / page_count as f64;

    // Check for scanned PDF (very low text density + no paragraphs)
    if doc.paragraphs.is_empty() && doc.text.as_ref().is_none_or(|t| t.len() < 100) {
        is_scanned = true;
        reasons.push("No extractable text".to_string());
        reasons.push("scanned".to_string());
    }

    // Check for embedded images (likely scanned)
    if !doc.images.is_empty() && text_density < 500.0 {
        is_scanned = true;
        reasons.push("embedded-images".to_string());
    }

    // Check for sparse text
    if text_density < 200.0 && total_chars > 0 {
        reasons.push("sparse-text".to_string());
    }

    // Check for no text at all
    if total_chars == 0 {
        reasons.push("no-text".to_string());
        is_scanned = true;
    }

    // Determine complexity level
    let estimated_complexity = if is_scanned {
        Complexity::Scanned
    } else if doc.tables.len() > 5 || doc.sections.len() > 10 {
        Complexity::Complex
    } else if !doc.tables.is_empty() || doc.paragraphs.len() > 50 {
        Complexity::Moderate
    } else {
        Complexity::Simple
    };

    // Determine recommended pipeline (use level method, not clone)
    let recommended_pipeline = if is_scanned {
        "ocr".to_string()
    } else if estimated_complexity.level() >= Complexity::Moderate.level() {
        "spatial".to_string()
    } else {
        "text".to_string()
    };

    ComplexityReport {
        needs_ocr: is_scanned,
        reasons,
        text_density,
        estimated_complexity,
        page_count,
        has_tables: !doc.tables.is_empty(),
        has_images: !doc.images.is_empty(),
        has_mixed_languages: false,
        recommended_pipeline,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::Document;

    #[test]
    fn test_complexity_levels() {
        assert!(Complexity::Simple.is_at_least(&Complexity::Simple));
        assert!(!Complexity::Simple.is_at_least(&Complexity::Moderate));
        assert!(Complexity::Complex.is_at_least(&Complexity::Simple));
        assert!(Complexity::Scanned.is_at_least(&Complexity::Complex));
    }

    #[test]
    fn test_analyze_empty_doc() {
        let doc = Document::new("txt");
        let report = analyze_complexity(&doc);
        assert!(report.needs_ocr);
        assert!(matches!(report.estimated_complexity, Complexity::Scanned));
    }
}
