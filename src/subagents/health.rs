//! Subagent execution health tracking, model fallback registry, and failure persistence.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
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
        self.updated_at = Some(crate::tools::shared_memory::now_rfc3339());
    }

    pub fn mark_failure(&mut self, error: &str) {
        self.failure_count = self.failure_count.saturating_add(1);
        self.last_error = Some(crate::model_registry::truncate_health_error(error));
        self.updated_at = Some(crate::tools::shared_memory::now_rfc3339());
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
        crate::config::loader::load_json_or_default(&Self::path())
    }

    pub fn save(&self) -> Result<()> {
        crate::config::loader::save_pretty_json(&Self::path(), self)
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

fn update_subagent_health(
    profile: &str,
    mutate: impl FnOnce(&mut SubagentHealthRecord),
) -> Result<()> {
    let mut registry = SubagentHealthRegistry::load();
    mutate(registry.profiles.entry(profile.to_string()).or_default());
    registry.save()
}

pub fn record_subagent_success(profile: &str, model: &str) -> Result<()> {
    update_subagent_health(profile, |record| record.mark_success(model))
}

pub fn record_subagent_failure(profile: &str, error: &str) -> Result<()> {
    update_subagent_health(profile, |record| record.mark_failure(error))
}
