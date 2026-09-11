//! Subagent profile definitions, lifecycle management, and fallback execution.

pub mod defaults;
pub mod health;
pub mod interactive;

pub use defaults::*;
pub use health::*;
pub use interactive::*;

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SubagentProfile {
    pub name: String,          // e.g. "twitter_researcher"
    pub description: String,   // What it does
    pub system_prompt: String, // System prompt tailored for its role
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>, // Primary model override
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallbacks: Option<Vec<String>>, // Fallback models override
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

pub fn subagents_file_path() -> PathBuf {
    crate::config::subagents_file()
}

struct ProfilesCache {
    profiles: Vec<SubagentProfile>,
    last_mtime: Option<std::time::SystemTime>,
}

static CACHE: std::sync::OnceLock<std::sync::Mutex<ProfilesCache>> = std::sync::OnceLock::new();

pub fn load_profiles() -> Result<Vec<SubagentProfile>> {
    let path = subagents_file_path();
    let mtime = std::fs::metadata(&path).and_then(|m| m.modified()).ok();

    let cache_mutex = CACHE.get_or_init(|| {
        std::sync::Mutex::new(ProfilesCache {
            profiles: Vec::new(),
            last_mtime: None,
        })
    });

    let mut cache = cache_mutex.lock().unwrap_or_else(|e| e.into_inner());

    if !cache.profiles.is_empty() && cache.last_mtime.is_some() && cache.last_mtime == mtime {
        return Ok(cache.profiles.clone());
    }

    let loaded = load_profiles_uncached()?;
    cache.profiles = loaded.clone();
    cache.last_mtime = std::fs::metadata(&path).and_then(|m| m.modified()).ok();

    Ok(loaded)
}

pub fn load_profiles_uncached() -> Result<Vec<SubagentProfile>> {
    let path = subagents_file_path();
    let defaults = default_profiles();

    if !path.exists() {
        save_profiles(&defaults)?;
        return Ok(defaults);
    }

    let content = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read subagents file at {:?}", path))?;

    // Attempt to parse the content as a general JSON Value first
    let parsed_json: serde_json::Value = match serde_json::from_str(&content) {
        Ok(val) => val,
        Err(e) => {
            let backup_path =
                path.with_extension(format!("corrupt.{}", chrono::Utc::now().timestamp()));
            let _ = fs::copy(&path, &backup_path);
            tracing::error!(
                "Failed to parse subagents.json ({:?}). A backup was created at {:?}. Reverting to defaults.",
                e,
                backup_path
            );
            save_profiles(&defaults)?;
            return Ok(defaults);
        }
    };

    let mut loaded_profiles = Vec::new();
    let mut has_errors = false;

    if let serde_json::Value::Array(arr) = parsed_json {
        for item in arr {
            match serde_json::from_value::<SubagentProfile>(item.clone()) {
                Ok(profile) => {
                    loaded_profiles.push(profile);
                }
                Err(e) => {
                    has_errors = true;
                    tracing::error!(
                        "Failed to parse subagent profile: {:?}. Error: {:?}",
                        item,
                        e
                    );
                }
            }
        }
    } else {
        has_errors = true;
        tracing::error!("subagents.json is not a JSON array. Reverting to defaults.");
    }

    if has_errors && loaded_profiles.is_empty() {
        let backup_path =
            path.with_extension(format!("corrupt.{}", chrono::Utc::now().timestamp()));
        let _ = fs::copy(&path, &backup_path);
        loaded_profiles = defaults.clone();
        save_profiles(&loaded_profiles)?;
        return Ok(loaded_profiles);
    }

    let mut migrated = false;
    for default_profile in defaults {
        if !loaded_profiles
            .iter()
            .any(|p| p.name == default_profile.name)
        {
            loaded_profiles.push(default_profile);
            migrated = true;
        }
    }

    for profile in &mut loaded_profiles {
        if is_default_subagent(&profile.name) {
            let is_old_default_model = matches!(
                profile.model.as_deref(),
                Some("gpt-4o-mini")
                    | Some("claude-3-5-sonnet")
                    | Some("gpt-4o")
                    | Some("google_ai_studio/gemini-2.0-flash")
            );
            if is_old_default_model {
                profile.model = None;
                profile.fallbacks = None;
                migrated = true;
            }
        }

        if let Some(ref mut fbs) = profile.fallbacks {
            fbs.retain(|s| !s.is_empty());
            if fbs.len() < MAX_SUBAGENT_FALLBACKS {
                while fbs.len() < 2 {
                    fbs.push(String::new());
                }
                fbs.push("openrouter/free".to_string());
                migrated = true;
            }
        }
    }

    if migrated || has_errors {
        save_profiles(&loaded_profiles)?;
    }

    Ok(loaded_profiles)
}

pub fn save_profiles(profiles: &[SubagentProfile]) -> Result<()> {
    let path = subagents_file_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(profiles)?;
    fs::write(&path, content)
        .with_context(|| format!("Failed to write subagents to {:?}", path))?;
    Ok(())
}
