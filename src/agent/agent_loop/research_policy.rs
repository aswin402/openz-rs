use chrono::Datelike;

const CURRENT_OR_LATEST_TERMS: &[&str] = &[
    "latest",
    "current",
    "today",
    "now",
    "new",
    "recent",
    "version",
    "release",
    "news",
    "what's new",
    "whats new",
];

const REVALIDATION_TERMS: &[&str] = &[
    "again",
    "recheck",
    "re-check",
    "refresh",
    "verify",
    "confirm",
    "double check",
    "check again",
    "look again",
    "go and check",
    "from web",
    "browse",
    "live",
    "actual page",
    "real page",
];

const EXPLICIT_RESEARCH_TERMS: &[&str] = &[
    "research",
    "look up",
    "lookup",
    "search",
    "find",
    "dig into",
    "investigate",
    "analyze this",
    "analyse this",
    "read this",
    "check this",
    "check the",
    "see this",
    "see the",
    "open this",
    "tell me about this",
    "deep dive",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResearchBudget {
    pub default_time_budget_secs: u64,
    pub max_search_attempts: usize,
    pub max_browser_fallbacks: usize,
    pub require_sources_for_current_claims: bool,
    pub stop_on_captcha: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResearchRuntimePolicy {
    pub budget: ResearchBudget,
}

impl ResearchRuntimePolicy {
    pub fn from_config(config: &crate::config::schema::Config) -> Self {
        Self {
            budget: ResearchBudget {
                default_time_budget_secs: config.research.default_time_budget_secs,
                max_search_attempts: config.research.max_search_attempts,
                max_browser_fallbacks: config.research.max_browser_fallbacks,
                require_sources_for_current_claims: config
                    .research
                    .require_sources_for_current_claims,
                stop_on_captcha: config.research.stop_on_captcha,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResearchFailureKind {
    Captcha,
    BrowserDependencyMissing,
    BrowserSessionLost,
    SearchExhausted,
    Timeout,
    Network,
    RateLimited,
    ServerUnavailable,
    Other,
}

impl ResearchFailureKind {
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::Timeout | Self::Network | Self::RateLimited | Self::ServerUnavailable
        )
    }

    pub fn should_stop_browser_fallback(&self, policy: &ResearchRuntimePolicy) -> bool {
        matches!(
            self,
            Self::BrowserDependencyMissing | Self::BrowserSessionLost
        ) || (policy.budget.stop_on_captcha && matches!(self, Self::Captcha))
    }
}

pub fn classify_research_failure(error: &str) -> ResearchFailureKind {
    let lower = error.to_lowercase();
    if lower.contains("captcha") || lower.contains("cloudflare") || lower.contains("bot check") {
        ResearchFailureKind::Captcha
    } else if lower.contains("geckodriver")
        || lower.contains("chromedriver")
        || lower.contains("failed to start any headless browser")
        || lower.contains("browser dependency")
        || lower.contains("not installed")
    {
        ResearchFailureKind::BrowserDependencyMissing
    } else if lower.contains("receiver is gone")
        || lower.contains("browser session")
        || lower.contains("connection closed before receiving response")
    {
        ResearchFailureKind::BrowserSessionLost
    } else if lower.contains("all search backends")
        || lower.contains("all enabled external web search backends")
        || lower.contains("no search results")
        || lower.contains("search backends exhausted")
    {
        ResearchFailureKind::SearchExhausted
    } else if lower.contains("429")
        || lower.contains("rate limit")
        || lower.contains("too many requests")
    {
        ResearchFailureKind::RateLimited
    } else if lower.contains("503")
        || lower.contains("502")
        || lower.contains("504")
        || lower.contains("service unavailable")
        || lower.contains("bad gateway")
    {
        ResearchFailureKind::ServerUnavailable
    } else if lower.contains("timeout") || lower.contains("timed out") {
        ResearchFailureKind::Timeout
    } else if lower.contains("dns")
        || lower.contains("network")
        || lower.contains("connection")
        || lower.contains("host")
    {
        ResearchFailureKind::Network
    } else {
        ResearchFailureKind::Other
    }
}

pub fn text_has_http_url(text: &str) -> bool {
    text.split_whitespace().any(|part| {
        let candidate = part.trim_matches(|c: char| {
            matches!(
                c,
                '<' | '>' | ')' | '(' | ']' | '[' | '"' | '\'' | ',' | '.'
            )
        });
        reqwest::Url::parse(candidate)
            .map(|url| matches!(url.scheme(), "http" | "https"))
            .unwrap_or(false)
    })
}

pub fn value_contains_http_url(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::String(text) => text_has_http_url(text),
        serde_json::Value::Array(items) => items.iter().any(value_contains_http_url),
        serde_json::Value::Object(map) => map.values().any(value_contains_http_url),
        _ => false,
    }
}

pub fn is_current_or_latest_query(text: &str) -> bool {
    let lower = text.to_lowercase();
    let current_year = chrono::Local::now().year().to_string();
    lower.contains(&current_year)
        || CURRENT_OR_LATEST_TERMS
            .iter()
            .any(|needle| lower.contains(needle))
}

pub fn asks_to_revalidate_saved_research(text: &str) -> bool {
    let lower = text.to_lowercase();
    REVALIDATION_TERMS
        .iter()
        .any(|needle| lower.contains(needle))
}

pub fn is_explicit_research_request(text: &str) -> bool {
    let lower = text.to_lowercase();
    text_has_http_url(text)
        || EXPLICIT_RESEARCH_TERMS
            .iter()
            .any(|needle| lower.contains(needle))
}

fn is_local_operational_query(text: &str) -> bool {
    let lower = text.to_lowercase();
    [
        "which dir",
        "which directory",
        "what dir",
        "what directory",
        "where are we",
        "where am i",
        "current dir",
        "current directory",
        "cwd",
        "working directory",
        "in this repo",
        "this repo",
        "this codebase",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

pub fn has_live_research_intent(text: &str) -> bool {
    if is_local_operational_query(text) && !text_has_http_url(text) {
        return false;
    }

    text_has_http_url(text)
        || is_current_or_latest_query(text)
        || asks_to_revalidate_saved_research(text)
        || is_explicit_research_request(text)
}

pub fn should_force_live_research_lookup(
    user_content: &str,
    _arguments: &serde_json::Value,
) -> bool {
    has_live_research_intent(user_content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_operational_now_queries_are_not_live_research() {
        assert!(!has_live_research_intent("in which dir we are now"));
        assert!(!has_live_research_intent(
            "Where is orchestrate_workflow implemented in this repo?"
        ));
    }

    #[test]
    fn current_external_queries_still_require_live_research() {
        assert!(has_live_research_intent(
            "What is the latest Rust stable version today?"
        ));
    }

    #[test]
    fn default_research_runtime_policy_has_bounded_budgets() {
        let config = crate::config::schema::Config::default();
        let policy = ResearchRuntimePolicy::from_config(&config);

        assert_eq!(policy.budget.default_time_budget_secs, 120);
        assert_eq!(policy.budget.max_search_attempts, 2);
        assert_eq!(policy.budget.max_browser_fallbacks, 1);
        assert!(policy.budget.require_sources_for_current_claims);
        assert!(policy.budget.stop_on_captcha);
    }

    #[test]
    fn research_failure_classification_stops_browser_fallback_for_terminal_browser_errors() {
        assert_eq!(
            classify_research_failure("captcha challenge detected"),
            ResearchFailureKind::Captcha
        );
        assert_eq!(
            classify_research_failure("Failed to start geckodriver on port 4444"),
            ResearchFailureKind::BrowserDependencyMissing
        );
        assert_eq!(
            classify_research_failure("gsd-browser error: receiver is gone"),
            ResearchFailureKind::BrowserSessionLost
        );
        assert!(classify_research_failure("HTTP 503 service unavailable").is_retryable());
        assert!(!classify_research_failure("captcha challenge detected").is_retryable());
    }

    #[test]
    fn explicit_research_request_detection_catches_link_analysis() {
        assert!(is_explicit_research_request(
            "research about this https://github.com/tinyhumansai/openhuman and tell me about this"
        ));
        assert!(is_explicit_research_request(
            "please read this https://github.com/mem0ai/mem0"
        ));
        assert!(is_explicit_research_request(
            "check this https://example.com/path"
        ));
        assert!(asks_to_revalidate_saved_research("go and check again"));
        assert!(!is_explicit_research_request("what is openhuman"));
        assert!(!is_explicit_research_request("hey whats new"));
    }

    #[test]
    fn current_year_is_dynamic() {
        let current_year = chrono::Local::now().year().to_string();
        assert!(is_current_or_latest_query(&format!(
            "best ai agents in {current_year}"
        )));
        assert!(!is_current_or_latest_query("best ai agents in 1999"));
    }

    #[test]
    fn tool_argument_urls_do_not_override_stable_user_intent() {
        assert!(!should_force_live_research_lookup(
            "what is this",
            &serde_json::json!({ "url": "https://example.com/pricing" })
        ));
        assert!(should_force_live_research_lookup(
            "check again https://example.com/pricing",
            &serde_json::json!({ "url": "https://example.com/pricing" })
        ));
    }
}
