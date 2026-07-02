use crate::tools::Tool;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

// ─── Constants ───────────────────────────────────────────────────

const SCOPE_FILES: &[&str] = &["AGENTS.md", "CLAUDE.md", "CURSOR.md", ".cursorrules"];

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

fn is_binary_file(path: &Path) -> bool {
    if let Ok(content) = std::fs::read(path) {
        content.iter().take(4096).any(|&b| b == 0x00)
    } else {
        false
    }
}

fn detect_project_type(root: &Path) -> &'static str {
    if root.join("Cargo.toml").exists() { return "Rust"; }
    if root.join("package.json").exists() { return "Node.js"; }
    if root.join("go.mod").exists() { return "Go"; }
    if root.join("pom.xml").exists() || root.join("build.gradle").exists() { return "Java"; }
    if root.join("pyproject.toml").exists() || root.join("setup.py").exists() { return "Python"; }
    if root.join("Gemfile").exists() { return "Ruby"; }
    if root.join("CMakeLists.txt").exists() { return "C/C++"; }
    if root.join(".csproj").exists() { return "C#"; }
    if root.join("Cargo.toml").exists() { return "Rust"; }
    "Unknown"
}

// ─── TreeNode for summarize_codebase ────────────────────────────

#[derive(Debug, Clone)]
struct TreeNode {
    name: String,
    is_dir: bool,
    line_count: usize,
    files_count: usize,
    children: BTreeMap<String, TreeNode>,
}

impl TreeNode {
    fn insert(&mut self, parts: &[String], is_dir: bool, line_count: usize) {
        if parts.is_empty() { return; }
        if parts.len() == 1 {
            let child = self.children.entry(parts[0].clone()).or_insert_with(|| TreeNode {
                name: parts[0].clone(), is_dir, line_count: 0, files_count: 0, children: BTreeMap::new(),
            });
            child.line_count = line_count;
            child.files_count = 1;
        } else {
            let child = self.children.entry(parts[0].clone()).or_insert_with(|| TreeNode {
                name: parts[0].clone(), is_dir: true, line_count: 0, files_count: 0, children: BTreeMap::new(),
            });
            child.insert(&parts[1..], is_dir, line_count);
        }
    }

    fn update_counts(&mut self) {
        self.files_count = 0;
        self.line_count = 0;
        for child in self.children.values_mut() {
            child.update_counts();
            self.files_count += child.files_count;
            self.line_count += child.line_count;
        }
    }

    fn format_tree(&self, prefix: &str, is_last: bool, depth: usize, max_depth: usize) -> String {
        if depth > max_depth {
            if self.files_count > 0 {
                return format!("{}└── {} ({} files, {} lines)\n", prefix, self.name, self.files_count, self.line_count);
            }
            return String::new();
        }
        let connector = if is_last { "└── " } else { "├── " };
        let mut result = format!("{}{}{}", prefix, connector, self.name);
        if !self.is_dir && depth > 0 {
            result.push_str(&format!(" ({} lines)", self.line_count));
        } else if depth > 0 {
            result.push('/');
        }
        result.push('\n');

        let child_prefix = if is_last { format!("{}    ", prefix) } else { format!("{}│   ", prefix) };
        let children: Vec<&str> = self.children.keys().map(|k| k.as_str()).collect();
        for (i, child_name) in children.iter().enumerate() {
            if let Some(child) = self.children.get(*child_name) {
                let is_last_child = i == children.len() - 1;
                result.push_str(&child.format_tree(&child_prefix, is_last_child, depth + 1, max_depth));
            }
        }
        result
    }
}

// ═══════════════════════════════════════════════════════════════════
// Tool 1: ScopeContextTool
// ═══════════════════════════════════════════════════════════════════

pub struct ScopeContextTool;

#[async_trait::async_trait]
impl Tool for ScopeContextTool {
    fn name(&self) -> &str { "scope_context" }

    fn description(&self) -> &str {
        "Walks up the directory tree from the target path and retrieves all relevant context files (AGENTS.md, CLAUDE.md, CURSOR.md, .cursorrules) for the target file path."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "target_path": {
                    "type": "string",
                    "description": "Absolute or relative path to the file/directory the agent is working with."
                }
            },
            "required": ["target_path"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let target_path = arguments["target_path"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing target_path parameter"))?;

        let cwd = std::env::current_dir().map_err(|e| anyhow!("Failed to get cwd: {}", e))?;
        let path = Path::new(target_path);
        let absolute_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            cwd.join(path)
        };

        let resolved_path = absolute_path.canonicalize()
            .map_err(|e| anyhow!("Failed to resolve path '{}': {}", target_path, e))?;

        let target_dir = if resolved_path.is_dir() {
            resolved_path.clone()
        } else {
            resolved_path.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| cwd.clone())
        };

        let mut found_files: Vec<PathBuf> = Vec::new();
        let mut current_dir = Some(target_dir.as_path());

        while let Some(dir) = current_dir {
            for filename in SCOPE_FILES {
                let file_path = dir.join(filename);
                if file_path.is_file() {
                    found_files.push(file_path);
                }
            }
            if dir.join(".git").exists() || dir.parent().is_none() {
                break;
            }
            current_dir = dir.parent();
        }

        found_files.reverse();

        if found_files.is_empty() {
            return Ok(json!({
                "status": "empty",
                "content": "No context files (AGENTS.md, CLAUDE.md, etc.) found in the path hierarchy."
            }));
        }

        let mut combined = String::new();
        for fp in &found_files {
            if let Ok(content) = std::fs::read_to_string(fp) {
                let relative = fp.strip_prefix(&cwd).unwrap_or(fp);
                combined.push_str(&format!("### Context File: {}\n\n{}\n\n", relative.display(), content));
            }
        }

        let enforce_yagni = std::env::var("HEADROOM_ENFORCE_YAGNI")
            .map(|val| val.to_lowercase() == "true")
            .unwrap_or(false);

        if enforce_yagni {
            combined.push_str(YAGNI_DIRECTIVES);
        }

        Ok(json!({ "status": "ok", "files": found_files.iter().filter_map(|f| f.to_str()).collect::<Vec<_>>(), "content": combined }))
    }
}

// ═══════════════════════════════════════════════════════════════════
// Tool 19: SummarizeCodebaseTool
// ═══════════════════════════════════════════════════════════════════

pub struct SummarizeCodebaseTool;

#[async_trait::async_trait]
impl Tool for SummarizeCodebaseTool {
    fn name(&self) -> &str { "summarize_codebase" }
    fn description(&self) -> &str { "Analyzes the codebase and returns a summary of language usage, file sizes, and directory layout." }
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
        let root = Path::new(root_path_str);
        let resolved_root = if root.is_absolute() { root.to_path_buf() }
            else { std::env::current_dir().map_err(|e| anyhow!("{}", e))?.join(root) };
        let resolved_root = resolved_root.canonicalize()
            .map_err(|e| anyhow!("Failed to resolve path '{}': {}", root_path_str, e))?;

        if !resolved_root.is_dir() {
            return Err(anyhow!("'{}' is not a directory", root_path_str));
        }

        let file_limit = 1000usize;

        let mut tree_root = TreeNode {
            name: resolved_root.file_name().and_then(|n| n.to_str()).unwrap_or(".").to_string(),
            is_dir: true, line_count: 0, files_count: 0, children: BTreeMap::new(),
        };

        let mut total_files = 0usize;
        let mut total_lines = 0usize;
        let mut ext_counts: std::collections::HashMap<String, (usize, usize)> = std::collections::HashMap::new();

        let mut walk_dir = vec![(resolved_root.clone(), 0usize)];
        while let Some((dir_path, depth)) = walk_dir.pop() {
            if depth > 10 { continue; }
            if total_files >= file_limit { break; }

            let entries = match std::fs::read_dir(&dir_path) {
                Ok(e) => e,
                Err(_) => continue,
            };

            let mut dirs = Vec::new();
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.components().any(|c| c.as_os_str() == ".git") { continue; }

                if path.is_dir() {
                    dirs.push(path);
                    continue;
                }

                if is_binary_file(&path) { continue; }

                let content = match std::fs::read_to_string(&path) {
                    Ok(c) => c,
                    Err(_) => continue,
                };
                let file_lines = content.lines().count();

                total_files += 1;
                total_lines += file_lines;

                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("no_ext").to_lowercase();
                let entry = ext_counts.entry(ext).or_insert((0, 0));
                entry.0 += 1;
                entry.1 += file_lines;

                if let Ok(rel_path) = path.strip_prefix(&resolved_root) {
                    let parts: Vec<String> = rel_path.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
                    tree_root.insert(&parts, false, file_lines);
                }

                if total_files >= file_limit { break; }
            }

            for d in dirs.into_iter().rev() {
                walk_dir.push((d, depth + 1));
            }
        }

        tree_root.update_counts();
        let tree_str = tree_root.format_tree("", true, 0, 3);

        let project_type = detect_project_type(&resolved_root);
        let mut breakdown: Vec<Value> = ext_counts.into_iter()
            .map(|(ext, (count, lines))| json!({ "extension": ext, "files": count, "lines": lines }))
            .collect();
        breakdown.sort_by(|a, b| b["lines"].as_u64().unwrap_or(0).cmp(&a["lines"].as_u64().unwrap_or(0)));

        let suffix = if total_files >= file_limit {
            format!("\nWarning: Walk stopped early because file limit ({}) was reached.", file_limit)
        } else { String::new() };

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
