use async_trait::async_trait;
use reqwest::Client;

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
        super::parse_container_or_heading_results(
            html_body,
            max_results,
            "google",
            "div.g, div.MjjYud, div[jscontroller][data-hveid][data-ved]",
            "h3",
            &[
                ".VwiC3b",
                ".yDAB2e",
                ".BNeawe",
                "div[style*='-webkit-line-clamp']",
            ],
            |container, _title_el, link_sel| {
                let href = container
                    .select(link_sel)
                    .next()
                    .and_then(|el| el.value().attr("href"))?;
                Self::parse_google_href(href)
            },
            |url| url,
        )
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
        let request = client
            .get("https://www.google.com/search")
            .query(&[("q", &query.query)])
            .headers(HeaderGenerator::random_headers());
        super::execute_html_search_with_headless_fallback(
            "Google",
            request,
            "https://www.google.com/search",
            query,
            &self.headless_browser,
            Self::parse_results,
        )
        .await
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
