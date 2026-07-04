use rmcp::handler::server::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::schemars::JsonSchema;
use rmcp::{tool, tool_router, transport::stdio, ServiceExt};
use serde::Deserialize;

pub use octocrab;


#[derive(Clone)]
pub struct GithubMcpServer {
    #[allow(dead_code)]
    tool_router: ToolRouter<Self>,
    client: octocrab::Octocrab,
}

fn mcp_error<E: std::fmt::Display>(err: E) -> rmcp::ErrorData {
    rmcp::ErrorData::internal_error(err.to_string(), None)
}

#[tool_router(server_handler)]
impl GithubMcpServer {
    pub fn new(client: octocrab::Octocrab) -> Self {
        Self {
            tool_router: Self::tool_router(),
            client,
        }
    }

    #[tool(description = "Create a new pull request in a GitHub repository.")]
    pub async fn create_pull_request(
        &self,
        req: Parameters<CreatePullRequestRequest>,
    ) -> Result<String, rmcp::ErrorData> {
        let p = req.0;
        let pulls = self.client.pulls(&p.owner, &p.repo);
        let mut builder = pulls.create(&p.title, &p.head, &p.base);
        if let Some(body) = p.body {
            builder = builder.body(body);
        }

        let pr = builder.send().await.map_err(mcp_error)?;
        Ok(format!(
            "Successfully created Pull Request #{}: {}\nURL: {}",
            pr.number,
            pr.title.clone().unwrap_or_default(),
            pr.html_url.map(|u| u.to_string()).unwrap_or_default()
        ))
    }

    #[tool(description = "Search for issues and pull requests on GitHub.")]
    pub async fn search_issues(
        &self,
        req: Parameters<SearchIssuesRequest>,
    ) -> Result<String, rmcp::ErrorData> {
        let query = &req.0.query;
        let results = self
            .client
            .search()
            .issues_and_pull_requests(query)
            .send()
            .await
            .map_err(mcp_error)?;

        let mut output = String::new();
        output.push_str(&format!(
            "Found {} matching issues/PRs:\n\n",
            results.total_count.unwrap_or(0)
        ));
        for item in results.items {
            output.push_str(&format!(
                "- [#{}] {} (State: {:?}, Type: {})\n  URL: {}\n",
                item.number,
                item.title,
                item.state,
                if item.pull_request.is_some() {
                    "PR"
                } else {
                    "Issue"
                },
                item.html_url
            ));
        }

        Ok(output)
    }

    #[tool(description = "Retrieve comments for a specific GitHub issue or pull request.")]
    pub async fn get_issue_comments(
        &self,
        req: Parameters<GetIssueCommentsRequest>,
    ) -> Result<String, rmcp::ErrorData> {
        let p = req.0;
        let issues = self.client.issues(&p.owner, &p.repo);
        let comments = issues
            .list_comments(p.issue_number)
            .send()
            .await
            .map_err(mcp_error)?;

        let mut output = String::new();
        output.push_str(&format!("Comments for Issue #{}:\n\n", p.issue_number));
        for comment in comments.items {
            output.push_str(&format!(
                "--- Comment by {} at {} ---\n{}\n\n",
                comment.user.login,
                comment.created_at.to_string(),
                comment.body.unwrap_or_default()
            ));
        }

        Ok(output)
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct CreatePullRequestRequest {
    #[schemars(description = "The owner of the repository (e.g. 'octocat').")]
    pub owner: String,
    #[schemars(description = "The name of the repository (e.g. 'hello-world').")]
    pub repo: String,
    #[schemars(description = "The title of the pull request.")]
    pub title: String,
    #[schemars(
        description = "The name of the branch where your changes are (e.g. 'my-new-feature')."
    )]
    pub head: String,
    #[schemars(description = "The name of the branch you want to merge into (e.g. 'main').")]
    pub base: String,
    #[schemars(description = "The body/description of the pull request.")]
    pub body: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct SearchIssuesRequest {
    #[schemars(
        description = "The query string to search for (e.g. 'repo:octocat/hello-world is:open label:bug')."
    )]
    pub query: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct GetIssueCommentsRequest {
    #[schemars(description = "The owner of the repository.")]
    pub owner: String,
    #[schemars(description = "The name of the repository.")]
    pub repo: String,
    #[schemars(description = "The issue or pull request number.")]
    pub issue_number: u64,
}

