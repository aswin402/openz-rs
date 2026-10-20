use crate::memory::MemoryService;
use crate::tools::Tool;
use anyhow::{anyhow, Result};
use futures_util::StreamExt;
use reqwest::{header, Client};
use rusqlite::OptionalExtension;
use scraper::node::Node;
use scraper::Html;
use std::time::Duration;

const WEB_CONNECT_TIMEOUT_SECS: u64 = 10;
const WEB_READ_TIMEOUT_SECS: u64 = 30;
const WEB_TOTAL_TIMEOUT_SECS: u64 = 45;

fn collapse_spaces(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut prev_space = false;
    for ch in input.chars() {
        let is_space = ch == ' ';
        let _ = (!(is_space && prev_space)).then(|| out.push(ch));
        prev_space = is_space;
    }
    out
}

fn collapse_blank_lines(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(nl_pos) = rest.find('\n') {
        out.push_str(&rest[..nl_pos]);
        let after_nl = &rest[nl_pos + 1..];
        let ws_len = after_nl
            .char_indices()
            .take_while(|(_, ch)| ch.is_whitespace())
            .last()
            .map(|(idx, ch)| idx + ch.len_utf8())
            .unwrap_or(0);
        let ws_prefix = &after_nl[..ws_len];
        out.push('\n');
        rest = match ws_prefix.rfind('\n') {
            Some(last_nl_in_ws) => &after_nl[last_nl_in_ws + 1..],
            None => after_nl,
        };
    }
    out.push_str(rest);
    out
}

/// Validate that an IP address is safe (not private, loopback, or reserved).
pub fn is_safe_ip(ip: &std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => {
            !(v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast())
        }
        std::net::IpAddr::V6(v6) => {
            let v4_ok = v6
                .to_ipv4()
                .map(|v4| is_safe_ip(&std::net::IpAddr::V4(v4)))
                .unwrap_or(true);
            let segments = v6.segments();
            let is_ula = (segments[0] & 0xfe00) == 0xfc00;
            let is_link_local = (segments[0] & 0xffc0) == 0xfe80;
            v4_ok
                && !v6.is_loopback()
                && !v6.is_unspecified()
                && !v6.is_multicast()
                && !is_ula
                && !is_link_local
        }
    }
}

pub fn validate_url_sync(url: &reqwest::Url) -> Result<std::net::IpAddr> {
    let host = url
        .host_str()
        .ok_or_else(|| anyhow!("URL has no host"))?
        .to_lowercase();

    // Block non-HTTP schemes
    matches!(url.scheme(), "http" | "https")
        .then_some(())
        .ok_or_else(|| {
            anyhow!(
                "SSRF blocked: only http/https URLs are allowed (got '{}')",
                url.scheme()
            )
        })?;

    // Block cloud metadata endpoints by hostname
    (!matches!(host.as_str(), "169.254.169.254" | "metadata.google.internal"))
        .then_some(())
        .ok_or_else(|| anyhow!("SSRF blocked: cloud metadata endpoints are not allowed"))?;

    // If the host is already a literal IP, check it directly
    match host.parse::<std::net::IpAddr>() {
        Ok(ip) => {
            is_safe_ip(&ip).then_some(ip).ok_or_else(|| {
                anyhow!("SSRF blocked: private/reserved IP addresses are not allowed")
            })
        }
        Err(_) => {
            // DNS resolution check
            use std::net::ToSocketAddrs;
            let resolved_ips = format!("{}:0", host)
                .to_socket_addrs()
                .map(|iter| iter.map(|addr| addr.ip()).collect::<Vec<_>>())
                .unwrap_or_default();

            for ip in &resolved_ips {
                is_safe_ip(ip).then_some(()).ok_or_else(|| {
                    anyhow!(
                        "SSRF blocked: hostname '{}' resolved to private/reserved IP {}",
                        host,
                        ip
                    )
                })?;
            }

            resolved_ips.first().copied().ok_or_else(|| {
                anyhow!(
                    "SSRF blocked: hostname '{}' could not be resolved to any IP",
                    host
                )
            })
        }
    }
}

pub async fn validate_url(url: &str) -> Result<std::net::IpAddr> {
    let parsed = reqwest::Url::parse(url).map_err(|e| anyhow!("Invalid URL: {}", e))?;
    validate_url_sync(&parsed)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WebFetchCacheMode {
    Auto,
    PreferCache,
    Revalidate,
    Bypass,
}

impl WebFetchCacheMode {
    fn from_args(arguments: &serde_json::Value) -> Result<Self> {
        let raw = arguments
            .get("cache_mode")
            .or_else(|| arguments.get("cacheMode"))
            .and_then(|v| v.as_str())
            .unwrap_or("auto")
            .trim()
            .to_lowercase();
        match raw.as_str() {
            "auto" | "" => Ok(Self::Auto),
            "prefer_cache" | "prefer-cache" | "cache" => Ok(Self::PreferCache),
            "revalidate" | "validate" | "refresh" => Ok(Self::Revalidate),
            "bypass" | "no_cache" | "no-cache" | "fresh" => Ok(Self::Bypass),
            other => Err(anyhow!(
                "Invalid cache_mode '{}'. Use auto, prefer_cache, revalidate, or bypass.",
                other
            )),
        }
    }
}

#[derive(Debug, Clone)]
struct CachedWebFetch {
    body_text: String,
    etag: Option<String>,
    last_modified: Option<String>,
    expires_at: String,
}

fn now_utc() -> chrono::DateTime<chrono::Utc> {
    chrono::Utc::now()
}

fn parse_http_date(raw: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::parse_from_rfc2822(raw)
        .ok()
        .map(|dt| dt.with_timezone(&chrono::Utc))
}

fn cache_control_directives(cache_control: &str) -> Vec<String> {
    cache_control
        .split(',')
        .map(|part| part.trim().to_ascii_lowercase())
        .filter(|part| !part.is_empty())
        .collect()
}

fn cache_control_max_age(cache_control: &str) -> Option<i64> {
    cache_control_directives(cache_control)
        .into_iter()
        .find_map(|directive| {
            directive
                .strip_prefix("max-age=")
                .and_then(|raw| raw.trim_matches('"').parse::<i64>().ok())
        })
        .filter(|age| *age >= 0)
}

fn cache_control_has(cache_control: &str, needle: &str) -> bool {
    cache_control_directives(cache_control)
        .iter()
        .any(|directive| directive == needle)
}

fn compute_expires_at(
    cache_control: Option<&str>,
    last_modified: Option<&str>,
    fetched_at: chrono::DateTime<chrono::Utc>,
) -> Option<chrono::DateTime<chrono::Utc>> {
    match cache_control.map(str::trim).filter(|s| !s.is_empty()) {
        Some(cc) => {
            (!cache_control_has(cc, "no-store")).then_some(())?;
            let immediate_revalidate =
                cache_control_has(cc, "no-cache") || cache_control_has(cc, "must-revalidate");
            match immediate_revalidate {
                true => return Some(fetched_at),
                false => {}
            }
            match cache_control_max_age(cc) {
                Some(max_age) => return Some(fetched_at + chrono::Duration::seconds(max_age)),
                None => {}
            }
        }
        None => {}
    }

    last_modified
        .and_then(parse_http_date)
        .map(|lm| {
            let age = fetched_at.signed_duration_since(lm).num_seconds().max(0);
            let heuristic = (age / 10).clamp(60, 86_400);
            fetched_at + chrono::Duration::seconds(heuristic)
        })
        .or(Some(fetched_at))
}

fn is_cache_fresh(expires_at: &str, now: chrono::DateTime<chrono::Utc>) -> bool {
    chrono::DateTime::parse_from_rfc3339(expires_at)
        .ok()
        .map(|dt| dt.with_timezone(&chrono::Utc) > now)
        .unwrap_or(false)
}

fn header_to_string(headers: &header::HeaderMap, name: header::HeaderName) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn load_cached_web_fetch(url: &str) -> Result<Option<CachedWebFetch>> {
    MemoryService::current().with_shared_db(|conn| {
        let mut stmt = conn.prepare(
            "SELECT body_text, etag, last_modified, expires_at FROM web_fetch_cache WHERE url = ?1",
        )?;
        let cached = stmt
            .query_row(rusqlite::params![url], |row| {
                Ok(CachedWebFetch {
                    body_text: row.get(0)?,
                    etag: row.get(1)?,
                    last_modified: row.get(2)?,
                    expires_at: row.get(3)?,
                })
            })
            .optional()?;
        Ok(cached)
    })
}

fn save_cached_web_fetch(
    url: &str,
    body_text: &str,
    headers: &header::HeaderMap,
    status_code: u16,
) -> Result<()> {
    let fetched_at = now_utc();
    let etag = header_to_string(headers, header::ETAG);
    let last_modified = header_to_string(headers, header::LAST_MODIFIED);
    let cache_control = header_to_string(headers, header::CACHE_CONTROL);
    let expires_at = match compute_expires_at(
        cache_control.as_deref(),
        last_modified.as_deref(),
        fetched_at,
    ) {
        Some(expires_at) => expires_at,
        None => return Ok(()),
    };
    MemoryService::current().with_shared_db(|conn| {
        conn.execute(
            "INSERT INTO web_fetch_cache (url, body_text, etag, last_modified, cache_control, fetched_at, expires_at, status_code, use_count)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0)
             ON CONFLICT(url) DO UPDATE SET body_text=excluded.body_text, etag=excluded.etag, last_modified=excluded.last_modified, cache_control=excluded.cache_control, fetched_at=excluded.fetched_at, expires_at=excluded.expires_at, status_code=excluded.status_code",
            rusqlite::params![
                url,
                body_text,
                etag,
                last_modified,
                cache_control,
                fetched_at.to_rfc3339(),
                expires_at.to_rfc3339(),
                i64::from(status_code),
            ],
        )?;
        Ok(())
    })
}

fn refresh_cached_web_fetch_validators(url: &str, headers: &header::HeaderMap) -> Result<()> {
    let fetched_at = now_utc();
    let etag = header_to_string(headers, header::ETAG);
    let last_modified = header_to_string(headers, header::LAST_MODIFIED);
    let cache_control = header_to_string(headers, header::CACHE_CONTROL);
    let expires_at = match compute_expires_at(
        cache_control.as_deref(),
        last_modified.as_deref(),
        fetched_at,
    ) {
        Some(expires_at) => expires_at,
        None => return Ok(()),
    };
    MemoryService::current().with_shared_db(|conn| {
        conn.execute(
            "UPDATE web_fetch_cache
             SET etag = COALESCE(?2, etag),
                 last_modified = COALESCE(?3, last_modified),
                 cache_control = COALESCE(?4, cache_control),
                 fetched_at = ?5,
                 expires_at = ?6,
                 use_count = use_count + 1
             WHERE url = ?1",
            rusqlite::params![
                url,
                etag,
                last_modified,
                cache_control,
                fetched_at.to_rfc3339(),
                expires_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    })
}

fn mark_cached_web_fetch_used(url: &str) -> Result<()> {
    MemoryService::current().with_shared_db(|conn| {
        conn.execute(
            "UPDATE web_fetch_cache SET use_count = use_count + 1 WHERE url = ?1",
            rusqlite::params![url],
        )?;
        Ok(())
    })
}

fn extract_text_from_html(html: &str) -> String {
    let document = Html::parse_document(html);
    let mut raw_text = String::new();
    walk_nodes(document.tree.root(), &mut raw_text);

    let clean_text = raw_text
        .replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'");

    let clean_text_spaces = collapse_spaces(&clean_text);
    let final_text = collapse_blank_lines(&clean_text_spaces);
    final_text.trim().to_string()
}

fn web_fetch_render_js_enabled(arguments: &serde_json::Value) -> bool {
    let val = arguments
        .get("render_js")
        .or_else(|| arguments.get("renderJs"));
    match val {
        Some(serde_json::Value::Bool(b)) => *b,
        Some(serde_json::Value::String(s)) => {
            let s = s.trim().to_lowercase();
            s != "false" && s != "0" && s != "no"
        }
        _ => true,
    }
}

fn web_fetch_should_retry_browser_render(
    html: &str,
    extracted_text: &str,
    arguments: &serde_json::Value,
) -> bool {
    web_fetch_render_js_enabled(arguments) && {
        let visible_chars = extracted_text.trim().chars().count();
        let normalized = html.to_ascii_lowercase();
        let script_count = normalized.matches("<script").count();
        let has_app_root = [
            "id=\"root\"",
            "id='root'",
            "id=\"app\"",
            "id='app'",
            "data-reactroot",
        ]
        .iter()
        .any(|needle| normalized.contains(needle));
        let has_js_required_text = [
            "enable javascript",
            "requires javascript",
            "javascript is required",
            "you need javascript",
            "please enable js",
        ]
        .iter()
        .any(|needle| normalized.contains(needle));

        visible_chars < 240 && (has_js_required_text || (has_app_root && script_count > 0))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WebFetchBrowserRenderBackend {
    Gsd,
    Firefox,
    Obscura,
}

impl WebFetchBrowserRenderBackend {
    fn name(self) -> &'static str {
        match self {
            Self::Gsd => "gsd_browser",
            Self::Firefox => "firefox_browser",
            Self::Obscura => "obscura_browser",
        }
    }
}

fn web_fetch_browser_render_backends() -> [WebFetchBrowserRenderBackend; 3] {
    [
        WebFetchBrowserRenderBackend::Gsd,
        WebFetchBrowserRenderBackend::Firefox,
        WebFetchBrowserRenderBackend::Obscura,
    ]
}

#[cfg(test)]
fn web_fetch_browser_render_backend_names() -> [&'static str; 3] {
    web_fetch_browser_render_backends().map(WebFetchBrowserRenderBackend::name)
}

fn web_fetch_extract_browser_output(value: serde_json::Value) -> Result<String> {
    value
        .as_str()
        .or_else(|| value.get("output").and_then(|output| output.as_str()))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("Browser render returned no readable text"))
}

async fn web_fetch_render_with_backend(
    backend: WebFetchBrowserRenderBackend,
    url: &str,
) -> Result<String> {
    match backend {
        WebFetchBrowserRenderBackend::Gsd => {
            let browser = crate::tools::gsd_browser::GsdBrowserTool;
            browser
                .call(&serde_json::json!({
                    "action": "navigate",
                    "url": url,
                }))
                .await?;
            let page_source = browser
                .call(&serde_json::json!({ "action": "page_source" }))
                .await?;
            let html = page_source
                .get("output")
                .and_then(|output| output.as_str())
                .ok_or_else(|| anyhow!("gsd_browser page_source returned no output"))?;
            let markdown = html2md::parse_html(html).trim().to_string();
            (!markdown.is_empty())
                .then_some(markdown)
                .ok_or_else(|| anyhow!("gsd_browser rendered empty text"))
        }
        WebFetchBrowserRenderBackend::Firefox => {
            let browser = crate::tools::firefox::FirefoxBrowserTool::new();
            let rendered = browser
                .call(&serde_json::json!({
                    "url": url,
                    "action": "render",
                }))
                .await?;
            web_fetch_extract_browser_output(rendered)
        }
        WebFetchBrowserRenderBackend::Obscura => {
            let browser = crate::tools::obscura::ObscuraBrowserTool;
            let rendered = browser
                .call(&serde_json::json!({
                    "url": url,
                    "action": "render",
                    "timeout": 20,
                }))
                .await?;
            web_fetch_extract_browser_output(rendered)
        }
    }
}

async fn web_fetch_browser_render(url: &str) -> Result<String> {
    let mut failures = Vec::new();
    for backend in web_fetch_browser_render_backends() {
        match web_fetch_render_with_backend(backend, url).await {
            Ok(text) => return Ok(text),
            Err(err) => {
                tracing::warn!(
                    backend = backend.name(),
                    url = %url,
                    error = ?err,
                    "web_fetch browser render backend failed; trying next backend"
                );
                failures.push(format!("{}: {}", backend.name(), err));
            }
        }
    }

    Err(anyhow!(
        "All browser render backends failed: {}",
        failures.join("; ")
    ))
}

pub fn ssrf_redirect_policy() -> reqwest::redirect::Policy {
    reqwest::redirect::Policy::custom(|attempt| match validate_url_sync(attempt.url()) {
        Ok(_) => attempt.follow(),
        Err(_) => attempt.stop(),
    })
}

pub struct WebFetchTool {
    #[allow(dead_code)]
    client: Client,
}

impl Default for WebFetchTool {
    fn default() -> Self {
        Self::new()
    }
}

impl WebFetchTool {
    pub fn new() -> Self {
        WebFetchTool {
            client: Client::builder()
                .use_rustls_tls()
                .redirect(ssrf_redirect_policy())
                .connect_timeout(Duration::from_secs(WEB_CONNECT_TIMEOUT_SECS))
                .read_timeout(Duration::from_secs(WEB_READ_TIMEOUT_SECS))
                .timeout(Duration::from_secs(WEB_TOTAL_TIMEOUT_SECS))
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
            match matches!(tag_name, "script" | "style" | "head") {
                true => return,
                false => {}
            }

            let is_block = matches!(
                tag_name,
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
                    | "thead"
                    | "tbody"
            );
            let _ = is_block.then(|| text.push('\n'));
            for child in node.children() {
                walk_nodes(child, text);
            }
            let _ = is_block.then(|| text.push('\n'));
        }
        _ => {
            for child in node.children() {
                walk_nodes(child, text);
            }
        }
    }
}

/// Normalize web URL by trimming, stripping wrapper characters, and auto-prefixing https:// for scheme-less URLs.
pub fn normalize_web_url(raw: &str) -> String {
    let trimmed_outer = raw.trim();
    let is_wrapped = (trimmed_outer.starts_with('<') && trimmed_outer.ends_with('>'))
        || (trimmed_outer.starts_with('"') && trimmed_outer.ends_with('"'))
        || (trimmed_outer.starts_with('\'') && trimmed_outer.ends_with('\''));
    let trimmed = match is_wrapped {
        true => trimmed_outer[1..trimmed_outer.len() - 1].trim(),
        false => trimmed_outer,
    };
    match (trimmed.starts_with("//"), trimmed.contains("://")) {
        (true, _) => format!("https:{}", trimmed),
        (false, false) => format!("https://{}", trimmed),
        (false, true) => trimmed.to_string(),
    }
}

const WEB_FETCH_URL_ALIASES: &[&str] = &[
    "url",
    "target_url",
    "targetUrl",
    "uri",
    "link",
    "target",
    "href",
    "page",
    "endpoint",
    "address",
    "site",
    "website",
    "domain",
    "host",
    "path",
];

pub fn parse_max_length(arguments: &serde_json::Value) -> Option<usize> {
    crate::tools::arguments::extract_usize_arg(
        arguments,
        &["max_length", "maxLength", "limit", "max_chars", "maxChars"],
    )
}

pub fn truncate_web_fetch_output(text: String, max_length: Option<usize>) -> String {
    let total = text.chars().count();
    match max_length.filter(|&limit| limit > 0 && total > limit) {
        Some(limit) => {
            let truncated: String = text.chars().take(limit).collect();
            format!(
                "{}\n\n[Content truncated at {} chars; total length: {} chars]",
                truncated, limit, total
            )
        }
        None => text,
    }
}

pub fn extract_web_fetch_url(arguments: &serde_json::Value) -> Result<String> {
    crate::tools::arguments::extract_normalized_url_from_aliases(
        arguments,
        WEB_FETCH_URL_ALIASES,
        "Missing 'url' argument",
        "Missing 'url' argument (received empty string)",
    )
}

fn return_cached_body(
    url_str: &str,
    body_text: String,
    max_length: Option<usize>,
) -> serde_json::Value {
    let _ = mark_cached_web_fetch_used(url_str);
    serde_json::Value::String(truncate_web_fetch_output(body_text, max_length))
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
                "url": { "type": "string", "description": "The URL to fetch (supports bare domains, href/target aliases, or direct string)" },
                "cache_mode": {
                    "type": "string",
                    "enum": ["auto", "prefer_cache", "revalidate", "bypass"],
                    "description": "Exact-URL cache policy. auto uses fresh cached responses and revalidates stale ones; prefer_cache returns any cached response first; revalidate always sends conditional validators; bypass ignores cache."
                },
                "render_js": {
                    "type": "boolean",
                    "description": "Automatically retry through local browser rendering when static fetch looks like an empty JavaScript app shell. Defaults to true; set false to opt out."
                },
                "max_length": {
                    "type": "integer",
                    "description": "Optional maximum character limit for returned text. Truncated output includes a notice with the total length."
                }
            },
            "required": ["url"]
        })
    }

    async fn call(&self, arguments: &serde_json::Value) -> Result<serde_json::Value> {
        let normalized_url = extract_web_fetch_url(arguments)?;
        let url_str = &normalized_url;
        let max_length = parse_max_length(arguments);
        let cache_mode = WebFetchCacheMode::from_args(arguments)?;
        let cached = load_cached_web_fetch(url_str).unwrap_or_else(|err| {
            tracing::debug!(error = ?err, url = %url_str, "web_fetch cache lookup skipped");
            None
        });

        match cached.as_ref() {
            Some(cached_item) => {
                let use_immediately = match cache_mode {
                    WebFetchCacheMode::PreferCache => true,
                    WebFetchCacheMode::Auto => is_cache_fresh(&cached_item.expires_at, now_utc()),
                    _ => false,
                };
                match use_immediately {
                    true => {
                        return Ok(return_cached_body(
                            url_str,
                            cached_item.body_text.clone(),
                            max_length,
                        ));
                    }
                    false => {}
                }
            }
            None => {}
        }

        let resolved_ip = validate_url(url_str).await?;

        // Parse host and port to build the resolved Client
        let parsed_url = reqwest::Url::parse(url_str).map_err(|e| anyhow!("Invalid URL: {}", e))?;
        let host = parsed_url
            .host_str()
            .ok_or_else(|| anyhow!("URL has no host"))?
            .to_lowercase();
        let port = parsed_url.port_or_known_default().unwrap_or(80);

        let socket_addr = std::net::SocketAddr::new(resolved_ip, port);
        let client = Client::builder()
            .use_rustls_tls()
            .redirect(ssrf_redirect_policy())
            .resolve(&host, socket_addr)
            .connect_timeout(Duration::from_secs(WEB_CONNECT_TIMEOUT_SECS))
            .read_timeout(Duration::from_secs(WEB_READ_TIMEOUT_SECS))
            .timeout(Duration::from_secs(WEB_TOTAL_TIMEOUT_SECS))
            .build()?;

        let base_request = client
            .get(url_str)
            .header("user-agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64)");
        let request = match (cache_mode != WebFetchCacheMode::Bypass, cached.as_ref()) {
            (true, Some(cached_item)) => {
                let req_with_etag = match cached_item.etag.as_deref() {
                    Some(etag) => base_request.header(header::IF_NONE_MATCH, etag),
                    None => base_request,
                };
                match cached_item.last_modified.as_deref() {
                    Some(last_modified) => {
                        req_with_etag.header(header::IF_MODIFIED_SINCE, last_modified)
                    }
                    None => req_with_etag,
                }
            }
            _ => base_request,
        };

        let res = match request.send().await {
            Ok(res) => res,
            Err(err) => match cached {
                Some(cached_item) => {
                    tracing::warn!(error = ?err, url = %url_str, "web_fetch live request failed; using stale cached response");
                    return Ok(return_cached_body(url_str, cached_item.body_text, max_length));
                }
                None => return Err(err.into()),
            },
        };

        match (res.status() == reqwest::StatusCode::NOT_MODIFIED, cached.as_ref()) {
            (true, Some(cached_item)) => {
                let headers = res.headers().clone();
                let _ = refresh_cached_web_fetch_validators(url_str, &headers)
                    .or_else(|_| mark_cached_web_fetch_used(url_str));
                return Ok(serde_json::Value::String(truncate_web_fetch_output(
                    cached_item.body_text.clone(),
                    max_length,
                )));
            }
            _ => {}
        }

        match res.status().is_success() {
            true => {}
            false => match cached {
                Some(cached_item) => {
                    tracing::warn!(status = %res.status(), url = %url_str, "web_fetch live request returned error; using stale cached response");
                    return Ok(return_cached_body(url_str, cached_item.body_text, max_length));
                }
                None => return Err(anyhow!("Failed to fetch URL: HTTP {}", res.status())),
            },
        }

        let status_code = res.status().as_u16();
        let headers = res.headers().clone();

        // Check Content-Length to avoid downloading enormous pages
        const MAX_RESPONSE_SIZE: usize = 10 * 1024 * 1024; // 10MB
        match res
            .content_length()
            .filter(|&len| len > MAX_RESPONSE_SIZE as u64)
        {
            Some(content_length) => {
                return Err(anyhow!(
                    "Response too large ({} bytes, max {} bytes)",
                    content_length,
                    MAX_RESPONSE_SIZE
                ));
            }
            None => {}
        }

        let mut stream = res.bytes_stream();
        let mut body_bytes = Vec::new();
        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result?;
            (body_bytes.len() + chunk.len() <= MAX_RESPONSE_SIZE)
                .then_some(())
                .ok_or_else(|| {
                    anyhow!("Response too large (exceeds max {} bytes)", MAX_RESPONSE_SIZE)
                })?;
            body_bytes.extend_from_slice(&chunk);
        }
        let html = String::from_utf8_lossy(&body_bytes).into_owned();
        let mut result_text = extract_text_from_html(&html);

        match web_fetch_should_retry_browser_render(&html, &result_text, arguments) {
            true => match web_fetch_browser_render(url_str).await {
                Ok(rendered_text) => match rendered_text.trim().is_empty() {
                    false => {
                        tracing::info!(url = %url_str, "web_fetch static output looked like a JS shell; using browser-rendered text");
                        result_text = rendered_text;
                    }
                    true => {
                        tracing::warn!(url = %url_str, "web_fetch browser render retry returned empty text; using static fetch output");
                    }
                },
                Err(err) => {
                    tracing::warn!(url = %url_str, error = ?err, "web_fetch browser render retry failed; using static fetch output");
                }
            },
            false => {}
        }

        let _ =
            save_cached_web_fetch(url_str, &result_text, &headers, status_code).map_err(|err| {
                tracing::debug!(error = ?err, url = %url_str, "web_fetch cache save skipped");
                err
            });

        let _ = crate::tools::shared_memory::archive_research_entry(
            url_str,
            &result_text,
            &format!("web_fetch: {}", url_str),
        )
        .await;

        Ok(serde_json::Value::String(truncate_web_fetch_output(
            result_text,
            max_length,
        )))
    }
}

#[cfg(test)]
#[path = "web_tests.rs"]
mod tests;

