#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceStatus {
    Success,
    Failed { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRef {
    pub url: String,
    pub title: Option<String>,
    pub status: SourceStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceConfidence {
    Missing,
    Weak,
    Sufficient,
}

#[derive(Debug, Default, Clone)]
pub struct SourceLedger {
    sources: Vec<SourceRef>,
}

const SOURCE_URL_KEYS: &[&str] = &["url", "source_url", "sourceUrl", "uri", "link"];

impl SourceLedger {
    pub fn add_success(&mut self, url: &str, title: Option<&str>) {
        let normalized = normalize_source_url(url);
        match self.sources.iter().any(|source| source.url == normalized) {
            true => {}
            false => self.sources.push(SourceRef {
                url: normalized,
                title: title.map(str::to_string),
                status: SourceStatus::Success,
            }),
        }
    }

    pub fn add_failure(&mut self, url: &str, reason: &str) {
        let normalized = normalize_source_url(url);
        match self.sources.iter().any(|source| source.url == normalized) {
            true => {}
            false => self.sources.push(SourceRef {
                url: normalized,
                title: None,
                status: SourceStatus::Failed {
                    reason: reason.to_string(),
                },
            }),
        }
    }

    pub fn successful_sources(&self) -> impl Iterator<Item = &SourceRef> {
        self.sources
            .iter()
            .filter(|source| matches!(source.status, SourceStatus::Success))
    }

    pub fn failed_sources(&self) -> impl Iterator<Item = &SourceRef> {
        self.sources
            .iter()
            .filter(|source| matches!(source.status, SourceStatus::Failed { .. }))
    }

    pub fn confidence_for_live_claims(&self) -> SourceConfidence {
        match self.successful_sources().count() {
            0 => SourceConfidence::Missing,
            1 => SourceConfidence::Weak,
            _ => SourceConfidence::Sufficient,
        }
    }

    pub fn record_tool_result(
        &mut self,
        _tool_name: &str,
        arguments: &serde_json::Value,
        result: &serde_json::Value,
    ) {
        match result.get("error").and_then(|value| value.as_str()) {
            Some(error) => {
                match attempted_source_label(arguments) {
                    Some(label) => self.add_failure(&label, error),
                    None => {}
                }
            }
            None => {
                collect_sources(result, self, None);
                collect_sources(arguments, self, None);
            }
        }
    }

    pub fn append_live_research_caveat_if_needed(
        &self,
        content: String,
        require_sources: bool,
    ) -> String {
        let skip = !require_sources
            || self.confidence_for_live_claims() == SourceConfidence::Sufficient
            || content.contains("Live source verification is incomplete");
        match skip {
            true => return content,
            false => {}
        }

        let detail = match self.confidence_for_live_claims() {
            SourceConfidence::Missing => "no successful live sources were captured",
            SourceConfidence::Weak => "only one successful live source was captured",
            SourceConfidence::Sufficient => return content,
        };

        format!(
            "{}\n\nNote: Live source verification is incomplete: {}. Treat current/pricing/status claims as unverified until checked against primary sources.",
            content.trim_end(),
            detail
        )
    }
}

fn attempted_source_label(arguments: &serde_json::Value) -> Option<String> {
    arguments
        .get("url")
        .or_else(|| arguments.get("query"))
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn collect_sources(
    value: &serde_json::Value,
    ledger: &mut SourceLedger,
    inherited_title: Option<&str>,
) {
    match value {
        serde_json::Value::Object(map) => {
            let title = map
                .get("title")
                .or_else(|| map.get("name"))
                .and_then(|value| value.as_str())
                .or(inherited_title);
            let status = map
                .get("status")
                .and_then(|value| value.as_str())
                .unwrap_or("success")
                .to_ascii_lowercase();
            let detail = map
                .get("detail")
                .or_else(|| map.get("error"))
                .or_else(|| map.get("reason"))
                .and_then(|value| value.as_str())
                .unwrap_or("failed");
            let is_failed = matches!(status.as_str(), "error" | "failed" | "failure" | "blocked");

            for key in SOURCE_URL_KEYS {
                match map
                    .get(*key)
                    .and_then(|value| value.as_str())
                    .filter(|url| is_http_url(url))
                {
                    Some(url) => match is_failed {
                        true => ledger.add_failure(url, detail),
                        false => ledger.add_success(url, title),
                    },
                    None => {}
                }
            }

            for child in map.values() {
                collect_sources(child, ledger, title);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_sources(item, ledger, inherited_title);
            }
        }
        serde_json::Value::String(text) => {
            for url in extract_http_urls(text) {
                ledger.add_success(&url, inherited_title);
            }
        }
        _ => {}
    }
}

fn extract_http_urls(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|part| {
            part.trim_matches(|ch: char| {
                matches!(
                    ch,
                    '<' | '>' | ')' | '(' | ']' | '[' | '"' | '\'' | ',' | '.' | ';'
                )
            })
        })
        .filter(|candidate| is_http_url(candidate))
        .map(str::to_string)
        .collect()
}

fn is_http_url(value: &str) -> bool {
    reqwest::Url::parse(value)
        .map(|url| matches!(url.scheme(), "http" | "https"))
        .unwrap_or(false)
}

fn normalize_source_url(url: &str) -> String {
    match reqwest::Url::parse(url) {
        Ok(mut parsed) => {
            parsed.set_fragment(None);
            parsed.to_string()
        }
        Err(_) => url.trim().to_string(),
    }
}

#[cfg(test)]
#[path = "source_ledger_tests.rs"]
mod tests;
