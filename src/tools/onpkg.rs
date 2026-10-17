use crate::tools::Tool;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use tokio::process::Command;

pub struct OnpkgTool;

impl OnpkgTool {
    fn resolve_binary() -> String {
        dirs::home_dir()
            .map(|home| home.join(".cargo").join("bin").join("onpkg"))
            .filter(|p| p.exists())
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "onpkg".to_string())
    }
}

fn required_arg<'a>(arguments: &'a Value, keys: &[&str], message: &str) -> Result<&'a str> {
    keys.iter()
        .find_map(|key| arguments.get(*key).and_then(Value::as_str))
        .ok_or_else(|| anyhow!("{}", message))
}

fn optional_arg<'a>(arguments: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| arguments.get(*key).and_then(Value::as_str))
}

fn append_pkg_subcommand(cmd: &mut Command, subcmd: &str, arguments: &Value, action: &str) -> Result<()> {
    let name = required_arg(
        arguments,
        &["name", "pkg", "package"],
        &format!("Missing 'name' parameter for {} action", action),
    )?;
    cmd.args(["pkg", subcmd, name]);
    match optional_arg(arguments, &["runtime", "rt"]) {
        Some(rt) => {
            cmd.arg("--runtime");
            cmd.arg(rt);
        }
        None => {}
    }
    Ok(())
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
                    "description": "The onpkg action to perform (defaults to 'list_stacks')."
                },
                "stack_name": {
                    "type": "string",
                    "description": "The name of the template/stack to scaffold or show details for (required for 'scaffold' and 'show_stack', aliases: stack, template)."
                },
                "dir": {
                    "type": "string",
                    "description": "The target directory to scaffold the stack into (optional for 'scaffold', defaults to current directory, aliases: path, directory)."
                },
                "name": {
                    "type": "string",
                    "description": "The name for the template, skill, or package to add/install (aliases: pkg, package, skill)."
                },
                "source": {
                    "type": "string",
                    "description": "The path to the local directory/file or the remote git URL to add (aliases: src, url, path)."
                },
                "runtime": {
                    "type": "string",
                    "enum": ["npm", "pypi", "pub", "cargo"],
                    "description": "The runtime category for packages (optional for package actions, aliases: rt)."
                }
            }
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let action_raw = arguments
            .as_str()
            .or_else(|| optional_arg(arguments, &["action", "command", "subcommand", "cmd"]))
            .unwrap_or("list_stacks")
            .to_string();

        let action_norm = action_raw.trim().to_lowercase();
        let action = match action_norm.as_str() {
            "list_stacks" | "list" | "stacks" | "stack_list" => "list_stacks",
            "show_stack" | "show" | "info" | "stack_show" => "show_stack",
            "scaffold" | "add_stack" | "create" | "init" => "scaffold",
            "doctor" | "health" | "check" => "doctor",
            "add_template" | "template_add" => "add_template",
            "add_skill" | "skill_add" => "add_skill",
            "add_package" | "pkg_add" => "add_package",
            "install_package" | "install" | "pkg_install" => "install_package",
            _ => action_norm.as_str(),
        };

        let onpkg_bin = Self::resolve_binary();
        let mut cmd = Command::new(&onpkg_bin);
        crate::config::loader::set_tokio_command_cwd(&mut cmd);

        match action {
            "list_stacks" => {
                cmd.args(["stack", "list"]);
            }
            "show_stack" => {
                let stack = required_arg(
                    arguments,
                    &["stack_name", "stack", "template", "name"],
                    "Missing 'stack_name' parameter for show_stack action",
                )?;
                cmd.args(["stack", "show", stack]);
            }
            "scaffold" => {
                let stack = required_arg(
                    arguments,
                    &["stack_name", "stack", "template", "name"],
                    "Missing 'stack_name' parameter for scaffold action",
                )?;
                cmd.args(["stack", "add", stack]);
                match optional_arg(arguments, &["dir", "path", "directory"]) {
                    Some(dir) => {
                        let resolved = crate::config::resolve_path(dir);
                        cmd.arg("--dir");
                        cmd.arg(resolved.to_string_lossy().to_string());
                    }
                    None => {}
                }
            }
            "doctor" => {
                cmd.arg("doctor");
            }
            "add_template" => {
                let name = required_arg(
                    arguments,
                    &["name", "template"],
                    "Missing 'name' parameter for add_template action",
                )?;
                let source = required_arg(
                    arguments,
                    &["source", "src", "url", "path"],
                    "Missing 'source' parameter for add_template action",
                )?;
                cmd.args(["template", "add", name, source]);
            }
            "add_skill" => {
                let name = required_arg(
                    arguments,
                    &["name", "skill"],
                    "Missing 'name' parameter for add_skill action",
                )?;
                let source = required_arg(
                    arguments,
                    &["source", "src", "url", "path"],
                    "Missing 'source' parameter for add_skill action",
                )?;
                cmd.args(["skill", "add", name, source]);
            }
            "add_package" => {
                append_pkg_subcommand(&mut cmd, "add", arguments, "add_package")?;
            }
            "install_package" => {
                append_pkg_subcommand(&mut cmd, "install", arguments, "install_package")?;
            }
            _ => return Err(anyhow!("Unsupported onpkg action: {}", action)),
        }

        let output = cmd.output().await?;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let status = match output.status.success() {
            true => "success",
            false => "error",
        };

        Ok(json!({
            "status": status,
            "stdout": stdout,
            "stderr": stderr,
            "code": output.status.code()
        }))
    }
}

const ARCHITECTURE_PATH_PROBES: &[(&str, &[&str])] = &[
    ("components", &["src/components", "components"]),
    ("routing", &["src/pages", "pages", "src/routes", "routes"]),
    (
        "entrypoint",
        &[
            "src/main.rs",
            "src/lib.rs",
            "src/main.tsx",
            "src/index.js",
            "src/index.ts",
            "index.html",
            "src/main.py",
            "main.py",
        ],
    ),
    ("styles", &["src/index.css", "index.css", "src/app.css", "app.css"]),
];

fn collect_cargo_toml_packages(core_packages: &mut std::collections::HashSet<String>) {
    let toml_str = match std::fs::read_to_string("Cargo.toml") {
        Ok(content) => content,
        Err(_) => return,
    };
    let mut in_deps = false;
    for line in toml_str.lines() {
        let trimmed = line.trim();
        match trimmed.starts_with('[') {
            true => {
                in_deps = trimmed == "[dependencies]" || trimmed == "[workspace.dependencies]";
                match trimmed
                    .strip_prefix("[dependencies.")
                    .and_then(|s| s.strip_suffix(']'))
                {
                    Some(pkg) => {
                        core_packages.insert(pkg.trim().to_string());
                    }
                    None => {}
                }
            }
            false => match in_deps && !trimmed.is_empty() && !trimmed.starts_with('#') {
                true => match trimmed.split('=').next().map(str::trim) {
                    Some(pkg_name) => {
                        core_packages.insert(pkg_name.to_string());
                    }
                    None => {}
                },
                false => {}
            },
        }
    }
}

fn collect_package_json_packages(core_packages: &mut std::collections::HashSet<String>) {
    let pkg_json = match std::fs::read_to_string("package.json")
        .ok()
        .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
    {
        Some(val) => val,
        None => return,
    };
    for field in ["dependencies", "devDependencies"] {
        match pkg_json.get(field).and_then(Value::as_object) {
            Some(map) => {
                for key in map.keys() {
                    core_packages.insert(key.to_string());
                }
            }
            None => {}
        }
    }
}

pub fn sync_onpkg_manifest() -> Result<()> {
    let onpkg_json_path = std::path::Path::new("onpkg.json");
    match onpkg_json_path.exists() {
        true => {}
        false => return Ok(()),
    }

    // 1. Read onpkg.json
    let content = std::fs::read_to_string(onpkg_json_path)?;
    let mut manifest: Value =
        serde_json::from_str(&content).map_err(|e| anyhow!("Failed to parse onpkg.json: {}", e))?;

    // Ensure agent_instructions and docs_directory exist
    match manifest.get("agent_instructions") {
        Some(_) => {}
        None => {
            manifest["agent_instructions"] = json!({
                "active_skills": [],
                "docs_directory": "onpkg_docs/"
            });
        }
    }
    let docs_dir_name = manifest["agent_instructions"]
        .get("docs_directory")
        .and_then(|v| v.as_str())
        .unwrap_or("onpkg_docs/")
        .to_string();

    let docs_dir = std::path::PathBuf::from(docs_dir_name);
    match docs_dir.exists() {
        true => {}
        false => std::fs::create_dir_all(&docs_dir)?,
    }

    // 2. Create the 5 core workflow files if they don't exist
    let workflow_files = [
        ("prd.md", prd_template()),
        ("content.md", content_template()),
        ("design.md", design_template()),
        ("implementation.md", implementation_template()),
        ("todo.md", todo_template()),
    ];

    for (filename, template) in &workflow_files {
        let file_path = docs_dir.join(filename);
        match file_path.exists() {
            true => {}
            false => std::fs::write(&file_path, template)?,
        }
    }

    // 3. Scan docs_dir for all .md files (except INDEX.md) to update active_skills
    let mut active_skills: Vec<String> = std::fs::read_dir(&docs_dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .filter(|path| path.extension().and_then(|s| s.to_str()) == Some("md"))
        .filter_map(|path| {
            path.file_name()
                .and_then(|s| s.to_str())
                .filter(|name| *name != "INDEX.md")
                .map(str::to_string)
        })
        .collect();
    active_skills.sort();

    // Update active_skills in manifest
    manifest["agent_instructions"]["active_skills"] = json!(active_skills);

    // 4. Update INDEX.md dynamically
    let index_path = docs_dir.join("INDEX.md");
    let mut index_content = String::from(
        "# Project AI Agent Skills 🧠\n\nThis directory contains instructions and guidelines for AI agents working on this project.\n\n## Available Skills\n",
    );
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
    match manifest.get("architecture") {
        Some(_) => {}
        None => manifest["architecture"] = json!({}),
    }

    for (field, candidates) in ARCHITECTURE_PATH_PROBES {
        match candidates.iter().find(|p| std::path::Path::new(p).exists()) {
            Some(found) => manifest["architecture"][*field] = json!(found),
            None => {}
        }
    }

    // 6. Detect packages
    match manifest.get("packages") {
        Some(_) => {}
        None => {
            manifest["packages"] = json!({
                "added_by_agent": [],
                "core": []
            });
        }
    }

    let mut core_packages = std::collections::HashSet::new();
    collect_cargo_toml_packages(&mut core_packages);
    collect_package_json_packages(&mut core_packages);

    let mut core_vec: Vec<String> = core_packages.into_iter().collect();
    core_vec.sort();

    // Preserve existing added_by_agent packages, but update core packages
    let mut added_by_agent: Vec<String> = manifest["packages"]
        .get("added_by_agent")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect();
    added_by_agent.sort();

    manifest["packages"]["core"] = json!(core_vec);
    manifest["packages"]["added_by_agent"] = json!(added_by_agent);

    // 7. Write back if modified
    let new_content = serde_json::to_string_pretty(&manifest)?;
    match new_content != content {
        true => std::fs::write(onpkg_json_path, new_content)?,
        false => {}
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
#[path = "onpkg_tests.rs"]
mod tests;

