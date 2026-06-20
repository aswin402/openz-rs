use crate::config::schema::Config;
use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

tokio::task_local! {
    pub static ACTIVE_WORKSPACE: PathBuf;
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
    if let Ok(override_dir) = std::env::var("OPENZ_CONFIG_DIR") {
        PathBuf::from(override_dir)
    } else {
        resolve_path("~/.openz")
    }
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.json")
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

    // Clean up any Node/JS based default MCP servers from existing config
    let mut modified = false;
    let remove_names = ["sequential-thinking", "fetch", "memory", "puppeteer", "context7"];
    for name in &remove_names {
        if let Some(mcp) = config.mcp_servers.get(*name) {
            if mcp.command == "npx" {
                config.mcp_servers.remove(*name);
                modified = true;
            }
        }
    }

    // Auto-populate default Rust MCP servers if they are missing
    let defaults = Config::default();
    for (name, server_config) in defaults.mcp_servers {
        if let std::collections::hash_map::Entry::Vacant(e) = config.mcp_servers.entry(name) {
            e.insert(server_config);
            modified = true;
        }
    }

    // Upgrade existing memory server config to use gRPC if args are empty
    if let Some(mcp) = config.mcp_servers.get_mut("memory") {
        if mcp.command.contains("openmemory_rs") && mcp.args.is_empty() {
            mcp.args = vec!["--grpc".to_string(), "50051".to_string()];
            modified = true;
        }
    }

    // Upgrade existing database server config to sqlite default if only "stdio" is set
    if let Some(mcp) = config.mcp_servers.get_mut("database") {
        if mcp.args.len() == 1 && mcp.args[0] == "stdio" {
            let db_path = if let Some(home) = dirs::home_dir() {
                home.join(".openz").join("memory.db").to_string_lossy().to_string()
            } else {
                "memory.db".to_string()
            };
            mcp.args = vec![
                "stdio".to_string(),
                "--db-backend".to_string(),
                "sqlite".to_string(),
                "--db-name".to_string(),
                db_path,
            ];
            modified = true;
        }
    }

    if modified {
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
    fs::write(&path, content)
        .with_context(|| format!("Failed to write config file to {:?}", path))?;

    Ok(())
}
