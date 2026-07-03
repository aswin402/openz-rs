use pptx::Presentation;
use std::io::{Read, Write, Cursor};

fn get_rpr_xml(font_size: Option<f32>, font_color: Option<&str>, font_family: Option<&str>, bold: bool, italic: bool) -> String {
    let mut attrs = Vec::new();
    if let Some(sz) = font_size {
        attrs.push(format!("sz=\"{}\"", (sz * 100.0) as u32));
    }
    if bold {
        attrs.push("b=\"1\"".to_string());
    }
    if italic {
        attrs.push("i=\"1\"".to_string());
    }
    
    let attrs_str = if attrs.is_empty() {
        String::new()
    } else {
        format!(" {}", attrs.join(" "))
    };

    let mut children = String::new();
    if let Some(color) = font_color {
        children.push_str(&format!("<a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill>", color));
    }
    if let Some(family) = font_family {
        children.push_str(&format!("<a:latin typeface=\"{}\"/><a:cs typeface=\"{}\"/>", family, family));
    }

    if children.is_empty() {
        format!("<a:rPr{}/>", attrs_str)
    } else {
        format!("<a:rPr{}>{}</a:rPr>", attrs_str, children)
    }
}

fn get_ppr_xml(alignment: Option<&str>) -> String {
    if let Some(align) = alignment {
        let align_val = match align.to_lowercase().as_str() {
            "center" | "ctr" => "ctr",
            "right" | "r" => "r",
            "justify" | "just" => "just",
            _ => "l",
        };
        format!("<a:pPr algn=\"{}\"/>", align_val)
    } else {
        String::new()
    }
}

/// Create a simple slide with a title text box
fn create_title_slide_xml(
    title: &str,
    bg_color: Option<&str>,
    font_size: Option<f32>,
    font_color: Option<&str>,
    font_family: Option<&str>,
    alignment: Option<&str>,
) -> Vec<u8> {
    let bg_xml = bg_color.map(|color| {
        format!(
            r#"<p:bg><p:bgPr><a:solidFill><a:srgbClr val="{}"/></a:solidFill><a:effectLst/></p:bgPr></p:bg>"#,
            color
        )
    }).unwrap_or_default();

    let ppr = get_ppr_xml(alignment);
    let rpr = get_rpr_xml(font_size, font_color, font_family, true, false);

    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
       xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
       xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld>
    {bg_xml}
    <p:spTree>
      <p:nvGrpSpPr>
        <p:cNvPr id="1" name=""/>
        <p:cNvGrpSpPr/>
        <p:nvPr/>
      </p:nvGrpSpPr>
      <p:grpSpPr/>
      <p:sp>
        <p:nvSpPr>
          <p:cNvPr id="2" name="Title"/>
          <p:cNvSpPr txBox="1"/>
          <p:nvPr/>
        </p:nvSpPr>
        <p:spPr>
          <a:xfrm>
            <a:off x="914400" y="2000000"/>
            <a:ext cx="8229600" cy="3000000"/>
          </a:xfrm>
          <a:prstGeom prst="rect">
            <a:avLst/>
          </a:prstGeom>
        </p:spPr>
        <p:txBody>
          <a:bodyPr/>
          <a:lstStyle/>
          <a:p>
            {ppr}
            <a:r>
              {rpr}
              <a:t>{title}</a:t>
            </a:r>
          </a:p>
        </p:txBody>
      </p:sp>
    </p:spTree>
  </p:cSld>
  <p:clrMapOvr>
    <a:masterClrMapping/>
  </p:clrMapOvr>
</p:sld>"#,
        title = escape_xml(title),
        bg_xml = bg_xml,
        ppr = ppr,
        rpr = rpr
    ).into_bytes()
}

/// Create a content slide with title and body text
fn create_content_slide_xml(
    title: &str,
    body_items: &[String],
    bg_color: Option<&str>,
    font_size: Option<f32>,
    font_color: Option<&str>,
    font_family: Option<&str>,
    alignment: Option<&str>,
) -> Vec<u8> {
    let bg_xml = bg_color.map(|color| {
        format!(
            r#"<p:bg><p:bgPr><a:solidFill><a:srgbClr val="{}"/></a:solidFill><a:effectLst/></p:bgPr></p:bg>"#,
            color
        )
    }).unwrap_or_default();

    let ppr = get_ppr_xml(alignment);
    let title_rpr = get_rpr_xml(Some(44.0), font_color, font_family, true, false);

    let body_rpr = get_rpr_xml(font_size.or(Some(28.0)), font_color, font_family, false, false);
    let body_xml: String = body_items.iter().map(|item| {
        format!(
            r#"<a:p>{}<a:r>{}<a:t>{}</a:t></a:r></a:p>"#,
            ppr,
            body_rpr,
            escape_xml(item)
        )
    }).collect();

    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
       xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
       xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld>
    {bg_xml}
    <p:spTree>
      <p:nvGrpSpPr>
        <p:cNvPr id="1" name=""/>
        <p:cNvGrpSpPr/>
        <p:nvPr/>
      </p:nvGrpSpPr>
      <p:grpSpPr/>
      <p:sp>
        <p:nvSpPr>
          <p:cNvPr id="2" name="Title"/>
          <p:cNvSpPr txBox="1"/>
          <p:nvPr/>
        </p:nvSpPr>
        <p:spPr>
          <a:xfrm>
            <a:off x="914400" y="685800"/>
            <a:ext cx="8229600" cy="1143000"/>
          </a:xfrm>
          <a:prstGeom prst="rect">
            <a:avLst/>
          </a:prstGeom>
        </p:spPr>
        <p:txBody>
          <a:bodyPr/>
          <a:lstStyle/>
          <a:p>
            {ppr}
            <a:r>
              {title_rpr}
              <a:t>{title}</a:t>
            </a:r>
          </a:p>
        </p:txBody>
      </p:sp>
      <p:sp>
        <p:nvSpPr>
          <p:cNvPr id="3" name="Content"/>
          <p:cNvSpPr txBox="1"/>
          <p:nvPr/>
        </p:nvSpPr>
        <p:spPr>
          <a:xfrm>
            <a:off x="914400" y="2286000"/>
            <a:ext cx="8229600" cy="4953000"/>
          </a:xfrm>
          <a:prstGeom prst="rect">
            <a:avLst/>
          </a:prstGeom>
        </p:spPr>
        <p:txBody>
          <a:bodyPr/>
          <a:lstStyle/>
          {body_xml}
        </p:txBody>
      </p:sp>
    </p:spTree>
  </p:cSld>
  <p:clrMapOvr>
    <a:masterClrMapping/>
  </p:clrMapOvr>
</p:sld>"#,
        title = escape_xml(title),
        bg_xml = bg_xml,
        ppr = ppr,
        title_rpr = title_rpr,
        body_xml = body_xml
    ).into_bytes()
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Create a new PowerPoint presentation and save to a file path.
pub fn create_presentation(file_path: &str, _title: Option<&str>) -> String {
    match Presentation::new() {
        Ok(mut prs) => {
            // Add a title slide
            if let Ok(layouts) = prs.slide_layouts() {
                if let Some(layout) = layouts.first() {
                    let _ = prs.add_slide(layout);
                }
            }
            match prs.save(file_path) {
                Ok(_) => serde_json::json!({
                    "success": true,
                    "path": file_path,
                    "format": "pptx"
                }).to_string(),
                Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
            }
        }
        Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
    }
}

/// Open a PPTX file and return its metadata (slide count, format).
pub fn open_presentation(file_path: &str) -> String {
    match Presentation::open(file_path) {
        Ok(prs) => {
            let slide_count = prs.slide_count().unwrap_or(0);
            let info = serde_json::json!({
                "path": file_path,
                "slides": slide_count,
                "format": "pptx"
            });
            serde_json::to_string_pretty(&info).unwrap_or_default()
        }
        Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
    }
}

/// Add a slide to an existing presentation with optional body bullet points and styles.
pub fn add_slide(
    file_path: &str,
    title: &str,
    body: Option<&[String]>,
    bg_color: Option<String>,
    font_size: Option<f32>,
    font_color: Option<String>,
    font_family: Option<String>,
    alignment: Option<String>,
) -> String {
    let mut prs = match Presentation::open(file_path) {
        Ok(p) => p,
        Err(e) => return serde_json::json!({"error": e.to_string()}).to_string(),
    };

    let layouts = match prs.slide_layouts() {
        Ok(l) => l,
        Err(e) => return serde_json::json!({"error": e.to_string()}).to_string(),
    };

    // Pick the first layout from available layouts
    let layout = match layouts.first() {
        Some(l) => l.clone(),
        None => return serde_json::json!({"error": "no slide layouts available"}).to_string(),
    };

    match prs.add_slide(&layout) {
        Ok(slide_ref) => {
            // Get the slide index
            let slide_idx = prs.slide_index(&slide_ref).unwrap_or(0);

            // Determine XML content based on whether we have body
            let xml = if let Some(body_items) = body {
                if body_items.is_empty() {
                    create_title_slide_xml(
                        title,
                        bg_color.as_deref(),
                        font_size,
                        font_color.as_deref(),
                        font_family.as_deref(),
                        alignment.as_deref(),
                    )
                } else {
                    create_content_slide_xml(
                        title,
                        body_items,
                        bg_color.as_deref(),
                        font_size,
                        font_color.as_deref(),
                        font_family.as_deref(),
                        alignment.as_deref(),
                    )
                }
            } else {
                create_title_slide_xml(
                    title,
                    bg_color.as_deref(),
                    font_size,
                    font_color.as_deref(),
                    font_family.as_deref(),
                    alignment.as_deref(),
                )
            };

            // Set the slide content
            if let Ok(xml_mut) = prs.slide_xml_mut(&slide_ref) {
                *xml_mut = xml;
            }

            match prs.save(file_path) {
                Ok(_) => serde_json::json!({
                    "success": true,
                    "slide_number": slide_idx + 1,
                    "title": title
                }).to_string(),
                Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
            }
        }
        Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
    }
}

/// Embed an image onto a specific slide by manipulating the PPTX OPC package directly.
/// Supports PNG, JPEG, GIF, BMP, TIFF, and SVG formats.
pub fn add_slide_image(file_path: &str, slide_number: u32, image_path: &str) -> String {
    // Read source image
    let img_data = match std::fs::read(image_path) {
        Ok(d) => d,
        Err(e) => return serde_json::json!({
            "error": format!("Cannot read image file '{}': {}", image_path, e),
            "error_code": "FILE_READ_ERROR",
            "category": "io",
            "suggestion": "Check that the image path exists and is readable."
        }).to_string(),
    };

    let path = std::path::Path::new(image_path);
    let img_ext = path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png")
        .to_lowercase();

    let (mime_type, storage_ext) = match img_ext.as_str() {
        "png" => ("image/png", "png"),
        "jpg" | "jpeg" => ("image/jpeg", "jpg"),
        "gif" => ("image/gif", "gif"),
        "bmp" => ("image/bmp", "bmp"),
        "tiff" | "tif" => ("image/tiff", "tiff"),
        "svg" => ("image/svg+xml", "svg"),
        _ => return serde_json::json!({
            "error": format!("Unsupported image format: '{}'. Supported: png, jpg, gif, bmp, tiff, svg", img_ext),
            "error_code": "UNSUPPORTED_FORMAT",
            "category": "validation",
            "suggestion": "Convert the image to PNG and try again."
        }).to_string(),
    };

    // Read the existing PPTX into memory
    let pptx_data = match std::fs::read(file_path) {
        Ok(d) => d,
        Err(e) => return serde_json::json!({
            "error": format!("Cannot read PPTX file '{}': {}", file_path, e),
            "error_code": "FILE_READ_ERROR",
            "category": "io",
            "suggestion": "Check that the file exists and is a valid .pptx file."
        }).to_string(),
    };

    // Open as ZIP using the zip crate (v2)
    let cursor = Cursor::new(pptx_data);
    let mut archive = match zip::ZipArchive::new(cursor) {
        Ok(a) => a,
        Err(e) => return serde_json::json!({
            "error": format!("Cannot open PPTX as ZIP archive: {}", e),
            "error_code": "ZIP_ERROR",
            "category": "parse",
            "suggestion": "The file may be corrupted. Try re-saving it from PowerPoint."
        }).to_string(),
    };

    let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
    let mut content_types: Option<Vec<u8>> = None;
    let mut slide_rels: Option<Vec<u8>> = None;
    let mut slide_xml: Option<Vec<u8>> = None;
    let mut slide_xml_path = String::new();

    for i in 0..archive.len() {
        let mut file = match archive.by_index(i) {
            Ok(f) => f,
            Err(_) => continue,
        };
        let name = file.name().to_string();
        let mut data = Vec::new();
        let _ = file.read_to_end(&mut data);

        // Capture the requested slide's XML and its rels
        if name == format!("ppt/slides/slide{}.xml", slide_number) {
            slide_xml = Some(data.clone());
            slide_xml_path = name.clone();
        }
        if name == format!("ppt/slides/_rels/slide{}.xml.rels", slide_number) {
            slide_rels = Some(data.clone());
        }
        if name == "[Content_Types].xml" {
            content_types = Some(data);
            continue;
        }
        entries.push((name, data));
    }

    if slide_xml.is_none() {
        return serde_json::json!({
            "error": format!("Slide {} not found in presentation", slide_number),
            "error_code": "SLIDE_NOT_FOUND",
            "category": "validation",
            "suggestion": format!("Slide numbers start at 1. Check the slide count first.")
        }).to_string();
    }

    // Find max existing image number in media folder
    let max_img_num = entries.iter()
        .filter(|(name, _)| name.starts_with("ppt/media/image"))
        .filter_map(|(name, _)| {
            let rest = name.strip_prefix("ppt/media/image")?;
            let num: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            num.parse::<u32>().ok()
        })
        .max()
        .unwrap_or(0);

    let new_img_num = max_img_num + 1;
    let media_path = format!("ppt/media/image{}.{}", new_img_num, storage_ext);
    let rel_id = format!("rId{}", new_img_num + 100); // high rId to avoid conflicts

    // Add the image as a new entry
    entries.push((media_path, img_data));

    // Update [Content_Types].xml to include the image
    if let Some(ref ct_bytes) = content_types {
        let ct_str = String::from_utf8_lossy(ct_bytes);
        // Insert new content type before </Types>
        let insert_before = "</Types>";
        if let Some(pos) = ct_str.rfind(insert_before) {
            let mut new_ct = ct_str[..pos].to_string();
            new_ct.push_str(&format!(
                "  <Override PartName=\"/ppt/media/image{}.{}\" ContentType=\"{}\"/>\n",
                new_img_num, storage_ext, mime_type
            ));
            new_ct.push_str("</Types>");
            entries.push(("[Content_Types].xml".to_string(), new_ct.into_bytes()));
        }
    }

    // Update slide rels to include image relationship
    let mut new_slide_rels = if let Some(ref rels_bytes) = slide_rels {
        let rels_str = String::from_utf8_lossy(rels_bytes).to_string();
        rels_str
    } else {
        // Create minimal rels file
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
</Relationships>"#.to_string()
    };

    // Insert image relationship before </Relationships>
    if let Some(pos) = new_slide_rels.rfind("</Relationships>") {
        new_slide_rels.insert_str(pos, &format!(
            "  <Relationship Id=\"{}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/image\" Target=\"../media/image{}.{}\"/>\n",
            rel_id, new_img_num, storage_ext
        ));
    }
    entries.push((format!("ppt/slides/_rels/slide{}.xml.rels", slide_number), new_slide_rels.into_bytes()));

    // Generate picture XML for the slide
    let pic_xml = format!(
        r#"<p:pic xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
       xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
       xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:nvPicPr>
    <p:cNvPr id="{}" name="Image"/>
    <p:cNvPicPr/>
    <p:nvPr/>
  </p:nvPicPr>
  <p:blipFill>
    <a:blip r:embed="{}"/>
    <a:stretch>
      <a:fillRect/>
    </a:stretch>
  </p:blipFill>
  <p:spPr>
    <a:xfrm>
      <a:off x="914400" y="685800"/>
      <a:ext cx="6858000" cy="4572000"/>
    </a:xfrm>
    <a:prstGeom prst="rect">
      <a:avLst/>
    </a:prstGeom>
  </p:spPr>
</p:pic>"#,
        new_img_num + 100, rel_id
    );

    // Insert picture into slide XML (inside spTree but before </p:spTree>)
    if let Some(ref slide_bytes) = slide_xml {
        let mut slide_str = String::from_utf8_lossy(slide_bytes).to_string();
        if let Some(pos) = slide_str.rfind("</p:spTree>") {
            slide_str.insert_str(pos, &format!("\n{}", pic_xml));
        }
        entries.push((slide_xml_path, slide_str.into_bytes()));
    }

    // Write everything back to a new ZIP file
    let temp_path = format!("{}.tmp.pptx", file_path);
    let temp_file = match std::fs::File::create(&temp_path) {
        Ok(f) => f,
        Err(e) => return serde_json::json!({
            "error": format!("Cannot create temp file: {}", e),
            "error_code": "FILE_WRITE_ERROR",
            "category": "io",
            "suggestion": "Check disk space and permissions."
        }).to_string(),
    };

    {
        let mut zip_writer = zip::ZipWriter::new(temp_file);
        let options = zip::write::FileOptions::<()>::default()
            .compression_method(zip::CompressionMethod::Deflated);

        for (name, data) in &entries {
            if zip_writer.start_file(name, options).is_ok() {
                let _ = zip_writer.write_all(data);
            }
        }
        let _ = zip_writer.finish();
    }

    // Replace original with temp
    if std::fs::rename(&temp_path, file_path).is_err() {
        let _ = std::fs::copy(&temp_path, file_path);
        let _ = std::fs::remove_file(&temp_path);
    }

    serde_json::json!({
        "success": true,
        "slide": slide_number,
        "image": image_path,
        "image_number": new_img_num
    }).to_string()
}

/// Convert a PPTX presentation to PDF by delegating to the converter module.
pub fn to_pdf(source: &str, output: &str) -> String {
    // Delegate to the converter module which has a real implementation
    match crate::converters::convert(source, "pdf", output) {
        Ok(result) => serde_json::json!({
            "success": true,
            "source": result.source,
            "output": result.output,
            "size_bytes": result.size_bytes
        }).to_string(),
        Err(e) => serde_json::json!({
            "error": format!("PPTX to PDF conversion failed: {}", e),
            "error_code": "CONVERSION_FAILED",
            "category": "conversion",
            "suggestion": "Try converting to Markdown first, or check that the file is valid."
        }).to_string(),
    }
}

fn html_to_markdown_pptx(html_str: &str) -> String {
    use scraper::{Html, Selector};
    let document = Html::parse_document(html_str);
    let mut parts = Vec::new();
    
    let selector = match Selector::parse("h1, h2, h3, h4, h5, h6, p, div, li, br") {
        Ok(s) => s,
        Err(_) => return html_str.to_string(),
    };
    
    for el in document.select(&selector) {
        let tag = el.value().name();
        let text = el.text().collect::<String>().trim().to_string();
        if text.is_empty() {
            if tag == "br" {
                parts.push("\n".to_string());
            }
            continue;
        }
        match tag {
            "h1" => parts.push(format!("\n# {}\n", text)),
            "h2" => parts.push(format!("\n## {}\n", text)),
            "h3" | "h4" | "h5" | "h6" => parts.push(format!("\n### {}\n", text)),
            "li" => parts.push(format!("- {}\n", text)),
            "p" | "div" => parts.push(format!("{}\n", text)),
            _ => parts.push(text),
        }
    }
    parts.join("")
}

/// Export a PPTX presentation to Markdown via HTML intermediate.
pub fn to_markdown(source: &str) -> String {
    match Presentation::open(source) {
        Ok(prs) => {
            let html = prs.export_html().unwrap_or_default();
            let md = html_to_markdown_pptx(&html);

            let slide_count = prs.slide_count().unwrap_or(0);
            let result = serde_json::json!({
                "success": true,
                "slides": slide_count,
                "markdown": md
            });
            serde_json::to_string_pretty(&result).unwrap_or_default()
        }
        Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
    }
}

/// Load a PPTX file into the Internal Representation (IR)
pub fn to_ir(file_path: &str) -> Result<crate::ir::Document, crate::handlers::LoadError> {
    let prs = Presentation::open(file_path)
        .map_err(|e| crate::handlers::LoadError::ParseError(e.to_string()))?;

    let mut ir = crate::ir::Document::new("pptx");
    ir.path = Some(file_path.to_string());
    ir.metadata.page_count = prs.slide_count().ok().map(|c| c as u32);

    let html = prs.export_html().unwrap_or_default();
    let text = html_to_markdown_pptx(&html);

    for line in text.lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty() && trimmed.len() > 2 {
            ir.paragraphs.push(crate::ir::elements::Paragraph::new(trimmed));
        }
    }

    Ok(ir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pptx_styling_lifecycle() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_styling.pptx");
        let p = path.to_str().unwrap();

        // Create presentation
        let res = create_presentation(p, None);
        assert!(res.contains("\"success\":true"));

        // Add styled slide
        let body_items = vec!["Point A".to_string(), "Point B".to_string()];
        let res_slide = add_slide(
            p,
            "My Styled Slide",
            Some(&body_items),
            Some("FFCDD2".to_string()),
            Some(32.0),
            Some("0000FF".to_string()),
            Some("Courier New".to_string()),
            Some("center".to_string()),
        );
        assert!(res_slide.contains("\"success\":true"));

        // Verify PPTX slide count is 2 (1 initial title slide, 1 added slide)
        let info = open_presentation(p);
        assert!(info.contains("\"slides\": 2"));

        let _ = std::fs::remove_file(path);
    }
}
