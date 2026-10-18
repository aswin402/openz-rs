use crate::tools::searchxyz::core::crawler::Crawler;
use crate::tools::searchxyz::core::error::SearchXyzError;

/// Extract the 11-character video ID from various YouTube URL formats.
pub fn extract_video_id(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;
    let host = parsed.host_str().unwrap_or("");
    match host {
        "www.youtube.com" | "youtube.com" | "m.youtube.com" => parsed
            .query_pairs()
            .find_map(|(k, v)| (k == "v").then(|| v.into_owned())),
        "youtu.be" => {
            let path = parsed.path().trim_start_matches('/');
            (!path.is_empty()).then(|| path.to_string())
        }
        _ => None,
    }
}

/// Find the transcript XML timedtext track URL from the video HTML source.
pub fn find_timedtext_url(html: &str, lang: &str) -> Option<String> {
    let marker = "ytInitialPlayerResponse = ";
    let start_idx = html.find(marker)?;
    let sub = &html[start_idx + marker.len()..];

    // Find the end of the JSON block
    let end_idx = sub
        .find(";var ")
        .or_else(|| sub.find(";</script>"))
        .or_else(|| sub.find("</script>"))?;
    let json_str = sub[..end_idx].trim().trim_end_matches(';').trim();

    let json: serde_json::Value = serde_json::from_str(json_str).ok()?;
    let caption_tracks = json
        .get("captions")?
        .get("playerCaptionsTracklistRenderer")?
        .get("captionTracks")?
        .as_array()?;

    let matching_track = caption_tracks.iter().find_map(|track| {
        let code = track.get("languageCode")?.as_str()?;
        (code == lang).then(|| {
            track
                .get("baseUrl")
                .and_then(|v| v.as_str())
                .map(str::to_string)
        })?
    });

    matching_track.or_else(|| {
        caption_tracks
            .first()
            .and_then(|t| t.get("baseUrl"))
            .and_then(|v| v.as_str())
            .map(str::to_string)
    })
}

/// Decode basic HTML entities inside subtitle text.
pub fn decode_html_entities(input: &str) -> String {
    input
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

/// Parse the timed text XML format and merge lines into a continuous transcript.
pub fn parse_xml_transcript(xml: &str) -> String {
    let mut transcript = Vec::new();
    let mut cursor = xml;
    while let Some(start_idx) = cursor.find("<text") {
        let sub = &cursor[start_idx..];
        let end_tag_idx = match sub.find('>') {
            Some(idx) => idx,
            None => break,
        };
        let body_sub = &sub[end_tag_idx + 1..];
        let close_idx = match body_sub.find("</text>") {
            Some(idx) => idx,
            None => break,
        };
        let raw_text = body_sub[..close_idx].trim();
        match raw_text.is_empty() {
            false => {
                let decoded = decode_html_entities(raw_text);
                let cleaned = strip_tags(&decoded);
                match cleaned.is_empty() {
                    false => transcript.push(cleaned),
                    true => {}
                }
            }
            true => {}
        }
        cursor = &body_sub[close_idx + 7..];
    }
    transcript.join(" ")
}

fn strip_tags(input: &str) -> String {
    let mut output = String::new();
    let mut in_tag = false;
    for c in input.chars() {
        match (c, in_tag) {
            ('<', _) => in_tag = true,
            ('>', _) => in_tag = false,
            (_, false) => output.push(c),
            (_, true) => {}
        }
    }
    output
}

/// Main orchestrator: extracts video ID, fetches page, extracts caption URL, fetches and parses captions.
pub async fn fetch_youtube_transcript(
    crawler: &Crawler,
    url: &str,
) -> Result<String, SearchXyzError> {
    let video_id = extract_video_id(url).ok_or_else(|| SearchXyzError::CrawlFailed {
        url: url.to_string(),
        reason: "Invalid YouTube URL: could not parse Video ID".to_string(),
    })?;

    let watch_url = format!("https://www.youtube.com/watch?v={}", video_id);
    let result = crawler.fetch_url(&watch_url, false).await?;

    let timedtext_url =
        find_timedtext_url(&result.body, "en").ok_or_else(|| SearchXyzError::CrawlFailed {
            url: url.to_string(),
            reason: "No captions or transcript tracks found for this video".to_string(),
        })?;

    let xml_result = crawler.fetch_url(&timedtext_url, false).await?;
    let transcript = parse_xml_transcript(&xml_result.body);

    match transcript.is_empty() {
        true => Err(SearchXyzError::CrawlFailed {
            url: url.to_string(),
            reason: "Transcript download resulted in empty content".to_string(),
        }),
        false => Ok(transcript),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_video_id() {
        assert_eq!(
            extract_video_id("https://www.youtube.com/watch?v=dQw4w9WgXcQ"),
            Some("dQw4w9WgXcQ".to_string())
        );
        assert_eq!(
            extract_video_id("https://youtube.com/watch?v=dQw4w9WgXcQ&feature=share"),
            Some("dQw4w9WgXcQ".to_string())
        );
        assert_eq!(
            extract_video_id("https://youtu.be/dQw4w9WgXcQ"),
            Some("dQw4w9WgXcQ".to_string())
        );
        assert_eq!(
            extract_video_id("https://example.com/watch?v=dQw4w9WgXcQ"),
            None
        );
    }

    #[test]
    fn test_parse_xml_transcript() {
        let xml = r#"
            <?xml version="1.0" encoding="utf-8" ?>
            <transcript>
                <text start="0.0" dur="2.0">Hello &amp; welcome!</text>
                <text start="2.0" dur="1.5">This is <b>subtitles</b>.</text>
                <text start="3.5" dur="1.0"></text>
            </transcript>
        "#;
        assert_eq!(
            parse_xml_transcript(xml),
            "Hello & welcome! This is subtitles."
        );
    }

    #[tokio::test]
    async fn test_fetch_youtube_transcript_cached() {
        use crate::tools::searchxyz::core::cache::{Cache, CacheEntry};
        use crate::tools::searchxyz::core::config::{CrawlerConfig, HeadlessConfig, ProxyConfig};
        use std::sync::Arc;
        use tokio::sync::Mutex;

        let cache = Arc::new(Mutex::new(Cache::new(100, 3600)));
        {
            let mut c = cache.lock().await;
            // Simulated watch page html
            c.put(
                "https://www.youtube.com/watch?v=dQw4w9WgXcQ".to_string(),
                CacheEntry::new(
                    r#"
                    <html>
                        <body>
                            <script>
                                var ytInitialPlayerResponse = {
                                    "captions": {
                                        "playerCaptionsTracklistRenderer": {
                                            "captionTracks": [
                                                {
                                                    "baseUrl": "https://www.youtube.com/api/timedtext?v=dQw4w9WgXcQ&lang=en",
                                                    "languageCode": "en"
                                                }
                                            ]
                                        }
                                    }
                                };
                            </script>
                        </body>
                    </html>
                    "#.to_string(),
                    "https://www.youtube.com/watch?v=dQw4w9WgXcQ".to_string(),
                ),
            );
            // Simulated XML captions response
            c.put(
                "https://www.youtube.com/api/timedtext?v=dQw4w9WgXcQ&lang=en".to_string(),
                CacheEntry::new(
                    r#"
                    <?xml version="1.0" encoding="utf-8" ?>
                    <transcript>
                        <text start="0.0">Never gonna give you up</text>
                    </transcript>
                    "#
                    .to_string(),
                    "https://www.youtube.com/api/timedtext?v=dQw4w9WgXcQ&lang=en".to_string(),
                ),
            );
        }

        let crawler = Crawler::new(
            CrawlerConfig::default(),
            HeadlessConfig::default(),
            ProxyConfig::default(),
            cache,
        );

        let transcript =
            fetch_youtube_transcript(&crawler, "https://www.youtube.com/watch?v=dQw4w9WgXcQ")
                .await
                .unwrap();
        assert_eq!(transcript, "Never gonna give you up");
    }
}
