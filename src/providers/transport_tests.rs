use super::*;

#[test]
fn provider_http_config_preserves_timeout_defaults() {
    assert_eq!(
        ProviderHttpConfig::default(),
        ProviderHttpConfig {
            connect_timeout: Duration::from_secs(15),
            read_timeout: Duration::from_secs(120),
            request_timeout: Duration::from_secs(300),
        }
    );
}

#[test]
fn provider_http_transport_preserves_endpoints() {
    assert_eq!(
        openai_chat_endpoint("https://api.openai.com/v1/"),
        "https://api.openai.com/v1/chat/completions"
    );
    assert_eq!(
        openai_chat_endpoint("https://example.openai.azure.com/openai/deployments/model"),
        "https://example.openai.azure.com/openai/deployments/model"
    );
    assert_eq!(
        anthropic_messages_endpoint("https://api.anthropic.com/"),
        "https://api.anthropic.com/v1/messages"
    );
    assert_eq!(
        anthropic_messages_endpoint("https://proxy.example/v1/"),
        "https://proxy.example/v1/messages"
    );
}
