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
#[cfg(test)]
pub(crate) use diagnostics::normalize_diagnose_mock_args;
pub(crate) use config::ManageConfigTool;
#[cfg(test)]
pub(crate) use config::redact_secrets;
pub use backups::ManageBackupsTool;
pub use sessions::ManageSessionsTool;

#[cfg(test)]
mod tests;
