/// Configurable mock LLM provider for testing.
///
/// This module is only compiled under `#[cfg(test)]`. It provides
/// [`MockProvider`] which implements [`LLMProvider`] and supports:
///
/// - Pre-defined response sequences (one response returned per call, cycling
///   through the sequence)
/// - Default response when the sequence is exhausted
/// - Inspecting call history for assertions
/// - Custom error injection
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// A single canned response returned by [`MockProvider`].
#[derive(Debug, Clone)]
pub struct MockResponse {
    pub content: Option<String>,
    pub tool_calls: Vec<crate::providers::ToolCallRequest>,
    pub finish_reason: String,
    pub reasoning_content: Option<String>,
}

impl MockResponse {
    pub fn text(content: impl Into<String>) -> Self {
        Self {
            content: Some(content.into()),
            tool_calls: Vec::new(),
            finish_reason: "stop".to_string(),
            reasoning_content: None,
        }
    }

    pub fn tool_call(name: impl Into<String>, args: serde_json::Value) -> Self {
        Self {
            content: None,
            tool_calls: vec![crate::providers::ToolCallRequest {
                id: "mock_call_1".into(),
                name: name.into(),
                arguments: args,
            }],
            finish_reason: "tool_calls".to_string(),
            reasoning_content: None,
        }
    }

    pub fn error() -> Self {
        Self {
            content: None,
            tool_calls: Vec::new(),
            finish_reason: "error".to_string(),
            reasoning_content: None,
        }
    }
}

impl From<crate::providers::LLMResponse> for MockResponse {
    fn from(r: crate::providers::LLMResponse) -> Self {
        Self {
            content: r.content,
            tool_calls: r.tool_calls,
            finish_reason: r.finish_reason,
            reasoning_content: r.reasoning_content,
        }
    }
}

impl From<MockResponse> for crate::providers::LLMResponse {
    fn from(r: MockResponse) -> Self {
        Self {
            content: r.content,
            tool_calls: r.tool_calls,
            finish_reason: r.finish_reason,
            reasoning_content: r.reasoning_content,
        }
    }
}

/// A mock LLM provider with configurable response sequences.
///
/// # Usage
///
/// ```ignore
/// let provider = MockProvider::new()
///     .with_response(MockResponse::text("Hello"))
///     .with_response(MockResponse::tool_call("get_weather", json!({"city": "NYC"})));
///
/// // First call returns "Hello", second call returns the tool call, third returns default.
/// ```
#[derive(Debug)]
pub struct MockProvider {
    /// Ordered list of responses. Index advances on each successful `chat()` call.
    responses: Vec<MockResponse>,
    /// Call counter used to index into `responses`.
    call_count: Arc<AtomicUsize>,
    /// Default response returned when `call_count` exceeds `responses.len()`.
    default_response: MockResponse,
    /// Errors remaining to inject (consumed before incrementing `call_count`).
    inject_errors: Arc<AtomicUsize>,
}

impl Default for MockProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl MockProvider {
    pub fn new() -> Self {
        Self {
            responses: Vec::new(),
            call_count: Arc::new(AtomicUsize::new(0)),
            default_response: MockResponse::text("mock default response"),
            inject_errors: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Append a canned response to the sequence.
    pub fn with_response(mut self, response: MockResponse) -> Self {
        self.responses.push(response);
        self
    }

    /// Set the default response (used when the response sequence is exhausted).
    pub fn with_default(mut self, response: MockResponse) -> Self {
        self.default_response = response;
        self
    }

    /// Inject errors for the next `n` calls (returns `Err(anyhow!(...))`).
    pub fn with_errors(mut self, n: usize) -> Self {
        self.inject_errors = Arc::new(AtomicUsize::new(n));
        self
    }

    /// Return the total number of `chat()` calls made so far.
    pub fn call_count(&self) -> usize {
        self.call_count.load(Ordering::Relaxed)
    }

    pub fn into_call_count(self) -> Arc<AtomicUsize> {
        self.call_count
    }
}

#[async_trait::async_trait]
impl crate::providers::LLMProvider for MockProvider {
    async fn chat(
        &self,
        _system_prompt: &str,
        _messages: &[crate::session::Message],
        _tools: &[serde_json::Value],
        _settings: &crate::providers::GenerationSettings,
    ) -> anyhow::Result<crate::providers::LLMResponse> {
        // Inject error if remaining (without advancing the response counter).
        // Use fetch_update for atomic compare-and-swap to avoid race conditions.
        let prev = self.inject_errors.fetch_update(
            Ordering::SeqCst,
            Ordering::SeqCst,
            |current| {
                if current > 0 {
                    Some(current - 1)
                } else {
                    None
                }
            },
        );
        if prev.is_ok() {
            anyhow::bail!("MockProvider injected error");
        }

        let count = self.call_count.fetch_add(1, Ordering::SeqCst);

        let response = self
            .responses
            .get(count)
            .unwrap_or(&self.default_response)
            .clone();

        Ok(response.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::LLMProvider;

    #[tokio::test]
    async fn test_mock_provider_default_response() {
        let provider = MockProvider::new();
        let resp = provider
            .chat("test", &[], &[], &crate::providers::GenerationSettings {
                temperature: 0.0,
                max_tokens: 100,
                reasoning_effort: None,
            })
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
        let provider = MockProvider::new()
            .with_response(MockResponse::tool_call("get_weather", serde_json::json!({"city": "NYC"})));

        let resp = provider
            .chat("", &[], &[], &crate::providers::GenerationSettings {
                temperature: 0.0,
                max_tokens: 100,
                reasoning_effort: None,
            })
            .await
            .unwrap();

        assert_eq!(resp.finish_reason, "tool_calls");
        assert_eq!(resp.tool_calls.len(), 1);
        assert_eq!(resp.tool_calls[0].name, "get_weather");
        assert_eq!(resp.tool_calls[0].arguments["city"], "NYC");
    }
}
