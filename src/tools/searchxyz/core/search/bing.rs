use async_trait::async_trait;
use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD};
use base64::Engine;
use reqwest::Client;

use super::{SearchBackend, SearchQuery, SearchResult};
use crate::tools::searchxyz::core::crawler::fingerprint::HeaderGenerator;
use crate::tools::searchxyz::core::error::SearchXyzError;

/// Unwraps Bing click-tracking redirect URLs (e.g. `https://www.bing.com/ck/a?!...&u=a1<base64>&...`)
/// into direct canonical destination URLs.
pub fn unwrap_bing_redirect(url_str: &str) -> String {
    match url_str.contains("bing.com/ck/a") {
        false => return url_str.to_string(),
        true => {}
    }

    let parsed = match url::Url::parse(url_str) {
        Ok(p) => p,
        Err(_) => return url_str.to_string(),
    };

    for (k, v) in parsed.query_pairs() {
        match k == "u" {
            false => continue,
            true => {
                let payload = match v.len() > 2 && (v.starts_with("a1") || v.starts_with("a0")) {
                    true => &v[2..],
                    false => &v[..],
                };

                let try_decoders = [
                    STANDARD.decode(payload),
                    URL_SAFE.decode(payload),
                    STANDARD_NO_PAD.decode(payload.trim_end_matches('=')),
                    URL_SAFE_NO_PAD.decode(payload.trim_end_matches('=')),
                ];

                let decoded_url = try_decoders
                    .into_iter()
                    .flatten()
                    .filter_map(|bytes| String::from_utf8(bytes).ok())
                    .find(|decoded| decoded.starts_with("http://") || decoded.starts_with("https://"));

                match decoded_url {
                    Some(decoded) => return decoded,
                    None => {}
                }
            }
        }
    }

    url_str.to_string()
}

/// Native Bing Scraper Backend — no API key required.
pub struct BingBackend {
    client: Client,
}

impl BingBackend {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    fn parse_results(html_body: &str, max_results: usize) -> Vec<SearchResult> {
        super::parse_container_or_heading_results(
            html_body,
            max_results,
            "bing",
            "li.b_algo",
            "h2",
            &[".b_caption p", ".b_caption", ".b_snippet", "p"],
            |container, title_el, link_sel| {
                let raw_url = title_el
                    .select(link_sel)
                    .next()
                    .or_else(|| container.select(link_sel).next())
                    .and_then(|el| el.value().attr("href"))
                    .filter(|href| href.starts_with("http"))?;
                Some(unwrap_bing_redirect(raw_url))
            },
            |url| unwrap_bing_redirect(&url),
        )
    }
}

#[async_trait]
impl SearchBackend for BingBackend {
    fn name(&self) -> &str {
        "bing"
    }

    fn is_available(&self) -> bool {
        true // no key needed
    }

    async fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>, SearchXyzError> {
        let resp = self
            .client
            .get("https://www.bing.com/search")
            .query(&[("q", &query.query)])
            .headers(HeaderGenerator::random_headers())
            .send()
            .await
            .map_err(|e| SearchXyzError::SearchFailed {
                query: query.query.clone(),
                reason: format!("Bing HTTP request failed: {e}"),
            })?;

        match resp.status().is_success() {
            true => {}
            false => {
                return Err(SearchXyzError::SearchFailed {
                    query: query.query.clone(),
                    reason: format!("Bing returned HTTP {}", resp.status()),
                });
            }
        }

        let html_body = resp
            .text()
            .await
            .map_err(|e| SearchXyzError::SearchFailed {
                query: query.query.clone(),
                reason: format!("Failed to read Bing response body: {e}"),
            })?;

        let results = Self::parse_results(&html_body, query.max_results);

        match results.is_empty() {
            true => tracing::warn!(query = %query.query, "Bing returned no parsable results"),
            false => {}
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bing_parsing() {
        let html = r#"
            <html>
            <body>
                <ol id="b_results">
                    <li class="b_algo">
                        <h2><a href="https://rust-lang.org">Rust Programming Language</a></h2>
                        <div class="b_caption">
                            <p>Safe, fast, productive language.</p>
                        </div>
                    </li>
                    <li class="b_algo">
                        <h2><a href="https://github.com/rust-lang/rust">Rust Github Repo</a></h2>
                        <div class="b_snippet">Source code repository for Rust.</div>
                    </li>
                </ol>
            </body>
            </html>
        "#;
        let results = BingBackend::parse_results(html, 10);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "Rust Programming Language");
        assert_eq!(results[0].url, "https://rust-lang.org");
        assert_eq!(results[0].snippet, "Safe, fast, productive language.");
        assert_eq!(results[0].source, "bing");

        assert_eq!(results[1].title, "Rust Github Repo");
        assert_eq!(results[1].url, "https://github.com/rust-lang/rust");
        assert_eq!(results[1].snippet, "Source code repository for Rust.");
    }

    #[test]
    fn test_unwrap_bing_redirect() {
        let tracking = "https://www.bing.com/ck/a?!&&p=84c478a0c242337a50352ff90ee693b4823293dc2c2cfa4192b45e7f1396b7adJmltdHM9MTczMjU3OTIwMA&ptn=3&ver=2&hsh=4&fclid=123&u=a1aHR0cHM6Ly9lbi53aWtpcGVkaWEub3JnL3dpa2kvUnVzdF8ocHJvZ3JhbW1pbmdfbGFuZ3VhZ2Up&ntb=1";
        let clean = unwrap_bing_redirect(tracking);
        assert_eq!(
            clean,
            "https://en.wikipedia.org/wiki/Rust_(programming_language)"
        );

        let regular = "https://rust-lang.org/learn";
        assert_eq!(unwrap_bing_redirect(regular), regular);
    }
}
