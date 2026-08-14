#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroundingClass {
    Trivial,
    Stable,
    LocalProject,
    PersonalMemory,
    CurrentExternal,
    SourceSpecific,
    HighStakes,
    Uncertain,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepExecutionPolicy {
    pub grounding_class: GroundingClass,
    pub allow_web: bool,
    pub allow_nested_delegation: bool,
    pub require_sources: bool,
    pub suppress_evolution: bool,
}

fn normalized(text: &str) -> String {
    text.to_ascii_lowercase()
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

fn looks_like_url_or_path(text: &str) -> bool {
    contains_any(
        text,
        &[
            "http://", "https://", "www.", ".pdf", ".docx", ".xlsx", "/tmp/", "~/", "./", "../",
        ],
    )
}

fn looks_trivial(text: &str) -> bool {
    let trimmed = text.trim();
    let words = trimmed.split_whitespace().count();
    words <= 10
        && contains_any(
            text,
            &[
                "hello",
                "hi",
                "hii",
                "hey",
                "greeting",
                "smoke test",
                "demo",
                "simple two-step",
                "summarize hello",
            ],
        )
}

pub fn classify_grounding_text(text: &str) -> GroundingClass {
    let text = normalized(text);

    if looks_like_url_or_path(&text) {
        return GroundingClass::SourceSpecific;
    }
    if contains_any(
        &text,
        &[
            "medical",
            "legal",
            "financial",
            "diagnosis",
            "law",
            "security vulnerability",
            "exploit",
            "cve",
        ],
    ) {
        return GroundingClass::HighStakes;
    }
    if contains_any(
        &text,
        &[
            "latest", "today", "current", "recent", "news", "price", "version", "schedule",
            "score", "release", "changed", "updated",
        ],
    ) {
        return GroundingClass::CurrentExternal;
    }
    if contains_any(
        &text,
        &[
            "this repo",
            "codebase",
            "file",
            "directory",
            "path",
            "function",
            "struct",
            "module",
            "implementation",
            "where is",
            "where are",
        ],
    ) {
        return GroundingClass::LocalProject;
    }
    if contains_any(
        &text,
        &[
            "remember",
            "memory",
            "preference",
            "saved",
            "skill",
            "what did we",
            "previous",
        ],
    ) {
        return GroundingClass::PersonalMemory;
    }
    if contains_any(
        &text,
        &["not sure", "unknown", "verify", "check if", "doubt"],
    ) {
        return GroundingClass::Uncertain;
    }
    if looks_trivial(&text) {
        return GroundingClass::Trivial;
    }
    GroundingClass::Stable
}

fn asks_for_delegation_or_complex_work(text: &str) -> bool {
    contains_any(
        text,
        &[
            "delegate",
            "subagent",
            "parallel",
            "multi-source",
            "research",
            "scan",
            "repo-wide",
            "implement",
            "refactor",
            "debug",
            "compare sources",
        ],
    )
}

pub fn step_execution_policy(
    workflow_goal: &str,
    step_goal: &str,
    agent: &str,
) -> StepExecutionPolicy {
    let combined = format!("{workflow_goal}\n{step_goal}");
    let class = classify_grounding_text(&combined);
    let combined_norm = normalized(&combined);
    let agent_norm = normalized(agent);
    let explicitly_complex = asks_for_delegation_or_complex_work(&combined_norm);
    let manager_agent = contains_any(&agent_norm, &["manager", "coordinator", "orchestrator"]);
    let researcher_agent = contains_any(&agent_norm, &["researcher", "research"]);

    let require_sources = matches!(
        class,
        GroundingClass::CurrentExternal
            | GroundingClass::SourceSpecific
            | GroundingClass::HighStakes
            | GroundingClass::Uncertain
    );
    let allow_web = require_sources
        || researcher_agent
        || contains_any(&combined_norm, &["web", "internet", "search", "docs"]);
    let allow_nested_delegation = manager_agent
        || explicitly_complex
        || matches!(
            class,
            GroundingClass::LocalProject | GroundingClass::SourceSpecific
        ) && !matches!(class, GroundingClass::Trivial | GroundingClass::Stable);
    let suppress_evolution = matches!(class, GroundingClass::Trivial)
        || contains_any(&combined_norm, &["smoke test", "demo", "hello"]);

    StepExecutionPolicy {
        grounding_class: class,
        allow_web,
        allow_nested_delegation,
        require_sources,
        suppress_evolution,
    }
}

pub fn main_agent_grounding_rules() -> &'static str {
    "\n
Balanced grounding rules:
- Answer directly for stable/trivial tasks. Do not search or delegate for greetings, simple wording, creative writing, provided-text summaries, or simple stable concepts.
- For user/project questions, use OpenZ memory, skills, saved links, local files, docs, and code tools before relying on model memory.
- Use web/search/fetch for current, changing, source-specific, high-stakes, or uncertain claims.
- Cite or name sources when live sources are used. If sources are missing or weak, say verification is incomplete instead of pretending certainty."
}

pub fn should_suppress_evolution(goal: &str, context: &str, summary: &str) -> bool {
    let combined = normalized(&format!("{goal}\n{context}\n{summary}"));
    let class = classify_grounding_text(&combined);
    let summary_words = summary.split_whitespace().count();
    matches!(class, GroundingClass::Trivial)
        || summary_words < 8
        || contains_any(&combined, &["smoke test", "demo", "hello"])
        || contains_any(
            &combined,
            &[
                "timed out",
                "failed",
                "cannot research",
                "missing tool",
                "tool limitation",
                "available tools",
            ],
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grounding_policy_classifies_trivial_hello() {
        let policy = step_execution_policy(
            "Run a smoke test workflow",
            "Summarize the word hello",
            "planner",
        );

        assert_eq!(policy.grounding_class, GroundingClass::Trivial);
        assert!(!policy.allow_web);
        assert!(!policy.allow_nested_delegation);
        assert!(!policy.require_sources);
        assert!(policy.suppress_evolution);
    }

    #[test]
    fn grounding_policy_requires_web_for_latest_version() {
        let class = classify_grounding_text("What is the latest stable Rust version today?");
        assert_eq!(class, GroundingClass::CurrentExternal);

        let policy = step_execution_policy(
            "Answer current software version question",
            "Find the latest stable Rust version today",
            "researcher",
        );
        assert!(policy.allow_web);
        assert!(policy.require_sources);
        assert!(!policy.suppress_evolution);
    }

    #[test]
    fn grounding_policy_detects_source_specific_requests() {
        assert_eq!(
            classify_grounding_text("Read https://example.com/docs and summarize it"),
            GroundingClass::SourceSpecific
        );
        assert_eq!(
            classify_grounding_text("Compare the PDF at /tmp/report.pdf"),
            GroundingClass::SourceSpecific
        );
    }

    #[test]
    fn grounding_policy_detects_project_questions() {
        assert_eq!(
            classify_grounding_text("In this repo, where is orchestrate_workflow implemented?"),
            GroundingClass::LocalProject
        );
    }

    #[test]
    fn evolution_suppressed_for_smoke_test_outputs() {
        assert!(should_suppress_evolution(
            "Run smoke test workflow",
            "demo context",
            "Planner summary accurate. No issues."
        ));
        assert!(should_suppress_evolution(
            "Review tiny output",
            "context",
            "Cannot research with available tools."
        ));
        assert!(!should_suppress_evolution(
            "Refactor tool routing",
            "Detailed coding task",
            "When adding tool schemas, keep required fields explicit and add focused regression tests before implementation."
        ));
    }
}
