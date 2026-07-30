pub mod fast_spider;
pub mod fingerprint;
pub mod github;
pub mod headless;
pub mod sitemap;
pub mod spider;
pub mod youtube;

use headless::HeadlessBrowser;
use std::net::IpAddr;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;

use governor::{clock::DefaultClock, state::keyed::DefaultKeyedStateStore, Quota, RateLimiter};
use reqwest::{header, redirect::Policy, Client, StatusCode};
use tokio::net::lookup_host;
use tokio::sync::Mutex;
use url::Url;

use crate::cache::{Cache, CacheEntry};
use crate::config::CrawlerConfig;
use crate::error::SearchXyzError;

/// Per-domain keyed rate limiter type.
type DomainRateLimiter = RateLimiter<String, DefaultKeyedStateStore<String>, DefaultClock>;

/// The crawler fetches HTML pages with timeouts, retries, and
/// per-domain rate limiting.
pub struct Crawler {
    clients: Vec<Client>,
    config: CrawlerConfig,
    rate_limiter: Arc<DomainRateLimiter>,
    cache: Arc<Mutex<Cache>>,
    headless_browser: HeadlessBrowser,
}

/// Raw fetch result before extraction.
#[derive(Debug, Clone)]
pub struct FetchResult {
    pub url: String,
    pub final_url: String, // after redirects
    pub body: String,
    pub content_type: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FetchCacheMode {
    Auto,
    PreferCache,
    Revalidate,
    Bypass,
}

impl FetchCacheMode {
    fn may_read_cache(self) -> bool {
        matches!(self, FetchCacheMode::Auto | FetchCacheMode::PreferCache)
    }

    fn may_write_cache(self) -> bool {
        !matches!(self, FetchCacheMode::Bypass)
    }
}

impl Crawler {
    pub fn new(
        config: CrawlerConfig,
        headless_config: crate::config::HeadlessConfig,
        proxy_config: crate::config::ProxyConfig,
        cache: Arc<Mutex<Cache>>,
    ) -> Self {
        let mut clients = Vec::new();
        if proxy_config.enabled && !proxy_config.urls.is_empty() {
            for proxy_url in &proxy_config.urls {
                match reqwest::Proxy::all(proxy_url) {
                    Ok(proxy) => {
                        match Client::builder()
                            .timeout(Duration::from_secs(config.timeout_secs))
                            .connect_timeout(Duration::from_secs(10))
                            .user_agent(&config.user_agent)
                            .redirect(Policy::limited(config.max_redirects))
                            .pool_max_idle_per_host(4)
                            .gzip(true)
                            .brotli(true)
                            .proxy(proxy)
                            .build()
                        {
                            Ok(client) => clients.push(client),
                            Err(e) => {
                                tracing::error!(proxy_url, error = %e, "Failed to build proxied HTTP client");
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!(proxy_url, error = %e, "Failed to parse proxy URL");
                    }
                }
            }
        }

        if clients.is_empty() {
            let client = Client::builder()
                .timeout(Duration::from_secs(config.timeout_secs))
                .connect_timeout(Duration::from_secs(10))
                .user_agent(&config.user_agent)
                .redirect(Policy::limited(config.max_redirects))
                .pool_max_idle_per_host(4)
                .gzip(true)
                .brotli(true)
                .build()
                .expect("Failed to build default HTTP client");
            clients.push(client);
        }

        // Per-domain rate limiter: N requests/sec per domain.
        let quota = Quota::per_second(
            NonZeroU32::new(config.rate_limit_per_sec as u32)
                .unwrap_or(NonZeroU32::new(2).unwrap()),
        );
        let rate_limiter = Arc::new(RateLimiter::keyed(quota));
        let headless_browser = HeadlessBrowser::new(headless_config, proxy_config);

        Self {
            clients,
            config,
            rate_limiter,
            cache,
            headless_browser,
        }
    }

    pub fn clients(&self) -> &[Client] {
        &self.clients
    }

    pub fn headless_browser(&self) -> &HeadlessBrowser {
        &self.headless_browser
    }

    /// Fetch a URL, respecting cache, rate limits, and retries.
    pub async fn fetch_url(
        &self,
        url: &str,
        render_js: bool,
    ) -> Result<FetchResult, SearchXyzError> {
        self.fetch_url_with_cache_mode(url, render_js, FetchCacheMode::Auto)
            .await
    }

    /// Fetch a URL with explicit cache behavior.
    pub async fn fetch_url_with_cache_mode(
        &self,
        url: &str,
        render_js: bool,
        cache_mode: FetchCacheMode,
    ) -> Result<FetchResult, SearchXyzError> {
        validate_http_url(url, self.config.allow_private_network).await?;

        // ── 1. Check cache ──
        let cached_entry = {
            let cache = self.cache.lock().await;
            if cache_mode.may_read_cache() {
                if let Some(entry) = cache.get(url) {
                    tracing::debug!(url, "Cache hit");
                    return Ok(FetchResult {
                        url: url.to_string(),
                        final_url: url.to_string(),
                        body: entry.content.clone(),
                        content_type: entry
                            .content_type
                            .clone()
                            .unwrap_or_else(|| "text/html".into()),
                    });
                }
                if matches!(cache_mode, FetchCacheMode::PreferCache) {
                    tracing::debug!(url, "Prefer-cache requested but no fresh entry found");
                }
            }
            cache.get_any(url).cloned()
        };

        // ── 2. Rate limit ──
        let domain = Url::parse(url)
            .map(|u| u.host_str().unwrap_or("unknown").to_string())
            .unwrap_or_else(|_| "unknown".into());

        self.rate_limiter.until_key_ready(&domain).await;

        // ── 3. Headless JS execution if requested ──
        if render_js {
            tracing::info!(url, "Fetching URL with headless browser");
            let body = self.headless_browser.fetch_html(url).await?;

            // Cache the response
            if cache_mode.may_write_cache() {
                let mut cache = self.cache.lock().await;
                cache.put(
                    url.to_string(),
                    CacheEntry::new(body.clone(), url.to_string()),
                );
            }

            return Ok(FetchResult {
                url: url.to_string(),
                final_url: url.to_string(),
                body,
                content_type: "text/html".into(),
            });
        }

        // ── 4. Fetch with retries (exponential backoff) ──
        let allow_stale_fallback = !matches!(cache_mode, FetchCacheMode::Bypass);
        let mut attempt = 0u32;
        loop {
            attempt += 1;

            let mut headers = fingerprint::HeaderGenerator::random_headers();
            if matches!(cache_mode, FetchCacheMode::Revalidate) {
                if let Some(entry) = &cached_entry {
                    if let Some(etag) = &entry.etag {
                        if let Ok(value) = etag.parse() {
                            headers.insert(header::IF_NONE_MATCH, value);
                        }
                    }
                    if let Some(last_modified) = &entry.last_modified {
                        if let Ok(value) = last_modified.parse() {
                            headers.insert(header::IF_MODIFIED_SINCE, value);
                        }
                    }
                }
            }
            use rand::seq::IndexedRandom;
            let client = self
                .clients
                .choose(&mut rand::rng())
                .unwrap_or(&self.clients[0]);
            let resp = client.get(url).headers(headers).send().await;

            match resp {
                Ok(response) => {
                    let final_url = response.url().to_string();
                    validate_http_url(&final_url, self.config.allow_private_network).await?;
                    let status = response.status();

                    // ── Handle common HTTP errors ──
                    match status {
                        StatusCode::OK => {}

                        StatusCode::NOT_MODIFIED => {
                            if let Some(entry) = cached_entry {
                                tracing::debug!(url, "Origin returned 304; reusing cached body");
                                if cache_mode.may_write_cache() {
                                    let mut refreshed = entry.clone();
                                    refreshed.fetched_at = chrono::Utc::now();
                                    let mut cache = self.cache.lock().await;
                                    cache.put(url.to_string(), refreshed);
                                }
                                return Ok(FetchResult {
                                    url: url.into(),
                                    final_url,
                                    body: entry.content,
                                    content_type: entry
                                        .content_type
                                        .unwrap_or_else(|| "text/html".into()),
                                });
                            }
                            return Err(SearchXyzError::HttpError {
                                url: url.into(),
                                status: 304,
                                reason: "Origin returned 304 but SearchXyz has no cached body"
                                    .into(),
                            });
                        }

                        StatusCode::FORBIDDEN => {
                            return Err(SearchXyzError::HttpError {
                                url: url.into(),
                                status: 403,
                                reason: "Access forbidden — site blocks automated access".into(),
                            });
                        }

                        StatusCode::NOT_FOUND => {
                            return Err(SearchXyzError::HttpError {
                                url: url.into(),
                                status: 404,
                                reason: "Page not found".into(),
                            });
                        }

                        StatusCode::TOO_MANY_REQUESTS => {
                            if attempt <= self.config.max_retries {
                                let delay = Duration::from_millis(1000 * 2u64.pow(attempt - 1));
                                tracing::warn!(url, attempt, "Rate limited, backing off");
                                tokio::time::sleep(delay).await;
                                continue;
                            }
                            return Err(SearchXyzError::RateLimited {
                                provider: domain,
                                retry_after_secs: 60,
                            });
                        }

                        StatusCode::INTERNAL_SERVER_ERROR | StatusCode::SERVICE_UNAVAILABLE => {
                            if attempt <= self.config.max_retries {
                                let delay = Duration::from_millis(500 * 2u64.pow(attempt - 1));
                                tracing::warn!(
                                    url, status = %status, attempt,
                                    "Server error, retrying"
                                );
                                tokio::time::sleep(delay).await;
                                continue;
                            }
                            let err = SearchXyzError::HttpError {
                                url: url.into(),
                                status: status.as_u16(),
                                reason: format!(
                                    "Server error after {} attempts",
                                    self.config.max_retries
                                ),
                            };
                            if allow_stale_fallback {
                                if let Some(result) =
                                    stale_fallback(url, cached_entry.clone(), err.to_string())
                                {
                                    return Ok(result);
                                }
                            }
                            return Err(err);
                        }

                        other if !other.is_success() => {
                            return Err(SearchXyzError::HttpError {
                                url: url.into(),
                                status: other.as_u16(),
                                reason: format!("Unexpected status: {other}"),
                            });
                        }

                        _ => {} // other 2xx — proceed
                    }

                    // ── Content-Type guard ──
                    let content_type = response
                        .headers()
                        .get(reqwest::header::CONTENT_TYPE)
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("")
                        .to_string();

                    let etag = response_header_string(&response, header::ETAG);
                    let last_modified = response_header_string(&response, header::LAST_MODIFIED);
                    let cache_control = response_header_string(&response, header::CACHE_CONTROL);

                    let is_pdf = content_type.contains("application/pdf");
                    let is_supported = is_pdf
                        || content_type.contains("text/html")
                        || content_type.contains("text/plain")
                        || content_type.contains("application/xhtml");

                    if !is_supported {
                        return Err(SearchXyzError::CrawlFailed {
                            url: url.into(),
                            reason: format!(
                                "Unsupported Content-Type: {content_type}. \
                                 Only HTML pages and PDF documents are supported."
                            ),
                        });
                    }

                    // ── Size guard ──
                    if let Some(len) = response.content_length() {
                        if len as usize > self.config.max_body_bytes {
                            return Err(SearchXyzError::CrawlFailed {
                                url: url.into(),
                                reason: format!(
                                    "Response too large ({len} bytes, max {})",
                                    self.config.max_body_bytes
                                ),
                            });
                        }
                    }

                    // ── Read response bytes ──
                    let bytes =
                        response
                            .bytes()
                            .await
                            .map_err(|e| SearchXyzError::CrawlFailed {
                                url: url.into(),
                                reason: format!("Failed to read response bytes: {e}"),
                            })?;

                    if bytes.len() > self.config.max_body_bytes {
                        return Err(SearchXyzError::CrawlFailed {
                            url: url.into(),
                            reason: format!("Body exceeds limit ({} bytes)", bytes.len()),
                        });
                    }

                    // ── Extract body string (HTML/text vs PDF) ──
                    let body = if is_pdf {
                        tracing::info!(url, "Extracting text from PDF document");
                        pdf_extract::extract_text_from_mem(&bytes).map_err(|e| {
                            SearchXyzError::ExtractionFailed {
                                url: url.into(),
                                reason: format!("Failed to extract PDF text: {e}"),
                            }
                        })?
                    } else {
                        String::from_utf8_lossy(&bytes).into_owned()
                    };

                    // ── Cache the response ──
                    if cache_mode.may_write_cache() {
                        let mut entry = CacheEntry::new(body.clone(), url.to_string());
                        entry.etag = etag;
                        entry.last_modified = last_modified;
                        entry.cache_control = cache_control;
                        entry.content_type = Some(content_type.clone());

                        let mut cache = self.cache.lock().await;
                        cache.put(url.to_string(), entry);
                    }

                    return Ok(FetchResult {
                        url: url.into(),
                        final_url,
                        body,
                        content_type,
                    });
                }

                Err(e) => {
                    // Network-level error — retry on transient failures.
                    if attempt <= self.config.max_retries && (e.is_timeout() || e.is_connect()) {
                        let delay = Duration::from_millis(500 * 2u64.pow(attempt - 1));
                        tracing::warn!(
                            url, error = %e, attempt,
                            "Transient error, retrying"
                        );
                        tokio::time::sleep(delay).await;
                        continue;
                    }
                    let err = SearchXyzError::from(e);
                    if allow_stale_fallback {
                        if let Some(result) =
                            stale_fallback(url, cached_entry.clone(), err.to_string())
                        {
                            return Ok(result);
                        }
                    }
                    return Err(err);
                }
            }
        }
    }
}

pub async fn validate_public_http_url(url: &str) -> Result<(), SearchXyzError> {
    validate_http_url(url, false).await
}

pub async fn validate_http_url(
    url: &str,
    allow_private_network: bool,
) -> Result<(), SearchXyzError> {
    let parsed = Url::parse(url).map_err(|e| SearchXyzError::UnsafeUrl {
        url: url.to_string(),
        reason: format!("invalid URL: {e}"),
    })?;

    match parsed.scheme() {
        "http" | "https" => {}
        scheme => {
            return Err(SearchXyzError::UnsafeUrl {
                url: url.to_string(),
                reason: format!("unsupported scheme `{scheme}`"),
            });
        }
    }

    let host = parsed.host_str().ok_or_else(|| SearchXyzError::UnsafeUrl {
        url: url.to_string(),
        reason: "missing host".to_string(),
    })?;
    let host_lc = host.to_ascii_lowercase();
    if !allow_private_network
        && (matches!(host_lc.as_str(), "localhost" | "localhost.localdomain")
            || host_lc.ends_with(".localhost"))
    {
        return Err(SearchXyzError::UnsafeUrl {
            url: url.to_string(),
            reason: "localhost targets are not allowed".to_string(),
        });
    }

    if let Ok(ip) = host_lc.parse::<IpAddr>() {
        if !allow_private_network {
            reject_private_ip(url, ip)?;
        }
        return Ok(());
    }

    let port = parsed.port_or_known_default().unwrap_or(443);
    let addrs = lookup_host((host, port))
        .await
        .map_err(|e| SearchXyzError::UnsafeUrl {
            url: url.to_string(),
            reason: format!("DNS lookup failed: {e}"),
        })?;

    if !allow_private_network {
        for addr in addrs {
            reject_private_ip(url, addr.ip())?;
        }
    }

    Ok(())
}

fn stale_fallback(
    url: &str,
    cached_entry: Option<CacheEntry>,
    reason: String,
) -> Option<FetchResult> {
    let entry = cached_entry?;
    tracing::warn!(url, reason = %reason, "Live fetch failed; returning stale cached body");
    Some(FetchResult {
        url: url.to_string(),
        final_url: url.to_string(),
        body: entry.content,
        content_type: entry.content_type.unwrap_or_else(|| "text/html".into()),
    })
}

fn response_header_string(
    response: &reqwest::Response,
    name: header::HeaderName,
) -> Option<String> {
    response
        .headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_string())
}

fn reject_private_ip(url: &str, ip: IpAddr) -> Result<(), SearchXyzError> {
    let blocked = match ip {
        IpAddr::V4(v4) => {
            v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_documentation()
                || v4.is_unspecified()
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_unique_local()
                || v6.is_unicast_link_local()
                || (v6.segments()[0] == 0x2001 && v6.segments()[1] == 0x0db8)
        }
    };

    if blocked {
        return Err(SearchXyzError::UnsafeUrl {
            url: url.to_string(),
            reason: format!("host resolves to non-public address {ip}"),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::Cache;
    use crate::config::{CrawlerConfig, HeadlessConfig, ProxyConfig};
    use std::sync::Arc;
    use tokio::sync::Mutex;

    #[tokio::test]
    async fn revalidate_falls_back_to_stale_cache_on_fetch_failure() {
        let cache = Arc::new(Mutex::new(Cache::new(10, 60)));
        let mut entry = CacheEntry::new(
            "<html><body>stale fallback</body></html>".to_string(),
            "http://127.0.0.1:9/unreachable".to_string(),
        );
        entry.ttl_secs = 0;
        entry.etag = Some(r#""old""#.to_string());
        entry.content_type = Some("text/html".to_string());
        cache
            .lock()
            .await
            .put("http://127.0.0.1:9/unreachable".to_string(), entry);

        let mut crawler_config = CrawlerConfig::default();
        crawler_config.allow_private_network = true;
        crawler_config.max_retries = 0;
        let crawler = Crawler::new(
            crawler_config,
            HeadlessConfig::default(),
            ProxyConfig::default(),
            cache,
        );

        let result = crawler
            .fetch_url_with_cache_mode(
                "http://127.0.0.1:9/unreachable",
                false,
                FetchCacheMode::Revalidate,
            )
            .await
            .unwrap();

        assert!(result.body.contains("stale fallback"));
        assert_eq!(result.content_type, "text/html");
    }

    #[tokio::test]
    async fn revalidate_uses_etag_and_returns_cached_body_on_304() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let requests = Arc::new(AtomicUsize::new(0));
        let request_counter = requests.clone();

        tokio::spawn(async move {
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut buf = [0_u8; 2048];
                let n = stream.read(&mut buf).await.unwrap();
                let request = String::from_utf8_lossy(&buf[..n]);
                let count = request_counter.fetch_add(1, Ordering::SeqCst);

                if count == 0 {
                    let body = "<html><body>cached body</body></html>";
                    let response = format!(
                        r#"HTTP/1.1 200 OK
content-type: text/html
etag: "v1"
content-length: {}
connection: close

{}"#,
                        body.len(),
                        body
                    );
                    stream.write_all(response.as_bytes()).await.unwrap();
                } else {
                    assert!(
                        request.contains(r#"if-none-match: "v1""#)
                            || request.contains(r#"If-None-Match: "v1""#),
                        "revalidation request should send If-None-Match, got: {request}"
                    );
                    let response = "HTTP/1.1 304 Not Modified
connection: close

";
                    stream.write_all(response.as_bytes()).await.unwrap();
                }
            }
        });

        let cache = Arc::new(Mutex::new(Cache::new(10, 60)));
        let mut crawler_config = CrawlerConfig::default();
        crawler_config.allow_private_network = true;
        let crawler = Crawler::new(
            crawler_config,
            HeadlessConfig::default(),
            ProxyConfig::default(),
            cache,
        );
        let url = format!("http://{addr}/etag");

        let first = crawler
            .fetch_url_with_cache_mode(&url, false, FetchCacheMode::Auto)
            .await
            .unwrap();
        assert!(first.body.contains("cached body"));

        let second = crawler
            .fetch_url_with_cache_mode(&url, false, FetchCacheMode::Revalidate)
            .await
            .unwrap();
        assert!(second.body.contains("cached body"));
        assert_eq!(requests.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn blocks_private_network_urls_by_default() {
        let err = validate_public_http_url("http://127.0.0.1:8080/secret")
            .await
            .unwrap_err();
        assert!(matches!(err, SearchXyzError::UnsafeUrl { .. }));

        let err = validate_public_http_url("http://localhost:8080/secret")
            .await
            .unwrap_err();
        assert!(matches!(err, SearchXyzError::UnsafeUrl { .. }));
    }

    #[tokio::test]
    async fn private_network_can_be_explicitly_allowed() {
        validate_http_url("http://127.0.0.1:8080/test", true)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_crawler_client_pooling() {
        let crawler_config = CrawlerConfig::default();
        let headless_config = HeadlessConfig::default();
        let cache = Arc::new(Mutex::new(Cache::new(10, 60)));

        // Case 1: Proxy disabled
        let proxy_config = ProxyConfig {
            enabled: false,
            urls: vec!["http://127.0.0.1:8080".to_string()],
        };
        let crawler = Crawler::new(
            crawler_config.clone(),
            headless_config.clone(),
            proxy_config,
            cache.clone(),
        );
        assert_eq!(crawler.clients.len(), 1);

        // Case 2: Proxy enabled with valid urls
        let proxy_config = ProxyConfig {
            enabled: true,
            urls: vec![
                "http://127.0.0.1:8080".to_string(),
                "socks5://127.0.0.1:1080".to_string(),
            ],
        };
        let crawler = Crawler::new(crawler_config, headless_config, proxy_config, cache);
        assert_eq!(crawler.clients.len(), 2);
    }
}
