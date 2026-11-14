use crate::config::schema::Config;
use anyhow::{Context, Result};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

tokio::task_local! {
    pub static ACTIVE_WORKSPACE: PathBuf;
    pub static CONFIG_DIR_OVERRIDE: PathBuf;
}

pub fn resolve_path(path_str: &str) -> PathBuf {
    let base = ACTIVE_WORKSPACE.try_with(|w| w.clone()).ok();

    let path = match path_str {
        "~/.openz" => config_dir(),
        "~" => dirs::home_dir().unwrap_or_else(|| PathBuf::from(path_str)),
        _ => match (
            path_str.strip_prefix("~/.openz/"),
            path_str.strip_prefix("~/"),
        ) {
            (Some(rel), _) => config_dir().join(rel),
            (None, Some(rel)) => match dirs::home_dir() {
                Some(home) => home.join(rel),
                None => PathBuf::from(path_str),
            },
            (None, None) => PathBuf::from(path_str),
        },
    };

    match (path.is_absolute(), base) {
        (true, _) | (false, None) => path,
        (false, Some(b)) => b.join(path),
    }
}

pub fn active_workspace_or_current_dir() -> PathBuf {
    ACTIVE_WORKSPACE
        .try_with(|w| w.clone())
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

pub fn workspace_for_agent_turn(config: &Config) -> PathBuf {
    let workspace = config.agents.defaults.workspace.trim();
    match workspace.is_empty() || workspace == "~/.openz/workspace" {
        true => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        false => resolve_path(workspace),
    }
}

pub fn set_command_cwd(cmd: &mut std::process::Command) {
    match ACTIVE_WORKSPACE.try_with(|w| w.clone()) {
        Ok(dir) => {
            cmd.current_dir(dir);
        }
        Err(_) => {}
    }
}

pub fn set_tokio_command_cwd(cmd: &mut tokio::process::Command) {
    match ACTIVE_WORKSPACE.try_with(|w| w.clone()) {
        Ok(dir) => {
            cmd.current_dir(dir);
        }
        Err(_) => {}
    }
}

pub fn verify_safe_path(path: &Path) -> Result<()> {
    crate::config::path_policy::PathPolicy::workspace()
        .validate(path)
        .map(|_| ())
}

pub fn config_dir() -> PathBuf {
    match CONFIG_DIR_OVERRIDE.try_with(|p| p.clone()) {
        Ok(dir) => dir,
        Err(_) => match std::env::var("OPENZ_CONFIG_DIR") {
            Ok(override_dir) => PathBuf::from(override_dir),
            Err(_) => dirs::home_dir()
                .map(|h| h.join(".openz"))
                .unwrap_or_else(|| PathBuf::from("~/.openz")),
        },
    }
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.json")
}

/// Known runtime database / cache filenames that must live under the global
/// OpenZ data directory (see [`runtime_data_dir`]), never inside a project or
/// workspace root. These are runtime artifacts, not source files.
pub const RUNTIME_DB_FILENAMES: &[&str] = &[
    "memory.db",
    "graph_memory.db",
    "thoughts.db",
    "ccr_cache.db",
    "embeddings_cache.json",
    "embeddings_cache.db",
];

pub const RUNTIME_SQLITE_DB_FILENAMES: &[&str] = &[
    "memory.db",
    "docs.db",
    "graph_memory.db",
    "ccr_cache.db",
    "context-bus.db",
    "logs.db",
    "thoughts.db",
    "embeddings_cache.db",
];

/// Directory where all global runtime state (databases, caches, sessions,
/// config) lives. Honors the `CONFIG_DIR_OVERRIDE` task-local (used inside
/// tool execution) and the `OPENZ_CONFIG_DIR` environment variable, falling
/// back to `~/.openz`.
///
/// This path is derived from `~/.openz` (or an explicit override) and is
/// **never** the project working directory, so runtime state never leaks into
/// a repository root.
pub fn runtime_data_dir() -> PathBuf {
    config_dir()
}

/// Resolve the on-disk path for a named runtime database/cache file.
///
/// The result always lives under [`runtime_data_dir`] and therefore never
/// resolves to the workspace or repository root, regardless of the active
/// workspace task-local. Use this for every SQLite database and cache file
/// (memory, graph memory, thoughts, CCR cache, embeddings cache).
pub fn runtime_db_path(filename: &str) -> PathBuf {
    runtime_data_dir().join(filename)
}

/// Directory where saved chat sessions are stored (`~/.openz/sessions`).
pub fn sessions_dir() -> PathBuf {
    runtime_data_dir().join("sessions")
}

/// Directory where user and system skills are stored (`~/.openz/skills`).
pub fn skills_dir() -> PathBuf {
    runtime_data_dir().join("skills")
}

/// Directory where execution traces are stored (`~/.openz/traces`).
pub fn traces_dir() -> PathBuf {
    runtime_data_dir().join("traces")
}

/// Directory where truncated tool outputs are persisted (`~/.openz/tool_outputs`).
pub fn tool_outputs_dir() -> PathBuf {
    runtime_data_dir().join("tool_outputs")
}

/// Directory where cron job execution logs are persisted (`~/.openz/cron_logs`).
pub fn cron_logs_dir() -> PathBuf {
    runtime_data_dir().join("cron_logs")
}

/// Path to the custom subagent profiles file (`~/.openz/subagents.json`).
pub fn subagents_file() -> PathBuf {
    runtime_data_dir().join("subagents.json")
}

/// Path to the global activity log file (`~/.openz/activity.json`).
pub fn activity_file() -> PathBuf {
    runtime_data_dir().join("activity.json")
}

/// Find runtime DB artifacts (e.g. `memory.db`, `*.db`, `*.db-wal`,
/// `*.db-shm`, `embeddings_cache.json`) sitting directly inside `root`.
///
/// This detects stale files that an older build may have dropped into a
/// project/working directory. It is pure and safe to call from tests.
pub fn root_runtime_db_artifacts(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    match root.is_dir() {
        false => return found,
        true => {}
    }

    let mut push_if_present = |name: &str| {
        let p = root.join(name);
        match p.is_file() {
            true => found.push(p),
            false => {}
        }
    };

    for name in RUNTIME_DB_FILENAMES {
        push_if_present(name);
    }

    // Also catch any stray SQLite companions in the root, even if the base
    // filename is not in the known list.
    match std::fs::read_dir(root) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let is_regular_file = entry
                    .file_type()
                    .map(|ft| ft.is_file())
                    .unwrap_or(false);
                match is_regular_file {
                    false => continue,
                    true => {}
                }
                let fname = entry.file_name().to_string_lossy().to_string();
                let is_sqlite_artifact = fname.ends_with(".db")
                    || fname.ends_with(".db-wal")
                    || fname.ends_with(".db-shm");
                let p = entry.path();
                match is_sqlite_artifact && !found.contains(&p) {
                    true => found.push(p),
                    false => {}
                }
            }
        }
        Err(_) => {}
    }

    found.sort();
    found
}

/// Result of scanning the working directory for stray runtime DB files.
#[derive(Debug, Clone)]
pub struct RootDbDiagnostics {
    pub root: PathBuf,
    pub found: Vec<PathBuf>,
    pub global_memory_db_exists: bool,
}

impl RootDbDiagnostics {
    /// True when one or more runtime DB artifacts were found in the root.
    pub fn has_root_runtime_dbs(&self) -> bool {
        !self.found.is_empty()
    }
}

/// Scan the current working directory for stray runtime DB artifacts and emit
/// a diagnostic warning when found. This is the "doctor" check that should run
/// at startup so stale `./memory.db` files are surfaced instead of silently
/// shadowing the real global database.
pub fn check_root_runtime_dbs() -> RootDbDiagnostics {
    let root = std::env::current_dir().unwrap_or_default();
    let found = root_runtime_db_artifacts(&root);
    let global_memory_db_exists = runtime_db_path("memory.db").exists();

    match found.is_empty() {
        false => {
            let files: Vec<String> = found
                .iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect();
            tracing::warn!(
                root = %root.display(),
                files = ?files,
                global_memory_db_exists,
                "Runtime database files found in the working directory. These are stale artifacts and should live under the global data dir (~/.openz). Migrate or archive them (e.g. via `openz doctor`) so they do not shadow the real database."
            );
        }
        true => {}
    }

    RootDbDiagnostics {
        root,
        found,
        global_memory_db_exists,
    }
}

/// Relocate stray runtime DB artifacts out of the working directory.
///
/// - If the matching global DB does not yet exist, the artifact is **migrated**
///   (moved) into the global data dir so its data is preserved.
/// - Otherwise the artifact is **archived** under
///   `~/.openz/legacy-root-backup/<timestamp>/` so nothing is destroyed.
///
/// Returns the `(from, to)` moves performed. This never deletes data.
pub fn migrate_root_runtime_dbs() -> Vec<(PathBuf, PathBuf)> {
    let diag = check_root_runtime_dbs();
    match diag.has_root_runtime_dbs() {
        false => return Vec::new(),
        true => {}
    }

    let global_dir = runtime_data_dir();
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%S");
    let archive_dir = global_dir.join(format!("legacy-root-backup/{}", stamp));

    let mut moves = Vec::new();
    for src in &diag.found {
        let fname = src
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();
        let dst = match runtime_db_path(&fname).exists() {
            true => {
                let _ = std::fs::create_dir_all(&archive_dir);
                archive_dir.join(&fname)
            }
            false => {
                let _ = std::fs::create_dir_all(&global_dir);
                global_dir.join(&fname)
            }
        };
        match std::fs::rename(src, &dst) {
            Ok(()) => moves.push((src.clone(), dst)),
            Err(_) => {}
        }
    }
    moves
}

/// Generate a unique CLI session key based on the current working directory.
/// This allows multiple `openz agent` instances in different directories
/// to each have their own session, lock, and inbox.
pub fn get_cli_session_key() -> String {
    let cwd = std::env::current_dir().unwrap_or_default();
    let path_str = cwd.to_string_lossy().to_string();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    path_str.hash(&mut hasher);
    format!("cli:{:016x}", hasher.finish())
}

const AGENT_DEFAULTS_LEGACY_ALIASES: &[&str] = &[
    "max_tokens",
    "bot_name",
    "bot_icon",
    "max_messages",
    "max_tool_iterations",
    "fallback_models",
    "caveman_mode",
    "caveman_level",
    "context_limit",
    "security_mode",
    "tool_output_limit",
    "enable_sandbox",
    "tool_timeout_secs",
    "show_tool_router_status",
    "show_auto_capture_notices",
    "tui_thought_display",
    "min_free_disk_gb",
    "allow_network_tools",
    "max_concurrent_process_tools",
    "warn_before_expensive_tools",
];

const SKILLS_LEGACY_ALIASES: &[&str] = &[
    "workspace_skills_enabled",
    "external_dirs",
    "write_approval",
];

fn object_has_any_key(value: Option<&serde_json::Value>, keys: &[&str]) -> bool {
    value
        .and_then(|v| v.as_object())
        .is_some_and(|object| keys.iter().any(|key| object.contains_key(*key)))
}

fn config_uses_legacy_aliases(raw: &serde_json::Value) -> bool {
    let agent_defaults = raw.get("agents").and_then(|agents| agents.get("defaults"));
    let skills = raw.get("skills");
    object_has_any_key(agent_defaults, AGENT_DEFAULTS_LEGACY_ALIASES)
        || object_has_any_key(skills, SKILLS_LEGACY_ALIASES)
}

const REMOVED_MCP_SERVERS: &[&str] = &[
    "sequential-thinking",
    "fetch",
    "memory",
    "headroom",
    "headroom-mcp",
    "context-bus",
    "puppeteer",
    "context7",
    "office",
    "spreadsheet",
    "just",
    "docs",
    "github",
    "database",
    "browser",
    "sediment",
];

fn migrate_security_mode_value(mode: &mut String) -> bool {
    let migrated = match mode.trim().to_ascii_lowercase().as_str() {
        "strict" => Some("high"),
        "normal" | "loose" | "" => Some("low"),
        _ => None,
    };
    match migrated {
        Some(next) => {
            *mode = next.to_string();
            true
        }
        None => false,
    }
}

pub fn migrate_config(config: &mut Config) -> bool {
    let removed_any = REMOVED_MCP_SERVERS
        .iter()
        .fold(false, |acc, name| config.mcp_servers.remove(*name).is_some() || acc);

    // v0.0.51 raised the default tool timeout from 120s to 300s. Existing
    // configs that still carry the historical default should inherit the new
    // safer default, while intentionally customized values must remain intact.
    let timeout_migrated = (config.agents.defaults.tool_timeout_secs == 120)
        .then(|| {
            config.agents.defaults.tool_timeout_secs = 300;
        })
        .is_some();

    let default_sec_migrated = migrate_security_mode_value(&mut config.agents.defaults.security_mode);
    let profiles_sec_migrated = config.ui_settings.iter_mut().fold(false, |acc, profile| {
        let changed = match profile.security_mode.as_mut() {
            Some(mode) => migrate_security_mode_value(mode),
            None => false,
        };
        changed || acc
    });

    removed_any || timeout_migrated || default_sec_migrated || profiles_sec_migrated
}

struct ConfigCache {
    config: Config,
    last_modified: std::time::SystemTime,
    path: PathBuf,
}

static CONFIG_CACHE: std::sync::Mutex<Option<ConfigCache>> = std::sync::Mutex::new(None);

/// Lock the config cache, recovering from poisoning: a panic while the lock
/// was held doesn't corrupt the cached value, so reuse the guard's data.
fn lock_config_cache() -> std::sync::MutexGuard<'static, Option<ConfigCache>> {
    CONFIG_CACHE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn read_file_mtime(path: &Path) -> std::time::SystemTime {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .unwrap_or_else(|_| std::time::SystemTime::now())
}

fn store_config_cache(
    path: PathBuf,
    config: Config,
    last_modified: std::time::SystemTime,
) -> Config {
    let mut cache = lock_config_cache();
    *cache = Some(ConfigCache {
        config: config.clone(),
        last_modified,
        path,
    });
    config
}

const MAX_AUTO_CONFIG_BACKUPS: usize = 15;

fn write_atomic_private_text(path: &Path, content: &str) -> Result<()> {
    let dir = path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(config_dir);
    match dir.exists() {
        false => {
            fs::create_dir_all(&dir)
                .with_context(|| format!("Failed to create directory at {:?}", dir))?;
        }
        true => {}
    }

    let temp_name = format!(
        "{}.tmp.{}",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("file.json"),
        uuid::Uuid::new_v4()
    );
    let temp_path = dir.join(temp_name);
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;

        let mut temp_file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temp_path)
            .with_context(|| format!("Failed to create temporary file at {:?}", temp_path))?;
        temp_file
            .write_all(content.as_bytes())
            .with_context(|| format!("Failed to write temporary file to {:?}", temp_path))?;
        temp_file
            .sync_all()
            .with_context(|| format!("Failed to sync temporary file at {:?}", temp_path))?;
    }
    #[cfg(not(unix))]
    {
        fs::write(&temp_path, content)
            .with_context(|| format!("Failed to write temporary file to {:?}", temp_path))?;
    }

    match fs::rename(&temp_path, path) {
        Ok(()) => {}
        Err(err) => {
            let _ = fs::remove_file(&temp_path);
            return Err(err).context(format!(
                "Failed to rename temporary file to {:?}",
                path
            ));
        }
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }

    Ok(())
}

fn prune_rolling_auto_backups(backups_dir: &Path) {
    let mut auto_files: Vec<(String, PathBuf)> = backups_dir
        .exists()
        .then(|| fs::read_dir(backups_dir).ok())
        .flatten()
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let entry_path = entry.path();
            let name = entry_path.file_name()?.to_str()?.to_string();
            let matches_pattern = entry_path.is_file()
                && name.starts_with("config_auto_")
                && name.ends_with(".json");
            matches_pattern.then_some((name, entry_path))
        })
        .collect();

    auto_files.sort_by(|left, right| right.0.cmp(&left.0));
    for (_, stale_path) in auto_files.into_iter().skip(MAX_AUTO_CONFIG_BACKUPS) {
        let _ = fs::remove_file(stale_path);
    }
}

fn persist_config_and_webui_backups(dir: &Path, serialized_config: &str, config: &Config) {
    let backups_dir = dir.join("backups");
    match fs::create_dir_all(&backups_dir) {
        Ok(()) => {}
        Err(_) => return,
    }

    let _ = write_atomic_private_text(&backups_dir.join("config.latest.json"), serialized_config);
    let stamp = chrono::Utc::now().format("%Y%m%d_%H%M%S_%3f").to_string();
    let _ = write_atomic_private_text(
        &backups_dir.join(format!("config_auto_{stamp}.json")),
        serialized_config,
    );
    prune_rolling_auto_backups(&backups_dir);

    match config.ui_settings.webui.as_ref() {
        Some(webui_profile) => {
            let payload = serde_json::json!({
                "version": "1.0",
                "updatedAt": chrono::Utc::now().to_rfc3339(),
                "webui": webui_profile,
            });
            match serde_json::to_string_pretty(&payload) {
                Ok(text) => {
                    let _ = write_atomic_private_text(&dir.join("webui_preferences.json"), &text);
                    let _ = write_atomic_private_text(
                        &backups_dir.join("webui_preferences.latest.json"),
                        &text,
                    );
                }
                Err(_) => {}
            }
        }
        None => {}
    }
}

fn recover_webui_profile_from_dir(
    dir: &Path,
) -> Option<crate::config::schema::UiProfileSettings> {
    let candidates = [
        dir.join("webui_preferences.json"),
        dir.join("backups").join("webui_preferences.latest.json"),
    ];
    candidates.into_iter().find_map(|candidate| {
        let raw = fs::read_to_string(candidate).ok()?;
        let parsed: serde_json::Value = serde_json::from_str(&raw).ok()?;
        let section = parsed.get("webui").unwrap_or(&parsed);
        let profile: crate::config::schema::UiProfileSettings =
            serde_json::from_value(section.clone()).ok()?;
        (profile != crate::config::schema::UiProfileSettings::default()).then_some(profile)
    })
}

fn parse_backup_config_value(raw_str: &str) -> Option<Config> {
    let raw_val: serde_json::Value = serde_json::from_str(raw_str).ok()?;
    let target = match raw_val.get("config").filter(|v| v.is_object()) {
        Some(inner) => inner.clone(),
        None => raw_val,
    };
    let mut cfg: Config = serde_json::from_value(target).ok()?;
    let _ = migrate_config(&mut cfg);
    Some(cfg)
}

fn recover_config_from_backups_dir(dir: &Path) -> Option<Config> {
    let backups_dir = dir.join("backups");
    let latest_path = backups_dir.join("config.latest.json");
    let direct = fs::read_to_string(&latest_path)
        .ok()
        .and_then(|raw| parse_backup_config_value(&raw));

    let mut recovered = direct.or_else(|| {
        let mut candidates: Vec<(String, PathBuf)> = backups_dir
            .exists()
            .then(|| fs::read_dir(&backups_dir).ok())
            .flatten()
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|entry| {
                let entry_path = entry.path();
                let name = entry_path.file_name()?.to_str()?.to_string();
                let is_candidate = entry_path.is_file()
                    && name.ends_with(".json")
                    && (name.starts_with("config_auto_") || name.starts_with("backup_"));
                is_candidate.then_some((name, entry_path))
            })
            .collect();
        candidates.sort_by(|left, right| right.0.cmp(&left.0));
        candidates.into_iter().find_map(|(_, candidate_path)| {
            let raw = fs::read_to_string(candidate_path).ok()?;
            parse_backup_config_value(&raw)
        })
    })?;

    match recovered.ui_settings.webui.is_none() {
        true => {
            recovered.ui_settings.webui = recover_webui_profile_from_dir(dir);
        }
        false => {}
    }

    Some(recovered)
}

pub fn load_config() -> Result<Config> {
    let path = config_path();
    let dir = path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(config_dir);

    let current_modified = match path.exists() {
        true => read_file_mtime(&path),
        false => {
            let mut initial_config = recover_config_from_backups_dir(&dir).unwrap_or_default();
            match initial_config.ui_settings.webui.is_none() {
                true => {
                    initial_config.ui_settings.webui = recover_webui_profile_from_dir(&dir);
                }
                false => {}
            }
            let _ = save_config(&initial_config);
            let mtime = read_file_mtime(&path);
            return Ok(store_config_cache(path, initial_config, mtime));
        }
    };

    let cached = {
        let cache = lock_config_cache();
        cache
            .as_ref()
            .filter(|c| c.path == path && c.last_modified == current_modified)
            .map(|c| c.config.clone())
    };
    match cached {
        Some(config) => return Ok(config),
        None => {}
    }

    let content = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read config file at {:?}", path))?;

    let raw_json: serde_json::Value = match serde_json::from_str(&content) {
        Ok(raw) => raw,
        Err(e) => {
            let backup_path =
                path.with_extension(format!("corrupt.{}", chrono::Utc::now().timestamp()));
            let _ = fs::copy(&path, &backup_path);
            tracing::error!(
                "Failed to parse config.json ({:?}). A backup was created at {:?}. Attempting recovery from backups.",
                e,
                backup_path
            );
            let mut fallback_config = recover_config_from_backups_dir(&dir).unwrap_or_default();
            match fallback_config.ui_settings.webui.is_none() {
                true => {
                    fallback_config.ui_settings.webui = recover_webui_profile_from_dir(&dir);
                }
                false => {}
            }
            let _ = save_config(&fallback_config);
            let mtime = read_file_mtime(&path);
            return Ok(store_config_cache(path, fallback_config, mtime));
        }
    };

    let legacy_aliases_used = config_uses_legacy_aliases(&raw_json);
    let mut config: Config = serde_json::from_value(raw_json)
        .with_context(|| format!("Failed to parse config file at {:?}", path))?;

    let webui_restored = match config.ui_settings.webui.is_none() {
        true => match recover_webui_profile_from_dir(&dir) {
            Some(profile) => {
                config.ui_settings.webui = Some(profile);
                true
            }
            None => false,
        },
        false => false,
    };

    let migrated = migrate_config(&mut config) || legacy_aliases_used || webui_restored;
    let final_modified = match migrated {
        true => {
            tracing::info!(
                path = %path.display(),
                legacy_aliases_used,
                webui_restored,
                "Migrating OpenZ config to the canonical schema"
            );
            let _ = save_config(&config);
            read_file_mtime(&path)
        }
        false => current_modified,
    };

    Ok(store_config_cache(path, config, final_modified))
}

pub fn read_config_from_path(path: &Path) -> Result<Config> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read config file at {:?}", path))?;

    let raw_json: serde_json::Value = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse config file at {:?}", path))?;

    let mut config: Config = serde_json::from_value(raw_json)
        .with_context(|| format!("Failed to deserialize config file at {:?}", path))?;

    let _ = migrate_config(&mut config);
    Ok(config)
}

pub fn load_config_from_path(path: &Path) -> Result<Config> {
    read_config_from_path(path)
}

pub fn save_config(config: &Config) -> Result<()> {
    save_config_to_path(&config_path(), config)
}

pub fn save_config_to_path(path: &Path, config: &Config) -> Result<()> {
    let dir = path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(config_dir);
    let content = serde_json::to_string_pretty(config)?;
    write_atomic_private_text(path, &content)?;
    persist_config_and_webui_backups(&dir, &content, config);

    // Invalidate the cache so the next load gets the newly saved file
    {
        let mut cache = lock_config_cache();
        *cache = None;
    }

    Ok(())
}

pub fn load_json_or_default<T: Default + serde::de::DeserializeOwned>(
    path: &std::path::Path,
) -> T {
    fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

pub fn save_pretty_json<T: serde::Serialize + ?Sized>(
    path: &std::path::Path,
    value: &T,
) -> Result<()> {
    match path.parent() {
        Some(parent) => fs::create_dir_all(parent)?,
        None => {}
    }
    fs::write(path, serde_json::to_string_pretty(value)?)?;
    Ok(())
}

pub fn save_pretty_json_atomic<T: serde::Serialize + ?Sized>(
    path: &std::path::Path,
    value: &T,
) -> Result<()> {
    match path.parent() {
        Some(parent) => fs::create_dir_all(parent)?,
        None => {}
    }
    let tmp_path = path.with_extension(format!("json.tmp.{}", uuid::Uuid::new_v4()));
    fs::write(&tmp_path, serde_json::to_string_pretty(value)?)?;
    match fs::rename(&tmp_path, path) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = fs::remove_file(&tmp_path);
            Err(e.into())
        }
    }
}

#[cfg(test)]
pub(crate) struct TestConfigDirLock(#[allow(dead_code)] crate::cron::FileLock);

#[cfg(test)]
impl TestConfigDirLock {
    pub(crate) fn acquire() -> Self {
        Self(crate::cron::FileLock::acquire(
            std::env::temp_dir().join("openz_test_config_dir.lock"),
        ))
    }
}

#[cfg(test)]
#[path = "loader_tests.rs"]
mod tests;
