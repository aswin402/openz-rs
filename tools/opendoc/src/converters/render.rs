use std::process::Command;
use std::fs;
use std::path::Path;

/// Render pages of a PDF, image, or office document (DOCX, PPTX, XLSX, HTML) into PNG screenshots.
/// For Office/HTML documents, it first converts to a temporary PDF via LibreOffice headless.
pub fn render_document_pages(
    file_path: &str,
    output_dir: &str,
    dpi: Option<u32>,
    pages: Option<Vec<u32>>,
) -> Result<Vec<String>, String> {
    let source_path = Path::new(file_path);
    if !source_path.exists() {
        return Err(format!("File not found: {}", file_path));
    }

    let ext = source_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let target_dir = Path::new(output_dir);
    if !target_dir.exists() {
        fs::create_dir_all(target_dir).map_err(|e| format!("Failed to create output directory: {}", e))?;
    }

    let dpi_val = dpi.unwrap_or(150);

    // If it's already an image, copy it and return its path
    if ext == "png" || ext == "jpg" || ext == "jpeg" || ext == "bmp" || ext == "tiff" {
        let dest_path = target_dir.join(source_path.file_name().unwrap());
        fs::copy(source_path, &dest_path).map_err(|e| format!("Failed to copy image: {}", e))?;
        return Ok(vec![dest_path.to_string_lossy().to_string()]);
    }

    // Determine the PDF representation path.
    // If it's a PDF, use it directly. Otherwise, convert to PDF via LibreOffice headless.
    let temp_pdf_dir = std::env::temp_dir().join(format!(
        "opendoc_render_pdf_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    let pdf_path = if ext == "pdf" {
        source_path.to_path_buf()
    } else if ext == "docx" || ext == "doc" || ext == "pptx" || ext == "ppt" || ext == "xlsx" || ext == "xls" || ext == "html" || ext == "htm" {
        // Convert to PDF via LibreOffice headless
        fs::create_dir_all(&temp_pdf_dir).map_err(|e| format!("Failed to create temp PDF directory: {}", e))?;

        let output = Command::new("soffice")
            .arg("--headless")
            .arg("--convert-to")
            .arg("pdf")
            .arg("--outdir")
            .arg(&temp_pdf_dir)
            .arg(file_path)
            .output();

        match output {
            Ok(out) if out.status.success() => {
                let file_stem = source_path.file_stem().unwrap().to_str().unwrap();
                let generated_pdf = temp_pdf_dir.join(format!("{}.pdf", file_stem));
                if !generated_pdf.exists() {
                    let _ = fs::remove_dir_all(&temp_pdf_dir);
                    return Err("soffice completed but output PDF was not found.".to_string());
                }
                generated_pdf
            }
            Ok(out) => {
                let _ = fs::remove_dir_all(&temp_pdf_dir);
                return Err(format!("LibreOffice conversion failed: {}", String::from_utf8_lossy(&out.stderr)));
            }
            Err(e) => {
                let _ = fs::remove_dir_all(&temp_pdf_dir);
                return Err(format!("soffice execution error: {}", e));
            }
        }
    } else {
        return Err(format!("Unsupported format for rendering: {}", ext));
    };

    // Render PDF pages to PNG using pdftoppm
    let file_stem = source_path.file_stem().unwrap().to_str().unwrap();
    let img_prefix = target_dir.join(format!("{}_page", file_stem));

    let mut cmd = Command::new("pdftoppm");
    cmd.arg("-png")
       .arg("-r")
       .arg(dpi_val.to_string());

    let mut rendered_files = Vec::new();

    if let Some(ref page_nums) = pages {
        for &p in page_nums {
            let single_prefix = target_dir.join(format!("{}_page_{}", file_stem, p));
            let output = Command::new("pdftoppm")
                .arg("-png")
                .arg("-r")
                .arg(dpi_val.to_string())
                .arg("-f")
                .arg(p.to_string())
                .arg("-l")
                .arg(p.to_string())
                .arg(&pdf_path)
                .arg(&single_prefix)
                .output();

            match output {
                Ok(out) if out.status.success() => {
                    if let Ok(entries) = fs::read_dir(target_dir) {
                        for entry in entries.flatten() {
                            let path = entry.path();
                            if path.is_file() {
                                if let Some(fname) = path.file_name().and_then(|f| f.to_str()) {
                                    if fname.starts_with(&format!("{}_page_{}-", file_stem, p)) && fname.ends_with(".png") {
                                        rendered_files.push(path.to_string_lossy().to_string());
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(out) => {
                    if ext != "pdf" {
                        let _ = fs::remove_dir_all(&temp_pdf_dir);
                    }
                    return Err(format!("pdftoppm failed on page {}: {}", p, String::from_utf8_lossy(&out.stderr)));
                }
                Err(e) => {
                    if ext != "pdf" {
                        let _ = fs::remove_dir_all(&temp_pdf_dir);
                    }
                    return Err(format!("pdftoppm execution error: {}", e));
                }
            }
        }
    } else {
        // Render all pages
        let output = cmd.arg(&pdf_path)
                        .arg(&img_prefix)
                        .output();

        match output {
            Ok(out) if out.status.success() => {
                if let Ok(entries) = fs::read_dir(target_dir) {
                    let prefix_str = format!("{}_page-", file_stem);
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_file() {
                            if let Some(fname) = path.file_name().and_then(|f| f.to_str()) {
                                if fname.starts_with(&prefix_str) && fname.ends_with(".png") {
                                    rendered_files.push(path.to_string_lossy().to_string());
                                }
                            }
                        }
                    }
                }
            }
            Ok(out) => {
                if ext != "pdf" {
                    let _ = fs::remove_dir_all(&temp_pdf_dir);
                }
                return Err(format!("pdftoppm failed: {}", String::from_utf8_lossy(&out.stderr)));
            }
            Err(e) => {
                if ext != "pdf" {
                    let _ = fs::remove_dir_all(&temp_pdf_dir);
                }
                return Err(format!("pdftoppm execution error: {}", e));
            }
        }
    }

    // Clean up temporary PDF directory if it was created
    if ext != "pdf" {
        let _ = fs::remove_dir_all(&temp_pdf_dir);
    }

    // Sort files numerically to guarantee correct page order in response
    rendered_files.sort_by_key(|f| {
        let fname = Path::new(f).file_name().and_then(|x| x.to_str()).unwrap_or("");
        let num_str: String = fname.chars().filter(|c| c.is_ascii_digit()).collect();
        num_str.parse::<u32>().unwrap_or(0)
    });

    Ok(rendered_files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_image_lifecycle() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_render_pixel.bmp");
        let p = path.to_str().unwrap();

        // 1x1 uncompressed BMP white pixel image
        let bmp_data: Vec<u8> = vec![
            0x42, 0x4D, 0x3A, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x36, 0x00, 0x00, 0x00, 0x28, 0x00,
            0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00,
            0x00, 0x00, 0x01, 0x00, 0x18, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x13, 0x0B,
            0x00, 0x00, 0x13, 0x0B, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xFF,
            0xFF, 0x00,
        ];
        std::fs::write(p, &bmp_data).unwrap();

        let out_dir = dir.join("test_render_out");
        let res = render_document_pages(p, out_dir.to_str().unwrap(), None, None);
        assert!(res.is_ok());

        let files = res.unwrap();
        assert_eq!(files.len(), 1);
        assert!(Path::new(&files[0]).exists());

        let _ = std::fs::remove_file(p);
        let _ = std::fs::remove_dir_all(out_dir);
    }
}
