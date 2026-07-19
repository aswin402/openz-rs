use crate::tools::graph_memory::{scope_from_args, with_db};
use crate::tools::Tool;
use anyhow::{anyhow, Result};
use regex::Regex;
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::Path;

const STOP_WORDS: &[&str] = &[
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
        let text = arguments["text"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing 'text'"))?;
        let ratio = arguments
            .get("ratio")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.5)
            .clamp(0.0, 1.0);

        // Simple sentence splitting
        let sentences: Vec<&str> = text
            .split(['.', '!', '?'])
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();

        if sentences.is_empty() {
            return Ok(json!({
                "originalLength": text.len(),
                "compressedLength": 0,
                "ratio": ratio,
                "compressedText": "",
            }));
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
                if !seen.insert(term) {
                    continue;
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

        Ok(json!({
            "originalLength": text.len(),
            "compressedLength": compressed.len(),
            "ratio": ratio,
            "compressedText": if compressed.is_empty() { sentences[0] } else { &compressed },
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
        let snapshot = crate::tools::memory_extra::coordinator::MemoryCoordinator::default()
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
            if strategy == "decay" || strategy == "both" {
                // Archive low-importance semantic facts older than max_age
                let rows: i64 = if dry_run {
                    conn.query_row(
                        "SELECT COUNT(*) FROM semantic_metadata WHERE importance < ?1 AND timestamp < ?2 AND valid_until IS NULL AND (?3 IS NULL OR user_id = ?3 OR user_id = '*') AND (?4 IS NULL OR session_id = ?4 OR session_id = '*') AND (?5 IS NULL OR agent_id = ?5 OR agent_id = '*')",
                        params![min_importance, cutoff_str, uid, sid, aid],
                        |r| r.get::<_, i64>(0),
                    )?
                } else {
                    conn.execute(
                        "UPDATE semantic_metadata SET valid_until = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE importance < ?1 AND timestamp < ?2 AND valid_until IS NULL AND (?3 IS NULL OR user_id = ?3 OR user_id = '*') AND (?4 IS NULL OR session_id = ?4 OR session_id = '*') AND (?5 IS NULL OR agent_id = ?5 OR agent_id = '*')",
                        params![min_importance, cutoff_str, uid, sid, aid],
                    )? as i64
                };
                archived += rows;
            }

            if strategy == "cluster" || strategy == "both" {
                // Simple consolidation: expire very old episodic logs
                let rows: i64 = if dry_run {
                    conn.query_row(
                        "SELECT COUNT(*) FROM episodic_logs WHERE created_at < ?1 AND (?2 IS NULL OR user_id = ?2 OR user_id = '*') AND (?3 IS NULL OR session_id = ?3 OR session_id = '*') AND (?4 IS NULL OR agent_id = ?4 OR agent_id = '*')",
                        params![cutoff_str, uid, sid, aid],
                        |r| r.get::<_, i64>(0),
                    )?
                } else {
                    conn.execute(
                        "DELETE FROM episodic_logs WHERE created_at < ?1 AND (?2 IS NULL OR user_id = ?2 OR user_id = '*') AND (?3 IS NULL OR session_id = ?3 OR session_id = '*') AND (?4 IS NULL OR agent_id = ?4 OR agent_id = '*')",
                        params![cutoff_str, uid, sid, aid],
                    )? as i64
                };
                merged += rows;
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

fn scan_and_index(
    dir: &Path,
    user_id: &str,
    session_id: &str,
    agent_id: &str,
    conn: &Connection,
) -> Result<i64> {
    let mut count = 0;
    if !dir.is_dir() {
        return Ok(0);
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            if name != "target"
                && name != ".git"
                && name != "external"
                && name != "node_modules"
                && name != ".venv"
            {
                count += scan_and_index(&path, user_id, session_id, agent_id, conn)?;
            }
        } else {
            let ext = path
                .extension()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            if matches!(
                ext.as_str(),
                "rs" | "py"
                    | "js"
                    | "jsx"
                    | "ts"
                    | "tsx"
                    | "go"
                    | "rb"
                    | "java"
                    | "swift"
                    | "kt"
                    | "c"
                    | "h"
                    | "cpp"
                    | "hpp"
            ) {
                if let Ok(_) = index_file(&path, user_id, session_id, agent_id, conn) {
                    count += 1;
                }
            }
        }
    }
    Ok(count)
}

fn index_file(
    path: &Path,
    user_id: &str,
    session_id: &str,
    agent_id: &str,
    conn: &Connection,
) -> Result<()> {
    let content = std::fs::read_to_string(path)?;
    let relative_path = path.to_string_lossy().to_string();
    let lines: Vec<&str> = content.lines().collect();
    let mut elements: Vec<(String, String, String, String, i64, i64)> = Vec::new(); // id, type, name, signature, start, end

    // Patterns for function/struct/enum/class definitions
    let fn_re = Regex::new(r"^\s*(public\s+|pub\s+)?(async\s+)?fn\s+([a-zA-Z_]\w*)")?;
    let struct_re = Regex::new(r"^\s*(public\s+|pub\s+)?struct\s+([a-zA-Z_]\w*)")?;
    let enum_re = Regex::new(r"^\s*(public\s+|pub\s+)?enum\s+([a-zA-Z_]\w*)")?;
    let impl_re =
        Regex::new(r"^\s*(public\s+|pub\s+)?impl(?:\s+([a-zA-Z_]\w*)\s+for)?\s+([a-zA-Z_]\w*)")?;
    let def_re = Regex::new(r"^\s*def\s+([a-zA-Z_]\w*)")?;
    let class_re = Regex::new(r"^\s*class\s+([a-zA-Z_]\w*)")?;
    let func_re = Regex::new(r"^\s*func\s+([a-zA-Z_]\w*)")?;
    let type_re = Regex::new(r"^\s*type\s+([a-zA-Z_]\w*)")?;
    let trait_re = Regex::new(r"^\s*(public\s+|pub\s+)?trait\s+([a-zA-Z_]\w*)")?;
    let interface_re = Regex::new(r"^\s*interface\s+([a-zA-Z_]\w*)")?;

    for (idx, line) in lines.iter().enumerate() {
        let line_num = (idx + 1) as i64;
        let trimmed = line.trim();
        let mut element_type: Option<&str> = None;
        let mut name: Option<String> = None;
        let mut signature = String::new();

        if let Some(caps) = fn_re.captures(trimmed) {
            element_type = Some("Function");
            name = caps.get(3).or(caps.get(2)).map(|m| m.as_str().to_string());
            signature = caps
                .get(0)
                .map(|m| m.as_str().to_string() + "(...)")
                .unwrap_or_default();
        } else if let Some(caps) = def_re.captures(trimmed) {
            element_type = Some("Function");
            name = caps.get(1).map(|m| m.as_str().to_string());
            signature = caps
                .get(0)
                .map(|m| m.as_str().to_string() + "(...)")
                .unwrap_or_default();
        } else if let Some(caps) = func_re.captures(trimmed) {
            element_type = Some("Function");
            name = caps.get(1).map(|m| m.as_str().to_string());
            signature = caps
                .get(0)
                .map(|m| m.as_str().to_string() + "(...)")
                .unwrap_or_default();
        } else if let Some(caps) = struct_re.captures(trimmed) {
            element_type = Some("Struct");
            name = caps.get(2).map(|m| m.as_str().to_string());
            signature = caps
                .get(0)
                .map(|m| m.as_str().to_string())
                .unwrap_or_default();
        } else if let Some(caps) = enum_re.captures(trimmed) {
            element_type = Some("Enum");
            name = caps.get(2).map(|m| m.as_str().to_string());
            signature = caps
                .get(0)
                .map(|m| m.as_str().to_string())
                .unwrap_or_default();
        } else if let Some(caps) = impl_re.captures(trimmed) {
            element_type = Some("ImplBlock");
            name = caps.get(3).map(|m| format!("impl_{}", m.as_str()));
            signature = caps
                .get(0)
                .map(|m| m.as_str().to_string())
                .unwrap_or_default();
        } else if let Some(caps) = class_re.captures(trimmed) {
            element_type = Some("Class");
            name = caps.get(1).map(|m| m.as_str().to_string());
            signature = caps
                .get(0)
                .map(|m| m.as_str().to_string())
                .unwrap_or_default();
        } else if let Some(caps) = trait_re.captures(trimmed) {
            element_type = Some("Trait");
            name = caps.get(2).map(|m| m.as_str().to_string());
            signature = caps
                .get(0)
                .map(|m| m.as_str().to_string())
                .unwrap_or_default();
        } else if let Some(caps) = interface_re.captures(trimmed) {
            element_type = Some("Interface");
            name = caps.get(1).map(|m| m.as_str().to_string());
            signature = caps
                .get(0)
                .map(|m| m.as_str().to_string())
                .unwrap_or_default();
        } else if let Some(caps) = type_re.captures(trimmed) {
            element_type = Some("TypeAlias");
            name = caps.get(1).map(|m| m.as_str().to_string());
            signature = caps
                .get(0)
                .map(|m| m.as_str().to_string())
                .unwrap_or_default();
        }

        if let (Some(el_type), Some(el_name)) = (element_type, name) {
            let el_id = format!("{}:{}:{}", relative_path, el_name, line_num);
            let end_line = (lines.len() as i64).min(line_num + 10);
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
    }

    // Call detection: find `name(` patterns that match known element names
    let call_re = Regex::new(r"([a-zA-Z_]\w*)\s*\(")?;
    let known_names: HashSet<String> = elements.iter().map(|e| e.1.clone()).collect();
    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        // Skip definition lines themselves
        if trimmed.starts_with("fn ")
            || trimmed.starts_with("pub fn ")
            || trimmed.starts_with("def ")
            || trimmed.starts_with("func ")
        {
            continue;
        }
        for cap in call_re.captures_iter(trimmed) {
            let Some(callee_name) = cap.get(1).map(|m| m.as_str().to_string()) else {
                continue;
            };
            // Skip keywords
            if matches!(
                callee_name.as_str(),
                "if" | "for"
                    | "while"
                    | "match"
                    | "return"
                    | "let"
                    | "mut"
                    | "Some"
                    | "None"
                    | "Ok"
                    | "Err"
                    | "self"
                    | "Self"
                    | "super"
                    | "crate"
            ) {
                continue;
            }
            if known_names.contains(&callee_name) {
                // Find the callee element_id
                if let Some(callee) = elements.iter().find(|e| e.1 == callee_name) {
                    // Find nearest caller (the enclosing function/element on this line)
                    let line_num = (idx + 1) as i64;
                    if let Some(caller) = elements
                        .iter()
                        .filter(|e| e.4 <= line_num && line_num <= e.5)
                        .next_back()
                    {
                        conn.execute(
                            "INSERT OR IGNORE INTO code_calls (caller_id, callee_id, call_site) VALUES (?1, ?2, ?3)",
                            params![caller.0.clone(), callee.0.clone(), format!("{}:{}", relative_path, line_num)],
                        )?;
                    }
                }
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

            if !file_path.is_empty() {
                conditions.push(format!("file_path LIKE ?{}", param_values.len() + 1));
                param_values.push(format!("%{}%", file_path));
            }
            if !query.is_empty() {
                conditions.push(format!(
                    "(name LIKE ?{} OR element_type LIKE ?{})",
                    param_values.len() + 1,
                    param_values.len() + 1
                ));
                param_values.push(format!("%{}%", query));
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
        if target_symbol.is_empty() {
            return Ok(json!({"error": "target_symbol is required"}));
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
                if depth > 0 {
                    affected.push(current.clone());
                    if depth > max_depth {
                        max_depth = depth;
                    }
                }
                if let Some(callers) = reverse_graph.get(&current) {
                    for caller in callers {
                        if !visited.contains(caller) {
                            visited.insert(caller.clone());
                            queue.push((caller.clone(), depth + 1));
                        }
                    }
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
                element_name, element_type, file_path, direct_callers, transitive_callers, max_depth
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
