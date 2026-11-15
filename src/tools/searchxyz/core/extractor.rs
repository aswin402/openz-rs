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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BodyFormat {
    Pdf,
    Json,
    Xml,
    PlainOrMarkdown,
    Html,
}

fn classify_body_format(content_type: Option<&str>, body: &str) -> BodyFormat {
    let ct_lower = content_type
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    let trimmed = body.trim_start();

    match ct_lower.as_str() {
        "application/pdf" => BodyFormat::Pdf,
        "application/json" | "text/json" => BodyFormat::Json,
        ct_val => match (
            ct_val.ends_with("+json"),
            matches!(
                ct_val,
                "text/xml"
                    | "application/xml"
                    | "application/rss+xml"
                    | "application/atom+xml"
                    | "application/rdf+xml"
            ) || ct_val.ends_with("+xml")
                || trimmed.starts_with("<?xml")
                || trimmed.starts_with("<urlset")
                || trimmed.starts_with("<sitemapindex")
                || trimmed.starts_with("<rss")
                || trimmed.starts_with("<feed"),
            matches!(ct_val, "text/plain" | "text/markdown" | "text/x-markdown"),
        ) {
            (true, _, _) => BodyFormat::Json,
            (false, true, _) => BodyFormat::Xml,
            (false, false, true) => BodyFormat::PlainOrMarkdown,
            (false, false, false) => BodyFormat::Html,
        },
    }
}

fn fallback_url_title<'a>(url: &'a str, default_label: &'a str) -> String {
    let seg = url
        .split('?')
        .next()
        .unwrap_or(url)
        .trim_end_matches('/')
        .split('/')
        .next_back()
        .unwrap_or(default_label)
        .trim();
    match seg.is_empty() {
        true => default_label.to_string(),
        false => seg.to_string(),
    }
}

/// Pipeline that converts raw HTML, XML/RSS/Atom, JSON, Markdown, or PDF text into clean Markdown.
pub struct ExtractionPipeline {
    config: ExtractorConfig,
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

    /// Extract readable content from raw HTML, XML/RSS/Atom, JSON, plain/Markdown, or PDF text.
    pub fn extract(
        &self,
        url: &str,
        body: &str,
        content_type: Option<&str>,
    ) -> Result<ExtractedContent, SearchXyzError> {
        match classify_body_format(content_type, body) {
            BodyFormat::Pdf => Ok(ExtractedContent {
                url: url.into(),
                title: fallback_url_title(url, "PDF Document"),
                description: "Extracted PDF Document Text".into(),
                content_markdown: body.to_string(),
                links: Vec::new(),
            }),
            BodyFormat::Json => {
                let pretty = serde_json::from_str::<serde_json::Value>(body)
                    .ok()
                    .and_then(|val| serde_json::to_string_pretty(&val).ok())
                    .unwrap_or_else(|| body.trim().to_string());
                Ok(ExtractedContent {
                    url: url.into(),
                    title: fallback_url_title(url, "JSON Payload"),
                    description: "Structured JSON response".into(),
                    content_markdown: format!("```json\n{pretty}\n```"),
                    links: Vec::new(),
                })
            }
            BodyFormat::Xml => Ok(extract_xml_content(url, body)),
            BodyFormat::PlainOrMarkdown => {
                let trimmed = body.trim();
                match trimmed.is_empty() {
                    true => Err(SearchXyzError::EmptyContent {
                        url: url.into(),
                        min_length: 1,
                    }),
                    false => Ok(ExtractedContent {
                        url: url.into(),
                        title: fallback_url_title(url, "Text Document"),
                        description: String::new(),
                        content_markdown: trimmed.to_string(),
                        links: Vec::new(),
                    }),
                }
            }
            BodyFormat::Html => self.extract_html(url, body),
        }
    }

    fn extract_html(&self, url: &str, body: &str) -> Result<ExtractedContent, SearchXyzError> {
        let document = Html::parse_document(body);

        let title = self.extract_title(&document);
        let description = self.extract_meta_description(&document);

        let content_html = self
            .find_main_content(&document)
            .unwrap_or_else(|| self.extract_body_text(&document));

        let cleaned = self.strip_noise(&content_html, url);
        let markdown = self.html_to_markdown(&cleaned);

        match markdown.trim().len() < self.config.min_content_length {
            true => Err(SearchXyzError::EmptyContent {
                url: url.into(),
                min_length: self.config.min_content_length,
            }),
            false => {
                let links = self.extract_links(&document, url);
                Ok(ExtractedContent {
                    url: url.into(),
                    title,
                    description,
                    content_markdown: markdown,
                    links,
                })
            }
        }
    }

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

    fn find_main_content(&self, doc: &Html) -> Option<String> {
        self.content_selectors
            .iter()
            .find_map(|sel| doc.select(sel).next().map(|el| el.inner_html()))
    }

    fn extract_body_text(&self, doc: &Html) -> String {
        let sel = Selector::parse("body").unwrap();
        doc.select(&sel)
            .next()
            .map(|el| el.inner_html())
            .unwrap_or_default()
    }

    fn strip_noise(&self, html: &str, base_url: &str) -> String {
        let fragment = Html::parse_fragment(html);
        let mut output = String::new();
        let base = url::Url::parse(base_url).ok();

        let skip_ids: std::collections::HashSet<_> = self
            .strip_selectors
            .iter()
            .flat_map(|sel| fragment.select(sel))
            .map(|el| el.id())
            .collect();

        fn collect_plain_subtree(
            node: ego_tree::NodeRef<scraper::Node>,
            skip: &std::collections::HashSet<ego_tree::NodeId>,
            preserve_whitespace: bool,
            buf: &mut String,
        ) {
            match skip.contains(&node.id()) {
                true => return,
                false => {}
            }
            match node.value() {
                scraper::Node::Text(t) => match preserve_whitespace {
                    true => buf.push_str(&t.text),
                    false => {
                        let trimmed = t.text.trim();
                        match trimmed.is_empty() {
                            true => {}
                            false => {
                                match !buf.is_empty() && !buf.ends_with(' ') {
                                    true => buf.push(' '),
                                    false => {}
                                }
                                buf.push_str(trimmed);
                            }
                        }
                    }
                },
                _ => {
                    for child in node.children() {
                        collect_plain_subtree(child, skip, preserve_whitespace, buf);
                    }
                }
            }
        }

        fn resolve_href(raw: &str, base: Option<&url::Url>) -> Option<String> {
            let trimmed = raw.trim();
            match trimmed.is_empty()
                || trimmed.starts_with('#')
                || trimmed.starts_with("javascript:")
                || trimmed.starts_with("mailto:")
            {
                true => return None,
                false => {}
            }
            let resolved = match base {
                Some(b) => b.join(trimmed).ok().map(|u| u.to_string()),
                None => url::Url::parse(trimmed).ok().map(|u| u.to_string()),
            }?;
            (resolved.starts_with("http://") || resolved.starts_with("https://"))
                .then_some(resolved)
        }

        fn collect_text(
            node: ego_tree::NodeRef<scraper::Node>,
            skip: &std::collections::HashSet<ego_tree::NodeId>,
            base: Option<&url::Url>,
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
                    match tag {
                        "pre" => {
                            let mut code_buf = String::new();
                            for child in node.children() {
                                collect_plain_subtree(child, skip, true, &mut code_buf);
                            }
                            let trimmed_code = code_buf.trim_matches('\n');
                            match trimmed_code.trim().is_empty() {
                                true => {}
                                false => {
                                    out.push_str("\n```\n");
                                    out.push_str(trimmed_code);
                                    out.push_str("\n```\n");
                                }
                            }
                            return;
                        }
                        "code" => {
                            let mut inline_buf = String::new();
                            for child in node.children() {
                                collect_plain_subtree(child, skip, false, &mut inline_buf);
                            }
                            let trimmed_inline = inline_buf.trim();
                            match trimmed_inline.is_empty() {
                                true => {}
                                false => {
                                    out.push('`');
                                    out.push_str(trimmed_inline);
                                    out.push_str("` ");
                                }
                            }
                            return;
                        }
                        "a" => {
                            let mut anchor_text = String::new();
                            for child in node.children() {
                                collect_plain_subtree(child, skip, false, &mut anchor_text);
                            }
                            let label = anchor_text.trim();
                            let resolved = el.attr("href").and_then(|h| resolve_href(h, base));
                            match (!label.is_empty(), resolved) {
                                (true, Some(href)) => {
                                    out.push('[');
                                    out.push_str(label);
                                    out.push_str("](");
                                    out.push_str(&href);
                                    out.push_str(") ");
                                }
                                (true, None) => {
                                    out.push_str(label);
                                    out.push(' ');
                                }
                                (false, _) => {}
                            }
                            return;
                        }
                        _ => {}
                    }

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
                        "h5" => out.push_str("##### "),
                        "h6" => out.push_str("###### "),
                        "li" => out.push_str("- "),
                        "blockquote" => out.push_str("> "),
                        _ => {}
                    }
                    for child in node.children() {
                        collect_text(child, skip, base, out);
                    }
                    match is_block {
                        true => out.push('\n'),
                        false => {}
                    }
                }
                _ => {
                    for child in node.children() {
                        collect_text(child, skip, base, out);
                    }
                }
            }
        }

        match fragment.tree.root().children().next() {
            Some(root) => collect_text(root, &skip_ids, base.as_ref(), &mut output),
            None => {}
        }

        output
    }

    fn html_to_markdown(&self, text: &str) -> String {
        let mut result = String::with_capacity(text.len());
        let mut prev_blank = false;
        let mut in_fence = false;

        for line in text.lines() {
            let trimmed = line.trim();
            match trimmed.starts_with("```") {
                true => {
                    in_fence = !in_fence;
                    result.push_str(trimmed);
                    result.push('\n');
                    prev_blank = false;
                    continue;
                }
                false => {}
            }
            match in_fence {
                true => {
                    result.push_str(line);
                    result.push('\n');
                }
                false => match (trimmed.is_empty(), prev_blank) {
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
                },
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

fn extract_xml_content(url: &str, body: &str) -> ExtractedContent {
    let doc = Html::parse_document(body);
    let title = Selector::parse("title")
        .ok()
        .and_then(|sel| doc.select(&sel).next())
        .map(|el| el.text().collect::<String>().trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| fallback_url_title(url, "XML Feed"));

    let mut links = Vec::new();
    match Selector::parse("loc") {
        Ok(loc_sel) => {
            for el in doc.select(&loc_sel) {
                let loc = el.text().collect::<String>().trim().to_string();
                match loc.starts_with("http://") || loc.starts_with("https://") {
                    true => links.push(loc),
                    false => {}
                }
            }
        }
        Err(_) => {}
    }
    match Selector::parse("link") {
        Ok(link_sel) => {
            for el in doc.select(&link_sel) {
                let candidate = el
                    .value()
                    .attr("href")
                    .map(|s| s.trim().to_string())
                    .or_else(|| {
                        let txt = el.text().collect::<String>().trim().to_string();
                        (!txt.is_empty()).then_some(txt)
                    });
                match candidate {
                    Some(u) => match u.starts_with("http://") || u.starts_with("https://") {
                        true => links.push(u),
                        false => {}
                    },
                    None => {}
                }
            }
        }
        Err(_) => {}
    }

    let mut lines = Vec::new();
    match Selector::parse("item, entry") {
        Ok(item_sel) => {
            let title_sel = Selector::parse("title").unwrap();
            let desc_sel = Selector::parse("description, summary, content").unwrap();
            for item in doc.select(&item_sel) {
                let item_title = item
                    .select(&title_sel)
                    .next()
                    .map(|e| e.text().collect::<String>().trim().to_string())
                    .unwrap_or_default();
                let item_desc = item
                    .select(&desc_sel)
                    .next()
                    .map(|e| e.text().collect::<String>().trim().to_string())
                    .unwrap_or_default();
                match (!item_title.is_empty(), !item_desc.is_empty()) {
                    (true, true) => lines.push(format!("### {item_title}\n{item_desc}")),
                    (true, false) => lines.push(format!("- {item_title}")),
                    (false, true) => lines.push(item_desc),
                    (false, false) => {}
                }
            }
        }
        Err(_) => {}
    }

    let content_markdown = match (!lines.is_empty(), !links.is_empty()) {
        (true, _) => lines.join("\n\n"),
        (false, true) => links
            .iter()
            .map(|l| format!("- {l}"))
            .collect::<Vec<_>>()
            .join("\n"),
        (false, false) => body.trim().to_string(),
    };

    ExtractedContent {
        url: url.into(),
        title,
        description: "Extracted XML/Feed Content".into(),
        content_markdown,
        links,
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
    fn test_xml_and_json_extraction() {
        let pipeline = ExtractionPipeline::new(ExtractorConfig::default());

        let xml = r#"<?xml version="1.0"?><urlset><url><loc>https://example.com/docs</loc></url></urlset>"#;
        let xml_res = pipeline
            .extract("https://example.com/sitemap.xml", xml, Some("application/xml"))
            .unwrap();
        assert_eq!(xml_res.links, vec!["https://example.com/docs".to_string()]);
        assert!(xml_res.content_markdown.contains("https://example.com/docs"));

        let json_res = pipeline
            .extract(
                "https://api.example.com/v1/status",
                r#"{"ok":true}"#,
                Some("application/json"),
            )
            .unwrap();
        assert!(json_res.content_markdown.contains("```json"));
        assert!(json_res.content_markdown.contains("\"ok\": true"));
    }

    #[test]
    fn test_html_preserves_markdown_links_and_code() {
        let pipeline = ExtractionPipeline::new(ExtractorConfig::default());
        let html = r#"
            <html><head><title>Guide</title></head>
            <body><article>
                <h2>Getting Started</h2>
                <p>Read the <a href="/docs/intro">Tokio Intro</a> and run <code>cargo check</code> inside your workspace today.</p>
                <pre><code>fn main() {
    println!("hi");
}</code></pre>
            </article></body></html>
        "#;
        let res = pipeline
            .extract("https://tokio.rs/guide", html, Some("text/html"))
            .unwrap();
        assert!(res
            .content_markdown
            .contains("[Tokio Intro](https://tokio.rs/docs/intro)"));
        assert!(res.content_markdown.contains("`cargo check`"));
        assert!(res.content_markdown.contains("```\nfn main()"));
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
