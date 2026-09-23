//! Prompt-driven dynamic tool routing and BM25 lexical search helpers.
//!
//! Provides in-process BM25 lexical ranking across all registered native tools
//! (names, aliases, domains, descriptions, when_to_use, and examples) to select
//! the optimal compact tool subset for any user prompt without rigid hardcoding.

use super::{ToolMetadata, ToolRisk};
use std::collections::{BTreeSet, HashMap, HashSet};

pub(crate) fn is_core_tool(name: &str) -> bool {
    matches!(
        name,
        "tool_catalog"
            | "openz_inventory"
            | "request_tool_scope"
            | "manage_servers"
            | "workflow_memory"
            | "curate_skill"
            | "optimize_tool_scope"
            | "diagnose_tool"
            | "delegate_task"
            | "send_remote_input"
            | "read_file"
            | "find_files"
            | "grep_search"
    )
}

pub(crate) fn tool_allowed_by_filter(name: &str, filter: Option<&Vec<String>>) -> bool {
    if let Some(prefixes) = filter {
        is_core_tool(name) || prefixes.iter().any(|prefix| name.starts_with(prefix))
    } else {
        true
    }
}

/// Tokenize input string into lowercase alphanumeric words, filtering out single chars and stopwords.
pub(crate) fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|token| token.len() >= 2 && !is_stopword(token))
        .map(|s| s.to_string())
        .collect()
}

fn is_stopword(token: &str) -> bool {
    matches!(
        token,
        "a" | "an"
            | "the"
            | "in"
            | "on"
            | "of"
            | "to"
            | "for"
            | "with"
            | "is"
            | "are"
            | "at"
            | "by"
            | "from"
            | "this"
            | "that"
            | "and"
            | "or"
            | "it"
            | "as"
            | "be"
            | "was"
            | "were"
            | "will"
            | "would"
            | "can"
            | "could"
            | "should"
            | "have"
            | "has"
            | "had"
            | "do"
            | "does"
            | "did"
            | "my"
            | "your"
            | "me"
            | "you"
            | "all"
    )
}

/// BM25 term weighting and document structure for a native tool.
#[derive(Debug, Clone)]
pub struct ToolDoc<'a> {
    pub name: &'a str,
    #[allow(dead_code)]
    pub domain: &'static str,
    pub name_tokens: Vec<String>,
    pub alias_tokens: Vec<String>,
    pub domain_tokens: Vec<String>,
    pub desc_tokens: Vec<String>,
    pub total_len: usize,
}

impl<'a> ToolDoc<'a> {
    pub fn from_tool(tool: &'a dyn super::Tool) -> Self {
        let meta = tool.metadata();
        let name_tokens = tokenize(tool.name());
        let mut alias_tokens = Vec::new();
        for alias in meta.aliases {
            alias_tokens.extend(tokenize(alias));
        }
        if let Some(spec) = crate::tools::tool_spec(tool.name()) {
            for alias in spec.aliases {
                alias_tokens.extend(tokenize(alias));
            }
        }
        let inferred = ToolMetadata::infer(tool.name());
        for alias in inferred.aliases {
            alias_tokens.extend(tokenize(alias));
        }
        let domain_tokens = tokenize(meta.domain);
        let mut desc_tokens = tokenize(tool.description());
        desc_tokens.extend(tokenize(meta.when_to_use));
        desc_tokens.extend(tokenize(&meta.presentation_name));
        for ex in meta.examples {
            desc_tokens.extend(tokenize(ex));
        }

        let total_len =
            name_tokens.len() + alias_tokens.len() + domain_tokens.len() + desc_tokens.len();
        Self {
            name: tool.name(),
            domain: meta.domain,
            name_tokens,
            alias_tokens,
            domain_tokens,
            desc_tokens,
            total_len: total_len.max(1),
        }
    }
}

/// Compute BM25 relevance scores for all registered tools against a prompt.
pub(crate) fn compute_bm25_scores(
    prompt: &str,
    static_tools: &HashMap<String, std::sync::Arc<dyn super::Tool>>,
) -> HashMap<String, f32> {
    let query_tokens = tokenize(prompt);
    if query_tokens.is_empty() || static_tools.is_empty() {
        return HashMap::new();
    }

    let docs: Vec<ToolDoc> = static_tools
        .values()
        .map(|t| ToolDoc::from_tool(t.as_ref()))
        .collect();
    let num_docs = docs.len() as f32;
    let avg_doc_len = docs.iter().map(|d| d.total_len as f32).sum::<f32>() / num_docs.max(1.0);

    // Compute Document Frequency (DF) for each query token
    let mut df: HashMap<&str, usize> = HashMap::new();
    for token in &query_tokens {
        if df.contains_key(token.as_str()) {
            continue;
        }
        let count = docs
            .iter()
            .filter(|doc| {
                doc.name_tokens.contains(token)
                    || doc.alias_tokens.contains(token)
                    || doc.domain_tokens.contains(token)
                    || doc.desc_tokens.contains(token)
            })
            .count();
        df.insert(token.as_str(), count);
    }

    let k1: f32 = 1.2;
    let b: f32 = 0.75;
    let prompt_lower = prompt.to_ascii_lowercase();

    let mut scores = HashMap::new();
    for doc in docs {
        let mut score: f32 = 0.0;
        let len_norm = 1.0 - b + b * (doc.total_len as f32 / avg_doc_len);

        for q in &query_tokens {
            let doc_freq = *df.get(q.as_str()).unwrap_or(&0);
            if doc_freq == 0 {
                continue;
            }

            // Robertson-Spärck Jones IDF
            let idf = ((num_docs - doc_freq as f32 + 0.5) / (doc_freq as f32 + 0.5) + 1.0).ln();
            if idf <= 0.0 {
                continue;
            }

            // Weighted Term Frequency across fields
            let tf_name = doc.name_tokens.iter().filter(|t| *t == q).count() as f32 * 4.0;
            let tf_alias = doc.alias_tokens.iter().filter(|t| *t == q).count() as f32 * 3.0;
            let tf_domain = doc.domain_tokens.iter().filter(|t| *t == q).count() as f32 * 2.5;
            let tf_desc = doc.desc_tokens.iter().filter(|t| *t == q).count() as f32 * 1.2;
            let tf_total = tf_name + tf_alias + tf_domain + tf_desc;

            if tf_total > 0.0 {
                let tf_component = (tf_total * (k1 + 1.0)) / (tf_total + k1 * len_norm);
                score += idf * tf_component;
            }
        }

        // Substring / verbatim phrase bonuses
        let name_lower = doc.name.to_ascii_lowercase();
        if prompt_lower.contains(&name_lower) {
            score += 25.0;
        } else {
            let spaced = name_lower.replace('_', " ");
            if spaced.len() >= 4 && prompt_lower.contains(&spaced) {
                score += 20.0;
            }
        }

        scores.insert(doc.name.to_string(), score);
    }

    scores
}

pub(crate) fn select_domains_for_prompt(prompt: &str) -> BTreeSet<&'static str> {
    let lower = prompt.to_lowercase();
    let mut domains = BTreeSet::new();
    domains.insert("self_management");
    domains.insert("filesystem");
    domains.insert("subagent");

    if contains_any(
        &lower,
        &[
            "cargo", "rust", "test", "build", "compile", "compiler", "error", "code", "function",
            "module", "refactor", "lint", "clippy",
        ],
    ) {
        domains.insert("code");
        domains.insert("shell");
        domains.insert("git");
    }

    if contains_any(
        &lower,
        &[
            "cron",
            "cronjob",
            "scheduled job",
            "schedule job",
            "timer",
            "job logs",
            "run job",
            "pause job",
            "resume job",
        ],
    ) {
        domains.insert("cron");
    }
    if contains_any(
        &lower,
        &[
            "website", "web", "url", "browser", "page", "crawl", "fetch", "search", "internet",
            "research", "http", "https",
        ],
    ) {
        domains.insert("web");
    }
    if contains_any(
        &lower,
        &[
            "image",
            "photo",
            "picture",
            "screenshot",
            "svg",
            "video",
            "media",
            "mermaid",
            "diagram",
            "render",
            "chart",
            "animate",
            "animation",
            "mp4",
        ],
    ) {
        domains.insert("media");
        domains.insert("document");
    }
    if contains_any(
        &lower,
        &[
            "pdf",
            "docx",
            "xlsx",
            "pptx",
            "document",
            "spreadsheet",
            "archive",
            "excel",
            "slides",
            "presentation",
            "csv",
        ],
    ) {
        domains.insert("document");
    }
    if contains_any(
        &lower,
        &[
            "git", "commit", "push", "pull", "pr", "github", "branch", "diff",
        ],
    ) {
        domains.insert("git");
        domains.insert("code");
    }
    if contains_any(
        &lower,
        &["memory", "remember", "recall", "fact", "knowledge", "graph"],
    ) {
        domains.insert("memory");
    }
    if contains_any(&lower, &["think", "reason", "plan", "analyze", "breakdown"]) {
        domains.insert("reasoning");
        domains.insert("context");
    }
    if contains_any(
        &lower,
        &["terminal", "shell", "command", "bash", "process", "port", "exec"],
    ) {
        domains.insert("shell");
    }
    if contains_any(&lower, &["mcp", "server", "gateway", "bridge"]) {
        domains.insert("mcp");
    }

    domains
}

pub(crate) fn select_domains_with_bm25(
    prompt: &str,
    bm25_scores: &HashMap<String, f32>,
    static_tools: &HashMap<String, std::sync::Arc<dyn super::Tool>>,
) -> BTreeSet<&'static str> {
    let mut domains = select_domains_for_prompt(prompt);

    // Dynamically include domains of tools that have significant BM25 relevance (> 2.0)
    for (name, score) in bm25_scores {
        if *score > 2.0 {
            if let Some(tool) = static_tools.get(name) {
                domains.insert(tool.metadata().domain);
            }
        }
    }

    domains
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

#[allow(dead_code)]
pub(crate) fn tool_selection_score(
    name: &str,
    metadata: &ToolMetadata,
    selected_domains: &BTreeSet<&'static str>,
) -> i32 {
    tool_selection_score_with_bm25(name, metadata, selected_domains, 0.0)
}

pub(crate) fn tool_selection_score_with_bm25(
    name: &str,
    metadata: &ToolMetadata,
    selected_domains: &BTreeSet<&'static str>,
    bm25_score: f32,
) -> i32 {
    let mut score = metadata.priority as i32;
    if is_core_tool(name) {
        score += 1_000;
    }
    if selected_domains.contains(metadata.domain) {
        score += 500;
    }
    if bm25_score > 0.0 {
        score += ((bm25_score * 30.0) as i32).min(2000);
    }
    score -= match metadata.risk {
        ToolRisk::Low => 0,
        ToolRisk::Medium => 10,
        ToolRisk::High => 25,
    };
    if metadata.requires_approval {
        score -= 10;
    }
    score
}

#[allow(dead_code)]
pub(crate) fn tool_selection_reasons(
    name: &str,
    metadata: &ToolMetadata,
    selected_domains: &BTreeSet<&'static str>,
) -> Vec<&'static str> {
    tool_selection_reasons_with_bm25(name, metadata, selected_domains, 0.0)
}

pub(crate) fn tool_selection_reasons_with_bm25(
    name: &str,
    metadata: &ToolMetadata,
    selected_domains: &BTreeSet<&'static str>,
    bm25_score: f32,
) -> Vec<&'static str> {
    let mut reasons = Vec::new();
    if is_core_tool(name) {
        reasons.push("core_tool");
    }
    if selected_domains.contains(metadata.domain) {
        reasons.push("prompt_domain");
    }
    if bm25_score > 0.0 {
        reasons.push("bm25_relevance");
    }
    match metadata.risk {
        ToolRisk::Low => reasons.push("low_risk"),
        ToolRisk::Medium => reasons.push("medium_risk_penalty"),
        ToolRisk::High => reasons.push("high_risk_penalty"),
    }
    if metadata.requires_approval {
        reasons.push("requires_approval");
    }
    reasons
}

pub(crate) fn explicitly_requested_tool_names(
    prompt: &str,
    static_tools: &HashMap<String, std::sync::Arc<dyn super::Tool>>,
) -> HashSet<String> {
    let lower = prompt.to_ascii_lowercase();
    let mut requested = HashSet::new();

    for (name, tool) in static_tools {
        let name_lower = name.to_ascii_lowercase();
        // 1. Direct tool name match
        if lower.contains(&name_lower) {
            requested.insert(name.clone());
            continue;
        }

        // 2. Direct alias match from tool metadata or curated tool spec
        let metadata = tool.metadata();
        let mut matched = false;
        for alias in metadata.aliases {
            let alias_lower = alias.to_ascii_lowercase();
            if alias_lower.len() >= 3 && lower.contains(&alias_lower) {
                requested.insert(name.clone());
                matched = true;
                break;
            }
        }
        if !matched {
            if let Some(spec) = crate::tools::tool_spec(name) {
                for alias in spec.aliases {
                    let alias_lower = alias.to_ascii_lowercase();
                    if alias_lower.len() >= 3 && lower.contains(&alias_lower) {
                        requested.insert(name.clone());
                        matched = true;
                        break;
                    }
                }
            }
        }
        if !matched {
            let inferred = ToolMetadata::infer(name);
            for alias in inferred.aliases {
                let alias_lower = alias.to_ascii_lowercase();
                if alias_lower.len() >= 3 && lower.contains(&alias_lower) {
                    requested.insert(name.clone());
                    break;
                }
            }
        }

        // 3. Spaced name match (e.g. "device inventory" -> "device_inventory")
        let spaced = name_lower.replace('_', " ");
        if spaced.len() >= 4 && lower.contains(&spaced) {
            requested.insert(name.clone());
        }
    }

    // 4. Browser requests
    let browser_request = lower.contains("browser")
        || lower.contains("firefox")
        || lower.contains("youtube")
        || (lower.contains("play") && lower.contains("song"));
    if browser_request {
        for name in static_tools.keys() {
            let name_lower = name.to_ascii_lowercase();
            if name_lower.contains("browser") || name_lower == "inspect_browsers" {
                requested.insert(name.clone());
            }
        }
    }

    requested
}
