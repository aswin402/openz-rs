//! Subagent execution health tracking, model fallback registry, and failure persistence.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

pub const MAX_SUBAGENT_FALLBACKS: usize = 3;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SubagentHealthRecord {
    pub last_successful_model: Option<String>,
    pub last_error: Option<String>,
    pub failure_count: u64,
    pub updated_at: Option<String>,
}

impl SubagentHealthRecord {
    pub fn mark_success(&mut self, model: &str) {
        self.last_successful_model = Some(model.to_string());
        self.last_error = None;
        self.updated_at = Some(chrono::Utc::now().to_rfc3339());
    }

    pub fn mark_failure(&mut self, error: &str) {
        self.failure_count = self.failure_count.saturating_add(1);
        self.last_error = Some(error.chars().take(500).collect());
        self.updated_at = Some(chrono::Utc::now().to_rfc3339());
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SubagentHealthRegistry {
    pub profiles: BTreeMap<String, SubagentHealthRecord>,
}

impl SubagentHealthRegistry {
    pub fn path() -> PathBuf {
        crate::config::loader::config_dir().join("subagent_health.json")
    }

    pub fn load() -> Self {
        fs::read_to_string(Self::path())
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn get(&self, profile: &str) -> Option<&SubagentHealthRecord> {
        self.profiles.get(profile)
    }

    pub fn record_success(&mut self, profile: &str, model: &str) {
        self.profiles
            .entry(profile.to_string())
            .or_default()
            .mark_success(model);
    }

    pub fn record_failure(&mut self, profile: &str, error: &str) {
        self.profiles
            .entry(profile.to_string())
            .or_default()
            .mark_failure(error);
    }
}

pub fn record_subagent_success(profile: &str, model: &str) -> Result<()> {
    let mut registry = SubagentHealthRegistry::load();
    registry.record_success(profile, model);
    registry.save()
}

pub fn record_subagent_failure(profile: &str, error: &str) -> Result<()> {
    let mut registry = SubagentHealthRegistry::load();
    registry.record_failure(profile, error);
    registry.save()
}
