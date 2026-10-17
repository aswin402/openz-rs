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
    filter.is_none_or(|prefixes| {
        is_core_tool(name) || prefixes.iter().any(|prefix| name.starts_with(prefix))
    })
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
        let spec_aliases = crate::tools::tool_spec(tool.name())
            .map(|spec| spec.aliases)
            .unwrap_or(&[]);
        let inferred = ToolMetadata::infer(tool.name());
        let unique_aliases: BTreeSet<&str> = meta
            .aliases
            .iter()
            .chain(spec_aliases.iter())
            .chain(inferred.aliases.iter())
            .copied()
            .collect();
        let alias_tokens: Vec<String> = unique_aliases.into_iter().flat_map(tokenize).collect();
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
    match query_tokens.is_empty() || static_tools.is_empty() {
        true => return HashMap::new(),
        false => {}
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
        df.entry(token.as_str()).or_insert_with(|| {
            docs.iter()
                .filter(|doc| {
                    doc.name_tokens.contains(token)
                        || doc.alias_tokens.contains(token)
                        || doc.domain_tokens.contains(token)
                        || doc.desc_tokens.contains(token)
                })
                .count()
        });
    }

    let k1: f32 = 1.2;
    let b: f32 = 0.75;
    let prompt_lower = prompt.to_ascii_lowercase();

    let mut scores = HashMap::new();
    for doc in docs {
        let len_norm = 1.0 - b + b * (doc.total_len as f32 / avg_doc_len);
        let lexical_score: f32 = query_tokens
            .iter()
            .filter_map(|q| {
                let doc_freq = *df.get(q.as_str()).unwrap_or(&0);
                (doc_freq > 0).then(|| {
                    let idf =
                        ((num_docs - doc_freq as f32 + 0.5) / (doc_freq as f32 + 0.5) + 1.0).ln();
                    let tf_name = doc.name_tokens.iter().filter(|t| *t == q).count() as f32 * 4.0;
                    let tf_alias = doc.alias_tokens.iter().filter(|t| *t == q).count() as f32 * 3.0;
                    let tf_domain =
                        doc.domain_tokens.iter().filter(|t| *t == q).count() as f32 * 2.5;
                    let tf_desc = doc.desc_tokens.iter().filter(|t| *t == q).count() as f32 * 1.2;
                    let tf_total = tf_name + tf_alias + tf_domain + tf_desc;
                    match idf > 0.0 && tf_total > 0.0 {
                        true => {
                            let tf_component =
                                (tf_total * (k1 + 1.0)) / (tf_total + k1 * len_norm);
                            idf * tf_component
                        }
                        false => 0.0,
                    }
                })
            })
            .sum();

        // Substring / verbatim phrase bonuses
        let name_lower = doc.name.to_ascii_lowercase();
        let spaced = name_lower.replace('_', " ");
        let phrase_bonus = match (
            prompt_lower.contains(&name_lower),
            spaced.len() >= 4 && prompt_lower.contains(&spaced),
        ) {
            (true, _) => 25.0,
            (false, true) => 20.0,
            (false, false) => 0.0,
        };

        scores.insert(doc.name.to_string(), lexical_score + phrase_bonus);
    }

    scores
}

const PROMPT_DOMAIN_RULES: &[(&[&str], &[&str])] = &[
    (
        &[
            "cargo", "rust", "test", "build", "compile", "compiler", "error", "code", "function",
            "module", "refactor", "lint", "clippy",
        ],
        &["code", "shell", "git"],
    ),
    (
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
        &["cron"],
    ),
    (
        &[
            "website", "web", "url", "browser", "page", "crawl", "fetch", "search", "internet",
            "research", "http", "https",
        ],
        &["web"],
    ),
    (
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
        &["media", "document"],
    ),
    (
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
        &["document"],
    ),
    (
        &[
            "git", "commit", "push", "pull", "pr", "github", "branch", "diff",
        ],
        &["git", "code"],
    ),
    (
        &["memory", "remember", "recall", "fact", "knowledge", "graph"],
        &["memory"],
    ),
    (
        &["think", "reason", "plan", "analyze", "breakdown"],
        &["reasoning", "context"],
    ),
    (
        &[
            "terminal", "shell", "command", "bash", "process", "port", "exec",
        ],
        &["shell"],
    ),
    (&["mcp", "server", "gateway", "bridge"], &["mcp"]),
];

pub(crate) fn select_domains_for_prompt(prompt: &str) -> BTreeSet<&'static str> {
    let lower = prompt.to_lowercase();
    let mut domains = BTreeSet::from(["self_management", "filesystem", "subagent"]);

    for (keywords, target_domains) in PROMPT_DOMAIN_RULES {
        match contains_any(&lower, keywords) {
            true => {
                for domain in *target_domains {
                    domains.insert(*domain);
                }
            }
            false => {}
        }
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
        match (*score > 2.0, static_tools.get(name)) {
            (true, Some(tool)) => {
                domains.insert(tool.metadata().domain);
            }
            _ => {}
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
    let core_bonus = match is_core_tool(name) {
        true => 1_000,
        false => 0,
    };
    let domain_bonus = match selected_domains.contains(metadata.domain) {
        true => 500,
        false => 0,
    };
    let bm25_bonus = match bm25_score > 0.0 {
        true => ((bm25_score * 30.0) as i32).min(2000),
        false => 0,
    };
    let risk_penalty = match metadata.risk {
        ToolRisk::Low => 0,
        ToolRisk::Medium => 10,
        ToolRisk::High => 25,
    };
    let approval_penalty = match metadata.requires_approval {
        true => 10,
        false => 0,
    };

    metadata.priority as i32 + core_bonus + domain_bonus + bm25_bonus - risk_penalty
        - approval_penalty
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
    let risk_reason = match metadata.risk {
        ToolRisk::Low => "low_risk",
        ToolRisk::Medium => "medium_risk_penalty",
        ToolRisk::High => "high_risk_penalty",
    };
    [
        is_core_tool(name).then_some("core_tool"),
        selected_domains
            .contains(metadata.domain)
            .then_some("prompt_domain"),
        (bm25_score > 0.0).then_some("bm25_relevance"),
        Some(risk_reason),
        metadata.requires_approval.then_some("requires_approval"),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn contains_phrase_bounded(haystack: &str, needle: &str) -> bool {
    !needle.is_empty()
        && haystack.match_indices(needle).any(|(start, _)| {
            let end = start + needle.len();
            let before_ok = haystack[..start]
                .chars()
                .next_back()
                .is_none_or(|c| !c.is_ascii_alphanumeric() && c != '_');
            let after_ok = haystack[end..]
                .chars()
                .next()
                .is_none_or(|c| !c.is_ascii_alphanumeric() && c != '_');
            before_ok && after_ok
        })
}

pub(crate) fn explicitly_requested_tool_names(
    prompt: &str,
    static_tools: &HashMap<String, std::sync::Arc<dyn super::Tool>>,
) -> HashSet<String> {
    let lower = prompt.to_ascii_lowercase();
    let browser_request = lower.contains("browser")
        || lower.contains("firefox")
        || lower.contains("youtube")
        || (lower.contains("play") && lower.contains("song"));

    static_tools
        .iter()
        .filter_map(|(name, tool)| {
            let name_lower = name.to_ascii_lowercase();
            let direct_name_match = contains_phrase_bounded(&lower, &name_lower);
            let metadata = tool.metadata();
            let spec_aliases = crate::tools::tool_spec(name)
                .map(|spec| spec.aliases)
                .unwrap_or(&[]);
            let inferred = ToolMetadata::infer(name);
            let alias_match = metadata
                .aliases
                .iter()
                .chain(spec_aliases.iter())
                .chain(inferred.aliases.iter())
                .map(|alias| alias.to_ascii_lowercase())
                .any(|alias_lower| {
                    alias_lower.len() >= 3 && contains_phrase_bounded(&lower, &alias_lower)
                });
            let spaced = name_lower.replace('_', " ");
            let spaced_match = spaced.len() >= 4 && contains_phrase_bounded(&lower, &spaced);
            let browser_match = browser_request
                && (name_lower.contains("browser") || name_lower == "inspect_browsers");

            (direct_name_match || alias_match || spaced_match || browser_match)
                .then(|| name.clone())
        })
        .collect()
}
