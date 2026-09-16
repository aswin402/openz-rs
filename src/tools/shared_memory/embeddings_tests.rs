use super::*;
use crate::config::schema::ProviderConfig;

#[test]
fn test_resolve_cohere_embed_url() {
    let mut config = ProviderConfig::default();
    assert_eq!(
        resolve_cohere_embed_url(&config),
        "https://api.cohere.com/v1/embed"
    );

    config.api_base = Some("".to_string());
    assert_eq!(
        resolve_cohere_embed_url(&config),
        "https://api.cohere.com/v1/embed"
    );

    config.api_base = Some("https://proxy.example.com".to_string());
    assert_eq!(
        resolve_cohere_embed_url(&config),
        "https://proxy.example.com/v1/embed"
    );

    config.api_base = Some("https://proxy.example.com/".to_string());
    assert_eq!(
        resolve_cohere_embed_url(&config),
        "https://proxy.example.com/v1/embed"
    );

    config.api_base = Some("https://proxy.example.com/v1".to_string());
    assert_eq!(
        resolve_cohere_embed_url(&config),
        "https://proxy.example.com/v1/embed"
    );

    config.api_base = Some("https://proxy.example.com/v1/embed".to_string());
    assert_eq!(
        resolve_cohere_embed_url(&config),
        "https://proxy.example.com/v1/embed"
    );
}
