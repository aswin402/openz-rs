pub mod csv;
pub mod docx;
pub mod html;
pub mod md;
pub mod pdf;
pub mod pdf_forms;
pub mod pptx;
pub mod xlsx;

use crate::tools::opendoc::ir::Document;
use std::path::Path;

/// Load any supported document into the Internal Representation (IR).
///
/// This is the universal entry point. Format is detected from file extension.
pub fn load_to_ir(file_path: &str) -> Result<Document, LoadError> {
    load_to_ir_with_password(file_path, None)
}

fn sniff_zip_format(file: std::fs::File) -> Option<String> {
    let mut archive = zip::ZipArchive::new(file).ok()?;
    for i in 0..archive.len() {
        match archive.by_index(i) {
            Ok(f) => {
                let name = f.name();
                match (
                    name.starts_with("word/"),
                    name.starts_with("xl/"),
                    name == "content.xml",
                    name.starts_with("ppt/"),
                ) {
                    (true, _, _, _) => return Some("docx".to_string()),
                    (_, true, _, _) => return Some("xlsx".to_string()),
                    (_, _, true, _) => return Some("ods".to_string()),
                    (_, _, _, true) => return Some("pptx".to_string()),
                    _ => {}
                }
            }
            Err(_) => {}
        }
    }
    None
}

fn sniff_format(file_path: &str) -> Option<String> {
    let mut file = std::fs::File::open(file_path).ok()?;
    use std::io::Read;
    let mut buf = [0u8; 16];
    let n = file.read(&mut buf).ok()?;
    match n >= 4 {
        false => None,
        true => match &buf[0..4] {
            b"%PDF" => Some("pdf".to_string()),
            b"PK\x03\x04" => sniff_zip_format(file),
            _ => None,
        },
    }
}

fn require_no_office_password(password: Option<&str>, format_label: &str) -> Result<(), LoadError> {
    match password {
        Some(_) => Err(LoadError::ParseError(format!(
            "Password decryption for Office {} is not supported under offline mode due to missing office_crypto library. Encrypted PDFs are fully supported.",
            format_label
        ))),
        None => Ok(()),
    }
}

/// Load any supported document into the Internal Representation (IR) with an optional password.
pub fn load_to_ir_with_password(
    file_path: &str,
    password: Option<&str>,
) -> Result<Document, LoadError> {
    let path = Path::new(file_path);
    let mut ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match matches!(
        ext.as_str(),
        "docx"
            | "doc"
            | "pptx"
            | "ppt"
            | "pdf"
            | "xlsx"
            | "xls"
            | "ods"
            | "xlsb"
            | "md"
            | "markdown"
            | "html"
            | "htm"
            | "csv"
            | "txt"
            | "text"
    ) {
        true => {}
        false => {
            ext = sniff_format(file_path).unwrap_or(ext);
        }
    }

    match ext.as_str() {
        "docx" | "doc" => {
            require_no_office_password(password, "documents (.docx)")?;
            docx::to_ir(file_path)
        }
        "pptx" | "ppt" => {
            require_no_office_password(password, "documents (.pptx)")?;
            pptx::to_ir(file_path)
        }
        "pdf" => {
            let mut doc = pdf::to_ir_with_password(file_path, password)?;
            // Try to attach form field info if available
            doc.metadata.form_fields = pdf_forms::list_form_fields(file_path).ok().map(|f| f.len());
            Ok(doc)
        }
        "xlsx" | "xls" | "ods" | "xlsb" => {
            require_no_office_password(password, "spreadsheets")?;
            xlsx::to_ir(file_path).map_err(LoadError::ParseError)
        }
        "md" | "markdown" => md::to_ir(file_path).map_err(LoadError::ParseError),
        "html" | "htm" => html::to_ir(file_path).map_err(LoadError::ParseError),
        "csv" => csv::to_ir(file_path).map_err(LoadError::ParseError),
        "txt" | "text" => {
            let content = std::fs::read_to_string(file_path)
                .map_err(|e| LoadError::IoError(e.to_string()))?;
            let mut doc = Document::new("txt");
            for line in content.lines() {
                let trimmed = line.trim();
                match trimmed.is_empty() {
                    true => {}
                    false => {
                        doc.paragraphs.push(crate::tools::opendoc::ir::Paragraph::new(
                            trimmed.to_string(),
                        ));
                    }
                }
            }
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
    let file = std::fs::File::open(file_path).map_err(|e| format!("Failed to open file: {e}"))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("Failed to read zip archive: {e}"))?;

    let output_path = std::path::Path::new(output_dir);
    match output_path.exists() {
        true => {}
        false => {
            std::fs::create_dir_all(output_path)
                .map_err(|e| format!("Failed to create output directory: {e}"))?;
        }
    }

    let mut extracted = Vec::new();

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read file from zip: {e}"))?;
        let name = file.name().to_string();

        // We look for word/media/ or ppt/media/
        match (
            name.starts_with("word/media/") || name.starts_with("ppt/media/"),
            file.is_dir(),
        ) {
            (true, false) => {
                // Get the file name itself (e.g. image1.png)
                let file_name = std::path::Path::new(&name)
                    .file_name()
                    .and_then(|f| f.to_str())
                    .unwrap_or("");
                match file_name.is_empty() {
                    true => {}
                    false => {
                        let target_file_path = output_path.join(file_name);
                        let mut outfile = std::fs::File::create(&target_file_path).map_err(|e| {
                            format!("Failed to create output image file {}: {}", file_name, e)
                        })?;
                        std::io::copy(&mut file, &mut outfile).map_err(|e| {
                            format!("Failed to write output image file {}: {}", file_name, e)
                        })?;

                        extracted.push(target_file_path.to_string_lossy().to_string());
                    }
                }
            }
            _ => {}
        }
    }

    Ok(extracted)
}
