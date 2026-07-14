use crate::agent::AgentLoop;
use crate::config::loader::resolve_path;
use crate::config::schema::Config;
use crate::session::SessionManager;
use crate::tools::ToolRegistry;
use anyhow::Result;

pub fn get_provider_api_key(config: &Config, provider_name: &str) -> Option<String> {
    let (key, _) = config.resolve_provider_config(provider_name);
    if key.is_empty() {
        None
    } else {
        Some(key)
    }
}

pub async fn build_agent_loop(config: Config) -> Result<AgentLoop> {
    let resolved =
        crate::providers::resolver::resolve_provider_full(&config, &config.agents.defaults.model)?;
    let provider = resolved.instance;

    let sessions_dir = resolve_path("~/.openz/sessions");
    let session_manager = SessionManager::new(sessions_dir);

    let registry =
        ToolRegistry::new_with_context(config.clone(), provider.clone(), session_manager.clone());
    crate::cli::tools::register_all_tools(
        &registry,
        &config,
        provider.clone(),
        session_manager.clone(),
    )?;

    Ok(AgentLoop::new(config, provider, registry, session_manager))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_native_tool_registration_names() {
        let registry = ToolRegistry::new();

        // ── Sequential Thinking ──
        registry.register(std::sync::Arc::new(
            crate::tools::sequential_thinking::SequentialThinkingTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::sequential_thinking::AnalyzeGraphTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::sequential_thinking::ExportSessionTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::sequential_thinking::SummarizeReasoningTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::sequential_thinking::TemplatesTool,
        ));

        // ── Headroom ──
        registry.register(std::sync::Arc::new(
            crate::tools::headroom::ScopeContextTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::headroom::CompressContentTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::headroom::RetrieveOriginalTool,
        ));
        registry.register(std::sync::Arc::new(crate::tools::headroom::PingTool));
        registry.register(std::sync::Arc::new(crate::tools::headroom::ServerInfoTool));
        registry.register(std::sync::Arc::new(crate::tools::headroom::CountTokensTool));
        registry.register(std::sync::Arc::new(crate::tools::headroom::CacheStatsTool));
        registry.register(std::sync::Arc::new(
            crate::tools::headroom::HeadroomStatsTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::headroom::HeadroomUsageTool,
        ));
        registry.register(std::sync::Arc::new(crate::tools::headroom::ClearCacheTool));
        registry.register(std::sync::Arc::new(crate::tools::headroom::SearchCacheTool));
        registry.register(std::sync::Arc::new(crate::tools::headroom::CacheAlignTool));
        registry.register(std::sync::Arc::new(
            crate::tools::headroom::CompressSchemaTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::headroom::CompressFileTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::headroom::CompressDiffTool,
        ));
        registry.register(std::sync::Arc::new(crate::tools::headroom::ExportCacheTool));
        registry.register(std::sync::Arc::new(crate::tools::headroom::ImportCacheTool));
        registry.register(std::sync::Arc::new(crate::tools::headroom::CompressUrlTool));
        registry.register(std::sync::Arc::new(
            crate::tools::headroom::RunAndCompressTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::headroom::CompressDirectoryTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::headroom::SummarizeCodebaseTool,
        ));

        // ── Graph Memory ──
        registry.register(std::sync::Arc::new(
            crate::tools::graph_memory::CreateEntitiesTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::graph_memory::CreateRelationsTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::graph_memory::AddObservationsTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::graph_memory::DeleteEntitiesTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::graph_memory::DeleteObservationsTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::graph_memory::DeleteRelationsTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::graph_memory::ReadGraphTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::graph_memory::SearchNodesTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::graph_memory::OpenNodesTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::graph_memory::CreateDatabaseBranchTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::graph_memory::CommitDatabaseBranchTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::graph_memory::RollbackDatabaseBranchTool,
        ));

        // ── Memory Extra ──
        registry.register(std::sync::Arc::new(
            crate::tools::memory_extra::SetWorkingMemoryTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::memory_extra::GetWorkingMemoryTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::memory_extra::EvictExpiredWorkingMemoryTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::memory_extra::PromoteWorkingMemoryTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::memory_extra::LogExecutionEpisodeTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::memory_extra::LogReflectionTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::memory_extra::RetrieveEpisodicReflectionsTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::memory_extra::RecordToolPerformanceTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::memory_extra::QueryToolPerformanceTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::memory_extra::StoreSharedTeamMemoryTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::memory_extra::RetrieveSharedTeamMemoryTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::memory_extra::SearchTextTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::memory_extra::HybridSearchTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::memory_extra::InvalidateFactTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::memory_extra::ForgetMemoryTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::memory_extra::QueryFactHistoryTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::memory_extra::QueryAsOfTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::memory_extra::SmartStoreTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::memory_extra::ExtractAndStoreFactsTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::memory_extra::ProactiveRecallTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::memory_extra::CompressContextTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::memory_extra::MemoryStatsTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::memory_extra::LogRepositoryEvolutionTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::memory_extra::QueryRepositoryEvolutionTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::memory_extra::TraverseGraphTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::memory_extra::FindPathTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::memory_extra::AnalyzeGraphCommunitiesTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::memory_extra::DetectAndResolveConflictsTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::memory_extra::CompactMemoriesTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::memory_extra::IndexCodebaseTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::memory_extra::QueryCodeGraphTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::memory_extra::AnalyzeCodeImpactTool,
        ));

        // ── Shared Memory ──
        registry.register(std::sync::Arc::new(
            crate::tools::shared_memory::StoreMemoryTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::shared_memory::RecallMemoryTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::shared_memory::ClearMemoryTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::shared_memory::DeleteMemoryTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::shared_memory::UpdateMemoryTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::shared_memory::ArchiveResearchTool,
        ));
        registry.register(std::sync::Arc::new(
            crate::tools::shared_memory::SearchResearchTool,
        ));

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
        let seq_names = [
            "sequentialthinking",
            "analyze_graph",
            "export_session",
            "summarize_reasoning",
            "reasoning_templates",
        ];
        let headroom_names = [
            "scope_context",
            "compress_content",
            "retrieve_original",
            "ping",
            "server_info",
            "count_tokens",
            "cache_stats",
            "headroom_stats",
            "headroom_usage",
            "clear_cache",
            "search_cache",
            "cache_align",
            "compress_schema",
            "compress_file",
            "compress_diff",
            "export_cache",
            "import_cache",
            "compress_url",
            "run_and_compress",
            "compress_directory",
            "summarize_codebase",
        ];
        let graph_mem_names = [
            "create_entities",
            "create_relations",
            "add_observations",
            "delete_entities",
            "delete_observations",
            "delete_relations",
            "read_graph",
            "search_nodes",
            "open_nodes",
            "create_database_branch",
            "commit_database_branch",
            "rollback_database_branch",
        ];
        let mem_extra_names = [
            "set_working_memory",
            "get_working_memory",
            "evict_expired_working_memory",
            "promote_working_memory",
            "log_execution_episode",
            "log_reflection",
            "retrieve_episodic_reflections",
            "record_tool_performance",
            "query_tool_performance",
            "store_shared_team_memory",
            "retrieve_shared_team_memory",
            "search_text",
            "hybrid_search",
            "invalidate_fact",
            "forget_memory",
            "query_fact_history",
            "query_as_of",
            "smart_store",
            "extract_and_store_facts",
            "proactive_recall",
            "compress_context",
            "memory_stats",
            "log_repository_evolution",
            "query_repository_evolution",
            "traverse_graph",
            "find_path",
            "analyze_graph_communities",
            "detect_and_resolve_conflicts",
            "compact_memories",
            "index_codebase",
            "query_code_graph",
            "analyze_code_impact",
        ];
        let shared_names = [
            "store_memory",
            "recall_memory",
            "clear_memory",
            "delete_memory",
            "update_memory",
            "archive_research",
            "search_research",
        ];

        let seq_count = names.iter().filter(|n| seq_names.contains(n)).count();
        let headroom_count = names.iter().filter(|n| headroom_names.contains(n)).count();
        let graph_mem_count = names.iter().filter(|n| graph_mem_names.contains(n)).count();
        let mem_extra_count = names.iter().filter(|n| mem_extra_names.contains(n)).count();
        let shared_count = names.iter().filter(|n| shared_names.contains(n)).count();

        assert_eq!(
            seq_count,
            5,
            "Expected 5 sequential thinking tools, got {seq_count}: {:?}",
            names
                .iter()
                .filter(|n| seq_names.contains(n))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            headroom_count, 21,
            "Expected 21 headroom tools, got {headroom_count}"
        );
        assert_eq!(
            graph_mem_count, 12,
            "Expected 12 graph memory tools, got {graph_mem_count}"
        );
        assert_eq!(
            mem_extra_count, 32,
            "Expected 32 memory extra tools, got {mem_extra_count}"
        );
        assert_eq!(
            shared_count, 7,
            "Expected 7 shared memory tools, got {shared_count}"
        );
        assert_eq!(
            names.len(),
            5 + 21 + 12 + 32 + 7,
            "Total tool count mismatch"
        );
    }
}
