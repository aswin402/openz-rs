#[cfg(feature = "ocr")]
mod ocr_impl {
    use crate::ir::Document;
    use crate::ir::elements::Paragraph;
    use std::process::Command;
    use std::fs;
    use std::path::Path;

    pub fn is_ocr_available() -> bool {
        Command::new("which")
            .arg("tesseract")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    pub fn available_languages() -> Vec<String> {
        let output = match Command::new("tesseract").arg("--list-langs").output() {
            Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
            Err(_) => return vec!["eng".to_string()],
        };
        
        let mut langs = Vec::new();
        let mut lines = output.lines();
        let _ = lines.next(); // Skip header
        for line in lines {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                langs.push(trimmed.to_string());
            }
        }
        if langs.is_empty() {
            langs.push("eng".to_string());
        }
        langs
    }

    pub fn ocr_document(file_path: &str, language: Option<&str>) -> Result<Document, String> {
        if !is_ocr_available() {
            return Err("tesseract CLI not found. Please install tesseract-ocr on your system.".to_string());
        }

        let lang = language.unwrap_or("eng");
        let path = Path::new(file_path);
        if !path.exists() {
            return Err(format!("File not found: {}", file_path));
        }

        let ext = path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let mut doc = Document::new(&ext);
        doc.path = Some(file_path.to_string());

        let temp_dir = std::env::temp_dir().join(format!(
            "opendoc_ocr_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        if let Err(e) = fs::create_dir_all(&temp_dir) {
            return Err(format!("Failed to create temp directory: {}", e));
        }

        let mut extracted_text = String::new();

        if ext == "pdf" {
            let pdftoppm_ok = Command::new("which")
                .arg("pdftoppm")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);

            if !pdftoppm_ok {
                let _ = fs::remove_dir_all(&temp_dir);
                return Err("pdftoppm (poppler-utils) not found. Required for PDF OCR.".to_string());
            }

            let img_prefix = temp_dir.join("page");
            let output = Command::new("pdftoppm")
                .arg("-png")
                .arg("-r")
                .arg("150")
                .arg(file_path)
                .arg(&img_prefix)
                .output();

            match output {
                Ok(out) if out.status.success() => {}
                Ok(out) => {
                    let _ = fs::remove_dir_all(&temp_dir);
                    return Err(format!("pdftoppm failed: {}", String::from_utf8_lossy(&out.stderr)));
                }
                Err(e) => {
                    let _ = fs::remove_dir_all(&temp_dir);
                    return Err(format!("pdftoppm execution error: {}", e));
                }
            }

            let mut entries = Vec::new();
            if let Ok(read_dir) = fs::read_dir(&temp_dir) {
                for entry in read_dir.flatten() {
                    let p = entry.path();
                    if p.is_file() {
                        if let Some(fname) = p.file_name().and_then(|f| f.to_str()) {
                            if fname.starts_with("page-") && fname.ends_with(".png") {
                                entries.push(p);
                            }
                        }
                    }
                }
            }
            
            entries.sort_by_key(|p| {
                let fname = p.file_name().and_then(|f| f.to_str()).unwrap_or("");
                let num_str: String = fname.chars().filter(|c| c.is_ascii_digit()).collect();
                num_str.parse::<u32>().unwrap_or(0)
            });

            if entries.is_empty() {
                let _ = fs::remove_dir_all(&temp_dir);
                return Err("No pages were rendered from the PDF.".to_string());
            }

            for (idx, page_path) in entries.iter().enumerate() {
                let out_base = temp_dir.join(format!("text-{}", idx));
                let output = Command::new("tesseract")
                    .arg(page_path)
                    .arg(&out_base)
                    .arg("-l")
                    .arg(lang)
                    .output();

                match output {
                    Ok(out) if out.status.success() => {
                        let txt_path = temp_dir.join(format!("text-{}.txt", idx));
                        if let Ok(content) = fs::read_to_string(&txt_path) {
                            extracted_text.push_str(&content);
                            extracted_text.push_str("\n\n--- Page Break ---\n\n");
                        }
                    }
                    Ok(out) => {
                        let _ = fs::remove_dir_all(&temp_dir);
                        return Err(format!("Tesseract failed on page {}: {}", idx + 1, String::from_utf8_lossy(&out.stderr)));
                    }
                    Err(e) => {
                        let _ = fs::remove_dir_all(&temp_dir);
                        return Err(format!("Tesseract execution error: {}", e));
                    }
                }
            }
        } else if ext == "png" || ext == "jpg" || ext == "jpeg" || ext == "bmp" || ext == "tiff" {
            let out_base = temp_dir.join("text");
            let output = Command::new("tesseract")
                .arg(file_path)
                .arg(&out_base)
                .arg("-l")
                .arg(lang)
                .output();

            match output {
                Ok(out) if out.status.success() => {
                    let txt_path = temp_dir.join("text.txt");
                    if let Ok(content) = fs::read_to_string(&txt_path) {
                        extracted_text.push_str(&content);
                    }
                }
                Ok(out) => {
                    let _ = fs::remove_dir_all(&temp_dir);
                    return Err(format!("Tesseract failed: {}", String::from_utf8_lossy(&out.stderr)));
                }
                Err(e) => {
                    let _ = fs::remove_dir_all(&temp_dir);
                    return Err(format!("Tesseract execution error: {}", e));
                }
            }
        } else {
            let _ = fs::remove_dir_all(&temp_dir);
            return Err(format!("Unsupported file format for OCR: {}. Only PDF and images (PNG, JPG, BMP, TIFF) are supported.", ext));
        }

        let _ = fs::remove_dir_all(&temp_dir);

        let mut text_buf = String::new();
        for line in extracted_text.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                doc.paragraphs.push(Paragraph::new(trimmed));
                text_buf.push_str(trimmed);
                text_buf.push('\n');
            }
        }
        if !text_buf.is_empty() {
            doc.text = Some(text_buf);
        }

        Ok(doc)
    }
}

#[cfg(not(feature = "ocr"))]
mod ocr_impl {
    use crate::ir::Document;

    pub fn ocr_document(_file_path: &str, _language: Option<&str>) -> Result<Document, String> {
        Err("OCR feature not enabled. Build with --features ocr to enable.\n\
             See https://github.com/aswin402/opendoc-mcp#ocr for setup instructions.".to_string())
    }

    pub fn is_ocr_available() -> bool {
        false
    }

    pub fn available_languages() -> Vec<String> {
        vec![]
    }
}

pub use ocr_impl::*;

/// OCR configuration
#[derive(Debug, Clone)]
pub struct OcrConfig {
    pub language: String,
    pub dpi: u32,
    pub psm: i32,          // Tesseract page segmentation mode
    pub preprocess: bool,  // Apply image preprocessing (deskew, denoise)
}

impl Default for OcrConfig {
    fn default() -> Self {
        Self {
            language: "eng".to_string(),
            dpi: 300,
            psm: 3,  // Fully automatic page segmentation
            preprocess: true,
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    #[cfg(feature = "ocr")]
    fn test_ocr_image_lifecycle() {
        use super::*;
        if !is_ocr_available() {
            return;
        }

        let dir = std::env::temp_dir();
        let path = dir.join("test_ocr_pixel.bmp");
        let p = path.to_str().unwrap();

        // 1x1 uncompressed BMP white pixel image
        let bmp_data: Vec<u8> = vec![
            0x42, 0x4D, // BM magic
            0x3A, 0x00, 0x00, 0x00, // file size (58 bytes)
            0x00, 0x00, 0x00, 0x00, // reserved
            0x36, 0x00, 0x00, 0x00, // offset to pixel data (54 bytes)
            0x28, 0x00, 0x00, 0x00, // header size (40 bytes)
            0x01, 0x00, 0x00, 0x00, // width (1 pixel)
            0x01, 0x00, 0x00, 0x00, // height (1 pixel)
            0x01, 0x00, // planes (1)
            0x18, 0x00, // bits per pixel (24 bit color)
            0x00, 0x00, 0x00, 0x00, // compression (0 = none)
            0x04, 0x00, 0x00, 0x00, // image size (4 bytes padded)
            0x13, 0x0B, 0x00, 0x00, // X pixels per meter
            0x13, 0x0B, 0x00, 0x00, // Y pixels per meter
            0x00, 0x00, 0x00, 0x00, // colors in color table
            0x00, 0x00, 0x00, 0x00, // important colors
            0xFF, 0xFF, 0xFF, 0x00, // pixel data: white pixel + padding
        ];
        std::fs::write(p, &bmp_data).unwrap();

        let res = ocr_document(p, None);
        if let Err(ref e) = res {
            println!("OCR TEST ERROR: {}", e);
        }
        assert!(res.is_ok());

        let _ = std::fs::remove_file(p);
    }
}
