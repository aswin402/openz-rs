use lopdf::{Document, Object, ObjectId, Stream, Dictionary};

/// Configuration for PDF layout and formatting.
#[derive(Debug, Clone)]
pub struct PdfLayoutConfig {
    pub title: Option<String>,
    pub author: Option<String>,
    pub page_numbers: bool,
    pub margin_top: f64,
    pub margin_bottom: f64,
    pub margin_left: f64,
    pub margin_right: f64,
    pub font_size: f64,
    pub line_height: f64,
    pub page_width: f64,
    pub page_height: f64,
}

impl Default for PdfLayoutConfig {
    fn default() -> Self {
        Self {
            title: None,
            author: None,
            page_numbers: false,
            margin_top: 72.0,
            margin_bottom: 72.0,
            margin_left: 72.0,
            margin_right: 72.0,
            font_size: 12.0,
            line_height: 15.0,
            page_width: 612.0,
            page_height: 792.0,
        }
    }
}

/// Internal PDF builder that handles layout, pagination, and content flow.
struct PdfBuilder<'a> {
    config: &'a PdfLayoutConfig,
    pages: Vec<Vec<String>>,
    current_page: Vec<String>,
    y_pos: f64,
    page_num: usize,
}

impl<'a> PdfBuilder<'a> {
    fn new(config: &'a PdfLayoutConfig) -> Self {
        Self {
            config,
            pages: Vec::new(),
            current_page: Vec::new(),
            y_pos: config.page_height - config.margin_top,
            page_num: 0,
        }
    }

    /// Content width in points
    fn content_width(&self) -> f64 {
        self.config.page_width - self.config.margin_left - self.config.margin_right
    }

    /// Estimated max chars per line based on font size (Helvetica average char width ≈ 0.6 * font_size)
    fn chars_per_line(&self) -> usize {
        let avg_char_width = self.config.font_size * 0.6;
        (self.content_width() / avg_char_width).floor() as usize
    }

    /// Add a line of text at the current position
    fn add_text_line(&mut self, line: &str) {
        if self.y_pos < self.config.margin_top + self.config.font_size {
            self.flush_page();
        }
        let escaped = line
            .replace('\\', "\\\\")
            .replace('(', "\\(")
            .replace(')', "\\)");
        self.current_page.push(format!(
            "{} Td ({}) Tj",
            if self.current_page.is_empty() {
                format!("1 0 0 1 {} {} Tm", self.config.margin_left, self.y_pos)
            } else {
                format!("0 {} Td", -self.config.line_height)
            },
            escaped
        ));
        self.y_pos -= self.config.line_height;
    }

    /// Add wrapped text (word-wrap at margin width)
    #[allow(dead_code)]
    fn add_wrapped_text(&mut self, text: &str) {
        let max_chars = self.chars_per_line();
        for line in text.lines() {
            let mut remaining = line;
            while !remaining.is_empty() {
                if remaining.len() <= max_chars {
                    self.add_text_line(remaining);
                    break;
                }
                // Try to break at word boundary
                let break_at = match remaining[..max_chars].rfind(' ') {
                    Some(pos) => pos,
                    None => max_chars,
                };
                self.add_text_line(&remaining[..break_at]);
                remaining = remaining[break_at..].trim_start();
            }
        }
    }

    /// Add a page break marker
    fn add_page_break(&mut self) {
        self.flush_page();
    }

    /// Add centered title with spacing
    fn add_centered(&mut self, text: &str, size: f64, spacing: f64) {
        if self.y_pos < self.config.margin_top + size + spacing {
            self.flush_page();
        }
        let escaped = text
            .replace('\\', "\\\\")
            .replace('(', "\\(")
            .replace(')', "\\)");
        // Approximate centering: center of content area
        let x_center = self.config.margin_left + self.content_width() / 2.0;
        self.current_page.push(format!(
            "BT /F1 {:.0} Tf {} {} Tm ({}) Tj ET",
            size, x_center, self.y_pos, escaped
        ));
        self.y_pos -= spacing;
    }

    /// Flush current page to pages list
    fn flush_page(&mut self) {
        if !self.current_page.is_empty() || self.pages.is_empty() {
            self.page_num += 1;
            self.pages.push(std::mem::take(&mut self.current_page));
            self.y_pos = self.config.page_height - self.config.margin_top;
        }
    }

    /// Finalize and produce page content strings + total page count
    fn finalize(mut self) -> Vec<String> {
        if !self.current_page.is_empty() || self.pages.is_empty() {
            self.flush_page();
        }
        let total = self.pages.len();
        let show_page_numbers = self.config.page_numbers;
        let x_center = self.config.margin_left + self.content_width() / 2.0;
        let y_bottom = self.config.margin_bottom / 2.0;
        self.pages
            .into_iter()
            .enumerate()
            .map(|(idx, page_content)| {
                let joined = page_content.join(" ");
                let mut full_content = if !joined.is_empty() && !joined.starts_with("BT") {
                    format!("BT /F1 {} Tf {} ET", self.config.font_size, joined)
                } else {
                    joined
                };

                if show_page_numbers {
                    let page_num_text = format!("{} / {}", idx + 1, total);
                    let escaped = page_num_text
                        .replace('\\', "\\\\")
                        .replace('(', "\\(")
                        .replace(')', "\\)");
                    let page_num_content = format!(
                        " BT /F1 9 Tf {} {} Tmd ({}) Tj ET",
                        x_center, y_bottom, escaped
                    );
                    full_content.push_str(&page_num_content);
                }
                full_content
            })
            .collect()
    }
}

/// Create a PDF with word-wrapping, page breaks, and auto-pagination.
/// Uses default layout (Helvetica 12pt, US Letter, 1-inch margins).
/// For advanced layout options, use [`create_formatted_pdf`].
pub fn create_pdf(file_path: &str, text: &str, _author: Option<&str>) -> String {
    create_formatted_pdf(file_path, text, &PdfLayoutConfig::default())
}

/// Create a PDF with custom layout configuration.
/// Supports word-wrapping, page breaks (`\f`), page numbers, title page, and margins.
pub fn create_formatted_pdf(file_path: &str, text: &str, config: &PdfLayoutConfig) -> String {
    let mut doc = Document::new();

    // Create font object
    let font_id = doc.add_object(Object::Dictionary(Dictionary::from_iter([
        (b"Type".to_vec(), Object::Name(b"Font".to_vec())),
        (b"Subtype".to_vec(), Object::Name(b"Type1".to_vec())),
        (b"BaseFont".to_vec(), Object::Name(b"Helvetica".to_vec())),
    ])));

    // Build content using the layout engine
    let mut builder = PdfBuilder::new(config);

    // Title page if title is set
    if let Some(ref title) = config.title {
        // Push y to center area
        builder.y_pos = config.page_height * 0.7;
        builder.add_centered(title, 24.0, 30.0);
        if let Some(ref author) = config.author {
            builder.add_centered(&format!("by {}", author), 14.0, 20.0);
        }
        builder.add_centered("", 12.0, 40.0); // spacing
    }

    // Render text content with word-wrapping and explicit page breaks
    let max_chars = builder.chars_per_line();

    for segment in text.split('\x0c') {
        if segment != text.split('\x0c').next().unwrap_or("") {
            builder.add_page_break();
        }
        let segment = if segment == text && config.title.is_some() {
            // Skip first blank if it was just the title
            segment.trim_start()
        } else {
            segment
        };

        for line in segment.lines() {
            if line.trim().is_empty() {
                builder.add_text_line("");
                continue;
            }
            let mut remaining = line;
            while !remaining.is_empty() {
                if remaining.len() <= max_chars {
                    builder.add_text_line(remaining);
                    break;
                }
                let break_at = match remaining[..max_chars].rfind(|c: char| c.is_whitespace()) {
                    Some(pos) => pos,
                    None => max_chars,
                };
                builder.add_text_line(&remaining[..break_at]);
                remaining = remaining[break_at..].trim_start();
            }
        }
    }

    let page_contents = builder.finalize();
    let mut page_ids = Vec::new();
    let pages_id = doc.new_object_id();

    if page_contents.is_empty() {
        // Create empty page
        let content_bytes = b"BT /F1 12 Tf 50 700 Td () Tj ET".to_vec();
        let mut dict = Dictionary::new();
        dict.set("Length", content_bytes.len() as i64);
        let content_id = doc.add_object(Object::Stream(Stream {
            dict,
            content: content_bytes,
            allows_compression: true,
            start_position: None,
        }));
        let page_id = doc.new_object_id();
        let page = Object::Dictionary(Dictionary::from_iter([
            (b"Type".to_vec(), Object::Name(b"Page".to_vec())),
            (b"Parent".to_vec(), Object::Reference(pages_id)),
            (b"Contents".to_vec(), Object::Reference(content_id)),
            (
                b"Resources".to_vec(),
                Object::Dictionary(Dictionary::from_iter([(
                    b"Font".to_vec(),
                    Object::Dictionary(Dictionary::from_iter([(
                        b"F1".to_vec(),
                        Object::Reference(font_id),
                    )])),
                )])),
            ),
            (
                b"MediaBox".to_vec(),
                Object::Array(vec![
                    Object::Integer(0),
                    Object::Integer(0),
                    Object::Integer(config.page_width as i64),
                    Object::Integer(config.page_height as i64),
                ]),
            ),
        ]));
        doc.objects.insert(page_id, page);
        page_ids.push(page_id);
    } else {
        for content_str in &page_contents {
            let content_bytes = content_str.as_bytes().to_vec();
            let mut dict = Dictionary::new();
            dict.set("Length", content_bytes.len() as i64);
            let content_id = doc.add_object(Object::Stream(Stream {
                dict,
                content: content_bytes,
                allows_compression: true,
                start_position: None,
            }));

            let page_id = doc.new_object_id();
            let page = Object::Dictionary(Dictionary::from_iter([
                (b"Type".to_vec(), Object::Name(b"Page".to_vec())),
                (b"Parent".to_vec(), Object::Reference(pages_id)),
                (b"Contents".to_vec(), Object::Reference(content_id)),
                (
                    b"Resources".to_vec(),
                    Object::Dictionary(Dictionary::from_iter([(
                        b"Font".to_vec(),
                        Object::Dictionary(Dictionary::from_iter([(
                            b"F1".to_vec(),
                            Object::Reference(font_id),
                        )])),
                    )])),
                ),
                (
                    b"MediaBox".to_vec(),
                    Object::Array(vec![
                        Object::Integer(0),
                        Object::Integer(0),
                        Object::Integer(config.page_width as i64),
                        Object::Integer(config.page_height as i64),
                    ]),
                ),
            ]));
            doc.objects.insert(page_id, page);
            page_ids.push(page_id);
        }
    }

    let pages = Object::Dictionary(Dictionary::from_iter([
        (b"Type".to_vec(), Object::Name(b"Pages".to_vec())),
        (
            b"Kids".to_vec(),
            Object::Array(page_ids.iter().map(|id| Object::Reference(*id)).collect()),
        ),
        (b"Count".to_vec(), Object::Integer(page_ids.len() as i64)),
    ]));
    doc.objects.insert(pages_id, pages);

    let catalog_id = doc.new_object_id();
    let catalog = Object::Dictionary(Dictionary::from_iter([
        (b"Type".to_vec(), Object::Name(b"Catalog".to_vec())),
        (b"Pages".to_vec(), Object::Reference(pages_id)),
    ]));
    doc.objects.insert(catalog_id, catalog);
    doc.trailer.set("Root", Object::Reference(catalog_id));

    doc.max_id = catalog_id.0;

    match doc.save(file_path) {
        Ok(_) => serde_json::json!({
            "success": true,
            "path": file_path,
            "format": "pdf",
            "pages": page_ids.len()
        }).to_string(),
        Err(e) => serde_json::json!({"error": format!("io: {e}")}).to_string(),
    }
}

/// Open a PDF and return metadata (page count, encryption status, PDF version).
pub fn open_pdf(file_path: &str) -> String {
    match Document::load(file_path) {
        Ok(doc) => {
            let pages = doc.get_pages();
            let page_count = pages.len();
            let is_encrypted = doc.is_encrypted();

            serde_json::json!({
                "path": file_path,
                "pages": page_count,
                "encrypted": is_encrypted,
                "version": doc.version,
            }).to_string()
        }
        Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
    }
}

/// Merge multiple PDF files into a single document with automatic object renumbering.
pub fn merge_pdfs(sources: &[String], output: &str) -> String {
    if sources.is_empty() {
        return serde_json::json!({"error": "no source files provided"}).to_string();
    }

    let mut result_doc = match Document::load(&sources[0]) {
        Ok(d) => d,
        Err(e) => return serde_json::json!({"error": e.to_string()}).to_string(),
    };

    let mut max_id = result_doc.max_id + 1;

    for source in sources.iter().skip(1) {
        let doc = match Document::load(source) {
            Ok(d) => d,
            Err(e) => return serde_json::json!({"error": e.to_string()}).to_string(),
        };

        let mut renumbered = doc;
        renumbered.renumber_objects_with(max_id);
        let source_max_id = renumbered.max_id;
        max_id = source_max_id + 1;

        // Get pages before merging
        let page_ids: Vec<ObjectId> = renumbered.get_pages().values().copied().collect();

        // Add all objects
        for (obj_id, obj) in std::mem::take(&mut renumbered.objects) {
            result_doc.objects.insert(obj_id, obj);
        }

        // Get the root Pages object ID from Catalog
        let mut root_pages_id = None;
        if let Ok(catalog) = result_doc.catalog() {
            if let Ok(Object::Reference(p_ref)) = catalog.get(b"Pages") {
                root_pages_id = Some(*p_ref);
            }
        }

        if let Some(p_id) = root_pages_id {
            if let Ok(dict) = result_doc.get_dictionary_mut(p_id) {
                if let Ok(Object::Array(ref mut kids)) = dict.get_mut(b"Kids") {
                    for page_id in page_ids {
                        kids.push(Object::Reference(page_id));
                    }
                }
            }
        }

        // Find and update the Pages tree in result_doc
        let page_count = result_doc.get_pages().len() as i64;

        // Update page count in pages dict - find it by type
        let pages_to_update: Vec<ObjectId> = result_doc.objects.iter()
            .filter(|(_, obj)| {
                if let Object::Dictionary(dict) = obj {
                    matches!(dict.get(b"Type"), Ok(Object::Name(name)) if name == b"Pages")
                } else {
                    false
                }
            })
            .map(|(id, _)| *id)
            .collect();

        for pages_id in pages_to_update {
            if let Ok(dict) = result_doc.get_dictionary_mut(pages_id) {
                dict.set("Count", Object::Integer(page_count));
            }
        }
    }

    result_doc.max_id = max_id - 1;

    match result_doc.save(output) {
        Ok(_) => serde_json::json!({
            "success": true,
            "sources": sources,
            "output": output
        }).to_string(),
        Err(e) => serde_json::json!({"error": format!("io: {e}")}).to_string(),
    }
}

/// Extract text from a PDF, optionally from a specific page number (0-indexed).
pub fn extract_text(file_path: &str, page: Option<u32>) -> String {
    match Document::load(file_path) {
        Ok(doc) => {
            let pages = doc.get_pages();
            let page_numbers: Vec<u32> = match page {
                Some(p) if p < pages.len() as u32 => {
                    let keys: Vec<&u32> = pages.keys().collect();
                    vec![*keys[p as usize]]
                }
                None => pages.keys().copied().collect(),
                _ => return serde_json::json!({"error": "page number out of range"}).to_string(),
            };

            match doc.extract_text(&page_numbers) {
                Ok(text) => serde_json::json!({"success": true, "text": text, "pages": page_numbers.len()}).to_string(),
                Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
            }
        }
        Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
    }
}

/// Find and replace text in PDF content streams. Writes changes in-place.
pub fn replace_text(file_path: &str, find: &str, replace: &str) -> String {
    replace_text_with_password(file_path, find, replace, None)
}

/// Find and replace text in PDF content streams with password decryption. Writes changes in-place.
pub fn replace_text_with_password(file_path: &str, find: &str, replace: &str, password: Option<&str>) -> String {
    let mut doc = match Document::load(file_path) {
        Ok(d) => d,
        Err(e) => return serde_json::json!({"error": e.to_string()}).to_string(),
    };

    if doc.is_encrypted() {
        let pass = password.unwrap_or("");
        if let Err(e) = doc.decrypt(pass.as_bytes()) {
            return serde_json::json!({"error": format!("Failed to decrypt PDF: {}", e)}).to_string();
        }
    }

    let pages = doc.get_pages();
    let mut total_replacements = 0u32;
    for page_num in pages.keys() {
        if doc.replace_text(*page_num, find, replace).is_ok() {
            total_replacements += 1;
        }
    }
    match doc.save(file_path) {
        Ok(_) => serde_json::json!({
            "success": true,
            "pages_modified": total_replacements,
            "find": find,
            "replace": replace
        }).to_string(),
        Err(e) => serde_json::json!({"error": format!("io: {e}")}).to_string(),
    }
}

/// Load a PDF file into the Internal Representation (IR)
pub fn to_ir(file_path: &str) -> Result<crate::ir::Document, crate::handlers::LoadError> {
    to_ir_with_password(file_path, None)
}

/// Load a PDF file into the Internal Representation (IR) with an optional password
pub fn to_ir_with_password(file_path: &str, password: Option<&str>) -> Result<crate::ir::Document, crate::handlers::LoadError> {
    let mut doc = lopdf::Document::load(file_path)
        .map_err(|e| crate::handlers::LoadError::ParseError(e.to_string()))?;

    let is_encrypted = doc.is_encrypted();
    if is_encrypted {
        let pass = password.unwrap_or("");
        doc.decrypt(pass.as_bytes())
            .map_err(|e| crate::handlers::LoadError::ParseError(format!("Failed to decrypt PDF: {}", e)))?;
    }

    let mut ir = crate::ir::Document::new("pdf");
    ir.path = Some(file_path.to_string());
    ir.metadata.page_count = Some(doc.get_pages().len() as u32);
    ir.metadata.encrypted = doc.is_encrypted();

    let pages: Vec<u32> = doc.get_pages().keys().copied().collect();
    if let Ok(text) = doc.extract_text(&pages) {
        for line in text.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                ir.paragraphs.push(crate::ir::elements::Paragraph::new(trimmed));
            }
        }
    }

    Ok(ir)
}

/// Split a PDF document keeping only the specified page range (1-based, inclusive).
pub fn split_pdf(file_path: &str, output_path: &str, start_page: u32, end_page: u32) -> Result<(), String> {
    split_pdf_with_password(file_path, output_path, start_page, end_page, None)
}

/// Split a PDF document keeping only the specified page range (1-based, inclusive), with optional password support.
pub fn split_pdf_with_password(
    file_path: &str,
    output_path: &str,
    start_page: u32,
    end_page: u32,
    password: Option<&str>,
) -> Result<(), String> {
    let mut doc = lopdf::Document::load(file_path)
        .map_err(|e| format!("Failed to load PDF: {}", e))?;

    if doc.is_encrypted() {
        let pass = password.unwrap_or("");
        doc.decrypt(pass.as_bytes())
            .map_err(|e| format!("Failed to decrypt PDF: {}", e))?;
    }

    let total_pages = doc.get_pages().len() as u32;
    if start_page == 0 || end_page == 0 || start_page > end_page || start_page > total_pages {
        return Err(format!("Invalid page range {}-{} for a {} page document", start_page, end_page, total_pages));
    }
    
    let end_page = std::cmp::min(end_page, total_pages);
    
    // Find page numbers to delete
    let mut pages_to_delete = Vec::new();
    for p in 1..=total_pages {
        if p < start_page || p > end_page {
            pages_to_delete.push(p);
        }
    }

    doc.delete_pages(&pages_to_delete);
    doc.prune_objects();
    
    doc.save(output_path)
        .map_err(|e| format!("Failed to save split PDF: {}", e))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_pdf_single_page() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_single.pdf");
        let p = path.to_str().unwrap();

        let res = create_pdf(p, "Short text", None);
        assert!(res.contains("\"success\":true"));
        assert!(res.contains("\"pages\":1"));

        let doc = lopdf::Document::load(p).unwrap();
        assert_eq!(doc.get_pages().len(), 1);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_create_pdf_multi_page() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_multipage.pdf");
        let p = path.to_str().unwrap();

        // Create 50 lines of text to trigger 2 pages
        let mut text = String::new();
        for i in 1..=50 {
            text.push_str(&format!("Line {}\n", i));
        }

        let res = create_pdf(p, &text, None);
        assert!(res.contains("\"success\":true"));
        assert!(res.contains("\"pages\":2"));

        let doc = lopdf::Document::load(p).unwrap();
        assert_eq!(doc.get_pages().len(), 2);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_create_pdf_empty_text() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_empty.pdf");
        let p = path.to_str().unwrap();

        let res = create_pdf(p, "", None);
        assert!(res.contains("\"success\":true"));
        assert!(res.contains("\"pages\":1"));

        let doc = lopdf::Document::load(p).unwrap();
        assert_eq!(doc.get_pages().len(), 1);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_formatted_pdf_with_title() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_title.pdf");
        let p = path.to_str().unwrap();

        let config = PdfLayoutConfig {
            title: Some("My Document".to_string()),
            author: Some("Test Author".to_string()),
            ..Default::default()
        };

        let res = create_formatted_pdf(p, "Body content here", &config);
        assert!(res.contains("\"success\":true"));
        assert!(res.contains("\"pages\":1"));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_formatted_pdf_page_numbers() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_pagenum.pdf");
        let p = path.to_str().unwrap();

        let config = PdfLayoutConfig {
            page_numbers: true,
            ..Default::default()
        };

        let mut text = String::new();
        for i in 1..=50 {
            text.push_str(&format!("Line {}\n", i));
        }

        let res = create_formatted_pdf(p, &text, &config);
        assert!(res.contains("\"success\":true"));
        assert!(res.contains("\"pages\":2"));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_formatted_pdf_explicit_page_break() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_pagebreak.pdf");
        let p = path.to_str().unwrap();

        let config = PdfLayoutConfig::default();
        let text = "Page 1 content\x0cPage 2 content";

        let res = create_formatted_pdf(p, text, &config);
        assert!(res.contains("\"success\":true"));
        assert!(res.contains("\"pages\":2"));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_formatted_pdf_word_wrap() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_wrap.pdf");
        let p = path.to_str().unwrap();

        let config = PdfLayoutConfig {
            page_width: 300.0, // Narrow page to force wrapping
            margin_left: 10.0,
            margin_right: 10.0,
            ..Default::default()
        };

        let text = "This is a very long line that should wrap to multiple lines on this narrow page layout to test word wrapping logic";
        let res = create_formatted_pdf(p, text, &config);
        assert!(res.contains("\"success\":true"));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_merge_pdfs() {
        let dir = std::env::temp_dir();
        let path1 = dir.join("test_merge_1.pdf");
        let path2 = dir.join("test_merge_2.pdf");
        let path_out = dir.join("test_merge_out.pdf");

        let p1 = path1.to_str().unwrap();
        let p2 = path2.to_str().unwrap();
        let p_out = path_out.to_str().unwrap();

        // Create two single page PDFs
        let _ = create_pdf(p1, "First PDF Page Content", None);
        let _ = create_pdf(p2, "Second PDF Page Content", None);

        // Merge them
        let res = merge_pdfs(&[p1.to_string(), p2.to_string()], p_out);
        assert!(res.contains("\"success\":true"));

        // Load the merged PDF and verify page count is 2
        let doc = lopdf::Document::load(p_out).unwrap();
        assert_eq!(doc.get_pages().len(), 2);

        // Clean up
        let _ = std::fs::remove_file(path1);
        let _ = std::fs::remove_file(path2);
        let _ = std::fs::remove_file(path_out);
    }

    #[test]
    fn test_encrypted_pdf_detection() {
        let mut doc = lopdf::Document::new();
        doc.trailer.set("Encrypt", lopdf::Object::Reference((1, 0)));
        doc.objects.insert((1, 0), lopdf::Object::Dictionary(lopdf::Dictionary::new()));
        assert!(doc.is_encrypted());
    }

    #[test]
    fn test_replace_text_invalid_password_error() {
        let res_nonexistent = replace_text_with_password("nonexistent_file.pdf", "a", "b", Some("pass"));
        assert!(res_nonexistent.contains("\"error\""));
    }

    #[test]
    fn test_split_pdf() {
        let dir = std::env::temp_dir();
        let path_in = dir.join("test_split_in.pdf");
        let path_out = dir.join("test_split_out.pdf");
        let p_in = path_in.to_str().unwrap();
        let p_out = path_out.to_str().unwrap();

        let _ = create_pdf(p_in, "Page 1 content\x0cPage 2 content\x0cPage 3 content", None);

        let doc_in = lopdf::Document::load(p_in).unwrap();
        assert_eq!(doc_in.get_pages().len(), 3);

        let split_res = split_pdf(p_in, p_out, 2, 2);
        assert!(split_res.is_ok());

        let doc_out = lopdf::Document::load(p_out).unwrap();
        assert_eq!(doc_out.get_pages().len(), 1);

        let pages: Vec<u32> = doc_out.get_pages().keys().copied().collect();
        let text = doc_out.extract_text(&pages).unwrap();
        println!("EXTRACTED TEXT: {:?}", text);
        assert!(text.contains("Page 2 content"));
        assert!(!text.contains("Page 1 content"));

        let _ = std::fs::remove_file(path_in);
        let _ = std::fs::remove_file(path_out);
    }
}
