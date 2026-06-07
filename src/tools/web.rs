use crate::tools::Tool;
use anyhow::{Result, anyhow};
use reqwest::Client;
use regex::Regex;

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
            .send()
            .await?;

        if !res.status().is_success() {
            return Err(anyhow!("Failed to fetch URL: HTTP {}", res.status()));
        }

        let html = res.text().await?;

        let re_script = Regex::new(r"(?is)<script[^>]*>.*?</script>")?;
        let re_style = Regex::new(r"(?is)<style[^>]*>.*?</style>")?;
        let html_no_scripts = re_script.replace_all(&html, "");
        let html_no_styles = re_style.replace_all(&html_no_scripts, "");

        let re_block = Regex::new(r"(?i)</?(p|div|br|h[1-6]|li|tr|thead|tbody)[^>]*>")?;
        let html_blocks = re_block.replace_all(&html_no_styles, "\n");

        let re_tags = Regex::new(r"<[^>]*>")?;
        let text = re_tags.replace_all(&html_blocks, "");

        let clean_text = text
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

        Ok(serde_json::Value::String(final_text.trim().to_string()))
    }
}
