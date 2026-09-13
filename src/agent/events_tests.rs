use super::*;

#[test]
fn default_visibility_hides_private_events() {
    let visibility = OutputVisibility::default();

    assert_eq!(
        AgentEvent::PrivateReasoning("internal tool strategy".to_string())
            .public_text(&visibility),
        None
    );
    assert_eq!(
        AgentEvent::TraceDebug(serde_json::json!({ "system": "hidden" }))
            .public_text(&visibility),
        None
    );
    assert_eq!(
        AgentEvent::WorkflowNotice("◇ Workflow matched: internal".to_string())
            .public_text(&visibility),
        None
    );
    assert_eq!(
        AgentEvent::MemoryCaptureSummary {
            sources_saved: 40,
            briefs_saved: 12,
            topics: "langgenius/dify".to_string(),
        }
        .public_text(&visibility),
        None
    );
}

#[test]
fn public_events_render_by_default() {
    let visibility = OutputVisibility::default();
    assert_eq!(
        AgentEvent::PublicMessage("done".to_string()).public_text(&visibility),
        Some("done".to_string())
    );
    assert_eq!(
        AgentEvent::PublicProgress("searching".to_string()).public_text(&visibility),
        Some("searching".to_string())
    );
}

#[test]
fn compact_reasoning_visibility_does_not_return_full_raw_text() {
    let visibility = OutputVisibility {
        reasoning: PublicReasoningVisibility::Compact,
        workflow_notices: false,
        memory_notices: false,
    };
    let raw = "private reasoning step ".repeat(80);
    let rendered = AgentEvent::PrivateReasoning(raw.clone())
        .public_text(&visibility)
        .expect("compact reasoning should render summary");

    assert!(rendered.starts_with("▶ Thought"));
    assert!(rendered.chars().count() < raw.chars().count());
    assert!(!rendered.contains(&raw));
}

#[test]
fn full_reasoning_visibility_is_explicit() {
    let visibility = OutputVisibility {
        reasoning: PublicReasoningVisibility::Full,
        workflow_notices: false,
        memory_notices: false,
    };
    assert_eq!(
        AgentEvent::PrivateReasoning("raw reasoning".to_string()).public_text(&visibility),
        Some("▶ Thought\n\n> raw reasoning".to_string())
    );
}
