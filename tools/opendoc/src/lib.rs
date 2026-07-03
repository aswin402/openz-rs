/// Internal Document Representation — universal format-agnostic model
pub mod ir;

/// Document editing engine — all operations work on IR
pub mod engine;

/// Format-specific handlers (import/export from/to IR)
pub mod handlers;

/// Cross-format conversion pipeline
pub mod converters;

/// Document validation
pub mod validators;

/// Batch processing (parallel via rayon)
pub mod batch;

/// OCR pipeline (feature-gated internally)
pub mod ocr;

/// MCP server
#[cfg(feature = "server")]
pub mod server;

/// CLI interface
#[cfg(feature = "cli")]
pub mod cli;



/// Security and path validation
pub mod security;
