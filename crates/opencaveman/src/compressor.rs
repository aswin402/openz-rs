use crate::ast_guard::{scan_secret_exposure, verify_compression_fidelity, FidelityReport};
use crate::level::CavemanLevel;
use crate::stats::{estimate_tokens, record_compression_savings};
use anyhow::{bail, Context, Result};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::ops::Range;
use std::path::{Path, PathBuf};

/// Phrase-level verbose-to-concise replacements (ordered longest-first).
const PHRASE_REPLACEMENTS: &[(&str, &str)] = &[
    ("due to the fact that", "because"),
    ("in the event that", "when"),
    ("at this point in time", "now"),
    ("for the purpose of", "for"),
    ("in order to", "to"),
    ("with regard to", "on"),
    ("with respect to", "on"),
    ("in accordance with", "per"),
    ("it is important to note that", ""),
    ("please note that", ""),
    ("keep in mind that", ""),
    ("make sure that", "ensure"),
    ("make sure to", "ensure"),
    ("be sure to", "ensure"),
    ("a large number of", "many"),
    ("a variety of", "various"),
    ("as well as", "and"),
    ("prior to", "before"),
    ("subsequent to", "after"),
    ("is able to", "can"),
    ("are able to", "can"),
    ("has the ability to", "can"),
    ("on a regular basis", "regularly"),
    ("in addition to", "plus"),
];

/// Sentence-leading throat-clearing prefixes stripped cleanly.
const THROAT_CLEARING_PREFIXES: &[&str] = &[
    "sure! i'd be happy to help you with that.",
    "sure, i'd be happy to help.",
    "i would be happy to help with that.",
    "certainly! here is",
    "of course! let's look at",
    "please remember that ",
    "note that you should ",
    "you should always remember to ",
];

/// Filler adverbs removed when surrounded by word boundaries.
const FILLER_WORDS: &[&str] = &[
    "basically",
    "actually",
    "literally",
    "honestly",
    "simply",
    "really",
    "just",
    "very",
    "quite",
    "perhaps",
];

/// Standard engineering shorthand applied only in `Ultra` / telegraphic tiers.
const ULTRA_SHORTHAND: &[(&str, &str)] = &[
    ("configuration", "cfg"),
    ("authentication", "auth"),
    ("authorization", "authz"),
    ("repository", "repo"),
    ("database", "db"),
    ("application", "app"),
    ("environment", "env"),
    ("implementation", "impl"),
    ("dependencies", "deps"),
    ("dependency", "dep"),
    ("parameters", "params"),
    ("parameter", "param"),
    ("directory", "dir"),
    ("documentation", "docs"),
];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupEntry {
    pub backup_id: String,
    pub original_path: String,
    pub backup_file: String,
    pub sha256_before: String,
    pub created_at: String,
    pub original_tokens: usize,
    pub compressed_tokens: usize,
    pub level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompressionOutcome {
    pub path: Option<String>,
    pub level: String,
    pub original_tokens: usize,
    pub compressed_tokens: usize,
    pub saved_tokens: usize,
    pub savings_pct: f64,
    pub used_llm_candidate: bool,
    pub fidelity: FidelityReport,
    pub backup: Option<BackupEntry>,
    pub compressed_content: String,
}

pub struct BackupStore {
    root_dir: PathBuf,
}

fn frontmatter_end_offset(markdown: &str) -> usize {
    match markdown.starts_with("---\n") || markdown.starts_with("---\r\n") {
        false => 0,
        true => {
            let after_open = &markdown[3..];
            after_open
                .find("\n---\n")
                .map(|idx| 3 + idx + 5)
                .or_else(|| after_open.find("\n---\r\n").map(|idx| 3 + idx + 6))
                .unwrap_or(0)
        }
    }
}

fn merge_ranges(mut ranges: Vec<Range<usize>>) -> Vec<Range<usize>> {
    ranges.sort_by_key(|r| (r.start, r.end));
    let mut merged: Vec<Range<usize>> = Vec::with_capacity(ranges.len());
    for r in ranges {
        match merged.last_mut() {
            Some(last) => match r.start <= last.end {
                true => last.end = last.end.max(r.end),
                false => merged.push(r),
            },
            None => merged.push(r),
        }
    }
    merged
}

/// Computes exact byte spans in `markdown` that MUST remain 100% untouched:
/// YAML frontmatter, fenced/indented code blocks, inline code spans, links/images,
/// HTML blocks, and Markdown tables.
fn collect_protected_spans(markdown: &str) -> Vec<Range<usize>> {
    let mut protected = Vec::new();
    let fm_end = frontmatter_end_offset(markdown);
    match fm_end > 0 {
        true => protected.push(0..fm_end),
        false => {}
    }

    let body = &markdown[fm_end..];
    let parser = Parser::new_ext(body, Options::all()).into_offset_iter();
    let mut active_depth = 0usize;
    let mut block_start = 0usize;

    for (event, range) in parser {
        let shifted = (range.start + fm_end)..(range.end + fm_end);
        match event {
            Event::Start(
                Tag::CodeBlock(_)
                | Tag::Link { .. }
                | Tag::Image { .. }
                | Tag::HtmlBlock
                | Tag::Table(_),
            ) => {
                match active_depth == 0 {
                    true => block_start = shifted.start,
                    false => {}
                }
                active_depth += 1;
            }
            Event::End(
                TagEnd::CodeBlock
                | TagEnd::Link
                | TagEnd::Image
                | TagEnd::HtmlBlock
                | TagEnd::Table,
            ) => {
                active_depth = active_depth.saturating_sub(1);
                match active_depth == 0 {
                    true => protected.push(block_start..shifted.end),
                    false => {}
                }
            }
            Event::Code(_) | Event::Html(_) | Event::InlineHtml(_) => match active_depth == 0 {
                true => protected.push(shifted),
                false => {}
            },
            _ => {}
        }
    }

    merge_ranges(protected)
}

fn replace_case_insensitive_phrase(input: &str, needle_lower: &str, replacement: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut cursor = 0usize;
    let lowered = input.to_ascii_lowercase();
    let nlen = needle_lower.len();

    while cursor < input.len() {
        match lowered[cursor..].find(needle_lower) {
            None => {
                out.push_str(&input[cursor..]);
                break;
            }
            Some(rel) => {
                let start = cursor + rel;
                let end = start + nlen;
                let prev_ok = start == 0
                    || !input[..start]
                        .chars()
                        .next_back()
                        .is_some_and(|c| c.is_alphanumeric());
                let next_ok = end >= input.len()
                    || !input[end..]
                        .chars()
                        .next()
                        .is_some_and(|c| c.is_alphanumeric());
                let same_byte_boundaries =
                    input.is_char_boundary(start) && input.is_char_boundary(end);

                match prev_ok && next_ok && same_byte_boundaries {
                    true => {
                        out.push_str(&input[cursor..start]);
                        let capitalize = input[start..]
                            .chars()
                            .next()
                            .is_some_and(|c| c.is_ascii_uppercase());
                        match (capitalize, replacement.chars().next()) {
                            (true, Some(first)) => {
                                out.extend(first.to_uppercase());
                                out.push_str(&replacement[first.len_utf8()..]);
                            }
                            _ => out.push_str(replacement),
                        }
                        cursor = end;
                    }
                    false => {
                        let step = input[cursor..]
                            .chars()
                            .next()
                            .map(char::len_utf8)
                            .unwrap_or(1);
                        out.push_str(&input[cursor..cursor + step]);
                        cursor += step;
                    }
                }
            }
        }
    }

    out
}

fn split_markdown_line_prefix(line: &str) -> (&str, &str) {
    let ws_bytes = line.bytes().take_while(|b| *b == b' ' || *b == b'\t').count();
    let rest = &line[ws_bytes..];

    // Heading prefix `### `
    let hashes = rest.bytes().take_while(|b| *b == b'#').count();
    match hashes > 0 && hashes <= 6 && rest.as_bytes().get(hashes) == Some(&b' ') {
        true => return (&line[..ws_bytes + hashes + 1], &rest[hashes + 1..]),
        false => {}
    }

    // Bullet prefix `- `, `* `, `+ `
    match rest.starts_with("- [ ] ") || rest.starts_with("- [x] ") || rest.starts_with("- [X] ") {
        true => return (&line[..ws_bytes + 6], &rest[6..]),
        false => {}
    }
    match rest.starts_with("- ") || rest.starts_with("* ") || rest.starts_with("+ ") || rest.starts_with("> ") {
        true => return (&line[..ws_bytes + 2], &rest[2..]),
        false => {}
    }

    // Ordered list `1. `
    let digits = rest.bytes().take_while(|b| b.is_ascii_digit()).count();
    match digits > 0 && rest[digits..].starts_with(". ") {
        true => (&line[..ws_bytes + digits + 2], &rest[digits + 2..]),
        false => (&line[..ws_bytes], rest),
    }
}

fn compress_prose_line_body(body: &str, drops_articles: bool, telegraphic: bool) -> String {
    let trimmed = body.trim();
    match trimmed.is_empty() {
        true => return String::new(),
        false => {}
    }

    let mut working = body.to_string();
    let lower_start = working.trim_start().to_ascii_lowercase();
    for prefix in THROAT_CLEARING_PREFIXES {
        match lower_start.starts_with(prefix) {
            true => {
                let lead_ws = working.len() - working.trim_start().len();
                working = working[lead_ws + prefix.len()..].trim_start().to_string();
                break;
            }
            false => {}
        }
    }

    for (verbose, concise) in PHRASE_REPLACEMENTS {
        working = replace_case_insensitive_phrase(&working, verbose, concise);
    }

    match telegraphic {
        true => {
            for (word, short) in ULTRA_SHORTHAND {
                working = replace_case_insensitive_phrase(&working, word, short);
            }
        }
        false => {}
    }

    let raw_tokens: Vec<&str> = working.split_whitespace().collect();
    let mut kept: Vec<String> = Vec::with_capacity(raw_tokens.len());
    let mut capitalize_next = false;

    for (idx, tok) in raw_tokens.iter().copied().enumerate() {
        let bare = tok.trim_matches(|c: char| matches!(c, ',' | '.' | ';' | ':' | '(' | ')'));
        let lower = bare.to_ascii_lowercase();

        let is_filler = FILLER_WORDS.contains(&lower.as_str()) && bare.len() == tok.len();
        let is_article = drops_articles
            && matches!(lower.as_str(), "a" | "an" | "the")
            && bare.len() == tok.len()
            && raw_tokens.len() > 3;
        let is_telegraphic_copula = telegraphic
            && idx > 0
            && matches!(lower.as_str(), "is" | "are" | "was" | "were" | "that")
            && bare.len() == tok.len();

        let was_sentence_start = tok
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_uppercase())
            && (idx == 0
                || kept
                    .last()
                    .is_some_and(|prev| prev.ends_with('.') || prev.ends_with('!')));

        match is_filler || is_article || is_telegraphic_copula {
            true => {
                capitalize_next = capitalize_next || was_sentence_start;
            }
            false => {
                let final_tok = match (capitalize_next, tok.chars().next().filter(|c| c.is_ascii_lowercase())) {
                    (true, Some(first)) => {
                        capitalize_next = false;
                        let mut s = first.to_uppercase().collect::<String>();
                        s.push_str(&tok[first.len_utf8()..]);
                        s
                    }
                    _ => {
                        capitalize_next = false;
                        tok.to_string()
                    }
                };
                kept.push(final_tok);
            }
        }
    }

    kept.join(" ")
}

fn compress_unprotected_segment(segment: &str, level: CavemanLevel) -> String {
    let desc = level.descriptor();
    let drops_articles = desc.drops_articles;
    let telegraphic = desc.telegraphic_clauses;

    let trailing_newline = segment.ends_with('\n');
    let lines: Vec<&str> = segment.split('\n').collect();
    let count = lines.len();

    let mut out_lines: Vec<String> = Vec::with_capacity(count);
    for (i, line) in lines.into_iter().enumerate() {
        let is_final_empty_after_newline = trailing_newline && i + 1 == count && line.is_empty();
        match is_final_empty_after_newline {
            true => {}
            false => {
                let (prefix, body) = split_markdown_line_prefix(line);
                match body.trim().is_empty() {
                    true => out_lines.push(line.to_string()),
                    false => {
                        let leading_space = segment.starts_with(' ') && i == 0 && prefix.is_empty();
                        let trailing_space = body.ends_with(' ');
                        let compressed =
                            compress_prose_line_body(body, drops_articles, telegraphic);
                        let mut rebuilt = String::new();
                        match leading_space {
                            true => rebuilt.push(' '),
                            false => {}
                        }
                        rebuilt.push_str(prefix);
                        rebuilt.push_str(&compressed);
                        match trailing_space && !rebuilt.ends_with(' ') {
                            true => rebuilt.push(' '),
                            false => {}
                        }
                        out_lines.push(rebuilt);
                    }
                }
            }
        }
    }

    let mut joined = out_lines.join("\n");
    match trailing_newline {
        true => joined.push('\n'),
        false => {}
    }
    joined
}

/// Pure-Rust AST-guarded Markdown prose compressor.
/// Guarantees 100% byte-for-byte preservation of YAML frontmatter, code blocks,
/// inline code literals, links/URLs, HTML blocks, and tables.
pub fn compress_markdown_deterministic(markdown: &str, level: CavemanLevel) -> String {
    let protected = collect_protected_spans(markdown);
    let mut out = String::with_capacity(markdown.len());
    let mut cursor = 0usize;

    for span in protected {
        match cursor < span.start {
            true => {
                let prose = &markdown[cursor..span.start];
                out.push_str(&compress_unprotected_segment(prose, level));
            }
            false => {}
        }
        out.push_str(&markdown[span.start..span.end]);
        cursor = span.end;
    }

    match cursor < markdown.len() {
        true => {
            out.push_str(&compress_unprotected_segment(&markdown[cursor..], level));
        }
        false => {}
    }

    out
}

/// Validates an optional LLM-generated candidate against structural AST anchors;
/// automatically falls back to `compress_markdown_deterministic` on any drift.
pub fn validate_or_fallback_compression(
    original: &str,
    llm_candidate: Option<&str>,
    level: CavemanLevel,
) -> (String, bool, FidelityReport) {
    match llm_candidate.filter(|c| !c.trim().is_empty()) {
        Some(candidate) => {
            let report = verify_compression_fidelity(original, candidate);
            let saves_tokens = estimate_tokens(candidate) < estimate_tokens(original);
            match report.passed && saves_tokens {
                true => return (candidate.to_string(), true, report),
                false => {}
            }
        }
        None => {}
    }

    let deterministic = compress_markdown_deterministic(original, level);
    let report = verify_compression_fidelity(original, &deterministic);
    match report.passed {
        true => (deterministic, false, report),
        false => (
            original.to_string(),
            false,
            verify_compression_fidelity(original, original),
        ),
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

impl BackupStore {
    pub fn new(runtime_data_dir: &Path) -> Self {
        Self {
            root_dir: runtime_data_dir.join("backups").join("opencaveman"),
        }
    }

    fn manifest_path(&self) -> PathBuf {
        self.root_dir.join("manifest.json")
    }

    pub fn load_entries(&self) -> Vec<BackupEntry> {
        let path = self.manifest_path();
        std::fs::read_to_string(path)
            .ok()
            .and_then(|raw| serde_json::from_str::<Vec<BackupEntry>>(&raw).ok())
            .unwrap_or_default()
    }

    fn save_entries(&self, entries: &[BackupEntry]) -> Result<()> {
        std::fs::create_dir_all(&self.root_dir)?;
        let serialized = serde_json::to_string_pretty(entries)?;
        std::fs::write(self.manifest_path(), serialized)?;
        Ok(())
    }

    pub fn record_backup(
        &self,
        original_path: &Path,
        original_content: &str,
        original_tokens: usize,
        compressed_tokens: usize,
        level: CavemanLevel,
    ) -> Result<BackupEntry> {
        let canonical = original_path
            .canonicalize()
            .unwrap_or_else(|_| original_path.to_path_buf());
        let parent_slug = canonical
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("workspace");
        let safe_slug: String = parent_slug
            .chars()
            .map(|c| match c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                true => c,
                false => '_',
            })
            .collect();

        let bucket_dir = self.root_dir.join(&safe_slug);
        std::fs::create_dir_all(&bucket_dir)?;

        let hash = sha256_hex(original_content.as_bytes());
        let short_hash = &hash[..12];
        let ts = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
        let file_name = canonical
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("document.md");
        let backup_id = format!("{safe_slug}_{ts}_{short_hash}");
        let backup_file_path = bucket_dir.join(format!("{ts}_{short_hash}_{file_name}.bak"));

        std::fs::write(&backup_file_path, original_content)?;

        let entry = BackupEntry {
            backup_id,
            original_path: canonical.display().to_string(),
            backup_file: backup_file_path.display().to_string(),
            sha256_before: hash,
            created_at: chrono::Utc::now().to_rfc3339(),
            original_tokens,
            compressed_tokens,
            level: level.as_str().to_string(),
        };

        let mut entries = self.load_entries();
        entries.push(entry.clone());
        self.save_entries(&entries)?;

        Ok(entry)
    }
}

/// Compresses a Markdown/context file with pre-flight secret scanning, AST anchor verification,
/// out-of-tree backup under `<runtime_data_dir>/backups/opencaveman/`, and savings telemetry.
pub fn compress_file_with_backup(
    target_path: &Path,
    runtime_data_dir: &Path,
    level: CavemanLevel,
    llm_candidate: Option<&str>,
    dry_run: bool,
) -> Result<CompressionOutcome> {
    let original = std::fs::read_to_string(target_path)
        .with_context(|| format!("Failed to read {}", target_path.display()))?;

    match scan_secret_exposure(Some(target_path), &original) {
        Some(rejection) => {
            bail!(
                "OpenCaveman safety guard blocked compression: {} ({})",
                rejection.reason,
                rejection.indicator
            );
        }
        None => {}
    }

    let (compressed, used_llm, fidelity) =
        validate_or_fallback_compression(&original, llm_candidate, level);

    let orig_tokens = estimate_tokens(&original);
    let comp_tokens = estimate_tokens(&compressed);
    let saved_tokens = orig_tokens.saturating_sub(comp_tokens);
    let savings_pct = match orig_tokens == 0 {
        true => 0.0,
        false => ((saved_tokens as f64) / (orig_tokens as f64)) * 100.0,
    };

    let backup = match !dry_run && saved_tokens > 0 && compressed != original {
        false => None,
        true => {
            let store = BackupStore::new(runtime_data_dir);
            let entry =
                store.record_backup(target_path, &original, orig_tokens, comp_tokens, level)?;
            std::fs::write(target_path, &compressed)
                .with_context(|| format!("Failed to write compressed {}", target_path.display()))?;
            let _ = record_compression_savings(runtime_data_dir, orig_tokens, comp_tokens);
            Some(entry)
        }
    };

    Ok(CompressionOutcome {
        path: Some(target_path.display().to_string()),
        level: level.as_str().to_string(),
        original_tokens: orig_tokens,
        compressed_tokens: comp_tokens,
        saved_tokens,
        savings_pct,
        used_llm_candidate: used_llm,
        fidelity,
        backup,
        compressed_content: compressed,
    })
}

pub fn list_backups(runtime_data_dir: &Path) -> Vec<BackupEntry> {
    let mut entries = BackupStore::new(runtime_data_dir).load_entries();
    entries.reverse();
    entries
}

/// Restores the most recent (or explicitly specified `backup_id`) out-of-tree backup.
pub fn restore_backup(
    runtime_data_dir: &Path,
    backup_id_or_path: &str,
) -> Result<BackupEntry> {
    let entries = BackupStore::new(runtime_data_dir).load_entries();
    let needle = backup_id_or_path.trim();
    let target_canonical = Path::new(needle)
        .canonicalize()
        .ok()
        .map(|p| p.display().to_string());

    let matched = entries
        .into_iter()
        .rev()
        .find(|entry| {
            entry.backup_id == needle
                || entry.original_path == needle
                || target_canonical.as_deref() == Some(entry.original_path.as_str())
        })
        .with_context(|| format!("No OpenCaveman backup found matching '{needle}'"))?;

    let backup_bytes = std::fs::read_to_string(&matched.backup_file)
        .with_context(|| format!("Backup payload missing at {}", matched.backup_file))?;
    std::fs::write(&matched.original_path, backup_bytes)
        .with_context(|| format!("Failed to restore {}", matched.original_path))?;

    Ok(matched)
}
