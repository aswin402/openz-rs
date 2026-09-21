use super::*;

#[tokio::test]
async fn test_github_create_pull_request_tool_metadata() {
    let tool = GithubCreatePullRequestTool;
    assert_eq!(tool.name(), "github_create_pull_request");
    assert!(tool.description().contains("pull request"));

    let params = tool.parameters();
    assert_eq!(params["type"], "object");
    assert!(params["properties"]["owner"].is_object());
    assert!(params["properties"]["repo"].is_object());
    assert!(params["properties"]["title"].is_object());
    assert!(params["properties"]["head"].is_object());
    assert!(params["properties"]["base"].is_object());
}

#[tokio::test]
async fn test_github_search_issues_tool_metadata() {
    let tool = GithubSearchIssuesTool;
    assert_eq!(tool.name(), "github_search_issues");
    assert!(tool.description().contains("Search"));

    let params = tool.parameters();
    assert_eq!(params["type"], "object");
    assert!(params["properties"]["query"].is_object());
}

#[tokio::test]
async fn test_github_get_issue_comments_tool_metadata() {
    let tool = GithubGetIssueCommentsTool;
    assert_eq!(tool.name(), "github_get_issue_comments");
    assert!(tool.description().contains("comments"));

    let params = tool.parameters();
    assert_eq!(params["type"], "object");
    assert!(params["properties"]["owner"].is_object());
    assert!(params["properties"]["repo"].is_object());
    assert!(params["properties"]["issue_number"].is_object());
}

#[tokio::test]
async fn test_github_tools_parameter_validation() {
    let pr_tool = GithubCreatePullRequestTool;
    let res = pr_tool.call(&json!({})).await;
    assert!(res.is_err(), "Missing owner should error");

    let search_tool = GithubSearchIssuesTool;
    let res = search_tool.call(&json!({})).await;
    assert!(res.is_err(), "Missing query should error");

    let comments_tool = GithubGetIssueCommentsTool;
    let res = comments_tool.call(&json!({ "owner": "test", "repo": "test" })).await;
    assert!(res.is_err(), "Missing issue_number should error");
}
