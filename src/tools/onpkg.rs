use crate::tools::Tool;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use tokio::process::Command;

pub struct OnpkgTool;

impl OnpkgTool {
    fn resolve_binary() -> String {
        if let Some(home) = dirs::home_dir() {
            let p = home.join(".cargo").join("bin").join("onpkg");
            if p.exists() {
                return p.to_string_lossy().to_string();
            }
        }
        "onpkg".to_string()
    }
}

#[async_trait::async_trait]
impl Tool for OnpkgTool {
    fn name(&self) -> &str {
        "onpkg"
    }

    fn description(&self) -> &str {
        "Use onpkg to list available templates/stacks, show details of a stack, scaffold a stack (website, app, backend, frontend from scratch), run environment diagnostics, or register/add new stacks, templates, skills, and packages for future use."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": [
                        "list_stacks",
                        "show_stack",
                        "scaffold",
                        "doctor",
                        "add_template",
                        "add_skill",
                        "add_package",
                        "install_package"
                    ],
                    "description": "The onpkg action to perform."
                },
                "stack_name": {
                    "type": "string",
                    "description": "The name of the template/stack to scaffold or show details for (required for 'scaffold' and 'show_stack')."
                },
                "dir": {
                    "type": "string",
                    "description": "The target directory to scaffold the stack into (optional for 'scaffold', defaults to current directory)."
                },
                "name": {
                    "type": "string",
                    "description": "The name for the template, skill, or package to add/install (required for 'add_template', 'add_skill', 'add_package', 'install_package')."
                },
                "source": {
                    "type": "string",
                    "description": "The path to the local directory/file or the remote git URL to add (required for 'add_template', 'add_skill')."
                },
                "runtime": {
                    "type": "string",
                    "enum": ["npm", "pypi", "pub", "cargo"],
                    "description": "The runtime category for packages (optional for package actions)."
                }
            },
            "required": ["action"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let action = arguments
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'action' parameter"))?;

        let onpkg_bin = Self::resolve_binary();
        let mut cmd = Command::new(&onpkg_bin);
        crate::config::loader::set_tokio_command_cwd(&mut cmd);

        match action {
            "list_stacks" => {
                cmd.args(["stack", "list"]);
            }
            "show_stack" => {
                let stack = arguments
                    .get("stack_name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        anyhow!("Missing 'stack_name' parameter for show_stack action")
                    })?;
                cmd.args(["stack", "show", stack]);
            }
            "scaffold" => {
                let stack = arguments
                    .get("stack_name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'stack_name' parameter for scaffold action"))?;
                cmd.args(["stack", "add", stack]);
                if let Some(dir) = arguments.get("dir").and_then(|v| v.as_str()) {
                    let resolved = crate::config::resolve_path(dir);
                    cmd.arg("--dir");
                    cmd.arg(resolved.to_string_lossy().to_string());
                }
            }
            "doctor" => {
                cmd.arg("doctor");
            }
            "add_template" => {
                let name = arguments
                    .get("name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'name' parameter for add_template action"))?;
                let source = arguments
                    .get("source")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'source' parameter for add_template action"))?;
                cmd.args(["template", "add", name, source]);
            }
            "add_skill" => {
                let name = arguments
                    .get("name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'name' parameter for add_skill action"))?;
                let source = arguments
                    .get("source")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'source' parameter for add_skill action"))?;
                cmd.args(["skill", "add", name, source]);
            }
            "add_package" => {
                let name = arguments
                    .get("name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'name' parameter for add_package action"))?;
                cmd.args(["pkg", "add", name]);
                if let Some(rt) = arguments.get("runtime").and_then(|v| v.as_str()) {
                    cmd.arg("--runtime");
                    cmd.arg(rt);
                }
            }
            "install_package" => {
                let name = arguments
                    .get("name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        anyhow!("Missing 'name' parameter for install_package action")
                    })?;
                cmd.args(["pkg", "install", name]);
                if let Some(rt) = arguments.get("runtime").and_then(|v| v.as_str()) {
                    cmd.arg("--runtime");
                    cmd.arg(rt);
                }
            }
            _ => return Err(anyhow!("Unsupported onpkg action: {}", action)),
        }

        let output = cmd.output().await?;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        Ok(json!({
            "status": if output.status.success() { "success" } else { "error" },
            "stdout": stdout,
            "stderr": stderr,
            "code": output.status.code()
        }))
    }
}

pub fn sync_onpkg_manifest() -> Result<()> {
    let onpkg_json_path = std::path::Path::new("onpkg.json");
    if !onpkg_json_path.exists() {
        return Ok(());
    }

    // 1. Read onpkg.json
    let content = std::fs::read_to_string(onpkg_json_path)?;
    let mut manifest: Value =
        serde_json::from_str(&content).map_err(|e| anyhow!("Failed to parse onpkg.json: {}", e))?;

    // Ensure agent_instructions and docs_directory exist
    if manifest.get("agent_instructions").is_none() {
        manifest["agent_instructions"] = json!({
            "active_skills": [],
            "docs_directory": "onpkg_docs/"
        });
    }
    let docs_dir_name = manifest["agent_instructions"]
        .get("docs_directory")
        .and_then(|v| v.as_str())
        .unwrap_or("onpkg_docs/")
        .to_string();

    let docs_dir = std::path::PathBuf::from(docs_dir_name);
    if !docs_dir.exists() {
        std::fs::create_dir_all(&docs_dir)?;
    }

    // 2. Create the 5 core workflow files if they don't exist
    let workflow_files = vec![
        ("prd.md", prd_template()),
        ("content.md", content_template()),
        ("design.md", design_template()),
        ("implementation.md", implementation_template()),
        ("todo.md", todo_template()),
    ];

    for (filename, template) in &workflow_files {
        let file_path = docs_dir.join(filename);
        if !file_path.exists() {
            std::fs::write(&file_path, template)?;
        }
    }

    // 3. Scan docs_dir for all .md files (except INDEX.md) to update active_skills
    let mut active_skills = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&docs_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                    if ext == "md" {
                        if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                            if name != "INDEX.md" {
                                active_skills.push(name.to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    active_skills.sort();

    // Update active_skills in manifest
    manifest["agent_instructions"]["active_skills"] = json!(active_skills);

    // 4. Update INDEX.md dynamically
    let index_path = docs_dir.join("INDEX.md");
    let mut index_content = String::from("# Project AI Agent Skills 🧠\n\nThis directory contains instructions and guidelines for AI agents working on this project.\n\n## Available Skills\n");
    for skill in &active_skills {
        let name_without_ext = skill.strip_suffix(".md").unwrap_or(skill);
        index_content.push_str(&format!(
            "- [{name}](file://./{skill})\n",
            name = name_without_ext,
            skill = skill
        ));
    }
    std::fs::write(index_path, index_content)?;

    // 5. Scan project structure to find Architecture elements
    if manifest.get("architecture").is_none() {
        manifest["architecture"] = json!({});
    }

    // Components
    let comp_paths = vec!["src/components", "components"];
    for p in comp_paths {
        if std::path::Path::new(p).exists() {
            manifest["architecture"]["components"] = json!(p);
            break;
        }
    }

    // Routing
    let route_paths = vec!["src/pages", "pages", "src/routes", "routes"];
    for p in route_paths {
        if std::path::Path::new(p).exists() {
            manifest["architecture"]["routing"] = json!(p);
            break;
        }
    }

    // Entrypoint
    let entry_paths = vec![
        "src/main.rs",
        "src/lib.rs",
        "src/main.tsx",
        "src/index.js",
        "src/index.ts",
        "index.html",
        "src/main.py",
        "main.py",
    ];
    for p in entry_paths {
        if std::path::Path::new(p).exists() {
            manifest["architecture"]["entrypoint"] = json!(p);
            break;
        }
    }

    // Styles
    let style_paths = vec!["src/index.css", "index.css", "src/app.css", "app.css"];
    for p in style_paths {
        if std::path::Path::new(p).exists() {
            manifest["architecture"]["styles"] = json!(p);
            break;
        }
    }

    // 6. Detect packages
    if manifest.get("packages").is_none() {
        manifest["packages"] = json!({
            "added_by_agent": [],
            "core": []
        });
    }

    let mut core_packages = std::collections::HashSet::new();

    // Parse Cargo.toml
    if std::path::Path::new("Cargo.toml").exists() {
        if let Ok(toml_str) = std::fs::read_to_string("Cargo.toml") {
            let mut in_deps = false;
            for line in toml_str.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with('[') {
                    in_deps = trimmed == "[dependencies]" || trimmed == "[workspace.dependencies]";
                    if let Some(pkg) = trimmed
                        .strip_prefix("[dependencies.")
                        .and_then(|s| s.strip_suffix(']'))
                    {
                        core_packages.insert(pkg.trim().to_string());
                    }
                    continue;
                }
                if in_deps && !trimmed.is_empty() && !trimmed.starts_with('#') {
                    if let Some(pkg_name) = trimmed.split('=').next().map(|s| s.trim()) {
                        core_packages.insert(pkg_name.to_string());
                    }
                }
            }
        }
    }

    // Parse package.json
    if std::path::Path::new("package.json").exists() {
        if let Ok(pkg_json_str) = std::fs::read_to_string("package.json") {
            if let Ok(pkg_json) = serde_json::from_str::<Value>(&pkg_json_str) {
                if let Some(deps) = pkg_json.get("dependencies").and_then(|d| d.as_object()) {
                    for key in deps.keys() {
                        core_packages.insert(key.to_string());
                    }
                }
                if let Some(dev_deps) = pkg_json.get("devDependencies").and_then(|d| d.as_object())
                {
                    for key in dev_deps.keys() {
                        core_packages.insert(key.to_string());
                    }
                }
            }
        }
    }

    let mut core_vec: Vec<String> = core_packages.into_iter().collect();
    core_vec.sort();

    // Preserve existing added_by_agent packages, but update core packages
    let mut added_by_agent = Vec::new();
    if let Some(arr) = manifest["packages"]
        .get("added_by_agent")
        .and_then(|a| a.as_array())
    {
        for v in arr {
            if let Some(s) = v.as_str() {
                added_by_agent.push(s.to_string());
            }
        }
    }
    added_by_agent.sort();

    manifest["packages"]["core"] = json!(core_vec);
    manifest["packages"]["added_by_agent"] = json!(added_by_agent);

    // 7. Write back if modified
    let new_content = serde_json::to_string_pretty(&manifest)?;
    if new_content != content {
        std::fs::write(onpkg_json_path, new_content)?;
    }

    Ok(())
}

fn prd_template() -> String {
    r#"---
name: prd
description: "Product Requirements Document (PRD) — defines user stories, personas, target features, and success criteria for the project."
---

# Product Requirements Document (PRD) 📝

## 1. Executive Summary
- **Overview**: 
- **Target Audience**: 

## 2. Core Features & Requirements
- [ ] Feature 1:
- [ ] Feature 2:

## 3. Out of Scope
- Items not included in this release.

## 4. Success Metrics
- How to measure success.
"#.to_string()
}

fn content_template() -> String {
    r#"---
name: content
description: "Copywriting & Media Content — tracks text copy, assets, and messaging styles for the application."
---

# Product Copy & Media Content 📄

## 1. Interface Text & Copy
- **Headings**: 
- **Descriptions**: 
- **Buttons / Actions**: 

## 2. Media Assets
- **Images**: 
- **Icons**: 
"#.to_string()
}

fn design_template() -> String {
    r#"---
name: design
description: "UI & Design System Specifications — outlines color tokens, typography, layouts, animations, and responsive breakpoints."
---

# UI & Design System Specifications 🎨

## 1. Aesthetics
- **Theme**: Sleek dark mode / HSL tailored colors / Glassmorphism
- **Typography**: Fonts, sizes, and hierarchies
- **Colors**: Hex/HSL color codes

## 2. Layout & Responsive Breakpoints
- **Mobile**: 
- **Desktop**: 

## 3. Animations & Transitions
- Micro-animations and hover effects.
"#.to_string()
}

fn implementation_template() -> String {
    r#"---
name: implementation
description: "Technical Implementation Plan — details system architecture, database schema, data flow, API routing, and code-outline analyses."
---

# Technical Implementation Plan 🛠️

## 1. System Architecture
- **Tech Stack**: 
- **Entrypoint**: 

## 2. Data Flow & State Management
- Database schemas, API routes, and state stores.

## 3. Structural Analysis (Tree-sitter / ast-grep)
- Key modules and design patterns.
"#.to_string()
}

fn todo_template() -> String {
    r#"---
name: todo
description: "Project Task Tracker — tracks todo items, in-progress tasks, and completed milestones."
---

# Project Task Tracker 📋

## Todo
- [ ] Initialize project structure
- [ ] Define schemas & state
- [ ] Build core components
- [ ] Verify implementation

## In Progress
- None

## Done
- [x] Create project manifest
"#.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_onpkg_tool() -> Result<()> {
        let tool = OnpkgTool;
        let res = tool
            .call(&json!({
                "action": "doctor"
            }))
            .await?;

        assert_eq!(res["status"], "success");
        assert!(res["stdout"].as_str().unwrap().contains("Doctor complete"));

        Ok(())
    }

    #[test]
    fn test_sync_onpkg_manifest() -> Result<()> {
        let res = sync_onpkg_manifest();
        assert!(res.is_ok());
        Ok(())
    }
}
