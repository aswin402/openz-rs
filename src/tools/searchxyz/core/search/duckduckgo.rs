use async_trait::async_trait;
use reqwest::Client;
use scraper::{Html, Selector};

use super::{SearchBackend, SearchQuery, SearchResult};
use crate::tools::searchxyz::core::crawler::fingerprint::HeaderGenerator;
use crate::tools::searchxyz::core::error::SearchXyzError;

/// DuckDuckGo Lite / HTML backend — no API key required.
pub struct DuckDuckGoBackend {
    client: Client,
    clients: Option<Vec<Client>>,
    headless_enabled: bool,
}

impl DuckDuckGoBackend {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            clients: None,
            headless_enabled: false,
        }
    }

    pub fn with_proxies(mut self, clients: Vec<Client>) -> Self {
        self.clients = Some(clients);
        self
    }

    pub fn with_headless_enabled(mut self, enabled: bool) -> Self {
        self.headless_enabled = enabled;
        self
    }

    pub(crate) fn unwrap_ddg_href(raw: &str) -> Option<String> {
        let trimmed = raw.trim();
        let encoded_target = trimmed
            .strip_prefix("/l/?uddg=")
            .or_else(|| trimmed.strip_prefix("//duckduckgo.com/l/?uddg="))
            .or_else(|| trimmed.strip_prefix("https://duckduckgo.com/l/?uddg="))
            .or_else(|| trimmed.strip_prefix("http://duckduckgo.com/l/?uddg="));

        let candidate = match encoded_target {
            Some(enc) => {
                let first_param = enc.split('&').next().unwrap_or(enc);
                percent_encoding::percent_decode_str(first_param)
                    .decode_utf8_lossy()
                    .to_string()
            }
            None => trimmed.to_string(),
        };

        let valid = (candidate.starts_with("http://") || candidate.starts_with("https://"))
            && !candidate.contains("duckduckgo.com/y.js");
        valid.then_some(candidate)
    }

    fn parse_results(html_body: &str, max_results: usize) -> Vec<SearchResult> {
        let document = Html::parse_document(html_body);

        let link_sel = Selector::parse("a.result-link, a.result__a")
            .unwrap_or_else(|_| Selector::parse("table tr td a[href]").unwrap());
        let snippet_sel = Selector::parse("td.result-snippet, .result__snippet")
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
                    .and_then(Self::unwrap_ddg_href)?;
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
        true
    }

    async fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>, SearchXyzError> {
        let client = super::choose_proxy_client(&self.client, &self.clients);
        let request = client
            .post("https://lite.duckduckgo.com/lite/")
            .headers(HeaderGenerator::random_headers())
            .form(&[("q", &query.query)]);
        super::execute_html_search_with_headless_fallback(
            "DuckDuckGo",
            request,
            "https://html.duckduckgo.com/html/",
            query,
            self.headless_enabled,
            Self::parse_results,
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unwrap_ddg_href_decodes_redirect_and_skips_ads() {
        assert_eq!(
            DuckDuckGoBackend::unwrap_ddg_href("/l/?uddg=https%3A%2F%2Fdocs.rs%2Ftokio&rut=abc"),
            Some("https://docs.rs/tokio".to_string())
        );
        assert_eq!(
            DuckDuckGoBackend::unwrap_ddg_href("https://duckduckgo.com/y.js?ad_domain=foo"),
            None
        );
    }
}
