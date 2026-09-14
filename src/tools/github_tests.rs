use super::*;
use serde_json::json;

#[tokio::test]
async fn test_git_provider_validation() {
    let tool = GitProviderTool;

    // Missing action
    let res = tool
        .call(&json!({
            "repo": "owner/repo"
        }))
        .await;
    assert!(res.is_err());

    // Missing repo
    let res = tool
        .call(&json!({
            "action": "list_issues"
        }))
        .await;
    assert!(res.is_err());
}

#[tokio::test]
async fn test_git_provider_ssrf_blocking() {
    let tool = GitProviderTool;

    // SSRF target: local loopback
    let res = tool
        .call(&json!({
            "action": "list_issues",
            "repo": "owner/repo",
            "api_base": "http://127.0.0.1:8080",
            "token": "dummy_token"
        }))
        .await;
    assert!(
        res.is_err(),
        "Loopback API base should be blocked by SSRF filter"
    );

    // SSRF target: local domain
    let res = tool
        .call(&json!({
            "action": "list_issues",
            "repo": "owner/repo",
            "api_base": "http://localhost:8080",
            "token": "dummy_token"
        }))
        .await;
    assert!(
        res.is_err(),
        "Localhost API base should be blocked by SSRF filter"
    );
}
