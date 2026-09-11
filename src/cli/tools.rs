use crate::config::schema::Config;
use crate::providers::LLMProvider;
use crate::session::SessionManager;
use crate::tools::ToolRegistry;
use anyhow::Result;
use std::sync::Arc;

pub fn register_all_tools(
    registry: &ToolRegistry,
    config: &Config,
    provider: Arc<dyn LLMProvider>,
    session_manager: SessionManager,
) -> Result<()> {
    crate::cli::tool_registration::core::register_core_tools(
        registry,
        config,
        provider.clone(),
        session_manager.clone(),
    );
    crate::cli::tool_registration::memory::register_sequential_thinking_tools(registry);
    crate::cli::tool_registration::memory::register_headroom_tools(registry);
    crate::cli::tool_registration::memory::register_graph_memory_tools(registry);
    crate::cli::tool_registration::memory::register_memory_extra_tools(registry);
    crate::cli::tool_registration::integrations::register_searchxyz_tools(registry);
    crate::cli::tool_registration::media::register_openmedia_tools(registry);
    crate::cli::tool_registration::media::register_opendoc_tools(registry);
    crate::cli::tool_registration::integrations::register_github_mcp_tools(registry);
    crate::cli::tool_registration::integrations::register_docs_mcp_tools(registry);
    crate::cli::tool_registration::integrations::register_lazy_mcp_tools(registry, config);

    Ok(())
}

#[cfg(test)]
#[path = "tools_tests.rs"]
mod tests;
