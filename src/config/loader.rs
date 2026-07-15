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

    let path = if path_str.starts_with("~/") || path_str == "~" {
        if let Some(home) = dirs::home_dir() {
            if path_str == "~" {
                home
            } else {
                home.join(&path_str[2..])
            }
        } else {
            PathBuf::from(path_str)
        }
    } else {
        PathBuf::from(path_str)
    };

    if path.is_absolute() {
        path
    } else if let Some(b) = base {
        b.join(path)
    } else {
        path
    }
}

pub fn set_command_cwd(cmd: &mut std::process::Command) {
    if let Ok(dir) = ACTIVE_WORKSPACE.try_with(|w| w.clone()) {
        cmd.current_dir(dir);
    }
}

pub fn set_tokio_command_cwd(cmd: &mut tokio::process::Command) {
    if let Ok(dir) = ACTIVE_WORKSPACE.try_with(|w| w.clone()) {
        cmd.current_dir(dir);
    }
}

pub fn config_dir() -> PathBuf {
    if let Ok(dir) = CONFIG_DIR_OVERRIDE.try_with(|p| p.clone()) {
        dir
    } else if let Ok(override_dir) = std::env::var("OPENZ_CONFIG_DIR") {
        PathBuf::from(override_dir)
    } else {
        resolve_path("~/.openz")
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

/// Find runtime DB artifacts (e.g. `memory.db`, `*.db`, `*.db-wal`,
/// `*.db-shm`, `embeddings_cache.json`) sitting directly inside `root`.
///
/// This detects stale files that an older build may have dropped into a
/// project/working directory. It is pure and safe to call from tests.
pub fn root_runtime_db_artifacts(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    if !root.is_dir() {
        return found;
    }

    let mut push_if_present = |name: &str| {
        let p = root.join(name);
        if p.is_file() {
            found.push(p);
        }
    };

    for name in RUNTIME_DB_FILENAMES {
        push_if_present(name);
    }

    // Also catch any stray SQLite companions in the root, even if the base
    // filename is not in the known list.
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            if let Ok(ft) = entry.file_type() {
                if !ft.is_file() {
                    continue;
                }
                let fname = entry.file_name().to_string_lossy().to_string();
                if fname.ends_with(".db")
                    || fname.ends_with(".db-wal")
                    || fname.ends_with(".db-shm")
                {
                    let p = entry.path();
                    if !found.contains(&p) {
                        found.push(p);
                    }
                }
            }
        }
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

    if !found.is_empty() {
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
    if !diag.has_root_runtime_dbs() {
        return Vec::new();
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
        let dst = if runtime_db_path(&fname).exists() {
            let _ = std::fs::create_dir_all(&archive_dir);
            archive_dir.join(&fname)
        } else {
            let _ = std::fs::create_dir_all(&global_dir);
            global_dir.join(&fname)
        };
        if let Ok(()) = std::fs::rename(src, &dst) {
            moves.push((src.clone(), dst));
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
    "context_limit",
    "security_mode",
    "tool_output_limit",
    "enable_sandbox",
    "tool_timeout_secs",
    "show_tool_router_status",
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

pub fn migrate_config(config: &mut Config) -> bool {
    let mut modified = false;
    let remove_names = [
        "sequential-thinking",
        "fetch",
        "memory",
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
    for name in &remove_names {
        if config.mcp_servers.remove(*name).is_some() {
            modified = true;
        }
    }

    // v0.0.51 raised the default tool timeout from 120s to 300s. Existing
    // configs that still carry the historical default should inherit the new
    // safer default, while intentionally customized values must remain intact.
    if config.agents.defaults.tool_timeout_secs == 120 {
        config.agents.defaults.tool_timeout_secs = 300;
        modified = true;
    }

    modified
}

pub fn load_config() -> Result<Config> {
    let path = config_path();
    if !path.exists() {
        let default_config = Config::default();
        let _ = save_config(&default_config);
        return Ok(default_config);
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
                "Failed to parse config.json ({:?}). A backup was created at {:?}. Reverting to defaults.",
                e,
                backup_path
            );
            let default_config = Config::default();
            let _ = save_config(&default_config);
            return Ok(default_config);
        }
    };

    let legacy_aliases_used = config_uses_legacy_aliases(&raw_json);
    let mut config: Config = serde_json::from_value(raw_json)
        .with_context(|| format!("Failed to parse config file at {:?}", path))?;

    let migrated = migrate_config(&mut config) || legacy_aliases_used;
    if migrated {
        tracing::info!(
            path = %path.display(),
            legacy_aliases_used,
            "Migrating OpenZ config to the canonical schema"
        );
        let _ = save_config(&config);
    }

    Ok(config)
}

pub fn save_config(config: &Config) -> Result<()> {
    let dir = config_dir();
    if !dir.exists() {
        fs::create_dir_all(&dir)
            .with_context(|| format!("Failed to create config directory at {:?}", dir))?;
    }

    let path = config_path();
    let content = serde_json::to_string_pretty(config)?;

    // Write atomically: write to a temporary file first, then rename it
    let temp_name = format!("config.json.tmp.{}", uuid::Uuid::new_v4());
    let temp_path = dir.join(temp_name);
    fs::write(&temp_path, content)
        .with_context(|| format!("Failed to write temporary config file to {:?}", temp_path))?;

    if let Err(e) = fs::rename(&temp_path, &path) {
        let _ = fs::remove_file(&temp_path);
        return Err(e).context(format!(
            "Failed to rename temporary config file to {:?}",
            path
        ));
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn cli_session_key_changes_with_current_directory() {
        let original = std::env::current_dir().unwrap();
        let first =
            std::env::temp_dir().join(format!("openz_session_key_a_{}", uuid::Uuid::new_v4()));
        let second =
            std::env::temp_dir().join(format!("openz_session_key_b_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&first).unwrap();
        std::fs::create_dir_all(&second).unwrap();

        std::env::set_current_dir(&first).unwrap();
        let first_key = super::get_cli_session_key();
        std::env::set_current_dir(&second).unwrap();
        let second_key = super::get_cli_session_key();
        std::env::set_current_dir(original).unwrap();

        let _ = std::fs::remove_dir_all(first);
        let _ = std::fs::remove_dir_all(second);

        assert_ne!(first_key, second_key);
        assert!(first_key.starts_with("cli:"));
        assert!(second_key.starts_with("cli:"));
    }

    #[tokio::test]
    async fn runtime_db_path_never_resolves_to_workspace_root() {
        let workspace = std::env::temp_dir().join(format!("openz_ws_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&workspace).unwrap();

        // runtime_db_path must be derived from ~/.openz, independent of the
        // active workspace task-local, so it can never land in the repo root.
        let path = super::ACTIVE_WORKSPACE
            .scope(workspace.clone(), async {
                super::runtime_db_path("memory.db")
            })
            .await;

        assert!(
            !path.starts_with(&workspace),
            "runtime DB resolved inside workspace: {path:?}"
        );
        assert!(path.ends_with("memory.db"));
        assert!(path.to_string_lossy().contains(".openz"));
        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[test]
    fn openz_config_dir_moves_db_location() {
        let custom = std::env::temp_dir().join(format!("openz_cfg_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&custom).unwrap();
        std::env::set_var("OPENZ_CONFIG_DIR", &custom);

        let path = super::runtime_db_path("memory.db");

        assert_eq!(path, custom.join("memory.db"));
        std::env::remove_var("OPENZ_CONFIG_DIR");
        let _ = std::fs::remove_dir_all(&custom);
    }

    #[test]
    fn root_memory_db_triggers_diagnostic() {
        let root = std::env::temp_dir().join(format!("openz_root_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("memory.db"), b"stale").unwrap();
        std::fs::write(root.join("memory.db-shm"), b"").unwrap();
        std::fs::write(root.join("memory.db-wal"), b"").unwrap();

        let found = super::root_runtime_db_artifacts(&root);
        let names: Vec<String> = found
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();

        assert!(
            names.contains(&"memory.db".to_string()),
            "memory.db not detected: {names:?}"
        );
        assert!(names.contains(&"memory.db-shm".to_string()));
        assert!(names.contains(&"memory.db-wal".to_string()));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn load_config_migrates_historical_tool_timeout_default() {
        let dir = std::env::temp_dir().join(format!(
            "openz_cfg_timeout_migration_{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("config.json"),
            serde_json::json!({
                "agents": { "defaults": { "toolTimeoutSecs": 120 } }
            })
            .to_string(),
        )
        .unwrap();

        let config = super::CONFIG_DIR_OVERRIDE
            .scope(dir.clone(), async { super::load_config().unwrap() })
            .await;

        assert_eq!(config.agents.defaults.tool_timeout_secs, 300);
        let saved: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("config.json")).unwrap())
                .unwrap();
        assert_eq!(saved["agents"]["defaults"]["toolTimeoutSecs"], 300);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn load_config_preserves_custom_tool_timeout() {
        let dir =
            std::env::temp_dir().join(format!("openz_cfg_timeout_custom_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("config.json"),
            serde_json::json!({
                "agents": { "defaults": { "toolTimeoutSecs": 60 } }
            })
            .to_string(),
        )
        .unwrap();

        let config = super::CONFIG_DIR_OVERRIDE
            .scope(dir.clone(), async { super::load_config().unwrap() })
            .await;

        assert_eq!(config.agents.defaults.tool_timeout_secs, 60);
        let saved: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("config.json")).unwrap())
                .unwrap();
        assert_eq!(saved["agents"]["defaults"]["toolTimeoutSecs"], 60);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn load_config_rewrites_legacy_aliases_to_canonical_schema() {
        let dir = std::env::temp_dir().join(format!(
            "openz_cfg_alias_migration_{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("config.json"),
            serde_json::json!({
                "agents": {
                    "defaults": {
                        "max_tokens": 1234,
                        "enable_sandbox": true,
                        "tool_timeout_secs": 77,
                        "show_tool_router_status": true
                    }
                },
                "skills": {
                    "workspace_skills_enabled": false,
                    "external_dirs": ["/tmp/skills"],
                    "write_approval": true
                },
                "mcp_servers": {
                    "memory": { "command": "old-memory", "args": [], "enabled": true }
                }
            })
            .to_string(),
        )
        .unwrap();

        let config = super::CONFIG_DIR_OVERRIDE
            .scope(dir.clone(), async { super::load_config().unwrap() })
            .await;

        assert_eq!(config.agents.defaults.max_tokens, 1234);
        assert!(config.agents.defaults.enable_sandbox);
        assert_eq!(config.agents.defaults.tool_timeout_secs, 77);
        assert!(config.agents.defaults.show_tool_router_status);
        assert!(!config.skills.workspace_skills_enabled);
        assert_eq!(config.skills.external_dirs, vec!["/tmp/skills".to_string()]);
        assert!(config.skills.write_approval);
        assert!(!config.mcp_servers.contains_key("memory"));

        let saved: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("config.json")).unwrap())
                .unwrap();
        let defaults = &saved["agents"]["defaults"];
        assert_eq!(defaults["maxTokens"], 1234);
        assert_eq!(defaults["enableSandbox"], true);
        assert_eq!(defaults["toolTimeoutSecs"], 77);
        assert_eq!(defaults["showToolRouterStatus"], true);
        assert!(defaults.get("max_tokens").is_none());
        assert!(defaults.get("enable_sandbox").is_none());
        assert!(defaults.get("tool_timeout_secs").is_none());
        assert!(defaults.get("show_tool_router_status").is_none());

        assert_eq!(saved["skills"]["workspaceSkillsEnabled"], false);
        assert_eq!(saved["skills"]["externalDirs"][0], "/tmp/skills");
        assert_eq!(saved["skills"]["writeApproval"], true);
        assert!(saved["skills"].get("workspace_skills_enabled").is_none());
        assert!(saved["skills"].get("external_dirs").is_none());
        assert!(saved["skills"].get("write_approval").is_none());
        assert!(saved["mcp_servers"].get("memory").is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn workspace_openz_runtime_files_are_gitignored() {
        let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let gitignore = std::fs::read_to_string(format!("{manifest}/.gitignore"))
            .expect(".gitignore must exist at repo root");
        let lower = gitignore.to_lowercase();
        assert!(
            lower.contains(".openz/"),
            ".gitignore must ignore workspace .openz/ runtime dir"
        );
        assert!(
            lower.contains("/memory.db"),
            ".gitignore must ignore root memory.db"
        );
        assert!(
            lower.contains(".db-wal"),
            ".gitignore must ignore sqlite -wal companions"
        );
    }
}
