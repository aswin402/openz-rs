use super::*;

fn fake_run_result(content: &str) -> crate::agent::agent_loop::RunResult {
    crate::agent::agent_loop::RunResult {
        content: content.to_string(),
        tools_used: Vec::new(),
        streamed: false,
    }
}

#[test]
fn test_schema_retry_accepts_valid_fenced_json() {
    let schema = serde_json::json!({
        "type": "object",
        "properties": { "answer": { "type": "string" } },
        "required": ["answer"]
    });

    let decision = evaluate_schema_retry(
        r#"```json
{"answer":"ok"}
```"#,
        &schema,
        0,
        2,
    )
    .unwrap();

    assert_eq!(
        decision,
        SchemaRetryDecision::Accepted(r#"{"answer":"ok"}"#.to_string())
    );
}

#[test]
fn test_schema_retry_retries_invalid_json_before_limit() {
    let schema = serde_json::json!({ "type": "object" });

    let decision = evaluate_schema_retry("not json", &schema, 0, 2).unwrap();

    match decision {
        SchemaRetryDecision::Retry { prompt, reason } => {
            assert!(reason.contains("Parse Error"));
            assert!(prompt.contains("not valid JSON"));
        }
        other => panic!("expected retry decision, got {other:?}"),
    }
}

#[test]
fn test_schema_retry_errors_invalid_json_at_limit() {
    let schema = serde_json::json!({ "type": "object" });

    let err = evaluate_schema_retry("not json", &schema, 2, 2).unwrap_err();

    assert!(err.to_string().contains("failed to parse as JSON"));
}

#[test]
fn test_schema_retry_retries_schema_mismatch_before_limit() {
    let schema = serde_json::json!({
        "type": "object",
        "properties": { "answer": { "type": "string" } },
        "required": ["answer"]
    });

    let decision = evaluate_schema_retry("{}", &schema, 0, 2).unwrap();

    match decision {
        SchemaRetryDecision::Retry { prompt, reason } => {
            assert!(reason.contains("Missing required field"));
            assert!(prompt.contains("did not conform"));
        }
        other => panic!("expected retry decision, got {other:?}"),
    }
}

#[tokio::test]
async fn schema_retry_loop_accepts_fenced_json_and_cleans_content() {
    let initial = Ok(fake_run_result("```json\n{\"answer\": 42}\n```"));
    let mut rerun_calls = 0;
    let result = execute_with_schema_retries(
        initial,
        &serde_json::json!({"type": "object", "properties": {"answer": {"type": "number"}}, "required": ["answer"]}),
        |_prompt| {
            rerun_calls += 1;
            std::future::ready(Ok(fake_run_result("{\"answer\": 42}")))
        },
    )
    .await
    .unwrap();

    assert_eq!(rerun_calls, 0);
    assert_eq!(result.content, "{\"answer\": 42}");
}

#[tokio::test]
async fn schema_retry_loop_retries_then_accepts() {
    // First response violates the schema; the corrected rerun is accepted.
    let initial = Ok(fake_run_result("{\"wrong_field\": true}"));
    let mut rerun_calls = 0;
    let result = execute_with_schema_retries(
        initial,
        &serde_json::json!({"type": "object", "properties": {"answer": {"type": "number"}}, "required": ["answer"]}),
        |prompt| {
            rerun_calls += 1;
            assert!(prompt.contains("did not conform to the JSON Schema"));
            std::future::ready(Ok(fake_run_result("{\"answer\": 7}")))
        },
    )
    .await
    .unwrap();

    assert_eq!(rerun_calls, 1);
    assert_eq!(result.content, "{\"answer\": 7}");
}

#[tokio::test]
async fn schema_retry_loop_errors_after_attempt_limit() {
    // Always-invalid JSON: attempt 1 and 2 retry, then evaluate returns Err at the limit.
    let initial = Ok(fake_run_result("not json at all"));
    let mut rerun_calls = 0;
    let result = execute_with_schema_retries(
        initial,
        &serde_json::json!({"type": "object"}),
        |_prompt| {
            rerun_calls += 1;
            std::future::ready(Ok(fake_run_result("still not json")))
        },
    )
    .await;

    assert!(result.is_err());
    assert!(result
        .err()
        .expect("schema retry must fail at attempt limit")
        .to_string()
        .contains("failed to parse as JSON"));
    // initial evaluation (attempt 0) + 2 reruns evaluated at attempts 1 and 2
    assert_eq!(rerun_calls, 2);
}
