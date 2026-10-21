use super::is_binary_file;
use super::policy::{resolve_user_path, to_absolute_path};
use crate::tools::Tool;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

// ─── Constants ───────────────────────────────────────────────────

const SCOPE_FILES: &[&str] = &[
    "AGENTS.md",
    "CLAUDE.md",
    "GEMINI.md",
    "CURSOR.md",
    ".cursorrules",
];

const PROJECT_MARKERS: &[(&[&str], &str)] = &[
    (&["Cargo.toml"], "Rust"),
    (&["package.json"], "Node.js"),
    (&["go.mod"], "Go"),
    (&["pom.xml", "build.gradle"], "Java"),
    (&["pyproject.toml", "setup.py"], "Python"),
    (&["Gemfile"], "Ruby"),
    (&["CMakeLists.txt"], "C/C++"),
    (&[".csproj"], "C#"),
];

const SCOPE_PATH_KEYS: &[&str] = &[
    "target_path",
    "path",
    "target",
    "dir",
    "directory",
    "file",
    "filepath",
];

pub const YAGNI_DIRECTIVES: &str = r#"
---
### Headroom: YAGNI Minimalism Directives

Before writing ANY code, walk down this ladder and stop at the FIRST rung that applies:

1. **Does this need to exist?** → If no: skip it entirely (YAGNI).
2. **Already in this codebase?** → Reuse it. Do not rewrite.
3. **Standard library does it?** → Use std. No external crate/package.
4. **Native platform feature?** → Use it (e.g., `<input type="date">` over a date-picker library).
5. **Already-installed dependency does it?** → Use what's there. Don't add a new dep.
6. **Can it be one line?** → Write one line.
7. **Only then:** Implement the minimum that works.

**Never skip:** validation, error handling, security checks, accessibility.
The code should be small because it is *necessary*, not golfed.
---
"#;

// ─── Helpers ─────────────────────────────────────────────────────

pub fn detect_project_type(root: &Path) -> &'static str {
    PROJECT_MARKERS
        .iter()
        .find_map(|(markers, project_type)| {
            markers
                .iter()
                .any(|m| root.join(m).exists())
                .then_some(*project_type)
        })
        .unwrap_or("Unknown")
}

fn canonical_dir_or_parent(canonical: PathBuf, fallback: &Path) -> PathBuf {
    match canonical.is_dir() {
        true => canonical,
        false => canonical
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| fallback.to_path_buf()),
    }
}

fn resolve_nearest_existing_dir(absolute_path: &Path, cwd: &Path) -> PathBuf {
    match absolute_path.canonicalize() {
        Ok(canonical) => canonical_dir_or_parent(canonical, cwd),
        Err(_) => {
            let mut ancestor = absolute_path.parent();
            let mut resolved = None;
            while let Some(parent) = ancestor {
                match parent.canonicalize() {
                    Ok(canonical) => {
                        resolved = Some(canonical_dir_or_parent(canonical, cwd));
                        break;
                    }
                    Err(_) => ancestor = parent.parent(),
                }
            }
            resolved.unwrap_or_else(|| cwd.to_path_buf())
        }
    }
}

// ─── TreeNode for summarize_codebase ────────────────────────────

use super::DirTreeNode;

// ═══════════════════════════════════════════════════════════════════
// Tool 1: ScopeContextTool
// ═══════════════════════════════════════════════════════════════════

pub struct ScopeContextTool;

#[async_trait::async_trait]
impl Tool for ScopeContextTool {
    fn name(&self) -> &str {
        "scope_context"
    }

    fn description(&self) -> &str {
        "Walks up the directory tree from the target path and retrieves all relevant context files (AGENTS.md, CLAUDE.md, CURSOR.md, .cursorrules) for the target file path."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "target_path": {
                    "type": "string",
                    "description": "Absolute or relative path to the file/directory the agent is working with (aliases: path, target, dir, directory, file). Defaults to current directory."
                }
            }
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let target_path = arguments
            .as_str()
            .or_else(|| {
                SCOPE_PATH_KEYS
                    .iter()
                    .find_map(|&k| arguments.get(k).and_then(|v| v.as_str()))
            })
            .map(str::trim)
            .unwrap_or(".");

        let cwd = std::env::current_dir().map_err(|e| anyhow!("Failed to get cwd: {}", e))?;
        let absolute_path = to_absolute_path(target_path)?;
        let target_dir = resolve_nearest_existing_dir(&absolute_path, &cwd);

        let mut found_files: Vec<PathBuf> = Vec::new();
        let mut current_dir = Some(target_dir.as_path());

        while let Some(dir) = current_dir {
            for filename in SCOPE_FILES {
                let file_path = dir.join(filename);
                match file_path.is_file() {
                    true => found_files.push(file_path),
                    false => {}
                }
            }
            match dir.join(".git").exists() || dir.parent().is_none() {
                true => break,
                false => current_dir = dir.parent(),
            }
        }

        found_files.reverse();

        match found_files.is_empty() {
            true => {
                return Ok(json!({
                    "status": "empty",
                    "content": "No context files (AGENTS.md, CLAUDE.md, etc.) found in the path hierarchy."
                }))
            }
            false => {}
        }

        let mut combined = String::new();
        for fp in &found_files {
            match std::fs::read_to_string(fp) {
                Ok(content) => {
                    let relative = fp.strip_prefix(&cwd).unwrap_or(fp);
                    combined.push_str(&format!(
                        "### Context File: {}\n\n{}\n\n",
                        relative.display(),
                        content
                    ));
                }
                Err(_) => {}
            }
        }

        let enforce_yagni = std::env::var("HEADROOM_ENFORCE_YAGNI")
            .map(|val| val.to_lowercase() == "true")
            .unwrap_or(false);

        match enforce_yagni {
            true => combined.push_str(YAGNI_DIRECTIVES),
            false => {}
        }

        Ok(json!({
            "status": "ok",
            "files": found_files.iter().filter_map(|f| f.to_str()).collect::<Vec<_>>(),
            "content": combined
        }))
    }
}

// ═══════════════════════════════════════════════════════════════════
// Tool 19: SummarizeCodebaseTool
// ═══════════════════════════════════════════════════════════════════

pub struct SummarizeCodebaseTool;

#[async_trait::async_trait]
impl Tool for SummarizeCodebaseTool {
    fn name(&self) -> &str {
        "summarize_codebase"
    }
    fn description(&self) -> &str {
        "Analyzes the codebase and returns a summary of language usage, file sizes, and directory layout."
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "root_path": { "type": "string", "description": "Root directory to analyze (default: current directory)." }
            }
        })
    }
    async fn call(&self, arguments: &Value) -> Result<Value> {
        let root_path_str = arguments["root_path"].as_str().unwrap_or(".");
        let resolved_root = resolve_user_path(root_path_str)?;

        match resolved_root.is_dir() {
            true => {}
            false => return Err(anyhow!("'{}' is not a directory", root_path_str)),
        }

        let file_limit = 1000usize;

        let mut tree_root = DirTreeNode::new_root(&resolved_root);

        let mut total_files = 0usize;
        let mut total_lines = 0usize;
        let mut ext_counts: std::collections::HashMap<String, (usize, usize)> =
            std::collections::HashMap::new();

        let mut walk_dir = vec![(resolved_root.clone(), 0usize)];
        while let Some((dir_path, depth)) = walk_dir.pop() {
            match depth > 10 {
                true => continue,
                false => {}
            }
            match total_files >= file_limit {
                true => break,
                false => {}
            }

            let entries = match std::fs::read_dir(&dir_path) {
                Ok(e) => e,
                Err(_) => continue,
            };

            let mut dirs = Vec::new();
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                match path.components().any(|c| c.as_os_str() == ".git") {
                    true => continue,
                    false => {}
                }

                match path.is_dir() {
                    true => {
                        dirs.push(path);
                        continue;
                    }
                    false => {}
                }

                match is_binary_file(&path) {
                    true => continue,
                    false => {}
                }

                let content = match std::fs::read_to_string(&path) {
                    Ok(c) => c,
                    Err(_) => continue,
                };
                let file_lines = content.lines().count();

                total_files += 1;
                total_lines += file_lines;

                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("no_ext")
                    .to_lowercase();
                let entry = ext_counts.entry(ext).or_insert((0, 0));
                entry.0 += 1;
                entry.1 += file_lines;

                match path.strip_prefix(&resolved_root) {
                    Ok(rel_path) => {
                        tree_root.insert_rel_path(
                            rel_path,
                            file_lines,
                            format!(" ({} lines)", file_lines),
                        );
                    }
                    Err(_) => {}
                }

                match total_files >= file_limit {
                    true => break,
                    false => {}
                }
            }

            for d in dirs.into_iter().rev() {
                walk_dir.push((d, depth + 1));
            }
        }

        tree_root.update_counts();
        let tree_str = tree_root.format_tree("", true, 0, 3, true);

        let project_type = detect_project_type(&resolved_root);
        let mut breakdown: Vec<Value> = ext_counts
            .into_iter()
            .map(
                |(ext, (count, lines))| json!({ "extension": ext, "files": count, "lines": lines }),
            )
            .collect();
        breakdown.sort_by(|a, b| {
            b["lines"]
                .as_u64()
                .unwrap_or(0)
                .cmp(&a["lines"].as_u64().unwrap_or(0))
        });

        let suffix = match total_files >= file_limit {
            true => format!(
                "\nWarning: Walk stopped early because file limit ({}) was reached.",
                file_limit
            ),
            false => String::new(),
        };

        Ok(json!({
            "project_name": tree_root.name,
            "project_type": project_type,
            "total_files": total_files,
            "total_lines": total_lines,
            "breakdown": breakdown,
            "directory_tree": tree_str,
            "note": suffix,
        }))
    }
}
