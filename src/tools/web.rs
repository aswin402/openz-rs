#[cfg(test)]
use crate::tools::browser_broker::browser_backend_priority;
use crate::tools::browser_broker::render_page_with_browser_broker;
use crate::tools::searchxyz::core::crawler::ssrf::{
    build_pinned_http_client, is_safe_ip as ssrf_is_safe_ip, is_supported_web_content_type,
    read_bounded_response_bytes, ssrf_redirect_policy as shared_ssrf_redirect_policy,
    validate_http_url_addrs, validate_url_sync_with_policy, validate_url_without_dns,
};
#[cfg(test)]
use crate::tools::shared_memory::web_cache::compute_expires_at;
use crate::tools::shared_memory::web_cache::{
    is_cache_fresh, load_http_cache, now_utc, refresh_http_cache_validators, save_http_cache,
    touch_http_cache, CachedHttpEntry, WebCacheMode,
};
use crate::tools::Tool;
use anyhow::{anyhow, Result};
use reqwest::header;
use scraper::node::Node;
use scraper::Html;

type WebFetchCacheMode = WebCacheMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WebFetchOutputFormat {
    Text,
    Markdown,
}

impl WebFetchOutputFormat {
    fn from_args(arguments: &serde_json::Value) -> Self {
        let raw = arguments
            .get("format")
            .or_else(|| arguments.get("output_format"))
            .or_else(|| arguments.get("outputFormat"))
            .and_then(|v| v.as_str())
            .unwrap_or("text")
            .trim()
            .to_ascii_lowercase();
        match raw.as_str() {
            "markdown" | "md" => Self::Markdown,
            _ => Self::Text,
        }
    }
}

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
    ssrf_is_safe_ip(ip)
}

pub fn validate_url_sync(url: &reqwest::Url) -> Result<std::net::IpAddr> {
    let addrs = validate_url_sync_with_policy(url, false).map_err(|err| anyhow!("{err}"))?;
    addrs
        .first()
        .map(std::net::SocketAddr::ip)
        .ok_or_else(|| anyhow!("SSRF blocked: URL resolved to zero socket addresses"))
}

pub async fn validate_url(url: &str) -> Result<std::net::IpAddr> {
    let (_parsed, _host, addrs) = validate_http_url_addrs(url, false)
        .await
        .map_err(|err| anyhow!("{err}"))?;
    addrs
        .first()
        .map(std::net::SocketAddr::ip)
        .ok_or_else(|| anyhow!("SSRF blocked: URL resolved to zero socket addresses"))
}

pub fn ssrf_redirect_policy() -> reqwest::redirect::Policy {
    let max_redirects = crate::tools::searchxyz::get_server()
        .config
        .crawler
        .max_redirects;
    shared_ssrf_redirect_policy(max_redirects, false)
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

fn extract_title_from_html(html: &str) -> Option<String> {
    let document = Html::parse_document(html);
    let selector = scraper::Selector::parse("title").ok()?;
    document
        .select(&selector)
        .next()
        .map(|el| el.text().collect::<Vec<_>>().join(" ").trim().to_string())
        .filter(|title| !title.is_empty())
}

fn render_body_for_output(
    body: &str,
    content_type: &str,
    final_url: &str,
    format: WebFetchOutputFormat,
) -> (String, Option<String>) {
    let ct_lc = content_type.to_ascii_lowercase();
    let looks_like_html = ct_lc.contains("text/html")
        || ct_lc.contains("application/xhtml+xml")
        || (ct_lc.trim().is_empty()
            && (body.contains("<html") || body.contains("<body") || body.contains("<!doctype")));

    match looks_like_html {
        false => (body.trim().to_string(), None),
        true => match format {
            WebFetchOutputFormat::Markdown => {
                match crate::tools::searchxyz::get_server()
                    .extractor
                    .extract(final_url, body, Some(content_type))
                {
                    Ok(extracted) => {
                        let title = (!extracted.title.trim().is_empty()).then_some(extracted.title);
                        (extracted.content_markdown, title)
                    }
                    Err(_) => (extract_text_from_html(body), extract_title_from_html(body)),
                }
            }
            WebFetchOutputFormat::Text => {
                (extract_text_from_html(body), extract_title_from_html(body))
            }
        },
    }
}

fn web_fetch_render_js_enabled(arguments: &serde_json::Value) -> bool {
    let val = arguments
        .get("render_js")
        .or_else(|| arguments.get("renderJs"));
    match val {
        Some(serde_json::Value::Bool(b)) => *b,
        Some(serde_json::Value::String(s)) => {
            let s = s.trim().to_ascii_lowercase();
            s != "false" && s != "0" && s != "no"
        }
        _ => true,
    }
}

fn web_fetch_force_render_js(arguments: &serde_json::Value) -> bool {
    let val = arguments
        .get("force_render_js")
        .or_else(|| arguments.get("forceRenderJs"));
    match val {
        Some(serde_json::Value::Bool(b)) => *b,
        Some(serde_json::Value::String(s)) => {
            matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes")
        }
        _ => false,
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

#[cfg(test)]
fn web_fetch_browser_render_backend_names() -> [&'static str; 1] {
    browser_backend_priority().map(|backend| backend.as_str())
}

async fn web_fetch_browser_render(url: &str) -> Result<(String, String)> {
    let timeout_secs = crate::tools::searchxyz::get_server()
        .config
        .crawler
        .timeout_secs
        .clamp(10, 45);
    let rendered = render_page_with_browser_broker(url, timeout_secs).await?;
    validate_url(&rendered.final_url).await?;
    (!rendered.html.trim().is_empty())
        .then_some((rendered.final_url, rendered.html))
        .ok_or_else(|| anyhow!("Browser render returned empty HTML"))
}

pub struct WebFetchTool;

impl Default for WebFetchTool {
    fn default() -> Self {
        Self::new()
    }
}

impl WebFetchTool {
    pub fn new() -> Self {
        WebFetchTool
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

const MAX_BATCH_WEB_FETCH_URLS: usize = 12;

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

const WEB_FETCH_BATCH_URL_ALIASES: &[&str] = &["urls", "targets", "links", "pages", "endpoints"];

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

pub fn extract_web_fetch_urls(arguments: &serde_json::Value) -> Vec<String> {
    let raw_items = match arguments {
        serde_json::Value::Array(items) => Some(items),
        serde_json::Value::Object(map) => WEB_FETCH_BATCH_URL_ALIASES
            .iter()
            .find_map(|key| map.get(*key).and_then(|v| v.as_array())),
        _ => None,
    };

    let mut seen = std::collections::HashSet::new();
    let mut normalized_urls = Vec::new();
    for item in raw_items.into_iter().flatten() {
        let candidate = item
            .as_str()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(normalize_web_url);
        match candidate {
            Some(url) => {
                let _ = (seen.insert(url.clone())
                    && normalized_urls.len() < MAX_BATCH_WEB_FETCH_URLS)
                    .then(|| normalized_urls.push(url));
            }
            None => {}
        }
    }
    normalized_urls
}

fn return_cached_entry(
    url_str: &str,
    cached_item: &CachedHttpEntry,
    output_format: WebFetchOutputFormat,
    max_length: Option<usize>,
) -> (String, Option<String>) {
    let _ = touch_http_cache(url_str);
    let (rendered, title) = render_body_for_output(
        &cached_item.body_text,
        &cached_item.content_type,
        &cached_item.final_url,
        output_format,
    );
    (truncate_web_fetch_output(rendered, max_length), title)
}

async fn fetch_single_url(
    normalized_url: &str,
    arguments: &serde_json::Value,
) -> Result<(String, Option<String>)> {
    let url_str = normalized_url;
    let max_length = parse_max_length(arguments);
    let cache_mode = WebFetchCacheMode::from_args(arguments)?;
    let output_format = WebFetchOutputFormat::from_args(arguments);
    let depth = crate::tools::arguments::extract_usize_arg(arguments, &["depth"])
        .unwrap_or(1)
        .clamp(1, 3);

    let server = crate::tools::searchxyz::get_server();

    // Multi-page depth>1 delegates directly to SearchXyz's unified Spider + Extractor pipeline
    match depth > 1 {
        true => {
            let server_cache_mode = match cache_mode {
                WebFetchCacheMode::Auto => crate::tools::searchxyz::server::FetchCacheMode::Auto,
                WebFetchCacheMode::PreferCache => {
                    crate::tools::searchxyz::server::FetchCacheMode::PreferCache
                }
                WebFetchCacheMode::Revalidate => {
                    crate::tools::searchxyz::server::FetchCacheMode::Revalidate
                }
                WebFetchCacheMode::Bypass => {
                    crate::tools::searchxyz::server::FetchCacheMode::Bypass
                }
            };
            let markdown = server
                .read_url(crate::tools::searchxyz::server::ReadUrlRequest {
                    url: normalized_url.to_string(),
                    depth: Some(depth),
                    render_js: Some(web_fetch_force_render_js(arguments)),
                    cache_mode: Some(server_cache_mode),
                    save_mode: None,
                    include_diagnostics: Some(false),
                    max_chars: max_length,
                })
                .await?;
            return Ok((markdown, None));
        }
        false => {}
    }

    // YouTube transcript extraction shortcut
    match crate::tools::searchxyz::core::crawler::youtube::extract_video_id(url_str) {
        Some(vid) => {
            match crate::tools::searchxyz::core::crawler::youtube::fetch_youtube_transcript(
                &server.crawler,
                &vid,
            )
            .await
            {
                Ok(transcript) => {
                    let yt_title = format!("YouTube Video {vid}");
                    let _ = crate::tools::shared_memory::archive_research_entry_with_meta(
                        url_str,
                        &yt_title,
                        &transcript,
                        &format!("web_fetch:youtube:{vid}"),
                        Some(url_str),
                    )
                    .await;
                    return Ok((
                        truncate_web_fetch_output(transcript, max_length),
                        Some(yt_title),
                    ));
                }
                Err(err) => {
                    tracing::debug!(video_id = %vid, error = ?err, "YouTube transcript unavailable; falling back to page fetch");
                }
            }
        }
        None => {}
    }

    let server = crate::tools::searchxyz::get_server();
    let default_ttl_secs = server.config.crawler.default_cache_ttl_secs as i64;
    let max_response_bytes = server.config.crawler.max_body_bytes;

    let cached = match cache_mode {
        WebFetchCacheMode::Bypass => None,
        _ => load_http_cache(url_str).unwrap_or_else(|err| {
            tracing::debug!(error = ?err, url = %url_str, "web_fetch cache lookup skipped");
            None
        }),
    };

    match cached.as_ref() {
        Some(cached_item) => {
            let use_immediately = match cache_mode {
                WebFetchCacheMode::PreferCache => true,
                WebFetchCacheMode::Auto => is_cache_fresh(&cached_item.expires_at, now_utc()),
                _ => false,
            };
            match use_immediately {
                true => {
                    validate_url_without_dns(url_str, false).map_err(|err| anyhow!("{err}"))?;
                    return Ok(return_cached_entry(
                        url_str,
                        cached_item,
                        output_format,
                        max_length,
                    ));
                }
                false => {}
            }
        }
        None => {}
    }

    let (_parsed_url, host_lc, resolved_addrs) = validate_http_url_addrs(url_str, false)
        .await
        .map_err(|err| anyhow!("{err}"))?;

    match web_fetch_force_render_js(arguments) {
        true => {
            let (final_url, rendered_html) = web_fetch_browser_render(url_str).await?;
            let empty_headers = header::HeaderMap::new();
            let _ = (cache_mode != WebFetchCacheMode::Bypass).then(|| {
                save_http_cache(
                    url_str,
                    &final_url,
                    &rendered_html,
                    "text/html",
                    &empty_headers,
                    200,
                    default_ttl_secs,
                )
            });
            let (rendered_output, title) = render_body_for_output(
                &rendered_html,
                "text/html",
                &final_url,
                output_format,
            );
            let _ = crate::tools::shared_memory::archive_research_entry_with_meta(
                url_str,
                title.as_deref().unwrap_or(url_str),
                &rendered_output,
                &format!("web_fetch: {}", url_str),
                Some(&final_url),
            )
            .await;
            return Ok((
                truncate_web_fetch_output(rendered_output, max_length),
                title,
            ));
        }
        false => {}
    }

    let client =
        build_pinned_http_client(&server.config.crawler, &host_lc, &resolved_addrs, None)?;

    let base_request = client.get(url_str);
    let request = match (cache_mode != WebFetchCacheMode::Bypass, cached.as_ref()) {
        (true, Some(cached_item)) => {
            let req_with_etag = match cached_item.etag.as_deref() {
                Some(etag) => base_request.header(header::IF_NONE_MATCH, etag),
                None => base_request,
            };
            match cached_item.last_modified.as_deref() {
                Some(last_modified) => req_with_etag.header(header::IF_MODIFIED_SINCE, last_modified),
                None => req_with_etag,
            }
        }
        _ => base_request,
    };

    let res = match request.send().await {
        Ok(res) => res,
        Err(err) => match cached.as_ref() {
            Some(cached_item) => {
                tracing::warn!(error = ?err, url = %url_str, "web_fetch live request failed; using stale cached response");
                return Ok(return_cached_entry(
                    url_str,
                    cached_item,
                    output_format,
                    max_length,
                ));
            }
            None => return Err(err.into()),
        },
    };

    let final_url = res.url().to_string();
    validate_url(&final_url).await?;

    match (
        res.status() == reqwest::StatusCode::NOT_MODIFIED,
        cached.as_ref(),
    ) {
        (true, Some(cached_item)) => {
            let headers = res.headers().clone();
            let _ = refresh_http_cache_validators(url_str, &headers, default_ttl_secs)
                .or_else(|_| touch_http_cache(url_str));
            return Ok(return_cached_entry(
                url_str,
                cached_item,
                output_format,
                max_length,
            ));
        }
        _ => {}
    }

    match res.status().is_success() {
        true => {}
        false => match cached.as_ref() {
            Some(cached_item) => {
                tracing::warn!(status = %res.status(), url = %url_str, "web_fetch live request returned error; using stale cached response");
                return Ok(return_cached_entry(
                    url_str,
                    cached_item,
                    output_format,
                    max_length,
                ));
            }
            None => return Err(anyhow!("Failed to fetch URL: HTTP {}", res.status())),
        },
    }

    let status_code = res.status().as_u16();
    let headers = res.headers().clone();
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    match is_supported_web_content_type(&content_type) {
        true => {}
        false => {
            return Err(anyhow!(
                "Unsupported Content-Type '{}' for URL '{}'",
                content_type,
                url_str
            ));
        }
    }

    let body_bytes = read_bounded_response_bytes(res, url_str, max_response_bytes)
        .await
        .map_err(|err| anyhow!("{err}"))?;

    let is_pdf = content_type.to_ascii_lowercase().contains("application/pdf")
        || body_bytes.starts_with(b"%PDF-");

    let (mut stored_body, mut stored_ct, mut effective_final_url) = match is_pdf {
        true => {
            let pdf_text = pdf_extract::extract_text_from_mem(&body_bytes)
                .map_err(|err| anyhow!("Failed to extract PDF text: {err}"))?;
            (pdf_text, "text/plain".to_string(), final_url)
        }
        false => {
            let raw_text = String::from_utf8_lossy(&body_bytes).into_owned();
            let ct = match content_type.trim().is_empty() {
                true => "text/html".to_string(),
                false => content_type,
            };
            (raw_text, ct, final_url)
        }
    };

    let (mut result_text, mut page_title) = render_body_for_output(
        &stored_body,
        &stored_ct,
        &effective_final_url,
        output_format,
    );

    let should_retry_js = !is_pdf
        && stored_ct.to_ascii_lowercase().contains("html")
        && web_fetch_should_retry_browser_render(&stored_body, &result_text, arguments);

    match should_retry_js {
        true => match web_fetch_browser_render(url_str).await {
            Ok((rendered_final_url, rendered_html)) => {
                let (rendered_output, rendered_title) = render_body_for_output(
                    &rendered_html,
                    "text/html",
                    &rendered_final_url,
                    output_format,
                );
                match rendered_output.trim().is_empty() {
                    false => {
                        tracing::info!(url = %url_str, "web_fetch static output looked like a JS shell; using headless-first BrowserBroker output");
                        stored_body = rendered_html;
                        stored_ct = "text/html".to_string();
                        effective_final_url = rendered_final_url;
                        result_text = rendered_output;
                        page_title = rendered_title;
                    }
                    true => {
                        tracing::warn!(url = %url_str, "web_fetch browser render retry returned empty text; using static fetch output");
                    }
                }
            }
            Err(err) => {
                tracing::warn!(url = %url_str, error = ?err, "web_fetch browser render retry failed; using static fetch output");
            }
        },
        false => {}
    }

    match cache_mode != WebFetchCacheMode::Bypass {
        true => {
            let _ = save_http_cache(
                url_str,
                &effective_final_url,
                &stored_body,
                &stored_ct,
                &headers,
                status_code,
                default_ttl_secs,
            )
            .map_err(|err| {
                tracing::debug!(error = ?err, url = %url_str, "web_fetch cache save skipped");
                err
            });
        }
        false => {}
    }

    let _ = crate::tools::shared_memory::archive_research_entry_with_meta(
        url_str,
        page_title.as_deref().unwrap_or(url_str),
        &result_text,
        &format!("web_fetch: {}", url_str),
        Some(&effective_final_url),
    )
    .await;

    Ok((truncate_web_fetch_output(result_text, max_length), page_title))
}

#[async_trait::async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &str {
        "web_fetch"
    }

    fn description(&self) -> &str {
        "Fetch one URL ('url') or multiple URLs concurrently ('urls', up to 12) — web pages, JSON/REST APIs, CSV endpoints, PDFs, RSS/Atom XML feeds, or YouTube transcripts — returning clean plain text, JSON, or Markdown without shell approval prompts."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "url": { "type": "string", "description": "Single URL to fetch (supports bare domains, href/target aliases, JSON APIs, RSS feeds, PDFs, or YouTube links)." },
                "urls": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional list of up to 12 URLs to fetch concurrently in a single call instead of shell curl loops."
                },
                "format": {
                    "type": "string",
                    "enum": ["text", "markdown"],
                    "description": "Output format: 'text' (default clean plain text) or 'markdown' (readability-extracted Markdown)."
                },
                "cache_mode": {
                    "type": "string",
                    "enum": ["auto", "prefer_cache", "revalidate", "bypass"],
                    "description": "Exact-URL cache policy. auto uses fresh cached responses and revalidates stale ones; prefer_cache returns any cached response first; revalidate always sends conditional validators; bypass ignores cache."
                },
                "render_js": {
                    "type": "boolean",
                    "description": "Automatically retry through OpenZ's headless-first BrowserBroker when static fetch looks like an empty JavaScript app shell. Defaults to true; set false to opt out."
                },
                "depth": {
                    "type": "integer",
                    "description": "Link-following depth (1 = single page default, 2-3 = follow internal links on the same domain)."
                },
                "max_length": {
                    "type": "integer",
                    "description": "Optional maximum character limit per fetched URL. Truncated output includes a notice with the total length."
                }
            }
        })
    }

    async fn call(&self, arguments: &serde_json::Value) -> Result<serde_json::Value> {
        let batch_urls = extract_web_fetch_urls(arguments);
        match batch_urls.is_empty() {
            true => {
                let normalized_url = extract_web_fetch_url(arguments)?;
                let (content, _title) = fetch_single_url(&normalized_url, arguments).await?;
                Ok(serde_json::Value::String(content))
            }
            false => {
                let concurrency = crate::tools::searchxyz::get_server()
                    .config
                    .crawler
                    .max_concurrent_fetches
                    .clamp(1, 8);
                let sem = std::sync::Arc::new(tokio::sync::Semaphore::new(concurrency));
                let tasks = batch_urls.iter().map(|url| {
                    let sem = sem.clone();
                    async move {
                        let _permit = sem.acquire_owned().await.ok();
                        match fetch_single_url(url, arguments).await {
                            Ok((content, title)) => (
                                true,
                                None,
                                serde_json::json!({
                                    "url": url,
                                    "title": title,
                                    "status": "success",
                                    "content": content,
                                }),
                            ),
                            Err(err) => {
                                let err_text = err.to_string();
                                (
                                    false,
                                    Some(err_text.clone()),
                                    serde_json::json!({
                                        "url": url,
                                        "status": "error",
                                        "error": err_text,
                                    }),
                                )
                            }
                        }
                    }
                });

                let outcomes = futures_util::future::join_all(tasks).await;
                let mut fetched = 0usize;
                let mut failed = 0usize;
                let mut first_error: Option<String> = None;
                let mut results = Vec::with_capacity(outcomes.len());
                for (ok, maybe_err, entry) in outcomes {
                    match ok {
                        true => fetched += 1,
                        false => {
                            failed += 1;
                            let _ = first_error
                                .is_none()
                                .then(|| first_error = maybe_err);
                        }
                    }
                    results.push(entry);
                }

                match fetched > 0 {
                    true => Ok(serde_json::json!({
                        "status": "success",
                        "fetched": fetched,
                        "failed": failed,
                        "results": results,
                    })),
                    false => Err(anyhow!(
                        "Failed to fetch all {} URLs: {}",
                        failed,
                        first_error.unwrap_or_else(|| "unknown fetch error".to_string())
                    )),
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "web_tests.rs"]
mod tests;
