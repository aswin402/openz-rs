use pptx::Presentation;

fn get_rpr_xml(
    font_size: Option<f32>,
    font_color: Option<&str>,
    font_family: Option<&str>,
    bold: bool,
    italic: bool,
) -> String {
    let mut attrs = Vec::new();
    match font_size {
        Some(sz) => attrs.push(format!("sz=\"{}\"", (sz * 100.0) as u32)),
        None => {}
    }
    match bold {
        true => attrs.push("b=\"1\"".to_string()),
        false => {}
    }
    match italic {
        true => attrs.push("i=\"1\"".to_string()),
        false => {}
    }

    let attrs_str = match attrs.is_empty() {
        true => String::new(),
        false => format!(" {}", attrs.join(" ")),
    };

    let mut children = String::new();
    match font_color {
        Some(color) => {
            children.push_str(&format!(
                "<a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill>",
                color
            ));
        }
        None => {}
    }
    match font_family {
        Some(family) => {
            children.push_str(&format!(
                "<a:latin typeface=\"{}\"/><a:cs typeface=\"{}\"/>",
                family, family
            ));
        }
        None => {}
    }

    match children.is_empty() {
        true => format!("<a:rPr{}/>", attrs_str),
        false => format!("<a:rPr{}>{}</a:rPr>", attrs_str, children),
    }
}

fn get_ppr_xml(alignment: Option<&str>) -> String {
    match alignment {
        Some(align) => {
            let align_val = match align.to_lowercase().as_str() {
                "center" | "ctr" => "ctr",
                "right" | "r" => "r",
                "justify" | "just" => "just",
                _ => "l",
            };
            format!("<a:pPr algn=\"{}\"/>", align_val)
        }
        None => String::new(),
    }
}

fn get_bg_xml(bg_color: Option<&str>) -> String {
    bg_color
        .map(|color| {
            format!(
                r#"<p:bg><p:bgPr><a:solidFill><a:srgbClr val="{}"/></a:solidFill><a:effectLst/></p:bgPr></p:bg>"#,
                color
            )
        })
        .unwrap_or_default()
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
    let bg_xml = get_bg_xml(bg_color);
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
    )
    .into_bytes()
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
    let bg_xml = get_bg_xml(bg_color);
    let ppr = get_ppr_xml(alignment);
    let title_rpr = get_rpr_xml(Some(44.0), font_color, font_family, true, false);

    let body_rpr = get_rpr_xml(
        font_size.or(Some(28.0)),
        font_color,
        font_family,
        false,
        false,
    );
    let body_xml: String = body_items
        .iter()
        .map(|item| {
            format!(
                r#"<a:p>{}<a:r>{}<a:t>{}</a:t></a:r></a:p>"#,
                ppr,
                body_rpr,
                escape_xml(item)
            )
        })
        .collect();

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
    )
    .into_bytes()
}

use crate::agent::style::escape_xml;

/// Create a new PowerPoint presentation and save to a file path.
pub fn create_presentation(file_path: &str, _title: Option<&str>) -> String {
    match Presentation::new() {
        Ok(mut prs) => {
            // Add a title slide
            match prs.slide_layouts().ok().and_then(|l| l.first().cloned()) {
                Some(layout) => {
                    let _ = prs.add_slide(&layout);
                }
                None => {}
            }
            match prs.save(file_path) {
                Ok(_) => serde_json::json!({
                    "success": true,
                    "path": file_path,
                    "format": "pptx"
                })
                .to_string(),
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
#[allow(clippy::too_many_arguments)]
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
            let xml = match body {
                Some(body_items) => match body_items.is_empty() {
                    true => create_title_slide_xml(
                        title,
                        bg_color.as_deref(),
                        font_size,
                        font_color.as_deref(),
                        font_family.as_deref(),
                        alignment.as_deref(),
                    ),
                    false => create_content_slide_xml(
                        title,
                        body_items,
                        bg_color.as_deref(),
                        font_size,
                        font_color.as_deref(),
                        font_family.as_deref(),
                        alignment.as_deref(),
                    ),
                },
                None => create_title_slide_xml(
                    title,
                    bg_color.as_deref(),
                    font_size,
                    font_color.as_deref(),
                    font_family.as_deref(),
                    alignment.as_deref(),
                ),
            };

            // Set the slide content
            match prs.slide_xml_mut(&slide_ref) {
                Ok(xml_mut) => {
                    *xml_mut = xml;
                }
                Err(_) => {}
            }

            match prs.save(file_path) {
                Ok(_) => serde_json::json!({
                    "success": true,
                    "slide_number": slide_idx + 1,
                    "title": title
                })
                .to_string(),
                Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
            }
        }
        Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
    }
}

/// Convert a PPTX presentation to PDF by delegating to the converter module.
pub fn to_pdf(source: &str, output: &str) -> String {
    // Delegate to the converter module which has a real implementation
    match crate::tools::opendoc::converters::convert(source, "pdf", output) {
        Ok(result) => serde_json::json!({
            "success": true,
            "source": result.source,
            "output": result.output,
            "size_bytes": result.size_bytes
        })
        .to_string(),
        Err(e) => serde_json::json!({
            "error": format!("PPTX to PDF conversion failed: {}", e),
            "error_code": "CONVERSION_FAILED",
            "category": "conversion",
            "suggestion": "Try converting to Markdown first, or check that the file is valid."
        })
        .to_string(),
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
        match (text.is_empty(), tag) {
            (true, "br") => parts.push("\n".to_string()),
            (true, _) => {}
            (false, "h1") => parts.push(format!("\n# {}\n", text)),
            (false, "h2") => parts.push(format!("\n## {}\n", text)),
            (false, "h3" | "h4" | "h5" | "h6") => parts.push(format!("\n### {}\n", text)),
            (false, "li") => parts.push(format!("- {}\n", text)),
            (false, "p" | "div") => parts.push(format!("{}\n", text)),
            (false, _) => parts.push(text),
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
pub fn to_ir(
    file_path: &str,
) -> Result<crate::tools::opendoc::ir::Document, crate::tools::opendoc::handlers::LoadError> {
    let prs = Presentation::open(file_path)
        .map_err(|e| crate::tools::opendoc::handlers::LoadError::ParseError(e.to_string()))?;

    let mut ir = crate::tools::opendoc::ir::Document::new("pptx");
    ir.path = Some(file_path.to_string());
    ir.metadata.page_count = prs.slide_count().ok().map(|c| c as u32);

    let html = prs.export_html().unwrap_or_default();
    let text = html_to_markdown_pptx(&html);

    for line in text.lines() {
        let trimmed = line.trim();
        match !trimmed.is_empty() && trimmed.len() > 2 {
            true => {
                ir.paragraphs
                    .push(crate::tools::opendoc::ir::elements::Paragraph::new(trimmed));
            }
            false => {}
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
