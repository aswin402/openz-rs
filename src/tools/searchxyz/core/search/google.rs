use async_trait::async_trait;
use reqwest::Client;
use scraper::{Html, Selector};

use super::{SearchBackend, SearchQuery, SearchResult};
use crate::tools::searchxyz::core::crawler::fingerprint::HeaderGenerator;
use crate::tools::searchxyz::core::crawler::headless::HeadlessBrowser;
use crate::tools::searchxyz::core::error::SearchXyzError;

/// Native Google Scraper Backend — no API key required.
pub struct GoogleBackend {
    client: Client,
    clients: Option<Vec<Client>>,
    headless_browser: Option<HeadlessBrowser>,
}

impl GoogleBackend {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            clients: None,
            headless_browser: None,
        }
    }

    pub fn with_proxies(mut self, clients: Vec<Client>) -> Self {
        self.clients = Some(clients);
        self
    }

    pub fn with_headless(mut self, headless_browser: HeadlessBrowser) -> Self {
        self.headless_browser = Some(headless_browser);
        self
    }

    fn parse_google_href(href: &str) -> Option<String> {
        match (href.starts_with("http"), href.starts_with("/url?q=")) {
            (true, _) => Some(href.to_string()),
            (false, true) => {
                let parsed = url::Url::parse(&format!("https://www.google.com{}", href)).ok();
                let target = parsed.and_then(|u| {
                    u.query_pairs()
                        .find_map(|(k, v)| (k == "q").then(|| v.into_owned()))
                });
                Some(target.unwrap_or_else(|| href.to_string()))
            }
            (false, false) => None,
        }
    }

    fn parse_results(html_body: &str, max_results: usize) -> Vec<SearchResult> {
        let document = Html::parse_document(html_body);

        let container_sel =
            Selector::parse("div.g, div.MjjYud, div[jscontroller][data-hveid][data-ved]").unwrap();
        let containers: Vec<_> = document.select(&container_sel).collect();
        let title_sel = Selector::parse("h3").unwrap();

        match containers.is_empty() {
            false => {
                let link_sel = Selector::parse("a[href]").unwrap();
                let snippet_selectors = vec![
                    Selector::parse(".VwiC3b").unwrap(),
                    Selector::parse(".yDAB2e").unwrap(),
                    Selector::parse(".BNeawe").unwrap(),
                    Selector::parse("div[style*='-webkit-line-clamp']").unwrap(),
                ];

                containers
                    .into_iter()
                    .filter_map(|container| {
                        let title_el = container.select(&title_sel).next()?;
                        let title = title_el
                            .text()
                            .collect::<Vec<_>>()
                            .join("")
                            .trim()
                            .to_string();
                        match title.is_empty() {
                            true => return None,
                            false => {}
                        }

                        let href = container
                            .select(&link_sel)
                            .next()
                            .and_then(|el| el.value().attr("href"))?;
                        let url = Self::parse_google_href(href)?;
                        let snippet = super::extract_container_snippet(
                            &container,
                            &snippet_selectors,
                            &title,
                        );

                        Some(SearchResult {
                            title,
                            url,
                            snippet,
                            source: "google".into(),
                        })
                    })
                    .take(max_results)
                    .collect()
            }
            true => document
                .select(&title_sel)
                .filter_map(|h3_el| {
                    let title = h3_el.text().collect::<Vec<_>>().join("").trim().to_string();
                    match title.is_empty() {
                        true => None,
                        false => {
                            let url = super::find_ancestor_http_href(&h3_el)?;
                            Some(SearchResult {
                                title,
                                url,
                                snippet: String::new(),
                                source: "google".into(),
                            })
                        }
                    }
                })
                .take(max_results)
                .collect(),
        }
    }
}

#[async_trait]
impl SearchBackend for GoogleBackend {
    fn name(&self) -> &str {
        "google"
    }

    fn is_available(&self) -> bool {
        true // no key needed
    }

    async fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>, SearchXyzError> {
        let client = super::choose_proxy_client(&self.client, &self.clients);

        let mut results = Vec::new();
        #[cfg(feature = "js-rendering")]
        let mut http_failed = false;

        match client
            .get("https://www.google.com/search")
            .query(&[("q", &query.query)])
            .headers(HeaderGenerator::random_headers())
            .send()
            .await
        {
            Ok(resp) => match resp.status().is_success() {
                true => match resp.text().await {
                    Ok(html_body) => {
                        results = Self::parse_results(&html_body, query.max_results);
                    }
                    Err(_) => {
                        #[cfg(feature = "js-rendering")]
                        {
                            http_failed = true;
                        }
                    }
                },
                false => {
                    #[cfg(feature = "js-rendering")]
                    {
                        http_failed = true;
                    }
                    tracing::warn!("Google HTTP request returned status {}", resp.status());
                }
            },
            Err(e) => {
                #[cfg(feature = "js-rendering")]
                {
                    http_failed = true;
                }
                tracing::warn!("Google HTTP request failed: {:?}", e);
            }
        }

        // Fallback to headless browser if raw HTTP failed or returned no organic results (bot detection / captcha page)
        #[cfg(feature = "js-rendering")]
        match (http_failed || results.is_empty(), &self.headless_browser) {
            (true, Some(headless)) => {
                let mut url_parsed = url::Url::parse("https://www.google.com/search").unwrap();
                url_parsed.query_pairs_mut().append_pair("q", &query.query);
                let search_url = url_parsed.to_string();

                tracing::info!(url = %search_url, "Google raw HTTP blocked or empty, falling back to headless browser");
                match headless.fetch_html(&search_url).await {
                    Ok(html_body) => {
                        results = Self::parse_results(&html_body, query.max_results);
                    }
                    Err(e) => {
                        tracing::error!("Google headless fallback failed: {:?}", e);
                    }
                }
            }
            _ => {}
        }

        match results.is_empty() {
            true => tracing::warn!(query = %query.query, "Google returned no parsable results"),
            false => {}
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_google_parsing() {
        let html = r#"
            <html>
            <body>
                <div class="g">
                    <h3><a href="https://rust-lang.org">Rust Programming Language</a></h3>
                    <div class="VwiC3b">Safe, fast, productive language.</div>
                </div>
                <div class="MjjYud">
                    <h3><a href="https://github.com/rust-lang/rust">Rust Github Repo</a></h3>
                    <div class="yDAB2e">Source code repository for Rust.</div>
                </div>
            </body>
            </html>
        "#;
        let results = GoogleBackend::parse_results(html, 10);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "Rust Programming Language");
        assert_eq!(results[0].url, "https://rust-lang.org");
        assert_eq!(results[0].snippet, "Safe, fast, productive language.");
        assert_eq!(results[0].source, "google");

        assert_eq!(results[1].title, "Rust Github Repo");
        assert_eq!(results[1].url, "https://github.com/rust-lang/rust");
        assert_eq!(results[1].snippet, "Source code repository for Rust.");
    }
}
