pub mod csv;
pub mod docx;
pub mod html;
pub mod md;
pub mod pdf;
pub mod pdf_forms;
pub mod pptx;
pub mod xlsx;

use crate::ir::Document;
use std::path::Path;

/// Load any supported document into the Internal Representation (IR).
///
/// This is the universal entry point. Format is detected from file extension.
pub fn load_to_ir(file_path: &str) -> Result<Document, LoadError> {
    load_to_ir_with_password(file_path, None)
}

/// Load any supported document into the Internal Representation (IR) with an optional password.
pub fn load_to_ir_with_password(file_path: &str, password: Option<&str>) -> Result<Document, LoadError> {
    let path = Path::new(file_path);
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "docx" | "doc" => {
            if password.is_some() {
                return Err(LoadError::ParseError("Password decryption for Office documents (.docx) is not supported under offline mode due to missing office_crypto library. Encrypted PDFs are fully supported.".to_string()));
            }
            let doc = docx::to_ir(file_path)?;
            Ok(doc)
        }
        "pptx" | "ppt" => {
            if password.is_some() {
                return Err(LoadError::ParseError("Password decryption for Office documents (.pptx) is not supported under offline mode due to missing office_crypto library. Encrypted PDFs are fully supported.".to_string()));
            }
            let doc = pptx::to_ir(file_path)?;
            Ok(doc)
        }
        "pdf" => {
            let mut doc = pdf::to_ir_with_password(file_path, password)?;
            // Try to attach form field info if available
            if let Ok(fields) = pdf_forms::list_form_fields(file_path) {
                doc.metadata.form_fields = Some(fields.len());
            }
            Ok(doc)
        }
        "xlsx" | "xls" => {
            if password.is_some() {
                return Err(LoadError::ParseError("Password decryption for Office documents (.xlsx) is not supported under offline mode due to missing office_crypto library. Encrypted PDFs are fully supported.".to_string()));
            }
            let doc = xlsx::to_ir(file_path)
                .map_err(LoadError::ParseError)?;
            Ok(doc)
        }
        "md" | "markdown" => {
            let doc = md::to_ir(file_path)
                .map_err(LoadError::ParseError)?;
            Ok(doc)
        }
        "html" | "htm" => {
            let doc = html::to_ir(file_path)
                .map_err(LoadError::ParseError)?;
            Ok(doc)
        }
        "csv" => {
            let doc = csv::to_ir(file_path)
                .map_err(LoadError::ParseError)?;
            Ok(doc)
        }
        "txt" | "text" => {
            let content = std::fs::read_to_string(file_path)
                .map_err(|e| LoadError::IoError(e.to_string()))?;
            let mut doc = Document::new("txt");
            doc.text = Some(content);
            Ok(doc)
        }
        _ => Err(LoadError::UnsupportedFormat(ext)),
    }
}

/// Errors that can occur when loading a document into IR.
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("Unsupported format: {0}")]
    UnsupportedFormat(String),
    #[error("IO error: {0}")]
    IoError(String),
    #[error("Parse error: {0}")]
    ParseError(String),
}

/// Extract embedded images from a zip-based Office document (DOCX, PPTX).
pub fn extract_images_from_zip(file_path: &str, output_dir: &str) -> Result<Vec<String>, String> {
    let file = std::fs::File::open(file_path)
        .map_err(|e| format!("Failed to open file: {e}"))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| format!("Failed to read zip archive: {e}"))?;

    let output_path = std::path::Path::new(output_dir);
    if !output_path.exists() {
        std::fs::create_dir_all(output_path)
            .map_err(|e| format!("Failed to create output directory: {e}"))?;
    }

    let mut extracted = Vec::new();

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)
            .map_err(|e| format!("Failed to read file from zip: {e}"))?;
        let name = file.name().to_string();
        
        // We look for word/media/ or ppt/media/
        if name.starts_with("word/media/") || name.starts_with("ppt/media/") {
            if file.is_dir() {
                continue;
            }
            // Get the file name itself (e.g. image1.png)
            let file_name = std::path::Path::new(&name)
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or("");
            if file_name.is_empty() {
                continue;
            }
            let target_file_path = output_path.join(file_name);
            let mut outfile = std::fs::File::create(&target_file_path)
                .map_err(|e| format!("Failed to create output image file {}: {}", file_name, e))?;
            std::io::copy(&mut file, &mut outfile)
                .map_err(|e| format!("Failed to write output image file {}: {}", file_name, e))?;
            
            extracted.push(target_file_path.to_string_lossy().to_string());
        }
    }

    Ok(extracted)
}
