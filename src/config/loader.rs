use crate::config::schema::Config;
use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

pub fn resolve_path(path_str: &str) -> PathBuf {
    if path_str.starts_with("~/") || path_str == "~" {
        if let Some(home) = dirs::home_dir() {
            if path_str == "~" {
                return home;
            }
            return home.join(&path_str[2..]);
        }
    }
    PathBuf::from(path_str)
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
    
    let mut config: Config = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse config file at {:?}", path))?;

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
        if !config.mcp_servers.contains_key(&name) {
            config.mcp_servers.insert(name, server_config);
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
