use crate::tools::Tool;
use anyhow::{anyhow, Result};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, USER_AGENT};
use serde_json::Value;
use std::env;

use crate::core::secrets::read_secret_file;

pub(crate) fn configured_git_token(
    config: Option<&crate::config::schema::GitIntegrationConfig>,
) -> Option<String> {
    config.and_then(|cfg| {
        cfg.token_env
            .as_deref()
            .and_then(|env| env::var(env).ok())
            .filter(|token| !token.trim().is_empty())
            .or_else(|| cfg.token_file.as_deref().and_then(read_secret_file))
            .or_else(|| cfg.token.clone().filter(|token| !token.trim().is_empty()))
    })
}

pub(crate) fn build_git_http_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .use_rustls_tls()
        .connect_timeout(crate::core::http::DEFAULT_CONNECT_TIMEOUT)
        .timeout(crate::core::http::DEFAULT_REQUEST_TIMEOUT)
        .redirect(crate::tools::web::ssrf_redirect_policy())
        .build()
        .map_err(|e| anyhow!("Failed to build HTTP client: {}", e))
}

async fn read_checked_response_text(resp: reqwest::Response, label: &str) -> Result<String> {
    let status = resp.status();
    let text = resp.text().await?;
    match status.is_success() {
        true => Ok(text),
        false => Err(anyhow!("{} API Error ({}): {}", label, status, text)),
    }
}

async fn read_checked_response_json(resp: reqwest::Response, label: &str) -> Result<Value> {
    let text = read_checked_response_text(resp, label).await?;
    let val: Value = serde_json::from_str(&text)?;
    Ok(val)
}

fn extract_pr_number(arguments: &Value) -> Result<i64> {
    arguments
        .get("pr_number")
        .or_else(|| arguments.get("number"))
        .or_else(|| arguments.get("pr"))
        .and_then(|n| {
            n.as_i64()
                .or_else(|| n.as_str().and_then(|s| s.trim().parse::<i64>().ok()))
        })
        .ok_or_else(|| anyhow!("Missing pr_number for get_pr_diff"))
}

fn extract_create_pr_fields<'a>(
    arguments: &'a Value,
) -> Result<(&'a str, &'a str, &'a str, &'a str)> {
    let title = arguments
        .get("title")
        .and_then(|t| t.as_str())
        .ok_or_else(|| anyhow!("Missing title for create_pr"))?;
    let body = arguments.get("body").and_then(|b| b.as_str()).unwrap_or("");
    let head = arguments
        .get("head")
        .and_then(|h| h.as_str())
        .ok_or_else(|| anyhow!("Missing head for create_pr"))?;
    let base = arguments
        .get("base")
        .and_then(|b| b.as_str())
        .ok_or_else(|| anyhow!("Missing base for create_pr"))?;
    Ok((title, body, head, base))
}

pub fn parse_repo_from_git_url(url: &str) -> Option<String> {
    let trimmed = url.trim();
    (!trimmed.is_empty()).then_some(())?;

    let path = match trimmed.strip_prefix("git@") {
        Some(stripped) => {
            let parts: Vec<&str> = stripped.splitn(2, ':').collect();
            (parts.len() == 2).then(|| parts[1])?
        }
        None => match trimmed.find("://") {
            Some(pos) => {
                let after_scheme = &trimmed[pos + 3..];
                let parts: Vec<&str> = after_scheme.splitn(2, '/').collect();
                (parts.len() == 2).then(|| parts[1])?
            }
            None => trimmed,
        },
    };

    let repo_path = path.strip_suffix(".git").unwrap_or(path).trim_matches('/');
    let parts: Vec<&str> = repo_path.split('/').collect();
    (parts.len() >= 2).then(|| format!("{}/{}", parts[parts.len() - 2], parts[parts.len() - 1]))
}

pub fn detect_git_remote_repo(cwd: Option<&str>) -> Option<String> {
    let mut cmd = std::process::Command::new("git");
    cmd.args(["config", "--get", "remote.origin.url"]);
    match cwd {
        Some(c) => {
            cmd.current_dir(crate::config::loader::resolve_path(c));
        }
        None => {
            crate::config::loader::set_command_cwd(&mut cmd);
        }
    }
    let output = cmd.output().ok()?;
    output.status.success().then_some(())?;
    let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    parse_repo_from_git_url(&url)
}

fn resolve_target_repo(repo_arg: Option<String>, cwd: Option<&str>) -> Result<String> {
    match repo_arg {
        Some(r) => match r.trim().eq_ignore_ascii_case("auto") {
            true => detect_git_remote_repo(cwd).ok_or_else(|| {
                anyhow!("Missing repo parameter (could not auto-detect git origin)")
            }),
            false => Ok(r),
        },
        None => detect_git_remote_repo(cwd).ok_or_else(|| anyhow!("Missing repo parameter")),
    }
}

pub struct GitProviderTool;

#[async_trait::async_trait]
impl Tool for GitProviderTool {
    fn name(&self) -> &str {
        "git_provider"
    }

    fn description(&self) -> &str {
        "Interact with GitHub/GitLab APIs natively to create PRs, list issues, search repositories, and read PR/MR diffs."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "platform": {
                    "type": "string",
                    "enum": ["github", "gitlab"],
                    "description": "The hosting platform (default is github)"
                },
                "action": {
                    "type": "string",
                    "enum": ["create_pr", "list_issues", "search_code", "get_pr_diff"],
                    "description": "The action to perform (aliases: pr, issues, search, diff)"
                },
                "repo": {
                    "type": "string",
                    "description": "Repository path formatted as 'owner/repo' (e.g. 'tokio-rs/tokio'). If omitted or 'auto', auto-detected from local git origin."
                },
                "title": {
                    "type": "string",
                    "description": "Title of the PR/Issue (required for create_pr)"
                },
                "body": {
                    "type": "string",
                    "description": "Body/Description of the PR (optional for create_pr)"
                },
                "head": {
                    "type": "string",
                    "description": "The name of the branch where your changes are implemented (required for create_pr)"
                },
                "base": {
                    "type": "string",
                    "description": "The name of the branch you want your changes pulled into (required for create_pr, e.g. 'main')"
                },
                "issue_state": {
                    "type": "string",
                    "enum": ["open", "closed", "all"],
                    "description": "State of issues to list (optional for list_issues, default is open)"
                },
                "query": {
                    "type": "string",
                    "description": "Search query or term (required for search_code)"
                },
                "pr_number": {
                    "description": "Pull/Merge request number (required for get_pr_diff, integer or string).",
                    "oneOf": [
                        { "type": "integer" },
                        { "type": "string" }
                    ]
                },
                "token": {
                    "type": "string",
                    "description": "API token. If not provided, it will fallback to GITHUB_TOKEN or GITLAB_TOKEN environment variables."
                },
                "api_base": {
                    "type": "string",
                    "description": "Custom API base URL (optional, e.g. for self-hosted instances)"
                }
            },
            "required": ["action"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let platform = arguments
            .get("platform")
            .and_then(|p| p.as_str())
            .unwrap_or("github");
        let action_raw = arguments
            .get("action")
            .or_else(|| arguments.get("command"))
            .or_else(|| arguments.get("operation"))
            .and_then(|a| a.as_str())
            .ok_or_else(|| anyhow!("Missing action parameter"))?;
        let action_norm = action_raw.trim().to_lowercase();
        let action = match action_norm.as_str() {
            "create_pr" | "pr" | "create_pull_request" | "pull_request" | "create_mr" => "create_pr",
            "list_issues" | "issues" | "list_issue" => "list_issues",
            "search_code" | "search" | "code_search" => "search_code",
            "get_pr_diff" | "diff" | "pr_diff" | "mr_diff" => "get_pr_diff",
            _ => action_norm.as_str(),
        };

        let cwd = arguments
            .get("cwd")
            .or_else(|| arguments.get("dir"))
            .and_then(|v| v.as_str());
        let repo_arg = arguments
            .get("repo")
            .or_else(|| arguments.get("repository"))
            .and_then(|r| r.as_str())
            .map(|s| s.to_string());
        let repo_owned = resolve_target_repo(repo_arg, cwd)?;
        let repo = repo_owned.as_str();

        let token_arg = arguments
            .get("token")
            .and_then(|t| t.as_str())
            .map(|s| s.to_string());
        let api_base_arg = arguments
            .get("api_base")
            .and_then(|b| b.as_str())
            .map(|s| s.to_string());

        let client = build_git_http_client()?;

        match platform {
            "github" => {
                let config = crate::config::loader::load_config().ok();
                let configured_github =
                    config.as_ref().and_then(|c| c.integrations.github.as_ref());
                let token = token_arg
                    .or_else(|| env::var("GITHUB_TOKEN").ok())
                    .or_else(|| env::var("GITHUB_PAT").ok())
                    .or_else(|| configured_git_token(configured_github))
                    .ok_or_else(|| anyhow!("GitHub token not found. Provide token, set GITHUB_TOKEN/GITHUB_PAT, or store it with manage_config set_credential after approval."))?;

                let api_base = api_base_arg
                    .or_else(|| configured_github.and_then(|cfg| cfg.api_base.clone()))
                    .unwrap_or_else(|| "https://api.github.com".to_string());
                crate::tools::web::validate_url(&api_base).await?;

                let mut headers = HeaderMap::new();
                headers.insert(USER_AGENT, HeaderValue::from_static("openz"));
                headers.insert(
                    ACCEPT,
                    HeaderValue::from_static("application/vnd.github+json"),
                );
                headers.insert(
                    AUTHORIZATION,
                    HeaderValue::from_str(&format!("Bearer {}", token))?,
                );

                match action {
                    "create_pr" => {
                        let (title, body, head, base) = extract_create_pr_fields(arguments)?;
                        let url = format!("{}/repos/{}/pulls", api_base, repo);
                        let payload = serde_json::json!({
                            "title": title,
                            "body": body,
                            "head": head,
                            "base": base
                        });

                        let resp = client
                            .post(&url)
                            .headers(headers)
                            .json(&payload)
                            .send()
                            .await?;
                        read_checked_response_json(resp, "GitHub").await
                    }
                    "list_issues" => {
                        let state = arguments
                            .get("issue_state")
                            .and_then(|s| s.as_str())
                            .unwrap_or("open");
                        let url = format!("{}/repos/{}/issues?state={}", api_base, repo, state);
                        let resp = client.get(&url).headers(headers).send().await?;
                        read_checked_response_json(resp, "GitHub").await
                    }
                    "search_code" => {
                        let query = arguments
                            .get("query")
                            .and_then(|q| q.as_str())
                            .ok_or_else(|| anyhow!("Missing query for search_code"))?;
                        let encoded_query = percent_encoding::utf8_percent_encode(
                            &format!("{} repo:{}", query, repo),
                            percent_encoding::NON_ALPHANUMERIC,
                        )
                        .to_string();

                        let url = format!("{}/search/code?q={}", api_base, encoded_query);
                        let resp = client.get(&url).headers(headers).send().await?;
                        read_checked_response_json(resp, "GitHub").await
                    }
                    "get_pr_diff" => {
                        let pr_number = extract_pr_number(arguments)?;
                        let url = format!("{}/repos/{}/pulls/{}", api_base, repo, pr_number);
                        headers.insert(
                            ACCEPT,
                            HeaderValue::from_static("application/vnd.github.diff"),
                        );

                        let resp = client.get(&url).headers(headers).send().await?;
                        let text = read_checked_response_text(resp, "GitHub").await?;
                        Ok(serde_json::json!({
                            "pr_number": pr_number,
                            "diff": text
                        }))
                    }
                    _ => Err(anyhow!(
                        "Unsupported action '{}' for platform github",
                        action
                    )),
                }
            }
            "gitlab" => {
                let config = crate::config::loader::load_config().ok();
                let configured_gitlab =
                    config.as_ref().and_then(|c| c.integrations.gitlab.as_ref());
                let token = token_arg
                    .or_else(|| env::var("GITLAB_TOKEN").ok())
                    .or_else(|| env::var("GITLAB_PAT").ok())
                    .or_else(|| configured_git_token(configured_gitlab))
                    .ok_or_else(|| anyhow!("GitLab token not found. Provide token, set GITLAB_TOKEN/GITLAB_PAT, or store it with manage_config set_credential after approval."))?;

                let api_base = api_base_arg
                    .or_else(|| configured_gitlab.and_then(|cfg| cfg.api_base.clone()))
                    .unwrap_or_else(|| "https://gitlab.com/api/v4".to_string());
                crate::tools::web::validate_url(&api_base).await?;
                let urlencoded_repo =
                    percent_encoding::utf8_percent_encode(repo, percent_encoding::NON_ALPHANUMERIC)
                        .to_string();

                let mut headers = HeaderMap::new();
                headers.insert(USER_AGENT, HeaderValue::from_static("openz"));
                headers.insert("PRIVATE-TOKEN", HeaderValue::from_str(&token)?);

                match action {
                    "create_pr" => {
                        let (title, body, head, base) = extract_create_pr_fields(arguments)?;
                        let url =
                            format!("{}/projects/{}/merge_requests", api_base, urlencoded_repo);
                        let payload = serde_json::json!({
                            "title": title,
                            "description": body,
                            "source_branch": head,
                            "target_branch": base
                        });

                        let resp = client
                            .post(&url)
                            .headers(headers)
                            .json(&payload)
                            .send()
                            .await?;
                        read_checked_response_json(resp, "GitLab").await
                    }
                    "list_issues" => {
                        let state = arguments
                            .get("issue_state")
                            .and_then(|s| s.as_str())
                            .unwrap_or("opened");
                        let gitlab_state = match state {
                            "open" => "opened",
                            other => other,
                        };

                        let url = format!(
                            "{}/projects/{}/issues?state={}",
                            api_base, urlencoded_repo, gitlab_state
                        );
                        let resp = client.get(&url).headers(headers).send().await?;
                        read_checked_response_json(resp, "GitLab").await
                    }
                    "search_code" => {
                        let query = arguments
                            .get("query")
                            .and_then(|q| q.as_str())
                            .ok_or_else(|| anyhow!("Missing query for search_code"))?;
                        let encoded_query = percent_encoding::utf8_percent_encode(
                            query,
                            percent_encoding::NON_ALPHANUMERIC,
                        )
                        .to_string();

                        let url = format!(
                            "{}/projects/{}/search?scope=blobs&ref=master&search={}",
                            api_base, urlencoded_repo, encoded_query
                        );

                        let resp = client.get(&url).headers(headers.clone()).send().await?;
                        match read_checked_response_json(resp, "GitLab").await {
                            Ok(val) => Ok(val),
                            Err(_) => {
                                let url_main = format!(
                                    "{}/projects/{}/search?scope=blobs&ref=main&search={}",
                                    api_base, urlencoded_repo, encoded_query
                                );
                                let resp_main =
                                    client.get(&url_main).headers(headers).send().await?;
                                read_checked_response_json(resp_main, "GitLab").await
                            }
                        }
                    }
                    "get_pr_diff" => {
                        let pr_number = extract_pr_number(arguments)?;
                        let url = format!(
                            "{}/projects/{}/merge_requests/{}/diffs",
                            api_base, urlencoded_repo, pr_number
                        );

                        let resp = client.get(&url).headers(headers).send().await?;
                        let res_val = read_checked_response_json(resp, "GitLab").await?;

                        let mut diff_output = String::new();
                        for item in res_val.as_array().into_iter().flatten() {
                            let old_path =
                                item.get("old_path").and_then(|v| v.as_str()).unwrap_or("");
                            let new_path =
                                item.get("new_path").and_then(|v| v.as_str()).unwrap_or("");
                            let diff = item.get("diff").and_then(|v| v.as_str()).unwrap_or("");

                            diff_output
                                .push_str(&format!("--- a/{}\n+++ b/{}\n", old_path, new_path));
                            diff_output.push_str(diff);
                            diff_output.push('\n');
                        }

                        Ok(serde_json::json!({
                            "pr_number": pr_number,
                            "diff": diff_output
                        }))
                    }
                    _ => Err(anyhow!(
                        "Unsupported action '{}' for platform gitlab",
                        action
                    )),
                }
            }
            _ => Err(anyhow!("Unsupported platform '{}'", platform)),
        }
    }
}

#[cfg(test)]
#[path = "github_tests.rs"]
mod tests;
