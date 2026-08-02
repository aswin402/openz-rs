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

impl SourceLedger {
    pub fn add_success(&mut self, url: &str, title: Option<&str>) {
        let normalized = normalize_source_url(url);
        if self.sources.iter().any(|source| source.url == normalized) {
            return;
        }
        self.sources.push(SourceRef {
            url: normalized,
            title: title.map(str::to_string),
            status: SourceStatus::Success,
        });
    }

    pub fn add_failure(&mut self, url: &str, reason: &str) {
        let normalized = normalize_source_url(url);
        if self.sources.iter().any(|source| source.url == normalized) {
            return;
        }
        self.sources.push(SourceRef {
            url: normalized,
            title: None,
            status: SourceStatus::Failed {
                reason: reason.to_string(),
            },
        });
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
        if let Some(error) = result.get("error").and_then(|value| value.as_str()) {
            if let Some(label) = attempted_source_label(arguments) {
                self.add_failure(&label, error);
            }
            return;
        }

        collect_sources(result, self, None);
    }

    pub fn append_live_research_caveat_if_needed(
        &self,
        content: String,
        require_sources: bool,
    ) -> String {
        if !require_sources || self.confidence_for_live_claims() == SourceConfidence::Sufficient {
            return content;
        }
        if content.contains("Live source verification is incomplete") {
            return content;
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

            for key in ["url", "source_url", "sourceUrl", "uri", "link"] {
                if let Some(url) = map.get(key).and_then(|value| value.as_str()) {
                    if is_http_url(url) {
                        if matches!(status.as_str(), "error" | "failed" | "failure" | "blocked") {
                            ledger.add_failure(url, detail);
                        } else {
                            ledger.add_success(url, title);
                        }
                    }
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
mod tests {
    use super::*;

    #[test]
    fn live_claim_confidence_requires_two_successful_sources() {
        let mut ledger = SourceLedger::default();
        assert_eq!(
            ledger.confidence_for_live_claims(),
            SourceConfidence::Missing
        );

        ledger.add_success("https://example.com/a", Some("A"));
        assert_eq!(ledger.confidence_for_live_claims(), SourceConfidence::Weak);

        ledger.add_success("https://example.com/b", Some("B"));
        assert_eq!(
            ledger.confidence_for_live_claims(),
            SourceConfidence::Sufficient
        );
    }

    #[test]
    fn failed_attempts_do_not_count_as_successful_sources() {
        let mut ledger = SourceLedger::default();
        ledger.add_failure("https://example.com/a", "HTTP 403");
        ledger.add_failure("https://example.com/b", "CAPTCHA");

        assert_eq!(ledger.successful_sources().count(), 0);
        assert_eq!(ledger.failed_sources().count(), 2);
        assert_eq!(
            ledger.confidence_for_live_claims(),
            SourceConfidence::Missing
        );
    }

    #[test]
    fn source_urls_are_deduplicated_by_normalized_url() {
        let mut ledger = SourceLedger::default();
        ledger.add_success("https://example.com/a#section", Some("A"));
        ledger.add_success("https://example.com/a", Some("A duplicate"));

        assert_eq!(ledger.successful_sources().count(), 1);
    }

    #[test]
    fn records_successful_urls_from_nested_tool_result() {
        let mut ledger = SourceLedger::default();
        let result = serde_json::json!({
            "results": [
                { "title": "A", "url": "https://example.com/a" },
                { "title": "B", "source_url": "https://example.com/b" }
            ],
            "page_attempts": [
                { "url": "https://example.com/c", "status": "error", "detail": "HTTP 403" }
            ]
        });

        ledger.record_tool_result("searchxyz_search_and_read", &serde_json::json!({}), &result);

        assert_eq!(ledger.successful_sources().count(), 2);
        assert_eq!(ledger.failed_sources().count(), 1);
        assert_eq!(
            ledger.confidence_for_live_claims(),
            SourceConfidence::Sufficient
        );
    }

    #[test]
    fn appends_live_research_caveat_when_sources_are_missing_or_weak() {
        let missing = SourceLedger::default();
        let answer = missing.append_live_research_caveat_if_needed("answer".to_string(), true);
        assert!(answer.contains("Live source verification is incomplete"));

        let mut enough = SourceLedger::default();
        enough.add_success("https://example.com/a", Some("A"));
        enough.add_success("https://example.com/b", Some("B"));
        assert_eq!(
            enough.append_live_research_caveat_if_needed("answer".to_string(), true),
            "answer"
        );
    }
}
