use crate::config::schema::Config;
use crate::providers::LLMProvider;
use crate::session::SessionManager;
use crate::tools::ToolRegistry;
use anyhow::Result;
use std::sync::Arc;

use crate::cli::is_silent_mode;
use crate::tools::cron::{ListJobsTool, RemoveJobTool, ScheduleJobTool};
use crate::tools::db_inspector::{DbInspectorTool, DbWriteTool};
use crate::tools::doc_reader::DocReaderTool;
use crate::tools::filesystem::{
    FindFilesTool, ListDirTool, PatchFileTool, ReadFileTool, ReplaceLinesTool, WriteFileTool,
    ZenflowEditTool,
};
use crate::tools::js_format::JsFormatTool;
use crate::tools::network::CheckPortTool;
use crate::tools::notes::IndexNotesTool;
use crate::tools::remote::SendRemoteInputTool;
use crate::tools::rust_docs::RustDocsTool;
use crate::tools::semantic_search::SemanticSearchTool;
use crate::tools::shared_memory::{
    ArchiveResearchTool, ClearMemoryTool, DeleteMemoryTool, RecallMemoryTool, SearchResearchTool,
    StoreMemoryTool, UpdateMemoryTool,
};
use crate::tools::shell::{ExecCommandTool, PythonSandboxTool};
use crate::tools::social_search::SocialSearchTool;
use crate::tools::subagent::{
    CreateSubagentTool, DelegateTaskTool, DeleteSubagentTool, OptimizeSubagentTool,
    ParallelResearchTool,
};
use crate::tools::system_info::SystemInfoTool;
use crate::tools::wasm_sandbox::WasmSandboxTool;
use crate::tools::web::WebFetchTool;

pub fn register_all_tools(
    registry: &ToolRegistry,
    config: &Config,
    provider: Arc<dyn LLMProvider>,
    session_manager: SessionManager,
) -> Result<()> {
    register_core_tools(registry, config, provider.clone(), session_manager.clone());
    register_sequential_thinking_tools(registry);
    register_headroom_tools(registry);
    register_graph_memory_tools(registry);
    register_memory_extra_tools(registry);
    register_searchxyz_tools(registry);
    register_openmedia_tools(registry);
    register_opendoc_tools(registry);
    register_github_mcp_tools(registry);
    register_docs_mcp_tools(registry);
    register_lazy_mcp_tools(registry, config);

    Ok(())
}

fn register_core_tools(
    registry: &ToolRegistry,
    config: &Config,
    provider: Arc<dyn LLMProvider>,
    session_manager: SessionManager,
) {
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
    registry.register(std::sync::Arc::new(ZenflowEditTool {
        provider: provider.clone(),
    }));
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
    registry.register(std::sync::Arc::new(
        crate::tools::subagent::EvaluatorOptimizerLoopTool {
            config: config.clone(),
            parent_provider: provider.clone(),
            session_manager: session_manager.clone(),
            parent_tools: Vec::new(),
            cancellation_token: crate::tools::subagent::CancellationToken::new(),
        },
    ));

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
    registry.register(std::sync::Arc::new(
        crate::tools::mcp_manager::ManageMcpTool,
    ));
    registry.register(std::sync::Arc::new(crate::tools::grep::GrepSearchTool));
    registry.register(std::sync::Arc::new(
        crate::tools::git_manager::GitManagerTool,
    ));
    registry.register(std::sync::Arc::new(crate::tools::github::GitProviderTool));
    registry.register(std::sync::Arc::new(crate::tools::outline::CodeOutlineTool));
    registry.register(std::sync::Arc::new(DbInspectorTool));
    registry.register(std::sync::Arc::new(DbWriteTool));
    registry.register(std::sync::Arc::new(SystemInfoTool));
    registry.register(std::sync::Arc::new(CheckPortTool));
    registry.register(std::sync::Arc::new(
        crate::tools::cargo_manager::CargoManagerTool::new(provider.clone()),
    ));
    registry.register(std::sync::Arc::new(crate::tools::clipboard::ClipboardTool));
    registry.register(std::sync::Arc::new(crate::tools::open::OpenTool));
    registry.register(std::sync::Arc::new(crate::tools::watcher::FileWatcherTool));
    registry.register(std::sync::Arc::new(crate::tools::ast_grep::AstGrepTool));
    registry.register(std::sync::Arc::new(
        crate::tools::ast_grep::AstGrepIndexCodebaseTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::gsd_browser::GsdBrowserTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::web_search::WebSearchTool::new(),
    ));
    registry.register(std::sync::Arc::new(crate::tools::onpkg::OnpkgTool));
    registry.register(std::sync::Arc::new(
        crate::tools::self_management::DiagnoseToolTool::new(registry.clone()),
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::self_management::ToolCatalogTool::new(registry.clone()),
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::self_management::CurateSkillTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::self_management::OptimizeToolScopeTool::new(registry.clone()),
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::self_management::ManageConfigTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::self_management::DiagnoseSystemTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::self_management::ManageSessionsTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::self_management::ManageBackupsTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::image_generator::GenerateImageTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::crawl::CrawlSiteTool::new(),
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::obscura::ObscuraBrowserTool::new(),
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::firefox::FirefoxBrowserTool::new(),
    ));
    registry.register(std::sync::Arc::new(IndexNotesTool));
    registry.register(std::sync::Arc::new(SocialSearchTool::new()));
    registry.register(std::sync::Arc::new(
        crate::tools::template_compiler::CompileTemplateTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::mermaid::MermaidRendererTool,
    ));
    registry.register(std::sync::Arc::new(crate::tools::video::VideoGeneratorTool));
    registry.register(std::sync::Arc::new(
        crate::tools::html_video::HtmlToVideoTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::svg_animator::SvgAnimatorTool,
    ));
    registry.register(std::sync::Arc::new(crate::tools::sop::TriggerSopTool {
        config: config.clone(),
    }));
    registry.register(std::sync::Arc::new(
        crate::tools::compiler_auto_heal::CompilerAutoHealTool {
            config: config.clone(),
            provider: provider.clone(),
        },
    ));
}

fn register_sequential_thinking_tools(registry: &ToolRegistry) {
    // ── Sequential Thinking tools (native, ported from MCP) ──────
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
}

fn register_headroom_tools(registry: &ToolRegistry) {
    // ── CCR / Headroom tools (native, ported from MCP) ───────────
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
}

fn register_graph_memory_tools(registry: &ToolRegistry) {
    // ── Graph Memory tools (native, ported from memory_rs MCP) ─────────────
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
}

fn register_memory_extra_tools(registry: &ToolRegistry) {
    // ── Graph Memory Extra tools (ported from openmemory_rs MCP) ──────────
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
}

fn register_searchxyz_tools(registry: &ToolRegistry) {
    // ── searchxyz tools ───────────────────────────────────────────────────────
    registry.register(std::sync::Arc::new(
        crate::tools::searchxyz::SearchXyzSearchWebTool,
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

fn register_openmedia_tools(registry: &ToolRegistry) {
    // ── OpenMedia native tools ────────────────────────────────────────────────
    registry.register(std::sync::Arc::new(
        crate::tools::openmedia::OpenMediaModelDownloadTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::openmedia::OpenMediaRasterizeSvgTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::openmedia::OpenMediaDiagramGenerateMermaidTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::openmedia::OpenMediaHtmlToImageTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::openmedia::OpenMediaCreateSvgTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::openmedia::OpenMediaCreateChartTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::openmedia::OpenMediaCreateIconTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::openmedia::OpenMediaAnimateSvgTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::openmedia::OpenMediaAnimateCreateTimelineTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::openmedia::OpenMediaAnimateMorphPathsTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::openmedia::OpenMediaAnimateGenerateSpinnerTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::openmedia::OpenMediaAnimateFromLottieTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::openmedia::OpenMediaAnimateToLottieTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::openmedia::OpenMediaImageApplyFilterTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::openmedia::OpenMediaImageResizeTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::openmedia::OpenMediaImageCropTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::openmedia::OpenMediaImageTransformTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::openmedia::OpenMediaImageConvertTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::openmedia::OpenMediaImageBatchProcessTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::openmedia::OpenMediaVideoCreateTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::openmedia::OpenMediaVideoPreviewTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::openmedia::OpenMediaVideoCreateSlideshowTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::openmedia::OpenMediaVideoAddTransitionTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::openmedia::OpenMediaVideoAddAudioTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::openmedia::OpenMediaVideoFromTemplateTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::openmedia::OpenMediaVideoExtractFramesTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::openmedia::OpenMediaVideoTrimTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::openmedia::OpenMediaTemplateCreateTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::openmedia::OpenMediaTemplateReadTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::openmedia::OpenMediaTemplateUpdateTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::openmedia::OpenMediaTemplateDeleteTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::openmedia::OpenMediaImproveScoreImageTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::openmedia::OpenMediaImproveRefinePromptTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::openmedia::OpenMediaImproveAutoRefineTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::openmedia::OpenMediaImproveFeedbackTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::openmedia::OpenMediaImproveQualityReportTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::openmedia::OpenMediaPingTool,
    ));
}

fn register_opendoc_tools(registry: &ToolRegistry) {
    // ── Opendoc Native Tools ──────────────────────────────────────────────────
    registry.register(std::sync::Arc::new(
        crate::tools::opendoc::OpendocOpenDocumentTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::opendoc::OpendocReadDocumentTextTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::opendoc::OpendocSearchDocumentTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::opendoc::OpendocReplaceTextTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::opendoc::OpendocDiffDocumentsTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::opendoc::OpendocDiffDocumentsVisualTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::opendoc::OpendocChunkForEmbeddingTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::opendoc::OpendocFillTemplateTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::opendoc::OpendocValidateDocumentTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::opendoc::OpendocValidatePdfAComplianceTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::opendoc::OpendocExtractStructuredMetadataTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::opendoc::OpendocConvertTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::opendoc::OpendocExtractImagesTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::opendoc::OpendocSplitPdfTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::opendoc::OpendocCreateHtmlTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::opendoc::OpendocBatchConvertTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::opendoc::OpendocCreateDocxTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::opendoc::OpendocDocxAddParagraphTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::opendoc::OpendocDocxAddTableTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::opendoc::OpendocDocxAddImageTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::opendoc::OpendocCreatePptxTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::opendoc::OpendocPptxAddSlideTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::opendoc::OpendocCreateXlsxTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::opendoc::OpendocEditXlsxTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::opendoc::OpendocCreatePdfTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::opendoc::OpendocCreateFormattedPdfTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::opendoc::OpendocMergePdfsTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::opendoc::OpendocExtractPdfTextTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::opendoc::OpendocListPdfFieldsTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::opendoc::OpendocFillPdfFormTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::opendoc::OpendocFindTablesTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::opendoc::OpendocAnalyzeDocumentComplexityTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::opendoc::OpendocOcrDocumentTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::opendoc::OpendocCheckOcrAvailableTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::opendoc::OpendocExtractArchiveDigestTool,
    ));
}

fn register_github_mcp_tools(registry: &ToolRegistry) {
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

fn register_docs_mcp_tools(registry: &ToolRegistry) {
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

fn register_lazy_mcp_tools(registry: &ToolRegistry, config: &Config) {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn schema_required_fields(schema: &serde_json::Value) -> Vec<String> {
        schema
            .get("required")
            .and_then(|v| v.as_array())
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect()
    }

    fn schema_properties(
        schema: &serde_json::Value,
    ) -> Option<&serde_json::Map<String, serde_json::Value>> {
        schema.get("properties").and_then(|v| v.as_object())
    }

    fn is_opaque_json_property(prop: &serde_json::Value) -> bool {
        prop.get("type").is_none()
            && prop.get("anyOf").is_none()
            && prop.get("oneOf").is_none()
            && prop.get("allOf").is_none()
            && prop.get("$ref").is_none()
            && prop.get("properties").is_none()
            && prop.get("items").is_none()
    }

    fn has_actionable_json_example(prop: &serde_json::Value) -> bool {
        prop.get("description")
            .and_then(|v| v.as_str())
            .map(|description| {
                let lower = description.to_lowercase();
                lower.contains("json") && (description.contains('{') || description.contains('['))
            })
            .unwrap_or(false)
    }

    fn has_raw_json_normalizer(tool_name: &str, field_name: &str) -> bool {
        matches!(
            (tool_name, field_name),
            ("openmedia_create_svg", "elements")
                | ("openmedia_template_create", "parameter_schema")
                | ("openmedia_template_create", "scene_template")
                | ("openmedia_video_create", "scene")
                | ("openmedia_video_preview", "scene")
                | ("openmedia_video_from_template", "parameters")
        )
    }

    #[tokio::test]
    async fn required_raw_json_tool_fields_are_documented_or_normalized() {
        let registry = ToolRegistry::new();
        let config = Config::default();
        let provider = Arc::new(crate::providers::mock::MockProvider::new());
        let sessions = SessionManager::new(std::path::PathBuf::from("/tmp/openz-test-sessions"));

        register_all_tools(&registry, &config, provider, sessions).unwrap();

        let mut offenders = Vec::new();
        for entry in registry.catalog_entries(true) {
            let name = entry["name"].as_str().unwrap_or("<unknown>");
            let schema = &entry["parameters"];
            let Some(properties) = schema_properties(schema) else {
                continue;
            };

            for field in schema_required_fields(schema) {
                let Some(prop) = properties.get(&field) else {
                    continue;
                };
                if is_opaque_json_property(prop)
                    && !has_actionable_json_example(prop)
                    && !has_raw_json_normalizer(name, &field)
                {
                    offenders.push(format!("{name}.{field}"));
                }
            }
        }

        assert!(
            offenders.is_empty(),
            "required raw JSON fields need concrete schemas, examples, or normalizer coverage: {offenders:?}"
        );
    }

    #[tokio::test]
    async fn register_all_tools_includes_expected_domains_without_duplicates() {
        let registry = ToolRegistry::new();
        let config = Config::default();
        let provider = Arc::new(crate::providers::mock::MockProvider::new());
        let sessions = SessionManager::new(std::path::PathBuf::from("/tmp/openz-test-sessions"));

        register_all_tools(&registry, &config, provider, sessions).unwrap();

        let names = registry.tool_names();
        let unique: std::collections::BTreeSet<_> = names.iter().collect();
        assert_eq!(
            names.len(),
            unique.len(),
            "duplicate tool names must not appear"
        );
        assert!(names.contains(&"read_file".to_string()));
        assert!(names.contains(&"exec_command".to_string()));
        assert!(names.contains(&"delegate_task".to_string()));
        assert!(names.contains(&"tool_catalog".to_string()));
        assert!(names.contains(&"sequentialthinking".to_string()));
        assert!(names.contains(&"scope_context".to_string()));
        assert!(names.contains(&"create_entities".to_string()));
        assert!(names.contains(&"searchxyz_search_web".to_string()));
        assert!(names.contains(&"openmedia_ping".to_string()));
        assert!(names.contains(&"opendoc_open_document".to_string()));
        assert!(
            registry.tool_count() > 128,
            "full registry should exceed one OpenAI tool payload"
        );
    }

    #[test]
    fn openai_format_prioritizes_high_value_tools_when_truncated() {
        let registry = ToolRegistry::new();
        for i in 0..140 {
            registry.register(Arc::new(MetaTestTool {
                name: format!("low_tool_{i:03}"),
                domain: "general",
                priority: 1,
                risk: crate::tools::ToolRisk::Low,
            }));
        }
        registry.register(Arc::new(MetaTestTool {
            name: "cargo_manager".to_string(),
            domain: "code",
            priority: 95,
            risk: crate::tools::ToolRisk::Medium,
        }));
        registry.register(Arc::new(MetaTestTool {
            name: "read_file".to_string(),
            domain: "filesystem",
            priority: 90,
            risk: crate::tools::ToolRisk::Low,
        }));

        let tools =
            registry.to_openai_format_for_prompt("run cargo test and fix the rust compile errors");
        assert_eq!(tools.len(), 128);
        let names: Vec<_> = tools
            .iter()
            .map(|tool| tool["function"]["name"].as_str().unwrap().to_string())
            .collect();
        assert!(names.contains(&"cargo_manager".to_string()));
        assert!(names.contains(&"read_file".to_string()));
        assert!(
            !names.contains(&"low_tool_139".to_string()),
            "low priority unrelated tools should be dropped first"
        );
    }

    #[test]
    fn metadata_includes_aliases_and_examples_for_tool_choice() {
        let metadata = crate::tools::ToolMetadata::infer("cargo_manager");
        assert!(metadata.aliases.contains(&"cargo test"));
        assert!(metadata.aliases.contains(&"cargo check"));
        assert!(metadata
            .examples
            .iter()
            .any(|example| example.contains("cargo test")));
        assert!(metadata.when_to_use.contains("Rust"));
        assert!(metadata.when_not_to_use.contains("read"));
    }

    #[tokio::test]
    async fn provider_tool_description_includes_compact_choice_hints() {
        let registry = ToolRegistry::new();
        let config = Config::default();
        let provider = Arc::new(crate::providers::mock::MockProvider::new());
        let sessions = SessionManager::new(std::path::PathBuf::from("/tmp/openz-test-sessions"));
        register_all_tools(&registry, &config, provider, sessions).unwrap();

        let tools = registry.to_openai_format_for_prompt("run cargo test");
        let description = tools
            .iter()
            .find(|tool| tool["function"]["name"].as_str() == Some("cargo_manager"))
            .and_then(|tool| tool["function"]["description"].as_str())
            .expect("cargo_manager description");
        assert!(description.contains("Use when:"));
        assert!(description.contains("Avoid when:"));
        assert!(description.contains("Aliases:"));
        assert!(description.contains("cargo test"));
    }

    #[test]
    fn route_analysis_formats_compact_status_line() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(MetaTestTool {
            name: "cargo_manager".to_string(),
            domain: "code",
            priority: 95,
            risk: crate::tools::ToolRisk::Medium,
        }));
        for i in 0..140 {
            registry.register(Arc::new(MetaTestTool {
                name: format!("general_tool_{i:03}"),
                domain: "general",
                priority: 1,
                risk: crate::tools::ToolRisk::Low,
            }));
        }

        let summary = registry.tool_router_status_line("run cargo test");
        assert!(summary.contains("Tool Router selected 128/141 tools"));
        assert!(summary.contains("code"));
        assert!(summary.contains("filesystem"));
        assert!(summary.contains("dropped 13"));
    }

    #[test]
    fn route_analysis_reports_api_limit_hidden_reason() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(MetaTestTool {
            name: "cargo_manager".to_string(),
            domain: "code",
            priority: 95,
            risk: crate::tools::ToolRisk::Medium,
        }));
        for i in 0..140 {
            registry.register(Arc::new(MetaTestTool {
                name: format!("general_tool_{i:03}"),
                domain: "general",
                priority: 1,
                risk: crate::tools::ToolRisk::Low,
            }));
        }

        let route = registry.route_for_prompt("run cargo test");
        assert!(route.selected_domains.contains(&"code".to_string()));
        assert!(route.dropped_count > 0);
        let hidden = route
            .entries
            .iter()
            .find(|entry| entry.hidden_reason == Some("api_limit"))
            .expect("at least one tool hidden by API limit");
        assert!(!hidden.exposed_to_model);
    }

    #[test]
    fn prompt_aware_format_selects_relevant_tool_domains() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(MetaTestTool {
            name: "web_fetch".to_string(),
            domain: "web",
            priority: 80,
            risk: crate::tools::ToolRisk::Medium,
        }));
        registry.register(Arc::new(MetaTestTool {
            name: "gsd_browser".to_string(),
            domain: "web",
            priority: 80,
            risk: crate::tools::ToolRisk::Medium,
        }));
        registry.register(Arc::new(MetaTestTool {
            name: "openmedia_image_resize".to_string(),
            domain: "media",
            priority: 80,
            risk: crate::tools::ToolRisk::Medium,
        }));
        for i in 0..140 {
            registry.register(Arc::new(MetaTestTool {
                name: format!("general_tool_{i:03}"),
                domain: "general",
                priority: 1,
                risk: crate::tools::ToolRisk::Low,
            }));
        }

        let website_tools =
            registry.to_openai_format_for_prompt("research this website and summarize the page");
        let website_names: Vec<_> = website_tools
            .iter()
            .map(|tool| tool["function"]["name"].as_str().unwrap().to_string())
            .collect();
        assert!(website_names.contains(&"web_fetch".to_string()));
        assert!(website_names.contains(&"gsd_browser".to_string()));

        let image_tools =
            registry.to_openai_format_for_prompt("resize this image and make an svg preview");
        let image_names: Vec<_> = image_tools
            .iter()
            .map(|tool| tool["function"]["name"].as_str().unwrap().to_string())
            .collect();
        assert!(image_names.contains(&"openmedia_image_resize".to_string()));
    }

    #[tokio::test]
    async fn openai_format_reserves_api_slots_for_dynamic_subagents() {
        let config = Config::default();
        let provider = Arc::new(crate::providers::mock::MockProvider::new());
        let sessions = SessionManager::new(std::path::PathBuf::from("/tmp/openz-test-sessions"));
        let registry =
            ToolRegistry::new_with_context(config.clone(), provider.clone(), sessions.clone());

        register_all_tools(&registry, &config, provider, sessions).unwrap();

        let tools = registry.to_openai_format();
        assert_eq!(tools.len(), 128);
        let names: Vec<_> = tools
            .iter()
            .map(|tool| tool["function"]["name"].as_str().unwrap().to_string())
            .collect();
        assert!(
            names.contains(&"vision_agent".to_string()),
            "vision_agent must stay available even when native tools exceed provider API limits"
        );
        assert!(
            names.contains(&"planner".to_string()),
            "planner must stay available for orchestrated subagent workflows"
        );
    }

    struct MetaTestTool {
        name: String,
        domain: &'static str,
        priority: u8,
        risk: crate::tools::ToolRisk,
    }

    #[async_trait::async_trait]
    impl crate::tools::Tool for MetaTestTool {
        fn name(&self) -> &str {
            &self.name
        }
        fn description(&self) -> &str {
            "test"
        }
        fn parameters(&self) -> serde_json::Value {
            serde_json::json!({ "type": "object" })
        }
        fn metadata(&self) -> crate::tools::ToolMetadata {
            crate::tools::ToolMetadata {
                domain: self.domain,
                risk: self.risk,
                uses_network: self.domain == "web",
                writes_disk: false,
                spawns_process: false,
                requires_approval: matches!(self.risk, crate::tools::ToolRisk::High),
                priority: self.priority,
                aliases: &[],
                examples: &[],
                when_to_use: "",
                when_not_to_use: "",
                recommended_timeout_secs: None,
            }
        }
        async fn call(&self, _arguments: &serde_json::Value) -> anyhow::Result<serde_json::Value> {
            Ok(serde_json::json!({ "ok": true }))
        }
    }
}
