use crate::tools::routing::{contains_any, contains_phrase_bounded};
use chrono::Datelike;

const CURRENT_OR_LATEST_TERMS: &[&str] = &[
    "latest",
    "current",
    "today",
    "right now",
    "as of now",
    "recent",
    "latest version",
    "current version",
    "new version",
    "new release",
    "latest release",
    "news",
    "what's new",
    "whats new",
];

const REVALIDATION_TERMS: &[&str] = &[
    "again",
    "recheck",
    "re-check",
    "refresh",
    "double check",
    "check again",
    "look again",
    "go and check",
    "from web",
    "browse",
    "live web",
    "live data",
    "actual page",
    "real page",
];

const EXPLICIT_RESEARCH_TERMS: &[&str] = &[
    "research",
    "look up",
    "lookup",
    "search the web",
    "search online",
    "web search",
    "search for",
    "dig into",
    "investigate",
    "analyze this",
    "analyse this",
    "tell me about this",
    "deep dive",
];

const LOCAL_OPERATIONAL_QUERY_TERMS: &[&str] = &[
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
    "write_file",
    "read_file",
    "patch_file",
    "replace_lines",
    "exec_command",
    "list_jobs",
    "schedule_job",
    "remove_job",
    "cargo check",
    "cargo test",
    "cargo build",
    "src/",
    "/tmp/",
    "cron job",
    "scheduled cron",
];

const RESEARCH_FAILURE_RULES: &[(ResearchFailureKind, &[&str])] = &[
    (
        ResearchFailureKind::Captcha,
        &["captcha", "cloudflare", "bot check"],
    ),
    (
        ResearchFailureKind::BrowserDependencyMissing,
        &[
            "geckodriver",
            "chromedriver",
            "failed to start any headless browser",
            "browser dependency",
            "not installed",
        ],
    ),
    (
        ResearchFailureKind::BrowserSessionLost,
        &[
            "receiver is gone",
            "browser session",
            "connection closed before receiving response",
        ],
    ),
    (
        ResearchFailureKind::SearchExhausted,
        &[
            "all search backends",
            "all enabled external web search backends",
            "no search results",
            "search backends exhausted",
        ],
    ),
    (
        ResearchFailureKind::RateLimited,
        &["429", "rate limit", "too many requests"],
    ),
    (
        ResearchFailureKind::ServerUnavailable,
        &["503", "502", "504", "service unavailable", "bad gateway"],
    ),
    (ResearchFailureKind::Timeout, &["timeout", "timed out"]),
    (
        ResearchFailureKind::Network,
        &["dns", "network", "connection", "host"],
    ),
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
    RESEARCH_FAILURE_RULES
        .iter()
        .find_map(|(kind, needles)| contains_any(&lower, needles).then(|| kind.clone()))
        .unwrap_or(ResearchFailureKind::Other)
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
    contains_phrase_bounded(&lower, &current_year)
        || CURRENT_OR_LATEST_TERMS
            .iter()
            .any(|needle| contains_phrase_bounded(&lower, needle))
}

pub fn asks_to_revalidate_saved_research(text: &str) -> bool {
    let lower = text.to_lowercase();
    REVALIDATION_TERMS
        .iter()
        .any(|needle| contains_phrase_bounded(&lower, needle))
}

pub fn is_explicit_research_request(text: &str) -> bool {
    let lower = text.to_lowercase();
    text_has_http_url(text)
        || EXPLICIT_RESEARCH_TERMS
            .iter()
            .any(|needle| contains_phrase_bounded(&lower, needle))
}

fn is_local_operational_query(text: &str) -> bool {
    let lower = text.to_lowercase();
    contains_any(&lower, LOCAL_OPERATIONAL_QUERY_TERMS)
}

pub fn has_live_research_intent(text: &str) -> bool {
    (!is_local_operational_query(text) || text_has_http_url(text))
        && (text_has_http_url(text)
            || is_current_or_latest_query(text)
            || asks_to_revalidate_saved_research(text)
            || is_explicit_research_request(text))
}

pub fn should_force_live_research_lookup(
    user_content: &str,
    _arguments: &serde_json::Value,
) -> bool {
    has_live_research_intent(user_content)
}

#[cfg(test)]
#[path = "research_policy_tests.rs"]
mod tests;
