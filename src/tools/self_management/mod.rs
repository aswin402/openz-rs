//! Configuration, diagnostics, discovery, and maintenance tools for OpenZ.

mod backups;
mod catalog;
mod config;
mod diagnostics;
mod inventory;
mod scope;
mod sessions;
mod skills;

pub(crate) use skills::CurateSkillTool;
pub use inventory::OpenZInventoryTool;
pub use scope::{OptimizeToolScopeTool, RequestToolScopeTool};
pub use catalog::ToolCatalogTool;
pub use diagnostics::{DiagnoseSystemTool, DiagnoseToolTool};
pub(crate) use config::ManageConfigTool;
pub use backups::ManageBackupsTool;
pub use sessions::ManageSessionsTool;

pub(crate) const ACTION_KEYS: &[&str] = &["action", "act", "command", "cmd", "op", "mode"];

pub(crate) use crate::tools::arguments::{
    extract_first_str, extract_trimmed_str, parse_bool_value, parse_f64_value, parse_u64_value,
    to_non_empty_trimmed,
};

#[cfg(test)]
pub(crate) struct TestEnvLock {
    previous_config_dir: Option<String>,
    temp_dir: std::path::PathBuf,
}

#[cfg(test)]
impl TestEnvLock {
    pub(crate) fn acquire(prefix: &str) -> Self {
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
        let previous_config_dir = std::env::var("OPENZ_CONFIG_DIR").ok();
        let temp_dir = std::env::temp_dir().join(format!("{}_{}", prefix, uuid::Uuid::new_v4()));
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::env::set_var("OPENZ_CONFIG_DIR", &temp_dir);
        Self {
            previous_config_dir,
            temp_dir,
        }
    }

    pub(crate) fn dir(&self) -> &std::path::Path {
        &self.temp_dir
    }
}

#[cfg(test)]
impl Drop for TestEnvLock {
    fn drop(&mut self) {
        match self.previous_config_dir.as_ref() {
            Some(prev) => std::env::set_var("OPENZ_CONFIG_DIR", prev),
            None => std::env::remove_var("OPENZ_CONFIG_DIR"),
        }
        let _ = std::fs::remove_dir_all(&self.temp_dir);
        let lock_path = std::env::temp_dir().join("openz_test_config_dir.lock");
        let _ = std::fs::remove_file(lock_path);
    }
}
