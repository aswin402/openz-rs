use crate::tools::opendoc::ir::elements::*;
use crate::tools::opendoc::ir::metadata::Metadata;
use serde::{Deserialize, Serialize};

/// The universal document representation.
///
/// Every format handler converts to/from this struct.
/// All editing tools operate on this struct.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    /// File format (docx, pptx, pdf, xlsx, html, md, csv)
    pub format: String,

    /// File path if loaded from disk
    pub path: Option<String>,

    /// Document metadata
    pub metadata: Metadata,

    /// Content sections (headings, slides, sheets)
    pub sections: Vec<Section>,

    /// Paragraph content
    pub paragraphs: Vec<Paragraph>,

    /// Tables
    pub tables: Vec<Table>,

    /// Embedded images
    pub images: Vec<Image>,

    /// Raw text content (for formats without rich structure)
    pub text: Option<String>,
}

impl Document {
    /// Create a new Document with the given format identifier.
    /// Format is a lowercase string like "docx", "pdf", "pptx", "xlsx", "html", "md", "csv", "txt".
    pub fn new(format: impl Into<String>) -> Self {
        Self {
            format: format.into(),
            path: None,
            metadata: Metadata::default(),
            sections: Vec::new(),
            paragraphs: Vec::new(),
            tables: Vec::new(),
            images: Vec::new(),
            text: None,
        }
    }

    /// Get the document outline (headings / slide titles / sheet names)
    pub fn outline(&self) -> Vec<&str> {
        let mut out: Vec<&str> = self.sections.iter().map(|s| s.title.as_str()).collect();
        out.extend(
            self.paragraphs
                .iter()
                .filter(|p| p.is_heading)
                .map(|p| p.text.as_str()),
        );
        out
    }

    /// Estimate token count for LLM context window planning
    pub fn estimate_tokens(&self) -> usize {
        let mut count = 0usize;
        for p in &self.paragraphs {
            count += p.text.len() / 4; // ~4 chars per token
        }
        for t in &self.tables {
            for row in &t.rows {
                for cell in row {
                    count += cell.len() / 4;
                }
            }
        }
        count += self.text.as_ref().map_or(0, |text| text.len() / 4);
        count
    }

    /// Chunk document for RAG pipelines using default fixed strategy
    pub fn chunk_for_embedding(&self, max_tokens: usize) -> Vec<Chunk> {
        crate::tools::opendoc::engine::chunk::chunk_document(
            self,
            crate::tools::opendoc::engine::chunk::ChunkingStrategy::Fixed,
            max_tokens,
            0,
        )
    }

    /// Chunk document for RAG pipelines with configurable strategy and overlap
    pub fn chunk_with_strategy(
        &self,
        strategy: crate::tools::opendoc::engine::chunk::ChunkingStrategy,
        max_tokens: usize,
        overlap: usize,
    ) -> Vec<Chunk> {
        crate::tools::opendoc::engine::chunk::chunk_document(self, strategy, max_tokens, overlap)
    }

    /// Convert the universal document representation to Markdown format.
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();
        match !self.paragraphs.is_empty() || !self.tables.is_empty() {
            true => {
                for p in &self.paragraphs {
                    match p.is_heading {
                        true => {
                            let level = "#".repeat(p.heading_level.max(1) as usize);
                            md.push_str(&format!("{} {}\n\n", level, p.text));
                        }
                        false => {
                            md.push_str(&format!("{}\n\n", p.text));
                        }
                    }
                }
                for table in &self.tables {
                    match &table.caption {
                        Some(cap) => md.push_str(&format!("*{}*\n\n", cap)),
                        None => {}
                    }
                    match table.headers.is_empty() {
                        false => {
                            md.push_str("| ");
                            md.push_str(&table.headers.join(" | "));
                            md.push_str(" |\n");
                            md.push_str("| ");
                            md.push_str(&vec!["---"; table.headers.len()].join(" | "));
                            md.push_str(" |\n");
                        }
                        true => {}
                    }
                    for row in &table.rows {
                        md.push_str("| ");
                        md.push_str(&row.join(" | "));
                        md.push_str(" |\n");
                    }
                    md.push('\n');
                }
            }
            false => match &self.text {
                Some(raw) => md.push_str(raw),
                None => {}
            },
        }
        md
    }
}

/// A section of a document (chapter, slide, worksheet)
/// A content section within a document (heading group, slide, or worksheet).
/// Sections help organize hierarchical document structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Section {
    pub title: String,
    pub level: u32, // 0 = root, 1 = heading 1, 2 = heading 2, etc.
    pub index: usize,
    pub content: Vec<Paragraph>,
}

/// A chunk of text for RAG embedding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub text: String,
    pub heading: String,
    pub index: usize,
}
