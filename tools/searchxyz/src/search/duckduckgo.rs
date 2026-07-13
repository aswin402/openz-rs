use async_trait::async_trait;
use reqwest::Client;
use scraper::{Html, Selector};

use super::{SearchBackend, SearchQuery, SearchResult};
use crate::crawler::headless::HeadlessBrowser;
use crate::error::SearchXyzError;

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

        let mut results = Vec::new();

        for (i, link_el) in links.iter().enumerate() {
            if results.len() >= max_results {
                break;
            }

            let url = match link_el.value().attr("href") {
                Some(u) if u.starts_with("http") => u.to_string(),
                _ => continue,
            };

            let title = link_el
                .text()
                .collect::<Vec<_>>()
                .join("")
                .trim()
                .to_string();

            let snippet = snippets
                .get(i)
                .map(|el| el.text().collect::<Vec<_>>().join("").trim().to_string())
                .unwrap_or_default();

            if title.is_empty() {
                continue;
            }

            results.push(SearchResult {
                title,
                url,
                snippet,
                source: "duckduckgo".into(),
            });
        }

        results
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
        use rand::seq::IndexedRandom;

        let client = if let Some(ref clients) = self.clients {
            if !clients.is_empty() {
                clients.choose(&mut rand::rng()).unwrap_or(&self.client)
            } else {
                &self.client
            }
        } else {
            &self.client
        };

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
            Ok(resp) => {
                if resp.status().is_success() {
                    match resp.text().await {
                        Ok(html_body) => {
                            results = Self::parse_results(&html_body, query.max_results);
                        }
                        Err(_) => {
                            #[cfg(feature = "js-rendering")]
                            {
                                http_failed = true;
                            }
                        }
                    }
                } else {
                    #[cfg(feature = "js-rendering")]
                    {
                        http_failed = true;
                    }
                    tracing::warn!("DuckDuckGo HTTP request returned status {}", resp.status());
                }
            }
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
        if (http_failed || results.is_empty()) && self.headless_browser.is_some() {
            if let Some(ref headless) = self.headless_browser {
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
        }

        if results.is_empty() {
            tracing::warn!(query = %query.query, "DuckDuckGo returned no parsable results");
        }

        Ok(results)
    }
}
