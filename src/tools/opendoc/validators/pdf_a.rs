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
    match doc.is_encrypted() {
        true => errors.push("Document is encrypted. Encryption is forbidden in PDF/A.".to_string()),
        false => {}
    }

    // 2. Scan all objects
    let mut has_metadata = false;
    let mut has_output_intent = false;

    for object in doc.objects.values() {
        let dict = match object {
            Object::Dictionary(ref d) => d,
            _ => continue,
        };

        // Check for /Type
        match dict.get(b"Type").ok().and_then(|t| t.as_name().ok()) {
            Some(b"Catalog") => {
                has_metadata = has_metadata || dict.get(b"Metadata").is_ok();
                has_output_intent = has_output_intent
                    || match dict.get(b"OutputIntents") {
                        Ok(Object::Array(ref intents)) => !intents.is_empty(),
                        _ => false,
                    };
            }
            Some(b"Font") => {
                // Simple fonts can have /FontDescriptor which has /FontFile, /FontFile2, or /FontFile3
                let descriptor_embedded = dict
                    .get(b"FontDescriptor")
                    .ok()
                    .and_then(|descriptor_obj| match descriptor_obj {
                        Object::Reference(ref_id) => {
                            doc.get_object(*ref_id).ok().and_then(|o| o.as_dict().ok())
                        }
                        Object::Dictionary(d) => Some(d),
                        _ => None,
                    })
                    .map(|d| {
                        d.get(b"FontFile").is_ok()
                            || d.get(b"FontFile2").is_ok()
                            || d.get(b"FontFile3").is_ok()
                    })
                    .unwrap_or(false);

                // Type3 fonts are self-contained
                let is_type3 = dict.get(b"Subtype").ok().and_then(|s| s.as_name().ok())
                    == Some(b"Type3");

                match descriptor_embedded || is_type3 {
                    true => {}
                    false => {
                        let font_name = dict
                            .get(b"BaseFont")
                            .ok()
                            .and_then(|o| o.as_name().ok())
                            .map(|n| String::from_utf8_lossy(n).into_owned())
                            .unwrap_or_else(|| "Unknown Font".to_string());
                        errors.push(format!(
                            "Font '{}' is not embedded. All fonts must be embedded in PDF/A.",
                            font_name
                        ));
                    }
                }
            }
            _ => {}
        }

        // Check for forbidden actions (/JS, /JavaScript, /Sound, /Movie, /Launch)
        match dict.get(b"S") {
            Ok(Object::Name(ref name)) => match name.as_slice() {
                b"JavaScript" | b"JS" => errors.push(
                    "JavaScript action detected. JavaScript is forbidden in PDF/A.".to_string(),
                ),
                b"Launch" => errors.push(
                    "Launch action detected. Launching external programs is forbidden in PDF/A."
                        .to_string(),
                ),
                b"Sound" => errors.push(
                    "Sound action detected. Audio/video content is forbidden in PDF/A.".to_string(),
                ),
                b"Movie" => errors.push(
                    "Movie action detected. Audio/video content is forbidden in PDF/A.".to_string(),
                ),
                _ => {}
            },
            _ => {}
        }
    }

    // Re-check OutputIntents without a match guard
    has_output_intent = doc.objects.values().any(|object| match object {
        Object::Dictionary(ref dict) => {
            dict.get(b"Type").ok().and_then(|t| t.as_name().ok()) == Some(b"Catalog")
                && match dict.get(b"OutputIntents") {
                    Ok(Object::Array(ref intents)) => !intents.is_empty(),
                    _ => false,
                }
        }
        _ => false,
    });

    match has_metadata {
        true => {}
        false => errors.push(
            "Metadata stream (XMP) is missing. PDF/A requires document-level XMP metadata."
                .to_string(),
        ),
    }

    match has_output_intent {
        true => {}
        false => warnings.push("No OutputIntent specified. PDF/A requires Device-independent color spaces or an OutputIntent profile.".to_string()),
    }

    // Detect level claimed in XMP metadata
    for object in doc.objects.values() {
        let stream = match object {
            Object::Stream(ref s) => s,
            _ => continue,
        };
        let is_metadata = stream
            .dict
            .get(b"Type")
            .ok()
            .and_then(|t| t.as_name().ok())
            == Some(b"Metadata");
        match (is_metadata, stream.decompressed_content()) {
            (true, Ok(decompressed)) => {
                let xml_str = String::from_utf8_lossy(&decompressed);
                match xml_str.contains("pdfaid:part") {
                    true => {
                        let part = xml_str
                            .split("pdfaid:part>")
                            .nth(1)
                            .and_then(|s| s.split('<').next())
                            .unwrap_or("1")
                            .trim();
                        let conformance = xml_str
                            .split("pdfaid:conformance>")
                            .nth(1)
                            .and_then(|s| s.split('<').next())
                            .unwrap_or("B")
                            .trim();
                        detected_level =
                            Some(format!("PDF/A-{}{}", part, conformance.to_uppercase()));
                    }
                    false => {}
                }
            }
            _ => {}
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
        crate::tools::opendoc::handlers::pdf::create_pdf(p, "Hello compliance world!", None);

        // Run validation
        let res = validate_pdf_a(&pdf_path).unwrap();
        // The standard created PDF won't have OutputIntents and XMP metadata by default, so it's not compliant.
        assert!(!res.compliant);
        assert!(!res.errors.is_empty());

        let _ = std::fs::remove_file(pdf_path);
    }
}
