use anyhow::Result;
use serde_json::{json, Value};
use openz_github_mcp::{GithubMcpServer, CreatePullRequestRequest, SearchIssuesRequest, GetIssueCommentsRequest};
use rmcp::handler::server::wrapper::Parameters;

pub fn get_server() -> &'static GithubMcpServer {
    static SERVER: std::sync::OnceLock<GithubMcpServer> = std::sync::OnceLock::new();
    SERVER.get_or_init(|| {
        let token = std::env::var("GITHUB_TOKEN")
            .or_else(|_| std::env::var("OCTOCRAB_TOKEN"))
            .unwrap_or_default();

        let mut builder = openz_github_mcp::octocrab::Octocrab::builder();
        if !token.is_empty() {
            builder = builder.personal_token(token);
        }
        let client = builder.build().unwrap_or_else(|_| openz_github_mcp::octocrab::Octocrab::default());
        GithubMcpServer::new(client)
    })
}

pub struct GithubCreatePullRequestTool;
#[async_trait::async_trait]
impl crate::tools::Tool for GithubCreatePullRequestTool {
    fn name(&self) -> &str {
        "github_create_pull_request"
    }

    fn description(&self) -> &str {
        "Create a new pull request in a GitHub repository."
    }

    fn parameters(&self) -> Value {
        let schema = schemars::schema_for!(CreatePullRequestRequest);
        serde_json::to_value(schema).unwrap_or_else(|_| json!({}))
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let p: CreatePullRequestRequest = serde_json::from_value(arguments.clone())?;
        match get_server().create_pull_request(Parameters(p)).await {
            Ok(res_str) => Ok(json!({ "success": true, "result": res_str })),
            Err(e) => Ok(json!({ "success": false, "error": e.message }))
        }
    }
}

pub struct GithubSearchIssuesTool;
#[async_trait::async_trait]
impl crate::tools::Tool for GithubSearchIssuesTool {
    fn name(&self) -> &str {
        "github_search_issues"
    }

    fn description(&self) -> &str {
        "Search for issues and pull requests on GitHub."
    }

    fn parameters(&self) -> Value {
        let schema = schemars::schema_for!(SearchIssuesRequest);
        serde_json::to_value(schema).unwrap_or_else(|_| json!({}))
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let p: SearchIssuesRequest = serde_json::from_value(arguments.clone())?;
        match get_server().search_issues(Parameters(p)).await {
            Ok(res_str) => Ok(json!({ "success": true, "result": res_str })),
            Err(e) => Ok(json!({ "success": false, "error": e.message }))
        }
    }
}

pub struct GithubGetIssueCommentsTool;
#[async_trait::async_trait]
impl crate::tools::Tool for GithubGetIssueCommentsTool {
    fn name(&self) -> &str {
        "github_get_issue_comments"
    }

    fn description(&self) -> &str {
        "Retrieve comments for a specific GitHub issue or pull request."
    }

    fn parameters(&self) -> Value {
        let schema = schemars::schema_for!(GetIssueCommentsRequest);
        serde_json::to_value(schema).unwrap_or_else(|_| json!({}))
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let p: GetIssueCommentsRequest = serde_json::from_value(arguments.clone())?;
        match get_server().get_issue_comments(Parameters(p)).await {
            Ok(res_str) => Ok(json!({ "success": true, "result": res_str })),
            Err(e) => Ok(json!({ "success": false, "error": e.message }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_server_init() {
        let server = get_server();
        let _ = server.create_pull_request(Parameters(CreatePullRequestRequest {
            owner: "test".to_string(),
            repo: "test".to_string(),
            title: "test".to_string(),
            head: "test".to_string(),
            base: "test".to_string(),
            body: None,
        })).await;
    }
}
