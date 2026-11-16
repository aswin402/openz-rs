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
    matches!(tool_name, "web_fetch" | "searchxyz_read_url").then_some(())?;

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
