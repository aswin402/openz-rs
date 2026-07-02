use crate::agent::context_compactor;
use crate::tools::Tool;
use anyhow::{anyhow, Result};
use chrono::Utc;
use rusqlite::params;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use super::{estimate_tokens, MAX_INPUT_SIZE};
use super::cache::{cache_content, get_cache_connection, generate_ccr_id, evict_lru_if_needed};

// ─── Diff structures ─────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct DiffFile {
    pub path: String,
    pub insertions: usize,
    pub deletions: usize,
    pub hunks_count: usize,
    pub is_binary: bool,
    pub is_new: bool,
    pub is_deleted: bool,
    pub contexts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DiffSummary {
    pub files: Vec<DiffFile>,
    pub total_insertions: usize,
    pub total_deletions: usize,
}

// ─── Tree structures for compress_directory ─────────────────────

#[derive(Debug, Clone)]
struct CompTreeFile {
    #[allow(dead_code)]
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

// ─── Helpers ─────────────────────────────────────────────────────

pub fn auto_detect_type(text: &str) -> &'static str {
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

pub fn detect_content_type_from_ext(path: &Path) -> Option<&'static str> {
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

pub fn is_binary_file(path: &Path) -> bool {
    if let Ok(content) = std::fs::read(path) {
        content.iter().take(4096).any(|&b| b == 0x00)
    } else {
        false
    }
}

pub fn compress_csv(raw_csv: &str) -> String {
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

pub fn format_ccr_result(compressed: &str, raw_text: &str, ccr_id: Option<&str>, tool: &str) -> Value {
    let original_tokens = estimate_tokens(raw_text);
    let compressed_tokens = estimate_tokens(compressed);
    let saved_pct = if original_tokens > 0 {
        format!("{:.1}%", ((original_tokens as f64 - compressed_tokens as f64) / original_tokens as f64 * 100.0).max(0.0))
    } else {
        "0.0%".to_string()
    };
    format_ccr_result_detailed(compressed, raw_text, ccr_id, tool, &saved_pct, original_tokens, compressed_tokens)
}

pub fn format_ccr_result_detailed(compressed: &str, _raw_text: &str, ccr_id: Option<&str>, _tool: &str, saved_pct: &str, original_tokens: usize, compressed_tokens: usize) -> Value {
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

// ─── Diff helpers ────────────────────────────────────────────────

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

pub fn parse_unified_diff(text: &str) -> DiffSummary {
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

pub fn compress_diff_text(diff_text: &str) -> String {
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

// ─── Command filtering helpers ───────────────────────────────────

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

pub fn filter_cargo_output(raw: &str) -> String {
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

pub fn filter_git_output(raw: &str) -> String {
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

pub fn filter_python_output(raw: &str) -> String {
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

// ─── Requested wrappers ──────────────────────────────────────────

pub fn compress_csv_content(raw_csv: &str) -> String {
    compress_csv(raw_csv)
}

pub fn compress_json_content(raw_json: &str) -> String {
    context_compactor::compress_json(raw_json).unwrap_or_else(|_| raw_json.to_string())
}

pub fn auto_detect_code(text: &str) -> bool {
    auto_detect_type(text) == "code"
}

pub fn auto_detect_json(text: &str) -> bool {
    auto_detect_type(text) == "json"
}

pub fn parse_simple_git_diff(text: &str) -> DiffSummary {
    parse_unified_diff(text)
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
