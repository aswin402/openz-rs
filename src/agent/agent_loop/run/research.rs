//! Research lookup classification and fresh-result reuse policies.

pub(super) fn is_research_lookup_tool(tool_name: &str) -> bool {
    let canonical = crate::tools::canonical_tool_name(tool_name);
    matches!(
        canonical.as_str(),
        "web_search"
            | "web_fetch"
            | "crawl"
            | "crawl_site"
            | "crawl_website"
            | "parallel_research"
            | "searchxyz_search_web"
            | "searchxyz_read_url"
            | "searchxyz_search_and_read"
            | "searchxyz_browser_search"
            | "searchxyz_deep_research"
            | "searchxyz_site_map"
            | "searchxyz_read_github_repo"
            | "social_search"
            | "browser"
    )
}

pub(super) fn direct_research_url(
    tool_name: &str,
    arguments: &serde_json::Value,
) -> Option<String> {
    let canonical = crate::tools::canonical_tool_name(tool_name);
    matches!(canonical.as_str(), "web_fetch" | "searchxyz_read_url").then_some(())?;

    let raw_url = arguments
        .get("url")
        .and_then(|value| value.as_str())
        .or_else(|| {
            let urls = arguments.get("urls").and_then(|value| value.as_array())?;
            (urls.len() == 1)
                .then(|| urls.first().and_then(|value| value.as_str()))
                .flatten()
        })?;
    let mut parsed = reqwest::Url::parse(raw_url).ok()?;
    parsed.set_fragment(None);
    Some(parsed.to_string())
}

/// Parameter-aware deduplication key for direct URL fetches.
/// Allows re-fetching a previously truncated URL with a larger `max_length`,
/// pagination `start_index`, different `format`, or explicit `cache_mode = "bypass"`,
/// while still deduplicating identical redundant fetches across aliases.
pub(super) fn direct_research_fetch_key(
    tool_name: &str,
    arguments: &serde_json::Value,
) -> Option<String> {
    let base_url = direct_research_url(tool_name, arguments)?;
    let max_len = arguments
        .get("max_length")
        .or_else(|| arguments.get("maxLength"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let start_idx = arguments
        .get("start_index")
        .or_else(|| arguments.get("startIndex"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let format = arguments
        .get("format")
        .and_then(|v| v.as_str())
        .unwrap_or("default")
        .trim()
        .to_ascii_lowercase();
    let bypass = arguments
        .get("cache_mode")
        .or_else(|| arguments.get("cacheMode"))
        .and_then(|v| v.as_str())
        .is_some_and(|mode| mode.eq_ignore_ascii_case("bypass"));
    Some(format!(
        "{base_url}|len={max_len}|start={start_idx}|fmt={format}|bypass={bypass}"
    ))
}

pub(super) fn direct_page_research_only(user_content: &str) -> bool {
    let urls = super::super::research_policy::extract_distinct_http_urls(user_content);
    match urls.len() == 1 {
        false => false,
        true => {
            let lower = user_content.to_lowercase();
            let allows_related_sources = [
                "deep",
                "broader",
                "more detail",
                "related",
                "compare",
                "comparison",
                "multiple sources",
                "full research",
                "scrape",
                "scrap",
                "source code",
                "download",
                "assets",
                "asset",
                "run locally",
                "local copy",
                "implement",
                "build",
                "fix",
                "debug",
                "refactor",
                "integrate",
                "autonomous",
            ]
            .iter()
            .any(|term| lower.contains(term));
            !allows_related_sources
        }
    }
}

pub(super) fn user_content_requests_fresh_fetch(user_content: &str) -> bool {
    let lower = user_content.to_lowercase();
    [
        "latest",
        "current",
        "today",
        "now",
        "check again",
        "verify",
        "refresh",
        "recheck",
        "up to date",
        "what's new",
        "whats new",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

pub(super) async fn fresh_research_brief_blocks_lookup(
    user_content: &str,
    tool_name: &str,
    arguments: &serde_json::Value,
) -> bool {
    match !is_research_lookup_tool(tool_name)
        || super::super::research_policy::should_force_live_research_lookup(
            user_content,
            arguments,
        ) {
        true => false,
        false => match crate::tools::shared_memory::search_research_briefs(user_content, 1).await {
            Ok(items) => items.first().is_some_and(|item| {
                item.score >= super::super::build::MIN_MATCH_SCORE && item.freshness == "fresh"
            }),
            Err(err) => {
                tracing::debug!(error = ?err, "fresh research brief lookup gate skipped");
                false
            }
        },
    }
}
