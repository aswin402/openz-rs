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
        .or_else(|| {
            configured_github.and_then(|cfg| {
                cfg.token_env
                    .as_deref()
                    .and_then(|e| env::var(e).ok())
                    .filter(|t| !t.trim().is_empty())
                    .or_else(|| cfg.token.clone().filter(|t| !t.trim().is_empty()))
            })
        })
}

fn get_github_client() -> Result<reqwest::Client> {
    let redirect_policy = reqwest::redirect::Policy::custom(|attempt| {
        if crate::tools::web::validate_url_sync(attempt.url()).is_err() {
            attempt.stop()
        } else {
            attempt.follow()
        }
    });

    reqwest::Client::builder()
        .use_rustls_tls()
        .connect_timeout(crate::core::http::DEFAULT_CONNECT_TIMEOUT)
        .timeout(crate::core::http::DEFAULT_REQUEST_TIMEOUT)
        .redirect(redirect_policy)
        .build()
        .map_err(|e| anyhow!("Failed to build HTTP client: {}", e))
}

fn get_github_headers(token: Option<&str>) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static("openz"));
    headers.insert(
        ACCEPT,
        HeaderValue::from_static("application/vnd.github+json"),
    );
    if let Some(tok) = token {
        if !tok.trim().is_empty() {
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {}", tok.trim()))?,
            );
        }
    }
    Ok(headers)
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
        if let Err(e) = crate::tools::web::validate_url(&url).await {
            return Ok(json!({ "success": false, "error": format!("SSRF check failed: {}", e) }));
        }

        let client = match get_github_client() {
            Ok(c) => c,
            Err(e) => return Ok(json!({ "success": false, "error": e.to_string() })),
        };
        let headers = match get_github_headers(Some(&token)) {
            Ok(h) => h,
            Err(e) => return Ok(json!({ "success": false, "error": e.to_string() })),
        };

        let mut payload = json!({
            "title": title,
            "head": head,
            "base": base,
        });
        if let Some(b) = body {
            payload["body"] = json!(b);
        }

        match client.post(&url).headers(headers).json(&payload).send().await {
            Ok(resp) => {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                if !status.is_success() {
                    Ok(json!({
                        "success": false,
                        "error": format!("GitHub API Error ({}): {}", status, text)
                    }))
                } else if let Ok(val) = serde_json::from_str::<Value>(&text) {
                    let number = val.get("number").and_then(|v| v.as_i64()).unwrap_or(0);
                    let pr_title = val.get("title").and_then(|v| v.as_str()).unwrap_or(title);
                    let html_url = val.get("html_url").and_then(|v| v.as_str()).unwrap_or("");
                    let msg = format!(
                        "Successfully created Pull Request #{}: {}\nURL: {}",
                        number, pr_title, html_url
                    );
                    Ok(json!({ "success": true, "result": msg }))
                } else {
                    Ok(json!({ "success": true, "result": text }))
                }
            }
            Err(e) => Ok(json!({ "success": false, "error": e.to_string() })),
        }
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

        if let Err(e) = crate::tools::web::validate_url(&url).await {
            return Ok(json!({ "success": false, "error": format!("SSRF check failed: {}", e) }));
        }

        let client = match get_github_client() {
            Ok(c) => c,
            Err(e) => return Ok(json!({ "success": false, "error": e.to_string() })),
        };
        let headers = match get_github_headers(token.as_deref()) {
            Ok(h) => h,
            Err(e) => return Ok(json!({ "success": false, "error": e.to_string() })),
        };

        match client.get(&url).headers(headers).send().await {
            Ok(resp) => {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                if !status.is_success() {
                    Ok(json!({
                        "success": false,
                        "error": format!("GitHub API Error ({}): {}", status, text)
                    }))
                } else if let Ok(val) = serde_json::from_str::<Value>(&text) {
                    let total_count = val.get("total_count").and_then(|v| v.as_u64()).unwrap_or(0);
                    let mut output = format!("Found {} matching issues/PRs:\n\n", total_count);
                    if let Some(items) = val.get("items").and_then(|v| v.as_array()) {
                        for item in items {
                            let number = item.get("number").and_then(|v| v.as_i64()).unwrap_or(0);
                            let title = item.get("title").and_then(|v| v.as_str()).unwrap_or("");
                            let state = item.get("state").and_then(|v| v.as_str()).unwrap_or("");
                            let is_pr = item.get("pull_request").is_some();
                            let html_url = item.get("html_url").and_then(|v| v.as_str()).unwrap_or("");
                            output.push_str(&format!(
                                "- [#{}] {} (State: {:?}, Type: {})\n  URL: {}\n",
                                number,
                                title,
                                state,
                                if is_pr { "PR" } else { "Issue" },
                                html_url
                            ));
                        }
                    }
                    Ok(json!({ "success": true, "result": output }))
                } else {
                    Ok(json!({ "success": true, "result": text }))
                }
            }
            Err(e) => Ok(json!({ "success": false, "error": e.to_string() })),
        }
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

        if let Err(e) = crate::tools::web::validate_url(&url).await {
            return Ok(json!({ "success": false, "error": format!("SSRF check failed: {}", e) }));
        }

        let client = match get_github_client() {
            Ok(c) => c,
            Err(e) => return Ok(json!({ "success": false, "error": e.to_string() })),
        };
        let headers = match get_github_headers(token.as_deref()) {
            Ok(h) => h,
            Err(e) => return Ok(json!({ "success": false, "error": e.to_string() })),
        };

        match client.get(&url).headers(headers).send().await {
            Ok(resp) => {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                if !status.is_success() {
                    Ok(json!({
                        "success": false,
                        "error": format!("GitHub API Error ({}): {}", status, text)
                    }))
                } else if let Ok(val) = serde_json::from_str::<Value>(&text) {
                    let mut output = format!("Comments for Issue #{}:\n\n", issue_number);
                    if let Some(items) = val.as_array() {
                        for comment in items {
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
                    }
                    Ok(json!({ "success": true, "result": output }))
                } else {
                    Ok(json!({ "success": true, "result": text }))
                }
            }
            Err(e) => Ok(json!({ "success": false, "error": e.to_string() })),
        }
    }
}

#[cfg(test)]
#[path = "github_mcp_tests.rs"]
mod tests;
