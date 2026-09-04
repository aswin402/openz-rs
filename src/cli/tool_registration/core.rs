use crate::tools::ToolRegistry;

pub(crate) fn register_foundation_tools(registry: &ToolRegistry) {
    registry.register(std::sync::Arc::new(crate::tools::filesystem::ReadFileTool));
    registry.register(std::sync::Arc::new(crate::tools::filesystem::FindFilesTool));
    registry.register(std::sync::Arc::new(crate::tools::doc_reader::DocReaderTool));
    registry.register(std::sync::Arc::new(crate::tools::wasm_sandbox::WasmSandboxTool));
    registry.register(std::sync::Arc::new(crate::tools::js_format::JsFormatTool));
    registry.register(std::sync::Arc::new(crate::tools::semantic_search::SemanticSearchTool));
    registry.register(std::sync::Arc::new(crate::tools::shared_memory::StoreMemoryTool));
    registry.register(std::sync::Arc::new(crate::tools::shared_memory::RecallMemoryTool));
    registry.register(std::sync::Arc::new(crate::tools::shared_memory::ClearMemoryTool));
    registry.register(std::sync::Arc::new(crate::tools::shared_memory::DeleteMemoryTool));
    registry.register(std::sync::Arc::new(crate::tools::shared_memory::UpdateMemoryTool));
    registry.register(std::sync::Arc::new(crate::tools::shared_memory::ArchiveResearchTool));
    registry.register(std::sync::Arc::new(crate::tools::shared_memory::SearchResearchTool));
    registry.register(std::sync::Arc::new(crate::tools::shared_memory::KnowledgeSourceTool));
    registry.register(std::sync::Arc::new(crate::tools::shared_memory::ResearchBriefTool));
    registry.register(std::sync::Arc::new(crate::tools::shared_memory::WorkflowMemoryTool));
}

use crate::config::schema::Config;
use crate::providers::LLMProvider;
use crate::session::SessionManager;
use crate::tools::browser_status::InspectBrowsersTool;
use crate::tools::cron::{
    GetJobLogsTool, GetJobTool, ListJobsTool, PauseJobTool, RemoveJobTool, ResumeJobTool,
    RunJobNowTool, ScheduleJobTool,
};
use crate::tools::db_inspector::{DbInspectorTool, DbWriteTool};
use crate::tools::filesystem::{
    ListDirTool, PatchFileTool, ReplaceLinesTool, WriteFileTool, ZenflowEditTool,
};
use crate::tools::network::CheckPortTool;
use crate::tools::notes::IndexNotesTool;
use crate::tools::orchestrator::OrchestrateWorkflowTool;
use crate::tools::remote::SendRemoteInputTool;
use crate::tools::rust_docs::RustDocsTool;
use crate::tools::shell::{ExecCommandTool, ManageServersTool, PythonSandboxTool};
use crate::tools::social_search::SocialSearchTool;
use crate::tools::subagent::{
    CreateSubagentTool, DelegateTaskTool, DeleteSubagentTool, OptimizeSubagentTool,
    ParallelResearchTool, UpdateSubagentSettingsTool,
};
use crate::tools::system_info::SystemInfoTool;
use crate::tools::web::WebFetchTool;
use std::sync::Arc;

pub(crate) fn register_core_tools(
    registry: &ToolRegistry,
    config: &Config,
    provider: Arc<dyn LLMProvider>,
    session_manager: SessionManager,
) {
    crate::cli::tool_registration::core::register_foundation_tools(registry);
    registry.register(std::sync::Arc::new(OrchestrateWorkflowTool::default()));
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
    registry.register(std::sync::Arc::new(ManageServersTool));
    registry.register(std::sync::Arc::new(
        crate::tools::task_manager::ManageTasksTool,
    ));
    registry.register(std::sync::Arc::new(WebFetchTool::new()));
    registry.register(std::sync::Arc::new(crate::tools::get_logs::GetLogsTool));
    registry.register(std::sync::Arc::new(
        crate::tools::manage_whitelist::ManageWhitelistTool,
    ));
    registry.register(std::sync::Arc::new(InspectBrowsersTool));
    registry.register(std::sync::Arc::new(DelegateTaskTool {
        config: config.clone(),
        parent_provider: provider.clone(),
        session_manager: session_manager.clone(),
        parent_tools: Vec::new(),
        cancellation_token: crate::tools::subagent::CancellationToken::new(),
        capability_policy: None,
    }));
    registry.register(std::sync::Arc::new(ParallelResearchTool {
        config: config.clone(),
        parent_provider: provider.clone(),
        session_manager: session_manager.clone(),
        parent_tools: Vec::new(),
        cancellation_token: crate::tools::subagent::CancellationToken::new(),
        capability_policy: None,
    }));
    registry.register(std::sync::Arc::new(
        crate::tools::subagent::EvaluatorOptimizerLoopTool {
            config: config.clone(),
            parent_provider: provider.clone(),
            session_manager: session_manager.clone(),
            parent_tools: Vec::new(),
            cancellation_token: crate::tools::subagent::CancellationToken::new(),
            capability_policy: None,
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
    registry.register(std::sync::Arc::new(UpdateSubagentSettingsTool {
        config: config.clone(),
    }));

    registry.register(std::sync::Arc::new(ScheduleJobTool));
    registry.register(std::sync::Arc::new(ListJobsTool));
    registry.register(std::sync::Arc::new(RemoveJobTool));
    registry.register(std::sync::Arc::new(PauseJobTool));
    registry.register(std::sync::Arc::new(ResumeJobTool));
    registry.register(std::sync::Arc::new(GetJobTool));
    registry.register(std::sync::Arc::new(GetJobLogsTool));
    registry.register(std::sync::Arc::new(RunJobNowTool));
    registry.register(std::sync::Arc::new(SendRemoteInputTool));
    registry.register(std::sync::Arc::new(
        crate::tools::telegram_send::TelegramSendDocumentTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::telegram_send::TelegramSendMessageTool,
    ));
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
    registry.register(std::sync::Arc::new(
        crate::tools::desktop_notify::DesktopNotifyTool,
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::device_inventory::DeviceInventoryTool::new(),
    ));
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
        crate::tools::self_management::OpenZInventoryTool::new(registry.clone()),
    ));
    registry.register(std::sync::Arc::new(
        crate::tools::self_management::RequestToolScopeTool::new(registry.clone()),
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
