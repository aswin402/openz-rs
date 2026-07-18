use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{add_source_bookmark, save_research_brief};

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
            let path = parsed.path().trim_matches('/');
            text = if host == "github.com" || host == "raw.githubusercontent.com" {
                path.split('/')
                    .take(2)
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>()
                    .join("/")
            } else if path.is_empty() {
                host.to_string()
            } else {
                format!("{} {}", host, path.replace(['-', '_', '/'], " "))
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

fn repo_topic_from_candidates(candidates: &[SourceCandidate]) -> Option<String> {
    candidates.iter().find_map(|candidate| {
        let topic = canonical_research_topic(&candidate.url);
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
        if !user_topic.contains('/') && arg_topic.contains('/') {
            arg_topic
        } else if !user_topic.contains('/') {
            candidate_repo_topic.unwrap_or(user_topic)
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
