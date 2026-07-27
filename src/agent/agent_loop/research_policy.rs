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

pub fn has_live_research_intent(text: &str) -> bool {
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
