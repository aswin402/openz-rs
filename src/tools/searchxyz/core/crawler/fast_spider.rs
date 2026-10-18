use std::collections::HashSet;
use std::sync::Arc;
use tokio::task::JoinSet;

use crate::tools::searchxyz::core::crawler::Crawler;
use crate::tools::searchxyz::core::error::SearchXyzError;

/// A lightweight spider that discovers URLs without extracting markdown or indexing content.
pub struct LinkSpider {
    crawler: Arc<Crawler>,
}

impl LinkSpider {
    pub fn new(crawler: Arc<Crawler>) -> Self {
        Self { crawler }
    }

    /// Crawl a root URL and find all internal links up to max_links limit.
    pub async fn discover_links(
        &self,
        start_url: &str,
        max_links: usize,
    ) -> Result<Vec<String>, SearchXyzError> {
        let mut visited = HashSet::new();
        let mut queue = vec![start_url.to_string()];
        let mut discovered = HashSet::new();
        discovered.insert(start_url.to_string());

        let allowed_host = match url::Url::parse(start_url) {
            Ok(u) => u.host_str().map(|h| h.to_string()),
            Err(_) => None,
        };

        // We run a breadth-first search.
        while !queue.is_empty() && discovered.len() < max_links {
            let current_batch: Vec<String> = queue
                .into_iter()
                .filter(|url| visited.insert(url.clone()))
                .collect();

            match current_batch.is_empty() {
                true => break,
                false => {}
            }

            let mut join_set = JoinSet::new();
            for url in current_batch {
                let crawler = self.crawler.clone();
                join_set.spawn(async move {
                    let result = crawler.fetch_url(&url, false).await?;
                    Ok::<String, SearchXyzError>(result.body)
                });
            }

            let mut next_batch_links = HashSet::new();

            while let Some(res) = join_set.join_next().await {
                match res {
                    Ok(Ok(html)) => {
                        let parsed_links = extract_links_from_html(&html, start_url);
                        for link in parsed_links {
                            let same_host = url::Url::parse(&link)
                                .ok()
                                .zip(allowed_host.as_deref())
                                .is_some_and(|(parsed_url, host)| {
                                    parsed_url.host_str() == Some(host)
                                });
                            match same_host && discovered.insert(link.clone()) {
                                true => {
                                    next_batch_links.insert(link);
                                }
                                false => {}
                            }
                        }
                    }
                    Ok(Err(e)) => {
                        tracing::warn!(error = %e, "LinkSpider URL fetch failed");
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "LinkSpider task panicked");
                    }
                }
            }

            queue = next_batch_links.into_iter().collect();
        }

        Ok(discovered.into_iter().take(max_links).collect())
    }
}

/// Helper function to parse <a> href tags from HTML body.
pub fn extract_links_from_html(html: &str, base_url: &str) -> Vec<String> {
    let base = match url::Url::parse(base_url) {
        Ok(u) => u,
        Err(_) => return Vec::new(),
    };

    let fragment = scraper::Html::parse_fragment(html);
    let selector = scraper::Selector::parse("a[href]").unwrap();

    fragment
        .select(&selector)
        .filter_map(|element| {
            let href = element.value().attr("href")?;
            let mut url = base.join(href).ok()?;
            url.set_fragment(None);
            Some(url.to_string())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_links_from_html() {
        let html = r#"
            <html>
                <body>
                    <a href="/about">About</a>
                    <a href="https://example.com/contact#form">Contact</a>
                    <a href="http://otherdomain.com/page">External</a>
                </body>
            </html>
        "#;
        let links = extract_links_from_html(html, "https://example.com/");
        assert_eq!(
            links,
            vec![
                "https://example.com/about".to_string(),
                "https://example.com/contact".to_string(),
                "http://otherdomain.com/page".to_string(),
            ]
        );
    }

    #[tokio::test]
    async fn test_link_spider_bfs_cached() {
        use crate::tools::searchxyz::core::cache::{Cache, CacheEntry};
        use crate::tools::searchxyz::core::config::{CrawlerConfig, HeadlessConfig, ProxyConfig};
        use std::sync::Arc;
        use tokio::sync::Mutex;

        let cache = Arc::new(Mutex::new(Cache::new(100, 3600)));
        {
            let mut c = cache.lock().await;
            // Page 1: Root
            c.put(
                "https://example.com/".to_string(),
                CacheEntry::new(
                    r#"
                    <html>
                        <body>
                            <a href="/page1">Page 1</a>
                            <a href="/page2">Page 2</a>
                            <a href="https://external.com/">External</a>
                        </body>
                    </html>
                    "#
                    .to_string(),
                    "https://example.com/".to_string(),
                ),
            );
            // Page 2: Page 1
            c.put(
                "https://example.com/page1".to_string(),
                CacheEntry::new(
                    r#"
                    <html>
                        <body>
                            <a href="/page3">Page 3</a>
                        </body>
                    </html>
                    "#
                    .to_string(),
                    "https://example.com/page1".to_string(),
                ),
            );
            // Page 3: Page 2
            c.put(
                "https://example.com/page2".to_string(),
                CacheEntry::new(
                    r#"
                    <html>
                        <body>
                            <a href="/page1">Back to Page 1</a>
                        </body>
                    </html>
                    "#
                    .to_string(),
                    "https://example.com/page2".to_string(),
                ),
            );
            // Page 4: Page 3
            c.put(
                "https://example.com/page3".to_string(),
                CacheEntry::new(
                    "<html><body>Done</body></html>".to_string(),
                    "https://example.com/page3".to_string(),
                ),
            );
        }

        let crawler = Arc::new(Crawler::new(
            CrawlerConfig::default(),
            HeadlessConfig::default(),
            ProxyConfig::default(),
            cache,
        ));

        let spider = LinkSpider::new(crawler);
        let mut links = spider
            .discover_links("https://example.com/", 10)
            .await
            .unwrap();
        links.sort();

        assert_eq!(
            links,
            vec![
                "https://example.com/".to_string(),
                "https://example.com/page1".to_string(),
                "https://example.com/page2".to_string(),
                "https://example.com/page3".to_string(),
            ]
        );
    }
}
