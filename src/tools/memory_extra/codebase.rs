use crate::memory::{scope_from_args, with_graph_db as with_db};
use crate::tools::Tool;
use anyhow::{anyhow, Result};
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::Path;

pub(crate) const STOP_WORDS: &[&str] = &[
    "a",
    "about",
    "after",
    "again",
    "all",
    "am",
    "an",
    "and",
    "any",
    "are",
    "as",
    "at",
    "be",
    "been",
    "before",
    "being",
    "below",
    "between",
    "both",
    "but",
    "by",
    "can",
    "did",
    "do",
    "does",
    "doing",
    "down",
    "during",
    "each",
    "few",
    "for",
    "from",
    "further",
    "had",
    "has",
    "have",
    "having",
    "he",
    "her",
    "here",
    "hers",
    "herself",
    "him",
    "himself",
    "his",
    "how",
    "i",
    "if",
    "in",
    "into",
    "is",
    "it",
    "its",
    "itself",
    "just",
    "me",
    "more",
    "most",
    "my",
    "myself",
    "no",
    "nor",
    "not",
    "now",
    "of",
    "off",
    "on",
    "once",
    "only",
    "or",
    "other",
    "our",
    "ours",
    "ourselves",
    "out",
    "over",
    "own",
    "same",
    "she",
    "should",
    "so",
    "some",
    "someone",
    "something",
    "than",
    "that",
    "the",
    "their",
    "theirs",
    "them",
    "themselves",
    "then",
    "there",
    "these",
    "they",
    "this",
    "those",
    "through",
    "to",
    "too",
    "under",
    "until",
    "up",
    "very",
    "was",
    "we",
    "were",
    "what",
    "when",
    "where",
    "which",
    "who",
    "whom",
    "why",
    "will",
    "with",
    "you",
    "your",
    "yours",
    "yourself",
    "yourselves",
];

// ─── Tool: CompressContextTool ───────────────────────────────────

pub struct CompressContextTool;

#[async_trait::async_trait]
impl Tool for CompressContextTool {
    fn name(&self) -> &str {
        "compress_context"
    }

    fn description(&self) -> &str {
        "Compress text context by scoring sentences using TF-IDF and keeping a specified ratio."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "text": { "type": "string", "description": "Text to compress" },
                "ratio": { "type": "number", "default": 0.5, "description": "Ratio of sentences to keep (0.0-1.0)" },
                "userId": { "type": "string" },
                "sessionId": { "type": "string" },
                "agentId": { "type": "string" }
            },
            "required": ["text"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let text = match arguments.as_str() {
            Some(s) => s,
            None => arguments
                .get("text")
                .or_else(|| arguments.get("content"))
                .or_else(|| arguments.get("context"))
                .or_else(|| arguments.get("raw_text"))
                .or_else(|| arguments.get("input"))
                .or_else(|| arguments.get("body"))
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Missing 'text'"))?,
        };
        let ratio = arguments
            .get("ratio")
            .and_then(|v| {
                v.as_f64()
                    .or_else(|| v.as_str().and_then(|s| s.parse::<f64>().ok()))
            })
            .unwrap_or(0.5)
            .clamp(0.0, 1.0);

        // Simple sentence splitting
        let sentences: Vec<&str> = text
            .split(['.', '!', '?'])
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();

        match sentences.is_empty() {
            true => {
                return Ok(json!({
                    "originalLength": text.len(),
                    "compressedLength": 0,
                    "ratio": ratio,
                    "compressedText": "",
                }));
            }
            false => {}
        }

        // TF-IDF scoring (simplified)
        let mut sentence_terms: Vec<Vec<String>> = Vec::new();
        let mut term_dfs: HashMap<String, usize> = HashMap::new();

        for &sentence in &sentences {
            let words: Vec<String> = sentence
                .chars()
                .filter(|c| c.is_alphanumeric() || c.is_whitespace())
                .collect::<String>()
                .split_whitespace()
                .map(|w| w.to_lowercase())
                .filter(|w| w.len() >= 3 && STOP_WORDS.binary_search(&w.as_str()).is_err())
                .collect();
            let unique_terms: HashSet<String> = words.iter().cloned().collect();
            sentence_terms.push(words);
            for term in unique_terms {
                *term_dfs.entry(term.clone()).or_insert(0) += 1;
            }
        }

        let n_sentences = sentences.len() as f64;
        let mut scored: Vec<(usize, f64)> = Vec::new();

        for (i, terms) in sentence_terms.iter().enumerate() {
            let mut tfidf = 0.0f64;
            let mut seen = HashSet::new();
            for term in terms {
                match seen.insert(term) {
                    false => continue,
                    true => {}
                }
                let tf = terms.iter().filter(|t| *t == term).count() as f64 / terms.len() as f64;
                let df = *term_dfs.get(term).unwrap_or(&1) as f64;
                let idf = (n_sentences / df).ln() + 1.0;
                tfidf += tf * idf;
            }
            scored.push((i, tfidf));
        }

        // Sort by score descending, keep top ratio
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let keep_count = ((n_sentences * ratio).ceil() as usize).max(1);
        let top_indices: HashSet<usize> = scored.iter().take(keep_count).map(|(i, _)| *i).collect();

        // Reconstruct in original order
        let compressed: String = sentences
            .iter()
            .enumerate()
            .filter(|(i, _)| top_indices.contains(i))
            .map(|(_, s)| *s)
            .collect::<Vec<&str>>()
            .join(". ");

        let compressed_text = match compressed.is_empty() {
            true => sentences[0],
            false => &compressed,
        };

        Ok(json!({
            "originalLength": text.len(),
            "compressedLength": compressed.len(),
            "ratio": ratio,
            "compressedText": compressed_text,
        }))
    }
}

// ─── Tool: MemoryStatsTool ──────────────────────────────────────

pub struct MemoryStatsTool;

#[async_trait::async_trait]
impl Tool for MemoryStatsTool {
    fn name(&self) -> &str {
        "memory_stats"
    }

    fn description(&self) -> &str {
        "Get memory access statistics and record counts for all layers."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "userId": { "type": "string" },
                "sessionId": { "type": "string" },
                "agentId": { "type": "string" }
            }
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let (uid, sid, aid) = scope_from_args(arguments);
        let scope = crate::tools::memory_extra::coordinator::MemoryScope::new(uid, sid, aid);
        let snapshot = crate::tools::memory_extra::coordinator::MemoryCoordinator
            .stats(&scope)
            .await?;

        let (episodic, reflections, tool_perf) = with_db(|conn| {
            let episodic: i64 =
                conn.query_row("SELECT COUNT(*) FROM episodic_logs", [], |r| r.get(0))?;
            let reflections: i64 =
                conn.query_row("SELECT COUNT(*) FROM reflection_memory", [], |r| r.get(0))?;
            let tool_perf: i64 =
                conn.query_row("SELECT COUNT(*) FROM tool_performance", [], |r| r.get(0))?;
            Ok((episodic, reflections, tool_perf))
        })?;

        Ok(json!({
            "coordinator": true,
            "graphNodes": snapshot.graph_nodes,
            "graphEdges": snapshot.graph_edges,
            "episodicLogs": episodic,
            "reflections": reflections,
            "toolPerformance": tool_perf,
            "sharedMemory": snapshot.shared_memories,
            "semanticFacts": snapshot.semantic_facts,
            "semanticFactsWithEmbeddings": snapshot.semantic_facts_with_embeddings,
            "cognitiveMemories": snapshot.cognitive_memories,
            "researchEntries": snapshot.research_entries,
            "sessionMetadataMemories": snapshot.session_metadata_memories,
            "skillsMemories": snapshot.skills_memories,
            "workingMemory": snapshot.working_memory,
            "totalActive": snapshot.total_active,
        }))
    }
}

// ─── Tool: CompactMemoriesTool ───────────────────────────────────

pub struct CompactMemoriesTool;

#[async_trait::async_trait]
impl Tool for CompactMemoriesTool {
    fn name(&self) -> &str {
        "compact_memories"
    }

    fn description(&self) -> &str {
        "Compact memories using decay-based archival and cluster consolidation."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "strategy": { "type": "string", "default": "both", "enum": ["decay", "cluster", "both"] },
                "dryRun": { "type": "boolean", "default": false },
                "minImportance": { "type": "number", "default": 0.15 },
                "maxAgeHours": { "type": "number", "default": 24.0 },
                "userId": { "type": "string" },
                "sessionId": { "type": "string" },
                "agentId": { "type": "string" }
            }
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let strategy = arguments["strategy"].as_str().unwrap_or("both");
        let dry_run = arguments["dryRun"].as_bool().unwrap_or(false);
        let min_importance = arguments["minImportance"].as_f64().unwrap_or(0.15);
        let max_age_hours = arguments["maxAgeHours"].as_f64().unwrap_or(24.0);
        let (uid, sid, aid) = scope_from_args(arguments);

        let cutoff = chrono::Utc::now() - chrono::Duration::hours(max_age_hours as i64);
        let cutoff_str = cutoff.format("%Y-%m-%dT%H:%M:%SZ").to_string();
        let mut archived = 0i64;
        let mut merged = 0i64;

        with_db(|conn| {
            match strategy == "decay" || strategy == "both" {
                true => {
                    let rows: i64 = match dry_run {
                        true => conn.query_row(
                            "SELECT COUNT(*) FROM semantic_metadata WHERE importance < ?1 AND timestamp < ?2 AND valid_until IS NULL AND (?3 IS NULL OR user_id = ?3 OR user_id = '*') AND (?4 IS NULL OR session_id = ?4 OR session_id = '*') AND (?5 IS NULL OR agent_id = ?5 OR agent_id = '*')",
                            params![min_importance, cutoff_str, uid, sid, aid],
                            |r| r.get::<_, i64>(0),
                        )?,
                        false => conn.execute(
                            "UPDATE semantic_metadata SET valid_until = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE importance < ?1 AND timestamp < ?2 AND valid_until IS NULL AND (?3 IS NULL OR user_id = ?3 OR user_id = '*') AND (?4 IS NULL OR session_id = ?4 OR session_id = '*') AND (?5 IS NULL OR agent_id = ?5 OR agent_id = '*')",
                            params![min_importance, cutoff_str, uid, sid, aid],
                        )? as i64,
                    };
                    archived += rows;
                }
                false => {}
            }

            match strategy == "cluster" || strategy == "both" {
                true => {
                    let rows: i64 = match dry_run {
                        true => conn.query_row(
                            "SELECT COUNT(*) FROM episodic_logs WHERE created_at < ?1 AND (?2 IS NULL OR user_id = ?2 OR user_id = '*') AND (?3 IS NULL OR session_id = ?3 OR session_id = '*') AND (?4 IS NULL OR agent_id = ?4 OR agent_id = '*')",
                            params![cutoff_str, uid, sid, aid],
                            |r| r.get::<_, i64>(0),
                        )?,
                        false => conn.execute(
                            "DELETE FROM episodic_logs WHERE created_at < ?1 AND (?2 IS NULL OR user_id = ?2 OR user_id = '*') AND (?3 IS NULL OR session_id = ?3 OR session_id = '*') AND (?4 IS NULL OR agent_id = ?4 OR agent_id = '*')",
                            params![cutoff_str, uid, sid, aid],
                        )? as i64,
                    };
                    merged += rows;
                }
                false => {}
            }

            Ok(json!({
                "strategy": strategy, "dryRun": dry_run,
                "archivedFacts": archived, "consolidatedLogs": merged,
                "cutoffTimestamp": cutoff_str,
            }))
        })
    }
}

// ─── Tool: IndexCodebaseTool ──────────────────────────────────────

pub struct IndexCodebaseTool;

#[async_trait::async_trait]
impl Tool for IndexCodebaseTool {
    fn name(&self) -> &str {
        "index_codebase"
    }
    fn description(&self) -> &str {
        "Index functions, structs, enums and types in the codebase by scanning source files. Stores results for query_code_graph and analyze_code_impact."
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Directory path to scan (defaults to current directory '.')"
                },
                "user_id": { "type": "string", "description": "Optional scope identifier" },
                "session_id": { "type": "string", "description": "Optional scope identifier" },
                "agent_id": { "type": "string", "description": "Optional scope identifier" }
            }
        })
    }
    async fn call(&self, arguments: &Value) -> Result<Value> {
        let scan_path = arguments
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or(".")
            .to_string();
        let (user_id, session_id, agent_id) = scope_from_args(arguments);

        let start = std::time::Instant::now();
        let count = with_db(|conn| {
            let tx = conn.unchecked_transaction()?;
            // Clear existing data for this scope first
            tx.execute(
                "DELETE FROM code_calls WHERE caller_id IN (SELECT element_id FROM code_elements WHERE (user_id = ?1 OR user_id = '*') AND (session_id = ?2 OR session_id = '*') AND (agent_id = ?3 OR agent_id = '*'))",
                params![user_id, session_id, agent_id],
            )?;
            tx.execute(
                "DELETE FROM code_elements WHERE (user_id = ?1 OR user_id = '*') AND (session_id = ?2 OR session_id = '*') AND (agent_id = ?3 OR agent_id = '*')",
                params![user_id, session_id, agent_id],
            )?;
            let count =
                scan_and_index(Path::new(&scan_path), &user_id, &session_id, &agent_id, &tx)?;
            tx.commit()?;
            Ok(count)
        })?;

        Ok(json!({
            "status": format!("Indexed {} source files in {:?}", count, start.elapsed()),
            "path": scan_path,
            "filesIndexed": count,
        }))
    }
}

const IGNORED_SCAN_DIRS: &[&str] = &["target", ".git", "external", "node_modules", ".venv"];
const INDEXABLE_EXTENSIONS: &[&str] = &[
    "rs", "py", "js", "jsx", "ts", "tsx", "go", "rb", "java", "swift", "kt", "c", "h", "cpp", "hpp",
];

fn scan_and_index(
    dir: &Path,
    user_id: &str,
    session_id: &str,
    agent_id: &str,
    conn: &Connection,
) -> Result<i64> {
    match dir.is_dir() {
        false => return Ok(0),
        true => {}
    }
    let mut count = 0;
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        match path.is_dir() {
            true => {
                let name = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                match IGNORED_SCAN_DIRS.contains(&name.as_str()) {
                    false => {
                        count += scan_and_index(&path, user_id, session_id, agent_id, conn)?;
                    }
                    true => {}
                }
            }
            false => {
                let ext = path
                    .extension()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                let should_index = INDEXABLE_EXTENSIONS.contains(&ext.as_str())
                    && index_file(&path, user_id, session_id, agent_id, conn).is_ok();
                match should_index {
                    true => count += 1,
                    false => {}
                }
            }
        }
    }
    Ok(count)
}

use crate::tools::outline::{is_ident_continue, is_ident_start, strip_keyword_with_space};

fn read_identifier_prefix(input: &str) -> Option<&str> {
    let mut chars = input.char_indices();
    let (_, first) = chars.next()?;
    match is_ident_start(first) {
        false => return None,
        true => {}
    }
    let end = chars
        .find(|&(_, ch)| !is_ident_continue(ch))
        .map(|(idx, _)| idx)
        .unwrap_or(input.len());
    Some(&input[..end])
}

fn strip_one_modifier(input: &str) -> Option<&str> {
    let trimmed = input.trim_start();
    match trimmed.strip_prefix("pub") {
        Some(after_pub) => {
            let after_ws = after_pub.trim_start();
            match after_ws.strip_prefix('(') {
                Some(in_parens) => {
                    let close_idx = in_parens.find(')')?;
                    let after_close = &in_parens[close_idx + 1..];
                    return after_close
                        .chars()
                        .next()
                        .filter(|c| c.is_whitespace())
                        .map(|_| after_close.trim_start());
                }
                None => {}
            }
            match after_pub.chars().next().is_some_and(char::is_whitespace) {
                true => return Some(after_pub.trim_start()),
                false => {}
            }
        }
        None => {}
    }

    for kw in ["public", "export", "const", "async", "unsafe"] {
        match strip_keyword_with_space(trimmed, kw) {
            Some(rest) => return Some(rest),
            None => {}
        }
    }
    None
}

fn strip_modifiers(mut input: &str) -> &str {
    input = input.trim_start();
    while let Some(next) = strip_one_modifier(input) {
        input = next;
    }
    input
}

fn skip_angle_brackets(input: &str) -> &str {
    let trimmed = input.trim_start();
    match trimmed.starts_with('<') {
        false => input,
        true => {
            let mut depth = 0i32;
            for (idx, ch) in trimmed.char_indices() {
                match ch {
                    '<' => depth += 1,
                    '>' => {
                        depth -= 1;
                        match depth == 0 {
                            true => return &trimmed[idx + 1..],
                            false => {}
                        }
                    }
                    _ => {}
                }
            }
            input
        }
    }
}

fn read_path_identifier_prefix(input: &str) -> Option<&str> {
    let mut chars = input.char_indices();
    let (_, first) = chars.next()?;
    match is_ident_start(first) {
        false => return None,
        true => {}
    }
    let end = chars
        .find(|&(_, ch)| !(is_ident_continue(ch) || ch == ':'))
        .map(|(idx, _)| idx)
        .unwrap_or(input.len());
    Some(&input[..end])
}

fn parse_impl_declaration(trimmed: &str, after_mods: &str) -> Option<(&'static str, String, String)> {
    let after_impl = after_mods.strip_prefix("impl")?;
    let after_generics = skip_angle_brackets(after_impl);
    match after_generics.chars().next().is_some_and(char::is_whitespace) {
        false => return None,
        true => {}
    }
    let after_ws = after_generics.trim_start();
    let first_path = read_path_identifier_prefix(after_ws)?;
    let after_first = &after_ws[first_path.len()..];
    let after_first_generics = skip_angle_brackets(after_first);

    let target_ident = match after_first_generics
        .chars()
        .next()
        .is_some_and(char::is_whitespace)
    {
        true => {
            let rest_ws = after_first_generics.trim_start();
            match rest_ws.strip_prefix("for") {
                Some(after_for) => match after_for.chars().next().is_some_and(char::is_whitespace) {
                    true => {
                        let target_start = after_for.trim_start();
                        read_identifier_prefix(target_start)?
                    }
                    false => read_identifier_prefix(first_path)?,
                },
                None => read_identifier_prefix(first_path)?,
            }
        }
        false => read_identifier_prefix(first_path)?,
    };

    let end_offset = (target_ident.as_ptr() as usize)
        .saturating_sub(trimmed.as_ptr() as usize)
        .saturating_add(target_ident.len())
        .min(trimmed.len());
    let signature = trimmed[..end_offset].to_string();
    Some(("ImplBlock", format!("impl_{}", target_ident), signature))
}

const KEYWORD_DECLARATIONS: &[(&str, &str, bool)] = &[
    ("fn", "Function", true),
    ("def", "Function", true),
    ("function", "Function", true),
    ("func", "Function", true),
    ("struct", "Struct", false),
    ("enum", "Enum", false),
    ("class", "Class", false),
    ("trait", "Trait", false),
    ("interface", "Interface", false),
    ("type", "TypeAlias", false),
];

fn parse_element_declaration(trimmed: &str) -> Option<(&'static str, String, String)> {
    let after_mods = strip_modifiers(trimmed);
    for &(kw, element_type, append_parens) in KEYWORD_DECLARATIONS {
        match strip_keyword_with_space(after_mods, kw) {
            Some(after_kw) => {
                let ident = read_identifier_prefix(after_kw)?;
                let end_offset = (ident.as_ptr() as usize)
                    .saturating_sub(trimmed.as_ptr() as usize)
                    .saturating_add(ident.len())
                    .min(trimmed.len());
                let base_sig = &trimmed[..end_offset];
                let signature = match append_parens {
                    true => format!("{}(...)", base_sig),
                    false => base_sig.to_string(),
                };
                return Some((element_type, ident.to_string(), signature));
            }
            None => {}
        }
    }
    parse_impl_declaration(trimmed, after_mods)
}

fn extract_called_identifiers(line: &str) -> Vec<String> {
    let mut results = Vec::new();
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let ch = bytes[i] as char;
        match is_ident_start(ch) {
            true => {
                let start = i;
                i += 1;
                while i < bytes.len() && is_ident_continue(bytes[i] as char) {
                    i += 1;
                }
                let ident = &line[start..i];
                let after = line[i..].trim_start();
                match after.starts_with('(') {
                    true => results.push(ident.to_string()),
                    false => {}
                }
            }
            false => {
                i += 1;
            }
        }
    }
    results
}

fn compute_indent_end_line(lines: &[&str], start_idx: usize) -> i64 {
    let base_indent = lines[start_idx]
        .chars()
        .take_while(|c| c.is_whitespace())
        .count();
    let mut last_idx = start_idx;
    for (offset, line) in lines[start_idx + 1..].iter().enumerate() {
        let trimmed = line.trim();
        match trimmed.is_empty() || trimmed.starts_with('#') {
            true => continue,
            false => {}
        }
        let indent = line.chars().take_while(|c| c.is_whitespace()).count();
        match indent <= base_indent {
            true => break,
            false => last_idx = start_idx + 1 + offset,
        }
    }
    (last_idx + 1) as i64
}

fn compute_brace_end_line(lines: &[&str], start_idx: usize) -> i64 {
    let mut depth: i32 = 0;
    let mut seen_open_brace = false;
    for (offset, line) in lines[start_idx..].iter().enumerate() {
        for ch in line.chars() {
            match ch {
                '{' => {
                    depth += 1;
                    seen_open_brace = true;
                }
                '}' => {
                    depth -= 1;
                    match seen_open_brace && depth <= 0 {
                        true => return (start_idx + offset + 1) as i64,
                        false => {}
                    }
                }
                ';' => match !seen_open_brace {
                    true => return (start_idx + offset + 1) as i64,
                    false => {}
                },
                _ => {}
            }
        }
    }
    (lines.len() as i64).min((start_idx + 1) as i64 + 10)
}

fn compute_block_end_line(lines: &[&str], start_idx: usize, is_indent_lang: bool) -> i64 {
    match is_indent_lang {
        true => compute_indent_end_line(lines, start_idx),
        false => compute_brace_end_line(lines, start_idx),
    }
}

const IGNORED_CALL_KEYWORDS: &[&str] = &[
    "if", "for", "while", "match", "return", "let", "mut", "Some", "None", "Ok", "Err", "self",
    "Self", "super", "crate",
];

fn index_file(
    path: &Path,
    user_id: &str,
    session_id: &str,
    agent_id: &str,
    conn: &Connection,
) -> Result<()> {
    let content = std::fs::read_to_string(path)?;
    let relative_path = path.to_string_lossy().to_string();
    let is_indent_lang = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| ext == "py" || ext == "rb");
    let lines: Vec<&str> = content.lines().collect();
    let mut elements: Vec<(String, String, String, String, i64, i64)> = Vec::new();

    for (idx, line) in lines.iter().enumerate() {
        let line_num = (idx + 1) as i64;
        let trimmed = line.trim();
        match parse_element_declaration(trimmed) {
            Some((el_type, el_name, signature)) => {
                let el_id = format!("{}:{}:{}", relative_path, el_name, line_num);
                let end_line = compute_block_end_line(&lines, idx, is_indent_lang);
                conn.execute(
                    "INSERT OR IGNORE INTO code_elements (element_id, file_path, element_type, name, signature, ast_json, parent_id, start_line, end_line, user_id, session_id, agent_id) VALUES (?1, ?2, ?3, ?4, ?5, NULL, NULL, ?6, ?7, ?8, ?9, ?10)",
                    params![el_id, relative_path, el_type, el_name, signature, line_num, end_line, user_id, session_id, agent_id],
                )?;
                elements.push((
                    el_id,
                    el_name,
                    relative_path.clone(),
                    el_type.to_string(),
                    line_num,
                    end_line,
                ));
            }
            None => {}
        }
    }

    let known_names: HashSet<String> = elements.iter().map(|e| e.1.clone()).collect();
    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        let is_def_line = trimmed.starts_with("fn ")
            || trimmed.starts_with("pub fn ")
            || trimmed.starts_with("def ")
            || trimmed.starts_with("func ");
        match is_def_line {
            true => continue,
            false => {}
        }
        for callee_name in extract_called_identifiers(trimmed) {
            match IGNORED_CALL_KEYWORDS.contains(&callee_name.as_str())
                || !known_names.contains(&callee_name)
            {
                true => continue,
                false => {}
            }
            let line_num = (idx + 1) as i64;
            let pair = elements
                .iter()
                .find(|e| e.1 == callee_name)
                .zip(elements.iter().rfind(|e| e.4 <= line_num && line_num <= e.5));
            match pair {
                Some((callee, caller)) => {
                    conn.execute(
                        "INSERT OR IGNORE INTO code_calls (caller_id, callee_id, call_site) VALUES (?1, ?2, ?3)",
                        params![caller.0.clone(), callee.0.clone(), format!("{}:{}", relative_path, line_num)],
                    )?;
                }
                None => {}
            }
        }
    }

    Ok(())
}

// ─── QueryCodeGraphTool ─────────────────────────────────────────

pub struct QueryCodeGraphTool;

#[async_trait::async_trait]
impl Tool for QueryCodeGraphTool {
    fn name(&self) -> &str {
        "query_code_graph"
    }
    fn description(&self) -> &str {
        "Query structural elements (structs, functions, impls) and calling patterns indexed in the codebase"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Filter by file path (substring match)"
                },
                "query": {
                    "type": "string",
                    "description": "Search by name or element type (e.g. 'Struct', 'Function')"
                },
                "user_id": { "type": "string", "description": "Optional scope identifier" },
                "session_id": { "type": "string", "description": "Optional scope identifier" },
                "agent_id": { "type": "string", "description": "Optional scope identifier" }
            }
        })
    }
    async fn call(&self, arguments: &Value) -> Result<Value> {
        let file_path = arguments
            .get("file_path")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let query = arguments
            .get("query")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let (user_id, session_id, agent_id) = scope_from_args(arguments);

        let results = with_db(|conn| {
            let mut conditions = vec![
                "(user_id = ?1 OR user_id = '*')".to_string(),
                "(session_id = ?2 OR session_id = '*')".to_string(),
                "(agent_id = ?3 OR agent_id = '*')".to_string(),
            ];
            let mut param_values: Vec<String> = vec![user_id, session_id, agent_id];

            match file_path.is_empty() {
                false => {
                    conditions.push(format!("file_path LIKE ?{}", param_values.len() + 1));
                    param_values.push(format!("%{}%", file_path));
                }
                true => {}
            }
            match query.is_empty() {
                false => {
                    conditions.push(format!(
                        "(name LIKE ?{} OR element_type LIKE ?{})",
                        param_values.len() + 1,
                        param_values.len() + 1
                    ));
                    param_values.push(format!("%{}%", query));
                }
                true => {}
            }

            let sql = format!(
                "SELECT element_id, file_path, element_type, name, signature, start_line, end_line FROM code_elements WHERE {} ORDER BY file_path, start_line LIMIT 200",
                conditions.join(" AND ")
            );

            let mut stmt = conn.prepare(&sql)?;
            let mut rows = stmt.query(rusqlite::params_from_iter(
                param_values.iter().map(|s| s.as_str()),
            ))?;
            let mut items = Vec::new();
            while let Some(row) = rows.next()? {
                items.push(json!({
                    "id": row.get::<_, String>(0)?,
                    "filePath": row.get::<_, String>(1)?,
                    "elementType": row.get::<_, String>(2)?,
                    "name": row.get::<_, String>(3)?,
                    "signature": row.get::<_, String>(4)?,
                    "startLine": row.get::<_, i64>(5)?,
                    "endLine": row.get::<_, i64>(6)?,
                }));
            }
            Ok(json!(items))
        })?;

        Ok(results)
    }
}

// ─── AnalyzeCodeImpactTool ──────────────────────────────────────

pub struct AnalyzeCodeImpactTool;

#[async_trait::async_trait]
impl Tool for AnalyzeCodeImpactTool {
    fn name(&self) -> &str {
        "analyze_code_impact"
    }
    fn description(&self) -> &str {
        "Calculate downstream callers and change risk for a code symbol"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "target_symbol": {
                    "type": "string",
                    "description": "Element ID or name of the symbol to analyze (e.g. 'my_function')"
                },
                "user_id": { "type": "string", "description": "Optional scope identifier" },
                "session_id": { "type": "string", "description": "Optional scope identifier" },
                "agent_id": { "type": "string", "description": "Optional scope identifier" }
            }
        })
    }
    async fn call(&self, arguments: &Value) -> Result<Value> {
        let target_symbol = arguments
            .get("target_symbol")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        match target_symbol.is_empty() {
            true => return Ok(json!({"error": "target_symbol is required"})),
            false => {}
        }
        let (user_id, session_id, agent_id) = scope_from_args(arguments);

        with_db(|conn| {
            // Find the element - try direct ID match first, then name match
            let element = conn.query_row(
                "SELECT element_id, name, element_type, file_path FROM code_elements WHERE (element_id = ?1 OR name = ?1) AND (user_id = ?2 OR user_id = '*') AND (session_id = ?3 OR session_id = '*') AND (agent_id = ?4 OR agent_id = '*') LIMIT 1",
                params![target_symbol, user_id, session_id, agent_id],
                |row| Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                )),
            ).map_err(|_| anyhow!("Symbol '{}' not found in indexed codebase. Run index_codebase first.", target_symbol))?;

            let (element_id, element_name, element_type, file_path) = element;

            // Build reverse call graph: for each callee, collect all callers
            let mut stmt = conn.prepare(
                "SELECT caller_id, callee_id FROM code_calls WHERE caller_id IN (SELECT element_id FROM code_elements WHERE (user_id = ?1 OR user_id = '*') AND (session_id = ?2 OR session_id = '*') AND (agent_id = ?3 OR agent_id = '*'))"
            )?;
            let rows = stmt.query_map(params![user_id, session_id, agent_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;

            // Build adjacency list (callee -> callers)
            let mut reverse_graph: HashMap<String, Vec<String>> = HashMap::new();
            for r in rows {
                let (caller, callee) = r?;
                reverse_graph.entry(callee).or_default().push(caller);
            }

            // BFS from target symbol through reverse graph
            let mut visited: HashSet<String> = HashSet::new();
            let mut queue: Vec<(String, u32)> = Vec::new();
            let mut affected: Vec<String> = Vec::new();
            let mut max_depth: u32 = 0;

            queue.push((element_id.clone(), 0));
            visited.insert(element_id.clone());

            while let Some((current, depth)) = queue.pop() {
                match depth > 0 {
                    true => {
                        affected.push(current.clone());
                        max_depth = max_depth.max(depth);
                    }
                    false => {}
                }
                match reverse_graph.get(&current) {
                    Some(callers) => {
                        for caller in callers {
                            match visited.contains(caller) {
                                false => {
                                    visited.insert(caller.clone());
                                    queue.push((caller.clone(), depth + 1));
                                }
                                true => {}
                            }
                        }
                    }
                    None => {}
                }
            }

            // Risk heuristic
            let direct_callers = reverse_graph.get(&element_id).map(|v| v.len()).unwrap_or(0);
            let transitive_callers = affected.len().saturating_sub(direct_callers);
            let raw_score = 0.1 * (direct_callers as f64)
                + 0.05 * (transitive_callers as f64)
                + 0.1 * (max_depth as f64);
            let risk_score = raw_score.min(1.0);

            let details = format!(
                "Symbol '{}' ({}) in {} has {} direct callers and {} transitive callers. Maximum propagation depth: {}.",
                element_name,
                element_type,
                file_path,
                direct_callers,
                transitive_callers,
                max_depth
            );

            Ok(json!({
                "targetSymbol": element_name,
                "elementType": element_type,
                "filePath": file_path,
                "affectedSymbols": affected,
                "maxDepth": max_depth,
                "riskScore": (risk_score * 100.0).round() / 100.0,
                "details": details,
            }))
        })
    }
}

#[cfg(test)]
#[path = "codebase_tests.rs"]
mod tests;

