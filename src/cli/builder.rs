use crate::config::loader::resolve_path;
use crate::config::schema::Config;
use crate::session::SessionManager;
use crate::agent::AgentLoop;
use crate::tools::ToolRegistry;
use crate::cli::is_silent_mode;

use crate::tools::filesystem::{ReadFileTool, FindFilesTool};
use crate::tools::doc_reader::DocReaderTool;
use crate::tools::wasm_sandbox::WasmSandboxTool;
use crate::tools::js_format::JsFormatTool;
use crate::tools::semantic_search::SemanticSearchTool;
use crate::tools::shared_memory::{StoreMemoryTool, RecallMemoryTool, ClearMemoryTool, DeleteMemoryTool, UpdateMemoryTool, ArchiveResearchTool, SearchResearchTool};
use crate::tools::filesystem::ZenflowEditTool;
use crate::tools::shell::{PythonSandboxTool, ExecCommandTool};
use crate::tools::rust_docs::RustDocsTool;
use crate::tools::filesystem::{WriteFileTool, PatchFileTool, ReplaceLinesTool, ListDirTool};
use crate::tools::web::WebFetchTool;
use crate::tools::subagent::{DelegateTaskTool, OptimizeSubagentTool, CreateSubagentTool, DeleteSubagentTool, ParallelResearchTool};
use crate::tools::cron::{ScheduleJobTool, ListJobsTool, RemoveJobTool};
use crate::tools::remote::SendRemoteInputTool;
use crate::tools::notes::IndexNotesTool;
use crate::tools::social_search::SocialSearchTool;
use crate::tools::db_inspector::{DbInspectorTool, DbWriteTool};
use crate::tools::system_info::SystemInfoTool;
use crate::tools::network::CheckPortTool;

use anyhow::Result;

pub fn get_provider_api_key(config: &Config, provider_name: &str) -> Option<String> {
    match provider_name {
        "anthropic" => {
            config.providers.anthropic.as_ref()
                .and_then(|p| p.api_key.clone())
                .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok())
        }
        "openai" => {
            config.providers.openai.as_ref()
                .and_then(|p| p.api_key.clone())
                .or_else(|| std::env::var("OPENAI_API_KEY").ok())
        }
        "openrouter" => {
            config.providers.openrouter.as_ref()
                .and_then(|p| p.api_key.clone())
                .or_else(|| std::env::var("OPENROUTER_API_KEY").ok())
        }
        "deepseek" => {
            config.providers.deepseek.as_ref()
                .and_then(|p| p.api_key.clone())
                .or_else(|| std::env::var("DEEPSEEK_API_KEY").ok())
        }
        "groq" => {
            config.providers.groq.as_ref()
                .and_then(|p| p.api_key.clone())
                .or_else(|| std::env::var("GROQ_API_KEY").ok())
        }
        "ollama" => {
            config.providers.ollama.as_ref()
                .and_then(|p| p.api_key.clone())
                .or_else(|| std::env::var("OLLAMA_API_KEY").ok())
        }
        "minimax" => {
            config.providers.minimax.as_ref()
                .and_then(|p| p.api_key.clone())
                .or_else(|| std::env::var("MINIMAX_API_KEY").ok())
        }
        _ => None,
    }
}

pub async fn build_agent_loop(config: Config) -> Result<AgentLoop> {
    let resolved = crate::providers::resolver::resolve_provider_full(&config, &config.agents.defaults.model)?;
    let provider = resolved.instance;
    
    let sessions_dir = resolve_path("~/.openz/sessions");
    let session_manager = SessionManager::new(sessions_dir);

    let registry = ToolRegistry::new_with_context(config.clone(), provider.clone(), session_manager.clone());
    registry.register(std::sync::Arc::new(ReadFileTool));
    registry.register(std::sync::Arc::new(FindFilesTool));
    registry.register(std::sync::Arc::new(DocReaderTool));
    registry.register(std::sync::Arc::new(WasmSandboxTool));
    registry.register(std::sync::Arc::new(JsFormatTool));
    registry.register(std::sync::Arc::new(SemanticSearchTool));
    registry.register(std::sync::Arc::new(StoreMemoryTool));
    registry.register(std::sync::Arc::new(RecallMemoryTool));
    registry.register(std::sync::Arc::new(ClearMemoryTool));
    registry.register(std::sync::Arc::new(DeleteMemoryTool));
    registry.register(std::sync::Arc::new(UpdateMemoryTool));
    registry.register(std::sync::Arc::new(ArchiveResearchTool));
    registry.register(std::sync::Arc::new(SearchResearchTool));
    registry.register(std::sync::Arc::new(ZenflowEditTool { provider: provider.clone() }));
    registry.register(std::sync::Arc::new(PythonSandboxTool));
    registry.register(std::sync::Arc::new(RustDocsTool::new()));
    registry.register(std::sync::Arc::new(WriteFileTool));
    registry.register(std::sync::Arc::new(PatchFileTool));
    registry.register(std::sync::Arc::new(ReplaceLinesTool));
    registry.register(std::sync::Arc::new(ListDirTool));
    registry.register(std::sync::Arc::new(ExecCommandTool));
    registry.register(std::sync::Arc::new(WebFetchTool::new()));
    registry.register(std::sync::Arc::new(DelegateTaskTool {
        config: config.clone(),
        parent_provider: provider.clone(),
        session_manager: session_manager.clone(),
        parent_tools: Vec::new(),
        cancellation_token: crate::tools::subagent::CancellationToken::new(),
    }));
    registry.register(std::sync::Arc::new(ParallelResearchTool {
        config: config.clone(),
        parent_provider: provider.clone(),
        session_manager: session_manager.clone(),
        parent_tools: Vec::new(),
        cancellation_token: crate::tools::subagent::CancellationToken::new(),
    }));
    registry.register(std::sync::Arc::new(crate::tools::subagent::EvaluatorOptimizerLoopTool {
        config: config.clone(),
        parent_provider: provider.clone(),
        session_manager: session_manager.clone(),
        parent_tools: Vec::new(),
        cancellation_token: crate::tools::subagent::CancellationToken::new(),
    }));

    registry.register(std::sync::Arc::new(OptimizeSubagentTool {
        config: config.clone(),
        parent_provider: provider.clone(),
    }));

    registry.register(std::sync::Arc::new(CreateSubagentTool {
        config: config.clone(),
    }));
    registry.register(std::sync::Arc::new(DeleteSubagentTool));

    registry.register(std::sync::Arc::new(ScheduleJobTool));
    registry.register(std::sync::Arc::new(ListJobsTool));
    registry.register(std::sync::Arc::new(RemoveJobTool));
    registry.register(std::sync::Arc::new(SendRemoteInputTool));
    registry.register(std::sync::Arc::new(crate::tools::mcp_manager::ManageMcpTool));
    registry.register(std::sync::Arc::new(crate::tools::grep::GrepSearchTool));
    registry.register(std::sync::Arc::new(crate::tools::git_manager::GitManagerTool));
    registry.register(std::sync::Arc::new(crate::tools::github::GitProviderTool));
    registry.register(std::sync::Arc::new(crate::tools::outline::CodeOutlineTool));
    registry.register(std::sync::Arc::new(DbInspectorTool));
    registry.register(std::sync::Arc::new(DbWriteTool));
    registry.register(std::sync::Arc::new(SystemInfoTool));
    registry.register(std::sync::Arc::new(CheckPortTool));
    registry.register(std::sync::Arc::new(crate::tools::cargo_manager::CargoManagerTool::new(provider.clone())));
    registry.register(std::sync::Arc::new(crate::tools::clipboard::ClipboardTool));
    registry.register(std::sync::Arc::new(crate::tools::open::OpenTool));
    registry.register(std::sync::Arc::new(crate::tools::watcher::FileWatcherTool));
    registry.register(std::sync::Arc::new(crate::tools::ast_grep::AstGrepTool));
    registry.register(std::sync::Arc::new(crate::tools::ast_grep::AstGrepIndexCodebaseTool));
    registry.register(std::sync::Arc::new(crate::tools::gsd_browser::GsdBrowserTool));
    registry.register(std::sync::Arc::new(crate::tools::web_search::WebSearchTool::new()));
    registry.register(std::sync::Arc::new(crate::tools::onpkg::OnpkgTool));
    registry.register(std::sync::Arc::new(crate::tools::image_generator::GenerateImageTool));
    registry.register(std::sync::Arc::new(crate::tools::crawl::CrawlSiteTool::new()));
    registry.register(std::sync::Arc::new(crate::tools::obscura::ObscuraBrowserTool::new()));
    registry.register(std::sync::Arc::new(crate::tools::firefox::FirefoxBrowserTool::new()));
    registry.register(std::sync::Arc::new(IndexNotesTool));
    registry.register(std::sync::Arc::new(SocialSearchTool::new()));
    registry.register(std::sync::Arc::new(crate::tools::template_compiler::CompileTemplateTool));
    registry.register(std::sync::Arc::new(crate::tools::mermaid::MermaidRendererTool));
    registry.register(std::sync::Arc::new(crate::tools::video::VideoGeneratorTool));
    registry.register(std::sync::Arc::new(crate::tools::html_video::HtmlToVideoTool));
    registry.register(std::sync::Arc::new(crate::tools::svg_animator::SvgAnimatorTool));
    registry.register(std::sync::Arc::new(crate::tools::sop::TriggerSopTool { config: config.clone() }));
    registry.register(std::sync::Arc::new(crate::tools::compiler_auto_heal::CompilerAutoHealTool {
        config: config.clone(),
        provider: provider.clone(),
    }));

    // ── Sequential Thinking tools (native, ported from MCP) ──────
    registry.register(std::sync::Arc::new(crate::tools::sequential_thinking::SequentialThinkingTool));
    registry.register(std::sync::Arc::new(crate::tools::sequential_thinking::AnalyzeGraphTool));
    registry.register(std::sync::Arc::new(crate::tools::sequential_thinking::ExportSessionTool));
    registry.register(std::sync::Arc::new(crate::tools::sequential_thinking::SummarizeReasoningTool));
    registry.register(std::sync::Arc::new(crate::tools::sequential_thinking::TemplatesTool));

    // ── CCR / Headroom tools (native, ported from MCP) ───────────
    registry.register(std::sync::Arc::new(crate::tools::headroom::ScopeContextTool));
    registry.register(std::sync::Arc::new(crate::tools::headroom::CompressContentTool));
    registry.register(std::sync::Arc::new(crate::tools::headroom::RetrieveOriginalTool));
    registry.register(std::sync::Arc::new(crate::tools::headroom::PingTool));
    registry.register(std::sync::Arc::new(crate::tools::headroom::ServerInfoTool));
    registry.register(std::sync::Arc::new(crate::tools::headroom::CountTokensTool));
    registry.register(std::sync::Arc::new(crate::tools::headroom::CacheStatsTool));
    registry.register(std::sync::Arc::new(crate::tools::headroom::ClearCacheTool));
    registry.register(std::sync::Arc::new(crate::tools::headroom::SearchCacheTool));
    registry.register(std::sync::Arc::new(crate::tools::headroom::CacheAlignTool));
    registry.register(std::sync::Arc::new(crate::tools::headroom::CompressSchemaTool));
    registry.register(std::sync::Arc::new(crate::tools::headroom::CompressFileTool));
    registry.register(std::sync::Arc::new(crate::tools::headroom::CompressDiffTool));
    registry.register(std::sync::Arc::new(crate::tools::headroom::ExportCacheTool));
    registry.register(std::sync::Arc::new(crate::tools::headroom::ImportCacheTool));
    registry.register(std::sync::Arc::new(crate::tools::headroom::CompressUrlTool));
    registry.register(std::sync::Arc::new(crate::tools::headroom::RunAndCompressTool));
    registry.register(std::sync::Arc::new(crate::tools::headroom::CompressDirectoryTool));
    registry.register(std::sync::Arc::new(crate::tools::headroom::SummarizeCodebaseTool));

    // ── Graph Memory tools (native, ported from memory_rs MCP) ─────────────
    registry.register(std::sync::Arc::new(crate::tools::graph_memory::CreateEntitiesTool));
    registry.register(std::sync::Arc::new(crate::tools::graph_memory::CreateRelationsTool));
    registry.register(std::sync::Arc::new(crate::tools::graph_memory::AddObservationsTool));
    registry.register(std::sync::Arc::new(crate::tools::graph_memory::DeleteEntitiesTool));
    registry.register(std::sync::Arc::new(crate::tools::graph_memory::DeleteObservationsTool));
    registry.register(std::sync::Arc::new(crate::tools::graph_memory::DeleteRelationsTool));
    registry.register(std::sync::Arc::new(crate::tools::graph_memory::ReadGraphTool));
    registry.register(std::sync::Arc::new(crate::tools::graph_memory::SearchNodesTool));
    registry.register(std::sync::Arc::new(crate::tools::graph_memory::OpenNodesTool));
    registry.register(std::sync::Arc::new(crate::tools::graph_memory::CreateDatabaseBranchTool));
    registry.register(std::sync::Arc::new(crate::tools::graph_memory::CommitDatabaseBranchTool));
    registry.register(std::sync::Arc::new(crate::tools::graph_memory::RollbackDatabaseBranchTool));

    // ── Graph Memory Extra tools (ported from openmemory_rs MCP) ──────────
    registry.register(std::sync::Arc::new(crate::tools::memory_extra::SetWorkingMemoryTool));
    registry.register(std::sync::Arc::new(crate::tools::memory_extra::GetWorkingMemoryTool));
    registry.register(std::sync::Arc::new(crate::tools::memory_extra::EvictExpiredWorkingMemoryTool));
    registry.register(std::sync::Arc::new(crate::tools::memory_extra::PromoteWorkingMemoryTool));
    registry.register(std::sync::Arc::new(crate::tools::memory_extra::LogExecutionEpisodeTool));
    registry.register(std::sync::Arc::new(crate::tools::memory_extra::LogReflectionTool));
    registry.register(std::sync::Arc::new(crate::tools::memory_extra::RetrieveEpisodicReflectionsTool));
    registry.register(std::sync::Arc::new(crate::tools::memory_extra::RecordToolPerformanceTool));
    registry.register(std::sync::Arc::new(crate::tools::memory_extra::QueryToolPerformanceTool));
    registry.register(std::sync::Arc::new(crate::tools::memory_extra::StoreSharedTeamMemoryTool));
    registry.register(std::sync::Arc::new(crate::tools::memory_extra::RetrieveSharedTeamMemoryTool));
    registry.register(std::sync::Arc::new(crate::tools::memory_extra::SearchTextTool));
    registry.register(std::sync::Arc::new(crate::tools::memory_extra::HybridSearchTool));
    registry.register(std::sync::Arc::new(crate::tools::memory_extra::InvalidateFactTool));
    registry.register(std::sync::Arc::new(crate::tools::memory_extra::QueryFactHistoryTool));
    registry.register(std::sync::Arc::new(crate::tools::memory_extra::QueryAsOfTool));
    registry.register(std::sync::Arc::new(crate::tools::memory_extra::SmartStoreTool));
    registry.register(std::sync::Arc::new(crate::tools::memory_extra::ExtractAndStoreFactsTool));
    registry.register(std::sync::Arc::new(crate::tools::memory_extra::ProactiveRecallTool));
    registry.register(std::sync::Arc::new(crate::tools::memory_extra::CompressContextTool));
    registry.register(std::sync::Arc::new(crate::tools::memory_extra::MemoryStatsTool));
    registry.register(std::sync::Arc::new(crate::tools::memory_extra::LogRepositoryEvolutionTool));
    registry.register(std::sync::Arc::new(crate::tools::memory_extra::QueryRepositoryEvolutionTool));
    registry.register(std::sync::Arc::new(crate::tools::memory_extra::TraverseGraphTool));
    registry.register(std::sync::Arc::new(crate::tools::memory_extra::FindPathTool));
    registry.register(std::sync::Arc::new(crate::tools::memory_extra::AnalyzeGraphCommunitiesTool));
    registry.register(std::sync::Arc::new(crate::tools::memory_extra::DetectAndResolveConflictsTool));
    registry.register(std::sync::Arc::new(crate::tools::memory_extra::CompactMemoriesTool));
    registry.register(std::sync::Arc::new(crate::tools::memory_extra::IndexCodebaseTool));
    registry.register(std::sync::Arc::new(crate::tools::memory_extra::QueryCodeGraphTool));
    registry.register(std::sync::Arc::new(crate::tools::memory_extra::AnalyzeCodeImpactTool));

    // ── searchxyz tools ───────────────────────────────────────────────────────
    registry.register(std::sync::Arc::new(crate::tools::searchxyz::SearchXyzSearchWebTool));
    registry.register(std::sync::Arc::new(crate::tools::searchxyz::SearchXyzReadUrlTool));
    registry.register(std::sync::Arc::new(crate::tools::searchxyz::SearchXyzSearchAndReadTool));
    registry.register(std::sync::Arc::new(crate::tools::searchxyz::SearchXyzRecallTool));
    registry.register(std::sync::Arc::new(crate::tools::searchxyz::SearchXyzListSourcesTool));
    registry.register(std::sync::Arc::new(crate::tools::searchxyz::SearchXyzDeepResearchTool));
    registry.register(std::sync::Arc::new(crate::tools::searchxyz::SearchXyzIndexContentTool));
    registry.register(std::sync::Arc::new(crate::tools::searchxyz::SearchXyzSiteMapTool));
    registry.register(std::sync::Arc::new(crate::tools::searchxyz::SearchXyzIndexRelationshipTool));
    registry.register(std::sync::Arc::new(crate::tools::searchxyz::SearchXyzQueryGraphTool));
    registry.register(std::sync::Arc::new(crate::tools::searchxyz::SearchXyzReadGithubRepoTool));
    registry.register(std::sync::Arc::new(crate::tools::searchxyz::SearchXyzExportResearchTool));
    registry.register(std::sync::Arc::new(crate::tools::searchxyz::SearchXyzImportResearchTool));
    registry.register(std::sync::Arc::new(crate::tools::searchxyz::SearchXyzDeleteSourceTool));
    registry.register(std::sync::Arc::new(crate::tools::searchxyz::SearchXyzClearIndexTool));

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
                    let mcp_client = crate::tools::mcp::McpClient::spawn(&mcp_config_clone.command, &mcp_config_clone.args).await?;
                    if name_clone == "memory" {
                        crate::tools::mcp::set_memory_mcp_client(mcp_client.clone());
                    }
                    let tools = mcp_client.list_tools().await?;
                    Ok::<_, anyhow::Error>(tools)
                }).await;

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
                                    serde_json::json!({"type": "object", "properties": {}})
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
                        tracing::error!("Timed out starting MCP server {} after 15s: {:?}", name_clone, elapsed);
                        Err(anyhow::anyhow!("Timed out starting MCP server {}: {:?}", name_clone, elapsed))
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

    Ok(AgentLoop::new(config, provider, registry, session_manager))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_native_tool_registration_names() {
        let registry = ToolRegistry::new();

        // ── Sequential Thinking ──
        registry.register(std::sync::Arc::new(crate::tools::sequential_thinking::SequentialThinkingTool));
        registry.register(std::sync::Arc::new(crate::tools::sequential_thinking::AnalyzeGraphTool));
        registry.register(std::sync::Arc::new(crate::tools::sequential_thinking::ExportSessionTool));
        registry.register(std::sync::Arc::new(crate::tools::sequential_thinking::SummarizeReasoningTool));
        registry.register(std::sync::Arc::new(crate::tools::sequential_thinking::TemplatesTool));

        // ── Headroom ──
        registry.register(std::sync::Arc::new(crate::tools::headroom::ScopeContextTool));
        registry.register(std::sync::Arc::new(crate::tools::headroom::CompressContentTool));
        registry.register(std::sync::Arc::new(crate::tools::headroom::RetrieveOriginalTool));
        registry.register(std::sync::Arc::new(crate::tools::headroom::PingTool));
        registry.register(std::sync::Arc::new(crate::tools::headroom::ServerInfoTool));
        registry.register(std::sync::Arc::new(crate::tools::headroom::CountTokensTool));
        registry.register(std::sync::Arc::new(crate::tools::headroom::CacheStatsTool));
        registry.register(std::sync::Arc::new(crate::tools::headroom::ClearCacheTool));
        registry.register(std::sync::Arc::new(crate::tools::headroom::SearchCacheTool));
        registry.register(std::sync::Arc::new(crate::tools::headroom::CacheAlignTool));
        registry.register(std::sync::Arc::new(crate::tools::headroom::CompressSchemaTool));
        registry.register(std::sync::Arc::new(crate::tools::headroom::CompressFileTool));
        registry.register(std::sync::Arc::new(crate::tools::headroom::CompressDiffTool));
        registry.register(std::sync::Arc::new(crate::tools::headroom::ExportCacheTool));
        registry.register(std::sync::Arc::new(crate::tools::headroom::ImportCacheTool));
        registry.register(std::sync::Arc::new(crate::tools::headroom::CompressUrlTool));
        registry.register(std::sync::Arc::new(crate::tools::headroom::RunAndCompressTool));
        registry.register(std::sync::Arc::new(crate::tools::headroom::CompressDirectoryTool));
        registry.register(std::sync::Arc::new(crate::tools::headroom::SummarizeCodebaseTool));

        // ── Graph Memory ──
        registry.register(std::sync::Arc::new(crate::tools::graph_memory::CreateEntitiesTool));
        registry.register(std::sync::Arc::new(crate::tools::graph_memory::CreateRelationsTool));
        registry.register(std::sync::Arc::new(crate::tools::graph_memory::AddObservationsTool));
        registry.register(std::sync::Arc::new(crate::tools::graph_memory::DeleteEntitiesTool));
        registry.register(std::sync::Arc::new(crate::tools::graph_memory::DeleteObservationsTool));
        registry.register(std::sync::Arc::new(crate::tools::graph_memory::DeleteRelationsTool));
        registry.register(std::sync::Arc::new(crate::tools::graph_memory::ReadGraphTool));
        registry.register(std::sync::Arc::new(crate::tools::graph_memory::SearchNodesTool));
        registry.register(std::sync::Arc::new(crate::tools::graph_memory::OpenNodesTool));
        registry.register(std::sync::Arc::new(crate::tools::graph_memory::CreateDatabaseBranchTool));
        registry.register(std::sync::Arc::new(crate::tools::graph_memory::CommitDatabaseBranchTool));
        registry.register(std::sync::Arc::new(crate::tools::graph_memory::RollbackDatabaseBranchTool));

        // ── Memory Extra ──
        registry.register(std::sync::Arc::new(crate::tools::memory_extra::SetWorkingMemoryTool));
        registry.register(std::sync::Arc::new(crate::tools::memory_extra::GetWorkingMemoryTool));
        registry.register(std::sync::Arc::new(crate::tools::memory_extra::EvictExpiredWorkingMemoryTool));
        registry.register(std::sync::Arc::new(crate::tools::memory_extra::PromoteWorkingMemoryTool));
        registry.register(std::sync::Arc::new(crate::tools::memory_extra::LogExecutionEpisodeTool));
        registry.register(std::sync::Arc::new(crate::tools::memory_extra::LogReflectionTool));
        registry.register(std::sync::Arc::new(crate::tools::memory_extra::RetrieveEpisodicReflectionsTool));
        registry.register(std::sync::Arc::new(crate::tools::memory_extra::RecordToolPerformanceTool));
        registry.register(std::sync::Arc::new(crate::tools::memory_extra::QueryToolPerformanceTool));
        registry.register(std::sync::Arc::new(crate::tools::memory_extra::StoreSharedTeamMemoryTool));
        registry.register(std::sync::Arc::new(crate::tools::memory_extra::RetrieveSharedTeamMemoryTool));
        registry.register(std::sync::Arc::new(crate::tools::memory_extra::SearchTextTool));
        registry.register(std::sync::Arc::new(crate::tools::memory_extra::HybridSearchTool));
        registry.register(std::sync::Arc::new(crate::tools::memory_extra::InvalidateFactTool));
        registry.register(std::sync::Arc::new(crate::tools::memory_extra::QueryFactHistoryTool));
        registry.register(std::sync::Arc::new(crate::tools::memory_extra::QueryAsOfTool));
        registry.register(std::sync::Arc::new(crate::tools::memory_extra::SmartStoreTool));
        registry.register(std::sync::Arc::new(crate::tools::memory_extra::ExtractAndStoreFactsTool));
        registry.register(std::sync::Arc::new(crate::tools::memory_extra::ProactiveRecallTool));
        registry.register(std::sync::Arc::new(crate::tools::memory_extra::CompressContextTool));
        registry.register(std::sync::Arc::new(crate::tools::memory_extra::MemoryStatsTool));
        registry.register(std::sync::Arc::new(crate::tools::memory_extra::LogRepositoryEvolutionTool));
        registry.register(std::sync::Arc::new(crate::tools::memory_extra::QueryRepositoryEvolutionTool));
        registry.register(std::sync::Arc::new(crate::tools::memory_extra::TraverseGraphTool));
        registry.register(std::sync::Arc::new(crate::tools::memory_extra::FindPathTool));
        registry.register(std::sync::Arc::new(crate::tools::memory_extra::AnalyzeGraphCommunitiesTool));
        registry.register(std::sync::Arc::new(crate::tools::memory_extra::DetectAndResolveConflictsTool));
        registry.register(std::sync::Arc::new(crate::tools::memory_extra::CompactMemoriesTool));
        registry.register(std::sync::Arc::new(crate::tools::memory_extra::IndexCodebaseTool));
        registry.register(std::sync::Arc::new(crate::tools::memory_extra::QueryCodeGraphTool));
        registry.register(std::sync::Arc::new(crate::tools::memory_extra::AnalyzeCodeImpactTool));

        // ── Shared Memory ──
        registry.register(std::sync::Arc::new(crate::tools::shared_memory::StoreMemoryTool));
        registry.register(std::sync::Arc::new(crate::tools::shared_memory::RecallMemoryTool));
        registry.register(std::sync::Arc::new(crate::tools::shared_memory::ClearMemoryTool));
        registry.register(std::sync::Arc::new(crate::tools::shared_memory::DeleteMemoryTool));
        registry.register(std::sync::Arc::new(crate::tools::shared_memory::UpdateMemoryTool));
        registry.register(std::sync::Arc::new(crate::tools::shared_memory::ArchiveResearchTool));
        registry.register(std::sync::Arc::new(crate::tools::shared_memory::SearchResearchTool));

        // Collect all registered tool names
        let tools = registry.to_openai_format();
        let names: Vec<&str> = tools
            .iter()
            .filter_map(|t| t["function"]["name"].as_str())
            .collect();

        // Verify no duplicate names
        let mut sorted_names = names.clone();
        sorted_names.sort();
        sorted_names.dedup();
        assert_eq!(
            names.len(),
            sorted_names.len(),
            "Duplicate tool names found in registration!\nNames: {:?}",
            names
        );

        // Verify expected tool counts per module
        let seq_names = ["sequentialthinking", "analyze_graph", "export_session", "summarize_reasoning", "reasoning_templates"];
        let headroom_names = ["scope_context", "compress_content", "retrieve_original", "ping", "server_info", "count_tokens", "cache_stats", "clear_cache", "search_cache", "cache_align", "compress_schema", "compress_file", "compress_diff", "export_cache", "import_cache", "compress_url", "run_and_compress", "compress_directory", "summarize_codebase"];
        let graph_mem_names = ["create_entities", "create_relations", "add_observations", "delete_entities", "delete_observations", "delete_relations", "read_graph", "search_nodes", "open_nodes", "create_database_branch", "commit_database_branch", "rollback_database_branch"];
        let mem_extra_names = ["set_working_memory", "get_working_memory", "evict_expired_working_memory", "promote_working_memory", "log_execution_episode", "log_reflection", "retrieve_episodic_reflections", "record_tool_performance", "query_tool_performance", "store_shared_team_memory", "retrieve_shared_team_memory", "search_text", "hybrid_search", "invalidate_fact", "query_fact_history", "query_as_of", "smart_store", "extract_and_store_facts", "proactive_recall", "compress_context", "memory_stats", "log_repository_evolution", "query_repository_evolution", "traverse_graph", "find_path", "analyze_graph_communities", "detect_and_resolve_conflicts", "compact_memories", "index_codebase", "query_code_graph", "analyze_code_impact"];
        let shared_names = ["store_memory", "recall_memory", "clear_memory", "delete_memory", "update_memory", "archive_research", "search_research"];

        let seq_count = names.iter().filter(|n| seq_names.contains(n)).count();
        let headroom_count = names.iter().filter(|n| headroom_names.contains(n)).count();
        let graph_mem_count = names.iter().filter(|n| graph_mem_names.contains(n)).count();
        let mem_extra_count = names.iter().filter(|n| mem_extra_names.contains(n)).count();
        let shared_count = names.iter().filter(|n| shared_names.contains(n)).count();

        assert_eq!(seq_count, 5, "Expected 5 sequential thinking tools, got {seq_count}: {:?}", names.iter().filter(|n| seq_names.contains(n)).collect::<Vec<_>>());
        assert_eq!(headroom_count, 19, "Expected 19 headroom tools, got {headroom_count}");
        assert_eq!(graph_mem_count, 12, "Expected 12 graph memory tools, got {graph_mem_count}");
        assert_eq!(mem_extra_count, 31, "Expected 31 memory extra tools, got {mem_extra_count}");
        assert_eq!(shared_count, 7, "Expected 7 shared memory tools, got {shared_count}");
        assert_eq!(names.len(), 5 + 19 + 12 + 31 + 7, "Total tool count mismatch");
    }
}
