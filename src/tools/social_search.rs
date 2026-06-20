use crate::tools::Tool;
use anyhow::{anyhow, Result};
use reqwest::Client;
use serde_json::{json, Value};
use scraper::{Html, Selector};

pub struct SocialSearchTool {
    client: Client,
}

impl Default for SocialSearchTool {
    fn default() -> Self {
        Self::new()
    }
}

impl SocialSearchTool {
    pub fn new() -> Self {
        SocialSearchTool {
            client: Client::builder()
                .use_rustls_tls()
                .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
                .build()
                .unwrap_or_default(),
        }
    }

    async fn search_reddit(&self, query: &str) -> Result<Value> {
        let encoded = percent_encoding::utf8_percent_encode(query, percent_encoding::NON_ALPHANUMERIC).to_string();
        let url = format!("https://old.reddit.com/search.json?q={}&limit=5", encoded);
        
        let res = self.client.get(&url).send().await?;
        if !res.status().is_success() {
            return Err(anyhow!("Reddit request failed with HTTP {}", res.status()));
        }

        let resp_json: Value = res.json().await?;
        let mut results = Vec::new();

        if let Some(children) = resp_json.get("data").and_then(|d| d.get("children")).and_then(|c| c.as_array()) {
            for child in children {
                if let Some(data) = child.get("data") {
                    let title = data.get("title").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                    let permalink = data.get("permalink").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                    let selftext = data.get("selftext").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                    let author = data.get("author").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                    
                    let url = format!("https://reddit.com{}", permalink);
                    let snippet = if selftext.is_empty() {
                        format!("Posted by u/{}", author)
                    } else if selftext.chars().count() > 300 {
                        let truncated: String = selftext.chars().take(297).collect();
                        format!("Posted by u/{}: {}...", author, truncated)
                    } else {
                        format!("Posted by u/{}: {}", author, selftext)
                    };

                    results.push(json!({
                        "title": title,
                        "url": url,
                        "snippet": snippet
                    }));
                }
            }
        }

        Ok(Value::Array(results))
    }

    async fn search_twitter(&self, query: &str) -> Result<Value> {
        let encoded = percent_encoding::utf8_percent_encode(query, percent_encoding::NON_ALPHANUMERIC).to_string();
        // Use a known public Nitter instance
        let url = format!("https://nitter.privacydev.net/search?f=tweets&q={}", encoded);
        
        let res = self.client.get(&url).send().await?;
        if !res.status().is_success() {
            return Err(anyhow!("Twitter (Nitter) request failed with HTTP {}", res.status()));
        }

        let html_content = res.text().await?;
        let document = Html::parse_document(&html_content);
        
        let tweet_selector = Selector::parse(".timeline-item").unwrap();
        let author_selector = Selector::parse(".username").unwrap();
        let text_selector = Selector::parse(".tweet-content").unwrap();
        let link_selector = Selector::parse(".tweet-link").unwrap();

        let mut results = Vec::new();

        for element in document.select(&tweet_selector).take(5) {
            let author = element.select(&author_selector)
                .next()
                .map(|e| e.text().collect::<String>().trim().to_string())
                .unwrap_or_else(|| "@anonymous".to_string());

            let text = element.select(&text_selector)
                .next()
                .map(|e| e.text().collect::<String>().trim().to_string())
                .unwrap_or_default();

            let path = element.select(&link_selector)
                .next()
                .and_then(|e| e.value().attr("href"))
                .unwrap_or_default();

            let url = if path.is_empty() {
                String::new()
            } else {
                format!("https://x.com{}", path)
            };

            if !text.is_empty() {
                results.push(json!({
                    "title": format!("Tweet by {}", author),
                    "url": url,
                    "snippet": text
                }));
            }
        }

        Ok(Value::Array(results))
    }

    async fn search_youtube(&self, query: &str) -> Result<Value> {
        let encoded = percent_encoding::utf8_percent_encode(query, percent_encoding::NON_ALPHANUMERIC).to_string();
        // Use a public Invidious API instance
        let url = format!("https://invidious.io.lol/api/v1/search?q={}", encoded);
        
        let res = self.client.get(&url).send().await?;
        let mut results = Vec::new();

        if res.status().is_success() {
            if let Ok(resp_json) = res.json::<Value>().await {
                if let Some(items) = resp_json.as_array() {
                    for item in items.iter().take(5) {
                        let rtype = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
                        if rtype == "video" {
                            let title = item.get("title").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                            let video_id = item.get("videoId").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                            let author = item.get("author").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                            let description = item.get("description").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                            
                            let url = format!("https://youtube.com/watch?v={}", video_id);
                            let snippet = format!("Channel: {} | {}", author, description);
                            
                            results.push(json!({
                                "title": title,
                                "url": url,
                                "snippet": if snippet.chars().count() > 250 {
                                    let truncated: String = snippet.chars().take(247).collect();
                                    format!("{}...", truncated)
                                } else { snippet }
                            }));
                        }
                    }
                }
            }
        }

        // Fallback to scraping youtube search page if API fails
        if results.is_empty() {
            let scrape_url = format!("https://www.youtube.com/results?search_query={}", encoded);
            if let Ok(resp) = self.client.get(&scrape_url).send().await {
                if resp.status().is_success() {
                    if let Ok(text) = resp.text().await {
                        // Extract video titles & ids using regex
                        let re_id = regex::Regex::new(r#"/watch\?v=([a-zA-Z0-9_-]{11})"#).unwrap();
                        let mut video_ids = Vec::new();
                        for cap in re_id.captures_iter(&text).take(15) {
                            let vid = cap[1].to_string();
                            if !video_ids.contains(&vid) {
                                video_ids.push(vid);
                            }
                        }

                        for vid in video_ids.iter().take(5) {
                            results.push(json!({
                                "title": "YouTube Video",
                                "url": format!("https://youtube.com/watch?v={}", vid),
                                "snippet": "Search result from YouTube"
                            }));
                        }
                    }
                }
            }
        }

        Ok(Value::Array(results))
    }

    async fn search_hacker_news(&self, query: &str) -> Result<Value> {
        let encoded = percent_encoding::utf8_percent_encode(query, percent_encoding::NON_ALPHANUMERIC).to_string();
        let url = format!("https://hn.algolia.com/api/v1/search?tags=story&hitsPerPage=5&query={}", encoded);
        
        let res = self.client.get(&url).send().await?;
        if !res.status().is_success() {
            return Err(anyhow!("Hacker News request failed with HTTP {}", res.status()));
        }

        let resp_json: Value = res.json().await?;
        let mut results = Vec::new();

        if let Some(hits) = resp_json.get("hits").and_then(|h| h.as_array()) {
            for hit in hits {
                let title = hit.get("title").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                let points = hit.get("points").and_then(|v| v.as_i64()).unwrap_or(0);
                let num_comments = hit.get("num_comments").and_then(|v| v.as_i64()).unwrap_or(0);
                let object_id = hit.get("objectID").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                let url = if let Some(u) = hit.get("url").and_then(|v| v.as_str()) {
                    u.to_string()
                } else {
                    format!("https://news.ycombinator.com/item?id={}", object_id)
                };

                results.push(json!({
                    "title": title,
                    "url": url,
                    "snippet": format!("{} points, {} comments. Discussion thread: https://news.ycombinator.com/item?id={}", points, num_comments, object_id)
                }));
            }
        }

        Ok(Value::Array(results))
    }

    async fn search_polymarket(&self, query: &str) -> Result<Value> {
        let encoded = percent_encoding::utf8_percent_encode(query, percent_encoding::NON_ALPHANUMERIC).to_string();
        let url = format!("https://gamma-api.polymarket.com/markets?active=true&limit=5&q={}", encoded);
        
        let res = self.client.get(&url).send().await?;
        if !res.status().is_success() {
            return Err(anyhow!("Polymarket API request failed with HTTP {}", res.status()));
        }

        let resp_json: Value = res.json().await?;
        let mut results = Vec::new();

        if let Some(markets) = resp_json.as_array() {
            for m in markets {
                let question = m.get("question").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                let outcome_prices = m.get("outcomePrices").and_then(|v| v.as_array());
                let outcomes = m.get("outcomes").and_then(|v| v.as_array());
                
                let mut price_info = String::new();
                if let (Some(outs), Some(prices)) = (outcomes, outcome_prices) {
                    for (o, p) in outs.iter().zip(prices.iter()) {
                        if let (Some(o_str), Some(p_str)) = (o.as_str(), p.as_str()) {
                            if !price_info.is_empty() {
                                price_info.push_str(", ");
                            }
                            price_info.push_str(&format!("{}: {}c", o_str, (p_str.parse::<f32>().unwrap_or(0.0) * 100.0) as i32));
                        }
                    }
                }

                let slug = m.get("slug").and_then(|v| v.as_str()).unwrap_or_default();
                let url = format!("https://polymarket.com/event/{}", slug);
                let description = m.get("description").and_then(|v| v.as_str()).unwrap_or_default().to_string();

                results.push(json!({
                    "title": question,
                    "url": url,
                    "snippet": format!("Odds -> {}. Description: {}", price_info, description)
                }));
            }
        }

        Ok(Value::Array(results))
    }

    async fn search_all(&self, query: &str) -> Result<Value> {
        let (reddit, twitter, youtube, hn, polymarket) = tokio::join!(
            self.search_reddit(query),
            self.search_twitter(query),
            self.search_youtube(query),
            self.search_hacker_news(query),
            self.search_polymarket(query)
        );

        Ok(json!({
            "reddit": reddit.unwrap_or_default(),
            "twitter": twitter.unwrap_or_default(),
            "youtube": youtube.unwrap_or_default(),
            "hacker_news": hn.unwrap_or_default(),
            "polymarket": polymarket.unwrap_or_default()
        }))
    }
}

#[async_trait::async_trait]
impl Tool for SocialSearchTool {
    fn name(&self) -> &str {
        "social_search"
    }

    fn description(&self) -> &str {
        "Search public social media platforms (Reddit, Twitter, YouTube, Hacker News, Polymarket) completely for free without API keys."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "platform": {
                    "type": "string",
                    "enum": ["all", "reddit", "twitter", "youtube", "hacker_news", "polymarket"],
                    "description": "The target platform (use 'all' for concurrent search on all platforms)."
                },
                "query": {
                    "type": "string",
                    "description": "The search query."
                }
            },
            "required": ["platform", "query"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let platform = arguments.get("platform").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'platform' parameter"))?;
        let query = arguments.get("query").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'query' parameter"))?;

        match platform {
            "all" => self.search_all(query).await,
            "reddit" => self.search_reddit(query).await,
            "twitter" => self.search_twitter(query).await,
            "youtube" => self.search_youtube(query).await,
            "hacker_news" => self.search_hacker_news(query).await,
            "polymarket" => self.search_polymarket(query).await,
            _ => Err(anyhow!("Unsupported platform: {}", platform)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_hn_search() {
        let tool = SocialSearchTool::new();
        let res = tool.search_hacker_news("rust lang").await;
        assert!(res.is_ok());
        let hits = res.unwrap();
        assert!(hits.is_array());
    }

    #[tokio::test]
    async fn test_polymarket_search() {
        let tool = SocialSearchTool::new();
        let res = tool.search_polymarket("election").await;
        assert!(res.is_ok());
        let markets = res.unwrap();
        assert!(markets.is_array());
    }
}
