use super::research_policy::{has_live_research_intent, text_has_http_url};
use crate::tools::routing::{contains_any, contains_phrase_bounded};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnIntent {
    DirectAnswer,
    LocalRepoRead,
    LocalExecution,
    ExternalResearch,
    MemoryLookup,
    CronManagement,
    Orchestration,
    MediaOrDocument,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KnowledgePolicy {
    ModelOk,
    UseLocalContext,
    RequireLiveResearch,
    UseMemoryOrSkillFirst,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntentDecision {
    pub intent: TurnIntent,
    pub knowledge_policy: KnowledgePolicy,
    pub reasons: Vec<&'static str>,
}

const LOCAL_REPO_KEYWORDS: &[&str] = &[
    "this repo",
    "this codebase",
    "implemented",
    "where is",
    "which file",
    "src/",
    "cargo.toml",
    "function",
    "struct",
];

const EXPLICIT_MUTATION_KEYWORDS: &[&str] = &[
    "exec_command",
    "write_file",
    "patch_file",
    "replace_lines",
    "create a file",
    "write a file",
    "rm -",
];

const LOCAL_EXECUTION_KEYWORDS: &[&str] = &[
    "cargo",
    "just",
    "test",
    "check",
    "compile",
    "build",
    "run command",
    "exec_command",
    "write_file",
    "patch_file",
    "replace_lines",
    "create a file",
    "write a file",
    "save to",
    "rm -f",
    "rm -rf",
    "terminal",
    "shell",
];

const ORCHESTRATION_KEYWORDS: &[&str] = &[
    "orchestrate_workflow",
    "orchestrate workflow",
    "multi-step workflow",
    "planner",
    "reviewer",
    "subagent workflow",
];

const CRON_KEYWORDS: &[&str] = &[
    "cron",
    "cornjob",
    "schedule_job",
    "list_jobs",
    "remove_job",
    "scheduled job",
    "job logs",
];

const MEMORY_KEYWORDS: &[&str] = &["memory", "skill", "saved link", "remember", "my name"];

const MEDIA_OR_DOCUMENT_KEYWORDS: &[&str] = &[
    "image",
    "screenshot",
    "pdf",
    "docx",
    "video",
    "audio",
    "ocr",
];

pub fn classify_turn_intent(user_content: &str) -> IntentDecision {
    let lower = user_content.to_lowercase();

    let candidates: [(bool, TurnIntent, KnowledgePolicy, &'static str); 7] = [
        (
            is_local_repo_query(&lower) && !is_explicit_mutation_or_exec(&lower),
            TurnIntent::LocalRepoRead,
            KnowledgePolicy::UseLocalContext,
            "local_repo_query",
        ),
        (
            is_orchestration_query(&lower),
            TurnIntent::Orchestration,
            KnowledgePolicy::UseLocalContext,
            "orchestration_query",
        ),
        (
            is_cron_query(&lower),
            TurnIntent::CronManagement,
            KnowledgePolicy::UseLocalContext,
            "cron_query",
        ),
        (
            has_live_research_intent(user_content),
            TurnIntent::ExternalResearch,
            KnowledgePolicy::RequireLiveResearch,
            "live_research_intent",
        ),
        (
            is_local_execution_query(&lower),
            TurnIntent::LocalExecution,
            KnowledgePolicy::UseLocalContext,
            "local_execution_query",
        ),
        (
            is_memory_query(&lower),
            TurnIntent::MemoryLookup,
            KnowledgePolicy::UseMemoryOrSkillFirst,
            "memory_query",
        ),
        (
            is_media_or_document_query(&lower) || text_has_http_url(user_content),
            TurnIntent::MediaOrDocument,
            KnowledgePolicy::UseLocalContext,
            "media_or_document_query",
        ),
    ];

    candidates
        .into_iter()
        .find_map(|(matched, intent, knowledge_policy, reason)| {
            matched.then(|| IntentDecision {
                intent,
                knowledge_policy,
                reasons: vec![reason],
            })
        })
        .unwrap_or_else(|| IntentDecision {
            intent: TurnIntent::DirectAnswer,
            knowledge_policy: KnowledgePolicy::ModelOk,
            reasons: vec!["direct_answer_default"],
        })
}

fn is_local_repo_query(lower: &str) -> bool {
    contains_any(lower, LOCAL_REPO_KEYWORDS)
}

fn is_explicit_mutation_or_exec(lower: &str) -> bool {
    EXPLICIT_MUTATION_KEYWORDS
        .iter()
        .any(|needle| contains_phrase_bounded(lower, needle))
}

fn is_local_execution_query(lower: &str) -> bool {
    LOCAL_EXECUTION_KEYWORDS
        .iter()
        .any(|needle| contains_phrase_bounded(lower, needle))
}

fn is_orchestration_query(lower: &str) -> bool {
    contains_any(lower, ORCHESTRATION_KEYWORDS)
}

fn is_cron_query(lower: &str) -> bool {
    contains_any(lower, CRON_KEYWORDS)
}

fn is_memory_query(lower: &str) -> bool {
    contains_any(lower, MEMORY_KEYWORDS)
}

fn is_media_or_document_query(lower: &str) -> bool {
    contains_any(lower, MEDIA_OR_DOCUMENT_KEYWORDS)
}

#[cfg(test)]
#[path = "intent_tests.rs"]
mod tests;
