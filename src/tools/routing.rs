//! Prompt-driven static tool routing helpers.
//!
//! These functions preserve the existing routing scores and keyword rules,
//! but keep them outside the registry facade so name lookup and prompt
//! selection can evolve independently.

use super::{ToolMetadata, ToolRisk};
use std::collections::{BTreeSet, HashMap, HashSet};

pub(crate) fn is_core_tool(name: &str) -> bool {
    matches!(
        name,
        "tool_catalog"
            | "openz_inventory"
            | "manage_servers"
            | "schedule_job"
            | "list_jobs"
            | "remove_job"
            | "pause_job"
            | "resume_job"
            | "get_job"
            | "get_job_logs"
            | "run_job_now"
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
        &["terminal", "shell", "command", "bash", "process", "port"],
    ) {
        domains.insert("shell");
    }
    if contains_any(&lower, &["mcp", "server", "gateway", "bridge"]) {
        domains.insert("mcp");
    }

    domains
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

pub(crate) fn tool_selection_score(
    name: &str,
    metadata: &ToolMetadata,
    selected_domains: &BTreeSet<&'static str>,
) -> i32 {
    let mut score = metadata.priority as i32;
    if is_core_tool(name) {
        score += 1_000;
    }
    if selected_domains.contains(metadata.domain) {
        score += 500;
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

pub(crate) fn tool_selection_reasons(
    name: &str,
    metadata: &ToolMetadata,
    selected_domains: &BTreeSet<&'static str>,
) -> Vec<&'static str> {
    let mut reasons = Vec::new();
    if is_core_tool(name) {
        reasons.push("core_tool");
    }
    if selected_domains.contains(metadata.domain) {
        reasons.push("prompt_domain");
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
    let browser_request = lower.contains("browser")
        || lower.contains("firefox")
        || lower.contains("youtube")
        || lower.contains("play") && lower.contains("song");
    static_tools
        .keys()
        .filter(|name| {
            let name_lower = name.to_ascii_lowercase();
            lower.contains(&name_lower)
                || (*name == "open_path" && lower.contains("system image viewer"))
                || (*name == "device_inventory" && lower.contains("device inventory"))
                || (browser_request
                    && (name_lower.contains("browser") || name_lower == "inspect_browsers"))
        })
        .cloned()
        .collect()
}
