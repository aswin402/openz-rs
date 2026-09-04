//! Research lookup classification and fresh-result reuse policies.

pub(super) fn is_research_lookup_tool(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "web_search"
            | "web_fetch"
            | "crawl"
            | "crawl_site"
            | "parallel_research"
            | "searchxyz_search_web"
            | "searchxyz_read_url"
            | "searchxyz_search_and_read"
            | "searchxyz_browser_search"
            | "searchxyz_deep_research"
            | "searchxyz_site_map"
            | "searchxyz_read_github_repo"
            | "social_search"
            | "obscura_browser"
            | "firefox_browser"
            | "gsd_browser"
    )
}

pub(super) fn direct_research_url(
    tool_name: &str,
    arguments: &serde_json::Value,
) -> Option<String> {
    if !matches!(tool_name, "web_fetch" | "searchxyz_read_url") {
        return None;
    }

    let raw_url = arguments.get("url").and_then(|value| value.as_str())?;
    let mut parsed = reqwest::Url::parse(raw_url).ok()?;
    // URL fragments select a page section but do not change the fetched document.
    parsed.set_fragment(None);
    Some(parsed.to_string())
}

pub(super) fn direct_page_research_only(user_content: &str) -> bool {
    if !super::super::research_policy::text_has_http_url(user_content) {
        return false;
    }

    let lower = user_content.to_lowercase();
    // Extraction and download tasks legitimately follow embeds and asset URLs.
    // Ordinary "research this URL" requests remain page-local by default.
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
    if !is_research_lookup_tool(tool_name)
        || super::super::research_policy::should_force_live_research_lookup(
            user_content,
            arguments,
        )
    {
        return false;
    }
    match crate::tools::shared_memory::search_research_briefs(user_content, 1).await {
        Ok(items) => items.first().is_some_and(|item| {
            item.score >= super::super::build::MIN_MATCH_SCORE && item.freshness == "fresh"
        }),
        Err(err) => {
            tracing::debug!(error = ?err, "fresh research brief lookup gate skipped");
            false
        }
    }
}
