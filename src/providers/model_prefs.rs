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
    model_prefs_path_at(&crate::config::loader::config_dir())
}

pub fn model_prefs_path_at(dir: &std::path::Path) -> PathBuf {
    dir.join("model_prefs.json")
}

pub fn load_model_prefs() -> ModelPrefs {
    load_model_prefs_at(&crate::config::loader::config_dir())
}

pub fn load_model_prefs_at(dir: &std::path::Path) -> ModelPrefs {
    std::fs::read_to_string(model_prefs_path_at(dir))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub fn save_model_prefs(prefs: &ModelPrefs) -> anyhow::Result<()> {
    save_model_prefs_at(&crate::config::loader::config_dir(), prefs)
}

pub fn save_model_prefs_at(dir: &std::path::Path, prefs: &ModelPrefs) -> anyhow::Result<()> {
    let path = model_prefs_path_at(dir);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temp_file = path.with_extension(format!("tmp.{}", uuid::Uuid::new_v4()));
    let write_res = (|| -> anyhow::Result<()> {
        let mut file = std::fs::File::create(&temp_file)?;
        let json = serde_json::to_string_pretty(prefs)?;
        use std::io::Write;
        file.write_all(json.as_bytes())?;
        file.sync_all()?;
        Ok(())
    })();

    if let Err(e) = write_res {
        let _ = std::fs::remove_file(&temp_file);
        return Err(e);
    }

    if let Err(e) = std::fs::rename(&temp_file, &path) {
        let _ = std::fs::remove_file(&temp_file);
        return Err(e.into());
    }

    Ok(())
}

pub fn record_recent_model(provider: &str, model: &str) {
    record_recent_model_at(&crate::config::loader::config_dir(), provider, model);
}

pub fn record_recent_model_at(dir: &std::path::Path, provider: &str, model: &str) {
    let provider = provider.trim();
    let model = model.trim();
    if provider.is_empty() || model.is_empty() {
        return;
    }
    let mut prefs = load_model_prefs_at(dir);
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
    if let Err(err) = save_model_prefs_at(dir, &prefs) {
        tracing::warn!(error = ?err, "failed to save recent model prefs");
    }
}

pub fn toggle_favorite_model(provider: &str, model: &str) -> ModelPrefs {
    toggle_favorite_model_at(&crate::config::loader::config_dir(), provider, model)
}

pub fn toggle_favorite_model_at(dir: &std::path::Path, provider: &str, model: &str) -> ModelPrefs {
    let provider = provider.trim();
    let model = model.trim();
    let mut prefs = load_model_prefs_at(dir);
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
    if let Err(err) = save_model_prefs_at(dir, &prefs) {
        tracing::warn!(error = ?err, "failed to save favorite model prefs");
    }
    prefs
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDirGuard(std::path::PathBuf);

    impl TempDirGuard {
        fn new(prefix: &str) -> Self {
            let path = std::env::temp_dir().join(format!("{prefix}_{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&path).expect("failed to create temp dir");
            TempDirGuard(path)
        }

        fn path(&self) -> &std::path::Path {
            &self.0
        }
    }

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn test_record_recent_model() {
        let guard = TempDirGuard::new("openz_test_recent");
        let temp_dir = guard.path();

        // Recording empty should do nothing
        record_recent_model_at(temp_dir, "", "gpt-4o");
        record_recent_model_at(temp_dir, "openai", "   ");
        assert!(load_model_prefs_at(temp_dir).recent.is_empty());

        // Record a model
        record_recent_model_at(temp_dir, "openai", "gpt-4o");
        let prefs = load_model_prefs_at(temp_dir);
        assert_eq!(prefs.recent.len(), 1);
        assert_eq!(prefs.recent[0].provider, "openai");
        assert_eq!(prefs.recent[0].model, "gpt-4o");

        // Record another model
        record_recent_model_at(temp_dir, "anthropic", "claude-3-5-sonnet");
        let prefs = load_model_prefs_at(temp_dir);
        assert_eq!(prefs.recent.len(), 2);
        assert_eq!(prefs.recent[0].model, "claude-3-5-sonnet");
        assert_eq!(prefs.recent[1].model, "gpt-4o");

        // Re-recording moves to front without duplicating
        record_recent_model_at(temp_dir, "openai", "gpt-4o");
        let prefs = load_model_prefs_at(temp_dir);
        assert_eq!(prefs.recent.len(), 2);
        assert_eq!(prefs.recent[0].model, "gpt-4o");
        assert_eq!(prefs.recent[1].model, "claude-3-5-sonnet");

        // Test truncation to 12
        for i in 0..20 {
            record_recent_model_at(temp_dir, "provider", &format!("model-{i}"));
        }
        let prefs = load_model_prefs_at(temp_dir);
        assert_eq!(prefs.recent.len(), 12);
        assert_eq!(prefs.recent[0].model, "model-19");
    }

    #[test]
    fn test_toggle_favorite_model() {
        let guard = TempDirGuard::new("openz_test_fav");
        let temp_dir = guard.path();

        // Empty strings ignored
        let prefs = toggle_favorite_model_at(temp_dir, "  ", "gpt-4o");
        assert!(prefs.favorites.is_empty());

        // Toggle on
        let prefs = toggle_favorite_model_at(temp_dir, "openai", "gpt-4o");
        assert_eq!(prefs.favorites.len(), 1);
        assert_eq!(prefs.favorites[0].provider, "openai");
        assert_eq!(prefs.favorites[0].model, "gpt-4o");

        // Toggle another
        let prefs = toggle_favorite_model_at(temp_dir, "anthropic", "claude-3-5-sonnet");
        assert_eq!(prefs.favorites.len(), 2);
        assert_eq!(prefs.favorites[0].model, "claude-3-5-sonnet");
        assert_eq!(prefs.favorites[1].model, "gpt-4o");

        // Toggle off first
        let prefs = toggle_favorite_model_at(temp_dir, "openai", "gpt-4o");
        assert_eq!(prefs.favorites.len(), 1);
        assert_eq!(prefs.favorites[0].model, "claude-3-5-sonnet");

        // Toggle off remaining
        let prefs = toggle_favorite_model_at(temp_dir, "anthropic", "claude-3-5-sonnet");
        assert!(prefs.favorites.is_empty());

        // Test truncation to 24
        for i in 0..30 {
            toggle_favorite_model_at(temp_dir, "provider", &format!("model-{i}"));
        }
        let prefs = load_model_prefs_at(temp_dir);
        assert_eq!(prefs.favorites.len(), 24);
    }

    #[test]
    fn test_save_model_prefs_cleans_up_on_failure() {
        let guard = TempDirGuard::new("model-prefs-err-test");
        // Create an un-writable path to provoke an error or test invalid target
        let file_as_dir = guard.path().join("file_blocking_dir");
        std::fs::write(&file_as_dir, "blocking").unwrap();
        // Trying to save into a path where directory creation fails
        let invalid_dir = file_as_dir.join("sub");
        let res = save_model_prefs_at(&invalid_dir, &ModelPrefs::default());
        assert!(res.is_err());
        // Verify no leftover .tmp files were created in parent
        let entries = std::fs::read_dir(guard.path()).unwrap();
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            assert!(!name.contains(".tmp."), "leftover temp file found: {name}");
        }

        // Test failure during rename (target path is a directory, blocking rename)
        let rename_dir = guard.path().join("rename_dir");
        std::fs::create_dir_all(&rename_dir).unwrap();
        let blocking_target = rename_dir.join("model_prefs.json");
        std::fs::create_dir_all(&blocking_target).unwrap();
        let res_rename = save_model_prefs_at(&rename_dir, &ModelPrefs::default());
        assert!(res_rename.is_err());
        let rename_entries = std::fs::read_dir(&rename_dir).unwrap();
        for entry in rename_entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            assert!(!name.contains(".tmp."), "leftover temp file found: {name}");
        }
    }
}
