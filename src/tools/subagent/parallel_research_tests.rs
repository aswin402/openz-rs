use super::*;
use crate::config::schema::Config;
use crate::session::SessionManager;
use std::sync::Arc;

#[test]
fn test_parallel_research_partial_response_shape() {
    let response = parallel_research_response(vec![
        serde_json::json!({
            "task": "marketplaces",
            "status": "success",
            "summary": "found sources"
        }),
        serde_json::json!({
            "task": "pricing",
            "status": "timeout",
            "error": "Parallel research aggregate deadline reached"
        }),
    ]);

    assert_eq!(response["status"], "partial_success");
    assert_eq!(response["succeeded"], 1);
    assert_eq!(response["failed"], 1);
    assert_eq!(response["results"][0]["summary"], "found sources");
    assert_eq!(response["results"][1]["status"], "timeout");
}

#[test]
fn test_parallel_research_flush_deadline_precedes_child_timeout() {
    assert!(parallel_research_flush_deadline_secs(300) < 300);
    assert_eq!(
        parallel_research_flush_deadline_secs(1),
        1
    );
}

#[test]
fn test_parallel_research_metadata_is_explicit_for_router() {
    let tool = ParallelResearchTool {
        config: Config::default(),
        parent_provider: Arc::new(crate::providers::openai::OpenAIProvider::new(
            "mock_key".to_string(),
            "mock_base".to_string(),
            "gpt-4o-mini".to_string(),
        )),
        session_manager: SessionManager::new(std::env::temp_dir()),
        parent_tools: Vec::new(),
        cancellation_token: CancellationToken::new(),
        capability_policy: None,
    };

    let metadata = tool.metadata();

    assert_eq!(metadata.domain, "subagent");
    assert_eq!(metadata.risk, crate::tools::ToolRisk::Medium);
    assert!(!metadata.spawns_process); // in-process child agent loop, no OS process
    assert!(!metadata.requires_approval);
    assert_eq!(metadata.priority, 100);
    assert_eq!(metadata.recommended_timeout_secs, Some(600));
}
