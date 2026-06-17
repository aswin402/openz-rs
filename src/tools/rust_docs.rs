use crate::tools::Tool;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use scraper::{Html, Selector};

pub struct RustDocsTool {
    client: reqwest::Client,
}

impl RustDocsTool {
    pub fn new() -> Self {
        // Crates.io requires a User-Agent header, otherwise it returns 403 Forbidden.
        let client = reqwest::Client::builder()
            .user_agent("OpenZ-Agent (openz-dev@openz.ai)")
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self { client }
    }
}

#[async_trait::async_trait]
impl Tool for RustDocsTool {
    fn name(&self) -> &str {
        "rust_docs"
    }

    fn description(&self) -> &str {
        "Search for crates on crates.io or read documentation for any third-party Rust crate from docs.rs."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["search", "get_docs"],
                    "description": "The action to perform: 'search' for crates, or 'get_docs' to read documentation."
                },
                "crate_name": {
                    "type": "string",
                    "description": "The name of the Rust crate (e.g. 'tokio', 'serde', 'axum') - required for 'get_docs'."
                },
                "query": {
                    "type": "string",
                    "description": "The search query (required for 'search', or optional for searching inside a crate)."
                },
                "sub_path": {
                    "type": "string",
                    "description": "Optional specific sub-path or item (e.g. 'struct.HashMap.html', 'fn.spawn.html', 'index.html')."
                }
            },
            "required": ["action"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let action = arguments.get("action").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'action' parameter"))?;

        match action {
            "search" => {
                let query = arguments.get("query").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'query' parameter for search action"))?;
                
                let url = format!("https://crates.io/api/v1/crates?q={}&per_page=5", percent_encoding::utf8_percent_encode(query, percent_encoding::NON_ALPHANUMERIC));
                let resp = self.client.get(&url).send().await?;
                
                if !resp.status().is_success() {
                    return Err(anyhow!("Failed to search crates.io (HTTP {})", resp.status()));
                }

                let data: Value = resp.json().await?;
                let crates = data.get("crates").and_then(|v| v.as_array());
                
                let mut results = Vec::new();
                if let Some(arr) = crates {
                    for item in arr {
                        results.push(json!({
                            "name": item.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                            "version": item.get("max_version").and_then(|v| v.as_str()).unwrap_or(""),
                            "description": item.get("description").and_then(|v| v.as_str()).unwrap_or(""),
                            "documentation": item.get("documentation").and_then(|v| v.as_str()).unwrap_or("")
                        }));
                    }
                }

                Ok(json!({
                    "status": "success",
                    "results": results
                }))
            }
            "get_docs" => {
                let crate_name = arguments.get("crate_name").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'crate_name' parameter for get_docs action"))?;
                
                let sub_path = arguments.get("sub_path").and_then(|v| v.as_str()).unwrap_or("index.html");
                
                // Format the URL. Docs.rs uses the structure: https://docs.rs/<crate>/latest/<crate_normalized>/<sub_path>
                // We'll let docs.rs follow redirect from the base URL if we query: https://docs.rs/<crate>/latest/<crate_normalized>/
                // Let's normalize name (dashes to underscores for the module name)
                let module_name = crate_name.replace("-", "_");
                let url = if sub_path.starts_with("http") {
                    sub_path.to_string()
                } else {
                    format!("https://docs.rs/{}/latest/{}/{}", crate_name, module_name, sub_path)
                };

                let resp = self.client.get(&url).send().await?;
                if !resp.status().is_success() {
                    // Fallback to simpler URL if normalized structure failed
                    let fallback_url = format!("https://docs.rs/{}/latest/{}/", crate_name, module_name);
                    let fallback_resp = self.client.get(&fallback_url).send().await?;
                    if !fallback_resp.status().is_success() {
                        return Err(anyhow!("Failed to fetch docs.rs page for {} (HTTP {})", crate_name, fallback_resp.status()));
                    }
                    return self.parse_docs_html(fallback_resp.text().await?, &fallback_url);
                }

                self.parse_docs_html(resp.text().await?, &url)
            }
            _ => Err(anyhow!("Unsupported action: {}", action))
        }
    }
}

impl RustDocsTool {
    fn parse_docs_html(&self, html: String, url: &str) -> Result<Value> {
        let fragment = Html::parse_document(&html);
        
        // Target only the main content section, discarding sidebars/headers
        let main_selectors = vec!["main", "#main-content", ".content"];
        let mut main_html = html.clone();
        
        for selector_str in main_selectors {
            if let Ok(selector) = Selector::parse(selector_str) {
                if let Some(el) = fragment.select(&selector).next() {
                    main_html = el.html();
                    break;
                }
            }
        }

        let markdown = html2md::parse_html(&main_html);
        
        // Limit markdown output size to keep context clean
        let truncated_md = if markdown.len() > 15000 {
            format!("{}\n\n... (content truncated for size) ...", &markdown[..15000])
        } else {
            markdown
        };

        Ok(json!({
            "status": "success",
            "url": url,
            "markdown": truncated_md
        }))
    }
}
