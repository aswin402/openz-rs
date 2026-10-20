#[cfg(feature = "ocr")]
mod ocr_impl {
    use crate::tools::opendoc::ir::elements::Paragraph;
    use crate::tools::opendoc::ir::Document;
    use std::fs;
    use std::path::Path;
    use std::process::Command;

    fn run_tesseract_to_string(
        input_path: &Path,
        out_base: &Path,
        txt_path: &Path,
        lang: &str,
        temp_dir: &Path,
        err_prefix: &str,
    ) -> Result<String, String> {
        let output = Command::new("tesseract")
            .arg(input_path)
            .arg(out_base)
            .arg("-l")
            .arg(lang)
            .output();

        match output {
            Ok(out) => match out.status.success() {
                true => Ok(fs::read_to_string(txt_path).unwrap_or_default()),
                false => {
                    let _ = fs::remove_dir_all(temp_dir);
                    Err(format!(
                        "{}: {}",
                        err_prefix,
                        String::from_utf8_lossy(&out.stderr)
                    ))
                }
            },
            Err(e) => {
                let _ = fs::remove_dir_all(temp_dir);
                Err(format!("Tesseract execution error: {}", e))
            }
        }
    }

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

        let mut langs: Vec<String> = output
            .lines()
            .skip(1)
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map(str::to_string)
            .collect();
        match langs.is_empty() {
            true => langs.push("eng".to_string()),
            false => {}
        }
        langs
    }

    pub fn ocr_document(file_path: &str, language: Option<&str>) -> Result<Document, String> {
        match is_ocr_available() {
            false => {
                return Err(
                    "tesseract CLI not found. Please install tesseract-ocr on your system."
                        .to_string(),
                );
            }
            true => {}
        }

        let lang = language.unwrap_or("eng");
        let path = Path::new(file_path);
        match path.exists() {
            false => return Err(format!("File not found: {}", file_path)),
            true => {}
        }

        let ext = path
            .extension()
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
        fs::create_dir_all(&temp_dir)
            .map_err(|e| format!("Failed to create temp directory: {}", e))?;

        let mut extracted_text = String::new();

        match ext.as_str() {
            "pdf" => {
                let pdftoppm_ok = Command::new("which")
                    .arg("pdftoppm")
                    .output()
                    .map(|o| o.status.success())
                    .unwrap_or(false);

                match pdftoppm_ok {
                    false => {
                        let _ = fs::remove_dir_all(&temp_dir);
                        return Err(
                            "pdftoppm (poppler-utils) not found. Required for PDF OCR.".to_string(),
                        );
                    }
                    true => {}
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
                    Ok(out) => match out.status.success() {
                        true => {}
                        false => {
                            let _ = fs::remove_dir_all(&temp_dir);
                            return Err(format!(
                                "pdftoppm failed: {}",
                                String::from_utf8_lossy(&out.stderr)
                            ));
                        }
                    },
                    Err(e) => {
                        let _ = fs::remove_dir_all(&temp_dir);
                        return Err(format!("pdftoppm execution error: {}", e));
                    }
                }

                let mut entries: Vec<_> = match fs::read_dir(&temp_dir) {
                    Ok(read_dir) => read_dir
                        .flatten()
                        .map(|entry| entry.path())
                        .filter(|p| {
                            p.is_file()
                                && p.file_name()
                                    .and_then(|f| f.to_str())
                                    .map(|fname| {
                                        fname.starts_with("page-") && fname.ends_with(".png")
                                    })
                                    .unwrap_or(false)
                        })
                        .collect(),
                    Err(_) => Vec::new(),
                };

                entries.sort_by_key(|p| {
                    let fname = p.file_name().and_then(|f| f.to_str()).unwrap_or("");
                    let num_str: String = fname.chars().filter(|c| c.is_ascii_digit()).collect();
                    num_str.parse::<u32>().unwrap_or(0)
                });

                match entries.is_empty() {
                    true => {
                        let _ = fs::remove_dir_all(&temp_dir);
                        return Err("No pages were rendered from the PDF.".to_string());
                    }
                    false => {}
                }

                for (idx, page_path) in entries.iter().enumerate() {
                    let out_base = temp_dir.join(format!("text-{}", idx));
                    let txt_path = temp_dir.join(format!("text-{}.txt", idx));
                    let content = run_tesseract_to_string(
                        page_path,
                        &out_base,
                        &txt_path,
                        lang,
                        &temp_dir,
                        &format!("Tesseract failed on page {}", idx + 1),
                    )?;
                    extracted_text.push_str(&content);
                    extracted_text.push_str("\n\n--- Page Break ---\n\n");
                }
            }
            "png" | "jpg" | "jpeg" | "bmp" | "tiff" => {
                let out_base = temp_dir.join("text");
                let txt_path = temp_dir.join("text.txt");
                let content = run_tesseract_to_string(
                    path,
                    &out_base,
                    &txt_path,
                    lang,
                    &temp_dir,
                    "Tesseract failed",
                )?;
                extracted_text.push_str(&content);
            }
            _ => {
                let _ = fs::remove_dir_all(&temp_dir);
                return Err(format!("Unsupported file format for OCR: {}. Only PDF and images (PNG, JPG, BMP, TIFF) are supported.", ext));
            }
        }

        let _ = fs::remove_dir_all(&temp_dir);

        let mut text_buf = String::new();
        for line in extracted_text.lines() {
            let trimmed = line.trim();
            match trimmed.is_empty() {
                false => {
                    doc.paragraphs.push(Paragraph::new(trimmed));
                    text_buf.push_str(trimmed);
                    text_buf.push('\n');
                }
                true => {}
            }
        }
        doc.text = (!text_buf.is_empty()).then_some(text_buf);

        Ok(doc)
    }
}

#[cfg(not(feature = "ocr"))]
mod ocr_impl {
    use crate::tools::opendoc::ir::Document;

    pub fn ocr_document(_file_path: &str, _language: Option<&str>) -> Result<Document, String> {
        Err(
            "OCR feature not enabled. Build with --features ocr to enable.\n\
             See https://github.com/aswin402/opendoc-mcp#ocr for setup instructions."
                .to_string(),
        )
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
    pub psm: i32,         // Tesseract page segmentation mode
    pub preprocess: bool, // Apply image preprocessing (deskew, denoise)
}

impl Default for OcrConfig {
    fn default() -> Self {
        Self {
            language: "eng".to_string(),
            dpi: 300,
            psm: 3, // Fully automatic page segmentation
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
        match is_ocr_available() {
            false => return,
            true => {}
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
        match &res {
            Err(e) => println!("OCR TEST ERROR: {}", e),
            Ok(_) => {}
        }
        assert!(res.is_ok());

        let _ = std::fs::remove_file(p);
    }
}
