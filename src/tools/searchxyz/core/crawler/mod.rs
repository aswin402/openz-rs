pub mod fast_spider;
pub mod fingerprint;
pub mod github;
pub mod robots;
pub mod sitemap;
pub mod spider;
pub mod ssrf;
pub mod youtube;

use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;

use governor::{clock::DefaultClock, state::keyed::DefaultKeyedStateStore, Quota, RateLimiter};
use reqwest::{header, Client, StatusCode};
use url::Url;

use crate::tools::searchxyz::core::config::{CrawlerConfig, HeadlessConfig, ProxyConfig};
use crate::tools::searchxyz::core::error::SearchXyzError;
use crate::tools::shared_memory::web_cache::{
    is_cache_fresh, load_http_cache, refresh_http_cache_validators, save_http_cache,
    touch_http_cache, CachedHttpEntry,
};

pub use ssrf::{
    build_pinned_http_client, is_safe_ip, is_supported_web_content_type,
    read_bounded_response_bytes, ssrf_redirect_policy, validate_http_url,
    validate_http_url_addrs, validate_public_http_url, validate_url_sync_with_policy,
};

type DomainRateLimiter = RateLimiter<String, DefaultKeyedStateStore<String>, DefaultClock>;

#[derive(Clone)]
pub struct Crawler {
    clients: Vec<Client>,
    proxy_urls: Vec<String>,
    config: CrawlerConfig,
    headless_enabled: bool,
    rate_limiter: Arc<DomainRateLimiter>,
}

#[derive(Debug, Clone)]
pub struct FetchResult {
    pub url: String,
    pub final_url: String,
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
    pub fn may_read_cache(self) -> bool {
        matches!(self, FetchCacheMode::Auto | FetchCacheMode::PreferCache)
    }

    pub fn may_write_cache(self) -> bool {
        !matches!(self, FetchCacheMode::Bypass)
    }
}

fn build_pool_client(
    config: &CrawlerConfig,
    proxy: Option<reqwest::Proxy>,
) -> Result<Client, reqwest::Error> {
    let base = Client::builder()
        .use_rustls_tls()
        .timeout(Duration::from_secs(config.timeout_secs.max(1)))
        .connect_timeout(Duration::from_secs(10))
        .user_agent(&config.user_agent)
        .redirect(ssrf_redirect_policy(
            config.max_redirects,
            config.allow_private_network,
        ))
        .pool_max_idle_per_host(4)
        .gzip(true)
        .brotli(true);
    let builder = match proxy {
        Some(p) => base.proxy(p),
        None => base,
    };
    builder.build()
}

impl Crawler {
    pub fn new(
        config: CrawlerConfig,
        headless_config: HeadlessConfig,
        proxy_config: ProxyConfig,
    ) -> Self {
        let mut clients = Vec::new();
        let mut active_proxy_urls = Vec::new();
        match proxy_config.enabled && !proxy_config.urls.is_empty() {
            true => {
                for proxy_url in &proxy_config.urls {
                    match reqwest::Proxy::all(proxy_url) {
                        Ok(proxy) => match build_pool_client(&config, Some(proxy)) {
                            Ok(client) => {
                                clients.push(client);
                                active_proxy_urls.push(proxy_url.clone());
                            }
                            Err(e) => {
                                tracing::error!(proxy_url, error = %e, "Failed to build proxied HTTP client");
                            }
                        },
                        Err(e) => {
                            tracing::error!(proxy_url, error = %e, "Failed to parse proxy URL");
                        }
                    }
                }
            }
            false => {}
        }

        match clients.is_empty() {
            true => {
                let client = build_pool_client(&config, None)
                    .unwrap_or_else(|_| crate::core::http::default_http_client());
                clients.push(client);
            }
            false => {}
        }

        let quota = Quota::per_second(
            NonZeroU32::new(config.rate_limit_per_sec.max(1) as u32)
                .unwrap_or(NonZeroU32::new(2).unwrap()),
        );
        let rate_limiter = Arc::new(RateLimiter::keyed(quota));

        Self {
            clients,
            proxy_urls: active_proxy_urls,
            config,
            headless_enabled: headless_config.enabled,
            rate_limiter,
        }
    }

    pub fn clients(&self) -> &[Client] {
        &self.clients
    }

    pub fn config(&self) -> &CrawlerConfig {
        &self.config
    }

    pub fn headless_enabled(&self) -> bool {
        self.headless_enabled
    }

    pub async fn fetch_url(
        &self,
        url: &str,
        render_js: bool,
    ) -> Result<FetchResult, SearchXyzError> {
        self.fetch_url_with_cache_mode(url, render_js, FetchCacheMode::Auto)
            .await
    }

    pub async fn fetch_url_with_cache_mode(
        &self,
        url: &str,
        render_js: bool,
        cache_mode: FetchCacheMode,
    ) -> Result<FetchResult, SearchXyzError> {
        // 1. Check SQLite HTTP cache first (before DNS lookup so offline PreferCache / cached unit tests work cleanly)
        let cached_entry = match cache_mode {
            FetchCacheMode::Bypass => None,
            _ => load_http_cache(url).ok().flatten(),
        };

        match (&cached_entry, cache_mode) {
            (Some(entry), FetchCacheMode::PreferCache) => {
                validate_scheme_and_local_host_ok(url, self.config.allow_private_network)?;
                let _ = touch_http_cache(url);
                return Ok(cached_to_fetch_result(url, entry));
            }
            (Some(entry), FetchCacheMode::Auto) => {
                match is_cache_fresh(&entry.expires_at, chrono::Utc::now()) {
                    true => {
                        validate_scheme_and_local_host_ok(url, self.config.allow_private_network)?;
                        let _ = touch_http_cache(url);
                        return Ok(cached_to_fetch_result(url, entry));
                    }
                    false => {}
                }
            }
            _ => {}
        }

        let (_parsed_url, host_lc, resolved_addrs) =
            validate_http_url_addrs(url, self.config.allow_private_network).await?;

        let domain = Url::parse(url)
            .ok()
            .and_then(|u| u.host_str().map(ToString::to_string))
            .unwrap_or_else(|| "unknown".into());
        self.rate_limiter.until_key_ready(&domain).await;

        // 2. Brokered Headless JS execution when requested
        match render_js {
            true => {
                tracing::info!(url, "Fetching URL via OpenZ BrowserBroker");
                let rendered = crate::tools::browser_broker::render_page_with_browser_broker(
                    url,
                    self.config.timeout_secs.clamp(5, 60),
                )
                .await
                .map_err(|e| SearchXyzError::CrawlFailed {
                    url: url.to_string(),
                    reason: format!("BrowserBroker render failed: {e}"),
                })?;

                validate_http_url(&rendered.final_url, self.config.allow_private_network).await?;

                match cache_mode.may_write_cache() {
                    true => {
                        let empty_headers = header::HeaderMap::new();
                        let _ = save_http_cache(
                            url,
                            &rendered.final_url,
                            &rendered.html,
                            "text/html",
                            &empty_headers,
                            200,
                            self.config.default_cache_ttl_secs as i64,
                        );
                    }
                    false => {}
                }

                return Ok(FetchResult {
                    url: url.to_string(),
                    final_url: rendered.final_url,
                    body: rendered.html,
                    content_type: "text/html".into(),
                });
            }
            false => {}
        }

        // 3. Build DNS-pinned client for live HTTP fetch
        let chosen_proxy = self.pick_proxy();
        let client = build_pinned_http_client(&self.config, &host_lc, &resolved_addrs, chosen_proxy)
            .map_err(SearchXyzError::from)?;

        let allow_stale_fallback = !matches!(cache_mode, FetchCacheMode::Bypass);
        let send_conditional = matches!(
            cache_mode,
            FetchCacheMode::Auto | FetchCacheMode::Revalidate
        );
        let mut attempt = 0u32;

        loop {
            attempt += 1;
            let mut headers = fingerprint::HeaderGenerator::random_headers();
            match (send_conditional, cached_entry.as_ref()) {
                (true, Some(entry)) => {
                    match entry.etag.as_deref().and_then(|etag| etag.parse().ok()) {
                        Some(value) => {
                            headers.insert(header::IF_NONE_MATCH, value);
                        }
                        None => {}
                    }
                    match entry
                        .last_modified
                        .as_deref()
                        .and_then(|lm| lm.parse().ok())
                    {
                        Some(value) => {
                            headers.insert(header::IF_MODIFIED_SINCE, value);
                        }
                        None => {}
                    }
                }
                _ => {}
            }

            let resp = client.get(url).headers(headers).send().await;

            match resp {
                Ok(response) => {
                    let final_url = response.url().to_string();
                    validate_http_url(&final_url, self.config.allow_private_network).await?;
                    let status = response.status();

                    match status {
                        StatusCode::OK => {}

                        StatusCode::NOT_MODIFIED => match cached_entry {
                            Some(entry) => {
                                match cache_mode.may_write_cache() {
                                    true => {
                                        let _ = refresh_http_cache_validators(
                                            url,
                                            response.headers(),
                                            self.config.default_cache_ttl_secs as i64,
                                        );
                                    }
                                    false => {}
                                }
                                return Ok(FetchResult {
                                    url: url.to_string(),
                                    final_url,
                                    body: entry.body_text,
                                    content_type: entry.content_type,
                                });
                            }
                            None => {
                                return Err(SearchXyzError::HttpError {
                                    url: url.into(),
                                    status: 304,
                                    reason: "Origin returned 304 without cached body".into(),
                                });
                            }
                        },

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

                        StatusCode::TOO_MANY_REQUESTS => match attempt <= self.config.max_retries {
                            true => {
                                let delay = Duration::from_millis(1000 * 2u64.pow(attempt - 1));
                                tokio::time::sleep(delay).await;
                                continue;
                            }
                            false => {
                                return Err(SearchXyzError::RateLimited {
                                    provider: domain,
                                    retry_after_secs: 60,
                                });
                            }
                        },

                        StatusCode::INTERNAL_SERVER_ERROR
                        | StatusCode::BAD_GATEWAY
                        | StatusCode::SERVICE_UNAVAILABLE
                        | StatusCode::GATEWAY_TIMEOUT => match attempt <= self.config.max_retries {
                            true => {
                                let delay = Duration::from_millis(500 * 2u64.pow(attempt - 1));
                                tokio::time::sleep(delay).await;
                                continue;
                            }
                            false => {
                                let err = SearchXyzError::HttpError {
                                    url: url.into(),
                                    status: status.as_u16(),
                                    reason: format!(
                                        "Server error after {} attempts",
                                        self.config.max_retries
                                    ),
                                };
                                match allow_stale_fallback
                                    .then(|| stale_fallback(url, cached_entry.clone(), err.to_string()))
                                    .flatten()
                                {
                                    Some(result) => return Ok(result),
                                    None => return Err(err),
                                }
                            }
                        },

                        other => match other.is_success() {
                            true => {}
                            false => {
                                return Err(SearchXyzError::HttpError {
                                    url: url.into(),
                                    status: other.as_u16(),
                                    reason: format!("Unexpected status: {other}"),
                                });
                            }
                        },
                    }

                    let resp_headers = response.headers().clone();
                    let content_type = resp_headers
                        .get(header::CONTENT_TYPE)
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("")
                        .to_string();

                    match is_supported_web_content_type(&content_type) {
                        true => {}
                        false => {
                            return Err(SearchXyzError::CrawlFailed {
                                url: url.into(),
                                reason: format!(
                                    "Unsupported Content-Type: {content_type}. Supported types: HTML, text, Markdown, XML/RSS/Atom, JSON, and PDF."
                                ),
                            });
                        }
                    }

                    let bytes = read_bounded_response_bytes(
                        response,
                        url,
                        self.config.max_body_bytes,
                    )
                    .await?;

                    let is_pdf = content_type.to_ascii_lowercase().contains("application/pdf");
                    let body = match is_pdf {
                        true => pdf_extract::extract_text_from_mem(&bytes).map_err(|e| {
                            SearchXyzError::ExtractionFailed {
                                url: url.into(),
                                reason: format!("Failed to extract PDF text: {e}"),
                            }
                        })?,
                        false => String::from_utf8_lossy(&bytes).into_owned(),
                    };

                    match cache_mode.may_write_cache() {
                        true => {
                            let stored_ct = match (is_pdf, content_type.trim().is_empty()) {
                                (true, _) => "text/plain",
                                (false, true) => "text/html",
                                (false, false) => content_type.as_str(),
                            };
                            let _ = save_http_cache(
                                url,
                                &final_url,
                                &body,
                                stored_ct,
                                &resp_headers,
                                status.as_u16(),
                                self.config.default_cache_ttl_secs as i64,
                            );
                        }
                        false => {}
                    }

                    let returned_ct = match is_pdf {
                        true => "text/plain".to_string(),
                        false => content_type,
                    };

                    return Ok(FetchResult {
                        url: url.into(),
                        final_url,
                        body,
                        content_type: returned_ct,
                    });
                }

                Err(e) => {
                    match attempt <= self.config.max_retries && (e.is_timeout() || e.is_connect()) {
                        true => {
                            let delay = Duration::from_millis(500 * 2u64.pow(attempt - 1));
                            tokio::time::sleep(delay).await;
                            continue;
                        }
                        false => {
                            let err = SearchXyzError::from(e);
                            match allow_stale_fallback
                                .then(|| stale_fallback(url, cached_entry.clone(), err.to_string()))
                                .flatten()
                            {
                                Some(result) => return Ok(result),
                                None => return Err(err),
                            }
                        }
                    }
                }
            }
        }
    }

    fn pick_proxy(&self) -> Option<reqwest::Proxy> {
        use rand::seq::SliceRandom;
        self.proxy_urls
            .choose(&mut rand::thread_rng())
            .and_then(|url| reqwest::Proxy::all(url).ok())
    }
}

fn validate_scheme_and_local_host_ok(
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
    let host = parsed.host_str().unwrap_or("").to_ascii_lowercase();
    let is_local_literal = host == "localhost"
        || host.ends_with(".localhost")
        || host
            .parse::<std::net::IpAddr>()
            .ok()
            .is_some_and(|ip| !is_safe_ip(&ip));
    match !allow_private_network && is_local_literal {
        true => Err(SearchXyzError::UnsafeUrl {
            url: url.to_string(),
            reason: "private or localhost target is not allowed".to_string(),
        }),
        false => Ok(()),
    }
}

fn cached_to_fetch_result(url: &str, entry: &CachedHttpEntry) -> FetchResult {
    FetchResult {
        url: url.to_string(),
        final_url: entry.final_url.clone(),
        body: entry.body_text.clone(),
        content_type: entry.content_type.clone(),
    }
}

fn stale_fallback(
    url: &str,
    cached_entry: Option<CachedHttpEntry>,
    reason: String,
) -> Option<FetchResult> {
    let entry = cached_entry?;
    tracing::warn!(url, reason = %reason, "Live fetch failed; returning stale cached body");
    Some(cached_to_fetch_result(url, &entry))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn revalidate_falls_back_to_stale_cache_on_fetch_failure() {
        let test_url = format!("http://127.0.0.1:9/stale-{}", uuid::Uuid::new_v4());
        let mut headers = header::HeaderMap::new();
        headers.insert(header::ETAG, header::HeaderValue::from_static("\"old\""));
        headers.insert(
            header::CACHE_CONTROL,
            header::HeaderValue::from_static("max-age=0"),
        );
        save_http_cache(
            &test_url,
            &test_url,
            "<html><body>stale fallback</body></html>",
            "text/html",
            &headers,
            200,
            0,
        )
        .unwrap();

        let mut crawler_config = CrawlerConfig::default();
        crawler_config.allow_private_network = true;
        crawler_config.max_retries = 0;
        let crawler = Crawler::new(
            crawler_config,
            HeadlessConfig::default(),
            ProxyConfig::default(),
        );

        let result = crawler
            .fetch_url_with_cache_mode(&test_url, false, FetchCacheMode::Revalidate)
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

                match count == 0 {
                    true => {
                        let body = "<html><body>cached body</body></html>";
                        let response = format!(
                            "HTTP/1.1 200 OK\r\ncontent-type: text/html\r\netag: \"v1\"\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                            body.len(),
                            body
                        );
                        stream.write_all(response.as_bytes()).await.unwrap();
                    }
                    false => {
                        assert!(
                            request.to_ascii_lowercase().contains("if-none-match: \"v1\""),
                            "revalidation request should send If-None-Match, got: {request}"
                        );
                        let response = "HTTP/1.1 304 Not Modified\r\nconnection: close\r\n\r\n";
                        stream.write_all(response.as_bytes()).await.unwrap();
                    }
                }
            }
        });

        let mut crawler_config = CrawlerConfig::default();
        crawler_config.allow_private_network = true;
        let crawler = Crawler::new(
            crawler_config,
            HeadlessConfig::default(),
            ProxyConfig::default(),
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

        let proxy_config = ProxyConfig {
            enabled: false,
            urls: vec!["http://127.0.0.1:8080".to_string()],
        };
        let crawler = Crawler::new(crawler_config.clone(), headless_config.clone(), proxy_config);
        assert_eq!(crawler.clients().len(), 1);

        let proxy_config_enabled = ProxyConfig {
            enabled: true,
            urls: vec![
                "http://127.0.0.1:8080".to_string(),
                "socks5://127.0.0.1:1080".to_string(),
            ],
        };
        let crawler2 = Crawler::new(crawler_config, headless_config, proxy_config_enabled);
        assert_eq!(crawler2.clients().len(), 2);
    }
}
