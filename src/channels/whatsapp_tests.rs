use super::*;
use axum::response::IntoResponse;

#[test]
fn whatsapp_uses_shared_stop_command_detection() {
    assert!(crate::channels::is_stop_command("/stop"));
    assert!(!crate::channels::is_stop_command("please stop"));
}

#[tokio::test]
async fn test_verify_webhook_success() {
    let mut params = HashMap::new();
    params.insert("hub.mode".to_string(), "subscribe".to_string());
    params.insert("hub.verify_token".to_string(), "my_test_token".to_string());
    params.insert("hub.challenge".to_string(), "12345".to_string());

    let mut config = crate::config::schema::Config::default();
    config.agents.defaults.provider = "ollama".to_string();
    config.agents.defaults.model = "ollama/llama3".to_string();
    let agent_loop = crate::cli::build_agent_loop(config).await.unwrap();

    let state = WhatsAppState {
        agent_loop: Arc::new(agent_loop),
        api_key: "key".to_string(),
        phone_number_id: "phone".to_string(),
        verify_token: "my_test_token".to_string(),
        app_secret: String::new(),
        client: crate::core::http::default_http_client(),
        concurrency_limit: Arc::new(tokio::sync::Semaphore::new(5)),
    };

    let response = verify_webhook(Query(params), State(state))
        .await
        .into_response();
    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(response.into_body(), 1000)
        .await
        .unwrap();
    let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
    assert_eq!(body_str, "12345");
}

#[tokio::test]
async fn test_verify_webhook_failure() {
    let mut params = HashMap::new();
    params.insert("hub.mode".to_string(), "subscribe".to_string());
    params.insert("hub.verify_token".to_string(), "wrong_token".to_string());
    params.insert("hub.challenge".to_string(), "12345".to_string());

    let mut config = crate::config::schema::Config::default();
    config.agents.defaults.provider = "ollama".to_string();
    config.agents.defaults.model = "ollama/llama3".to_string();
    let agent_loop = crate::cli::build_agent_loop(config).await.unwrap();

    let state = WhatsAppState {
        agent_loop: Arc::new(agent_loop),
        api_key: "key".to_string(),
        phone_number_id: "phone".to_string(),
        verify_token: "my_test_token".to_string(),
        app_secret: String::new(),
        client: crate::core::http::default_http_client(),
        concurrency_limit: Arc::new(tokio::sync::Semaphore::new(5)),
    };

    let response = verify_webhook(Query(params), State(state))
        .await
        .into_response();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}
