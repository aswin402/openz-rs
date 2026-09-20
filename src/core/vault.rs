use crate::config::schema::Config;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Environment variable that overrides the vault directory path.
pub const OPENZ_VAULT_DIR_ENV: &str = "OPENZ_VAULT_DIR";

/// The 10 canonical categories for user deliverables in OpenZ Vault.
pub const VAULT_CATEGORIES: &[&str] = &[
    "websites",
    "scripts",
    "images",
    "videos",
    "documents",
    "presentations",
    "data",
    "diagrams",
    "templates",
    "exports",
];

/// Returns a human-friendly description of what belongs in a vault category.
pub fn category_description(cat: &str) -> &'static str {
    match cat {
        "websites" => "Web applications, landing pages, dashboards, and HTML/CSS/JS prototypes",
        "scripts" => "Automations, web scrapers, data pipelines, and CLI utilities",
        "images" => "Visual assets, logos, mockups, and AI-rendered images",
        "videos" => "Programmatic MP4 videos, animation timelines, and explainer media",
        "documents" => "Reports, technical whitepapers, PDF and DOCX documents",
        "presentations" => "Slide decks, pitch presentations, and keynote decks",
        "data" => "Extracted datasets, CSV tables, scraped JSON, and crawl outputs",
        "diagrams" => "Architecture diagrams, Mermaid charts, and SVG flowcharts",
        "templates" => "Reusable prompt templates, project boilerplates, and starter kits",
        "exports" => "Readable chat summaries, knowledge graph exports, and skill backups",
        _ => "User deliverables and workspace artifacts",
    }
}

/// Expands leading `~` or returns the path as is.
pub fn expand_vault_path(raw_path: &str) -> PathBuf {
    let trimmed = raw_path.trim();
    if trimmed.is_empty() {
        return dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("openz_vault");
    }

    if trimmed == "~" {
        return dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    }

    if let Some(stripped) = trimmed.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(stripped);
        }
    }

    PathBuf::from(trimmed)
}

/// Resolves the active root directory for OpenZ Vault.
///
/// Priority:
/// 1. `OPENZ_VAULT_DIR` environment variable
/// 2. `config.vault.path`
/// 3. Default: `~/openz_vault`
pub fn resolve_vault_path(config: &Config) -> PathBuf {
    if let Ok(env_val) = std::env::var(OPENZ_VAULT_DIR_ENV) {
        let trimmed = env_val.trim();
        if !trimmed.is_empty() {
            return expand_vault_path(trimmed);
        }
    }

    let config_path = config.vault.path.trim();
    if !config_path.is_empty() {
        expand_vault_path(config_path)
    } else {
        expand_vault_path("~/openz_vault")
    }
}

/// Resolves the absolute path for a specific category within the vault.
pub fn resolve_category_path(config: &Config, category: &str) -> PathBuf {
    let vault_root = resolve_vault_path(config);
    let cat_clean = category.trim().trim_matches('/');
    vault_root.join(cat_clean)
}

/// Resolves the destination path for an item inside a vault category.
pub fn resolve_vault_item_path(config: &Config, category: &str, item_name: &str) -> PathBuf {
    let cat_dir = resolve_category_path(config, category);
    let clean_item = item_name.trim().trim_start_matches('/');
    cat_dir.join(clean_item)
}

/// Generates the standard README.md content placed in the root of the vault.
pub fn generate_vault_readme(vault_name: &str) -> String {
    format!(
        "# {vault_name} 🦊\n\n\
Welcome to your **OpenZ Vault**! This directory is your dedicated personal workspace where OpenZ saves all user-facing deliverables, creative projects, generated media, and exported artifacts.\n\n\
## Directory Structure\n\n\
| Category | Purpose |\n\
|---|---|\n\
| `websites/` | Complete standalone websites, landing pages, and interactive web apps |\n\
| `scripts/` | Python, Bash, Node.js scripts, scrapers, and data pipelines |\n\
| `images/` | High-fidelity renders, logos, UI mockups, and visual assets |\n\
| `videos/` | Programmatic MP4 animations, timeline videos, and explainer media |\n\
| `documents/` | Research reports, whitepapers, PDF and DOCX specifications |\n\
| `presentations/` | Slide decks, pitch decks, and PowerPoint presentations |\n\
| `data/` | Scraped datasets, CSV tables, JSON files, and web crawl dumps |\n\
| `diagrams/` | Architecture charts, Mermaid sequence diagrams, and SVG maps |\n\
| `templates/` | Reusable prompt templates, SOP workflows, and project starter kits |\n\
| `exports/` | Session markdown summaries, knowledge graph exports, and skill backups |\n\n\
---\n\
*Note: OpenZ internal system state (configuration, API keys, SQLite vector/graph databases, and raw session logs) lives safely in `~/.openz/` and never clutters this vault.*\n"
    )
}

/// Ensures that the OpenZ Vault and its standard category subdirectories exist on disk.
/// Also writes a welcoming and informative `README.md` if not already present.
pub fn ensure_vault_initialized(config: &Config) -> Result<PathBuf> {
    if !config.vault.enabled {
        return Ok(resolve_vault_path(config));
    }

    let vault_dir = resolve_vault_path(config);
    if !vault_dir.exists() {
        fs::create_dir_all(&vault_dir)
            .with_context(|| format!("Failed to create OpenZ Vault at {}", vault_dir.display()))?;
    }

    if config.vault.auto_create {
        for category in VAULT_CATEGORIES {
            let cat_dir = vault_dir.join(category);
            if !cat_dir.exists() {
                let _ = fs::create_dir_all(&cat_dir);
            }
        }

        let readme_path = vault_dir.join("README.md");
        if !readme_path.exists() {
            let readme_content = generate_vault_readme(&config.vault.name);
            let _ = fs::write(&readme_path, readme_content);
        }
    }

    Ok(vault_dir)
}

/// Per-category breakdown of items in the vault.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultCategorySummary {
    pub name: String,
    pub description: String,
    pub path: String,
    pub item_count: usize,
}

/// Summary report of the OpenZ Vault status and contents.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultSummary {
    pub enabled: bool,
    pub name: String,
    pub root_path: String,
    pub exists: bool,
    pub writable: bool,
    pub total_items: usize,
    pub categories: Vec<VaultCategorySummary>,
}

/// Scans the vault directory and returns a structured summary of deliverable counts per category.
pub fn scan_vault(config: &Config) -> VaultSummary {
    let root_path = resolve_vault_path(config);
    let exists = root_path.exists();
    let writable = if exists {
        let test_file = root_path.join(".openz_write_probe");
        let write_ok = fs::write(&test_file, b"ok").is_ok();
        if write_ok {
            let _ = fs::remove_file(&test_file);
        }
        write_ok
    } else {
        false
    };

    let mut total_items = 0;
    let mut categories = Vec::new();

    for &category in VAULT_CATEGORIES {
        let cat_path = root_path.join(category);
        let count = if cat_path.exists() {
            fs::read_dir(&cat_path)
                .map(|entries| {
                    entries
                        .flatten()
                        .filter(|e| {
                            let name = e.file_name();
                            let s = name.to_string_lossy();
                            !s.starts_with('.')
                        })
                        .count()
                })
                .unwrap_or(0)
        } else {
            0
        };

        total_items += count;
        categories.push(VaultCategorySummary {
            name: category.to_string(),
            description: category_description(category).to_string(),
            path: cat_path.display().to_string(),
            item_count: count,
        });
    }

    VaultSummary {
        enabled: config.vault.enabled,
        name: config.vault.name.clone(),
        root_path: root_path.display().to_string(),
        exists,
        writable,
        total_items,
        categories,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_vault_path_tilde() {
        let expanded = expand_vault_path("~/openz_vault");
        assert!(!expanded.to_string_lossy().starts_with("~"));
        assert!(expanded.to_string_lossy().contains("openz_vault"));
    }

    #[test]
    fn test_vault_categories_count() {
        assert_eq!(VAULT_CATEGORIES.len(), 10);
        assert!(VAULT_CATEGORIES.contains(&"websites"));
        assert!(VAULT_CATEGORIES.contains(&"scripts"));
        assert!(VAULT_CATEGORIES.contains(&"images"));
        assert!(VAULT_CATEGORIES.contains(&"videos"));
        assert!(VAULT_CATEGORIES.contains(&"documents"));
        assert!(VAULT_CATEGORIES.contains(&"presentations"));
        assert!(VAULT_CATEGORIES.contains(&"data"));
        assert!(VAULT_CATEGORIES.contains(&"diagrams"));
        assert!(VAULT_CATEGORIES.contains(&"templates"));
        assert!(VAULT_CATEGORIES.contains(&"exports"));
    }

    #[test]
    fn test_ensure_vault_initialized_creates_subdirs_and_readme() {
        let temp_dir = std::env::temp_dir().join(format!("openz_vault_test_{}", uuid::Uuid::new_v4()));
        let mut config = Config::default();
        config.vault.path = temp_dir.display().to_string();

        let initialized_path = ensure_vault_initialized(&config).unwrap();
        assert_eq!(initialized_path, temp_dir);
        assert!(temp_dir.exists());

        // Check category directories
        for category in VAULT_CATEGORIES {
            assert!(temp_dir.join(category).exists(), "Missing category dir: {category}");
        }

        // Check README
        assert!(temp_dir.join("README.md").exists());

        // Create a dummy item in websites and scripts
        let _ = fs::write(temp_dir.join("websites").join("index.html"), "<html></html>");
        let _ = fs::write(temp_dir.join("scripts").join("run.sh"), "#!/bin/bash");

        // Scan vault
        let summary = scan_vault(&config);
        assert!(summary.exists);
        assert!(summary.writable);
        assert_eq!(summary.total_items, 2);

        let websites_cat = summary.categories.iter().find(|c| c.name == "websites").unwrap();
        assert_eq!(websites_cat.item_count, 1);

        let scripts_cat = summary.categories.iter().find(|c| c.name == "scripts").unwrap();
        assert_eq!(scripts_cat.item_count, 1);

        let _ = fs::remove_dir_all(temp_dir);
    }
}
