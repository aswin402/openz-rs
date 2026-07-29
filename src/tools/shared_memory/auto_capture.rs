use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{add_source_bookmark, save_research_brief, search_source_bookmarks};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AutoCaptureSummary {
    pub sources_saved: usize,
    pub brief_saved: bool,
    pub topic: String,
}

#[derive(Debug, Clone)]
struct SourceCandidate {
    url: String,
    label: String,
    summary: String,
    kind: String,
    trust: f64,
}

fn url_regex() -> &'static Regex {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"https?://[^\s\]\)>'\"}]+"#).unwrap())
}

fn is_research_tool(tool_name: &str) -> bool {
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
            | "searchxyz_deep_research"
            | "searchxyz_site_map"
            | "searchxyz_read_github_repo"
            | "social_search"
    )
}

fn first_str<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(|v| v.as_str()))
}

pub fn canonical_research_topic(raw: &str) -> String {
    let mut text = raw.trim().to_lowercase();
    let parse_target = url_regex()
        .find(&text)
        .map(|m| m.as_str())
        .unwrap_or(text.as_str());
    if let Ok(parsed) = reqwest::Url::parse(parse_target) {
        if let Some(host) = parsed.host_str() {
            let host = host.trim_start_matches("www.");
            let path_parts = parsed
                .path()
                .trim_matches('/')
                .split('/')
                .filter(|part| !part.is_empty())
                .map(|part| part.trim().trim_end_matches(".html"))
                .filter(|part| !part.is_empty())
                .take(3)
                .collect::<Vec<_>>();
            text = if host == "github.com" || host == "raw.githubusercontent.com" {
                path_parts.into_iter().take(2).collect::<Vec<_>>().join("/")
            } else if path_parts.is_empty() {
                host.to_string()
            } else {
                format!("{}/{}", host, path_parts.join("/"))
            };
        }
    }
    text = text
        .replace("%20", " ")
        .replace("+", " ")
        .replace(" 20", " ");
    for _ in 0..3 {
        let before = text.clone();
        for prefix in [
            "hey ",
            "hi ",
            "hello ",
            "yo ",
            "ok ",
            "okay ",
            "so ",
            "now ",
            "can you ",
            "could you ",
            "please ",
            "what is ",
            "whats ",
            "what's ",
            "tell me about ",
            "research about ",
            "research ",
            "compare ",
        ] {
            if let Some(rest) = text.strip_prefix(prefix) {
                text = rest.trim().to_string();
            }
        }
        if text == before {
            break;
        }
    }
    let stop = [
        "please", "and", "with", "from", "latest", "current", "about", "tell", "me",
    ];
    let words = text
        .split(|c: char| !c.is_alphanumeric() && c != '/' && c != '.' && c != '-')
        .filter(|w| !w.is_empty() && !stop.contains(w))
        .take(8)
        .collect::<Vec<_>>();
    if words.is_empty() {
        raw.trim().chars().take(160).collect()
    } else {
        words.join(" ").chars().take(160).collect()
    }
}

fn has_explicit_research_directive(normalized: &str) -> bool {
    [
        "research",
        "deep dive",
        "investigate",
        "analyze this",
        "analyse this",
        "compare",
        " vs ",
        " versus ",
        "difference between",
        "tell me about this",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
}

fn is_acquisition_or_display_user_content(text: &str) -> bool {
    let normalized = text
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let has_acquisition_action = [
        "download",
        "save it",
        "save that",
        "show it",
        "show that",
        "open it",
        "open that",
        "play ",
        "set wallpaper",
        "make wallpaper",
        "install",
    ]
    .iter()
    .any(|needle| normalized.contains(needle));
    let has_local_artifact_target = [
        "wallpaper",
        "image",
        "picture",
        "photo",
        "video",
        "song",
        "music",
        "desktop",
        "browser",
        "viewer",
        "file",
        "app",
    ]
    .iter()
    .any(|needle| normalized.contains(needle));

    has_acquisition_action
        && has_local_artifact_target
        && !has_explicit_research_directive(&normalized)
}

fn is_research_worthy_user_content(text: &str) -> bool {
    let normalized = text
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if normalized.trim().is_empty() {
        return false;
    }
    if url_regex().is_match(&normalized) {
        return true;
    }
    let explicit_research = [
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
        "compare",
        " vs ",
        " versus ",
        "difference between",
    ]
    .iter()
    .any(|needle| normalized.contains(needle));
    if explicit_research {
        return true;
    }
    let definition_query = [
        "what is ",
        "whats ",
        "what's ",
        "what are ",
        "who is ",
        "tell me about ",
    ]
    .iter()
    .any(|marker| normalized.starts_with(marker) || normalized.contains(&format!(" {marker}")));
    definition_query && !canonical_research_topic(&normalized).trim().is_empty()
}

fn is_refresh_only_user_content(text: &str) -> bool {
    let normalized = text
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if !url_regex().is_match(&normalized) {
        return false;
    }
    let asks_refresh = [
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
    ]
    .iter()
    .any(|needle| normalized.contains(needle));
    let asks_research = [
        "research",
        "deep dive",
        "tell me about",
        "compare",
        "analyze",
        "analyse",
        "investigate",
    ]
    .iter()
    .any(|needle| normalized.contains(needle));
    asks_refresh && !asks_research
}

fn topic_from(tool_name: &str, args: &Value, user_content: &str) -> String {
    if let Some(topic) = first_str(
        args,
        &["query", "topic", "goal", "url", "repo", "repository"],
    ) {
        return canonical_research_topic(topic);
    }
    if tool_name == "parallel_research" {
        if let Some(tasks) = args.get("tasks").and_then(|v| v.as_array()) {
            let joined = tasks
                .iter()
                .filter_map(|task| task.get("goal").and_then(|v| v.as_str()))
                .take(3)
                .collect::<Vec<_>>()
                .join("; ");
            if !joined.trim().is_empty() {
                return canonical_research_topic(&joined);
            }
        }
    }
    canonical_research_topic(user_content)
}

fn kind_for_url(url: &str) -> (&'static str, f64) {
    let lower = url.to_lowercase();
    if lower.contains("github.com") || lower.contains("gitlab.com") {
        ("repo", 0.85)
    } else if lower.contains("/docs") || lower.contains("docs.") || lower.contains("documentation")
    {
        ("docs", 0.8)
    } else if lower.contains("twitter.com")
        || lower.contains("x.com")
        || lower.contains("reddit.com")
        || lower.contains("youtube.com")
        || lower.contains("linkedin.com")
    {
        ("social", 0.55)
    } else {
        ("website", 0.65)
    }
}

fn label_for_url(url: &str) -> String {
    reqwest::Url::parse(url)
        .ok()
        .and_then(|parsed| {
            let host = parsed.host_str()?.trim_start_matches("www.");
            let parts = parsed
                .path_segments()
                .map(|segments| segments.collect::<Vec<_>>())
                .unwrap_or_default();
            if host == "github.com" && parts.len() >= 2 {
                let repo = format!("{}/{}", parts[0], parts[1]);
                return match parts.get(2).copied() {
                    Some("issues") => parts
                        .get(3)
                        .map(|num| format!("{repo} issue #{num}"))
                        .or_else(|| Some(format!("{repo} issues"))),
                    Some("pull") => parts
                        .get(3)
                        .map(|num| format!("{repo} PR #{num}"))
                        .or_else(|| Some(format!("{repo} pull requests"))),
                    Some("blob" | "tree") => {
                        if parts.len() > 4 {
                            Some(format!("{repo} {}", parts[4..].join("/")))
                        } else {
                            Some(repo)
                        }
                    }
                    _ => Some(repo),
                };
            }
            if host == "raw.githubusercontent.com" && parts.len() >= 2 {
                let repo = format!("{}/{}", parts[0], parts[1]);
                if parts.len() > 3 {
                    return Some(format!("{repo} {}", parts[3..].join("/")));
                }
                return Some(repo);
            }
            let path = parsed.path().trim_matches('/');
            if path.is_empty() {
                Some(host.to_string())
            } else {
                let last = path
                    .rsplit('/')
                    .next()
                    .unwrap_or(path)
                    .replace(['-', '_'], " ");
                Some(format!("{} - {}", host, last))
            }
        })
        .unwrap_or_else(|| url.chars().take(80).collect())
}

fn text_excerpt(text: &str, max_chars: usize) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(max_chars)
        .collect()
}

fn clean_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_navigation_noise(text: &str) -> bool {
    let lower = text.to_lowercase();
    let nav_terms = [
        "overview",
        "getting started",
        "troubleshooting",
        "pricing",
        "billing",
        "legal",
        "terms",
        "discord",
        "github",
        "website",
        "twitter",
        "x/twitter",
        "features",
        "docs",
        "more",
    ];
    let hits = nav_terms
        .iter()
        .filter(|term| lower.contains(**term))
        .count();
    hits >= 5 && !lower.contains(" is ") && !lower.contains(" built")
}

fn trim_leading_noise_to_definition(text: &str) -> String {
    let cleaned = clean_text(text);
    let lower = cleaned.to_lowercase();
    for marker in [" is ", " are ", " was ", " built "] {
        if let Some(idx) = lower.find(marker) {
            let prefix = &cleaned[..idx];
            let start = prefix.rfind(' ').map(|pos| pos + 1).unwrap_or(0);
            let trimmed = cleaned[start..].trim();
            if trimmed.chars().filter(|c| c.is_alphabetic()).count() >= 40 {
                return trimmed.to_string();
            }
        }
    }
    cleaned
}

fn sentence_chunks(text: &str) -> Vec<String> {
    trim_leading_noise_to_definition(text)
        .split(['.', '!', '?'])
        .map(str::trim)
        .filter(|s| s.chars().filter(|c| c.is_alphabetic()).count() >= 40)
        .map(ToString::to_string)
        .collect()
}

fn signal_score(text: &str) -> i32 {
    let lower = text.to_lowercase();
    let mut score = 0;
    for term in [
        " is ",
        " built",
        " open-source",
        " local-first",
        " memory",
        " workflow",
        " agent",
        " research",
        " orchestrat",
        " privacy",
        " rust",
        " typescript",
    ] {
        if lower.contains(term) {
            score += 2;
        }
    }
    if is_navigation_noise(text) {
        score -= 10;
    }
    score
}

fn signal_excerpt(text: &str, max_chars: usize) -> String {
    let mut chunks = sentence_chunks(text);
    chunks.sort_by(|a, b| signal_score(b).cmp(&signal_score(a)));
    let mut out = Vec::new();
    for chunk in chunks.into_iter().filter(|c| signal_score(c) > 0) {
        if out.iter().any(|existing: &String| existing == &chunk) {
            continue;
        }
        let candidate = if out.is_empty() {
            chunk.clone()
        } else {
            format!("{}. {}", out.join(". "), chunk)
        };
        if candidate.chars().count() > max_chars {
            break;
        }
        out.push(chunk);
        if out.len() >= 3 {
            break;
        }
    }
    if out.is_empty() {
        text_excerpt(text, max_chars)
    } else {
        out.join(". ")
    }
}

fn object_str<'a>(obj: &'a serde_json::Map<String, Value>, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| obj.get(*key).and_then(|v| v.as_str()))
}

fn candidate_from_object(obj: &serde_json::Map<String, Value>) -> Option<SourceCandidate> {
    let url = object_str(obj, &["url", "uri", "link", "href", "source"])?;
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return None;
    }
    let label = object_str(obj, &["title", "name", "label"])
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| label_for_url(url));
    let summary = object_str(
        obj,
        &["snippet", "summary", "content", "text", "description"],
    )
    .map(|s| signal_excerpt(s, 360))
    .unwrap_or_default();
    let (kind, trust) = kind_for_url(url);
    Some(SourceCandidate {
        url: url.to_string(),
        label,
        summary,
        kind: kind.to_string(),
        trust,
    })
}

fn collect_candidates(value: &Value, out: &mut Vec<SourceCandidate>) {
    match value {
        Value::Object(map) => {
            if let Some(candidate) = candidate_from_object(map) {
                out.push(candidate);
            }
            for child in map.values() {
                collect_candidates(child, out);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_candidates(item, out);
            }
        }
        Value::String(text) => {
            for mat in url_regex().find_iter(text) {
                let url = mat
                    .as_str()
                    .trim_end_matches(['.', ',', ';', ':'])
                    .to_string();
                let (kind, trust) = kind_for_url(&url);
                out.push(SourceCandidate {
                    label: label_for_url(&url),
                    summary: String::new(),
                    kind: kind.to_string(),
                    trust,
                    url,
                });
            }
        }
        _ => {}
    }
}

fn is_repo_url(url: &str) -> bool {
    reqwest::Url::parse(url)
        .ok()
        .and_then(|parsed| {
            parsed
                .host_str()
                .map(|host| host.trim_start_matches("www.").to_string())
        })
        .map(|host| {
            matches!(
                host.as_str(),
                "github.com" | "raw.githubusercontent.com" | "gitlab.com"
            )
        })
        .unwrap_or(false)
}

fn repo_topic_from_candidates(candidates: &[SourceCandidate]) -> Option<String> {
    candidates.iter().find_map(|candidate| {
        if !is_repo_url(&candidate.url) {
            return None;
        }
        let topic = canonical_research_topic(&candidate.url);
        if topic.contains('/') {
            Some(topic)
        } else {
            None
        }
    })
}

async fn existing_repo_topic_for(query: &str) -> Option<String> {
    let matches = search_source_bookmarks(query, 3).await.ok()?;
    matches.into_iter().find_map(|source| {
        if source.kind.trim().to_lowercase() != "repo" && !is_repo_url(&source.uri) {
            return None;
        }
        let topic = canonical_research_topic(&source.uri);
        if topic.contains('/') {
            Some(topic)
        } else {
            None
        }
    })
}

fn dedupe_candidates(candidates: Vec<SourceCandidate>) -> Vec<SourceCandidate> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for candidate in candidates {
        if seen.insert(candidate.url.clone()) {
            out.push(candidate);
        }
        if out.len() >= 5 {
            break;
        }
    }
    out
}

fn result_summary(result: &Value) -> String {
    let mut parts = Vec::new();
    fn collect(value: &Value, parts: &mut Vec<String>) {
        match value {
            Value::Object(map) => {
                let title = object_str(map, &["title", "name", "label"]).unwrap_or("");
                let snippet = object_str(
                    map,
                    &["snippet", "summary", "description", "content", "text"],
                )
                .unwrap_or("");
                let url = object_str(map, &["url", "uri", "link", "href"]).unwrap_or("");
                if !title.is_empty() || !snippet.is_empty() {
                    let mut line = String::new();
                    if !title.is_empty() {
                        line.push_str(title.trim());
                    }
                    if !snippet.is_empty() {
                        if !line.is_empty() {
                            line.push_str(": ");
                        }
                        line.push_str(&signal_excerpt(snippet, 420));
                    }
                    if !url.is_empty() {
                        line.push_str(" (");
                        line.push_str(url);
                        line.push(')');
                    }
                    parts.push(line);
                }
                for child in map.values() {
                    collect(child, parts);
                }
            }
            Value::Array(items) => items.iter().for_each(|item| collect(item, parts)),
            Value::String(text) => {
                if parts.is_empty() {
                    parts.push(signal_excerpt(text, 900));
                }
            }
            _ => {}
        }
    }
    collect(result, &mut parts);
    if parts.is_empty() {
        signal_excerpt(&serde_json::to_string(result).unwrap_or_default(), 900)
    } else {
        text_excerpt(
            &parts.into_iter().take(8).collect::<Vec<_>>().join("; "),
            1200,
        )
    }
}

pub async fn auto_capture_research_memory(
    tool_name: &str,
    arguments: &Value,
    result: &Value,
    user_content: &str,
) -> Result<Option<AutoCaptureSummary>> {
    if !is_research_tool(tool_name)
        || result.get("error").is_some()
        || result.get("status").and_then(|v| v.as_str()) == Some("skipped")
    {
        return Ok(None);
    }

    if is_acquisition_or_display_user_content(user_content) {
        return Ok(None);
    }

    let user_research_worthy = is_research_worthy_user_content(user_content);
    let has_research_target_arg = first_str(
        arguments,
        &["query", "topic", "goal", "url", "repo", "repository"],
    )
    .map(is_research_worthy_user_content)
    .unwrap_or(false);
    if !user_research_worthy && !(user_content.trim().is_empty() && has_research_target_arg) {
        return Ok(None);
    }
    if is_refresh_only_user_content(user_content) {
        return Ok(None);
    }

    let mut candidates = Vec::new();
    collect_candidates(arguments, &mut candidates);
    collect_candidates(result, &mut candidates);
    let candidates = dedupe_candidates(candidates);
    let candidate_repo_topic = repo_topic_from_candidates(&candidates);

    // Derive topic consistently from user_content so all tool calls in the
    // same turn share the same canonical topic (avoiding duplicate briefs).
    // If a vague follow-up like "what is dox" is attached to a concrete repo/docs
    // URL in tool args or result candidates, preserve the canonical URL topic
    // (e.g. agent0ai/dox).
    let topic = if user_content.trim().is_empty() {
        topic_from(tool_name, arguments, user_content)
    } else {
        let user_topic = canonical_research_topic(user_content);
        let arg_topic = topic_from(tool_name, arguments, "");
        let existing_repo_topic = if !user_topic.contains('/') {
            existing_repo_topic_for(&user_topic).await
        } else {
            None
        };
        if !user_topic.contains('/') {
            candidate_repo_topic
                .or(existing_repo_topic)
                .or_else(|| arg_topic.contains('/').then_some(arg_topic))
                .unwrap_or(user_topic)
        } else {
            user_topic
        }
    };
    if topic.trim().is_empty() {
        return Ok(None);
    }

    let mut source_ids = Vec::new();
    for candidate in candidates {
        if let Ok(saved) = add_source_bookmark(
            &candidate.label,
            &candidate.kind,
            &candidate.url,
            vec![topic.clone()],
            &candidate.summary,
            candidate.trust,
            0,
        )
        .await
        {
            source_ids.push(saved.id);
        }
    }

    let summary = result_summary(result);
    let brief_saved = if !summary.trim().is_empty() {
        save_research_brief(&topic, &summary, source_ids.clone(), 0.65, 0)
            .await
            .is_ok()
    } else {
        false
    };

    if source_ids.is_empty() && !brief_saved {
        return Ok(None);
    }

    Ok(Some(AutoCaptureSummary {
        sources_saved: source_ids.len(),
        brief_saved,
        topic,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_labels_are_human_readable() {
        assert_eq!(
            label_for_url("https://github.com/tonbistudio/turboquant-pytorch"),
            "tonbistudio/turboquant-pytorch"
        );
        assert_eq!(
            label_for_url("https://github.com/tonbistudio/turboquant-pytorch/issues/6"),
            "tonbistudio/turboquant-pytorch issue #6"
        );
        assert_eq!(
            label_for_url("https://github.com/barbel-bb/turboquant-cache/pull/12"),
            "barbel-bb/turboquant-cache PR #12"
        );
    }

    #[test]
    fn result_summary_prefers_signal_over_navigation_noise() {
        let result = serde_json::json!({
            "url": "https://github.com/example/openhuman",
            "title": "OpenHuman",
            "content": "OpenHuman GitHub Website Discord More English Overview Getting Started Troubleshooting Features Realtime Mascot Memory Third-party Integrations The Orchestrator Workflows Pricing Billing Legal Terms OpenHuman is a local-first personal AI agent that builds persistent memory, coordinates workflows, and performs deep research across your files and web sources. It stores user context locally and keeps automation approval-gated."
        });

        let summary = result_summary(&result);
        assert!(summary.contains("OpenHuman is a local-first personal AI agent"));
        assert!(summary.contains("persistent memory"));
        assert!(!summary.contains("GitHub Website Discord More English Overview"));
        assert!(!summary.contains("Pricing Billing Legal Terms"));
    }

    #[tokio::test]
    async fn auto_capture_saves_sources_and_brief_from_search_results() {
        let marker = uuid::Uuid::new_v4().to_string();
        let topic = format!("Hermes Agent {}", marker);
        let result = serde_json::json!([
            {
                "title": format!("Hermes Agent docs {}", marker),
                "url": format!("https://hermes-agent.nousresearch.com/docs/{}", marker),
                "snippet": "Official Hermes Agent documentation"
            }
        ]);
        let summary = auto_capture_research_memory(
            "web_search",
            &serde_json::json!({"query": topic}),
            &result,
            "what is hermes agent",
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(summary.sources_saved, 1);
        assert!(summary.brief_saved);
        assert_eq!(
            canonical_research_topic("https://github.com/mem0ai/mem0?utm_source=chatgpt.com"),
            "mem0ai/mem0"
        );
        assert_eq!(
            canonical_research_topic("https://sakana.ai/fugu/"),
            "sakana.ai/fugu"
        );
        assert_eq!(
            canonical_research_topic("https://www.duix.com/pricing?utm_source=test"),
            "duix.com/pricing"
        );
        assert_eq!(canonical_research_topic("what is mem0"), "mem0");
        assert_eq!(canonical_research_topic("hey whats hermes"), "hermes");
        assert_eq!(
            canonical_research_topic("ok now tell me about mem0"),
            "mem0"
        );
        assert_eq!(
            canonical_research_topic("https://github.com/tinyhumansai/openhuman research about this and tell me about this"),
            "tinyhumansai/openhuman"
        );
        let matches = crate::tools::shared_memory::search_source_bookmarks(&marker, 5)
            .await
            .unwrap();
        assert!(matches.iter().any(|m| m.uri.contains(&marker)));
        for item in matches.into_iter().filter(|m| m.uri.contains(&marker)) {
            let _ = crate::tools::shared_memory::delete_source(&item.id).await;
        }
        let _ =
            crate::tools::shared_memory::delete_research_brief(&format!("Hermes Agent {}", marker))
                .await;
    }
    #[tokio::test]
    async fn auto_capture_skips_refresh_only_url_checks() {
        let marker = uuid::Uuid::new_v4().to_string();
        let url = format!("https://github.com/example/recheck-{marker}");
        let result = serde_json::json!({
            "url": url,
            "content": "Example project is an open-source research repository with useful implementation notes."
        });

        let summary = auto_capture_research_memory(
            "web_fetch",
            &serde_json::json!({"url": format!("https://github.com/example/recheck-{marker}")}),
            &result,
            &format!("check again https://github.com/example/recheck-{marker}"),
        )
        .await
        .unwrap();

        assert!(summary.is_none());
    }

    #[tokio::test]
    async fn auto_capture_skips_download_and_display_workflows() {
        let marker = uuid::Uuid::new_v4().to_string();
        let url = format!("https://example.com/wallpaper-{marker}.jpg");
        let result = serde_json::json!({
            "title": format!("Wallpaper {marker}"),
            "url": url,
            "content": "A dark desktop wallpaper download result."
        });

        let summary = auto_capture_research_memory(
            "web_search",
            &serde_json::json!({"query": format!("hollow knight dark wallpaper 4k {marker}")}),
            &result,
            &format!("find a good platform to download a dark wallpaper image and show it to me {marker}"),
        )
        .await
        .unwrap();

        assert!(summary.is_none());
        let matches = crate::tools::shared_memory::search_research_briefs(&marker, 5)
            .await
            .unwrap();
        assert!(matches.iter().all(|item| !item.topic.contains(&marker)));
    }

    #[tokio::test]
    async fn auto_capture_skips_non_research_debug_turns() {
        let marker = uuid::Uuid::new_v4().to_string();
        let url = format!("https://example.com/debug-{marker}");
        let result = serde_json::json!({
            "url": url,
            "content": "Example.com is a documentation page used for a quick runtime check."
        });

        let summary = auto_capture_research_memory(
            "web_fetch",
            &serde_json::json!({"url": format!("https://example.com/debug-{marker}")}),
            &result,
            "why u needed source code",
        )
        .await
        .unwrap();

        assert!(summary.is_none());
        let matches = crate::tools::shared_memory::search_research_briefs(&marker, 5)
            .await
            .unwrap();
        assert!(matches.iter().all(|item| !item.topic.contains(&marker)));
    }

    #[tokio::test]
    async fn auto_capture_ignores_skipped_saved_brief_results() {
        let marker = uuid::Uuid::new_v4().to_string();
        let result = serde_json::json!({
            "status": "skipped",
            "reason": "Skipped web/search lookup: a fresh saved research brief already matches this non-latest query."
        });

        let summary = auto_capture_research_memory(
            "web_fetch",
            &serde_json::json!({"url": format!("https://github.com/example/{marker}")}),
            &result,
            &format!("hey whats {marker}"),
        )
        .await
        .unwrap();

        assert!(summary.is_none());
        let matches = crate::tools::shared_memory::search_research_briefs(&marker, 5)
            .await
            .unwrap();
        assert!(!matches.iter().any(|item| item.topic.contains(&marker)));
    }

    #[tokio::test]
    async fn auto_capture_repo_brief_uses_week_ttl() {
        let marker = uuid::Uuid::new_v4().to_string();
        let url = format!("https://github.com/example/dox-{marker}");
        let user_content = format!("{url} hey research about this and tell me about this");
        let result = serde_json::json!({
            "title": format!("DOX {marker}"),
            "url": url,
            "content": "DOX is a self-documenting AGENTS.md framework for AI coding agents."
        });

        let summary = auto_capture_research_memory(
            "web_fetch",
            &serde_json::json!({"url": url}),
            &result,
            &user_content,
        )
        .await
        .unwrap()
        .unwrap();

        assert_eq!(summary.topic, format!("example/dox-{marker}"));
        let briefs = crate::tools::shared_memory::search_research_briefs(&summary.topic, 1)
            .await
            .unwrap();
        assert_eq!(briefs[0].topic, summary.topic);
        assert!(briefs[0].stale_after_secs >= 604_800);

        let sources = crate::tools::shared_memory::search_source_bookmarks(&marker, 5)
            .await
            .unwrap();
        for source in sources.into_iter().filter(|s| s.uri.contains(&marker)) {
            let _ = crate::tools::shared_memory::delete_source(&source.id).await;
        }
        let _ = crate::tools::shared_memory::delete_research_brief(&summary.topic).await;
    }

    #[tokio::test]
    async fn auto_capture_uses_canonical_website_url_topic() {
        let marker = uuid::Uuid::new_v4().to_string();
        let url = format!("https://sakana.ai/fugu-{marker}/");
        let result = serde_json::json!({
            "title": format!("Sakana Fugu {marker}"),
            "url": url,
            "content": "Sakana Fugu is a multi-agent orchestration model exposed through an OpenAI-compatible API."
        });

        let summary = auto_capture_research_memory(
            "web_fetch",
            &serde_json::json!({"url": format!("https://sakana.ai/fugu-{marker}/")}),
            &result,
            &format!("hey research about this https://sakana.ai/fugu-{marker}/"),
        )
        .await
        .unwrap()
        .unwrap();

        assert_eq!(summary.topic, format!("sakana.ai/fugu-{marker}"));
        let matches = crate::tools::shared_memory::search_research_briefs(
            &format!("what is sakana fugu-{marker}"),
            5,
        )
        .await
        .unwrap();
        assert!(matches.iter().any(|item| item.topic == summary.topic));

        let sources = crate::tools::shared_memory::search_source_bookmarks(&marker, 5)
            .await
            .unwrap();
        for source in sources.into_iter().filter(|s| s.uri.contains(&marker)) {
            let _ = crate::tools::shared_memory::delete_source(&source.id).await;
        }
        let _ = crate::tools::shared_memory::delete_research_brief(&summary.topic).await;
    }

    #[tokio::test]
    async fn auto_capture_prefers_existing_repo_topic_for_short_alias() {
        let marker = uuid::Uuid::new_v4().to_string();
        let repo_url = format!("https://github.com/NousResearch/hermes-agent-{marker}");
        let source = crate::tools::shared_memory::add_source_bookmark(
            &format!("NousResearch/hermes-agent-{marker}"),
            "repo",
            &repo_url,
            vec![format!("hermes-{marker}")],
            "Official Hermes Agent repository",
            0.95,
            604800,
        )
        .await
        .unwrap();
        let result = serde_json::json!({
            "title": format!("Hermes Agent docs {marker}"),
            "url": format!("https://hermes-agent.nousresearch.com/docs/{marker}"),
            "content": "Hermes Agent is a self-improving AI agent framework with tools, skills, and messaging channels."
        });

        let summary = auto_capture_research_memory(
            "web_fetch",
            &serde_json::json!({"url": format!("https://hermes-agent.nousresearch.com/docs/{marker}")}),
            &result,
            &format!("so whats hermes-{marker}"),
        )
        .await
        .unwrap()
        .unwrap();

        assert_eq!(summary.topic, format!("nousresearch/hermes-agent-{marker}"));
        let _ = crate::tools::shared_memory::delete_source(&source.id).await;
        let _ = crate::tools::shared_memory::delete_research_brief(&summary.topic).await;
    }

    #[tokio::test]
    async fn auto_capture_uses_repo_topic_from_result_url_for_simple_followup() {
        let marker = uuid::Uuid::new_v4().to_string();
        let repo = format!("openhuman-{marker}");
        let repo_url = format!("https://github.com/tinyhumansai/{repo}");
        let result = serde_json::json!({
            "title": format!("OpenHuman {marker}"),
            "url": repo_url,
            "content": "OpenHuman is a local-first personal AI agent platform with memory, workflows, integrations, and research tools."
        });

        let summary = auto_capture_research_memory(
            "web_fetch",
            &serde_json::json!({"query": format!("what is openhuman-{marker}")}),
            &result,
            &format!("hey whats openhuman-{marker}"),
        )
        .await
        .unwrap()
        .unwrap();

        assert_eq!(summary.topic, format!("tinyhumansai/{repo}"));
        let generic = crate::tools::shared_memory::search_research_briefs(
            &format!("what is openhuman-{marker}"),
            1,
        )
        .await
        .unwrap();
        assert_eq!(generic[0].topic, summary.topic);

        let sources = crate::tools::shared_memory::search_source_bookmarks(&marker, 5)
            .await
            .unwrap();
        for source in sources.into_iter().filter(|s| s.uri.contains(&marker)) {
            let _ = crate::tools::shared_memory::delete_source(&source.id).await;
        }
        let _ = crate::tools::shared_memory::delete_research_brief(&summary.topic).await;
        let _ = crate::tools::shared_memory::delete_research_brief(&format!("openhuman-{marker}"))
            .await;
    }

    #[tokio::test]
    async fn auto_capture_followup_with_repo_url_keeps_canonical_repo_topic() {
        let marker = uuid::Uuid::new_v4().to_string();
        let url = format!("https://github.com/example/dox-{marker}");
        let result = serde_json::json!({
            "title": format!("DOX {marker}"),
            "url": url,
            "content": "DOX is a self-documenting AGENTS.md framework for AI coding agents."
        });

        let summary = auto_capture_research_memory(
            "web_fetch",
            &serde_json::json!({"url": url}),
            &result,
            &format!("hey whats dox-{marker}"),
        )
        .await
        .unwrap()
        .unwrap();

        assert_eq!(summary.topic, format!("example/dox-{marker}"));
        let generic = crate::tools::shared_memory::search_research_briefs(
            &format!("what is dox-{marker}"),
            1,
        )
        .await
        .unwrap();
        assert_eq!(generic[0].topic, summary.topic);

        let sources = crate::tools::shared_memory::search_source_bookmarks(&marker, 5)
            .await
            .unwrap();
        for source in sources.into_iter().filter(|s| s.uri.contains(&marker)) {
            let _ = crate::tools::shared_memory::delete_source(&source.id).await;
        }
        let _ = crate::tools::shared_memory::delete_research_brief(&summary.topic).await;
        let _ = crate::tools::shared_memory::delete_research_brief(&format!("dox-{marker}")).await;
    }
}
