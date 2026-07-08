pub mod agent;
pub mod channels;
pub mod cli;
pub mod config;
pub mod cron;
pub mod logs;
pub mod providers;
pub mod session;
pub mod shutdown;
pub mod sop;
pub mod subagents;
pub mod tools;

#[cfg(test)]
mod version_sync_tests {
    fn workspace_file(path: &str) -> String {
        std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path))
            .unwrap_or_else(|err| panic!("failed to read {path}: {err}"))
    }

    #[test]
    fn release_version_surfaces_match_cargo_package_version() {
        let version = env!("CARGO_PKG_VERSION");
        let readme = workspace_file("README.md");
        let onpkg = workspace_file("onpkg.json");
        let changelog = workspace_file("CHANGELOG.md");
        let cli_changelog = workspace_file("src/cli/changelog.rs");
        let onpkg_json: serde_json::Value = serde_json::from_str(&onpkg).expect("valid onpkg.json");

        assert!(
            readme.starts_with(&format!("# OpenZ 🦊 `v{version}`")),
            "README.md header must match Cargo package version {version}"
        );
        assert_eq!(
            onpkg_json["project"]["version"].as_str(),
            Some(version),
            "onpkg.json project.version must match Cargo package version"
        );
        assert!(
            changelog.contains(&format!("### v{version} (Latest Release)")),
            "CHANGELOG.md latest release heading must match Cargo package version {version}"
        );
        assert!(
            cli_changelog.contains("env!(\"CARGO_PKG_VERSION\")"),
            "CLI changelog current release must derive from CARGO_PKG_VERSION instead of a hardcoded version"
        );
        assert!(
            cli_changelog.contains("Current Release"),
            "CLI changelog must still render a current release label"
        );
    }
}
