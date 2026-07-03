use super::*;
use std::sync::OnceLock;
use crate::tools::Tool;
use serde_json::{json, Value};

/// Serialize tests that touch the shared ENGINE static.
static TEST_MUTEX: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
fn test_lock() -> &'static tokio::sync::Mutex<()> {
    TEST_MUTEX.get_or_init(|| tokio::sync::Mutex::new(()))
}

async fn seed_engine(session_id: &str) {
    let engine = tools::get_engine();
    let mut guard = engine.lock().await;
    guard.store = Box::new(MemoryThoughtStore::new());
    guard.current_session_id = String::new();
    guard.thought_history.clear();
    guard.branches.clear();

    let _ = guard.process_thought(ThoughtData {
        thought: "Initial thought".to_string(), thought_number: 1, total_thoughts: 3, next_thought_needed: true,
        is_revision: None, revises_thought: None, branch_from_thought: None, branch_id: None,
        needs_more_thoughts: None, parent_thoughts: None,
        assumptions: Some(vec!["A1".to_string()]), verified_assumptions: None,
        confidence_score: Some(0.8), criticism: None, hypothesis: Some("H1".to_string()),
        verification_method: Some("V1".to_string()), left_to_be_done: Some(vec!["Todo1".to_string()]),
        timestamp: None, session_id: Some(session_id.to_string()),
    });
    let _ = guard.process_thought(ThoughtData {
        thought: "Branching thought".to_string(), thought_number: 2, total_thoughts: 3, next_thought_needed: true,
        is_revision: None, revises_thought: None, branch_from_thought: Some(1),
        branch_id: Some("branch-a".to_string()), needs_more_thoughts: None, parent_thoughts: None,
        assumptions: Some(vec!["A2".to_string()]), verified_assumptions: Some(vec!["refuted: A1".to_string()]),
        confidence_score: Some(0.3), criticism: None, hypothesis: None,
        verification_method: None, left_to_be_done: None,
        timestamp: None, session_id: Some(session_id.to_string()),
    });
    let _ = guard.process_thought(ThoughtData {
        thought: "Revising first thought".to_string(), thought_number: 3, total_thoughts: 3, next_thought_needed: false,
        is_revision: Some(true), revises_thought: Some(1), branch_from_thought: None, branch_id: None,
        needs_more_thoughts: None, parent_thoughts: None, assumptions: None, verified_assumptions: None,
        confidence_score: Some(0.9), criticism: None, hypothesis: None,
        verification_method: None, left_to_be_done: None,
        timestamp: None, session_id: Some(session_id.to_string()),
    });
}

#[tokio::test]
async fn test_basic_thought() {
    let _l = test_lock().lock().await;
    let engine = tools::get_engine();
    let mut guard = engine.lock().await;

    let input = ThoughtData {
        thought: "First thought".to_string(), thought_number: 1, total_thoughts: 3, next_thought_needed: true,
        is_revision: None, revises_thought: None, branch_from_thought: None, branch_id: None,
        needs_more_thoughts: None, parent_thoughts: None, assumptions: None, verified_assumptions: None,
        confidence_score: None, criticism: None, hypothesis: None, verification_method: None,
        left_to_be_done: None, timestamp: None, session_id: None,
    };
    let result = guard.process_thought(input).unwrap();
    assert_eq!(result.thought_number, 1);
    assert_eq!(result.total_thoughts, 3);
    assert_eq!(result.next_thought_needed, true);
    assert_eq!(result.thought_history_length, 1);
}

#[tokio::test]
async fn test_auto_adjust_total_thoughts() {
    let _l = test_lock().lock().await;
    let engine = tools::get_engine();
    let mut guard = engine.lock().await;
    guard.store = Box::new(MemoryThoughtStore::new());
    guard.current_session_id = String::new();
    guard.thought_history.clear();
    guard.branches.clear();

    let input = ThoughtData {
        thought: "Future thought".to_string(), thought_number: 5, total_thoughts: 3, next_thought_needed: true,
        is_revision: None, revises_thought: None, branch_from_thought: None, branch_id: None,
        needs_more_thoughts: None, parent_thoughts: None, assumptions: None, verified_assumptions: None,
        confidence_score: None, criticism: None, hypothesis: None, verification_method: None,
        left_to_be_done: None, timestamp: None, session_id: None,
    };
    let result = guard.process_thought(input).unwrap();
    assert_eq!(result.total_thoughts, 5);
}

#[tokio::test]
async fn test_branching() {
    let _l = test_lock().lock().await;
    let engine = tools::get_engine();
    let mut guard = engine.lock().await;
    guard.store = Box::new(MemoryThoughtStore::new());
    guard.current_session_id = String::new();
    guard.thought_history.clear();
    guard.branches.clear();

    guard.process_thought(ThoughtData {
        thought: "Main line".to_string(), thought_number: 1, total_thoughts: 3, next_thought_needed: true,
        is_revision: None, revises_thought: None, branch_from_thought: None, branch_id: None,
        needs_more_thoughts: None, parent_thoughts: None, assumptions: None, verified_assumptions: None,
        confidence_score: None, criticism: None, hypothesis: None, verification_method: None,
        left_to_be_done: None, timestamp: None, session_id: None,
    }).unwrap();
    let result = guard.process_thought(ThoughtData {
        thought: "Branch line".to_string(), thought_number: 2, total_thoughts: 3, next_thought_needed: true,
        is_revision: None, revises_thought: None, branch_from_thought: Some(1),
        branch_id: Some("branch-a".to_string()), needs_more_thoughts: None, parent_thoughts: None,
        assumptions: None, verified_assumptions: None, confidence_score: None, criticism: None,
        hypothesis: None, verification_method: None, left_to_be_done: None, timestamp: None, session_id: None,
    }).unwrap();
    assert_eq!(result.branches.len(), 1);
    assert!(result.branches.contains(&"branch-a".to_string()));
    assert!(result.thought_graph_mermaid.contains("T1 --> T2"));
}

#[tokio::test]
async fn test_mermaid_got_parent() {
    let _l = test_lock().lock().await;
    let engine = tools::get_engine();
    let mut guard = engine.lock().await;
    guard.store = Box::new(MemoryThoughtStore::new());
    guard.current_session_id = String::new();
    guard.thought_history.clear();
    guard.branches.clear();

    guard.process_thought(ThoughtData {
        thought: "Idea A".to_string(), thought_number: 1, total_thoughts: 3, next_thought_needed: true,
        is_revision: None, revises_thought: None, branch_from_thought: None, branch_id: None,
        needs_more_thoughts: None, parent_thoughts: None, assumptions: None, verified_assumptions: None,
        confidence_score: None, criticism: None, hypothesis: None, verification_method: None,
        left_to_be_done: None, timestamp: None, session_id: None,
    }).unwrap();
    guard.process_thought(ThoughtData {
        thought: "Idea B".to_string(), thought_number: 2, total_thoughts: 3, next_thought_needed: true,
        is_revision: None, revises_thought: None, branch_from_thought: None, branch_id: None,
        needs_more_thoughts: None, parent_thoughts: None, assumptions: None, verified_assumptions: None,
        confidence_score: None, criticism: None, hypothesis: None, verification_method: None,
        left_to_be_done: None, timestamp: None, session_id: None,
    }).unwrap();
    let result = guard.process_thought(ThoughtData {
        thought: "Merge A and B".to_string(), thought_number: 3, total_thoughts: 3, next_thought_needed: false,
        is_revision: None, revises_thought: None, branch_from_thought: None, branch_id: None,
        needs_more_thoughts: None, parent_thoughts: Some(vec![1, 2]), assumptions: None,
        verified_assumptions: None, confidence_score: None, criticism: None, hypothesis: None,
        verification_method: None, left_to_be_done: None, timestamp: None, session_id: None,
    }).unwrap();
    assert!(result.thought_graph_mermaid.contains("T1 --> T3"));
    assert!(result.thought_graph_mermaid.contains("T2 --> T3"));
}

#[tokio::test]
async fn test_analyze_graph_tool() {
    let _l = test_lock().lock().await;
    seed_engine("test-session").await;
    let tool = AnalyzeGraphTool;

    // low_confidence
    let res = tool.call(&json!({"query": "low_confidence", "confidenceThreshold": 0.4, "sessionId": "test-session"})).await.unwrap();
    let list: Vec<Value> = serde_json::from_value(res).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["thoughtNumber"], 2);

    // contradictions
    let res = tool.call(&json!({"query": "contradictions", "sessionId": "test-session"})).await.unwrap();
    let list: Vec<String> = serde_json::from_value(res).unwrap();
    assert_eq!(list.len(), 1);

    // summary_stats
    let res = tool.call(&json!({"query": "summary_stats", "sessionId": "test-session"})).await.unwrap();
    assert_eq!(res["totalThoughts"], 3);
    assert!(res["qualityScore"].is_number());
}

#[tokio::test]
async fn test_export_mermaid() {
    let _l = test_lock().lock().await;
    seed_engine("test-session").await;
    let tool = ExportSessionTool;

    let res = tool.call(&json!({"format": "mermaid", "sessionId": "test-session"})).await.unwrap();
    assert!(res["data"].as_str().unwrap().contains("graph TD"));
}

#[tokio::test]
async fn test_export_markdown() {
    let _l = test_lock().lock().await;
    seed_engine("test-session").await;
    let tool = ExportSessionTool;

    let res = tool.call(&json!({"format": "markdown", "sessionId": "test-session"})).await.unwrap();
    assert!(res["data"].as_str().unwrap().contains("# Reasoning Session History"));
}

#[tokio::test]
async fn test_summarize_reasoning() {
    let _l = test_lock().lock().await;
    seed_engine("test-session").await;
    let tool = SummarizeReasoningTool;

    let res = tool.call(&json!({"sessionId": "test-session"})).await.unwrap();
    assert_eq!(res["totalThoughts"], 3);
    assert_eq!(res["totalBranches"], 1);
}

#[tokio::test]
async fn test_templates_tool() {
    let tool = TemplatesTool;
    let res = tool.call(&json!({"template": "all"})).await.unwrap();
    assert!(res["templates"].is_array());
    assert_eq!(res["templates"].as_array().unwrap().len(), 3);
}

#[test]
fn test_cycle_detection() {
    let t1 = ThoughtData {
        thought: "T1".to_string(), thought_number: 1, total_thoughts: 3, next_thought_needed: true,
        is_revision: None, revises_thought: None, branch_from_thought: None, branch_id: None,
        needs_more_thoughts: None, parent_thoughts: Some(vec![2]), assumptions: None,
        verified_assumptions: None, confidence_score: Some(0.8), criticism: None,
        hypothesis: None, verification_method: None, left_to_be_done: None, timestamp: None, session_id: None,
    };
    let t2 = ThoughtData {
        thought: "T2".to_string(), thought_number: 2, total_thoughts: 3, next_thought_needed: true,
        is_revision: None, revises_thought: None, branch_from_thought: None, branch_id: None,
        needs_more_thoughts: None, parent_thoughts: Some(vec![1]), assumptions: None,
        verified_assumptions: None, confidence_score: Some(0.9), criticism: None,
        hypothesis: None, verification_method: None, left_to_be_done: None, timestamp: None, session_id: None,
    };
    let thoughts = vec![t1, t2];
    assert!(detect_loop(&thoughts).is_some());
}

#[test]
fn test_quality_contradiction() {
    let t1 = ThoughtData {
        thought: "T1".to_string(), thought_number: 1, total_thoughts: 2, next_thought_needed: true,
        is_revision: None, revises_thought: None, branch_from_thought: None, branch_id: None,
        needs_more_thoughts: None, parent_thoughts: None,
        assumptions: Some(vec!["Gravity is constant".to_string()]), verified_assumptions: None,
        confidence_score: Some(0.8), criticism: None, hypothesis: None,
        verification_method: None, left_to_be_done: None, timestamp: None, session_id: None,
    };
    let t2 = ThoughtData {
        thought: "T2".to_string(), thought_number: 2, total_thoughts: 2, next_thought_needed: false,
        is_revision: None, revises_thought: None, branch_from_thought: None, branch_id: None,
        needs_more_thoughts: None, parent_thoughts: None,
        assumptions: None, verified_assumptions: Some(vec!["refuted: Gravity is constant".to_string()]),
        confidence_score: Some(0.7), criticism: None, hypothesis: None,
        verification_method: None, left_to_be_done: None, timestamp: None, session_id: None,
    };
    let thoughts = vec![t1, t2];
    let report = analyze_quality("test", &thoughts);
    assert_eq!(report.contradictions_count, 1);
}
