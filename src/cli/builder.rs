use crate::agent::AgentLoop;
use crate::config::loader::sessions_dir;
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
    let _ = crate::core::vault::ensure_vault_initialized(&config);
    let resolved =
        crate::providers::resolver::resolve_provider_full(&config, &config.agents.defaults.model)?;
    let provider = resolved.instance;

    let sessions_dir = sessions_dir();
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
#[path = "builder_tests.rs"]
mod tests;
