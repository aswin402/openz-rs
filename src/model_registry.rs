use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelHealthRecord {
    pub provider: String,
    pub model: String,
    pub tier: String,
    pub risky: bool,
    pub reasons: Vec<String>,
    pub last_checked: String,
    pub last_success: Option<String>,
    pub success_count: u64,
    pub failure_count: u64,
    pub blank_response_count: u64,
    pub think_leak_count: u64,
    pub fallback_count: u64,
    pub last_error: Option<String>,
}

impl ModelHealthRecord {
    pub fn new(provider: &str, model: &str, tier: &str, risky: bool, reasons: Vec<String>) -> Self {
        let now = crate::tools::shared_memory::now_rfc3339();
        Self {
            provider: provider.to_string(),
            model: model.to_string(),
            tier: tier.to_string(),
            risky,
            reasons,
            last_checked: now,
            last_success: None,
            success_count: 0,
            failure_count: 0,
            blank_response_count: 0,
            think_leak_count: 0,
            fallback_count: 0,
            last_error: None,
        }
    }

    pub fn mark_success(&mut self, blank: bool, think_leak: bool, fallback_used: bool) {
        let now = crate::tools::shared_memory::now_rfc3339();
        self.last_checked = now.clone();
        self.last_success = Some(now);
        self.success_count = self.success_count.saturating_add(1);
        self.last_error = None;
        self.blank_response_count = self
            .blank_response_count
            .saturating_add(u64::from(blank));
        self.think_leak_count = self
            .think_leak_count
            .saturating_add(u64::from(think_leak));
        self.fallback_count = self
            .fallback_count
            .saturating_add(u64::from(fallback_used));
    }

    pub fn mark_failure(&mut self, error: &str) {
        self.last_checked = crate::tools::shared_memory::now_rfc3339();
        self.failure_count = self.failure_count.saturating_add(1);
        self.last_error = Some(truncate_health_error(error));
    }
}

pub(crate) fn truncate_health_error(error: &str) -> String {
    error.chars().take(500).collect()
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelRegistry {
    pub models: BTreeMap<String, ModelHealthRecord>,
}

impl ModelRegistry {
    pub fn key(provider: &str, model: &str) -> String {
        format!("{}::{}", provider.trim(), model.trim())
    }

    pub fn path() -> PathBuf {
        crate::config::loader::config_dir().join("model_registry.json")
    }

    pub fn load() -> Self {
        crate::config::loader::load_json_or_default(&Self::path())
    }

    pub fn save(&self) -> Result<()> {
        crate::config::loader::save_pretty_json(&Self::path(), self)
    }

    pub fn get(&self, provider: &str, model: &str) -> Option<&ModelHealthRecord> {
        self.models.get(&Self::key(provider, model))
    }

    pub fn upsert(&mut self, record: ModelHealthRecord) {
        self.models
            .insert(Self::key(&record.provider, &record.model), record);
    }
}

fn update_model_record(
    provider: &str,
    model: &str,
    tier: &str,
    risky: bool,
    reasons: Vec<String>,
    mutate: impl FnOnce(&mut ModelHealthRecord),
) -> Result<()> {
    let mut registry = ModelRegistry::load();
    let key = ModelRegistry::key(provider, model);
    let mut record = registry
        .models
        .remove(&key)
        .unwrap_or_else(|| ModelHealthRecord::new(provider, model, tier, risky, reasons.clone()));
    record.tier = tier.to_string();
    record.risky = risky;
    record.reasons = reasons;
    mutate(&mut record);
    registry.upsert(record);
    registry.save()
}

pub fn record_model_risk(
    provider: &str,
    model: &str,
    tier: &str,
    risky: bool,
    reasons: Vec<String>,
) -> Result<()> {
    update_model_record(provider, model, tier, risky, reasons, |record| {
        record.last_checked = crate::tools::shared_memory::now_rfc3339();
    })
}

#[allow(clippy::too_many_arguments)]
pub fn record_model_success(
    provider: &str,
    model: &str,
    tier: &str,
    risky: bool,
    reasons: Vec<String>,
    blank: bool,
    think_leak: bool,
    fallback_used: bool,
) -> Result<()> {
    update_model_record(provider, model, tier, risky, reasons, |record| {
        record.mark_success(blank, think_leak, fallback_used);
    })
}

pub fn record_model_failure(
    provider: &str,
    model: &str,
    tier: &str,
    risky: bool,
    reasons: Vec<String>,
    error: &str,
) -> Result<()> {
    update_model_record(provider, model, tier, risky, reasons, |record| {
        record.mark_failure(error);
    })
}

#[cfg(test)]
#[path = "model_registry_tests.rs"]
mod tests;
