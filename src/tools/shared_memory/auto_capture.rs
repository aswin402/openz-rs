use anyhow::Result;
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

fn is_url_terminator(c: char) -> bool {
    c.is_whitespace() || matches!(c, ']' | ')' | '>' | '\'' | '"' | '}')
}

fn extract_urls(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        let next_http = rest.find("http://");
        let next_https = rest.find("https://");
        let start = match (next_http, next_https) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        };
        match start {
            Some(idx) => {
                let after_start = &rest[idx..];
                let scheme_len = match after_start.starts_with("https://") {
                    true => 8,
                    false => 7,
                };
                let body = &after_start[scheme_len..];
                let body_len = body.find(is_url_terminator).unwrap_or(body.len());
                let total_len = scheme_len + body_len;
                match body_len > 0 {
                    true => out.push(&after_start[..total_len]),
                    false => {}
                }
                rest = &after_start[total_len.max(scheme_len)..];
            }
            None => break,
        }
    }
    out
}

fn first_url(text: &str) -> Option<&str> {
    extract_urls(text).into_iter().next()
}

fn has_url(text: &str) -> bool {
    !extract_urls(text).is_empty()
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
    let parse_target = first_url(&text).unwrap_or(text.as_str());
    let parsed_host = reqwest::Url::parse(parse_target).ok().and_then(|parsed| {
        let host = parsed.host_str()?.trim_start_matches("www.").to_string();
        Some((host, parsed))
    });
    match parsed_host {
        Some((host, parsed)) => {
            let path_parts = parsed
                .path()
                .trim_matches('/')
                .split('/')
                .filter(|part| !part.is_empty())
                .map(|part| part.trim().trim_end_matches(".html"))
                .filter(|part| !part.is_empty())
                .take(3)
                .collect::<Vec<_>>();
            let fragment_part = parsed
                .fragment()
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .map(|part| part.trim_end_matches(".html"));
            text = match (
                host == "github.com" || host == "raw.githubusercontent.com",
                path_parts.is_empty(),
            ) {
                (true, _) => path_parts.into_iter().take(2).collect::<Vec<_>>().join("/"),
                (false, true) => fragment_part
                    .map(|fragment| format!("{host}/{fragment}"))
                    .unwrap_or(host),
                (false, false) => format!("{}/{}", host, path_parts.join("/")),
            };
        }
        None => {}
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
            "call web_search with query ",
            "call web search with query ",
            "call web_search query ",
            "call web search query ",
            "web_search with query ",
            "web search with query ",
            "web_search query ",
            "web search query ",
            "search query ",
            "query ",
            "search ",
            "compare ",
        ] {
            match text.strip_prefix(prefix) {
                Some(rest) => text = rest.trim().to_string(),
                None => {}
            }
        }
        match text == before {
            true => break,
            false => {}
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
    match (words.is_empty(), words.contains(&"rust")) {
        (true, _) => raw.trim().chars().take(160).collect(),
        (false, true) => {
            const KNOWN_RUST_CRATES: &[&str] = &[
                "tokio",
                "axum",
                "hyper",
                "tonic",
                "serde",
                "reqwest",
                "clap",
                "tracing",
                "rusqlite",
                "tantivy",
                "crossterm",
                "ratatui",
                "bevy",
            ];
            KNOWN_RUST_CRATES
                .iter()
                .find(|crate_name| words.iter().any(|word| *word == **crate_name))
                .map(|crate_name| format!("rust/{crate_name}"))
                .unwrap_or_else(|| words.join(" ").chars().take(160).collect())
        }
        (false, false) => words.join(" ").chars().take(160).collect(),
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
    match normalized.trim().is_empty() {
        true => return false,
        false => {}
    }
    match has_url(&normalized) {
        true => return true,
        false => {}
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
    match explicit_research {
        true => return true,
        false => {}
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
    match has_url(&normalized) {
        false => return false,
        true => {}
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
    match first_str(
        args,
        &["query", "topic", "goal", "url", "repo", "repository"],
    ) {
        Some(topic) => return canonical_research_topic(topic),
        None => {}
    }
    match tool_name == "parallel_research" {
        true => match args.get("tasks").and_then(|v| v.as_array()) {
            Some(tasks) => {
                let joined = tasks
                    .iter()
                    .filter_map(|task| task.get("goal").and_then(|v| v.as_str()))
                    .take(3)
                    .collect::<Vec<_>>()
                    .join("; ");
                match joined.trim().is_empty() {
                    false => return canonical_research_topic(&joined),
                    true => {}
                }
            }
            None => {}
        },
        false => {}
    }
    canonical_research_topic(user_content)
}

fn kind_for_url(url: &str) -> (&'static str, f64) {
    let lower = url.to_lowercase();
    let is_repo = lower.contains("github.com") || lower.contains("gitlab.com");
    let is_docs =
        lower.contains("/docs") || lower.contains("docs.") || lower.contains("documentation");
    let is_social = lower.contains("twitter.com")
        || lower.contains("x.com")
        || lower.contains("reddit.com")
        || lower.contains("youtube.com")
        || lower.contains("linkedin.com");
    match (is_repo, is_docs, is_social) {
        (true, _, _) => ("repo", 0.85),
        (false, true, _) => ("docs", 0.8),
        (false, false, true) => ("social", 0.55),
        (false, false, false) => ("website", 0.65),
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
            match (host, parts.len() >= 2) {
                ("github.com", true) => {
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
                        Some("blob" | "tree") => match parts.len() > 4 {
                            true => Some(format!("{repo} {}", parts[4..].join("/"))),
                            false => Some(repo),
                        },
                        _ => Some(repo),
                    };
                }
                ("raw.githubusercontent.com", true) => {
                    let repo = format!("{}/{}", parts[0], parts[1]);
                    return match parts.len() > 3 {
                        true => Some(format!("{repo} {}", parts[3..].join("/"))),
                        false => Some(repo),
                    };
                }
                _ => {}
            }
            let path = parsed.path().trim_matches('/');
            match path.is_empty() {
                true => Some(host.to_string()),
                false => {
                    let last = path
                        .rsplit('/')
                        .next()
                        .unwrap_or(path)
                        .replace(['-', '_'], " ");
                    Some(format!("{} - {}", host, last))
                }
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
    [" is ", " are ", " was ", " built "]
        .into_iter()
        .find_map(|marker| {
            let idx = lower.find(marker)?;
            let prefix = &cleaned[..idx];
            let start = prefix.rfind(' ').map(|pos| pos + 1).unwrap_or(0);
            let trimmed = cleaned[start..].trim();
            (trimmed.chars().filter(|c| c.is_alphabetic()).count() >= 40)
                .then(|| trimmed.to_string())
        })
        .unwrap_or(cleaned)
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
    let term_hits = [
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
    ]
    .into_iter()
    .filter(|term| lower.contains(term))
    .count() as i32;
    let penalty = match is_navigation_noise(text) {
        true => 10,
        false => 0,
    };
    term_hits * 2 - penalty
}

fn signal_excerpt(text: &str, max_chars: usize) -> String {
    let mut chunks = sentence_chunks(text);
    chunks.sort_by_key(|b| std::cmp::Reverse(signal_score(b)));
    let mut out = Vec::new();
    for chunk in chunks.into_iter().filter(|c| signal_score(c) > 0) {
        match out.iter().any(|existing: &String| existing == &chunk) {
            true => continue,
            false => {}
        }
        let candidate = match out.is_empty() {
            true => chunk.clone(),
            false => format!("{}. {}", out.join(". "), chunk),
        };
        match candidate.chars().count() > max_chars {
            true => break,
            false => {}
        }
        out.push(chunk);
        match out.len() >= 3 {
            true => break,
            false => {}
        }
    }
    match out.is_empty() {
        true => text_excerpt(text, max_chars),
        false => out.join(". "),
    }
}

fn object_str<'a>(obj: &'a serde_json::Map<String, Value>, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| obj.get(*key).and_then(|v| v.as_str()))
}

fn candidate_from_object(obj: &serde_json::Map<String, Value>) -> Option<SourceCandidate> {
    let url = object_str(obj, &["url", "uri", "link", "href", "source"])
        .filter(|u| u.starts_with("http://") || u.starts_with("https://"))?;
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
            match candidate_from_object(map) {
                Some(candidate) => out.push(candidate),
                None => {}
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
            for raw_url in extract_urls(text) {
                let url = raw_url.trim_end_matches(['.', ',', ';', ':']).to_string();
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
        is_repo_url(&candidate.url)
            .then(|| canonical_research_topic(&candidate.url))
            .filter(|topic| topic.contains('/'))
    })
}

async fn existing_repo_topic_for(query: &str) -> Option<String> {
    let matches = search_source_bookmarks(query, 3).await.ok()?;
    matches.into_iter().find_map(|source| {
        let is_repo = source.kind.trim().eq_ignore_ascii_case("repo") || is_repo_url(&source.uri);
        is_repo
            .then(|| canonical_research_topic(&source.uri))
            .filter(|topic| topic.contains('/'))
    })
}

fn dedupe_candidates(candidates: Vec<SourceCandidate>) -> Vec<SourceCandidate> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for candidate in candidates {
        match seen.insert(candidate.url.clone()) {
            true => out.push(candidate),
            false => {}
        }
        match out.len() >= 5 {
            true => break,
            false => {}
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
                match !title.is_empty() || !snippet.is_empty() {
                    true => {
                        let mut line = String::new();
                        match title.is_empty() {
                            true => {}
                            false => line.push_str(title.trim()),
                        }
                        match snippet.is_empty() {
                            true => {}
                            false => {
                                match line.is_empty() {
                                    true => {}
                                    false => line.push_str(": "),
                                }
                                line.push_str(&signal_excerpt(snippet, 420));
                            }
                        }
                        match url.is_empty() {
                            true => {}
                            false => {
                                line.push_str(" (");
                                line.push_str(url);
                                line.push(')');
                            }
                        }
                        parts.push(line);
                    }
                    false => {}
                }
                for child in map.values() {
                    collect(child, parts);
                }
            }
            Value::Array(items) => items.iter().for_each(|item| collect(item, parts)),
            Value::String(text) => match parts.is_empty() {
                true => parts.push(signal_excerpt(text, 900)),
                false => {}
            },
            _ => {}
        }
    }
    collect(result, &mut parts);
    match parts.is_empty() {
        true => signal_excerpt(&serde_json::to_string(result).unwrap_or_default(), 900),
        false => text_excerpt(
            &parts.into_iter().take(8).collect::<Vec<_>>().join("; "),
            1200,
        ),
    }
}

pub async fn auto_capture_research_memory(
    tool_name: &str,
    arguments: &Value,
    result: &Value,
    user_content: &str,
) -> Result<Option<AutoCaptureSummary>> {
    let skip_tool = !is_research_tool(tool_name)
        || result.get("error").is_some()
        || result.get("status").and_then(|v| v.as_str()) == Some("skipped");
    match skip_tool || is_acquisition_or_display_user_content(user_content) {
        true => return Ok(None),
        false => {}
    }

    let user_research_worthy = is_research_worthy_user_content(user_content);
    let has_research_target_arg = first_str(
        arguments,
        &["query", "topic", "goal", "url", "repo", "repository"],
    )
    .map(is_research_worthy_user_content)
    .unwrap_or(false);
    let allowed = user_research_worthy || (user_content.trim().is_empty() && has_research_target_arg);
    match !allowed || is_refresh_only_user_content(user_content) {
        true => return Ok(None),
        false => {}
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
    let topic = match user_content.trim().is_empty() {
        true => topic_from(tool_name, arguments, user_content),
        false => {
            let user_topic = canonical_research_topic(user_content);
            let arg_topic = topic_from(tool_name, arguments, "");
            match user_topic.contains('/') {
                true => user_topic,
                false => {
                    let existing_repo_topic = existing_repo_topic_for(&user_topic).await;
                    candidate_repo_topic
                        .or(existing_repo_topic)
                        .or_else(|| arg_topic.contains('/').then_some(arg_topic))
                        .unwrap_or(user_topic)
                }
            }
        }
    };
    match topic.trim().is_empty() {
        true => return Ok(None),
        false => {}
    }

    let mut source_ids = Vec::new();
    for candidate in candidates {
        match add_source_bookmark(
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
            Ok(saved) => source_ids.push(saved.id),
            Err(_) => {}
        }
    }

    let summary = result_summary(result);
    let brief_saved = match summary.trim().is_empty() {
        true => false,
        false => save_research_brief(&topic, &summary, source_ids.clone(), 0.65, 0)
            .await
            .is_ok(),
    };

    match source_ids.is_empty() && !brief_saved {
        true => return Ok(None),
        false => {}
    }

    Ok(Some(AutoCaptureSummary {
        sources_saved: source_ids.len(),
        brief_saved,
        topic,
    }))
}

#[cfg(test)]
#[path = "auto_capture_tests.rs"]
mod tests;
