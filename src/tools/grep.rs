use crate::tools::Tool;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

const SKIPPED_DIRS: &[&str] = &[
    "target",
    "node_modules",
    "build",
    "dist",
    "vendor",
    "bin",
    "obj",
];

const BINARY_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "ico", "pdf", "zip", "tar", "gz", "7z", "rar", "exe", "dll", "so",
    "dylib", "db", "sqlite", "wasm", "woff", "woff2", "ttf", "eot", "mp4", "mp3", "wav", "avi",
    "mov", "bin", "out", "o", "a",
];

#[derive(Clone, Copy)]
enum PatternAtom {
    Any,
    Digit,
    Whitespace,
    Word,
    Literal(char),
}

#[derive(Clone, Copy)]
enum Quantifier {
    Once,
    ZeroOrOne,
    ZeroOrMore,
    OneOrMore,
}

#[derive(Clone, Copy)]
struct PatternStep {
    atom: PatternAtom,
    quantifier: Quantifier,
}

impl PatternAtom {
    fn matches(self, ch: char) -> bool {
        match self {
            Self::Any => ch != '\n',
            Self::Digit => ch.is_ascii_digit(),
            Self::Whitespace => ch.is_whitespace(),
            Self::Word => ch.is_ascii_alphanumeric() || ch == '_',
            Self::Literal(expected) => ch == expected,
        }
    }
}

fn compile_pattern_steps(pattern: &str) -> Vec<PatternStep> {
    let chars: Vec<char> = pattern.chars().collect();
    let mut steps = Vec::new();
    let mut idx = 0usize;
    while idx < chars.len() {
        let (atom, next_idx) = match chars[idx] {
            '\\' => match chars.get(idx + 1).copied() {
                Some('d') => (PatternAtom::Digit, idx + 2),
                Some('s') => (PatternAtom::Whitespace, idx + 2),
                Some('w') => (PatternAtom::Word, idx + 2),
                Some(escaped) => (PatternAtom::Literal(escaped), idx + 2),
                None => (PatternAtom::Literal('\\'), idx + 1),
            },
            '.' => (PatternAtom::Any, idx + 1),
            ch => (PatternAtom::Literal(ch), idx + 1),
        };
        let (quantifier, after_quant) = match chars.get(next_idx).copied() {
            Some('?') => (Quantifier::ZeroOrOne, next_idx + 1),
            Some('*') => (Quantifier::ZeroOrMore, next_idx + 1),
            Some('+') => (Quantifier::OneOrMore, next_idx + 1),
            _ => (Quantifier::Once, next_idx),
        };
        steps.push(PatternStep { atom, quantifier });
        idx = after_quant;
    }
    steps
}

fn match_steps_at(steps: &[PatternStep], chars: &[char]) -> bool {
    match steps.first().copied() {
        None => true,
        Some(step) => match step.quantifier {
            Quantifier::Once => chars
                .first()
                .copied()
                .map(|ch| step.atom.matches(ch) && match_steps_at(&steps[1..], &chars[1..]))
                .unwrap_or(false),
            Quantifier::ZeroOrOne => {
                let consumed = chars
                    .first()
                    .copied()
                    .map(|ch| step.atom.matches(ch) && match_steps_at(&steps[1..], &chars[1..]))
                    .unwrap_or(false);
                consumed || match_steps_at(&steps[1..], chars)
            }
            Quantifier::ZeroOrMore => {
                let consumed = chars
                    .first()
                    .copied()
                    .map(|ch| step.atom.matches(ch) && match_steps_at(steps, &chars[1..]))
                    .unwrap_or(false);
                consumed || match_steps_at(&steps[1..], chars)
            }
            Quantifier::OneOrMore => chars
                .first()
                .copied()
                .map(|ch| {
                    step.atom.matches(ch)
                        && (match_steps_at(steps, &chars[1..])
                            || match_steps_at(&steps[1..], &chars[1..]))
                })
                .unwrap_or(false),
        },
    }
}

fn line_matches_pattern(line: &str, query: &str, is_regex: bool) -> bool {
    match is_regex {
        false => line.contains(query),
        true => {
            let steps = compile_pattern_steps(query);
            let chars: Vec<char> = line.chars().collect();
            (0..=chars.len()).any(|start| match_steps_at(&steps, &chars[start..]))
        }
    }
}

pub struct GrepSearchTool;

impl GrepSearchTool {
    fn find_git_root(start: &Path) -> Option<PathBuf> {
        let mut current = match start.is_file() {
            true => start.parent()?.to_path_buf(),
            false => start.to_path_buf(),
        };

        loop {
            match current.join(".git").exists() {
                true => return Some(current),
                false => match current.pop() {
                    true => {}
                    false => return None,
                },
            }
        }
    }

    fn scoped_search_dir(requested: PathBuf) -> PathBuf {
        let workspace = crate::config::loader::resolve_path(".");
        match Self::find_git_root(&workspace) {
            None => requested,
            Some(repo_root) => {
                let requested = requested.canonicalize().unwrap_or(requested);
                let repo_root = repo_root.canonicalize().unwrap_or(repo_root);
                match repo_root.starts_with(&requested) && requested != repo_root {
                    true => repo_root,
                    false => requested,
                }
            }
        }
    }

    fn should_skip(path: &Path) -> bool {
        path.file_name()
            .and_then(|s| s.to_str())
            .map(|name| {
                (name.starts_with('.') && name != "." && name != "..")
                    || SKIPPED_DIRS.contains(&name)
            })
            .unwrap_or(false)
    }

    fn is_binary_file(path: &Path) -> bool {
        path.extension()
            .and_then(|s| s.to_str())
            .map(|ext| BINARY_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
            .unwrap_or(false)
    }

    fn search_single_file(
        file_path: &Path,
        query: &str,
        is_regex: bool,
        results: &mut Vec<Value>,
        limit: usize,
    ) {
        match (!Self::is_binary_file(file_path), fs::read_to_string(file_path)) {
            (true, Ok(content)) => {
                for (idx, line) in content.lines().enumerate() {
                    match results.len() < limit && line_matches_pattern(line, query, is_regex) {
                        true => {
                            results.push(json!({
                                "file": file_path.to_string_lossy(),
                                "line": idx + 1,
                                "content": line.trim()
                            }));
                        }
                        false => {}
                    }
                }
            }
            _ => {}
        }
    }

    fn walk_and_search(
        dir: &Path,
        query: &str,
        is_regex: bool,
        results: &mut Vec<Value>,
        limit: usize,
    ) -> Result<()> {
        match results.len() >= limit || Self::should_skip(dir) {
            true => Ok(()),
            false => match (dir.is_file(), dir.is_dir()) {
                (true, _) => {
                    Self::search_single_file(dir, query, is_regex, results, limit);
                    Ok(())
                }
                (false, true) => {
                    match fs::read_dir(dir) {
                        Ok(entries) => {
                            for entry in entries.flatten() {
                                match results.len() < limit {
                                    true => {
                                        Self::walk_and_search(
                                            &entry.path(),
                                            query,
                                            is_regex,
                                            results,
                                            limit,
                                        )?;
                                    }
                                    false => break,
                                }
                            }
                        }
                        Err(_) => {}
                    }
                    Ok(())
                }
                (false, false) => Ok(()),
            },
        }
    }

    fn parse_ripgrep_match_line(line: &str) -> Option<Value> {
        let val = serde_json::from_str::<Value>(line).ok()?;
        (val.get("type").and_then(|t| t.as_str()) == Some("match")).then_some(())?;
        let data = val.get("data")?;
        let file = data
            .get("path")
            .and_then(|p| p.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or_default()
            .to_string();
        let line_num = data
            .get("line_number")
            .and_then(|l| l.as_u64())
            .unwrap_or(0);
        let content = data
            .get("lines")
            .and_then(|l| l.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or_default()
            .trim_end_matches('\n')
            .trim()
            .to_string();
        Some(json!({
            "file": file,
            "line": line_num,
            "content": content
        }))
    }

    async fn run_ripgrep(
        &self,
        dir: &Path,
        query: &str,
        is_regex: bool,
        limit: usize,
    ) -> Result<Value> {
        let mut cmd = tokio::process::Command::new("rg");
        cmd.arg("--json");

        for glob_exclusion in ["!target", "!node_modules", "!build", "!dist", "!.git"] {
            cmd.arg("--glob").arg(glob_exclusion);
        }

        match is_regex {
            false => {
                cmd.arg("-F");
            }
            true => {}
        }

        cmd.arg("--");
        cmd.arg(query);
        cmd.arg(dir);

        let output = cmd.output().await?;
        match output.status.success() || output.status.code() == Some(1) {
            false => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(anyhow!("ripgrep failed: {}", stderr))
            }
            true => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let results: Vec<Value> = stdout
                    .lines()
                    .filter_map(Self::parse_ripgrep_match_line)
                    .take(limit)
                    .collect();

                Ok(json!({
                    "status": "success",
                    "results": results,
                    "capped": results.len() >= limit
                }))
            }
        }
    }
}

#[async_trait::async_trait]
impl Tool for GrepSearchTool {
    fn name(&self) -> &str {
        "grep_search"
    }

    fn description(&self) -> &str {
        "Perform a high-speed text search across the codebase. Returns matching files, line numbers, and matching line contents."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search term or regex pattern to look for inside files."
                },
                "is_regex": {
                    "type": "boolean",
                    "description": "Whether to treat the query as a regular expression. Defaults to false."
                },
                "dir": {
                    "type": "string",
                    "description": "The directory path to search in (defaults to current directory '.')."
                }
            },
            "required": ["query"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let query = arguments
            .get("query")
            .or_else(|| arguments.get("pattern"))
            .or_else(|| arguments.get("search"))
            .or_else(|| arguments.get("term"))
            .or_else(|| arguments.get("text"))
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| anyhow!("Missing 'query' parameter"))?;

        let is_regex = arguments
            .get("is_regex")
            .or_else(|| arguments.get("regex"))
            .and_then(|v| {
                v.as_bool().or_else(|| {
                    v.as_str().map(|s| s.eq_ignore_ascii_case("true") || s == "1")
                })
            })
            .unwrap_or(false);

        let search_dir_str = arguments
            .get("dir")
            .or_else(|| arguments.get("directory"))
            .or_else(|| arguments.get("path"))
            .or_else(|| arguments.get("root"))
            .or_else(|| arguments.get("folder"))
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(".");
        let search_dir =
            Self::scoped_search_dir(crate::config::loader::resolve_path(search_dir_str));

        match search_dir.exists() {
            false => return Err(anyhow!("Directory '{}' does not exist", search_dir_str)),
            true => {}
        }

        let limit = 50;

        // Try using Ripgrep first since it is installed and extremely fast
        match self.run_ripgrep(&search_dir, query, is_regex, limit).await {
            Ok(output) => Ok(output),
            Err(_) => {
                // Fallback to manual recursive search when Ripgrep is unavailable
                let mut results = Vec::new();
                Self::walk_and_search(&search_dir, query, is_regex, &mut results, limit)?;

                Ok(json!({
                    "status": "success",
                    "results": results,
                    "capped": results.len() >= limit
                }))
            }
        }
    }
}

#[cfg(test)]
#[path = "grep_tests.rs"]
mod tests;
