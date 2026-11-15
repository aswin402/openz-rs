use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::time::Duration;

use futures_util::StreamExt;
use reqwest::{redirect::Policy, Client};
use tokio::net::lookup_host;
use url::Url;

use crate::tools::searchxyz::core::config::CrawlerConfig;
use crate::tools::searchxyz::core::error::SearchXyzError;

const BLOCKED_METADATA_HOSTS: &[&str] = &[
    "localhost",
    "localhost.localdomain",
    "169.254.169.254",
    "metadata.google.internal",
    "metadata.goog",
];

pub fn is_safe_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let octets = v4.octets();
            let is_cgnat = octets[0] == 100 && (64..=127).contains(&octets[1]);
            let is_benchmark = octets[0] == 198 && matches!(octets[1], 18 | 19);
            !(v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_multicast()
                || is_cgnat
                || is_benchmark)
        }
        IpAddr::V6(v6) => {
            let mapped_v4_ok = v6
                .to_ipv4()
                .map(|v4| is_safe_ip(&IpAddr::V4(v4)))
                .unwrap_or(true);
            let segments = v6.segments();
            let is_ula = (segments[0] & 0xfe00) == 0xfc00;
            let is_link_local = (segments[0] & 0xffc0) == 0xfe80;
            mapped_v4_ok
                && !v6.is_loopback()
                && !v6.is_unspecified()
                && !v6.is_multicast()
                && !v6.is_unique_local()
                && !v6.is_unicast_link_local()
                && !is_ula
                && !is_link_local
        }
    }
}

fn check_parsed_url_host(
    parsed: &Url,
    raw_url: &str,
    allow_private_network: bool,
) -> Result<(String, u16), SearchXyzError> {
    match parsed.scheme() {
        "http" | "https" => {}
        scheme => {
            return Err(SearchXyzError::UnsafeUrl {
                url: raw_url.to_string(),
                reason: format!(
                    "SSRF blocked: only http/https URLs are allowed (got `{scheme}`)"
                ),
            });
        }
    }

    let host = parsed.host_str().ok_or_else(|| SearchXyzError::UnsafeUrl {
        url: raw_url.to_string(),
        reason: "URL has no host".to_string(),
    })?;
    let host_lc = host.trim_matches(['[', ']']).to_ascii_lowercase();
    let is_blocked_host = BLOCKED_METADATA_HOSTS.contains(&host_lc.as_str())
        || host_lc.ends_with(".localhost")
        || host_lc.ends_with(".internal");

    match !allow_private_network && is_blocked_host {
        true => Err(SearchXyzError::UnsafeUrl {
            url: raw_url.to_string(),
            reason: format!("SSRF blocked: restricted host `{host_lc}` is not allowed"),
        }),
        false => {
            let port = parsed.port_or_known_default().unwrap_or(443);
            Ok((host_lc, port))
        }
    }
}

fn verify_ip_safety(
    raw_url: &str,
    ip: IpAddr,
    allow_private_network: bool,
) -> Result<(), SearchXyzError> {
    match allow_private_network || is_safe_ip(&ip) {
        true => Ok(()),
        false => Err(SearchXyzError::UnsafeUrl {
            url: raw_url.to_string(),
            reason: format!("SSRF blocked: host resolves to private/reserved address {ip}"),
        }),
    }
}

pub fn validate_url_without_dns(
    url: &str,
    allow_private_network: bool,
) -> Result<(), SearchXyzError> {
    let parsed = Url::parse(url).map_err(|e| SearchXyzError::UnsafeUrl {
        url: url.to_string(),
        reason: format!("invalid URL: {e}"),
    })?;
    let (host_lc, _port) = check_parsed_url_host(&parsed, url, allow_private_network)?;
    match host_lc.parse::<IpAddr>() {
        Ok(ip) => verify_ip_safety(url, ip, allow_private_network),
        Err(_) => Ok(()),
    }
}

pub fn validate_url_sync_with_policy(
    parsed: &Url,
    allow_private_network: bool,
) -> Result<Vec<SocketAddr>, SearchXyzError> {
    let raw_url = parsed.as_str();
    let (host_lc, port) = check_parsed_url_host(parsed, raw_url, allow_private_network)?;

    match host_lc.parse::<IpAddr>() {
        Ok(ip) => {
            verify_ip_safety(raw_url, ip, allow_private_network)?;
            Ok(vec![SocketAddr::new(ip, port)])
        }
        Err(_) => {
            let resolved: Vec<SocketAddr> = (host_lc.as_str(), port)
                .to_socket_addrs()
                .map(|iter| iter.collect())
                .unwrap_or_default();
            match resolved.is_empty() {
                true => Err(SearchXyzError::UnsafeUrl {
                    url: raw_url.to_string(),
                    reason: format!(
                        "SSRF blocked: hostname `{host_lc}` could not be resolved to any IP"
                    ),
                }),
                false => {
                    for addr in &resolved {
                        verify_ip_safety(raw_url, addr.ip(), allow_private_network)?;
                    }
                    Ok(resolved)
                }
            }
        }
    }
}

pub async fn validate_http_url_addrs(
    url: &str,
    allow_private_network: bool,
) -> Result<(Url, String, Vec<SocketAddr>), SearchXyzError> {
    let parsed = Url::parse(url).map_err(|e| SearchXyzError::UnsafeUrl {
        url: url.to_string(),
        reason: format!("invalid URL: {e}"),
    })?;
    let (host_lc, port) = check_parsed_url_host(&parsed, url, allow_private_network)?;

    match host_lc.parse::<IpAddr>() {
        Ok(ip) => {
            verify_ip_safety(url, ip, allow_private_network)?;
            Ok((parsed, host_lc, vec![SocketAddr::new(ip, port)]))
        }
        Err(_) => {
            let resolved: Vec<SocketAddr> = lookup_host((host_lc.as_str(), port))
                .await
                .map_err(|e| SearchXyzError::UnsafeUrl {
                    url: url.to_string(),
                    reason: format!("DNS lookup failed for `{host_lc}`: {e}"),
                })?
                .collect();
            match resolved.is_empty() {
                true => Err(SearchXyzError::UnsafeUrl {
                    url: url.to_string(),
                    reason: format!(
                        "SSRF blocked: hostname `{host_lc}` could not be resolved to any IP"
                    ),
                }),
                false => {
                    for addr in &resolved {
                        verify_ip_safety(url, addr.ip(), allow_private_network)?;
                    }
                    Ok((parsed, host_lc, resolved))
                }
            }
        }
    }
}

pub async fn validate_http_url(
    url: &str,
    allow_private_network: bool,
) -> Result<(), SearchXyzError> {
    validate_http_url_addrs(url, allow_private_network)
        .await
        .map(|_| ())
}

pub async fn validate_public_http_url(url: &str) -> Result<(), SearchXyzError> {
    validate_http_url(url, false).await
}

pub fn ssrf_redirect_policy(max_redirects: usize, allow_private_network: bool) -> Policy {
    Policy::custom(move |attempt| {
        match attempt.previous().len() >= max_redirects {
            true => return attempt.error("too many redirects"),
            false => {}
        }
        match validate_url_sync_with_policy(attempt.url(), allow_private_network) {
            Ok(_) => attempt.follow(),
            Err(err) => attempt.error(err),
        }
    })
}

pub fn build_pinned_http_client(
    config: &CrawlerConfig,
    host: &str,
    addrs: &[SocketAddr],
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
        .gzip(true)
        .brotli(true);

    let with_dns = match host.parse::<IpAddr>().is_ok() || addrs.is_empty() {
        true => base,
        false => base.resolve_to_addrs(host, addrs),
    };

    let builder = match proxy {
        Some(p) => with_dns.proxy(p),
        None => with_dns,
    };
    builder.build()
}

pub fn is_supported_web_content_type(content_type: &str) -> bool {
    let mime = content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    mime.is_empty()
        || mime.starts_with("text/")
        || mime == "application/pdf"
        || mime == "application/xhtml+xml"
        || mime == "application/xml"
        || mime == "application/rss+xml"
        || mime == "application/atom+xml"
        || mime == "application/json"
        || mime == "application/ld+json"
        || mime == "application/csv"
        || mime.ends_with("+xml")
        || mime.ends_with("+json")
}

pub async fn read_bounded_response_bytes(
    response: reqwest::Response,
    url: &str,
    max_bytes: usize,
) -> Result<Vec<u8>, SearchXyzError> {
    match response.content_length().filter(|&len| len as usize > max_bytes) {
        Some(len) => {
            return Err(SearchXyzError::CrawlFailed {
                url: url.to_string(),
                reason: format!("Response too large ({len} bytes, max {max_bytes})"),
            });
        }
        None => {}
    }

    let initial_cap = response
        .content_length()
        .map(|len| (len as usize).min(max_bytes))
        .unwrap_or(16 * 1024);
    let mut out = Vec::with_capacity(initial_cap);
    let mut stream = response.bytes_stream();

    while let Some(chunk_res) = stream.next().await {
        let chunk = chunk_res.map_err(|e| SearchXyzError::CrawlFailed {
            url: url.to_string(),
            reason: format!("Failed to read response stream: {e}"),
        })?;
        match out.len().saturating_add(chunk.len()) > max_bytes {
            true => {
                return Err(SearchXyzError::CrawlFailed {
                    url: url.to_string(),
                    reason: format!(
                        "Response stream exceeded max size limit ({max_bytes} bytes)"
                    ),
                });
            }
            false => out.extend_from_slice(&chunk),
        }
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_ipv4_mapped_ipv6_loopback_and_metadata() {
        let mapped_loopback: IpAddr = "::ffff:127.0.0.1".parse().unwrap();
        let mapped_metadata: IpAddr = "::ffff:169.254.169.254".parse().unwrap();
        let public_v4: IpAddr = "93.184.216.34".parse().unwrap();
        assert!(!is_safe_ip(&mapped_loopback));
        assert!(!is_safe_ip(&mapped_metadata));
        assert!(is_safe_ip(&public_v4));
    }

    #[test]
    fn allows_xml_rss_json_html_pdf_and_blocks_binaries() {
        assert!(is_supported_web_content_type("text/html; charset=utf-8"));
        assert!(is_supported_web_content_type("application/xml"));
        assert!(is_supported_web_content_type("text/xml"));
        assert!(is_supported_web_content_type("application/rss+xml"));
        assert!(is_supported_web_content_type("application/atom+xml"));
        assert!(is_supported_web_content_type("application/json"));
        assert!(is_supported_web_content_type("application/pdf"));
        assert!(is_supported_web_content_type(""));
        assert!(!is_supported_web_content_type("image/png"));
        assert!(!is_supported_web_content_type("video/mp4"));
        assert!(!is_supported_web_content_type("application/octet-stream"));
    }
}
