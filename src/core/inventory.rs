use crate::agent::skills::{SkillView, load_skill_views};
use crate::config::schema::Config;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeInventory {
    pub version: String,
    pub paths: RuntimePaths,
    pub defaults: RuntimeDefaults,
    pub counts: RuntimeCounts,
    pub channels: Vec<ChannelInventoryItem>,
    pub subagents: Vec<SubagentInventoryItem>,
    pub skills: Vec<SkillView>,
    pub memory: MemoryInventory,
    pub tools: Vec<ToolInventoryItem>,
    pub cron: CronInventory,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePaths {
    pub config_dir: String,
    pub workspace: String,
    pub memory_db: String,
    pub graph_db: String,
    pub subagents_file: String,
    pub skills_dir: String,
    pub workspace_skills_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeDefaults {
    pub model: String,
    pub provider: String,
    pub streaming: bool,
    pub caveman_mode: bool,
    pub max_messages: usize,
    pub max_tool_iterations: usize,
    pub tool_timeout_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCounts {
    pub subagents: usize,
    pub skills: usize,
    pub core_subagents: usize,
    pub custom_subagents: usize,
    pub channels: usize,
    pub enabled_channels: usize,
    pub tools: usize,
    pub cron_jobs: usize,
    pub active_cron_jobs: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelInventoryItem {
    pub name: String,
    pub enabled: bool,
    pub configured: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubagentInventoryItem {
    pub name: String,
    pub description: String,
    pub model: String,
    pub provider: String,
    pub fallback_count: usize,
    pub is_core: bool,
    pub is_protected: bool,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryInventory {
    pub memory_db: DatabaseInventoryItem,
    pub graph_db: DatabaseInventoryItem,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolInventoryItem {
    pub name: String,
    pub domain: String,
    pub risk: String,
    pub uses_network: bool,
    pub writes_disk: bool,
    pub spawns_process: bool,
    pub requires_approval: bool,
    pub priority: u8,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CronInventory {
    pub jobs_file: String,
    pub runs_file: String,
    pub jobs: Vec<CronJobInventoryItem>,
    pub recent_runs: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CronJobInventoryItem {
    pub id: String,
    pub schedule: String,
    pub enabled: bool,
    pub run_once: bool,
    pub status: String,
    pub quiet: bool,
    pub notify_on: String,
    pub next_run: Option<String>,
    pub last_run: Option<String>,
    pub last_started_at: Option<String>,
    pub last_finished_at: Option<String>,
    pub last_error: Option<String>,
    pub last_log_path: Option<String>,
    pub run_count: u64,
    pub failure_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseInventoryItem {
    pub path: String,
    pub exists: bool,
}

pub fn build_runtime_inventory(
    config: &Config,
    tools: Option<&crate::tools::ToolRegistry>,
) -> RuntimeInventory {
    let subagents = build_subagent_inventory();
    let skills = load_skill_views().unwrap_or_default();
    let channels = build_channel_inventory(config);
    let tool_inventory = tools.map(build_tool_inventory).unwrap_or_default();
    let cron = build_cron_inventory();
    let memory_db = crate::config::loader::runtime_db_path("memory.db");
    let graph_db = crate::config::loader::runtime_db_path("graph_memory.db");

    RuntimeInventory {
        version: env!("CARGO_PKG_VERSION").to_string(),
        paths: RuntimePaths {
            config_dir: config_dir_display(),
            workspace: config.agents.defaults.workspace.clone(),
            memory_db: path_display(&memory_db),
            graph_db: path_display(&graph_db),
            subagents_file: path_display(&crate::subagents::subagents_file_path()),
            skills_dir: path_display(&crate::agent::skills::get_skills_dir()),
            workspace_skills_dir: path_display(&crate::agent::skills::get_workspace_skills_dir()),
        },
        defaults: RuntimeDefaults {
            model: config.agents.defaults.model.clone(),
            provider: config.agents.defaults.provider.clone(),
            streaming: config.agents.defaults.streaming,
            caveman_mode: config.agents.defaults.caveman_mode,
            max_messages: config.agents.defaults.max_messages,
            max_tool_iterations: config.agents.defaults.max_tool_iterations,
            tool_timeout_secs: config.agents.defaults.tool_timeout_secs,
        },
        counts: RuntimeCounts {
            subagents: subagents.len(),
            skills: skills.len(),
            core_subagents: subagents.iter().filter(|s| s.is_core).count(),
            custom_subagents: subagents.iter().filter(|s| !s.is_core).count(),
            channels: channels.len(),
            enabled_channels: channels.iter().filter(|c| c.enabled).count(),
            tools: tool_inventory.len(),
            cron_jobs: cron.jobs.len(),
            active_cron_jobs: cron.jobs.iter().filter(|j| j.enabled).count(),
        },
        channels,
        subagents,
        skills,
        memory: MemoryInventory {
            memory_db: database_inventory(memory_db),
            graph_db: database_inventory(graph_db),
        },
        tools: tool_inventory,
        cron,
    }
}

fn build_tool_inventory(registry: &crate::tools::ToolRegistry) -> Vec<ToolInventoryItem> {
    registry
        .tool_inventory_snapshot()
        .into_iter()
        .map(|(name, description, metadata)| ToolInventoryItem {
            name,
            domain: metadata.domain.to_string(),
            risk: metadata.risk.as_str().to_string(),
            uses_network: metadata.uses_network,
            writes_disk: metadata.writes_disk,
            spawns_process: metadata.spawns_process,
            requires_approval: metadata.requires_approval,
            priority: metadata.priority,
            description,
        })
        .collect()
}

fn build_cron_inventory() -> CronInventory {
    let jobs_file = crate::cron::cron_file_path();
    let runs_file = crate::cron::cron_runs_file_path();
    let jobs = crate::cron::load_jobs()
        .unwrap_or_default()
        .into_iter()
        .map(|job| CronJobInventoryItem {
            id: job.id,
            schedule: job.schedule,
            enabled: job.enabled,
            run_once: job.run_once,
            status: serde_json::to_value(&job.status)
                .ok()
                .and_then(|v| v.as_str().map(ToOwned::to_owned))
                .unwrap_or_else(|| "unknown".to_string()),
            quiet: job.quiet,
            notify_on: serde_json::to_value(&job.notify_on)
                .ok()
                .and_then(|v| v.as_str().map(ToOwned::to_owned))
                .unwrap_or_else(|| "failure".to_string()),
            next_run: job.next_run,
            last_run: job.last_run,
            last_started_at: job.last_started_at,
            last_finished_at: job.last_finished_at,
            last_error: job.last_error,
            last_log_path: job.last_log_path,
            run_count: job.run_count,
            failure_count: job.failure_count,
        })
        .collect();
    let recent_runs = crate::cron::load_cron_run_records(None, 100)
        .map(|runs| runs.len())
        .unwrap_or(0);

    CronInventory {
        jobs_file: path_display(&jobs_file),
        runs_file: path_display(&runs_file),
        jobs,
        recent_runs,
    }
}

fn build_subagent_inventory() -> Vec<SubagentInventoryItem> {
    crate::subagents::load_profiles()
        .unwrap_or_default()
        .into_iter()
        .map(|profile| {
            let is_core = crate::subagents::is_default_subagent(&profile.name);
            let model = profile.model.unwrap_or_else(|| "default".to_string());
            let provider = model
                .split_once('/')
                .map(|(provider, _)| provider.to_string())
                .unwrap_or_else(|| "auto".to_string());
            SubagentInventoryItem {
                name: profile.name,
                description: profile.description,
                fallback_count: profile.fallbacks.as_ref().map_or(0, Vec::len),
                model,
                provider,
                is_core,
                is_protected: is_core,
                source: if is_core { "core" } else { "user" }.to_string(),
            }
        })
        .collect()
}

fn build_channel_inventory(config: &Config) -> Vec<ChannelInventoryItem> {
    let websocket = config.channels.websocket.as_ref();
    let telegram = config.channels.telegram.as_ref();
    let discord = config.channels.discord.as_ref();
    let whatsapp = config.channels.whatsapp.as_ref();
    let email = config.channels.email.as_ref();

    vec![
        ChannelInventoryItem {
            name: "websocket".to_string(),
            enabled: websocket.map(|c| c.enabled).unwrap_or(true),
            configured: websocket.is_some(),
        },
        ChannelInventoryItem {
            name: "telegram".to_string(),
            enabled: telegram.map(|c| c.enabled).unwrap_or(false),
            configured: telegram.is_some_and(|c| !c.bot_token.trim().is_empty()),
        },
        ChannelInventoryItem {
            name: "discord".to_string(),
            enabled: discord.map(|c| c.enabled).unwrap_or(false),
            configured: discord.is_some_and(|c| !c.bot_token.trim().is_empty()),
        },
        ChannelInventoryItem {
            name: "whatsapp".to_string(),
            enabled: whatsapp.map(|c| c.enabled).unwrap_or(false),
            configured: whatsapp.is_some_and(|c| !c.api_key.trim().is_empty()),
        },
        ChannelInventoryItem {
            name: "email".to_string(),
            enabled: email.map(|c| c.enabled).unwrap_or(false),
            configured: email
                .is_some_and(|c| !c.username.trim().is_empty() && !c.password.trim().is_empty()),
        },
    ]
}

fn database_inventory(path: PathBuf) -> DatabaseInventoryItem {
    DatabaseInventoryItem {
        exists: path.exists(),
        path: path_display(&path),
    }
}

fn path_display(path: &std::path::Path) -> String {
    path.display().to_string()
}

fn config_dir_display() -> String {
    std::env::var("OPENZ_CONFIG_DIR").unwrap_or_else(|_| {
        crate::config::resolve_path("~/.openz")
            .display()
            .to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_inventory_contains_core_paths_and_counts() {
        let config = Config::default();
        let inventory = build_runtime_inventory(&config, None);

        assert_eq!(inventory.version, env!("CARGO_PKG_VERSION"));
        assert!(inventory.paths.memory_db.ends_with("memory.db"));
        assert!(inventory.paths.graph_db.ends_with("graph_memory.db"));
        assert!(inventory.counts.channels >= 4);
        assert!(
            inventory
                .subagents
                .iter()
                .any(|s| s.name == "orchestrator" && s.is_core)
        );
    }
}
