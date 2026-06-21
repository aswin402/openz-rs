use crate::tools::Tool;
use anyhow::{Result, anyhow};
use reqwest::Client;
use regex::Regex;
use scraper::Html;
use scraper::node::Node;
use std::time::Duration;

fn web_re_whitespace() -> &'static Regex {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| Regex::new(r" +").unwrap())
}

fn web_re_newlines() -> &'static Regex {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\n\s*\n").unwrap())
}

/// Validate that an IP address is safe (not private, loopback, or reserved).
fn is_safe_ip(ip: &std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => {
            !(v4.is_private() || v4.is_loopback() || v4.is_link_local()
                || v4.is_unspecified() || v4.is_broadcast())
        }
        std::net::IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4() {
                if !is_safe_ip(&std::net::IpAddr::V4(v4)) {
                    return false;
                }
            }
            if v6.is_loopback() || v6.is_unspecified() || v6.is_multicast() {
                return false;
            }
            let segments = v6.segments();
            // ULA fc00::/7
            if (segments[0] & 0xfe00) == 0xfc00 {
                return false;
            }
            // Link-local fe80::/10
            if (segments[0] & 0xffc0) == 0xfe80 {
                return false;
            }
            true
        }
    }
}

fn validate_url_sync(url: &reqwest::Url) -> Result<()> {
    let host = url.host_str().ok_or_else(|| anyhow!("URL has no host"))?.to_lowercase();

    // Block non-HTTP schemes
    if url.scheme() != "http" && url.scheme() != "https" {
        return Err(anyhow!("SSRF blocked: only http/https URLs are allowed (got '{}')", url.scheme()));
    }

    // Block cloud metadata endpoints by hostname
    if host == "169.254.169.254" || host == "metadata.google.internal" {
        return Err(anyhow!("SSRF blocked: cloud metadata endpoints are not allowed"));
    }

    // If the host is already a literal IP, check it directly
    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        if !is_safe_ip(&ip) {
            return Err(anyhow!("SSRF blocked: private/reserved IP addresses are not allowed"));
        }
    }

    // DNS resolution check
    use std::net::ToSocketAddrs;
    let resolved_ips = format!("{}:0", host)
        .to_socket_addrs()
        .map(|iter| iter.map(|addr| addr.ip()).collect::<Vec<_>>())
        .unwrap_or_default();

    for ip in &resolved_ips {
        if !is_safe_ip(ip) {
            return Err(anyhow!(
                "SSRF blocked: hostname '{}' resolved to private/reserved IP {}",
                host, ip
            ));
        }
    }

    if resolved_ips.is_empty() {
        return Err(anyhow!("SSRF blocked: hostname '{}' could not be resolved to any IP", host));
    }

    Ok(())
}

async fn validate_url(url: &str) -> Result<()> {
    let parsed = reqwest::Url::parse(url).map_err(|e| anyhow!("Invalid URL: {}", e))?;
    validate_url_sync(&parsed)
}

pub struct WebFetchTool {
    client: Client,
}

impl Default for WebFetchTool {
    fn default() -> Self {
        Self::new()
    }
}

impl WebFetchTool {
    pub fn new() -> Self {
        let redirect_policy = reqwest::redirect::Policy::custom(|attempt| {
            if validate_url_sync(attempt.url()).is_err() {
                attempt.stop()
            } else {
                attempt.follow()
            }
        });
        WebFetchTool {
            client: Client::builder()
                .use_rustls_tls()
                .redirect(redirect_policy)
                .build()
                .unwrap_or_default(),
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

        validate_url(url_str).await?;

        let res = self.client.get(url_str)
            .header("user-agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64)")
            .timeout(Duration::from_secs(30))
            .send()
            .await?;

        if !res.status().is_success() {
            return Err(anyhow!("Failed to fetch URL: HTTP {}", res.status()));
        }

        // Check Content-Length to avoid downloading enormous pages
        const MAX_RESPONSE_SIZE: u64 = 10 * 1024 * 1024; // 10MB
        if let Some(content_length) = res.content_length() {
            if content_length > MAX_RESPONSE_SIZE {
                return Err(anyhow!("Response too large ({} bytes, max {} bytes)", content_length, MAX_RESPONSE_SIZE));
            }
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

            let clean_text_spaces = web_re_whitespace().replace_all(&clean_text, " ");
            let final_text = web_re_newlines().replace_all(&clean_text_spaces, "\n");
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

    #[test]
    fn test_is_safe_ip() {
        use std::net::IpAddr;

        // Safe IPv4
        assert!(is_safe_ip(&"1.1.1.1".parse::<IpAddr>().unwrap()));
        assert!(is_safe_ip(&"8.8.8.8".parse::<IpAddr>().unwrap()));

        // Unsafe IPv4
        assert!(!is_safe_ip(&"127.0.0.1".parse::<IpAddr>().unwrap()));
        assert!(!is_safe_ip(&"10.0.0.1".parse::<IpAddr>().unwrap()));
        assert!(!is_safe_ip(&"192.168.1.1".parse::<IpAddr>().unwrap()));
        assert!(!is_safe_ip(&"172.16.0.1".parse::<IpAddr>().unwrap()));
        assert!(!is_safe_ip(&"169.254.169.254".parse::<IpAddr>().unwrap()));
        assert!(!is_safe_ip(&"0.0.0.0".parse::<IpAddr>().unwrap()));

        // Safe IPv6
        assert!(is_safe_ip(&"2001:db8::1".parse::<IpAddr>().unwrap()));

        // Unsafe IPv6
        assert!(!is_safe_ip(&"::1".parse::<IpAddr>().unwrap()));
        assert!(!is_safe_ip(&"::".parse::<IpAddr>().unwrap()));
        assert!(!is_safe_ip(&"fc00::1".parse::<IpAddr>().unwrap())); // ULA
        assert!(!is_safe_ip(&"fe80::1".parse::<IpAddr>().unwrap())); // Link-Local
        assert!(!is_safe_ip(&"ff02::1".parse::<IpAddr>().unwrap())); // Multicast

        // IPv4-mapped IPv6
        assert!(!is_safe_ip(&"::ffff:127.0.0.1".parse::<IpAddr>().unwrap()));
        assert!(!is_safe_ip(&"::ffff:10.0.0.1".parse::<IpAddr>().unwrap()));
        assert!(is_safe_ip(&"::ffff:8.8.8.8".parse::<IpAddr>().unwrap()));
    }

    #[tokio::test]
    async fn test_validate_url() {
        assert!(validate_url("http://example.com").await.is_ok());
        assert!(validate_url("https://google.com/search?q=rust").await.is_ok());

        assert!(validate_url("ftp://example.com").await.is_err());
        assert!(validate_url("http://127.0.0.1").await.is_err());
        assert!(validate_url("http://localhost").await.is_err());
        assert!(validate_url("http://169.254.169.254").await.is_err());
        assert!(validate_url("http://[::1]").await.is_err());
        assert!(validate_url("http://[fc00::1]").await.is_err());
        assert!(validate_url("http://[::ffff:127.0.0.1]").await.is_err());
    }
}
