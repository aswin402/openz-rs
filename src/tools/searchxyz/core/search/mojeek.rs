use async_trait::async_trait;
use reqwest::Client;

use super::{SearchBackend, SearchQuery, SearchResult};
use crate::tools::searchxyz::core::crawler::fingerprint::HeaderGenerator;
use crate::tools::searchxyz::core::error::SearchXyzError;

/// Native Mojeek Scraper Backend — independent keyless search index.
pub struct MojeekBackend {
    client: Client,
    clients: Option<Vec<Client>>,
}

impl MojeekBackend {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            clients: None,
        }
    }

    pub fn with_proxies(mut self, clients: Vec<Client>) -> Self {
        self.clients = Some(clients);
        self
    }

    fn parse_results(html_body: &str, max_results: usize) -> Vec<SearchResult> {
        super::parse_container_or_heading_results(
            html_body,
            max_results,
            "mojeek",
            "ul.results-standard li",
            "a.title, h2",
            &["p.s", "p"],
            |container, title_el, link_sel| {
                title_el
                    .value()
                    .attr("href")
                    .or_else(|| {
                        title_el
                            .select(link_sel)
                            .next()
                            .and_then(|el| el.value().attr("href"))
                    })
                    .or_else(|| {
                        container
                            .select(link_sel)
                            .next()
                            .and_then(|el| el.value().attr("href"))
                    })
                    .filter(|href| href.starts_with("http://") || href.starts_with("https://"))
                    .map(str::to_string)
            },
            |url| url,
        )
    }
}

#[async_trait]
impl SearchBackend for MojeekBackend {
    fn name(&self) -> &str {
        "mojeek"
    }

    fn is_available(&self) -> bool {
        true
    }

    async fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>, SearchXyzError> {
        let client = super::choose_proxy_client(&self.client, &self.clients);
        let resp = client
            .get("https://www.mojeek.com/search")
            .query(&[("q", &query.query)])
            .headers(HeaderGenerator::random_headers())
            .send()
            .await
            .map_err(|e| SearchXyzError::SearchFailed {
                query: query.query.clone(),
                reason: format!("Mojeek HTTP request failed: {e}"),
            })?;

        match resp.status().is_success() {
            true => {}
            false => {
                return Err(SearchXyzError::SearchFailed {
                    query: query.query.clone(),
                    reason: format!("Mojeek returned HTTP {}", resp.status()),
                });
            }
        }

        let html_body = resp
            .text()
            .await
            .map_err(|e| SearchXyzError::SearchFailed {
                query: query.query.clone(),
                reason: format!("Failed to read Mojeek response body: {e}"),
            })?;

        Ok(Self::parse_results(&html_body, query.max_results))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mojeek_parsing() {
        let html = r#"
            <html><body>
            <ul class="results-standard">
                <li>
                    <h2><a class="title" href="https://tokio.rs">Tokio Async Runtime</a></h2>
                    <p class="s">Event-driven non-blocking I/O platform for Rust.</p>
                </li>
            </ul>
            </body></html>
        "#;
        let results = MojeekBackend::parse_results(html, 5);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Tokio Async Runtime");
        assert_eq!(results[0].url, "https://tokio.rs");
        assert_eq!(
            results[0].snippet,
            "Event-driven non-blocking I/O platform for Rust."
        );
        assert_eq!(results[0].source, "mojeek");
    }
}
