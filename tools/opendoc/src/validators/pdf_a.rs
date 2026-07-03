use lopdf::{Document as LopdfDocument, Object};
use std::path::Path;

#[derive(Debug, Clone, serde::Serialize)]
pub struct PdfAValidationResult {
    pub compliant: bool,
    pub level: Option<String>,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

/// Validate a PDF file for PDF/A compliance standards.
pub fn validate_pdf_a(path: &Path) -> Result<PdfAValidationResult, lopdf::Error> {
    let doc = LopdfDocument::load(path)?;
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let mut detected_level = None;

    // 1. Encryption check
    if doc.is_encrypted() {
        errors.push("Document is encrypted. Encryption is forbidden in PDF/A.".to_string());
    }

    // 2. Scan all objects
    let mut has_metadata = false;
    let mut has_output_intent = false;

    for (_, object) in &doc.objects {
        if let Object::Dictionary(ref dict) = object {
            // Check for /Type
            if let Ok(type_name) = dict.get(b"Type") {
                if type_name.as_name().ok() == Some(b"Catalog") {
                    // Check for Metadata
                    if dict.get(b"Metadata").is_ok() {
                        has_metadata = true;
                    }
                    // Check for OutputIntents
                    if let Ok(Object::Array(ref intents)) = dict.get(b"OutputIntents") {
                        if !intents.is_empty() {
                            has_output_intent = true;
                        }
                    }
                } else if type_name.as_name().ok() == Some(b"Font") {
                    // Check if it's embedded or a standard font
                    let mut font_is_embedded = false;
                    
                    // Simple fonts can have /FontDescriptor which has /FontFile, /FontFile2, or /FontFile3
                    if let Ok(descriptor_obj) = dict.get(b"FontDescriptor") {
                        let descriptor = match descriptor_obj {
                            Object::Reference(ref_id) => doc.get_object(*ref_id).ok().and_then(|o| o.as_dict().ok()),
                            Object::Dictionary(d) => Some(d),
                            _ => None,
                        };
                        if let Some(d) = descriptor {
                            if d.get(b"FontFile").is_ok() || d.get(b"FontFile2").is_ok() || d.get(b"FontFile3").is_ok() {
                                font_is_embedded = true;
                            }
                        }
                    }
                    
                    // Type3 fonts are self-contained
                    if let Ok(subtype) = dict.get(b"Subtype") {
                        if subtype.as_name().ok() == Some(b"Type3") {
                            font_is_embedded = true;
                        }
                    }

                    if !font_is_embedded {
                        let font_name = dict.get(b"BaseFont")
                            .ok()
                            .and_then(|o| o.as_name().ok())
                            .map(|n| String::from_utf8_lossy(n).into_owned())
                            .unwrap_or_else(|| "Unknown Font".to_string());
                        errors.push(format!("Font '{}' is not embedded. All fonts must be embedded in PDF/A.", font_name));
                    }
                }
            }

            // Check for forbidden actions (/JS, /JavaScript, /Sound, /Movie, /Launch)
            if let Ok(action_type) = dict.get(b"S") {
                if let Object::Name(ref name) = action_type {
                    if name == b"JavaScript" || name == b"JS" {
                        errors.push("JavaScript action detected. JavaScript is forbidden in PDF/A.".to_string());
                    } else if name == b"Launch" {
                        errors.push("Launch action detected. Launching external programs is forbidden in PDF/A.".to_string());
                    } else if name == b"Sound" {
                        errors.push("Sound action detected. Audio/video content is forbidden in PDF/A.".to_string());
                    } else if name == b"Movie" {
                        errors.push("Movie action detected. Audio/video content is forbidden in PDF/A.".to_string());
                    }
                }
            }
        }
    }

    if !has_metadata {
        errors.push("Metadata stream (XMP) is missing. PDF/A requires document-level XMP metadata.".to_string());
    }

    if !has_output_intent {
        warnings.push("No OutputIntent specified. PDF/A requires Device-independent color spaces or an OutputIntent profile.".to_string());
    }

    // Detect level claimed in XMP metadata
    for (_, object) in &doc.objects {
        if let Object::Stream(ref stream) = object {
            if let Ok(type_name) = stream.dict.get(b"Type") {
                if type_name.as_name().ok() == Some(b"Metadata") {
                    if let Ok(decompressed) = stream.decompressed_content() {
                        let xml_str = String::from_utf8_lossy(&decompressed);
                        if xml_str.contains("pdfaid:part") {
                            let part = xml_str.split("pdfaid:part>")
                                .nth(1)
                                .and_then(|s| s.split('<').next())
                                .unwrap_or("1")
                                .trim();
                            let conformance = xml_str.split("pdfaid:conformance>")
                                .nth(1)
                                .and_then(|s| s.split('<').next())
                                .unwrap_or("B")
                                .trim();
                            detected_level = Some(format!("PDF/A-{}{}", part, conformance.to_uppercase()));
                        }
                    }
                }
            }
        }
    }

    Ok(PdfAValidationResult {
        compliant: errors.is_empty(),
        level: detected_level,
        errors,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pdf_a_validation_lifecycle() {
        let dir = std::env::temp_dir();
        let pdf_path = dir.join("test_pdf_a_compliance.pdf");
        let p = pdf_path.to_str().unwrap();

        // Create a standard non-PDF/A PDF file first
        crate::handlers::pdf::create_pdf(p, "Hello compliance world!", None);

        // Run validation
        let res = validate_pdf_a(&pdf_path).unwrap();
        // The standard created PDF won't have OutputIntents and XMP metadata by default, so it's not compliant.
        assert!(!res.compliant);
        assert!(!res.errors.is_empty());

        let _ = std::fs::remove_file(pdf_path);
    }
}
