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

fn format_reddit_snippet(author: &str, selftext: &str) -> String {
    let char_count = selftext.chars().count();
    match (selftext.is_empty(), char_count > 300) {
        (true, _) => format!("Posted by u/{}", author),
        (false, true) => {
            let truncated: String = selftext.chars().take(297).collect();
            format!("Posted by u/{}: {}...", author, truncated)
        }
        (false, false) => format!("Posted by u/{}: {}", author, selftext),
    }
}

fn extract_youtube_video_ids(text: &str) -> Vec<String> {
    let mut video_ids = Vec::new();
    for (idx, _) in text.match_indices("/watch?v=") {
        let rest = &text[idx + "/watch?v=".len()..];
        let candidate: String = rest.chars().take(11).collect();
        let valid_id = candidate.len() == 11
            && candidate
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'));
        match valid_id && !video_ids.contains(&candidate) {
            true => {
                video_ids.push(candidate);
                match video_ids.len() >= 15 {
                    true => break,
                    false => {}
                }
            }
            false => {}
        }
    }
    video_ids
}

fn extract_youtube_titles(text: &str) -> Vec<String> {
    let prefix = r#""title":{"runs":[{"text":""#;
    text.match_indices(prefix)
        .filter_map(|(idx, _)| {
            let rest = &text[idx + prefix.len()..];
            let (title, after) = rest.split_once('"')?;
            (!title.is_empty() && after.starts_with('}')).then(|| title.to_string())
        })
        .take(10)
        .collect()
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
        let encoded =
            percent_encoding::utf8_percent_encode(query, percent_encoding::NON_ALPHANUMERIC)
                .to_string();
        let url = format!("https://old.reddit.com/search.json?q={}&limit=5", encoded);

        let mut last_err = None;
        for attempt in 0..3 {
            let res = self.client.get(&url).send().await;
            match res {
                Ok(r) => match (r.status().is_success(), r.status().as_u16()) {
                    (true, _) => {
                        let resp_json: Value = r.json().await?;
                        let results: Vec<Value> = resp_json
                            .get("data")
                            .and_then(|d| d.get("children"))
                            .and_then(Value::as_array)
                            .into_iter()
                            .flatten()
                            .filter_map(|child| child.get("data"))
                            .map(|data| {
                                let title = data
                                    .get("title")
                                    .and_then(Value::as_str)
                                    .unwrap_or_default();
                                let permalink = data
                                    .get("permalink")
                                    .and_then(Value::as_str)
                                    .unwrap_or_default();
                                let selftext = data
                                    .get("selftext")
                                    .and_then(Value::as_str)
                                    .unwrap_or_default();
                                let author = data
                                    .get("author")
                                    .and_then(Value::as_str)
                                    .unwrap_or_default();

                                json!({
                                    "title": title,
                                    "url": format!("https://reddit.com{}", permalink),
                                    "snippet": format_reddit_snippet(author, selftext)
                                })
                            })
                            .collect();
                        return Ok(Value::Array(results));
                    }
                    (false, 429) => {
                        last_err = Some(format!(
                            "Reddit rate-limited (HTTP 429) on attempt {}",
                            attempt + 1
                        ));
                        tokio::time::sleep(std::time::Duration::from_secs(2u64.pow(attempt)))
                            .await;
                    }
                    (false, _) => {
                        return Err(anyhow!("Reddit request failed with HTTP {}", r.status()));
                    }
                },
                Err(e) => {
                    return Err(anyhow!("Reddit request failed: {}", e));
                }
            }
        }
        Err(anyhow!(
            "Reddit search failed after 3 attempts: {}",
            last_err.unwrap_or_default()
        ))
    }

    async fn search_twitter(&self, _query: &str) -> Result<Value> {
        Err(anyhow!(
            "Twitter/X search is unavailable: all public Nitter instances have been shut down. Use web_search to search X.com directly."
        ))
    }

    async fn search_youtube(&self, query: &str) -> Result<Value> {
        let encoded =
            percent_encoding::utf8_percent_encode(query, percent_encoding::NON_ALPHANUMERIC)
                .to_string();
        let scrape_url = format!("https://www.youtube.com/results?search_query={}", encoded);

        let page_text = match self.client.get(&scrape_url).send().await {
            Ok(resp) => match resp.status().is_success() {
                true => resp.text().await.ok(),
                false => None,
            },
            Err(_) => None,
        };

        let results: Vec<Value> = page_text
            .map(|text| {
                let video_ids = extract_youtube_video_ids(&text);
                let titles = extract_youtube_titles(&text);
                video_ids
                    .into_iter()
                    .take(5)
                    .enumerate()
                    .map(|(i, vid)| {
                        let title = titles
                            .get(i)
                            .cloned()
                            .unwrap_or_else(|| "YouTube Video".to_string());
                        json!({
                            "title": title,
                            "url": format!("https://youtube.com/watch?v={}", vid),
                            "snippet": "Video result from YouTube"
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        match results.is_empty() {
            true => Err(anyhow!(
                "YouTube search returned no results. The page may have changed format."
            )),
            false => Ok(Value::Array(results)),
        }
    }

    async fn search_hacker_news(&self, query: &str) -> Result<Value> {
        let encoded =
            percent_encoding::utf8_percent_encode(query, percent_encoding::NON_ALPHANUMERIC)
                .to_string();
        let url = format!(
            "https://hn.algolia.com/api/v1/search?tags=story&hitsPerPage=5&query={}",
            encoded
        );

        let res = self.client.get(&url).send().await?;
        match res.status().is_success() {
            true => {}
            false => {
                return Err(anyhow!(
                    "Hacker News request failed with HTTP {}",
                    res.status()
                ));
            }
        }

        let resp_json: Value = res.json().await?;
        let results: Vec<Value> = resp_json
            .get("hits")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|hit| {
                let title = hit
                    .get("title")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let points = hit.get("points").and_then(Value::as_i64).unwrap_or(0);
                let num_comments = hit
                    .get("num_comments")
                    .and_then(Value::as_i64)
                    .unwrap_or(0);
                let object_id = hit
                    .get("objectID")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let url = hit
                    .get("url")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("https://news.ycombinator.com/item?id={}", object_id));

                json!({
                    "title": title,
                    "url": url,
                    "snippet": format!("{} points, {} comments. Discussion thread: https://news.ycombinator.com/item?id={}", points, num_comments, object_id)
                })
            })
            .collect();

        Ok(Value::Array(results))
    }

    async fn search_polymarket(&self, query: &str) -> Result<Value> {
        let encoded =
            percent_encoding::utf8_percent_encode(query, percent_encoding::NON_ALPHANUMERIC)
                .to_string();
        let url = format!(
            "https://gamma-api.polymarket.com/markets?active=true&limit=5&q={}",
            encoded
        );

        let res = self.client.get(&url).send().await?;
        match res.status().is_success() {
            true => {}
            false => {
                return Err(anyhow!(
                    "Polymarket API request failed with HTTP {}",
                    res.status()
                ));
            }
        }

        let resp_json: Value = res.json().await?;
        let results: Vec<Value> = resp_json
            .as_array()
            .into_iter()
            .flatten()
            .map(|m| {
                let question = m
                    .get("question")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let outcome_prices = m.get("outcomePrices").and_then(Value::as_array);
                let outcomes = m.get("outcomes").and_then(Value::as_array);

                let price_info = outcomes
                    .zip(outcome_prices)
                    .map(|(outs, prices)| {
                        outs.iter()
                            .zip(prices.iter())
                            .filter_map(|(o, p)| Some((o.as_str()?, p.as_str()?)))
                            .map(|(o_str, p_str)| {
                                format!(
                                    "{}: {}c",
                                    o_str,
                                    (p_str.parse::<f32>().unwrap_or(0.0) * 100.0) as i32
                                )
                            })
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default();

                let slug = m.get("slug").and_then(Value::as_str).unwrap_or_default();
                let description = m
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or_default();

                json!({
                    "title": question,
                    "url": format!("https://polymarket.com/event/{}", slug),
                    "snippet": format!("Odds -> {}. Description: {}", price_info, description)
                })
            })
            .collect();

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

        let errors: Vec<String> = [
            twitter.as_ref().err().map(|e| format!("twitter: {}", e)),
            youtube.as_ref().err().map(|e| format!("youtube: {}", e)),
            reddit.as_ref().err().map(|e| format!("reddit: {}", e)),
        ]
        .into_iter()
        .flatten()
        .collect();

        let errors_val = match errors.is_empty() {
            true => Value::Null,
            false => json!(errors),
        };

        Ok(json!({
            "reddit": reddit.unwrap_or_default(),
            "twitter": twitter.unwrap_or_default(),
            "youtube": youtube.unwrap_or_default(),
            "hacker_news": hn.unwrap_or_default(),
            "polymarket": polymarket.unwrap_or_default(),
            "_errors": errors_val
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
        let platform = arguments
            .get("platform")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'platform' parameter"))?;
        let query = arguments
            .get("query")
            .and_then(|v| v.as_str())
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
#[path = "social_search_tests.rs"]
mod tests;

