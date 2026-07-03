//! Internal Document Representation (IR).
//!
//! The core of opendoc-mcp. Every format is converted into this IR,
//! edited in IR, then exported back. This means adding a new format
//! is just writing an importer + exporter — all editing logic lives once.
//!
//! ```text
//! DOCX         ──┐
//! PPTX         ──┤
//! PDF          ──┤──▶  IR  ──▶  edit  ──▶  export
//! XLSX         ──┤
//! HTML/MD/CSV  ──┘
//! ```

pub mod document;
pub mod elements;
pub mod metadata;

pub use document::Document;
pub use document::Section;
pub use document::Chunk;
pub use elements::*;
pub use metadata::*;
