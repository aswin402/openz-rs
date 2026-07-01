use crate::agent::context_compactor;
use crate::tools::Tool;
use anyhow::{anyhow, Result};
use chrono::Utc;
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::SystemTime;

// ─── Constants ───────────────────────────────────────────────────

const SCOPE_FILES: &[&str] = &["AGENTS.md", "CLAUDE.md", "CURSOR.md", ".cursorrules"];
const MAX_INPUT_SIZE: usize = 512_000; // 500KB max input
const CACHE_CAPACITY: usize = 1000;

static CCR_COUNTER: AtomicU64 = AtomicU64::new(0);

// ─── DB path & connection ───────────────────────────────────────

fn get_db_path() -> PathBuf {
    if let Ok(override_dir) = std::env::var("OPENZ_CONFIG_DIR") {
        PathBuf::from(override_dir).join("ccr_cache.db")
    } else {
        crate::config::resolve_path("~/.openz/ccr_cache.db")
    }
}

fn get_cache_connection() -> Result<std::sync::MutexGuard<'static, Connection>> {
    static DB: OnceLock<std::sync::Mutex<Connection>> = OnceLock::new();
    let mtx = DB.get_or_init(|| {
        let path = get_db_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(&path).unwrap_or_else(|_| {
            Connection::open_in_memory().unwrap()
        });
        conn.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;
             CREATE TABLE IF NOT EXISTS cache_entries (
                 ccr_id TEXT PRIMARY KEY,
                 content TEXT NOT NULL,
                 created_at TEXT NOT NULL,
                 accessed_at TEXT NOT NULL,
                 size_bytes INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS compression_log (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 tool_name TEXT NOT NULL,
                 original_size INTEGER NOT NULL,
                 compressed_size INTEGER NOT NULL,
                 original_tokens INTEGER NOT NULL,
                 compressed_tokens INTEGER NOT NULL,
                 content_type TEXT NOT NULL,
                 model_hint TEXT,
                 created_at TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_cache_accessed ON cache_entries(accessed_at);",
        ).ok();
        std::sync::Mutex::new(conn)
    });
    Ok(mtx.lock().map_err(|e| anyhow!("Cache lock error: {}", e))?)
}

// ─── Helpers ─────────────────────────────────────────────────────

fn estimate_tokens(text: &str) -> usize {
    if text.is_empty() { return 0; }
    (text.len() + 3) / 4
}

fn auto_detect_type(text: &str) -> &'static str {
    let trimmed = text.trim();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        return "json";
    }
    let lines: Vec<&str> = trimmed.lines().collect();
    if lines.len() > 1 {
        let first_line = lines[0].trim();
        if first_line.len() <= 3 && lines.len() > 3 {
            return "csv";
        }
        let log_line_count = lines.iter().filter(|l| {
            l.len() >= 19 && (l.as_bytes().get(4) == Some(&b'-') || l.as_bytes().get(10) == Some(&b':'))
        }).count();
        if log_line_count as f64 > lines.len() as f64 * 0.3 {
            return "text_logs";
        }
    }
    let code_keywords = ["fn ", "def ", "function ", "class ", "import ", "const ", "let ", "var ", "pub ", "impl "];
    if code_keywords.iter().any(|kw| trimmed.contains(kw)) {
        return "code";
    }
    "text_logs"
}

fn detect_content_type_from_ext(path: &Path) -> Option<&'static str> {
    let ext = path.extension().and_then(|e| e.to_str())?;
    match ext.to_lowercase().as_str() {
        "json" => Some("json"),
        "csv" => Some("csv"),
        "md" | "markdown" => Some("markdown"),
        "yml" | "yaml" => Some("yaml"),
        "log" | "txt" => Some("text_logs"),
        "rs" | "py" | "js" | "ts" | "c" | "cpp" | "h" | "go" | "java" | "sh" | "sql" | "html" | "css" => Some("code"),
        _ => None,
    }
}

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

fn evict_lru_if_needed() {
    if let Ok(conn) = get_cache_connection() {
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM cache_entries", [], |r| r.get(0)).unwrap_or(0);
        if count > CACHE_CAPACITY as i64 {
            let _ = conn.execute(
                "DELETE FROM cache_entries WHERE ccr_id IN (
                    SELECT ccr_id FROM cache_entries ORDER BY accessed_at ASC LIMIT ?
                )", params![count - CACHE_CAPACITY as i64],
            );
        }
    }
}

fn compress_csv(raw_csv: &str) -> String {
    let mut lines = raw_csv.lines();
    let mut result = String::new();
    if let Some(header) = lines.next() {
        result.push_str(&format!("Headers: {}\n", header));
    }
    let mut count = 0;
    for line in lines.by_ref() {
        if count < 3 {
            result.push_str(&format!("Row {}: {}\n", count + 1, line));
        }
        count += 1;
    }
    result.push_str(&format!("[CCR Summary: CSV contains {} rows total]", count));
    result
}

/// Generate a new CCR ID using timestamp + atomic counter.
fn generate_ccr_id() -> String {
    let time_ns = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let seq = CCR_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("ccr_{:x}_{:x}", time_ns & 0xFFFFFFFF, seq)
}

/// Compute compression stats and format the CCR result string.
fn format_ccr_result(compressed: &str, raw_text: &str, ccr_id: Option<&str>, tool: &str) -> Value {
    let original_tokens = estimate_tokens(raw_text);
    let compressed_tokens = estimate_tokens(compressed);
    let saved_pct = if original_tokens > 0 {
        format!("{:.1}%", ((original_tokens as f64 - compressed_tokens as f64) / original_tokens as f64 * 100.0).max(0.0))
    } else {
        "0.0%".to_string()
    };
    format_ccr_result_detailed(compressed, raw_text, ccr_id, tool, &saved_pct, original_tokens, compressed_tokens)
}

fn format_ccr_result_detailed(compressed: &str, _raw_text: &str, ccr_id: Option<&str>, _tool: &str, saved_pct: &str, original_tokens: usize, compressed_tokens: usize) -> Value {
    let mut result = json!({
        "compressed": compressed,
        "ccr_id": ccr_id,
        "original_tokens": original_tokens,
        "compressed_tokens": compressed_tokens,
        "saved": saved_pct,
    });
    if let Some(id) = ccr_id {
        result["note"] = json!(format!("CCR Ref: {} | Use retrieve_original tool to inspect full content", id));
    }
    result
}

/// Insert content into cache and return a CCR ID.
fn cache_content(content: &str) -> Result<String> {
    let id = generate_ccr_id();
    let now = Utc::now().to_rfc3339();
    {
        let conn = get_cache_connection()?;
        conn.execute(
            "INSERT OR REPLACE INTO cache_entries (ccr_id, content, created_at, accessed_at, size_bytes) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, content, now, now, content.len() as i64],
        )?;
    }
    evict_lru_if_needed();
    Ok(id)
}

// ─── Diff parsing ────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
struct DiffFile {
    path: String,
    insertions: usize,
    deletions: usize,
    hunks_count: usize,
    is_binary: bool,
    is_new: bool,
    is_deleted: bool,
    contexts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
struct DiffSummary {
    files: Vec<DiffFile>,
    total_insertions: usize,
    total_deletions: usize,
}

fn clean_path(p: &str) -> String {
    let p = p.split('\t').next().unwrap_or(p).trim();
    if (p.starts_with("a/") || p.starts_with("b/")) && p.len() > 2 {
        p[2..].to_string()
    } else {
        p.to_string()
    }
}

fn commit_current_file(files: &mut Vec<DiffFile>, current: &mut Option<DiffFile>) {
    if let Some(f) = current.take() {
        if !f.path.is_empty() && f.path != "/dev/null" {
            files.push(f);
        }
    }
}

fn parse_unified_diff(text: &str) -> DiffSummary {
    let mut files = Vec::new();
    let mut current_file: Option<DiffFile> = None;

    for line in text.lines() {
        if line.starts_with("diff --git ") {
            commit_current_file(&mut files, &mut current_file);
        } else if line.starts_with("--- ") {
            let path_part = &line[4..];
            let cleaned = clean_path(path_part);
            if cleaned == "/dev/null" {
                if let Some(ref mut f) = current_file {
                    f.is_new = true;
                } else {
                    current_file = Some(DiffFile {
                        path: "/dev/null".to_string(),
                        insertions: 0, deletions: 0, hunks_count: 0, is_binary: false, is_new: true, is_deleted: false, contexts: Vec::new(),
                    });
                }
            } else {
                if current_file.is_none() {
                    current_file = Some(DiffFile {
                        path: cleaned, insertions: 0, deletions: 0, hunks_count: 0, is_binary: false, is_new: false, is_deleted: false, contexts: Vec::new(),
                    });
                } else {
                    let mut f = current_file.take().unwrap();
                    if f.path == "/dev/null" {
                        f.path = cleaned;
                    } else if f.path != cleaned {
                        files.push(f);
                        current_file = Some(DiffFile {
                            path: cleaned, insertions: 0, deletions: 0, hunks_count: 0, is_binary: false, is_new: false, is_deleted: false, contexts: Vec::new(),
                        });
                        continue;
                    }
                    current_file = Some(f);
                }
            }
        } else if line.starts_with("+++ ") {
            let path_part = &line[4..];
            let cleaned = clean_path(path_part);
            if cleaned == "/dev/null" {
                if let Some(ref mut f) = current_file { f.is_deleted = true; }
            } else {
                if let Some(ref mut f) = current_file {
                    if f.path == "/dev/null" { f.path = cleaned; }
                } else {
                    current_file = Some(DiffFile {
                        path: cleaned, insertions: 0, deletions: 0, hunks_count: 0, is_binary: false, is_new: false, is_deleted: false, contexts: Vec::new(),
                    });
                }
            }
        } else if line.starts_with("@@") {
            if let Some(pos) = line.rfind("@@") {
                let context = line[pos + 2..].trim();
                if let Some(ref mut f) = current_file {
                    f.hunks_count += 1;
                    if !context.is_empty() && !f.contexts.contains(&context.to_string()) {
                        f.contexts.push(context.to_string());
                    }
                }
            }
        } else if line.starts_with("Binary files ") && line.contains(" differ") {
            commit_current_file(&mut files, &mut current_file);
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 5 && parts[1] == "files" && parts[3] == "and" {
                current_file = Some(DiffFile {
                    path: clean_path(parts[2]), insertions: 0, deletions: 0, hunks_count: 0, is_binary: true, is_new: false, is_deleted: false, contexts: Vec::new(),
                });
                commit_current_file(&mut files, &mut current_file);
            }
        } else if line.starts_with('+') {
            if let Some(ref mut f) = current_file { if !f.is_binary { f.insertions += 1; } }
        } else if line.starts_with('-') {
            if let Some(ref mut f) = current_file { if !f.is_binary { f.deletions += 1; } }
        }
    }
    commit_current_file(&mut files, &mut current_file);

    let mut total_insertions = 0;
    let mut total_deletions = 0;
    for f in &files { total_insertions += f.insertions; total_deletions += f.deletions; }
    DiffSummary { files, total_insertions, total_deletions }
}

fn compress_diff_text(diff_text: &str) -> String {
    let summary = parse_unified_diff(diff_text);
    if summary.files.is_empty() {
        return "No files changed in diff.".to_string();
    }
    let files_count = summary.files.len();
    let mut output = format!(
        "Diff Summary: {} file{} changed, {} insertion{}(+), {} deletion{}(-)\n\nModified files:",
        files_count, if files_count == 1 { "" } else { "s" },
        summary.total_insertions, if summary.total_insertions == 1 { "" } else { "s" },
        summary.total_deletions, if summary.total_deletions == 1 { "" } else { "s" }
    );
    for f in summary.files {
        output.push_str("\n- ");
        output.push_str(&f.path);
        output.push_str(": ");
        if f.is_binary {
            output.push_str("binary file changed");
        } else {
            output.push_str(&format!("+{}/-{}", f.insertions, f.deletions));
            if f.hunks_count > 0 {
                output.push_str(&format!(" ({} hunk{})", f.hunks_count, if f.hunks_count == 1 { "" } else { "s" }));
            }
            if !f.contexts.is_empty() {
                output.push_str(" (modified: ");
                output.push_str(&f.contexts.join(", "));
                output.push(')');
            }
        }
        if f.is_new { output.push_str(" [NEW]"); }
        else if f.is_deleted { output.push_str(" [DELETED]"); }
    }
    output
}

// ─── Command output filters ─────────────────────────────────────

fn filter_command_output(command: &str, raw: &str) -> String {
    let base_cmd = command.split_whitespace().next().unwrap_or("");
    let base_name = Path::new(base_cmd)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(base_cmd)
        .to_lowercase();

    match base_name.as_str() {
        "cargo" | "rustc" => filter_cargo_output(raw),
        "npm" | "npx" | "yarn" | "pnpm" | "bun" => filter_npm_output(raw),
        "git" => filter_git_output(raw),
        "python" | "python3" | "pytest" | "pip" | "pip3" => filter_python_output(raw),
        _ => raw.to_string(),
    }
}

fn filter_cargo_output(raw: &str) -> String {
    let mut result = Vec::new();
    let mut warning_count = 0;
    let mut omitted_warnings = 0;
    for line in raw.lines() {
        let trimmed_start = line.trim_start();
        if trimmed_start.starts_with("Compiling ") || trimmed_start.starts_with("Downloading ") { continue; }
        if trimmed_start.starts_with("test ") && line.trim_end().ends_with("... ok") { continue; }
        let is_warning = trimmed_start.starts_with("warning:") || line.contains("warning: ");
        if is_warning {
            if warning_count < 5 { warning_count += 1; result.push(line); }
            else { omitted_warnings += 1; }
            continue;
        }
        result.push(line);
    }
    let mut output = result.join("\n");
    if omitted_warnings > 0 { output.push_str(&format!("\n[{} more warnings omitted]", omitted_warnings)); }
    output
}

fn filter_npm_output(raw: &str) -> String {
    let mut result = Vec::new();
    let mut deprecated_count = 0;
    for line in raw.lines() {
        let trimmed = line.trim();
        let lower = trimmed.to_lowercase();
        if lower.contains("warn deprecated") || lower.contains("warning deprecated") {
            if deprecated_count < 3 { deprecated_count += 1; result.push(line); }
            continue;
        }
        if lower.contains("npm warn") || lower.contains("npm notice") { continue; }
        if trimmed.starts_with('✓') || trimmed.starts_with("PASS") { continue; }
        if trimmed.ends_with('=') || trimmed.ends_with('.') { /* spinner chars */ let _ = 0; }
        result.push(line);
    }
    result.join("\n")
}

fn filter_git_output(raw: &str) -> String {
    let mut result = Vec::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("Enumerating objects:")
            || trimmed.starts_with("Counting objects:")
            || trimmed.starts_with("Compressing objects:")
            || trimmed.starts_with("remote: Resolving deltas:")
            || trimmed.starts_with("Writing objects:")
            || trimmed.starts_with("remote: Counting objects:")
            || trimmed.starts_with("remote: Compressing objects:")
        { continue; }
        result.push(line);
    }
    result.join("\n")
}

fn filter_python_output(raw: &str) -> String {
    let mut result = Vec::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("Collecting ")
            || trimmed.starts_with("Downloading ")
            || trimmed.starts_with("Installing collected packages")
            || trimmed.starts_with("Requirement already satisfied:")
        { continue; }
        if is_python_noise(trimmed) { continue; }
        result.push(line);
    }
    result.join("\n")
}

fn is_python_noise(trimmed: &str) -> bool {
    let lower = trimmed.to_lowercase();
    if lower.contains("failed") || lower.contains("error") || lower.contains("traceback") { return false; }
    if trimmed.ends_with("PASSED") || lower.contains("passed [") || lower.contains("passed  [") { return true; }
    if trimmed.contains('.') && trimmed.contains('%') && (trimmed.contains("test_") || trimmed.contains(".py")) { return true; }
    false
}

// ─── Tree structures for compress_directory ─────────────────────

use std::collections::BTreeMap;

#[derive(Debug, Clone)]
struct CompTreeFile {
    ccr_id: String,
    original_tokens: usize,
    compressed_tokens: usize,
    saved_pct: String,
}

#[derive(Debug, Clone)]
struct CompTreeNode {
    name: String,
    is_dir: bool,
    files_count: usize,
    file_info: Option<CompTreeFile>,
    children: BTreeMap<String, CompTreeNode>,
}

impl CompTreeNode {
    fn insert(&mut self, parts: &[String], info: CompTreeFile) {
        if parts.is_empty() { return; }
        if parts.len() == 1 {
            let child = self.children.entry(parts[0].clone()).or_insert_with(|| CompTreeNode {
                name: parts[0].clone(), is_dir: false, files_count: 0, file_info: None, children: BTreeMap::new(),
            });
            child.file_info = Some(info);
            child.files_count = 1;
        } else {
            let child = self.children.entry(parts[0].clone()).or_insert_with(|| CompTreeNode {
                name: parts[0].clone(), is_dir: true, files_count: 0, file_info: None, children: BTreeMap::new(),
            });
            child.insert(&parts[1..], info);
        }
    }

    fn update_counts(&mut self) {
        self.files_count = 0;
        for child in self.children.values_mut() {
            child.update_counts();
            self.files_count += child.files_count;
        }
        if self.file_info.is_some() { self.files_count += 1; }
    }

    fn format_tree(&self, prefix: &str, is_last: bool, depth: usize, max_depth: usize) -> String {
        if depth > max_depth {
            if self.files_count > 0 {
                return format!("{}└── {} ({} files)\n", prefix, self.name, self.files_count);
            }
            return String::new();
        }
        let connector = if is_last { "└── " } else { "├── " };
        let mut result = format!("{}{}{}", prefix, connector, self.name);
        if let Some(ref info) = self.file_info {
            result.push_str(&format!(" [{} tokens -> {} tokens, saved {}]", info.original_tokens, info.compressed_tokens, info.saved_pct));
        } else if self.is_dir && depth > 0 {
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
// Tool 2: CompressContentTool
// ═══════════════════════════════════════════════════════════════════

pub struct CompressContentTool;

#[async_trait::async_trait]
impl Tool for CompressContentTool {
    fn name(&self) -> &str { "compress_content" }

    fn description(&self) -> &str {
        "Compresses logs, JSON, or code content, and registers a CCR reference token for the agent. The CCR token can be used with retrieve_original to get back the full content."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "raw_text": { "type": "string", "description": "The raw content to compress." },
                "content_type": {
                    "type": "string",
                    "enum": ["auto", "json", "code", "text_logs", "csv", "markdown", "yaml"],
                    "default": "auto",
                    "description": "Content type hint for optimal compression."
                },
                "preview": { "type": "boolean", "description": "If true, returns a preview without caching." }
            },
            "required": ["raw_text"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let raw_text = arguments["raw_text"].as_str().ok_or_else(|| anyhow!("Missing raw_text"))?.trim().to_string();
        if raw_text.is_empty() {
            return Ok(json!({ "compressed": "", "ccr_id": null, "note": "Empty content provided." }));
        }

        if raw_text.len() > MAX_INPUT_SIZE {
            return Err(anyhow!("Content size exceeds maximum allowed size of {} bytes", MAX_INPUT_SIZE));
        }

        let content_type = arguments["content_type"].as_str().unwrap_or("auto").to_lowercase();
        let content_type = if content_type == "auto" || content_type.is_empty() {
            auto_detect_type(&raw_text)
        } else {
            match content_type.as_str() {
                "json" | "code" | "text_logs" | "csv" | "markdown" | "yaml" => content_type.as_str(),
                other => return Err(anyhow!("Unknown content_type '{}'. Use 'json', 'code', 'text_logs', 'csv', 'markdown', 'yaml', or 'auto'.", other)),
            }
        };

        let compressed = match content_type {
            "json" => context_compactor::compress_json(&raw_text).unwrap_or_else(|_| raw_text.clone()),
            "code" | "yaml" | "markdown" => context_compactor::compress_code(&raw_text),
            "csv" => compress_csv(&raw_text),
            _ => context_compactor::compress_logs(&raw_text),
        };

        let is_preview = arguments["preview"].as_bool().unwrap_or(false);

        let ccr_id = if is_preview {
            None
        } else {
            let id = generate_ccr_id();
            let now = Utc::now().to_rfc3339();
            if let Ok(conn) = get_cache_connection() {
                let _ = conn.execute(
                    "INSERT OR REPLACE INTO cache_entries (ccr_id, content, created_at, accessed_at, size_bytes) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![id, raw_text, now, now, raw_text.len() as i64],
                );
            }
            evict_lru_if_needed();
            Some(id)
        };

        Ok(format_ccr_result(&compressed, &raw_text, ccr_id.as_deref(), "compress_content"))
    }
}

// ═══════════════════════════════════════════════════════════════════
// Tool 3: RetrieveOriginalTool
// ═══════════════════════════════════════════════════════════════════

pub struct RetrieveOriginalTool;

#[async_trait::async_trait]
impl Tool for RetrieveOriginalTool {
    fn name(&self) -> &str { "retrieve_original" }

    fn description(&self) -> &str {
        "Retrieves the original, uncompressed content for a given CCR reference ID or a file path (starts with file:// or absolute path)."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "ccr_id": {
                    "type": "string",
                    "description": "CCR reference ID (e.g. ccr_a1b2c3) or file:// path to retrieve."
                }
            },
            "required": ["ccr_id"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let input = arguments["ccr_id"].as_str().ok_or_else(|| anyhow!("Missing ccr_id parameter"))?.trim();

        if input.starts_with("file://") || input.starts_with('/') || input.contains('/') || input.contains('\\') {
            let path_str = input.trim_start_matches("file://");
            let path = Path::new(path_str);
            let absolute = if path.is_absolute() {
                path.to_path_buf()
            } else {
                std::env::current_dir().map_err(|e| anyhow!("{}", e))?.join(path)
            };
            let content = std::fs::read_to_string(&absolute)
                .map_err(|e| anyhow!("Failed to read file '{}': {}", path_str, e))?;
            Ok(json!({ "content": content, "source": "file", "path": absolute.to_string_lossy() }))
        } else {
            let conn = get_cache_connection()?;
            let now = Utc::now().to_rfc3339();
            let result: Result<String, _> = conn.query_row(
                "SELECT content FROM cache_entries WHERE ccr_id = ?1",
                params![input],
                |row| row.get(0),
            );
            match result {
                Ok(content) => {
                    let _ = conn.execute(
                        "UPDATE cache_entries SET accessed_at = ?1 WHERE ccr_id = ?2",
                        params![now, input],
                    );
                    Ok(json!({ "content": content, "source": "cache", "ccr_id": input }))
                }
                Err(_) => Err(anyhow!("CCR reference ID '{}' not found or expired.", input)),
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// Tool 4: PingTool
// ═══════════════════════════════════════════════════════════════════

pub struct PingTool;

#[async_trait::async_trait]
impl Tool for PingTool {
    fn name(&self) -> &str { "ping" }
    fn description(&self) -> &str { "Health check. Returns 'ok' if the tool is responsive." }
    fn parameters(&self) -> Value {
        json!({ "type": "object", "properties": {} })
    }
    async fn call(&self, _arguments: &Value) -> Result<Value> {
        Ok(json!({ "status": "ok" }))
    }
}

// ═══════════════════════════════════════════════════════════════════
// Tool 5: ServerInfoTool
// ═══════════════════════════════════════════════════════════════════

pub struct ServerInfoTool;

#[async_trait::async_trait]
impl Tool for ServerInfoTool {
    fn name(&self) -> &str { "server_info" }
    fn description(&self) -> &str { "Returns information about the Headroom MCP server configuration and status." }
    fn parameters(&self) -> Value {
        json!({ "type": "object", "properties": {} })
    }
    async fn call(&self, _arguments: &Value) -> Result<Value> {
        let count = get_cache_connection()
            .map(|conn| conn.query_row("SELECT COUNT(*) FROM cache_entries", [], |r| r.get::<_, i64>(0)).unwrap_or(0))
            .unwrap_or(0);
        let total_bytes = get_cache_connection()
            .map(|conn| conn.query_row("SELECT COALESCE(SUM(size_bytes), 0) FROM cache_entries", [], |r| r.get::<_, i64>(0)).unwrap_or(0))
            .unwrap_or(0);
        Ok(json!({
            "version": "0.1.0",
            "cache_size": count,
            "total_bytes": total_bytes,
            "max_input_size": MAX_INPUT_SIZE,
            "cache_capacity": CACHE_CAPACITY,
        }))
    }
}

// ═══════════════════════════════════════════════════════════════════
// Tool 6: CountTokensTool
// ═══════════════════════════════════════════════════════════════════

pub struct CountTokensTool;

#[async_trait::async_trait]
impl Tool for CountTokensTool {
    fn name(&self) -> &str { "count_tokens" }
    fn description(&self) -> &str { "Estimates the token count for a given text. Helps agents decide whether compression is needed." }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "text": { "type": "string", "description": "The text to estimate tokens for." }
            },
            "required": ["text"]
        })
    }
    async fn call(&self, arguments: &Value) -> Result<Value> {
        let text = arguments["text"].as_str().ok_or_else(|| anyhow!("Missing text parameter"))?;
        let tokens = estimate_tokens(text);
        let chars = text.chars().count();
        Ok(json!({ "tokens": tokens, "characters": chars, "estimate": format!("~{} tokens ({} characters)", tokens, chars) }))
    }
}

// ═══════════════════════════════════════════════════════════════════
// Tool 7: CacheStatsTool
// ═══════════════════════════════════════════════════════════════════

pub struct CacheStatsTool;

#[async_trait::async_trait]
impl Tool for CacheStatsTool {
    fn name(&self) -> &str { "cache_stats" }
    fn description(&self) -> &str { "Returns statistics about the context cache." }
    fn parameters(&self) -> Value {
        json!({ "type": "object", "properties": {} })
    }
    async fn call(&self, _arguments: &Value) -> Result<Value> {
        let conn = get_cache_connection()?;
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM cache_entries", [], |r| r.get(0)).unwrap_or(0);
        let total_bytes: i64 = conn.query_row("SELECT COALESCE(SUM(size_bytes), 0) FROM cache_entries", [], |r| r.get(0)).unwrap_or(0);

        let mut stmt = conn.prepare("SELECT ccr_id, size_bytes FROM cache_entries ORDER BY accessed_at DESC LIMIT 50")?;
        let items: Vec<Value> = stmt.query_map([], |row| {
            Ok(json!({
                "ccr_id": row.get::<_, String>(0)?,
                "size_bytes": row.get::<_, i64>(1)?,
            }))
        })?.filter_map(|r| r.ok()).collect();

        Ok(json!({
            "total_items": count,
            "total_bytes": total_bytes,
            "items": items,
        }))
    }
}

// ═══════════════════════════════════════════════════════════════════
// Tool 8: ClearCacheTool
// ═══════════════════════════════════════════════════════════════════

pub struct ClearCacheTool;

#[async_trait::async_trait]
impl Tool for ClearCacheTool {
    fn name(&self) -> &str { "clear_cache" }
    fn description(&self) -> &str { "Clears all cached context entries to free memory." }
    fn parameters(&self) -> Value {
        json!({ "type": "object", "properties": {} })
    }
    async fn call(&self, _arguments: &Value) -> Result<Value> {
        let conn = get_cache_connection()?;
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM cache_entries", [], |r| r.get(0)).unwrap_or(0);
        let total_bytes: i64 = conn.query_row("SELECT COALESCE(SUM(size_bytes), 0) FROM cache_entries", [], |r| r.get(0)).unwrap_or(0);
        conn.execute("DELETE FROM cache_entries", [])?;
        Ok(json!({
            "evicted": count,
            "freed_bytes": total_bytes,
            "message": format!("Successfully cleared cache. Evicted {} items (freed {} bytes).", count, total_bytes),
        }))
    }
}

// ═══════════════════════════════════════════════════════════════════
// Tool 9: SearchCacheTool
// ═══════════════════════════════════════════════════════════════════

pub struct SearchCacheTool;

#[async_trait::async_trait]
impl Tool for SearchCacheTool {
    fn name(&self) -> &str { "search_cache" }
    fn description(&self) -> &str { "Searches cached content by keyword. Returns matching CCR IDs and content snippets." }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Keyword to search for in cached content." },
                "max_results": { "type": "integer", "description": "Maximum number of results (default 10)." }
            },
            "required": ["query"]
        })
    }
    async fn call(&self, arguments: &Value) -> Result<Value> {
        let query = arguments["query"].as_str().ok_or_else(|| anyhow!("Missing query parameter"))?;
        let max_results = arguments["max_results"].as_u64().unwrap_or(10) as usize;
        let conn = get_cache_connection()?;

        let search_pattern = format!("%{}%", query.replace('%', "\\%").replace('_', "\\_"));
        let mut stmt = conn.prepare(
            "SELECT ccr_id, content FROM cache_entries WHERE content LIKE ?1 ESCAPE '\\' ORDER BY accessed_at DESC LIMIT ?2"
        )?;

        let results: Vec<Value> = stmt.query_map(params![search_pattern, max_results as i64], |row| {
            let id: String = row.get(0)?;
            let content: String = row.get(1)?;
            let snippet = if let Some(idx) = content.to_lowercase().find(&query.to_lowercase()) {
                let start = idx.saturating_sub(30);
                let end = (idx + query.len() + 50).min(content.len());
                let sub = &content[start..end];
                format!("...{}...", sub.replace('\n', " "))
            } else {
                content.chars().take(80).collect::<String>()
            };
            Ok(json!({ "ccr_id": id, "snippet": snippet }))
        })?.filter_map(|r| r.ok()).collect();

        Ok(json!({
            "query": query,
            "count": results.len(),
            "results": results,
        }))
    }
}

// ═══════════════════════════════════════════════════════════════════
// Tool 10: CacheAlignTool
// ═══════════════════════════════════════════════════════════════════

pub struct CacheAlignTool;

#[async_trait::async_trait]
impl Tool for CacheAlignTool {
    fn name(&self) -> &str { "cache_align" }
    fn description(&self) -> &str { "Aligns context chunks deterministically, padding and wrapping them to optimize KV cache hits for LLM providers." }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "chunks": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "List of text chunks to align."
                },
                "padding_size": {
                    "type": "integer",
                    "description": "Alignment modulus in bytes (default 1024)."
                }
            },
            "required": ["chunks"]
        })
    }
    async fn call(&self, arguments: &Value) -> Result<Value> {
        let chunks: Vec<String> = serde_json::from_value(arguments["chunks"].clone())
            .map_err(|_| anyhow!("Invalid chunks: expected array of strings"))?;
        let size = arguments["padding_size"].as_u64().unwrap_or(1024) as usize;
        if size == 0 {
            return Err(anyhow!("Padding size must be greater than 0"));
        }

        let mut sorted_chunks = chunks;
        let total_chunks = sorted_chunks.len();
        sorted_chunks.sort();

        let mut aligned_output = String::new();
        for chunk in sorted_chunks {
            let trimmed = chunk.trim_end();
            let mut hasher = DefaultHasher::new();
            trimmed.hash(&mut hasher);
            let hash = format!("{:016x}", hasher.finish());

            let len = trimmed.len();
            let rem = len % size;
            let pad = if rem == 0 { 0 } else { size - rem };
            let padded = format!("{}{}", trimmed, " ".repeat(pad));

            aligned_output.push_str(&format!("<!-- chunk: {} -->\n{}\n<!-- endchunk -->\n", hash, padded));
        }

        Ok(json!({ "aligned": aligned_output, "chunks": total_chunks, "padding_size": size }))
    }
}

// ═══════════════════════════════════════════════════════════════════
// Tool 11: CompressSchemaTool
// ═══════════════════════════════════════════════════════════════════

pub struct CompressSchemaTool;

#[async_trait::async_trait]
impl Tool for CompressSchemaTool {
    fn name(&self) -> &str { "compress_schema" }
    fn description(&self) -> &str { "Minifies a JSON schema representation of MCP tools, stripping descriptions and comments to save token budget." }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "schema": { "type": "string", "description": "JSON schema to minify." }
            },
            "required": ["schema"]
        })
    }
    async fn call(&self, arguments: &Value) -> Result<Value> {
        let schema = arguments["schema"].as_str().ok_or_else(|| anyhow!("Missing schema parameter"))?;
        let mut json_val: Value = serde_json::from_str(schema)
            .map_err(|e| anyhow!("Invalid JSON provided: {}", e))?;
        minify_schema_val(&mut json_val);
        let minified = serde_json::to_string(&json_val)
            .map_err(|e| anyhow!("Failed to serialize minified schema: {}", e))?;
        Ok(json!({ "original_length": schema.len(), "compressed_length": minified.len(), "schema": minified }))
    }
}

fn minify_schema_val(val: &mut Value) {
    match val {
        Value::Object(map) => {
            map.remove("description");
            map.remove("title");
            map.remove("examples");
            for (_, child) in map.iter_mut() {
                minify_schema_val(child);
            }
        }
        Value::Array(arr) => {
            for item in arr.iter_mut() {
                minify_schema_val(item);
            }
        }
        _ => {}
    }
}

// ═══════════════════════════════════════════════════════════════════
// Tool 12: CompressFileTool
// ═══════════════════════════════════════════════════════════════════

pub struct CompressFileTool;

#[async_trait::async_trait]
impl Tool for CompressFileTool {
    fn name(&self) -> &str { "compress_file" }
    fn description(&self) -> &str { "Reads a file, auto-detects its content type, compresses it, and registers a CCR reference ID." }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": { "type": "string", "description": "Path to the file to compress (absolute or relative)." },
                "content_type": { "type": "string", "enum": ["auto", "json", "code", "text_logs", "csv", "markdown", "yaml"], "description": "Content type hint." },
                "preview": { "type": "boolean", "description": "If true, returns preview without caching." }
            },
            "required": ["file_path"]
        })
    }
    async fn call(&self, arguments: &Value) -> Result<Value> {
        let path_str = arguments["file_path"].as_str().ok_or_else(|| anyhow!("Missing file_path parameter"))?;
        let path = Path::new(path_str);
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir().map_err(|e| anyhow!("Failed to get cwd: {}", e))?.join(path)
        };
        let canonical = absolute.canonicalize()
            .map_err(|e| anyhow!("Failed to resolve path '{}': {}", path_str, e))?;

        let raw_text = std::fs::read_to_string(&canonical)
            .map_err(|e| anyhow!("Failed to read file '{}': {}", path_str, e))?;

        if raw_text.len() > MAX_INPUT_SIZE {
            return Err(anyhow!("File size exceeds maximum allowed size of {} bytes", MAX_INPUT_SIZE));
        }

        let content_type = arguments["content_type"].as_str().unwrap_or("auto").to_lowercase();
        let content_type = if content_type == "auto" || content_type.is_empty() {
            detect_content_type_from_ext(&canonical).unwrap_or_else(|| auto_detect_type(&raw_text))
        } else {
            match content_type.as_str() {
                "json" | "code" | "text_logs" | "csv" | "markdown" | "yaml" => content_type.as_str(),
                other => return Err(anyhow!("Unknown content_type '{}'.", other)),
            }
        };

        let compressed = match content_type {
            "json" => context_compactor::compress_json(&raw_text).unwrap_or_else(|_| raw_text.clone()),
            "code" | "yaml" | "markdown" => context_compactor::compress_code(&raw_text),
            "csv" => compress_csv(&raw_text),
            _ => context_compactor::compress_logs(&raw_text),
        };

        let is_preview = arguments["preview"].as_bool().unwrap_or(false);
        let ccr_id = if is_preview { None } else { Some(cache_content(&raw_text)?) };

        Ok(format_ccr_result(&compressed, &raw_text, ccr_id.as_deref(), "compress_file"))
    }
}

// ═══════════════════════════════════════════════════════════════════
// Tool 13: CompressDiffTool
// ═══════════════════════════════════════════════════════════════════

pub struct CompressDiffTool;

#[async_trait::async_trait]
impl Tool for CompressDiffTool {
    fn name(&self) -> &str { "compress_diff" }
    fn description(&self) -> &str { "Compresses unified diff output into a structural summary and caches the full diff." }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "diff_text": { "type": "string", "description": "The unified diff text to compress." },
                "preview": { "type": "boolean", "description": "If true, returns preview without caching." }
            },
            "required": ["diff_text"]
        })
    }
    async fn call(&self, arguments: &Value) -> Result<Value> {
        let diff_text = arguments["diff_text"].as_str().ok_or_else(|| anyhow!("Missing diff_text parameter"))?.trim();
        if diff_text.is_empty() {
            return Ok(json!({ "compressed": "Empty diff provided.", "ccr_id": null }));
        }
        if diff_text.len() > MAX_INPUT_SIZE {
            return Err(anyhow!("Diff size exceeds maximum allowed size of {} bytes", MAX_INPUT_SIZE));
        }

        let compressed = compress_diff_text(diff_text);
        let is_preview = arguments["preview"].as_bool().unwrap_or(false);
        let ccr_id = if is_preview { None } else { Some(cache_content(diff_text)?) };

        Ok(format_ccr_result(&compressed, diff_text, ccr_id.as_deref(), "compress_diff"))
    }
}

// ═══════════════════════════════════════════════════════════════════
// Tool 14: ExportCacheTool
// ═══════════════════════════════════════════════════════════════════

pub struct ExportCacheTool;

#[async_trait::async_trait]
impl Tool for ExportCacheTool {
    fn name(&self) -> &str { "export_cache" }
    fn description(&self) -> &str { "Exports the entire cache to a JSON file for session portability." }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": { "type": "string", "description": "Path for the export JSON file." }
            },
            "required": ["file_path"]
        })
    }
    async fn call(&self, arguments: &Value) -> Result<Value> {
        let path_str = arguments["file_path"].as_str().ok_or_else(|| anyhow!("Missing file_path parameter"))?;
        let path = Path::new(path_str);
        let absolute = if path.is_absolute() { path.to_path_buf() }
            else { std::env::current_dir().map_err(|e| anyhow!("{}", e))?.join(path) };

        let conn = get_cache_connection()?;
        let mut stmt = conn.prepare("SELECT ccr_id, content, created_at FROM cache_entries")?;
        let entries: Vec<Value> = stmt.query_map([], |row| {
            Ok(json!({
                "ccr_id": row.get::<_, String>(0)?,
                "content": row.get::<_, String>(1)?,
                "created_at": row.get::<_, String>(2)?,
            }))
        })?.filter_map(|r| r.ok()).collect();

        let json_str = serde_json::to_string_pretty(&entries)?;
        std::fs::write(&absolute, json_str)
            .map_err(|e| anyhow!("Failed to write export file: {}", e))?;

        Ok(json!({ "count": entries.len(), "file_path": absolute.to_string_lossy().to_string() }))
    }
}

// ═══════════════════════════════════════════════════════════════════
// Tool 15: ImportCacheTool
// ═══════════════════════════════════════════════════════════════════

pub struct ImportCacheTool;

#[async_trait::async_trait]
impl Tool for ImportCacheTool {
    fn name(&self) -> &str { "import_cache" }
    fn description(&self) -> &str { "Imports cached entries from a previously exported JSON file." }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": { "type": "string", "description": "Path to the JSON export file." }
            },
            "required": ["file_path"]
        })
    }
    async fn call(&self, arguments: &Value) -> Result<Value> {
        let path_str = arguments["file_path"].as_str().ok_or_else(|| anyhow!("Missing file_path parameter"))?;
        let path = Path::new(path_str);
        let absolute = if path.is_absolute() { path.to_path_buf() }
            else { std::env::current_dir().map_err(|e| anyhow!("{}", e))?.join(path) };

        let json_str = std::fs::read_to_string(&absolute)
            .map_err(|e| anyhow!("Failed to read import file: {}", e))?;

        let entries: Vec<Value> = serde_json::from_str(&json_str)
            .map_err(|e| anyhow!("Invalid JSON format: {}", e))?;

        let conn = get_cache_connection()?;
        let mut count = 0i64;
        for entry in &entries {
            let id = entry["ccr_id"].as_str().unwrap_or("");
            let content = entry["content"].as_str().unwrap_or("");
            let created_at = entry["created_at"].as_str().unwrap_or("");
            if !id.is_empty() && !content.is_empty() {
                let now = Utc::now().to_rfc3339();
                let _ = conn.execute(
                    "INSERT OR REPLACE INTO cache_entries (ccr_id, content, created_at, accessed_at, size_bytes) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![id, content, created_at, now, content.len() as i64],
                );
                count += 1;
            }
        }

        Ok(json!({ "imported": count, "file_path": absolute.to_string_lossy().to_string() }))
    }
}

// ═══════════════════════════════════════════════════════════════════
// Tool 16: CompressUrlTool
// ═══════════════════════════════════════════════════════════════════

pub struct CompressUrlTool;

#[async_trait::async_trait]
impl Tool for CompressUrlTool {
    fn name(&self) -> &str { "compress_url" }
    fn description(&self) -> &str { "Fetches a URL, extracts its text content, and returns a compressed summary with a CCR reference." }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": { "type": "string", "description": "URL to fetch and compress." }
            },
            "required": ["url"]
        })
    }
    async fn call(&self, arguments: &Value) -> Result<Value> {
        let url = arguments["url"].as_str().ok_or_else(|| anyhow!("Missing url parameter"))?;

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| anyhow!("Failed to build HTTP client: {}", e))?;

        let res = client
            .get(url)
            .header("User-Agent", "openz-headroom/0.1.0")
            .send()
            .await
            .map_err(|e| anyhow!("Failed to fetch URL: {}", e))?;

        if let Some(content_length) = res.content_length() {
            if content_length as usize > MAX_INPUT_SIZE {
                return Err(anyhow!("URL content size ({} bytes) exceeds maximum allowed size of {} bytes", content_length, MAX_INPUT_SIZE));
            }
        }

        let content_type = res.headers().get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_lowercase();

        let raw_text = res.text().await
            .map_err(|e| anyhow!("Failed to read response body: {}", e))?;

        if raw_text.len() > MAX_INPUT_SIZE {
            return Err(anyhow!("Fetched content size ({} bytes) exceeds maximum allowed size of {} bytes", raw_text.len(), MAX_INPUT_SIZE));
        }

        let trimmed = raw_text.trim();
        if trimmed.is_empty() {
            return Ok(json!({ "compressed": "URL returned empty content.", "ccr_id": null }));
        }

        let (compressed, derived_type) = if content_type.contains("html") {
            let md = html2md::parse_html(trimmed);
            (context_compactor::compress_code(&md), "markdown")
        } else if content_type.contains("json") {
            (context_compactor::compress_json(trimmed).unwrap_or_else(|_| trimmed.to_string()), "json")
        } else {
            let detected = auto_detect_type(trimmed);
            let comp = match detected {
                "json" => context_compactor::compress_json(trimmed).unwrap_or_else(|_| trimmed.to_string()),
                "code" | "markdown" => context_compactor::compress_code(trimmed),
                "csv" => compress_csv(trimmed),
                _ => context_compactor::compress_logs(trimmed),
            };
            (comp, detected)
        };

        let ccr_id = cache_content(trimmed)?;
        let mut result = format_ccr_result(&compressed, trimmed, Some(&ccr_id), "compress_url");
        result["url"] = json!(url);
        result["content_type"] = json!(derived_type);
        Ok(result)
    }
}

// ═══════════════════════════════════════════════════════════════════
// Tool 17: RunAndCompressTool
// ═══════════════════════════════════════════════════════════════════

pub struct RunAndCompressTool;

#[async_trait::async_trait]
impl Tool for RunAndCompressTool {
    fn name(&self) -> &str { "run_and_compress" }
    fn description(&self) -> &str { "Executes a shell command and returns its compressed output with a CCR reference." }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": { "type": "string", "description": "Command to execute." },
                "args": { "type": "array", "items": { "type": "string" }, "description": "Command arguments." }
            },
            "required": ["command"]
        })
    }
    async fn call(&self, arguments: &Value) -> Result<Value> {
        let command = arguments["command"].as_str().ok_or_else(|| anyhow!("Missing command parameter"))?;
        let args: Vec<String> = serde_json::from_value(arguments["args"].clone()).unwrap_or_default();

        let mut cmd = tokio::process::Command::new(command);
        cmd.env("PAGER", "cat");
        if !args.is_empty() {
            cmd.args(&args);
        }

        let output = cmd.output().await
            .map_err(|e| anyhow!("Failed to execute command '{}': {}", command, e))?;

        let stdout_str = String::from_utf8_lossy(&output.stdout);
        let stderr_str = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{}{}", stdout_str, stderr_str);
        let trimmed = combined.trim();

        if trimmed.is_empty() {
            return Ok(json!({
                "compressed": format!("Command exited with status {}. No output was produced.", output.status),
                "exit_code": output.status.code().unwrap_or(-1),
                "ccr_id": null,
            }));
        }

        let filtered = filter_command_output(command, trimmed);
        let compressed = context_compactor::compress_logs(&filtered);

        let ccr_id = cache_content(trimmed)?;
        let mut result = format_ccr_result(&compressed, trimmed, Some(&ccr_id), "run_and_compress");
        result["exit_code"] = json!(output.status.code().unwrap_or(-1));
        result["command"] = json!(command);
        Ok(result)
    }
}

// ═══════════════════════════════════════════════════════════════════
// Tool 18: CompressDirectoryTool
// ═══════════════════════════════════════════════════════════════════

pub struct CompressDirectoryTool;

#[async_trait::async_trait]
impl Tool for CompressDirectoryTool {
    fn name(&self) -> &str { "compress_directory" }
    fn description(&self) -> &str { "Recursively walks a directory, compresses each matching file, and registers CCR reference tokens." }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "dir_path": { "type": "string", "description": "Directory path to walk." },
                "extensions": { "type": "array", "items": { "type": "string" }, "description": "Only process files with these extensions (e.g. [\"rs\", \"toml\"])." },
                "max_depth": { "type": "integer", "description": "Maximum directory depth (default 4)." },
                "preview": { "type": "boolean", "description": "If true, returns preview without caching." }
            },
            "required": ["dir_path"]
        })
    }
    async fn call(&self, arguments: &Value) -> Result<Value> {
        let dir_path_str = arguments["dir_path"].as_str().ok_or_else(|| anyhow!("Missing dir_path parameter"))?;
        let root = Path::new(dir_path_str);
        let resolved_root = if root.is_absolute() {
            root.to_path_buf()
        } else {
            std::env::current_dir().map_err(|e| anyhow!("{}", e))?.join(root)
        };
        let resolved_root = resolved_root.canonicalize()
            .map_err(|e| anyhow!("Failed to resolve path '{}': {}", dir_path_str, e))?;

        if !resolved_root.is_dir() {
            return Err(anyhow!("'{}' is not a directory", dir_path_str));
        }

        let extensions: Vec<String> = serde_json::from_value(arguments["extensions"].clone()).unwrap_or_default();
        let max_depth = arguments["max_depth"].as_u64().unwrap_or(4) as usize;
        let is_preview = arguments["preview"].as_bool().unwrap_or(false);
        let file_limit = 500usize;

        let mut tree_root = CompTreeNode {
            name: resolved_root.file_name().and_then(|n| n.to_str()).unwrap_or(".").to_string(),
            is_dir: true, files_count: 0, file_info: None, children: BTreeMap::new(),
        };

        let mut processed = 0usize;
        let mut walk_dir = Vec::new();
        walk_dir.push((resolved_root.clone(), 0usize));

        while let Some((dir_path, depth)) = walk_dir.pop() {
            if depth > max_depth { continue; }
            if processed >= file_limit { break; }

            let entries = match std::fs::read_dir(&dir_path) {
                Ok(e) => e,
                Err(_) => continue,
            };

            let mut dirs = Vec::new();
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.components().any(|c| c.as_os_str() == ".git") { continue; }

                if path.is_dir() {
                    if depth < max_depth {
                        dirs.push(path);
                    }
                    continue;
                }

                if !extensions.is_empty() {
                    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                    if !extensions.iter().any(|f| f.to_lowercase() == ext) { continue; }
                }

                if is_binary_file(&path) { continue; }

                let raw_text = match std::fs::read_to_string(&path) {
                    Ok(c) => c,
                    Err(_) => continue,
                };

                if raw_text.len() > MAX_INPUT_SIZE { continue; }

                let content_type = detect_content_type_from_ext(&path).unwrap_or_else(|| auto_detect_type(&raw_text));
                let compressed = match content_type {
                    "json" => context_compactor::compress_json(&raw_text).unwrap_or_else(|_| raw_text.clone()),
                    "code" | "yaml" | "markdown" => context_compactor::compress_code(&raw_text),
                    "csv" => compress_csv(&raw_text),
                    _ => context_compactor::compress_logs(&raw_text),
                };

                let ccr_id = if is_preview { "PREVIEW".to_string() } else { cache_content(&raw_text)? };
                let original_tokens = estimate_tokens(&raw_text);
                let compressed_tokens = estimate_tokens(&compressed);
                let saved_pct = if original_tokens > 0 {
                    format!("{:.1}%", ((original_tokens as f64 - compressed_tokens as f64) / original_tokens as f64 * 100.0).max(0.0))
                } else { "0.0%".to_string() };

                let file_info = CompTreeFile { ccr_id, original_tokens, compressed_tokens, saved_pct };

                if let Ok(rel_path) = path.strip_prefix(&resolved_root) {
                    let parts: Vec<String> = rel_path.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
                    tree_root.insert(&parts, file_info);
                }

                processed += 1;
                if processed >= file_limit { break; }
            }

            // Add subdirectories (reverse to maintain order)
            for d in dirs.into_iter().rev() {
                walk_dir.push((d, depth + 1));
            }
        }

        tree_root.update_counts();
        let tree_str = tree_root.format_tree("", true, 0, max_depth);

        let preview_label = if is_preview { " [PREVIEW - not cached]" } else { "" };
        let suffix = if processed >= file_limit {
            format!("\nWarning: Walk stopped early because file count limit ({}) was reached.", file_limit)
        } else { String::new() };

        Ok(json!({
            "summary": format!("Compressed directory: {} ({} files processed){}", dir_path_str, processed, preview_label),
            "tree": tree_str,
            "files_processed": processed,
            "note": suffix,
        }))
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

// ═══════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::OnceLock;

    static TEST_LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    fn test_lock() -> &'static tokio::sync::Mutex<()> {
        TEST_LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
    }

    #[tokio::test]
    async fn test_auto_detect_json() {
        assert_eq!(auto_detect_type(r#"{"key": "value"}"#), "json");
        assert_eq!(auto_detect_type("[1, 2, 3]"), "json");
    }

    #[tokio::test]
    async fn test_auto_detect_code() {
        assert_eq!(auto_detect_type("fn main() { println!(\"hi\"); }"), "code");
        assert_eq!(auto_detect_type("def hello():\n    pass"), "code");
    }

    #[tokio::test]
    async fn test_estimate_tokens_basic() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("abcd"), 1);
        assert_eq!(estimate_tokens("abcdefgh"), 2);
    }

    #[tokio::test]
    async fn test_compress_content_preview() {
        let tool = CompressContentTool;
        let res = tool.call(&json!({
            "raw_text": "fn hello() { println!(\"world\"); }",
            "content_type": "code",
            "preview": true
        })).await.unwrap();
        assert!(res["compressed"].as_str().unwrap().contains("hello"));
        assert!(res["ccr_id"].is_null());
    }

    #[tokio::test]
    async fn test_compress_content_then_retrieve() {
        let _l = test_lock().lock().await;

        let tool_c = CompressContentTool;
        let res = tool_c.call(&json!({
            "raw_text": "This is a test string for CCR round-trip verification.",
            "content_type": "text_logs",
            "preview": false
        })).await.unwrap();

        let ccr_id = res["ccr_id"].as_str().unwrap().to_string();
        assert!(ccr_id.starts_with("ccr_"));
        assert!(res["compressed_tokens"].as_u64().unwrap() > 0);

        let tool_r = RetrieveOriginalTool;
        let res2 = tool_r.call(&json!({ "ccr_id": ccr_id })).await.unwrap();
        assert_eq!(res2["content"].as_str().unwrap(), "This is a test string for CCR round-trip verification.");
        assert_eq!(res2["source"], "cache");
    }

    #[tokio::test]
    async fn test_retrieve_original_missing() {
        let tool = RetrieveOriginalTool;
        let res = tool.call(&json!({ "ccr_id": "ccr_nonexistent_123" })).await;
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn test_compress_json_content() {
        let tool = CompressContentTool;
        let json_input = r#"{"name":"test","items":[1,2,3],"nested":{"key":"val"}}"#;
        let res = tool.call(&json!({ "raw_text": json_input, "content_type": "json", "preview": true })).await.unwrap();
        let compressed = res["compressed"].as_str().unwrap();
        assert!(compressed.contains("name") || compressed.contains("items"));
    }

    #[tokio::test]
    async fn test_ping() {
        let tool = PingTool;
        let res = tool.call(&json!({})).await.unwrap();
        assert_eq!(res["status"], "ok");
    }

    #[tokio::test]
    async fn test_server_info() {
        let tool = ServerInfoTool;
        let res = tool.call(&json!({})).await.unwrap();
        assert!(res["cache_size"].as_u64().is_some());
        assert_eq!(res["cache_capacity"], CACHE_CAPACITY);
    }

    #[tokio::test]
    async fn test_count_tokens() {
        let tool = CountTokensTool;
        let res = tool.call(&json!({ "text": "hello world" })).await.unwrap();
        assert_eq!(res["tokens"], 3);
        assert_eq!(res["characters"], 11);
    }

    #[tokio::test]
    async fn test_cache_clear_and_stats() {
        let _l = test_lock().lock().await;

        // First insert something
        let tool_c = CompressContentTool;
        let _ = tool_c.call(&json!({
            "raw_text": "cache test data for stats",
            "content_type": "text_logs",
            "preview": false
        })).await.unwrap();

        // Check stats
        let stats = CacheStatsTool;
        let res = stats.call(&json!({})).await.unwrap();
        assert!(res["total_items"].as_u64().unwrap() > 0);

        // Clear
        let clear = ClearCacheTool;
        let res2 = clear.call(&json!({})).await.unwrap();
        assert!(res2["evicted"].as_u64().unwrap() > 0);
    }

    #[tokio::test]
    async fn test_search_cache() {
        let _l = test_lock().lock().await;

        let tool_c = CompressContentTool;
        let _ = tool_c.call(&json!({
            "raw_text": "unique_search_term_for_testing_12345",
            "content_type": "text_logs",
            "preview": false
        })).await.unwrap();

        let search = SearchCacheTool;
        let res = search.call(&json!({ "query": "unique_search_term" })).await.unwrap();
        assert!(res["count"].as_u64().unwrap() > 0);
    }

    #[tokio::test]
    async fn test_cache_align() {
        let tool = CacheAlignTool;
        let res = tool.call(&json!({
            "chunks": ["chunk b", "chunk a"],
            "padding_size": 16
        })).await.unwrap();

        let aligned = res["aligned"].as_str().unwrap();
        assert!(aligned.find("chunk a").unwrap() < aligned.find("chunk b").unwrap());
        assert!(aligned.contains("<!-- chunk: "));
    }

    #[tokio::test]
    async fn test_compress_schema() {
        let tool = CompressSchemaTool;
        let schema = r#"{ "title": "Test", "description": "A test tool", "properties": { "name": { "type": "string", "description": "Name" } } }"#;
        let res = tool.call(&json!({ "schema": schema })).await.unwrap();
        let compressed = res["schema"].as_str().unwrap();
        assert!(!compressed.contains("description"));
        assert!(!compressed.contains("title"));
        assert!(compressed.contains("name"));
    }

    #[tokio::test]
    async fn test_compress_diff() {
        let tool = CompressDiffTool;
        let diff = r#"diff --git a/src/server.rs b/src/server.rs
--- a/src/server.rs
+++ b/src/server.rs
@@ -10,3 +10,4 @@ fn my_func()
-old
+new
"#;
        let res = tool.call(&json!({ "diff_text": diff, "preview": true })).await.unwrap();
        let compressed = res["compressed"].as_str().unwrap();
        assert!(compressed.contains("Diff Summary"));
        assert!(compressed.contains("src/server.rs"));
    }

    #[tokio::test]
    async fn test_compress_file() {
        let _l = test_lock().lock().await;
        let dir = std::env::temp_dir().join("headroom_test_compress_file");
        std::fs::create_dir_all(&dir).unwrap();
        let file_path = dir.join("test_hello.rs");
        std::fs::write(&file_path, "fn test() { println!(\"hello\"); }").unwrap();

        let tool = CompressFileTool;
        let res = tool.call(&json!({
            "file_path": file_path.to_string_lossy(),
            "content_type": "code",
            "preview": true
        })).await.unwrap();
        assert!(res["compressed"].as_str().unwrap().contains("test"));

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn test_export_import_cache() {
        let _l = test_lock().lock().await;

        // Insert test data
        let cid = cache_content("export_test_data").unwrap();
        assert!(cid.starts_with("ccr_"));

        let dir = std::env::temp_dir().join("headroom_test_export");
        std::fs::create_dir_all(&dir).unwrap();
        let export_path = dir.join("cache_export.json");

        // Export
        let export = ExportCacheTool;
        let res = export.call(&json!({ "file_path": export_path.to_string_lossy() })).await.unwrap();
        assert!(res["count"].as_u64().unwrap() > 0);

        // Clear cache
        let clear = ClearCacheTool;
        let _ = clear.call(&json!({})).await.unwrap();

        // Import
        let import = ImportCacheTool;
        let res2 = import.call(&json!({ "file_path": export_path.to_string_lossy() })).await.unwrap();
        assert!(res2["imported"].as_u64().unwrap() > 0);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn test_detect_content_type_from_ext() {
        assert_eq!(detect_content_type_from_ext(Path::new("test.rs")), Some("code"));
        assert_eq!(detect_content_type_from_ext(Path::new("data.json")), Some("json"));
        assert_eq!(detect_content_type_from_ext(Path::new("doc.md")), Some("markdown"));
        assert_eq!(detect_content_type_from_ext(Path::new("unknown.xyz")), None);
    }

    #[tokio::test]
    async fn test_detect_project_type() {
        let dir = std::env::temp_dir().join("headroom_test_projtype");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Cargo.toml"), "").unwrap();
        assert_eq!(detect_project_type(&dir), "Rust");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn test_summarize_codebase() {
        let _l = test_lock().lock().await;
        let dir = std::env::temp_dir().join("headroom_test_codebase_summary");
        let src = dir.join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(dir.join("Cargo.toml"), "[package]").unwrap();
        std::fs::write(src.join("main.rs"), "fn main() {\n    println!(\"hello\");\n}").unwrap();

        let tool = SummarizeCodebaseTool;
        let res = tool.call(&json!({ "root_path": dir.to_string_lossy() })).await.unwrap();
        assert_eq!(res["project_type"], "Rust");
        assert!(res["total_files"].as_u64().unwrap() >= 2);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn test_compress_csv_content() {
        let csv = "name,age,city\nAlice,30,NYC\nBob,25,LA\nCharlie,35,SF\nDiana,28,Chicago";
        let result = compress_csv(csv);
        assert!(result.contains("Headers: name,age,city"));
        assert!(result.contains("Row 1:"));
        assert!(result.contains("4 rows total"));
    }

    #[tokio::test]
    async fn test_detect_project_type_variants() {
        let dir = std::env::temp_dir().join("headroom_test_projvar");
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(detect_project_type(&dir), "Unknown");
        std::fs::write(dir.join("package.json"), "{}").unwrap();
        assert_eq!(detect_project_type(&dir), "Node.js");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn test_is_binary_file() {
        let dir = std::env::temp_dir().join("headroom_test_binary");
        std::fs::create_dir_all(&dir).unwrap();
        let bin = dir.join("test.bin");
        std::fs::write(&bin, b"Hello \x00 world").unwrap();
        assert!(is_binary_file(&bin));
        std::fs::write(&bin, b"Hello world").unwrap();
        assert!(!is_binary_file(&bin));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn test_parse_simple_git_diff() {
        let diff = r#"diff --git a/src/server.rs b/src/server.rs
--- a/src/server.rs
+++ b/src/server.rs
@@ -10,3 +10,4 @@ fn my_func()
 line1
 line2
-old_line
+new_line
"#;
        let summary = parse_unified_diff(diff);
        assert_eq!(summary.files.len(), 1);
        assert_eq!(summary.files[0].path, "src/server.rs");
        assert_eq!(summary.files[0].insertions, 1);
        assert_eq!(summary.files[0].deletions, 1);
        assert_eq!(summary.files[0].hunks_count, 1);
        assert!(!summary.files[0].is_binary);
    }

    #[tokio::test]
    async fn test_filter_cargo_output() {
        let raw = "Compiling foo v0.1.0\nwarning: unused variable\nwarning: another warning\nwarning: third\nwarning: fourth\nwarning: fifth\nwarning: sixth\nFinished\n";
        let filtered = filter_cargo_output(raw);
        assert!(!filtered.contains("Compiling foo"));
        assert!(filtered.contains("warning: unused variable"));
        assert!(filtered.contains("more warnings omitted"));
    }

    #[tokio::test]
    async fn test_filter_git_output() {
        let raw = "Enumerating objects: 5\nCounting objects: 100%\nCompressing objects: 100%\nSome real output\n";
        let filtered = filter_git_output(raw);
        assert!(!filtered.contains("Enumerating objects:"));
        assert!(filtered.contains("Some real output"));
    }

    #[tokio::test]
    async fn test_filter_python_output() {
        let raw = "Collecting requests\nDownloading requests-2.28.0-py3-none-any.whl\nreal output here\n";
        let filtered = filter_python_output(raw);
        assert!(!filtered.contains("Collecting requests"));
        assert!(filtered.contains("real output here"));
    }

    #[tokio::test]
    async fn test_scope_context_yagni_enabled() {
        let _l = test_lock().lock().await;
        std::env::set_var("HEADROOM_ENFORCE_YAGNI", "true");
        let temp_dir = std::env::temp_dir().join("headroom_test_yagni_enabled");
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::fs::write(temp_dir.join("AGENTS.md"), "test content").unwrap();

        let req = json!({ "target_path": temp_dir.to_str().unwrap() });
        let res = ScopeContextTool.call(&req).await.unwrap();
        let content = res["content"].as_str().unwrap();
        assert!(content.contains("YAGNI Minimalism Directives"));

        let _ = std::fs::remove_dir_all(&temp_dir);
        std::env::remove_var("HEADROOM_ENFORCE_YAGNI");
    }

    #[tokio::test]
    async fn test_scope_context_yagni_disabled() {
        let _l = test_lock().lock().await;
        std::env::remove_var("HEADROOM_ENFORCE_YAGNI");
        let temp_dir = std::env::temp_dir().join("headroom_test_yagni_disabled");
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::fs::write(temp_dir.join("AGENTS.md"), "test content").unwrap();

        let req = json!({ "target_path": temp_dir.to_str().unwrap() });
        let res = ScopeContextTool.call(&req).await.unwrap();
        let content = res["content"].as_str().unwrap();
        assert!(!content.contains("YAGNI"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
