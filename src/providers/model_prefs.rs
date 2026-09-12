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
#[path = "model_prefs_tests.rs"]
mod tests;

