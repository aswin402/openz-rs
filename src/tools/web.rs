use crate::tools::Tool;
use anyhow::{Result, anyhow};
use reqwest::Client;
use regex::Regex;
use scraper::Html;
use scraper::node::Node;
use std::time::Duration;

pub struct WebFetchTool {
    client: Client,
}

impl WebFetchTool {
    pub fn new() -> Self {
        WebFetchTool {
            client: Client::builder().use_rustls_tls().build().unwrap_or_default(),
        }
    }
}

fn walk_nodes(node: ego_tree::NodeRef<'_, Node>, text: &mut String) {
    match node.value() {
        Node::Text(t) => {
            text.push_str(&t.text);
        }
        Node::Element(e) => {
            let tag_name = e.name();
            if tag_name == "script" || tag_name == "style" || tag_name == "head" {
                return;
            }

            let is_block = matches!(
                tag_name,
                "p" | "div" | "br" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "li" | "tr" | "thead" | "tbody"
            );
            if is_block {
                text.push('\n');
            }
            for child in node.children() {
                walk_nodes(child, text);
            }
            if is_block {
                text.push('\n');
            }
        }
        _ => {
            for child in node.children() {
                walk_nodes(child, text);
            }
        }
    }
}

#[async_trait::async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &str {
        "web_fetch"
    }

    fn description(&self) -> &str {
        "Fetch contents of a web page and return it as clean plain text."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "url": { "type": "string", "description": "The URL to fetch" }
            },
            "required": ["url"]
        })
    }

    async fn call(&self, arguments: &serde_json::Value) -> Result<serde_json::Value> {
        let url_str = arguments.get("url").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'url' argument"))?;

        let res = self.client.get(url_str)
            .header("user-agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64)")
            .timeout(Duration::from_secs(30))
            .send()
            .await?;

        if !res.status().is_success() {
            return Err(anyhow!("Failed to fetch URL: HTTP {}", res.status()));
        }

        let html = res.text().await?;

        let result_text = {
            // Parse HTML DOM using scraper
            let document = Html::parse_document(&html);
            let mut raw_text = String::new();
            walk_nodes(document.tree.root(), &mut raw_text);

            // Replace html entities
            let clean_text = raw_text
                .replace("&nbsp;", " ")
                .replace("&lt;", "<")
                .replace("&gt;", ">")
                .replace("&amp;", "&")
                .replace("&quot;", "\"")
                .replace("&#39;", "'");

            let re_whitespace = Regex::new(r" +")?;
            let re_newlines = Regex::new(r"\n\s*\n")?;
            let clean_text_spaces = re_whitespace.replace_all(&clean_text, " ");
            let final_text = re_newlines.replace_all(&clean_text_spaces, "\n");
            final_text.trim().to_string()
        };

        let _ = crate::tools::shared_memory::archive_research_entry(url_str, &result_text, &format!("web_fetch: {}", url_str)).await;

        Ok(serde_json::Value::String(result_text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_html_parsing() {
        let html = r#"
            <html>
                <head>
                    <title>Test Page</title>
                    <style>body { color: red; }</style>
                </head>
                <body>
                    <h1>Hello World</h1>
                    <p>This is a <b>test</b> page.</p>
                    <script>console.log("ignore me");</script>
                </body>
            </html>
        "#;

        let document = Html::parse_document(html);
        let mut raw_text = String::new();
        walk_nodes(document.tree.root(), &mut raw_text);

        let clean = raw_text.trim();
        assert!(clean.contains("Hello World"));
        assert!(clean.contains("This is a test page."));
        assert!(!clean.contains("body {"));
        assert!(!clean.contains("console.log"));
    }
}
