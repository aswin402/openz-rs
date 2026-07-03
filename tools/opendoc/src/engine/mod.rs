//! Document editing engine.
//!
//! All editing operations work on the IR, not on native formats.
//! This means every operation works for ALL document types.

pub mod search;
pub mod replace;
pub mod template;
pub mod diff;
pub mod complexity;
pub mod chunk;
pub mod extract;

use crate::ir::Document;

/// Apply a series of edits to a document in one pipeline
pub fn edit_pipeline(doc: &mut Document, operations: &[EditOperation]) -> Vec<String> {
    let mut results = Vec::new();
    for op in operations {
        match op {
            EditOperation::Replace(find, replace) => {
                let count = replace::replace_text(doc, find, replace);
                results.push(format!("replace: {} occurrences", count));
            }
            EditOperation::Append(text) => {
                let p = crate::ir::elements::Paragraph::new(text);
                doc.paragraphs.push(p);
                results.push("append: 1 paragraph".to_string());
            }
            EditOperation::Prepend(text) => {
                let p = crate::ir::elements::Paragraph::new(text);
                doc.paragraphs.insert(0, p);
                results.push("prepend: 1 paragraph".to_string());
            }
            EditOperation::Remove(index) => {
                if *index < doc.paragraphs.len() {
                    doc.paragraphs.remove(*index);
                    results.push(format!("remove: paragraph {}", index));
                }
            }
            EditOperation::Translate(lang) => {
                results.push(format!("translate: target={} (requires AI backend)", lang));
            }
            EditOperation::Summarize => {
                results.push("summarize: (requires AI backend)".to_string());
            }
        }
    }
    results
}

/// Operations that can be composed in a pipeline
#[derive(Debug, Clone)]
pub enum EditOperation {
    Replace(String, String),
    Append(String),
    Prepend(String),
    Remove(usize),
    Translate(String),
    Summarize,
}
