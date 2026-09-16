use super::*;

#[tokio::test]
async fn test_server_init() {
    let server = get_server();
    let _ = server
        .create_pull_request(Parameters(CreatePullRequestRequest {
            owner: "test".to_string(),
            repo: "test".to_string(),
            title: "test".to_string(),
            head: "test".to_string(),
            base: "test".to_string(),
            body: None,
        }))
        .await;
}
