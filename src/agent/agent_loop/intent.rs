use super::research_policy::{has_live_research_intent, text_has_http_url};

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

fn contains_phrase_bounded(text_lower: &str, phrase: &str) -> bool {
    let bytes = text_lower.as_bytes();
    let p_len = phrase.len();
    for (idx, _) in text_lower.match_indices(phrase) {
        let before_ok = idx == 0 || !bytes[idx - 1].is_ascii_alphanumeric();
        let after_idx = idx + p_len;
        let after_ok = after_idx >= bytes.len() || !bytes[after_idx].is_ascii_alphanumeric();
        if before_ok && after_ok {
            return true;
        }
    }
    false
}

pub fn classify_turn_intent(user_content: &str) -> IntentDecision {
    let lower = user_content.to_lowercase();
    let mut reasons = Vec::new();

    if is_local_repo_query(&lower) && !is_explicit_mutation_or_exec(&lower) {
        reasons.push("local_repo_query");
        return IntentDecision {
            intent: TurnIntent::LocalRepoRead,
            knowledge_policy: KnowledgePolicy::UseLocalContext,
            reasons,
        };
    }

    if is_orchestration_query(&lower) {
        reasons.push("orchestration_query");
        return IntentDecision {
            intent: TurnIntent::Orchestration,
            knowledge_policy: KnowledgePolicy::UseLocalContext,
            reasons,
        };
    }

    if is_cron_query(&lower) {
        reasons.push("cron_query");
        return IntentDecision {
            intent: TurnIntent::CronManagement,
            knowledge_policy: KnowledgePolicy::UseLocalContext,
            reasons,
        };
    }

    if has_live_research_intent(user_content) {
        reasons.push("live_research_intent");
        return IntentDecision {
            intent: TurnIntent::ExternalResearch,
            knowledge_policy: KnowledgePolicy::RequireLiveResearch,
            reasons,
        };
    }

    if is_local_execution_query(&lower) {
        reasons.push("local_execution_query");
        return IntentDecision {
            intent: TurnIntent::LocalExecution,
            knowledge_policy: KnowledgePolicy::UseLocalContext,
            reasons,
        };
    }

    if is_memory_query(&lower) {
        reasons.push("memory_query");
        return IntentDecision {
            intent: TurnIntent::MemoryLookup,
            knowledge_policy: KnowledgePolicy::UseMemoryOrSkillFirst,
            reasons,
        };
    }

    if is_media_or_document_query(&lower) || text_has_http_url(user_content) {
        reasons.push("media_or_document_query");
        return IntentDecision {
            intent: TurnIntent::MediaOrDocument,
            knowledge_policy: KnowledgePolicy::UseLocalContext,
            reasons,
        };
    }

    reasons.push("direct_answer_default");
    IntentDecision {
        intent: TurnIntent::DirectAnswer,
        knowledge_policy: KnowledgePolicy::ModelOk,
        reasons,
    }
}

fn is_local_repo_query(lower: &str) -> bool {
    [
        "this repo",
        "this codebase",
        "implemented",
        "where is",
        "which file",
        "src/",
        "cargo.toml",
        "function",
        "struct",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn is_explicit_mutation_or_exec(lower: &str) -> bool {
    [
        "exec_command",
        "write_file",
        "patch_file",
        "replace_lines",
        "create a file",
        "write a file",
        "rm -",
    ]
    .iter()
    .any(|needle| contains_phrase_bounded(lower, needle))
}

fn is_local_execution_query(lower: &str) -> bool {
    [
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
    ]
    .iter()
    .any(|needle| contains_phrase_bounded(lower, needle))
}

fn is_orchestration_query(lower: &str) -> bool {
    [
        "orchestrate_workflow",
        "orchestrate workflow",
        "multi-step workflow",
        "planner",
        "reviewer",
        "subagent workflow",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn is_cron_query(lower: &str) -> bool {
    [
        "cron",
        "cornjob",
        "schedule_job",
        "list_jobs",
        "remove_job",
        "scheduled job",
        "job logs",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn is_memory_query(lower: &str) -> bool {
    ["memory", "skill", "saved link", "remember", "my name"]
        .iter()
        .any(|needle| lower.contains(needle))
}

fn is_media_or_document_query(lower: &str) -> bool {
    [
        "image",
        "screenshot",
        "pdf",
        "docx",
        "video",
        "audio",
        "ocr",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

#[cfg(test)]
#[path = "intent_tests.rs"]
mod tests;
