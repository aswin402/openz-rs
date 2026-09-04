use crate::tools::ToolRegistry;


use crate::config::schema::Config;
use crate::cli::is_silent_mode;

pub(crate) fn register_lazy_mcp_tools(registry: &ToolRegistry, config: &Config) {
    // ── MCP: lazy registration ────────────────────────────────────────────────
    let silent = is_silent_mode();

    let has_any_mcp = config.mcp_servers.values().any(|c| c.enabled);

    if has_any_mcp {
        tracing::info!("Setting up MCP servers (background)...");
    }

    // Collect enabled servers for the background task
    let mcp_configs: Vec<(String, crate::config::schema::McpServerConfig)> = config
        .mcp_servers
        .iter()
        .filter(|(_, c)| c.enabled)
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    let registry_bg = registry.clone();
    let num_configs = mcp_configs.len() as u32;

    let _mcp_handle = tokio::spawn(async move {
        if !silent {
            crate::channels::cli::init_mcp_progress(num_configs);
        }

        let mut servers_loaded = 0u32;
        let mut servers_failed = 0u32;

        let mut tasks = Vec::new();
        for (name, mcp_config) in mcp_configs {
            let registry_bg = registry_bg.clone();
            tasks.push(tokio::spawn(async move {
                let name_clone = name.clone();
                let mcp_config_clone = mcp_config.clone();
                let result = tokio::time::timeout(std::time::Duration::from_secs(15), async {
                    let mcp_client = crate::tools::mcp::McpClient::spawn(
                        &mcp_config_clone.command,
                        &mcp_config_clone.args,
                    )
                    .await?;
                    if name_clone == "memory" {
                        crate::tools::mcp::set_memory_mcp_client(mcp_client.clone());
                    }
                    let tools = mcp_client.list_tools().await?;
                    Ok::<_, anyhow::Error>(tools)
                })
                .await;

                match result {
                    Ok(Ok(tools)) => {
                        if !silent {
                            crate::channels::cli::increment_mcp_loaded();
                        }
                        let mut count = 0;
                        for t in tools {
                            if let (Some(t_name), Some(desc)) = (
                                t.get("name").and_then(|v| v.as_str()),
                                t.get("description").and_then(|v| v.as_str()),
                            ) {
                                let params = t.get("inputSchema").cloned().unwrap_or(
                                    serde_json::json!({"type": "object", "properties": {}}),
                                );
                                let wrapper = crate::tools::mcp::LazyMcpToolWrapper {
                                    server_name: name_clone.clone(),
                                    command: mcp_config_clone.command.clone(),
                                    args: mcp_config_clone.args.clone(),
                                    name: t_name.to_string(),
                                    description: desc.to_string(),
                                    parameters: params,
                                    is_memory_server: name_clone == "memory",
                                };
                                registry_bg.register(std::sync::Arc::new(wrapper));
                                count += 1;
                            }
                        }
                        Ok::<usize, anyhow::Error>(count)
                    }
                    Ok(Err(e)) => {
                        if !silent {
                            crate::channels::cli::increment_mcp_failed();
                        }
                        tracing::error!("Failed starting MCP server {}: {:?}", name_clone, e);
                        Err(e)
                    }
                    Err(elapsed) => {
                        if !silent {
                            crate::channels::cli::increment_mcp_failed();
                        }
                        tracing::error!(
                            "Timed out starting MCP server {} after 15s: {:?}",
                            name_clone,
                            elapsed
                        );
                        Err(anyhow::anyhow!(
                            "Timed out starting MCP server {}: {:?}",
                            name_clone,
                            elapsed
                        ))
                    }
                }
            }));
        }

        let results = futures_util::future::join_all(tasks).await;
        for res in results {
            match res {
                Ok(Ok(_count)) => {
                    servers_loaded += 1;
                }
                _ => {
                    servers_failed += 1;
                }
            }
        }

        // Update the status bar pill — the render loop reads these atomics every redraw
        if !silent {
            crate::channels::cli::set_mcp_status(servers_loaded, servers_failed);
            crate::channels::cli::set_mcp_done();
        }

        if has_any_mcp {
            crate::tools::mcp::start_mcp_health_checks();
        }
    });
}

pub(crate) fn register_searchxyz_tools(registry: &ToolRegistry) {
    // ── searchxyz tools ───────────────────────────────────────────────────────
    registry.register(std::sync::Arc::new(
        crate::tools::searchxyz::SearchXyzDoctorTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::searchxyz::SearchXyzSearchWebTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::searchxyz::SearchXyzBrowserSearchTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::searchxyz::SearchXyzReadUrlTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::searchxyz::SearchXyzSearchAndReadTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::searchxyz::SearchXyzRecallTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::searchxyz::SearchXyzListSourcesTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::searchxyz::SearchXyzDeepResearchTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::searchxyz::SearchXyzIndexContentTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::searchxyz::SearchXyzSiteMapTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::searchxyz::SearchXyzIndexRelationshipTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::searchxyz::SearchXyzQueryGraphTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::searchxyz::SearchXyzReadGithubRepoTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::searchxyz::SearchXyzExportResearchTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::searchxyz::SearchXyzImportResearchTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::searchxyz::SearchXyzDeleteSourceTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::searchxyz::SearchXyzClearIndexTool,
    ));
}

pub(crate) fn register_github_mcp_tools(registry: &ToolRegistry) {
    // ── GitHub MCP Tools ──
    registry.register(std::sync::Arc::new(
        crate::tools::github_mcp::GithubCreatePullRequestTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::github_mcp::GithubSearchIssuesTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::github_mcp::GithubGetIssueCommentsTool,
    ));
}

pub(crate) fn register_docs_mcp_tools(registry: &ToolRegistry) {
    // ── Docs MCP Tools ──
    registry.register(std::sync::Arc::new(
        crate::tools::docs_mcp::DocsListDocsetsTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::docs_mcp::DocsInstallDocsetTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::docs_mcp::DocsSearchDocsTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::docs_mcp::DocsReadDocPageTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::docs_mcp::DocsSearchRustCrateTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::docs_mcp::DocsReadRustDocsTool,
    ));
}
