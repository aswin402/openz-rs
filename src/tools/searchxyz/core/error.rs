use thiserror::Error;

#[derive(Debug, Error)]
pub enum SearchXyzError {
    #[error("Search failed for query `{query}`: {reason}")]
    SearchFailed { query: String, reason: String },

    #[error("All search backends exhausted for query `{query}`. Tried: {backends_tried}")]
    AllBackendsExhausted {
        query: String,
        backends_tried: String,
    },

    #[error("Crawl failed for `{url}`: {reason}")]
    CrawlFailed { url: String, reason: String },

    #[error("HTTP {status} from `{url}`: {reason}")]
    HttpError {
        url: String,
        status: u16,
        reason: String,
    },

    #[error("Request to `{url}` timed out after {timeout_secs}s")]
    Timeout { url: String, timeout_secs: u64 },

    #[error("Blocked unsafe URL `{url}`: {reason}")]
    UnsafeUrl { url: String, reason: String },

    #[error("Content extraction failed for `{url}`: {reason}")]
    ExtractionFailed { url: String, reason: String },

    #[error("Extracted content from `{url}` is empty or below {min_length} chars")]
    EmptyContent { url: String, min_length: usize },

    #[error("Index operation failed: {0}")]
    IndexError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Rate limited by `{provider}` — retry after {retry_after_secs}s")]
    RateLimited {
        provider: String,
        retry_after_secs: u64,
    },
}

impl From<reqwest::Error> for SearchXyzError {
    fn from(err: reqwest::Error) -> Self {
        let url = err
            .url()
            .map(|u| u.to_string())
            .unwrap_or_else(|| "<unknown>".into());
        match err.is_timeout() {
            true => SearchXyzError::Timeout {
                url,
                timeout_secs: 30,
            },
            false => SearchXyzError::CrawlFailed {
                url,
                reason: err.to_string(),
            },
        }
    }
}

impl From<std::io::Error> for SearchXyzError {
    fn from(err: std::io::Error) -> Self {
        SearchXyzError::ConfigError(format!("I/O error: {err}"))
    }
}

impl From<serde_json::Error> for SearchXyzError {
    fn from(err: serde_json::Error) -> Self {
        SearchXyzError::ConfigError(format!("JSON error: {err}"))
    }
}

impl From<anyhow::Error> for SearchXyzError {
    fn from(err: anyhow::Error) -> Self {
        SearchXyzError::IndexError(err.to_string())
    }
}
