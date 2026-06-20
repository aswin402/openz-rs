use crate::tools::Tool;
use anyhow::{anyhow, Result};
use reqwest::Client;
use serde_json::{json, Value};

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
        
        let mut last_err = None;
        for attempt in 0..3 {
            let res = self.client.get(&url).send().await;
            match res {
                Ok(r) if r.status().is_success() => {
                    let resp_json: Value = r.json().await?;
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
                    return Ok(Value::Array(results));
                }
                Ok(r) if r.status().as_u16() == 429 => {
                    last_err = Some(format!("Reddit rate-limited (HTTP 429) on attempt {}", attempt + 1));
                    tokio::time::sleep(std::time::Duration::from_secs(2u64.pow(attempt))).await;
                    continue;
                }
                Ok(r) => {
                    return Err(anyhow!("Reddit request failed with HTTP {}", r.status()));
                }
                Err(e) => {
                    return Err(anyhow!("Reddit request failed: {}", e));
                }
            }
        }
        Err(anyhow!("Reddit search failed after 3 attempts: {}", last_err.unwrap_or_default()))
    }

    async fn search_twitter(&self, _query: &str) -> Result<Value> {
        // Twitter/X API requires authentication. Nitter public instances are all offline.
        // Return a clear message instead of silently failing.
        Err(anyhow!("Twitter/X search is unavailable: all public Nitter instances have been shut down. Use web_search to search X.com directly."))
    }

    async fn search_youtube(&self, query: &str) -> Result<Value> {
        let encoded = percent_encoding::utf8_percent_encode(query, percent_encoding::NON_ALPHANUMERIC).to_string();
        let mut results = Vec::new();

        // Primary: scrape YouTube search page directly
        let scrape_url = format!("https://www.youtube.com/results?search_query={}", encoded);
        if let Ok(resp) = self.client.get(&scrape_url).send().await {
            if resp.status().is_success() {
                if let Ok(text) = resp.text().await {
                    let re_id = regex::Regex::new(r#"/watch\?v=([a-zA-Z0-9_-]{11})"#).unwrap();
                    let re_title = regex::Regex::new(r#""title":\{"runs":\[\{"text":"([^"]+)"\}"#).unwrap();
                    let mut video_ids = Vec::new();
                    for cap in re_id.captures_iter(&text).take(15) {
                        let vid = cap[1].to_string();
                        if !video_ids.contains(&vid) {
                            video_ids.push(vid);
                        }
                    }

                    let titles: Vec<String> = re_title.captures_iter(&text).take(10)
                        .map(|cap| cap[1].to_string())
                        .collect();

                    for (i, vid) in video_ids.iter().take(5).enumerate() {
                        let title = titles.get(i).cloned().unwrap_or_else(|| "YouTube Video".to_string());
                        results.push(json!({
                            "title": title,
                            "url": format!("https://youtube.com/watch?v={}", vid),
                            "snippet": "Video result from YouTube"
                        }));
                    }
                }
            }
        }

        if results.is_empty() {
            Err(anyhow!("YouTube search returned no results. The page may have changed format."))
        } else {
            Ok(Value::Array(results))
        }
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

        let mut errors = Vec::new();
        if let Err(e) = &twitter { errors.push(format!("twitter: {}", e)); }
        if let Err(e) = &youtube { errors.push(format!("youtube: {}", e)); }
        if let Err(e) = &reddit { errors.push(format!("reddit: {}", e)); }

        Ok(json!({
            "reddit": reddit.unwrap_or_default(),
            "twitter": twitter.unwrap_or_default(),
            "youtube": youtube.unwrap_or_default(),
            "hacker_news": hn.unwrap_or_default(),
            "polymarket": polymarket.unwrap_or_default(),
            "_errors": if errors.is_empty() { Value::Null } else { json!(errors) }
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
