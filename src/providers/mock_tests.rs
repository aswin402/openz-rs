use super::*;
use crate::providers::LLMProvider;

#[tokio::test]
async fn test_mock_provider_default_response() {
    let provider = MockProvider::new();
    let resp = provider
        .chat(
            "test",
            &[],
            &[],
            &crate::providers::GenerationSettings {
                temperature: 0.0,
                max_tokens: 100,
                reasoning_effort: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(resp.content.unwrap(), "mock default response");
    assert_eq!(provider.call_count(), 1);
}

#[tokio::test]
async fn test_mock_provider_response_sequence() {
    let provider = MockProvider::new()
        .with_response(MockResponse::text("first"))
        .with_response(MockResponse::text("second"));

    let settings = crate::providers::GenerationSettings {
        temperature: 0.0,
        max_tokens: 100,
        reasoning_effort: None,
    };

    let r1 = provider.chat("", &[], &[], &settings).await.unwrap();
    assert_eq!(r1.content.unwrap(), "first");

    let r2 = provider.chat("", &[], &[], &settings).await.unwrap();
    assert_eq!(r2.content.unwrap(), "second");

    // Third call falls back to default.
    let r3 = provider.chat("", &[], &[], &settings).await.unwrap();
    assert_eq!(r3.content.unwrap(), "mock default response");

    assert_eq!(provider.call_count(), 3);
}

#[tokio::test]
async fn test_mock_provider_inject_errors() {
    let provider = MockProvider::new()
        .with_response(MockResponse::text("ok"))
        .with_errors(2);

    let settings = crate::providers::GenerationSettings {
        temperature: 0.0,
        max_tokens: 100,
        reasoning_effort: None,
    };

    // First call — error injected.
    assert!(provider.chat("", &[], &[], &settings).await.is_err());
    // Second call — error injected.
    assert!(provider.chat("", &[], &[], &settings).await.is_err());
    // Third call — succeeds with the canned response.
    let r3 = provider.chat("", &[], &[], &settings).await.unwrap();
    assert_eq!(r3.content.unwrap(), "ok");
    // Fourth call — default.
    let r4 = provider.chat("", &[], &[], &settings).await.unwrap();
    assert_eq!(r4.content.unwrap(), "mock default response");
}

#[tokio::test]
async fn test_mock_provider_tool_call() {
    let provider = MockProvider::new().with_response(MockResponse::tool_call(
        "get_weather",
        serde_json::json!({"city": "NYC"}),
    ));

    let resp = provider
        .chat(
            "",
            &[],
            &[],
            &crate::providers::GenerationSettings {
                temperature: 0.0,
                max_tokens: 100,
                reasoning_effort: None,
            },
        )
        .await
        .unwrap();

    assert_eq!(resp.finish_reason, "tool_calls");
    assert_eq!(resp.tool_calls.len(), 1);
    assert_eq!(resp.tool_calls[0].name, "get_weather");
    assert_eq!(resp.tool_calls[0].arguments["city"], "NYC");
}
