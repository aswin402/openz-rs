use scraper::{Html, Selector};

use crate::tools::searchxyz::core::config::ExtractorConfig;
use crate::tools::searchxyz::core::error::SearchXyzError;

/// Extracted content from a crawled page.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ExtractedContent {
    pub url: String,
    pub title: String,
    pub description: String,
    pub content_markdown: String,
    pub links: Vec<String>,
}

/// Pipeline that converts raw HTML into clean markdown text.
pub struct ExtractionPipeline {
    config: ExtractorConfig,
    // Pre-compiled selectors for performance.
    strip_selectors: Vec<Selector>,
    content_selectors: Vec<Selector>,
}

impl ExtractionPipeline {
    pub fn new(config: ExtractorConfig) -> Self {
        let strip_selectors = config
            .strip_selectors
            .iter()
            .filter_map(|s| Selector::parse(s).ok())
            .collect();

        let content_selectors = config
            .content_selectors
            .iter()
            .filter_map(|s| Selector::parse(s).ok())
            .collect();

        Self {
            config,
            strip_selectors,
            content_selectors,
        }
    }

    /// Extract readable content from raw HTML or PDF plain text.
    pub fn extract(
        &self,
        url: &str,
        body: &str,
        content_type: Option<&str>,
    ) -> Result<ExtractedContent, SearchXyzError> {
        let is_pdf = content_type.is_some_and(|ct| ct.contains("application/pdf"));
        match is_pdf {
            true => {
                let raw_title = url.split('/').next_back().unwrap_or("PDF Document").trim();
                let title = match raw_title.is_empty() {
                    true => "PDF Document",
                    false => raw_title,
                };

                return Ok(ExtractedContent {
                    url: url.into(),
                    title: title.to_string(),
                    description: "Extracted PDF Document Text".into(),
                    content_markdown: body.to_string(),
                    links: Vec::new(),
                });
            }
            false => {}
        }

        let document = Html::parse_document(body);

        // ── 1. Extract metadata ──
        let title = self.extract_title(&document);
        let description = self.extract_meta_description(&document);

        // ── 2. Find the main content element ──
        // Try priority selectors in order; fall back to <body>.
        let content_html = self
            .find_main_content(&document)
            .unwrap_or_else(|| self.extract_body_text(&document));

        // ── 3. Strip noisy elements from extracted HTML ──
        let cleaned = self.strip_noise(&content_html);

        // ── 4. Convert to markdown-like plain text ──
        let markdown = self.html_to_markdown(&cleaned);

        // ── 5. Validate length ──
        match markdown.trim().len() < self.config.min_content_length {
            true => {
                return Err(SearchXyzError::EmptyContent {
                    url: url.into(),
                    min_length: self.config.min_content_length,
                });
            }
            false => {}
        }

        // ── 6. Extract links ──
        let links = self.extract_links(&document, url);

        Ok(ExtractedContent {
            url: url.into(),
            title,
            description,
            content_markdown: markdown,
            links,
        })
    }

    // ── Private helpers ──────────────────────────────────────

    fn extract_title(&self, doc: &Html) -> String {
        let sel = Selector::parse("title").unwrap();
        doc.select(&sel)
            .next()
            .map(|el| el.text().collect::<String>().trim().to_string())
            .unwrap_or_default()
    }

    fn extract_meta_description(&self, doc: &Html) -> String {
        let sel = Selector::parse(r#"meta[name="description"]"#).unwrap();
        doc.select(&sel)
            .next()
            .and_then(|el| el.value().attr("content"))
            .map(|s| s.trim().to_string())
            .unwrap_or_default()
    }

    /// Walk priority selectors and return the first match's inner HTML.
    fn find_main_content(&self, doc: &Html) -> Option<String> {
        self.content_selectors
            .iter()
            .find_map(|sel| doc.select(sel).next().map(|el| el.inner_html()))
    }

    /// Fallback: grab all text inside <body>.
    fn extract_body_text(&self, doc: &Html) -> String {
        let sel = Selector::parse("body").unwrap();
        doc.select(&sel)
            .next()
            .map(|el| el.inner_html())
            .unwrap_or_default()
    }

    /// Remove noisy elements (script, style, nav, etc.) from an
    /// HTML fragment string.
    fn strip_noise(&self, html: &str) -> String {
        let fragment = Html::parse_fragment(html);
        let mut output = String::new();

        // Collect node IDs to skip (elements matching strip selectors).
        let skip_ids: std::collections::HashSet<_> = self
            .strip_selectors
            .iter()
            .flat_map(|sel| fragment.select(sel))
            .map(|el| el.id())
            .collect();

        // Walk the tree and emit text for non-skipped nodes.
        fn collect_text(
            node: ego_tree::NodeRef<scraper::Node>,
            skip: &std::collections::HashSet<ego_tree::NodeId>,
            out: &mut String,
        ) {
            match skip.contains(&node.id()) {
                true => return,
                false => {}
            }
            match node.value() {
                scraper::Node::Text(t) => {
                    let text = t.text.trim();
                    match text.is_empty() {
                        true => {}
                        false => {
                            out.push_str(text);
                            out.push(' ');
                        }
                    }
                }
                scraper::Node::Element(el) => {
                    let tag = el.name();
                    let is_block = matches!(
                        tag,
                        "p" | "div"
                            | "br"
                            | "h1"
                            | "h2"
                            | "h3"
                            | "h4"
                            | "h5"
                            | "h6"
                            | "li"
                            | "tr"
                            | "blockquote"
                            | "pre"
                            | "hr"
                    );
                    match is_block {
                        true => out.push('\n'),
                        false => {}
                    }
                    match tag {
                        "h1" => out.push_str("# "),
                        "h2" => out.push_str("## "),
                        "h3" => out.push_str("### "),
                        "h4" => out.push_str("#### "),
                        _ => {}
                    }
                    for child in node.children() {
                        collect_text(child, skip, out);
                    }
                    match is_block {
                        true => out.push('\n'),
                        false => {}
                    }
                }
                _ => {
                    for child in node.children() {
                        collect_text(child, skip, out);
                    }
                }
            }
        }

        match fragment.tree.root().children().next() {
            Some(root) => collect_text(root, &skip_ids, &mut output),
            None => {}
        }

        output
    }

    /// Normalise whitespace for the final markdown output.
    fn html_to_markdown(&self, text: &str) -> String {
        let mut result = String::with_capacity(text.len());
        let mut prev_blank = false;

        for line in text.lines() {
            let trimmed = line.trim();
            match (trimmed.is_empty(), prev_blank) {
                (true, false) => {
                    result.push('\n');
                    prev_blank = true;
                }
                (true, true) => {}
                (false, _) => {
                    result.push_str(trimmed);
                    result.push('\n');
                    prev_blank = false;
                }
            }
        }

        result.trim().to_string()
    }

    fn extract_links(&self, doc: &Html, base_url: &str) -> Vec<String> {
        let sel = Selector::parse("a[href]").unwrap();
        let base = url::Url::parse(base_url).ok();

        doc.select(&sel)
            .filter_map(|el| {
                let href = el.value().attr("href")?;
                match &base {
                    Some(base_parsed) => base_parsed.join(href).ok().map(|u| u.to_string()),
                    None => url::Url::parse(href).ok().map(|u| u.to_string()),
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pdf_extraction_bypass() {
        let config = ExtractorConfig::default();
        let pipeline = ExtractionPipeline::new(config);

        let url = "https://example.com/document.pdf";
        let body = "Hello World! This is extracted PDF text.";
        let res = pipeline
            .extract(url, body, Some("application/pdf"))
            .unwrap();

        assert_eq!(res.title, "document.pdf");
        assert_eq!(res.content_markdown, body);
        assert!(res.links.is_empty());
    }

    #[test]
    fn test_pdf_parsing() {
        use lopdf::content::{Content, Operation};
        use lopdf::{dictionary, Document, Object, Stream};

        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();

        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Courier",
        });

        let resources_id = doc.add_object(dictionary! {
            "Font" => dictionary! { "F1" => font_id },
        });

        let content = Content {
            operations: vec![
                Operation::new("BT", vec![]),
                Operation::new("Tf", vec!["F1".into(), 24.into()]),
                Operation::new("Td", vec![100.into(), 500.into()]),
                Operation::new("Tj", vec![Object::string_literal("Hello World")]),
                Operation::new("ET", vec![]),
            ],
        };
        let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));

        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
            "Resources" => resources_id,
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        });

        let pages = dictionary! {
            "Type" => "Pages",
            "Kids" => vec![page_id.into()],
            "Count" => 1,
        };
        doc.objects.insert(pages_id, Object::Dictionary(pages));
        let root_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", root_id);

        let mut pdf_bytes = Vec::new();
        doc.save_to(&mut pdf_bytes).unwrap();

        let extracted = pdf_extract::extract_text_from_mem(&pdf_bytes);
        assert!(extracted.is_ok());
        let text = extracted.unwrap();
        assert!(text.contains("Hello World"));
    }
}
