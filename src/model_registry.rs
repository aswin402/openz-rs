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
        let now = chrono::Utc::now().to_rfc3339();
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
        let now = chrono::Utc::now().to_rfc3339();
        self.last_checked = now.clone();
        self.last_success = Some(now);
        self.success_count = self.success_count.saturating_add(1);
        self.last_error = None;
        if blank {
            self.blank_response_count = self.blank_response_count.saturating_add(1);
        }
        if think_leak {
            self.think_leak_count = self.think_leak_count.saturating_add(1);
        }
        if fallback_used {
            self.fallback_count = self.fallback_count.saturating_add(1);
        }
    }

    pub fn mark_failure(&mut self, error: &str) {
        self.last_checked = chrono::Utc::now().to_rfc3339();
        self.failure_count = self.failure_count.saturating_add(1);
        self.last_error = Some(error.chars().take(500).collect());
    }
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
        let path = Self::path();
        let Ok(raw) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        serde_json::from_str(&raw).unwrap_or_default()
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let raw = serde_json::to_string_pretty(self)?;
        std::fs::write(path, raw)?;
        Ok(())
    }

    pub fn get(&self, provider: &str, model: &str) -> Option<&ModelHealthRecord> {
        self.models.get(&Self::key(provider, model))
    }

    pub fn upsert(&mut self, record: ModelHealthRecord) {
        self.models
            .insert(Self::key(&record.provider, &record.model), record);
    }
}

pub fn record_model_risk(
    provider: &str,
    model: &str,
    tier: &str,
    risky: bool,
    reasons: Vec<String>,
) -> Result<()> {
    let mut registry = ModelRegistry::load();
    let key = ModelRegistry::key(provider, model);
    let now = chrono::Utc::now().to_rfc3339();
    let mut record = registry
        .models
        .remove(&key)
        .unwrap_or_else(|| ModelHealthRecord::new(provider, model, tier, risky, reasons.clone()));
    record.tier = tier.to_string();
    record.risky = risky;
    record.reasons = reasons;
    record.last_checked = now;
    registry.upsert(record);
    registry.save()
}

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
    let mut registry = ModelRegistry::load();
    let key = ModelRegistry::key(provider, model);
    let mut record = registry
        .models
        .remove(&key)
        .unwrap_or_else(|| ModelHealthRecord::new(provider, model, tier, risky, reasons.clone()));
    record.tier = tier.to_string();
    record.risky = risky;
    record.reasons = reasons;
    record.mark_success(blank, think_leak, fallback_used);
    registry.upsert(record);
    registry.save()
}

pub fn record_model_failure(
    provider: &str,
    model: &str,
    tier: &str,
    risky: bool,
    reasons: Vec<String>,
    error: &str,
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
    record.mark_failure(error);
    registry.upsert(record);
    registry.save()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_registry_key_is_stable() {
        assert_eq!(
            ModelRegistry::key("groq", "llama-3.1-8b-instant"),
            "groq::llama-3.1-8b-instant"
        );
    }

    #[test]
    fn health_record_truncates_long_errors() {
        let mut record = ModelHealthRecord::new("p", "m", "risky", true, vec![]);
        record.mark_failure(&"x".repeat(800));
        assert_eq!(record.last_error.unwrap().len(), 500);
    }

    #[test]
    fn health_record_tracks_success_and_failure() {
        let mut record = ModelHealthRecord::new(
            "opencode_zen",
            "big-pickle",
            "risky",
            true,
            vec!["unknown".to_string()],
        );
        record.mark_success(true, true, false);
        record.mark_failure("provider failed");
        assert_eq!(record.success_count, 1);
        assert_eq!(record.failure_count, 1);
        assert_eq!(record.blank_response_count, 1);
        assert_eq!(record.think_leak_count, 1);
        assert_eq!(record.last_error.as_deref(), Some("provider failed"));
    }
}
