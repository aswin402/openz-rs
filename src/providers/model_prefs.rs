use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelRef {
    pub provider: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ModelPrefs {
    #[serde(default)]
    pub recent: Vec<ModelRef>,
    #[serde(default)]
    pub favorites: Vec<ModelRef>,
}

pub fn model_prefs_path() -> PathBuf {
    crate::config::loader::config_dir().join("model_prefs.json")
}

pub fn load_model_prefs() -> ModelPrefs {
    std::fs::read_to_string(model_prefs_path())
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub fn save_model_prefs(prefs: &ModelPrefs) -> anyhow::Result<()> {
    let path = model_prefs_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(prefs)?)?;
    Ok(())
}

pub fn record_recent_model(provider: &str, model: &str) {
    let provider = provider.trim();
    let model = model.trim();
    if provider.is_empty() || model.is_empty() {
        return;
    }
    let mut prefs = load_model_prefs();
    prefs
        .recent
        .retain(|entry| !(entry.provider == provider && entry.model == model));
    prefs.recent.insert(
        0,
        ModelRef {
            provider: provider.to_string(),
            model: model.to_string(),
        },
    );
    prefs.recent.truncate(12);
    let _ = save_model_prefs(&prefs);
}

pub fn toggle_favorite_model(provider: &str, model: &str) -> ModelPrefs {
    let provider = provider.trim();
    let model = model.trim();
    let mut prefs = load_model_prefs();
    if provider.is_empty() || model.is_empty() {
        return prefs;
    }
    if let Some(idx) = prefs
        .favorites
        .iter()
        .position(|entry| entry.provider == provider && entry.model == model)
    {
        prefs.favorites.remove(idx);
    } else {
        prefs.favorites.insert(
            0,
            ModelRef {
                provider: provider.to_string(),
                model: model.to_string(),
            },
        );
        prefs.favorites.truncate(24);
    }
    let _ = save_model_prefs(&prefs);
    prefs
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestEnvLock;

    impl TestEnvLock {
        fn acquire() -> Self {
            let lock_path = std::env::temp_dir().join("openz_test_config_dir.lock");
            loop {
                match std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&lock_path)
                {
                    Ok(_) => break,
                    Err(_) => std::thread::sleep(std::time::Duration::from_millis(50)),
                }
            }
            TestEnvLock
        }
    }

    impl Drop for TestEnvLock {
        fn drop(&mut self) {
            let lock_path = std::env::temp_dir().join("openz_test_config_dir.lock");
            let _ = std::fs::remove_file(lock_path);
        }
    }

    #[test]
    fn test_record_recent_model() {
        let _env_lock = TestEnvLock::acquire();
        let prev_dir = std::env::var("OPENZ_CONFIG_DIR").ok();
        let temp_dir = std::env::temp_dir().join(format!("openz_test_recent_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::env::set_var("OPENZ_CONFIG_DIR", &temp_dir);

        // Recording empty should do nothing
        record_recent_model("", "gpt-4o");
        record_recent_model("openai", "   ");
        assert!(load_model_prefs().recent.is_empty());

        // Record a model
        record_recent_model("openai", "gpt-4o");
        let prefs = load_model_prefs();
        assert_eq!(prefs.recent.len(), 1);
        assert_eq!(prefs.recent[0].provider, "openai");
        assert_eq!(prefs.recent[0].model, "gpt-4o");

        // Record another model
        record_recent_model("anthropic", "claude-3-5-sonnet");
        let prefs = load_model_prefs();
        assert_eq!(prefs.recent.len(), 2);
        assert_eq!(prefs.recent[0].model, "claude-3-5-sonnet");
        assert_eq!(prefs.recent[1].model, "gpt-4o");

        // Re-recording moves to front without duplicating
        record_recent_model("openai", "gpt-4o");
        let prefs = load_model_prefs();
        assert_eq!(prefs.recent.len(), 2);
        assert_eq!(prefs.recent[0].model, "gpt-4o");
        assert_eq!(prefs.recent[1].model, "claude-3-5-sonnet");

        // Test truncation to 12
        for i in 0..20 {
            record_recent_model("provider", &format!("model-{i}"));
        }
        let prefs = load_model_prefs();
        assert_eq!(prefs.recent.len(), 12);
        assert_eq!(prefs.recent[0].model, "model-19");

        if let Some(prev) = prev_dir {
            std::env::set_var("OPENZ_CONFIG_DIR", prev);
        } else {
            std::env::remove_var("OPENZ_CONFIG_DIR");
        }
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_toggle_favorite_model() {
        let _env_lock = TestEnvLock::acquire();
        let prev_dir = std::env::var("OPENZ_CONFIG_DIR").ok();
        let temp_dir = std::env::temp_dir().join(format!("openz_test_fav_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::env::set_var("OPENZ_CONFIG_DIR", &temp_dir);

        // Empty strings ignored
        let prefs = toggle_favorite_model("  ", "gpt-4o");
        assert!(prefs.favorites.is_empty());

        // Toggle on
        let prefs = toggle_favorite_model("openai", "gpt-4o");
        assert_eq!(prefs.favorites.len(), 1);
        assert_eq!(prefs.favorites[0].provider, "openai");
        assert_eq!(prefs.favorites[0].model, "gpt-4o");

        // Toggle another
        let prefs = toggle_favorite_model("anthropic", "claude-3-5-sonnet");
        assert_eq!(prefs.favorites.len(), 2);
        assert_eq!(prefs.favorites[0].model, "claude-3-5-sonnet");
        assert_eq!(prefs.favorites[1].model, "gpt-4o");

        // Toggle off first
        let prefs = toggle_favorite_model("openai", "gpt-4o");
        assert_eq!(prefs.favorites.len(), 1);
        assert_eq!(prefs.favorites[0].model, "claude-3-5-sonnet");

        // Toggle off remaining
        let prefs = toggle_favorite_model("anthropic", "claude-3-5-sonnet");
        assert!(prefs.favorites.is_empty());

        // Test truncation to 24
        for i in 0..30 {
            toggle_favorite_model("provider", &format!("model-{i}"));
        }
        let prefs = load_model_prefs();
        assert_eq!(prefs.favorites.len(), 24);

        if let Some(prev) = prev_dir {
            std::env::set_var("OPENZ_CONFIG_DIR", prev);
        } else {
            std::env::remove_var("OPENZ_CONFIG_DIR");
        }
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
