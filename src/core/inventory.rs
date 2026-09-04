use crate::agent::skills::{SkillView, load_skill_views};
use crate::config::schema::Config;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeInventory {
    pub version: String,
    pub paths: RuntimePaths,
    pub defaults: RuntimeDefaults,
    pub counts: RuntimeCounts,
    pub channels: Vec<ChannelInventoryItem>,
    pub sessions: SessionInventory,
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
    pub sessions_dir: String,
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
    pub running_cron_jobs: usize,
    pub sessions: usize,
    pub active_ui_sessions: usize,
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
pub struct SessionInventory {
    pub sessions_dir: String,
    pub webui_control_center: WebUiControlCenterInventory,
    pub active_ui_sessions: Vec<ActiveUiSessionInventoryItem>,
    pub recent_sessions: Vec<RecentSessionInventoryItem>,
    pub channel_counts: Vec<SessionChannelCount>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebUiControlCenterInventory {
    pub connected_clients: usize,
    pub attached_chats: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveUiSessionInventoryItem {
    pub session_key: String,
    pub channel: String,
    pub pid: u32,
    pub cwd: String,
    pub started_at: String,
    pub last_seen_at: String,
    pub model: String,
    pub provider: String,
    pub preview: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentSessionInventoryItem {
    pub key: String,
    pub channel: String,
    pub title: String,
    pub updated_at: String,
    pub message_count: usize,
    pub file_path: String,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionChannelCount {
    pub channel: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubagentInventoryItem {
    pub name: String,
    pub description: String,
    pub model: String,
    pub provider: String,
    pub fallbacks: Vec<String>,
    pub fallback_count: usize,
    pub effective_model: String,
    pub effective_provider: String,
    pub capabilities: Vec<String>,
    pub supports_vision: bool,
    pub last_successful_model: Option<String>,
    pub last_error: Option<String>,
    pub failure_count: u64,
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
    pub prompt: String,
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
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
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
    let subagents = build_subagent_inventory(config);
    let skills = load_skill_views().unwrap_or_default();
    let channels = build_channel_inventory(config);
    let sessions = build_session_inventory();
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
            sessions_dir: path_display(&sessions_dir_path()),
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
            running_cron_jobs: cron.jobs.iter().filter(|j| j.status == "running").count(),
            sessions: sessions.recent_sessions.len(),
            active_ui_sessions: sessions.active_ui_sessions.len(),
        },
        channels,
        sessions,
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

fn sessions_dir_path() -> PathBuf {
    crate::config::loader::runtime_data_dir().join("sessions")
}

fn build_session_inventory() -> SessionInventory {
    let active_ui_sessions = crate::agent::activity::list_active_tui_sessions();
    build_session_inventory_from_parts(&sessions_dir_path(), active_ui_sessions)
}

fn build_session_inventory_from_parts(
    sessions_dir: &Path,
    active_tui_sessions: Vec<crate::agent::activity::ActiveTuiSession>,
) -> SessionInventory {
    let active_keys = active_tui_sessions
        .iter()
        .map(|session| session.session_key.clone())
        .collect::<std::collections::HashSet<_>>();

    let active_ui_sessions = active_tui_sessions
        .into_iter()
        .map(|session| ActiveUiSessionInventoryItem {
            channel: channel_from_session_key(&session.session_key).to_string(),
            session_key: session.session_key,
            pid: session.pid,
            cwd: session.cwd,
            started_at: session.started_at,
            last_seen_at: session.last_seen_at,
            model: session.model,
            provider: session.provider,
            preview: session.preview,
        })
        .collect::<Vec<_>>();

    let mut channel_counts = BTreeMap::<String, usize>::new();
    let mut recent_sessions = Vec::new();

    if let Ok(entries) = fs::read_dir(sessions_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let Ok(content) = fs::read_to_string(&path) else {
                continue;
            };
            let Ok(session) = serde_json::from_str::<crate::session::Session>(&content) else {
                continue;
            };
            let channel = channel_from_session_key(&session.key).to_string();
            *channel_counts.entry(channel.clone()).or_insert(0) += 1;
            recent_sessions.push(RecentSessionInventoryItem {
                title: crate::agent::activity::session_preview_from_messages(&session.messages),
                updated_at: session.updated_at.to_rfc3339(),
                message_count: session.messages.len(),
                file_path: path_display(&path),
                active: active_keys.contains(&session.key),
                key: session.key,
                channel,
            });
        }
    }

    recent_sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    recent_sessions.truncate(24);

    SessionInventory {
        sessions_dir: path_display(sessions_dir),
        webui_control_center: build_webui_control_center_inventory(),
        active_ui_sessions,
        recent_sessions,
        channel_counts: channel_counts
            .into_iter()
            .map(|(channel, count)| SessionChannelCount { channel, count })
            .collect(),
    }
}

fn build_webui_control_center_inventory() -> WebUiControlCenterInventory {
    let connected_clients = crate::channels::get_active_ws_senders()
        .lock()
        .map(|senders| senders.len())
        .unwrap_or(0);
    let mut attached_chats = crate::channels::get_active_ws_client_chats()
        .lock()
        .map(|chats| chats.values().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    attached_chats.sort();
    attached_chats.dedup();

    WebUiControlCenterInventory {
        connected_clients,
        attached_chats,
    }
}

fn channel_from_session_key(key: &str) -> &'static str {
    match key.split_once(':').map(|(prefix, _)| prefix) {
        Some("cli") => "tui",
        Some("ratatui") => "ratatui",
        Some("ws") => "webui",
        Some("telegram") => "telegram",
        Some("discord") => "discord",
        Some("whatsapp") => "whatsapp",
        Some("email") => "email",
        Some("subagent") => "subagent",
        _ if key.starts_with("cli_") => "tui",
        _ if key.starts_with("ws_") => "webui",
        _ if key.starts_with("telegram_") => "telegram",
        _ if key.starts_with("discord_") => "discord",
        _ if key.starts_with("whatsapp_") => "whatsapp",
        _ => "unknown",
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
            prompt: job.prompt,
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
            created_at: job.created_at,
            updated_at: job.updated_at,
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

fn build_subagent_inventory(config: &Config) -> Vec<SubagentInventoryItem> {
    build_subagent_inventory_from_profiles(
        config,
        crate::subagents::load_profiles().unwrap_or_default(),
    )
}

fn build_subagent_inventory_from_profiles(
    config: &Config,
    profiles: Vec<crate::subagents::SubagentProfile>,
) -> Vec<SubagentInventoryItem> {
    let health = crate::subagents::SubagentHealthRegistry::load();
    profiles
        .into_iter()
        .map(|profile| {
            let record = health.get(&profile.name);
            build_subagent_inventory_item(config, profile, record)
        })
        .collect()
}

fn build_subagent_inventory_item(
    config: &Config,
    profile: crate::subagents::SubagentProfile,
    health: Option<&crate::subagents::SubagentHealthRecord>,
) -> SubagentInventoryItem {
    let is_core = crate::subagents::is_default_subagent(&profile.name);
    let explicit_model = profile
        .model
        .as_deref()
        .map(str::trim)
        .filter(|model| !model.is_empty())
        .map(ToOwned::to_owned);
    let model = explicit_model
        .clone()
        .unwrap_or_else(|| "default".to_string());
    let effective_model = explicit_model.unwrap_or_else(|| config.agents.defaults.model.clone());
    let provider = if model == "default" {
        "inherit".to_string()
    } else {
        provider_from_model_or_default(&model, "auto")
    };
    let effective_provider =
        provider_from_model_or_default(&effective_model, &config.agents.defaults.provider);
    let supports_vision = crate::providers::model_supports_vision(&effective_model);
    let capabilities = infer_subagent_capabilities(&profile, supports_vision);
    let fallbacks = profile.fallbacks.unwrap_or_default();

    SubagentInventoryItem {
        name: profile.name,
        description: profile.description,
        model,
        provider,
        fallback_count: fallbacks.len(),
        fallbacks,
        effective_model,
        effective_provider,
        capabilities,
        supports_vision,
        last_successful_model: health.and_then(|record| record.last_successful_model.clone()),
        last_error: health.and_then(|record| record.last_error.clone()),
        failure_count: health.map(|record| record.failure_count).unwrap_or(0),
        is_core,
        is_protected: is_core,
        source: if is_core { "core" } else { "user" }.to_string(),
    }
}

fn provider_from_model_or_default(model: &str, default_provider: &str) -> String {
    model
        .split_once('/')
        .map(|(provider, _)| provider.to_string())
        .unwrap_or_else(|| default_provider.to_string())
}

fn infer_subagent_capabilities(
    profile: &crate::subagents::SubagentProfile,
    supports_vision: bool,
) -> Vec<String> {
    let haystack = format!(
        "{} {} {}",
        profile.name, profile.description, profile.system_prompt
    )
    .to_lowercase();
    let mut capabilities = Vec::new();

    if supports_vision || haystack.contains("vision") || haystack.contains("image") {
        capabilities.push("vision".to_string());
    }
    if haystack.contains("research")
        || haystack.contains("web")
        || haystack.contains("internet")
        || haystack.contains("docs")
    {
        capabilities.push("web".to_string());
    }
    if haystack.contains("code")
        || haystack.contains("debug")
        || haystack.contains("test")
        || haystack.contains("refactor")
        || haystack.contains("rust")
    {
        capabilities.push("code".to_string());
    }
    if haystack.contains("browser") {
        capabilities.push("browser".to_string());
    }
    if haystack.contains("memory") || haystack.contains("knowledge") {
        capabilities.push("memory".to_string());
    }
    if capabilities.is_empty() {
        capabilities.push("general".to_string());
    }
    capabilities.sort();
    capabilities.dedup();
    capabilities
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
    crate::config::loader::config_dir().display().to_string()
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
        assert!(inventory.paths.sessions_dir.ends_with("sessions"));
        assert!(inventory.counts.channels >= 4);
        assert!(
            inventory
                .subagents
                .iter()
                .any(|s| s.name == "orchestrator" && s.is_core)
        );
    }

    #[test]
    fn subagent_runtime_inventory_reports_model_and_fallbacks() {
        let mut config = Config::default();
        config.agents.defaults.model = "opencode_zen/muse-spark-1.2-contributor-free".to_string();
        config.agents.defaults.provider = "opencode_zen".to_string();
        let profiles = vec![crate::subagents::SubagentProfile {
            name: "custom_researcher".to_string(),
            description: "Research agent".to_string(),
            system_prompt: "Research with web tools when needed.".to_string(),
            model: None,
            fallbacks: Some(vec![
                "google_ai_studio/gemini-2.5-flash".to_string(),
                "mistral/mistral-large-latest".to_string(),
            ]),
            extra: serde_json::Map::new(),
        }];

        let inventory = build_subagent_inventory_from_profiles(&config, profiles);
        let item = inventory.first().expect("subagent inventory item");

        assert_eq!(item.model, "default");
        assert_eq!(item.provider, "inherit");
        assert_eq!(
            item.effective_model,
            "opencode_zen/muse-spark-1.2-contributor-free"
        );
        assert_eq!(item.effective_provider, "opencode_zen");
        assert_eq!(item.fallback_count, 2);
        assert_eq!(
            item.fallbacks,
            vec![
                "google_ai_studio/gemini-2.5-flash".to_string(),
                "mistral/mistral-large-latest".to_string(),
            ]
        );
        assert!(item.capabilities.contains(&"web".to_string()));
        assert_eq!(item.failure_count, 0);
        assert!(item.last_error.is_none());
    }

    #[test]
    fn subagent_runtime_inventory_marks_vision_support() {
        let mut config = Config::default();
        config.agents.defaults.model = "deepseek-v4-flash-free".to_string();
        config.agents.defaults.provider = "opencode_zen".to_string();
        let profiles = vec![crate::subagents::SubagentProfile {
            name: "vision_agent".to_string(),
            description: "Describe images and screenshots.".to_string(),
            system_prompt: "Use vision for image analysis.".to_string(),
            model: Some("google_ai_studio/gemini-2.5-flash".to_string()),
            fallbacks: None,
            extra: serde_json::Map::new(),
        }];

        let inventory = build_subagent_inventory_from_profiles(&config, profiles);
        let item = inventory.first().expect("subagent inventory item");

        assert_eq!(item.model, "google_ai_studio/gemini-2.5-flash");
        assert_eq!(item.provider, "google_ai_studio");
        assert_eq!(item.effective_model, "google_ai_studio/gemini-2.5-flash");
        assert_eq!(item.effective_provider, "google_ai_studio");
        assert!(item.supports_vision);
        assert!(item.capabilities.contains(&"vision".to_string()));
        assert!(item.is_core);
        assert!(item.is_protected);
    }

    #[test]
    fn session_inventory_groups_active_and_recent_sessions_by_channel() {
        let dir =
            std::env::temp_dir().join(format!("openz_inventory_sessions_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();

        write_session_fixture(&dir, "cli:abc", "hello from tui");
        write_session_fixture(&dir, "ws:control", "hello from webui");
        write_session_fixture(&dir, "telegram:42", "hello from telegram");

        let active = vec![crate::agent::activity::ActiveTuiSession {
            session_key: "cli:abc".to_string(),
            pid: 123,
            cwd: "/workspace/openz".to_string(),
            started_at: "2026-08-21T00:00:00Z".to_string(),
            last_seen_at: "2026-08-21T00:00:05Z".to_string(),
            model: "test-model".to_string(),
            provider: "test-provider".to_string(),
            preview: "hello from tui".to_string(),
        }];

        let inventory = build_session_inventory_from_parts(&dir, active);

        assert_eq!(inventory.active_ui_sessions.len(), 1);
        assert_eq!(inventory.active_ui_sessions[0].channel, "tui");
        assert_eq!(inventory.recent_sessions.len(), 3);
        assert!(
            inventory
                .recent_sessions
                .iter()
                .any(|session| session.key == "cli:abc" && session.active)
        );
        assert!(
            inventory
                .channel_counts
                .iter()
                .any(|count| count.channel == "webui" && count.count == 1)
        );
        assert!(
            inventory
                .channel_counts
                .iter()
                .any(|count| count.channel == "telegram" && count.count == 1)
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    fn write_session_fixture(dir: &std::path::Path, key: &str, prompt: &str) {
        let mut session = crate::session::Session::new(key);
        session.add_message("user", prompt);
        session.populate_hashes();
        let safe_key = key.replace(':', "_").replace('/', "_").replace('\\', "_");
        let path = dir.join(format!("{safe_key}.json"));
        std::fs::write(path, serde_json::to_string_pretty(&session).unwrap()).unwrap();
    }
}
