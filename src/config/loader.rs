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
    resolve_path("~/.openz")
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.json")
}

pub fn load_config() -> Result<Config> {
    let path = config_path();
    if !path.exists() {
        return Ok(Config::default());
    }

    let content = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read config file at {:?}", path))?;
    
    let config: Config = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse config file at {:?}", path))?;

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
