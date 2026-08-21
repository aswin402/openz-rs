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

pub fn classify_turn_intent(user_content: &str) -> IntentDecision {
    let lower = user_content.to_lowercase();
    let mut reasons = Vec::new();

    if has_live_research_intent(user_content) && !is_local_repo_query(&lower) {
        reasons.push("live_research_intent");
        return IntentDecision {
            intent: TurnIntent::ExternalResearch,
            knowledge_policy: KnowledgePolicy::RequireLiveResearch,
            reasons,
        };
    }

    if is_local_repo_query(&lower) {
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

fn is_local_execution_query(lower: &str) -> bool {
    [
        "cargo ",
        "just ",
        "test ",
        "check ",
        "compile",
        "build",
        "run command",
        "terminal",
        "shell",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
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
mod tests {
    use super::*;

    #[test]
    fn local_repo_question_prefers_repo_read_not_web() {
        let decision =
            classify_turn_intent("Where is orchestrate_workflow implemented in this repo?");
        assert_eq!(decision.intent, TurnIntent::LocalRepoRead);
        assert_eq!(decision.knowledge_policy, KnowledgePolicy::UseLocalContext);
        assert!(decision.reasons.contains(&"local_repo_query"));
    }

    #[test]
    fn latest_external_question_requires_live_research() {
        let decision = classify_turn_intent("What is the latest Rust stable version today?");
        assert_eq!(decision.intent, TurnIntent::ExternalResearch);
        assert_eq!(
            decision.knowledge_policy,
            KnowledgePolicy::RequireLiveResearch
        );
        assert!(decision.reasons.contains(&"live_research_intent"));
    }

    #[test]
    fn simple_general_task_uses_model_directly() {
        let decision = classify_turn_intent("summarize hello");
        assert_eq!(decision.intent, TurnIntent::DirectAnswer);
        assert_eq!(decision.knowledge_policy, KnowledgePolicy::ModelOk);
    }

    #[test]
    fn build_request_prefers_local_execution_tools() {
        let decision = classify_turn_intent(
            "run the focused cargo test for workflow_spec_round_trips_from_json",
        );
        assert_eq!(decision.intent, TurnIntent::LocalExecution);
        assert_eq!(decision.knowledge_policy, KnowledgePolicy::UseLocalContext);
    }
}
