use rdocx::Document;


/// Create a new DOCX document and save to a file path.
/// Returns the path to the created file on success.
pub fn create_document(file_path: &str, title: Option<&str>) -> String {
    let mut doc = Document::new();
    doc.set_author("Opendoc MCP");

    if let Some(t) = title {
        let mut p = doc.add_paragraph("");
        p.add_run(t).bold(true).size(24.0);
    }

    match doc.save(file_path) {
        Ok(_) => serde_json::json!({"success": true, "path": file_path, "format": "docx"}).to_string(),
        Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
    }
}

/// Open a DOCX file and return its metadata (paragraphs, tables, content count, author).
pub fn open_document(file_path: &str) -> String {
    match Document::open(file_path) {
        Ok(doc) => {
            let info = serde_json::json!({
                "path": file_path,
                "paragraphs": doc.paragraph_count(),
                "tables": doc.table_count(),
                "content_items": doc.content_count(),
                "title": doc.title(),
                "author": doc.author(),
            });
            serde_json::to_string_pretty(&info).unwrap_or_default()
        }
        Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
    }
}

/// Append a formatted paragraph to an existing DOCX document.
pub fn add_paragraph(
    file_path: &str,
    text: &str,
    bold: Option<bool>,
    italic: Option<bool>,
    underline: Option<bool>,
    font_size: Option<f32>,
    font_family: Option<String>,
    color: Option<String>,
    highlight: Option<String>,
    alignment: Option<String>,
    shading: Option<String>,
    line_spacing: Option<f64>,
    keep_with_next: Option<bool>,
    keep_together: Option<bool>,
    page_break_before: Option<bool>,
) -> String {
    let mut doc = match Document::open(file_path) {
        Ok(d) => d,
        Err(e) => return serde_json::json!({"error": e.to_string()}).to_string(),
    };

    let mut p = doc.add_paragraph("");

    if let Some(align_str) = alignment {
        let align = match align_str.to_lowercase().as_str() {
            "left" => Some(rdocx::Alignment::Left),
            "center" => Some(rdocx::Alignment::Center),
            "right" => Some(rdocx::Alignment::Right),
            "justify" => Some(rdocx::Alignment::Justify),
            _ => None,
        };
        if let Some(a) = align {
            p = p.alignment(a);
        }
    }

    if let Some(ref shd) = shading {
        p = p.shading(shd);
    }
    if let Some(spacing) = line_spacing {
        p = p.line_spacing(spacing);
    }
    if keep_with_next.unwrap_or(false) {
        p = p.keep_with_next(true);
    }
    if keep_together.unwrap_or(false) {
        p = p.keep_together(true);
    }
    if page_break_before.unwrap_or(false) {
        p = p.page_break_before(true);
    }

    let mut run = p.add_run(text);
    if bold.unwrap_or(false) {
        run = run.bold(true);
    }
    if italic.unwrap_or(false) {
        run = run.italic(true);
    }
    if underline.unwrap_or(false) {
        run = run.underline(true);
    }
    if let Some(sz) = font_size {
        run = run.size(sz as f64);
    }
    if let Some(ref family) = font_family {
        run = run.font(family);
    }
    if let Some(ref col) = color {
        run = run.color(col);
    }
    if let Some(ref high) = highlight {
        let _ = run.highlight(high);
    }

    match doc.save(file_path) {
        Ok(_) => serde_json::json!({"success": true, "path": file_path, "text_length": text.len()}).to_string(),
        Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
    }
}

/// Insert a table with headers and data rows into a DOCX document with optional styling.
pub fn add_table(
    file_path: &str,
    headers: &[String],
    data: &[Vec<String>],
    width_pct: Option<f64>,
    alignment: Option<String>,
    border_style: Option<String>,
    border_size: Option<u32>,
    border_color: Option<String>,
    shading_header: Option<String>,
    shading_data: Option<String>,
    cant_split: Option<bool>,
) -> String {
    let mut doc = match Document::open(file_path) {
        Ok(d) => d,
        Err(e) => return serde_json::json!({"error": e.to_string()}).to_string(),
    };

    let rows = data.len() + 1; // +1 for header
    let cols = headers.len().max(if data.is_empty() { 0 } else { data[0].len() });

    let mut table = doc.add_table(rows, cols);

    if let Some(pct) = width_pct {
        table = table.width_pct(pct);
    }

    if let Some(align_str) = alignment {
        let align = match align_str.to_lowercase().as_str() {
            "left" => Some(rdocx::Alignment::Left),
            "center" => Some(rdocx::Alignment::Center),
            "right" => Some(rdocx::Alignment::Right),
            "justify" => Some(rdocx::Alignment::Justify),
            _ => None,
        };
        if let Some(a) = align {
            table = table.alignment(a);
        }
    }

    if let Some(bs_str) = border_style {
        let bs = match bs_str.to_lowercase().as_str() {
            "none" => Some(rdocx::BorderStyle::None),
            "single" => Some(rdocx::BorderStyle::Single),
            "thick" => Some(rdocx::BorderStyle::Thick),
            "double" => Some(rdocx::BorderStyle::Double),
            "dotted" => Some(rdocx::BorderStyle::Dotted),
            "dashed" => Some(rdocx::BorderStyle::Dashed),
            "dotdash" => Some(rdocx::BorderStyle::DotDash),
            "wave" => Some(rdocx::BorderStyle::Wave),
            _ => None,
        };
        if let Some(b) = bs {
            table = table.borders(b, border_size.unwrap_or(4), &border_color.clone().unwrap_or_else(|| "CCCCCC".to_string()));
        }
    }

    // Set headers
    for (col, header) in headers.iter().enumerate() {
        if let Some(mut cell) = table.cell(0, col) {
            cell.set_text(header);
            if let Some(ref color) = shading_header {
                let _ = cell.shading(color);
            }
        }
    }

    // Set data
    for (row_idx, row_data) in data.iter().enumerate() {
        let current_row_idx = row_idx + 1;
        
        // Apply row property cant_split if requested
        if cant_split.unwrap_or(false) {
            if let Some(row) = table.row(current_row_idx) {
                let _ = row.cant_split();
            }
        }

        for (col_idx, cell_text) in row_data.iter().enumerate() {
            if col_idx < cols {
                if let Some(mut cell) = table.cell(current_row_idx, col_idx) {
                    cell.set_text(cell_text);
                    if let Some(ref color) = shading_data {
                        let _ = cell.shading(color);
                    }
                }
            }
        }
    }

    match doc.save(file_path) {
        Ok(_) => serde_json::json!({"success": true, "rows": rows, "cols": cols}).to_string(),
        Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
    }
}

/// Find and replace text in a DOCX document using regex pattern matching.
pub fn find_replace_text(file_path: &str, find: &str, replace: &str) -> String {
    let mut doc = match Document::open(file_path) {
        Ok(d) => d,
        Err(e) => return serde_json::json!({"error": e.to_string()}).to_string(),
    };

    match doc.replace_regex(find, replace) {
        Ok(count) => match doc.save(file_path) {
            Ok(_) => serde_json::json!({"success": true, "replacements": count}).to_string(),
            Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
        },
        Err(e) => serde_json::json!({"error": format!("Replace error: {e}")}).to_string(),
    }
}

/// Convert a DOCX document to PDF using the rdocx layout engine.
pub fn to_pdf(source: &str, output: &str) -> String {
    let doc = match Document::open(source) {
        Ok(d) => d,
        Err(e) => return serde_json::json!({"error": e.to_string()}).to_string(),
    };

    match doc.to_pdf() {
        Ok(pdf_bytes) => {
            match std::fs::write(output, &pdf_bytes) {
                Ok(_) => serde_json::json!({
                    "success": true,
                    "source": source,
                    "output": output,
                    "size_bytes": pdf_bytes.len()
                }).to_string(),
                Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
            }
        }
        Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
    }
}

/// Export a DOCX document to Markdown format.
pub fn to_markdown(source: &str, output: &str) -> String {
    let doc = match Document::open(source) {
        Ok(d) => d,
        Err(e) => return serde_json::json!({"error": e.to_string()}).to_string(),
    };

    let md = doc.to_markdown();
    match std::fs::write(output, &md) {
        Ok(_) => serde_json::json!({
            "success": true,
            "source": source,
            "output": output,
            "size_bytes": md.len()
        }).to_string(),
        Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
    }
}

/// Embed an image into a DOCX document.
pub fn add_image(
    file_path: &str,
    image_path: &str,
    width_inches: Option<f64>,
    height_inches: Option<f64>,
) -> String {
    let mut doc = match Document::open(file_path) {
        Ok(d) => d,
        Err(e) => return serde_json::json!({"error": e.to_string()}).to_string(),
    };

    let img_bytes = match std::fs::read(image_path) {
        Ok(bytes) => bytes,
        Err(e) => {
            return serde_json::json!({
                "error": format!("Cannot read image file '{}': {}", image_path, e),
                "suggestion": "Check that the image path exists and is readable."
            })
            .to_string();
        }
    };

    let path = std::path::Path::new(image_path);
    let filename = path.file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("image.png");

    let w = rdocx::Length::inches(width_inches.unwrap_or(2.0));
    let h = rdocx::Length::inches(height_inches.unwrap_or(1.5));

    doc.add_picture(&img_bytes, filename, w, h);

    match doc.save(file_path) {
        Ok(_) => serde_json::json!({
            "success": true,
            "path": file_path,
            "image": image_path,
            "width_inches": width_inches.unwrap_or(2.0),
            "height_inches": height_inches.unwrap_or(1.5),
        }).to_string(),
        Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
    }
}

/// Load a DOCX file into the Internal Representation (IR)
pub fn to_ir(file_path: &str) -> Result<crate::ir::Document, crate::handlers::LoadError> {
    let doc = Document::open(file_path)
        .map_err(|e| crate::handlers::LoadError::ParseError(e.to_string()))?;

    let mut ir = crate::ir::Document::new("docx");
    ir.path = Some(file_path.to_string());
    ir.metadata.title = doc.title().map(|s| s.to_string());
    ir.metadata.author = doc.author().map(|s| s.to_string());

    for p in doc.paragraphs() {
        let text = p.text().to_string();
        if !text.is_empty() {
            ir.paragraphs.push(crate::ir::elements::Paragraph::new(text));
        }
    }

    for img in doc.images() {
        let width_pixels = (img.width_emu as f64 / 9525.0) as u32;
        let height_pixels = (img.height_emu as f64 / 9525.0) as u32;

        ir.images.push(crate::ir::elements::Image {
            name: img.name.clone().unwrap_or_else(|| img.embed_id.clone()),
            width: width_pixels,
            height: height_pixels,
            mime_type: "image/png".to_string(),
            data_base64: None,
            path: None,
        });
    }

    for table in doc.tables() {
            let rows = table.row_count();
            let cols = table.column_count();
            let mut headers = Vec::new();
            let mut data = Vec::new();

            for row in 0..rows {
                let mut row_data = Vec::new();
                for col in 0..cols {
                    if let Some(cell) = table.cell(row, col) {
                        row_data.push(cell.text().to_string());
                    } else {
                        row_data.push(String::new());
                    }
                }
                if row == 0 {
                    headers = row_data;
                } else {
                    data.push(row_data);
                }
            }

            ir.tables.push(crate::ir::elements::Table::new(headers, data));
    }

    Ok(ir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_docx_lifecycle() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_lifecycle.docx");
        let p = path.to_str().unwrap();

        // 1. Create document
        let res = create_document(p, Some("My Title"));
        assert!(res.contains("\"success\":true"));

        // 2. Open document info
        let info = open_document(p);
        assert!(info.contains("\"paragraphs\": 1"));

        // 3. Add paragraph
        let res_p = add_paragraph(
            p,
            "New Paragraph",
            Some(true),
            Some(false),
            Some(false), // underline
            Some(14.0), // font size
            None, // font family
            None, // color
            None, // highlight
            None, // alignment
            None, // shading
            None, // line spacing
            None, // keep with next
            None, // keep together
            None, // page break before
        );
        assert!(res_p.contains("\"success\":true"));

        // 4. Add table
        let headers = vec!["ColA".to_string(), "ColB".to_string()];
        let data = vec![vec!["A1".to_string(), "B1".to_string()]];
        let res_t = add_table(
            p,
            &headers,
            &data,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        assert!(res_t.contains("\"success\":true"));

        // 5. Find and replace
        let res_r = find_replace_text(p, "Paragraph", "DocPara");
        assert!(res_r.contains("\"success\":true"));

        // 6. Add image
        let img_path = dir.join("test_lifecycle_img.png");
        let png_data: Vec<u8> = vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A,
            0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
            0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
            0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53, 0xDE,
            0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54,
            0x08, 0xD7, 0x63, 0xF8, 0xCF, 0xC0, 0x00, 0x00, 0x00, 0x02, 0x00, 0x01, 0xE2, 0x21, 0xBC,
            0x33, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44,
            0xAE, 0x42, 0x60, 0x82,
        ];
        std::fs::write(&img_path, &png_data).unwrap();
        let res_img = add_image(p, img_path.to_str().unwrap(), Some(2.0), Some(1.5));
        assert!(res_img.contains("\"success\":true"));

        // 7. Convert to IR
        let ir = to_ir(p).unwrap();
        assert_eq!(ir.paragraphs.len(), 2); // Title and paragraph
        assert_eq!(ir.tables.len(), 1);
        assert_eq!(ir.tables[0].headers, vec!["ColA", "ColB"]);
        assert_eq!(ir.images.len(), 1);

        // 8. Extract images
        let out_img_dir = dir.join("extracted_docx_imgs");
        let imgs = crate::handlers::extract_images_from_zip(p, out_img_dir.to_str().unwrap()).unwrap();
        assert_eq!(imgs.len(), 1);
        assert!(std::path::Path::new(&imgs[0]).exists());

        // Clean up
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(img_path);
        let _ = std::fs::remove_dir_all(out_img_dir);
    }

    #[test]
    fn test_docx_enhanced_styling() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_docx_styling.docx");
        let p = path.to_str().unwrap();

        // Create document
        let _ = create_document(p, Some("Styling Test"));

        // Add styled paragraph
        let res_p = add_paragraph(
            p,
            "Styled text",
            Some(true),
            Some(true),
            Some(true), // underline
            Some(18.0),
            Some("Arial".to_string()),
            Some("FF0000".to_string()),
            Some("yellow".to_string()),
            Some("center".to_string()),
            Some("F0F0F0".to_string()),
            Some(1.5),
            Some(true),
            Some(true),
            Some(true),
        );
        assert!(res_p.contains("\"success\":true"));

        // Add styled table
        let headers = vec!["Col 1".to_string(), "Col 2".to_string()];
        let data = vec![vec!["Data 1".to_string(), "Data 2".to_string()]];
        let res_t = add_table(
            p,
            &headers,
            &data,
            Some(80.0),
            Some("center".to_string()),
            Some("single".to_string()),
            Some(8),
            Some("FF0000".to_string()),
            Some("CCCCCC".to_string()),
            Some("EEEEEE".to_string()),
            Some(true),
        );
        assert!(res_t.contains("\"success\":true"));

        // Open and read to verify basic parsing still works
        let ir = to_ir(p).unwrap();
        assert_eq!(ir.paragraphs.len(), 2);
        assert_eq!(ir.tables.len(), 1);

        let _ = std::fs::remove_file(path);
    }
}
