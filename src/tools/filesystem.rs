use crate::config::resolve_path;
use crate::tools::Tool;
use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::fs;

fn deserialize_flexible_opt_usize<'de, D>(deserializer: D) -> std::result::Result<Option<usize>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt = Option::<serde_json::Value>::deserialize(deserializer)?;
    match opt {
        Some(serde_json::Value::Number(n)) => Ok(n
            .as_u64()
            .map(|u| u as usize)
            .or_else(|| n.as_i64().and_then(|i| (i >= 0).then_some(i as usize)))),
        Some(serde_json::Value::String(s)) => Ok(s.trim().parse::<usize>().ok()),
        _ => Ok(None),
    }
}

fn deserialize_flexible_usize<'de, D>(deserializer: D) -> std::result::Result<usize, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let val = serde_json::Value::deserialize(deserializer)?;
    match val {
        serde_json::Value::Number(n) => n
            .as_u64()
            .map(|u| u as usize)
            .or_else(|| n.as_i64().and_then(|i| (i >= 0).then_some(i as usize)))
            .ok_or_else(|| serde::de::Error::custom("expected non-negative integer")),
        serde_json::Value::String(s) => s
            .trim()
            .parse::<usize>()
            .map_err(|_| serde::de::Error::custom("expected integer string")),
        _ => Err(serde::de::Error::custom("expected number or string integer")),
    }
}

/// Computes a deterministic 16-bit FNV-1a hex digest (`4` lowercase hex chars)
/// for a single line of text (ignoring trailing `\r`).
pub fn line_content_hash(line: &str) -> String {
    let hash32 = line
        .trim_end_matches('\r')
        .as_bytes()
        .iter()
        .fold(0x811c_9dc5_u32, |acc, &b| {
            (acc ^ (b as u32)).wrapping_mul(0x0100_0193)
        });
    let hash16 = ((hash32 >> 16) ^ (hash32 & 0xffff)) as u16;
    format!("{:04x}", hash16)
}

fn normalize_expected_hash(raw: &str) -> String {
    raw.rsplit(':')
        .next()
        .unwrap_or(raw)
        .trim()
        .to_ascii_lowercase()
}

fn is_four_hex_chars(s: &str) -> bool {
    s.len() == 4 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

fn is_hashline_tag(prefix: &str) -> bool {
    let trimmed = prefix.trim_start();
    match trimmed.split_once(':') {
        Some((line_digits, hash_part)) => {
            !line_digits.is_empty()
                && line_digits.bytes().all(|b| b.is_ascii_digit())
                && is_four_hex_chars(hash_part)
        }
        None => is_four_hex_chars(trimmed),
    }
}

/// Strips a leading `Hashline` anchor prefix (`<line>:<4-hex>| ` or `<4-hex>| `)
/// when an LLM copies anchored output from `read_file(include_hashes: true)` into
/// `patch_file` or `replace_lines`.
pub fn strip_hashline_line_prefix(line: &str) -> &str {
    line.split_once('|')
        .and_then(|(prefix, rest)| {
            is_hashline_tag(prefix).then(|| rest.strip_prefix(' ').unwrap_or(rest))
        })
        .unwrap_or(line)
}

fn find_line_span_by_anchor(
    lines: &[String],
    requested_start_idx: usize,
    anchor_text: &str,
) -> Option<(usize, usize)> {
    let anchor_lines: Vec<&str> = anchor_text.lines().collect();
    let span = anchor_lines.len();
    (span > 0 && span <= lines.len()).then(|| {
        let max_start = lines.len() - span;
        let exact_match = (0..=max_start)
            .filter(|&idx| {
                lines[idx..idx + span]
                    .iter()
                    .zip(anchor_lines.iter())
                    .all(|(file_line, anchor_line)| {
                        let f_clean = file_line.trim_end_matches('\r');
                        let a_clean = anchor_line.trim_end_matches('\r');
                        f_clean == a_clean || f_clean == strip_hashline_line_prefix(a_clean)
                    })
            })
            .min_by_key(|&idx| idx.abs_diff(requested_start_idx));

        exact_match
            .or_else(|| {
                (0..=max_start)
                    .filter(|&idx| {
                        lines[idx..idx + span]
                            .iter()
                            .zip(anchor_lines.iter())
                            .all(|(file_line, anchor_line)| {
                                let f_trim = file_line.trim();
                                f_trim == anchor_line.trim()
                                    || f_trim == strip_hashline_line_prefix(anchor_line).trim()
                            })
                    })
                    .min_by_key(|&idx| idx.abs_diff(requested_start_idx))
            })
            .map(|start_idx| (start_idx, start_idx + span))
    })?
}

fn find_line_span_by_hash(
    lines: &[String],
    requested_start_idx: usize,
    span_len: usize,
    expected_hash: &str,
) -> Option<(usize, usize)> {
    let target_hash = normalize_expected_hash(expected_hash);
    (!target_hash.is_empty() && !lines.is_empty()).then(|| {
        (0..lines.len())
            .filter(|&idx| idx.abs_diff(requested_start_idx) <= 50)
            .filter(|&idx| line_content_hash(&lines[idx]) == target_hash)
            .min_by_key(|&idx| idx.abs_diff(requested_start_idx))
            .map(|start_idx| (start_idx, (start_idx + span_len.max(1)).min(lines.len())))
    })?
}

#[derive(Deserialize, Clone)]
struct SearchReplaceEdit {
    #[serde(
        alias = "old_text",
        alias = "oldText",
        alias = "old_str",
        alias = "oldStr",
        alias = "search",
        alias = "find"
    )]
    target: String,
    #[serde(
        alias = "new_text",
        alias = "newText",
        alias = "new_str",
        alias = "newStr",
        alias = "replace",
        alias = "content"
    )]
    replacement: String,
}

fn parse_conflict_marker_edits(patch_text: &str) -> Vec<SearchReplaceEdit> {
    #[derive(Clone, Copy)]
    enum MarkerState {
        Outside,
        InSearch,
        InReplace,
    }

    let (edits, _, _, _) = patch_text.lines().fold(
        (Vec::new(), MarkerState::Outside, Vec::<String>::new(), Vec::<String>::new()),
        |(mut acc, state, mut search_buf, mut replace_buf), line| {
            let trimmed = line.trim_end_matches('\r');
            match (
                state,
                trimmed.starts_with("<<<<<<< SEARCH"),
                trimmed == "=======",
                trimmed.starts_with(">>>>>>> REPLACE"),
            ) {
                (MarkerState::Outside, true, _, _) => {
                    search_buf.clear();
                    replace_buf.clear();
                    (acc, MarkerState::InSearch, search_buf, replace_buf)
                }
                (MarkerState::InSearch, _, true, _) => {
                    (acc, MarkerState::InReplace, search_buf, replace_buf)
                }
                (MarkerState::InReplace, _, _, true) => {
                    acc.push(SearchReplaceEdit {
                        target: search_buf.join("\n"),
                        replacement: replace_buf.join("\n"),
                    });
                    search_buf.clear();
                    replace_buf.clear();
                    (acc, MarkerState::Outside, search_buf, replace_buf)
                }
                (MarkerState::InSearch, _, _, _) => {
                    search_buf.push(trimmed.to_string());
                    (acc, MarkerState::InSearch, search_buf, replace_buf)
                }
                (MarkerState::InReplace, _, _, _) => {
                    replace_buf.push(trimmed.to_string());
                    (acc, MarkerState::InReplace, search_buf, replace_buf)
                }
                (MarkerState::Outside, false, _, _) => {
                    (acc, MarkerState::Outside, search_buf, replace_buf)
                }
            }
        },
    );
    edits
}

fn apply_single_search_replace(content: &str, edit: &SearchReplaceEdit) -> Result<String> {
    let exact_replaced = (!edit.target.is_empty())
        .then(|| content.find(&edit.target))
        .flatten()
        .map(|pos| {
            let mut out = String::with_capacity(content.len() + edit.replacement.len());
            out.push_str(&content[..pos]);
            out.push_str(&edit.replacement);
            out.push_str(&content[pos + edit.target.len()..]);
            out
        });

    match exact_replaced {
        Some(updated) => Ok(updated),
        None => {
            let file_lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
            let target_lines: Vec<&str> = edit.target.lines().collect();
            let span = target_lines.len();
            let matched_start = (span > 0 && span <= file_lines.len())
                .then(|| {
                    let max_start = file_lines.len() - span;
                    (0..=max_start)
                        .find(|&idx| {
                            file_lines[idx..idx + span]
                                .iter()
                                .zip(target_lines.iter())
                                .all(|(a, b)| {
                                    a.trim_end() == b.trim_end()
                                        || a.trim_end() == strip_hashline_line_prefix(b).trim_end()
                                })
                        })
                        .or_else(|| {
                            (0..=max_start).find(|&idx| {
                                file_lines[idx..idx + span]
                                    .iter()
                                    .zip(target_lines.iter())
                                    .all(|(a, b)| {
                                        a.trim() == b.trim()
                                            || a.trim() == strip_hashline_line_prefix(b).trim()
                                    })
                            })
                        })
                })
                .flatten()
                .ok_or_else(|| anyhow!("Search block not found in target file"))?;

            let mut updated_lines = Vec::with_capacity(file_lines.len() + edit.replacement.lines().count());
            updated_lines.extend(file_lines[..matched_start].iter().cloned());
            updated_lines.extend(
                edit.replacement
                    .lines()
                    .map(|l| strip_hashline_line_prefix(l).to_string()),
            );
            updated_lines.extend(file_lines[matched_start + span..].iter().cloned());

            let mut joined = updated_lines.join("\n");
            let needs_trailing_newline = content.ends_with('\n') && !joined.ends_with('\n');
            needs_trailing_newline.then(|| joined.push('\n'));
            Ok(joined)
        }
    }
}

#[derive(Default)]
struct FuzzyHunk {
    old_lines: Vec<String>,
    new_lines: Vec<String>,
}

fn parse_fuzzy_hunks(patch_text: &str) -> Vec<FuzzyHunk> {
    let (mut hunks, current) = patch_text.lines().fold(
        (Vec::<FuzzyHunk>::new(), FuzzyHunk::default()),
        |(mut acc, mut cur), raw_line| {
            let line = raw_line.trim_end_matches('\r');
            let is_file_header = line.starts_with("--- ")
                || line.starts_with("+++ ")
                || line.starts_with("diff ")
                || line.starts_with("index ")
                || line.starts_with("\\ No newline");
            let is_hunk_header = line.starts_with("@@");

            match (is_file_header, is_hunk_header, line.chars().next()) {
                (true, _, _) => (acc, cur),
                (false, true, _) => {
                    let has_lines = !cur.old_lines.is_empty() || !cur.new_lines.is_empty();
                    has_lines.then(|| acc.push(std::mem::take(&mut cur)));
                    (acc, cur)
                }
                (false, false, Some('-')) => {
                    cur.old_lines.push(line[1..].to_string());
                    (acc, cur)
                }
                (false, false, Some('+')) => {
                    cur.new_lines.push(line[1..].to_string());
                    (acc, cur)
                }
                (false, false, Some(' ')) => {
                    cur.old_lines.push(line[1..].to_string());
                    cur.new_lines.push(line[1..].to_string());
                    (acc, cur)
                }
                _ => (acc, cur),
            }
        },
    );
    let has_trailing = !current.old_lines.is_empty() || !current.new_lines.is_empty();
    has_trailing.then(|| hunks.push(current));
    hunks
}

fn apply_fuzzy_unified_diff(content: &str, patch_text: &str) -> Result<String> {
    let hunks = parse_fuzzy_hunks(patch_text);
    (!hunks.is_empty())
        .then_some(())
        .ok_or_else(|| anyhow!("No valid diff hunks found in patch"))?;

    let initial_lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    let patched_lines = hunks.iter().try_fold(initial_lines, |mut lines, hunk| -> Result<Vec<String>> {
        let span = hunk.old_lines.len();
        let matched_idx = match span {
            0 => Some(lines.len()),
            _ => (span <= lines.len())
                .then(|| {
                    let max_start = lines.len() - span;
                    (0..=max_start)
                        .find(|&idx| {
                            lines[idx..idx + span]
                                .iter()
                                .zip(hunk.old_lines.iter())
                                .all(|(a, b)| a == b)
                        })
                        .or_else(|| {
                            (0..=max_start).find(|&idx| {
                                lines[idx..idx + span]
                                    .iter()
                                    .zip(hunk.old_lines.iter())
                                    .all(|(a, b)| a.trim_end() == b.trim_end())
                            })
                        })
                        .or_else(|| {
                            (0..=max_start).find(|&idx| {
                                lines[idx..idx + span]
                                    .iter()
                                    .zip(hunk.old_lines.iter())
                                    .all(|(a, b)| a.trim() == b.trim())
                            })
                        })
                })
                .flatten(),
        }
        .ok_or_else(|| anyhow!("Failed to locate fuzzy diff hunk context in file"))?;

        let mut next_lines = Vec::with_capacity(lines.len() + hunk.new_lines.len());
        next_lines.extend(lines[..matched_idx].iter().cloned());
        next_lines.extend(hunk.new_lines.iter().cloned());
        next_lines.extend(lines[matched_idx + span..].iter().cloned());
        lines = next_lines;
        Ok(lines)
    })?;

    let mut joined = patched_lines.join("\n");
    let preserve_newline = content.ends_with('\n') && !joined.ends_with('\n');
    preserve_newline.then(|| joined.push('\n'));
    Ok(joined)
}

#[derive(Deserialize)]
struct PathArg {
    #[serde(
        default = "default_find_dir",
        alias = "file_path",
        alias = "filePath",
        alias = "dir",
        alias = "directory",
        alias = "target_dir",
        alias = "folder"
    )]
    path: String,
}

#[derive(Deserialize)]
struct ReadFileArgs {
    #[serde(
        alias = "file_path",
        alias = "filePath",
        alias = "target_file",
        alias = "targetFile",
        alias = "file",
        alias = "filename",
        alias = "uri"
    )]
    path: String,
    #[serde(
        default,
        deserialize_with = "deserialize_flexible_opt_usize",
        alias = "startLine",
        alias = "start",
        alias = "from_line"
    )]
    start_line: Option<usize>,
    #[serde(
        default,
        deserialize_with = "deserialize_flexible_opt_usize",
        alias = "endLine",
        alias = "end",
        alias = "to_line"
    )]
    end_line: Option<usize>,
    #[serde(
        default,
        alias = "hashline",
        alias = "includeHashes",
        alias = "with_hashes",
        alias = "line_hashes"
    )]
    include_hashes: bool,
}

#[derive(Deserialize)]
struct WriteFileArgs {
    #[serde(
        alias = "file_path",
        alias = "filePath",
        alias = "target_file",
        alias = "targetFile",
        alias = "file",
        alias = "filename",
        alias = "output_path",
        alias = "outputPath"
    )]
    path: String,
    #[serde(
        alias = "text",
        alias = "code",
        alias = "data",
        alias = "body"
    )]
    content: String,
}

#[derive(Deserialize)]
struct PatchFileArgs {
    #[serde(
        alias = "file_path",
        alias = "filePath",
        alias = "target_file",
        alias = "targetFile",
        alias = "file"
    )]
    path: String,
    #[serde(
        default,
        alias = "diff",
        alias = "unified_diff",
        alias = "unifiedDiff",
        alias = "content"
    )]
    patch: Option<String>,
    #[serde(
        default,
        alias = "old_text",
        alias = "oldText",
        alias = "old_str",
        alias = "oldStr",
        alias = "search",
        alias = "find"
    )]
    target: Option<String>,
    #[serde(
        default,
        alias = "new_text",
        alias = "newText",
        alias = "new_str",
        alias = "newStr",
        alias = "replace"
    )]
    replacement: Option<String>,
    #[serde(default)]
    edits: Option<Vec<SearchReplaceEdit>>,
}

#[derive(Deserialize)]
struct ReplaceLinesArgs {
    #[serde(
        alias = "file_path",
        alias = "filePath",
        alias = "target_file",
        alias = "targetFile",
        alias = "file"
    )]
    path: String,
    #[serde(
        deserialize_with = "deserialize_flexible_usize",
        alias = "startLine",
        alias = "start",
        alias = "from_line"
    )]
    start_line: usize,
    #[serde(
        deserialize_with = "deserialize_flexible_usize",
        alias = "endLine",
        alias = "end",
        alias = "to_line"
    )]
    end_line: usize,
    #[serde(
        alias = "content",
        alias = "new_content",
        alias = "newContent",
        alias = "text",
        alias = "code"
    )]
    replacement: String,
    #[serde(
        default,
        alias = "expectedHash",
        alias = "start_hash",
        alias = "startHash",
        alias = "hash"
    )]
    expected_hash: Option<String>,
    #[serde(
        default,
        alias = "anchorText",
        alias = "target_text",
        alias = "targetText",
        alias = "old_text",
        alias = "oldText"
    )]
    anchor_text: Option<String>,
}

#[derive(Deserialize)]
struct FindFilesArgs {
    #[serde(
        default = "default_find_pattern",
        alias = "glob",
        alias = "query",
        alias = "search",
        alias = "name",
        alias = "filename_pattern"
    )]
    pattern: String,
    #[serde(
        default = "default_find_dir",
        alias = "directory",
        alias = "root",
        alias = "path",
        alias = "folder"
    )]
    dir: String,
}

fn default_find_pattern() -> String {
    "*".to_string()
}

fn default_find_dir() -> String {
    ".".to_string()
}

const PATH_ALIAS_KEYS: &[&str] = &[
    "path",
    "file_path",
    "filePath",
    "target_file",
    "targetFile",
    "target_path",
    "targetPath",
    "file",
    "filename",
    "uri",
    "output_path",
    "outputPath",
    "AbsolutePath",
    "absolute_path",
    "DirectoryPath",
    "directory_path",
];
const DIR_PATH_ALIAS_KEYS: &[&str] = &[
    "path",
    "file_path",
    "filePath",
    "dir",
    "directory",
    "target_dir",
    "targetDir",
    "folder",
    "AbsolutePath",
    "absolute_path",
    "DirectoryPath",
    "directory_path",
];
const START_LINE_ALIAS_KEYS: &[&str] = &["start_line", "startLine", "start", "from_line", "fromLine"];
const END_LINE_ALIAS_KEYS: &[&str] = &["end_line", "endLine", "end", "to_line", "toLine"];
const INCLUDE_HASHES_ALIAS_KEYS: &[&str] = &[
    "include_hashes",
    "includeHashes",
    "hashline",
    "with_hashes",
    "withHashes",
    "line_hashes",
    "lineHashes",
];
const CONTENT_ALIAS_KEYS: &[&str] = &["content", "text", "code", "data", "body"];
const PATCH_ALIAS_KEYS: &[&str] = &["patch", "diff", "unified_diff", "unifiedDiff", "content"];
const TARGET_ALIAS_KEYS: &[&str] = &[
    "target", "old_text", "oldText", "old_str", "oldStr", "search", "find",
];
const PATCH_REPLACEMENT_ALIAS_KEYS: &[&str] = &[
    "replacement",
    "new_text",
    "newText",
    "new_str",
    "newStr",
    "replace",
];
const REPLACE_LINES_REPLACEMENT_ALIAS_KEYS: &[&str] = &[
    "replacement",
    "content",
    "new_content",
    "newContent",
    "text",
    "code",
];
const EXPECTED_HASH_ALIAS_KEYS: &[&str] = &[
    "expected_hash",
    "expectedHash",
    "start_hash",
    "startHash",
    "hash",
];
const ANCHOR_TEXT_ALIAS_KEYS: &[&str] = &[
    "anchor_text",
    "anchorText",
    "target_text",
    "targetText",
    "old_text",
    "oldText",
];

const READ_FILE_GROUPS: &[(&str, &[&str])] = &[
    ("path", PATH_ALIAS_KEYS),
    ("start_line", START_LINE_ALIAS_KEYS),
    ("end_line", END_LINE_ALIAS_KEYS),
    ("include_hashes", INCLUDE_HASHES_ALIAS_KEYS),
];
const WRITE_FILE_GROUPS: &[(&str, &[&str])] = &[
    ("path", PATH_ALIAS_KEYS),
    ("content", CONTENT_ALIAS_KEYS),
];
const LIST_DIR_GROUPS: &[(&str, &[&str])] = &[("path", DIR_PATH_ALIAS_KEYS)];
const PATCH_FILE_GROUPS: &[(&str, &[&str])] = &[
    ("path", PATH_ALIAS_KEYS),
    ("patch", PATCH_ALIAS_KEYS),
    ("target", TARGET_ALIAS_KEYS),
    ("replacement", PATCH_REPLACEMENT_ALIAS_KEYS),
];
const REPLACE_LINES_GROUPS: &[(&str, &[&str])] = &[
    ("path", PATH_ALIAS_KEYS),
    ("start_line", START_LINE_ALIAS_KEYS),
    ("end_line", END_LINE_ALIAS_KEYS),
    ("replacement", REPLACE_LINES_REPLACEMENT_ALIAS_KEYS),
    ("expected_hash", EXPECTED_HASH_ALIAS_KEYS),
    ("anchor_text", ANCHOR_TEXT_ALIAS_KEYS),
];

fn collapse_alias_groups(
    arguments: &serde_json::Value,
    groups: &[(&str, &[&str])],
) -> serde_json::Value {
    match arguments.as_object() {
        None => arguments.clone(),
        Some(orig_map) => {
            let mut map = orig_map.clone();
            for &(canonical, aliases) in groups {
                let resolved = aliases
                    .iter()
                    .find_map(|&alias_key| map.get(alias_key).cloned());
                for &alias_key in aliases {
                    map.remove(alias_key);
                }
                resolved.map(|val| map.insert(canonical.to_string(), val));
            }
            serde_json::Value::Object(map)
        }
    }
}

pub struct ReadFileTool;

#[async_trait::async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Read contents of a file. Supports reading specific line ranges (1-indexed) and optional content-hash line anchors (include_hashes)."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Absolute or relative path to the file. Aliases accepted: file_path, filePath." },
                "start_line": { "type": "integer", "description": "Start line (1-indexed, inclusive). Alias accepted: startLine." },
                "end_line": { "type": "integer", "description": "End line (1-indexed, inclusive). Alias accepted: endLine." },
                "include_hashes": { "type": "boolean", "description": "When true, prefixes each returned line with '<line_no>:<4-char-fnv1a-hash>| ' for drift-safe editing with replace_lines. Alias accepted: hashline." }
            },
            "required": ["path"]
        })
    }

    async fn call(&self, arguments: &serde_json::Value) -> Result<serde_json::Value> {
        let collapsed = collapse_alias_groups(arguments, READ_FILE_GROUPS);
        let args: ReadFileArgs = serde_json::from_value(collapsed)
            .map_err(|e| anyhow!("Invalid read_file arguments: {}", e))?;
        let path = resolve_path(&args.path);
        crate::config::loader::verify_safe_path(&path)?;

        let metadata = fs::metadata(&path)
            .with_context(|| format!("Failed to read file metadata at {:?}", path))?;
        const MAX_FILE_SIZE: u64 = 50 * 1024 * 1024;
        (metadata.len() <= MAX_FILE_SIZE)
            .then_some(())
            .ok_or_else(|| {
                anyhow!(
                    "File too large to read ({} bytes, max {} bytes). Use start_line/end_line to read specific ranges.",
                    metadata.len(),
                    MAX_FILE_SIZE
                )
            })?;

        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read file at {:?}", path))?;

        let has_range = args.start_line.is_some() || args.end_line.is_some();
        match (has_range, args.include_hashes) {
            (false, false) => Ok(serde_json::Value::String(content)),
            _ => {
                let lines: Vec<&str> = content.lines().collect();
                let start = args.start_line.unwrap_or(1).saturating_sub(1);
                let end = args.end_line.unwrap_or(lines.len()).min(lines.len());
                let valid_slice = (start <= lines.len() && start < end)
                    .then(|| &lines[start..end])
                    .unwrap_or(&[]);

                let formatted = match args.include_hashes {
                    true => valid_slice
                        .iter()
                        .enumerate()
                        .map(|(offset, line)| {
                            let line_no = start + offset + 1;
                            format!("{}:{}| {}", line_no, line_content_hash(line), line)
                        })
                        .collect::<Vec<_>>()
                        .join("\n"),
                    false => valid_slice.join("\n"),
                };
                Ok(serde_json::Value::String(formatted))
            }
        }
    }
}

pub struct WriteFileTool;

#[async_trait::async_trait]
impl Tool for WriteFileTool {
    fn name(&self) -> &str {
        "write_file"
    }

    fn description(&self) -> &str {
        "Write content to a file, overwriting it if it exists. Keep content payloads small. For large generated files (roughly over 8KB), write in smaller chunks with shell redirection/heredoc or patch_file to avoid malformed/truncated JSON tool calls."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Absolute or relative path to the file. Aliases accepted: file_path, filePath." },
                "content": { "type": "string", "description": "File content to write. Keep under roughly 8KB per tool call; for larger files, write in chunks or use a command-line heredoc approach." }
            },
            "required": ["path", "content"]
        })
    }

    async fn call(&self, arguments: &serde_json::Value) -> Result<serde_json::Value> {
        let collapsed = collapse_alias_groups(arguments, WRITE_FILE_GROUPS);
        let args: WriteFileArgs = serde_json::from_value(collapsed)
            .map_err(|e| anyhow!("Invalid write_file arguments: {}", e))?;

        let path = resolve_path(&args.path);
        crate::config::loader::verify_safe_path(&path)?;
        path.parent().map(fs::create_dir_all).transpose()?;

        fs::write(&path, &args.content)
            .with_context(|| format!("Failed to write to file at {:?}", path))?;

        Ok(serde_json::json!({ "status": "success", "path": path.to_string_lossy() }))
    }
}

pub struct ListDirTool;

#[async_trait::async_trait]
impl Tool for ListDirTool {
    fn name(&self) -> &str {
        "list_dir"
    }

    fn description(&self) -> &str {
        "List the contents of a directory."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Absolute or relative path to the directory. Aliases accepted: file_path, filePath." }
            },
            "required": ["path"]
        })
    }

    async fn call(&self, arguments: &serde_json::Value) -> Result<serde_json::Value> {
        let collapsed = collapse_alias_groups(arguments, LIST_DIR_GROUPS);
        let args: PathArg = serde_json::from_value(collapsed)
            .map_err(|e| anyhow!("Invalid list_dir arguments: {}", e))?;
        let path = resolve_path(&args.path);
        crate::config::loader::verify_safe_path(&path)?;

        let mut entries = Vec::new();
        for entry in fs::read_dir(&path)
            .with_context(|| format!("Failed to read directory at {:?}", path))?
        {
            let entry = entry?;
            let file_name = entry.file_name().to_string_lossy().into_owned();
            let metadata = entry.metadata()?;
            let is_dir = metadata.is_dir();
            let size = metadata.len();

            entries.push(serde_json::json!({
                "name": file_name,
                "is_dir": is_dir,
                "size_bytes": size
            }));
        }

        let canonical_path = path.canonicalize().unwrap_or_else(|_| path.clone());
        Ok(serde_json::json!({
            "path": path.to_string_lossy(),
            "canonical_path": canonical_path.to_string_lossy(),
            "entries": entries
        }))
    }
}

pub struct PatchFileTool;

#[async_trait::async_trait]
impl Tool for PatchFileTool {
    fn name(&self) -> &str {
        "patch_file"
    }

    fn description(&self) -> &str {
        "Apply a unified diff patch (exact or fuzzy whitespace-tolerant) or search-and-replace blocks to a file without rewriting it entirely."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Absolute or relative path to the file to modify. Aliases accepted: file_path, filePath." },
                "patch": { "type": "string", "description": "Unified diff patch content or SEARCH/REPLACE conflict blocks to apply." },
                "target": { "type": "string", "description": "Optional search block to find when using direct search-and-replace mode. Aliases: old_text, search." },
                "replacement": { "type": "string", "description": "Optional replacement text when using direct search-and-replace mode. Aliases: new_text, replace." },
                "edits": {
                    "type": "array",
                    "description": "Optional list of {target, replacement} search-and-replace blocks.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "target": { "type": "string" },
                            "replacement": { "type": "string" }
                        },
                        "required": ["target", "replacement"]
                    }
                }
            },
            "required": ["path"]
        })
    }

    async fn call(&self, arguments: &serde_json::Value) -> Result<serde_json::Value> {
        let collapsed = collapse_alias_groups(arguments, PATCH_FILE_GROUPS);
        let args: PatchFileArgs = serde_json::from_value(collapsed)
            .map_err(|e| anyhow!("Invalid patch_file arguments: {}", e))?;

        let path = resolve_path(&args.path);
        crate::config::loader::verify_safe_path(&path)?;

        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read file at {:?}", path))?;

        let mut search_replace_edits: Vec<SearchReplaceEdit> = args.edits.unwrap_or_default();
        args.target
            .zip(args.replacement)
            .map(|(target, replacement)| {
                search_replace_edits.push(SearchReplaceEdit { target, replacement })
            });
        args.patch
            .as_deref()
            .map(parse_conflict_marker_edits)
            .filter(|parsed| !parsed.is_empty())
            .map(|parsed| search_replace_edits.extend(parsed));

        let (patched_content, mode) = match (!search_replace_edits.is_empty(), args.patch.as_deref()) {
            (true, _) => {
                let updated = search_replace_edits
                    .iter()
                    .try_fold(content.clone(), |acc, edit| apply_single_search_replace(&acc, edit))?;
                Ok((updated, "search_replace"))
            }
            (false, Some(patch_str)) => {
                let exact_attempt = diffy::Patch::from_str(patch_str)
                    .map_err(|e| anyhow!("Failed to parse patch: {}", e))
                    .and_then(|parsed| {
                        diffy::apply(&content, &parsed)
                            .map_err(|e| anyhow!("Failed to apply patch: {}", e))
                    });
                match exact_attempt {
                    Ok(applied) => Ok((applied, "exact_diff")),
                    Err(exact_err) => apply_fuzzy_unified_diff(&content, patch_str)
                        .map(|applied| (applied, "fuzzy_diff"))
                        .map_err(|_| exact_err),
                }
            }
            (false, None) => Err(anyhow!(
                "patch_file requires either 'patch' (unified diff) or 'target'+'replacement' / 'edits'"
            )),
        }?;

        fs::write(&path, &patched_content)
            .with_context(|| format!("Failed to write patched content to file at {:?}", path))?;

        Ok(serde_json::json!({
            "status": "success",
            "mode": mode,
            "path": path.to_string_lossy()
        }))
    }
}

pub struct ReplaceLinesTool;

#[async_trait::async_trait]
impl Tool for ReplaceLinesTool {
    fn name(&self) -> &str {
        "replace_lines"
    }

    fn description(&self) -> &str {
        "Replace a specific range of lines (1-indexed, inclusive) in a file with new content. Supports optional expected_hash (from read_file include_hashes) or anchor_text to automatically relocate drifted line numbers."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Absolute or relative path to the file to edit. Aliases accepted: file_path, filePath." },
                "start_line": { "type": "integer", "description": "Start line number (1-indexed, inclusive). Alias accepted: startLine." },
                "end_line": { "type": "integer", "description": "End line number (1-indexed, inclusive). Alias accepted: endLine." },
                "replacement": { "type": "string", "description": "The new replacement text content. Alias accepted: content." },
                "expected_hash": { "type": "string", "description": "Optional 4-char FNV-1a hex hash of start_line (from read_file with include_hashes=true) to verify content and auto-relocate drifted line numbers." },
                "anchor_text": { "type": "string", "description": "Optional expected original line(s) at the target range to verify content and auto-relocate drifted line numbers." }
            },
            "required": ["path", "start_line", "end_line", "replacement"]
        })
    }

    async fn call(&self, arguments: &serde_json::Value) -> Result<serde_json::Value> {
        let collapsed = collapse_alias_groups(arguments, REPLACE_LINES_GROUPS);
        let args: ReplaceLinesArgs = serde_json::from_value(collapsed)
            .map_err(|e| anyhow!("Invalid replace_lines arguments: {}", e))?;
        let start_line = args.start_line.max(1);
        let end_line = args.end_line.max(1);
        let replacement = args.replacement;

        (start_line <= end_line).then_some(()).ok_or_else(|| {
            anyhow!(
                "Invalid line range: {} to {} (start_line cannot be greater than end_line)",
                start_line,
                end_line
            )
        })?;

        let path = resolve_path(&args.path);
        crate::config::loader::verify_safe_path(&path)?;

        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read file at {:?}", path))?;

        let lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();

        let requested_start_idx = start_line - 1;
        let requested_span = end_line - start_line + 1;

        let anchor_resolved = match args.anchor_text.as_deref().filter(|s| !s.is_empty()) {
            Some(anchor) => Some(
                find_line_span_by_anchor(&lines, requested_start_idx, anchor).ok_or_else(|| {
                    anyhow!(
                        "anchor_text not found near line {} in {:?}",
                        start_line,
                        path
                    )
                })?,
            ),
            None => None,
        };

        let hash_resolved = match (
            anchor_resolved,
            args.expected_hash.as_deref().filter(|s| !s.is_empty()),
        ) {
            (Some(span), _) => Some(span),
            (None, Some(expected_hash)) => Some(
                find_line_span_by_hash(&lines, requested_start_idx, requested_span, expected_hash)
                    .ok_or_else(|| {
                        anyhow!(
                            "expected_hash '{}' did not match line {} or nearby lines in {:?}",
                            expected_hash,
                            start_line,
                            path
                        )
                    })?,
            ),
            (None, None) => None,
        };

        let (start_idx, end_idx, relocated) = match hash_resolved {
            Some((resolved_start, resolved_end)) => (
                resolved_start,
                resolved_end,
                resolved_start != requested_start_idx,
            ),
            None => {
                (start_line <= lines.len() + 1)
                    .then_some(())
                    .ok_or_else(|| {
                        anyhow!(
                            "start_line {} is beyond file line count {}",
                            start_line,
                            lines.len()
                        )
                    })?;
                (requested_start_idx, end_line.min(lines.len()), false)
            }
        };

        let mut new_lines = Vec::new();
        new_lines.extend(lines[..start_idx].iter().cloned());
        new_lines.extend(replacement.lines().map(|s| s.to_string()));
        (end_idx < lines.len()).then(|| {
            new_lines.extend(lines[end_idx..].iter().cloned());
        });

        let mut new_content = new_lines.join("\n");
        let preserve_trailing_newline = content.ends_with('\n') && !new_content.ends_with('\n');
        preserve_trailing_newline.then(|| new_content.push('\n'));

        fs::write(&path, &new_content)
            .with_context(|| format!("Failed to write to file at {:?}", path))?;

        Ok(serde_json::json!({
            "status": "success",
            "path": path.to_string_lossy(),
            "applied_start_line": start_idx + 1,
            "applied_end_line": end_idx,
            "relocated": relocated,
            "lines_modified": end_idx - start_idx,
            "new_line_count": new_lines.len()
        }))
    }
}

pub struct FindFilesTool;

impl FindFilesTool {
    async fn run_fd(&self, dir: &std::path::Path, pattern: &str) -> Result<Vec<String>> {
        let mut cmd = tokio::process::Command::new("fd");
        cmd.arg("-g"); // Treat pattern as a glob
        cmd.arg("--hidden"); // Include hidden files
        cmd.arg("--exclude").arg("target");
        cmd.arg("--exclude").arg("node_modules");
        cmd.arg("--exclude").arg(".git");
        cmd.arg(pattern);
        cmd.arg(dir);

        let output = cmd.output().await?;
        if !output.status.success() && output.status.code() != Some(1) {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("fd failed: {}", stderr));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let results = stdout
            .lines()
            .map(|s| s.to_string())
            .collect::<Vec<String>>();

        Ok(results)
    }

    fn walk_and_find(
        dir: &std::path::Path,
        re: &regex::Regex,
        results: &mut Vec<String>,
    ) -> Result<()> {
        if let Ok(metadata) = dir.symlink_metadata() {
            if metadata.file_type().is_symlink() {
                return Ok(());
            }
        }
        if results.len() >= 1000 {
            return Ok(());
        }

        if let Some(name) = dir.file_name().and_then(|s| s.to_str()) {
            if name == "target" || name == "node_modules" || name == ".git" {
                return Ok(());
            }
        }

        if dir.is_file() {
            if let Some(name) = dir.file_name().and_then(|s| s.to_str()) {
                if re.is_match(name) {
                    results.push(dir.to_string_lossy().to_string());
                }
            }
            return Ok(());
        }

        if dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    Self::walk_and_find(&entry.path(), re, results)?;
                }
            }
        }

        Ok(())
    }

    fn glob_to_regex(&self, pattern: &str) -> Result<regex::Regex> {
        let mut regex_str = String::from("^");
        for c in pattern.chars() {
            match c {
                '*' => regex_str.push_str(".*"),
                '?' => regex_str.push('.'),
                '.' | '+' | '(' | ')' | '[' | ']' | '{' | '}' | '^' | '$' | '|' | '\\' => {
                    regex_str.push('\\');
                    regex_str.push(c);
                }
                _ => regex_str.push(c),
            }
        }
        regex_str.push('$');
        regex::Regex::new(&regex_str).map_err(|e| anyhow!("Invalid pattern: {}", e))
    }
}

#[async_trait::async_trait]
impl Tool for FindFilesTool {
    fn name(&self) -> &str {
        "find_files"
    }

    fn description(&self) -> &str {
        "Search for files inside a directory hierarchy matching a specific filename pattern/glob."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": { "type": "string", "description": "The search pattern (e.g. '*.rs', 'Cargo.toml', 'index.*'). Alias accepted: glob." },
                "dir": { "type": "string", "description": "The root directory to search in (defaults to '.'). Aliases accepted: directory, root." }
            },
            "required": ["pattern"]
        })
    }

    async fn call(&self, arguments: &serde_json::Value) -> Result<serde_json::Value> {
        let args: FindFilesArgs = serde_json::from_value(arguments.clone())
            .map_err(|e| anyhow!("Invalid find_files arguments: {}", e))?;

        let search_dir = resolve_path(&args.dir);
        crate::config::loader::verify_safe_path(&search_dir)?;

        if !search_dir.exists() {
            return Err(anyhow!("Directory '{}' does not exist", args.dir));
        }

        // Try using fd first since it is installed and extremely fast
        if let Ok(results) = self.run_fd(&search_dir, &args.pattern).await {
            return Ok(serde_json::json!({ "status": "success", "results": results }));
        }

        // Fallback to manual recursive search if fd fails or is not found
        let re = self.glob_to_regex(&args.pattern)?;
        let mut results = Vec::new();
        Self::walk_and_find(&search_dir, &re, &mut results)?;

        Ok(serde_json::json!({ "status": "success", "results": results }))
    }
}

pub struct ZenflowEditTool {
    pub provider: std::sync::Arc<dyn crate::providers::LLMProvider>,
}

#[async_trait::async_trait]
impl Tool for ZenflowEditTool {
    fn name(&self) -> &str {
        "zenflow_edit"
    }

    fn description(&self) -> &str {
        "Edit a file transactionally. Takes a git snapshot before writing. If compilation/tests fail, it attempts to self-heal using the LLM. If healing fails, it automatically rolls back the changes."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Path to the file to edit." },
                "content": { "type": "string", "description": "Complete new content to write to the file." },
                "compile_command": { "type": "string", "description": "Command to run to verify the build/test (e.g. 'cargo check', 'npm run build', 'pytest')." }
            },
            "required": ["path", "content", "compile_command"]
        })
    }

    async fn call(&self, arguments: &serde_json::Value) -> Result<serde_json::Value> {
        let path_str = arguments
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'path' parameter"))?;
        let content = arguments
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'content' parameter"))?;
        let compile_cmd = arguments
            .get("compile_command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'compile_command' parameter"))?;

        let path = resolve_path(path_str);
        crate::config::loader::verify_safe_path(&path)?;

        crate::core::heal::run_transactional_heal_edit(
            self.provider.as_ref(),
            &path,
            content,
            compile_cmd,
        )
        .await
    }
}

#[cfg(test)]
#[path = "filesystem_tests.rs"]
mod tests;
