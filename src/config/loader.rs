use crate::config::schema::Config;
use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

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

pub fn migrate_config(config: &mut Config) -> bool {
    let mut modified = false;
    let remove_names = [
        "sequential-thinking", "fetch", "memory", "puppeteer", "context7",
        "office", "spreadsheet", "just", "docs", "github", "database", "browser", "sediment"
    ];
    for name in &remove_names {
        if config.mcp_servers.remove(*name).is_some() {
            modified = true;
        }
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
    
    let mut config: Config = match serde_json::from_str(&content) {
        Ok(c) => c,
        Err(e) => {
            let backup_path = path.with_extension(format!("corrupt.{}", chrono::Utc::now().timestamp()));
            let _ = fs::copy(&path, &backup_path);
            eprintln!(
                "⚠️ Warning: Failed to parse config.json ({:?}). A backup was created at {:?}. Reverting to defaults.",
                e, backup_path
            );
            let default_config = Config::default();
            let _ = save_config(&default_config);
            default_config
        }
    };

    // Migrate the config in memory so the running app gets migrated settings,
    // but do not automatically save to disk during read to avoid write side effects.
    let _ = migrate_config(&mut config);

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
        return Err(e).context(format!("Failed to rename temporary config file to {:?}", path));
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }

    Ok(())
}
