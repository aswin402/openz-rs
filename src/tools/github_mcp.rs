use crate::tools::github::{build_git_http_client, configured_git_token};
use crate::tools::Tool;
use anyhow::{anyhow, Result};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, USER_AGENT};
use serde_json::{json, Value};
use std::env;

fn resolve_github_token() -> Option<String> {
    let config = crate::config::loader::load_config().ok();
    let configured_github = config.as_ref().and_then(|c| c.integrations.github.as_ref());
    env::var("GITHUB_TOKEN")
        .ok()
        .or_else(|| env::var("OCTOCRAB_TOKEN").ok())
        .or_else(|| env::var("GITHUB_PAT").ok())
        .or_else(|| configured_git_token(configured_github))
}

fn get_github_headers(token: Option<&str>) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static("openz"));
    headers.insert(
        ACCEPT,
        HeaderValue::from_static("application/vnd.github+json"),
    );
    match token.map(str::trim).filter(|t| !t.is_empty()) {
        Some(tok) => {
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {}", tok))?,
            );
        }
        None => {}
    }
    Ok(headers)
}

async fn execute_github_mcp_request<F>(
    url: &str,
    token: Option<&str>,
    post_payload: Option<&Value>,
    format_json: F,
) -> Value
where
    F: FnOnce(&Value) -> String,
{
    match crate::tools::web::validate_url(url).await {
        Ok(_) => {}
        Err(e) => return json!({ "success": false, "error": format!("SSRF check failed: {}", e) }),
    }

    let client = match build_git_http_client() {
        Ok(c) => c,
        Err(e) => return json!({ "success": false, "error": e.to_string() }),
    };
    let headers = match get_github_headers(token) {
        Ok(h) => h,
        Err(e) => return json!({ "success": false, "error": e.to_string() }),
    };

    let request = match post_payload {
        Some(payload) => client.post(url).headers(headers).json(payload),
        None => client.get(url).headers(headers),
    };

    match request.send().await {
        Ok(resp) => {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            match status.is_success() {
                false => json!({
                    "success": false,
                    "error": format!("GitHub API Error ({}): {}", status, text)
                }),
                true => match serde_json::from_str::<Value>(&text) {
                    Ok(val) => json!({ "success": true, "result": format_json(&val) }),
                    Err(_) => json!({ "success": true, "result": text }),
                },
            }
        }
        Err(e) => json!({ "success": false, "error": e.to_string() }),
    }
}

pub struct GithubCreatePullRequestTool;

#[async_trait::async_trait]
impl Tool for GithubCreatePullRequestTool {
    fn name(&self) -> &str {
        "github_create_pull_request"
    }

    fn description(&self) -> &str {
        "Create a new pull request in a GitHub repository."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "owner": {
                    "type": "string",
                    "description": "The owner of the repository (e.g. 'octocat')."
                },
                "repo": {
                    "type": "string",
                    "description": "The name of the repository (e.g. 'hello-world')."
                },
                "title": {
                    "type": "string",
                    "description": "The title of the pull request."
                },
                "head": {
                    "type": "string",
                    "description": "The name of the branch where your changes are (e.g. 'my-new-feature')."
                },
                "base": {
                    "type": "string",
                    "description": "The name of the branch you want to merge into (e.g. 'main')."
                },
                "body": {
                    "type": "string",
                    "description": "The body/description of the pull request."
                }
            },
            "required": ["owner", "repo", "title", "head", "base"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let owner = arguments
            .get("owner")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'owner' parameter"))?;
        let repo = arguments
            .get("repo")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'repo' parameter"))?;
        let title = arguments
            .get("title")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'title' parameter"))?;
        let head = arguments
            .get("head")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'head' parameter"))?;
        let base = arguments
            .get("base")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'base' parameter"))?;
        let body = arguments.get("body").and_then(|v| v.as_str());

        let token = match resolve_github_token() {
            Some(tok) => tok,
            None => {
                return Ok(json!({
                    "success": false,
                    "error": "GitHub token not found. Set GITHUB_TOKEN or GITHUB_PAT."
                }));
            }
        };

        let url = format!("https://api.github.com/repos/{}/{}/pulls", owner, repo);
        let mut payload = json!({
            "title": title,
            "head": head,
            "base": base,
        });
        match body {
            Some(b) => {
                payload["body"] = json!(b);
            }
            None => {}
        }

        Ok(execute_github_mcp_request(
            &url,
            Some(&token),
            Some(&payload),
            |val| {
                let number = val.get("number").and_then(|v| v.as_i64()).unwrap_or(0);
                let pr_title = val.get("title").and_then(|v| v.as_str()).unwrap_or(title);
                let html_url = val.get("html_url").and_then(|v| v.as_str()).unwrap_or("");
                format!(
                    "Successfully created Pull Request #{}: {}\nURL: {}",
                    number, pr_title, html_url
                )
            },
        )
        .await)
    }
}

pub struct GithubSearchIssuesTool;

#[async_trait::async_trait]
impl Tool for GithubSearchIssuesTool {
    fn name(&self) -> &str {
        "github_search_issues"
    }

    fn description(&self) -> &str {
        "Search for issues and pull requests on GitHub."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The query string to search for (e.g. 'repo:octocat/hello-world is:open label:bug')."
                }
            },
            "required": ["query"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let query = arguments
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'query' parameter"))?;

        let token = resolve_github_token();
        let encoded_query = percent_encoding::utf8_percent_encode(
            query,
            percent_encoding::NON_ALPHANUMERIC,
        )
        .to_string();
        let url = format!("https://api.github.com/search/issues?q={}", encoded_query);

        Ok(execute_github_mcp_request(&url, token.as_deref(), None, |val| {
            let total_count = val.get("total_count").and_then(|v| v.as_u64()).unwrap_or(0);
            let mut output = format!("Found {} matching issues/PRs:\n\n", total_count);
            for item in val
                .get("items")
                .and_then(|v| v.as_array())
                .into_iter()
                .flatten()
            {
                let number = item.get("number").and_then(|v| v.as_i64()).unwrap_or(0);
                let title = item.get("title").and_then(|v| v.as_str()).unwrap_or("");
                let state = item.get("state").and_then(|v| v.as_str()).unwrap_or("");
                let item_type = match item.get("pull_request").is_some() {
                    true => "PR",
                    false => "Issue",
                };
                let html_url = item.get("html_url").and_then(|v| v.as_str()).unwrap_or("");
                output.push_str(&format!(
                    "- [#{}] {} (State: {:?}, Type: {})\n  URL: {}\n",
                    number, title, state, item_type, html_url
                ));
            }
            output
        })
        .await)
    }
}

pub struct GithubGetIssueCommentsTool;

#[async_trait::async_trait]
impl Tool for GithubGetIssueCommentsTool {
    fn name(&self) -> &str {
        "github_get_issue_comments"
    }

    fn description(&self) -> &str {
        "Retrieve comments for a specific GitHub issue or pull request."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "owner": {
                    "type": "string",
                    "description": "The owner of the repository."
                },
                "repo": {
                    "type": "string",
                    "description": "The name of the repository."
                },
                "issue_number": {
                    "type": "integer",
                    "description": "The issue or pull request number."
                }
            },
            "required": ["owner", "repo", "issue_number"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let owner = arguments
            .get("owner")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'owner' parameter"))?;
        let repo = arguments
            .get("repo")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'repo' parameter"))?;
        let issue_number = arguments
            .get("issue_number")
            .or_else(|| arguments.get("number"))
            .and_then(|v| {
                v.as_i64()
                    .or_else(|| v.as_str().and_then(|s| s.trim().parse::<i64>().ok()))
            })
            .ok_or_else(|| anyhow!("Missing 'issue_number' parameter"))?;

        let token = resolve_github_token();
        let url = format!(
            "https://api.github.com/repos/{}/{}/issues/{}/comments",
            owner, repo, issue_number
        );

        Ok(execute_github_mcp_request(&url, token.as_deref(), None, |val| {
            let mut output = format!("Comments for Issue #{}:\n\n", issue_number);
            for comment in val.as_array().into_iter().flatten() {
                let user = comment
                    .get("user")
                    .and_then(|u| u.get("login"))
                    .and_then(|l| l.as_str())
                    .unwrap_or("unknown");
                let created_at = comment
                    .get("created_at")
                    .and_then(|c| c.as_str())
                    .unwrap_or("");
                let body = comment
                    .get("body")
                    .and_then(|b| b.as_str())
                    .unwrap_or("");
                output.push_str(&format!(
                    "--- Comment by {} at {} ---\n{}\n\n",
                    user, created_at, body
                ));
            }
            output
        })
        .await)
    }
}

#[cfg(test)]
#[path = "github_mcp_tests.rs"]
mod tests;
