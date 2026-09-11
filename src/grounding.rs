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

fn contains_any_word(text: &str, needles: &[&str]) -> bool {
    text.split(|c: char| !c.is_ascii_alphanumeric())
        .any(|word| needles.contains(&word))
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
        && (contains_any(
            text,
            &[
                "hello",
                "hii",
                "greeting",
                "smoke test",
                "demo",
                "simple two-step",
                "summarize hello",
            ],
        ) || contains_any_word(text, &["hi", "hey"]))
}

pub fn classify_grounding_text(text: &str) -> GroundingClass {
    let text = normalized(text);

    if looks_like_url_or_path(&text) {
        return GroundingClass::SourceSpecific;
    }
    if contains_any_word(
        &text,
        &[
            "medical",
            "medication",
            "medicine",
            "legal",
            "financial",
            "diagnosis",
            "tax",
            "investment",
            "contract",
            "safety",
            "exploit",
            "cve",
        ],
    ) || contains_any(&text, &["security vulnerability"])
    {
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
            "refactor",
            "debug",
            "compare sources",
        ],
    ) || text.split_whitespace().any(|word| {
        matches!(
            word.trim_matches(|c: char| !c.is_ascii_alphanumeric()),
            "implement" | "implementation"
        )
    })
}

fn should_inherit_workflow_grounding(step_goal: &str) -> bool {
    let step = normalized(step_goal);
    !contains_any(
        &step,
        &[
            "review",
            "validate",
            "critique",
            "approve",
            "check output",
            "review output",
            "planner output",
            "prior step",
            "previous step",
        ],
    )
}

pub fn step_execution_policy(
    workflow_goal: &str,
    step_goal: &str,
    agent: &str,
) -> StepExecutionPolicy {
    let combined = format!("{workflow_goal}\n{step_goal}");
    let step_class = classify_grounding_text(step_goal);
    let workflow_class = classify_grounding_text(workflow_goal);
    let class = if matches!(step_class, GroundingClass::Stable)
        && should_inherit_workflow_grounding(step_goal)
    {
        workflow_class
    } else {
        step_class
    };
    let combined_norm = normalized(&combined);
    let step_norm = normalized(step_goal);
    let agent_norm = normalized(agent);
    let explicitly_complex = asks_for_delegation_or_complex_work(&step_norm);
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
        || contains_any(&step_norm, &["web", "internet", "search", "docs"]);
    let allow_nested_delegation = explicitly_complex;
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

fn looks_reusable_evolution_guidance(goal: &str, context: &str, summary: &str) -> bool {
    let combined = normalized(&format!("{goal}\n{context}\n{summary}"));
    contains_any(
        &combined,
        &[
            "add a focused regression test",
            "when adding",
            "when changing",
            "refactor",
            "routing",
            "implementation",
            "verify",
        ],
    )
}

pub fn should_suppress_evolution(goal: &str, context: &str, summary: &str) -> bool {
    let combined = normalized(&format!("{goal}\n{context}\n{summary}"));
    let class = classify_grounding_text(&combined);
    let summary_words = summary.split_whitespace().count();
    let reusable_guidance = looks_reusable_evolution_guidance(goal, context, summary);

    contains_any(
        &combined,
        &[
            "timed out",
            "failed",
            "cannot research",
            "missing tool",
            "tool limitation",
            "available tools",
        ],
    ) || (!reusable_guidance
        && (matches!(class, GroundingClass::Trivial)
            || contains_any(&combined, &["smoke test", "demo", "hello"])
            || summary_words < 18))
}

#[cfg(test)]
#[path = "grounding_tests.rs"]
mod tests;

