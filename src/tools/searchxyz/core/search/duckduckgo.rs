use async_trait::async_trait;
use reqwest::Client;
use scraper::{Html, Selector};

use super::{SearchBackend, SearchQuery, SearchResult};
use crate::tools::searchxyz::core::crawler::headless::HeadlessBrowser;
use crate::tools::searchxyz::core::error::SearchXyzError;

/// DuckDuckGo Lite — scrapes the lightweight HTML interface.
/// No API key required. This is the default/fallback backend.
pub struct DuckDuckGoBackend {
    client: Client,
    clients: Option<Vec<Client>>,
    headless_browser: Option<HeadlessBrowser>,
}

impl DuckDuckGoBackend {
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

    fn parse_results(html_body: &str, max_results: usize) -> Vec<SearchResult> {
        let document = Html::parse_document(html_body);

        let link_sel = Selector::parse("a.result-link")
            .unwrap_or_else(|_| Selector::parse("table tr td a[href]").unwrap());
        let snippet_sel = Selector::parse("td.result-snippet")
            .unwrap_or_else(|_| Selector::parse("table tr.result-snippet td").unwrap());

        let links: Vec<_> = document.select(&link_sel).collect();
        let snippets: Vec<_> = document.select(&snippet_sel).collect();

        links
            .iter()
            .enumerate()
            .filter_map(|(i, link_el)| {
                let url = link_el
                    .value()
                    .attr("href")
                    .filter(|u| u.starts_with("http"))?
                    .to_string();
                let title = link_el
                    .text()
                    .collect::<Vec<_>>()
                    .join("")
                    .trim()
                    .to_string();
                match title.is_empty() {
                    true => None,
                    false => {
                        let snippet = snippets
                            .get(i)
                            .map(|el| el.text().collect::<Vec<_>>().join("").trim().to_string())
                            .unwrap_or_default();
                        Some(SearchResult {
                            title,
                            url,
                            snippet,
                            source: "duckduckgo".into(),
                        })
                    }
                }
            })
            .take(max_results)
            .collect()
    }
}

#[async_trait]
impl SearchBackend for DuckDuckGoBackend {
    fn name(&self) -> &str {
        "duckduckgo"
    }

    fn is_available(&self) -> bool {
        true // no key needed
    }

    async fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>, SearchXyzError> {
        let client = super::choose_proxy_client(&self.client, &self.clients);

        let mut results = Vec::new();
        #[cfg(feature = "js-rendering")]
        let mut http_failed = false;

        // Try raw HTTP form POST first
        match client
            .post("https://lite.duckduckgo.com/lite/")
            .form(&[("q", &query.query)])
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
                    tracing::warn!("DuckDuckGo HTTP request returned status {}", resp.status());
                }
            },
            Err(e) => {
                #[cfg(feature = "js-rendering")]
                {
                    http_failed = true;
                }
                tracing::warn!("DuckDuckGo HTTP request failed: {:?}", e);
            }
        }

        // Fallback to headless browser if HTTP failed or returned no results (bot detection / captcha page)
        #[cfg(feature = "js-rendering")]
        match (http_failed || results.is_empty(), &self.headless_browser) {
            (true, Some(headless)) => {
                let mut url_parsed = url::Url::parse("https://lite.duckduckgo.com/lite/").unwrap();
                url_parsed.query_pairs_mut().append_pair("q", &query.query);
                let search_url = url_parsed.to_string();

                tracing::info!(url = %search_url, "DuckDuckGo raw HTTP blocked or empty, falling back to headless browser");
                match headless.fetch_html(&search_url).await {
                    Ok(html_body) => {
                        results = Self::parse_results(&html_body, query.max_results);
                    }
                    Err(e) => {
                        tracing::error!("DuckDuckGo headless fallback failed: {:?}", e);
                    }
                }
            }
            _ => {}
        }

        match results.is_empty() {
            true => tracing::warn!(query = %query.query, "DuckDuckGo returned no parsable results"),
            false => {}
        }

        Ok(results)
    }
}
