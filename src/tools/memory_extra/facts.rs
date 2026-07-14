use crate::tools::graph_memory::{scope_from_args, with_db};
use crate::tools::memory_extra::search::{query_fts5, text_similarity};
use crate::tools::memory_extra::working::semantic_embedding_blob;
use crate::tools::Tool;
use anyhow::{anyhow, Result};
use chrono::Utc;
use rusqlite::params;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

// ─── Tool: InvalidateFactTool ────────────────────────────────────

pub struct InvalidateFactTool;

#[async_trait::async_trait]
impl Tool for InvalidateFactTool {
    fn name(&self) -> &str {
        "invalidate_fact"
    }

    fn description(&self) -> &str {
        "Invalidate a graph relation (via from/to/relationType) or a semantic fact (via factId)."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "factId": { "type": "string", "description": "Semantic fact ID to invalidate" },
                "from": { "type": "string", "description": "Source entity name (for graph relation)" },
                "to": { "type": "string", "description": "Target entity name (for graph relation)" },
                "relationType": { "type": "string", "description": "Relation type (for graph relation)" },
                "userId": { "type": "string" },
                "sessionId": { "type": "string" },
                "agentId": { "type": "string" }
            }
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let (uid, sid, aid) = scope_from_args(arguments);
        let mut messages = Vec::new();
        let mut parameter_provided = false;

        if let Some(fact_id) = arguments.get("factId").and_then(|v| v.as_str()) {
            parameter_provided = true;
            let updated = with_db(|conn| {
                let rows = conn.execute(
                    "UPDATE semantic_metadata SET valid_until = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
                     WHERE node_id = ?1 AND user_id = ?2 AND session_id = ?3 AND agent_id = ?4 AND valid_until IS NULL",
                    params![fact_id, uid, sid, aid],
                )?;
                Ok(rows > 0)
            })?;
            if updated {
                messages.push(format!(
                    "Semantic fact '{}' invalidated successfully",
                    fact_id
                ));
            } else {
                messages.push(format!(
                    "Semantic fact '{}' not found or already invalidated",
                    fact_id
                ));
            }
        }

        if let (Some(from), Some(to), Some(rel_type)) = (
            arguments.get("from").and_then(|v| v.as_str()),
            arguments.get("to").and_then(|v| v.as_str()),
            arguments.get("relationType").and_then(|v| v.as_str()),
        ) {
            parameter_provided = true;
            let updated = with_db(|conn| {
                let rows = conn.execute(
                    "UPDATE graph_edges SET valid_until = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
                     WHERE from_name = ?1 AND to_name = ?2 AND relation_type = ?3 AND valid_until IS NULL
                       AND (user_id = ?4 OR user_id = '*')
                       AND (session_id = ?5 OR session_id = '*')
                       AND (agent_id = ?6 OR agent_id = '*')",
                    params![from, to, rel_type, uid, sid, aid],
                )?;
                Ok(rows > 0)
            })?;
            if updated {
                messages.push(format!(
                    "Graph relation '{}->{} ({})' invalidated",
                    from, to, rel_type
                ));
            } else {
                messages.push("Graph relation not found or already invalidated".to_string());
            }
        }

        if !parameter_provided {
            return Err(anyhow!(
                "Either factId or all of (from, to, relationType) must be provided"
            ));
        }

        Ok(json!({ "status": messages.join("\n") }))
    }
}

// ─── Tool: ForgetMemoryTool ──────────────────────────────────────

pub struct ForgetMemoryTool;

fn scrub_lines_containing(text: &str, query: &str) -> (String, bool) {
    let needle = query.to_lowercase();
    let mut changed = false;
    let kept = text
        .lines()
        .filter(|line| {
            let remove = line.to_lowercase().contains(&needle);
            changed |= remove;
            !remove
        })
        .collect::<Vec<_>>();

    if changed {
        (kept.join("\n"), true)
    } else {
        (text.to_string(), false)
    }
}

fn session_files_dir() -> PathBuf {
    crate::config::loader::runtime_data_dir().join("sessions")
}

pub(crate) fn forget_session_metadata_memories(query: &str) -> Result<i64> {
    let sessions_dir = session_files_dir();
    if !sessions_dir.exists() {
        return Ok(0);
    }

    let mut scrubbed = 0i64;
    for entry in fs::read_dir(sessions_dir)? {
        let path = entry?.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }

        let content = fs::read_to_string(&path)?;
        let mut value: Value = match serde_json::from_str(&content) {
            Ok(value) => value,
            Err(_) => continue,
        };

        let Some(memory) = value
            .get_mut("metadata")
            .and_then(|metadata| metadata.get_mut("memory"))
        else {
            continue;
        };
        let Some(memory_text) = memory.as_str() else {
            continue;
        };

        let (updated, changed) = scrub_lines_containing(memory_text, query);
        if !changed {
            continue;
        }

        *memory = Value::String(updated);
        fs::write(&path, serde_json::to_string_pretty(&value)?)?;
        scrubbed += 1;
    }

    Ok(scrubbed)
}

fn scrub_skill_file(path: &Path, query: &str) -> Result<bool> {
    if !path.is_file() {
        return Ok(false);
    }
    let content = fs::read_to_string(path)?;
    let (updated, changed) = scrub_lines_containing(&content, query);
    if changed {
        fs::write(path, updated)?;
    }
    Ok(changed)
}

fn forget_skill_files_in_dir(dir: &Path, query: &str) -> Result<i64> {
    if !dir.exists() || !dir.is_dir() {
        return Ok(0);
    }

    let mut scrubbed = 0i64;
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("md") {
            if scrub_skill_file(&path, query)? {
                scrubbed += 1;
            }
        } else if path.is_dir() && scrub_skill_file(&path.join("SKILL.md"), query)? {
            scrubbed += 1;
        }
    }

    Ok(scrubbed)
}

pub(crate) fn forget_skills(query: &str) -> Result<i64> {
    let like = format!("%{}%", query);
    let conn = crate::agent::skills::get_connection()?;
    let mut rows = Vec::new();
    {
        let mut stmt = conn.prepare(
            "SELECT name, profile, content FROM skills
             WHERE name LIKE ?1 OR content LIKE ?1",
        )?;
        let matches = stmt.query_map(params![like], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;
        for row in matches {
            rows.push(row?);
        }
    }

    let mut scrubbed = 0i64;
    for (name, profile, content) in rows {
        let (updated, changed) = scrub_lines_containing(&content, query);
        if !changed {
            continue;
        }

        if updated.trim().is_empty() {
            if let Some(profile) = profile {
                conn.execute(
                    "DELETE FROM skills WHERE name = ?1 AND profile = ?2",
                    params![name, profile],
                )?;
            } else {
                conn.execute(
                    "DELETE FROM skills WHERE name = ?1 AND profile IS NULL",
                    params![name],
                )?;
            }
        } else if let Some(profile) = profile {
            conn.execute(
                "UPDATE skills SET content = ?1 WHERE name = ?2 AND profile = ?3",
                params![updated, name, profile],
            )?;
        } else {
            conn.execute(
                "UPDATE skills SET content = ?1 WHERE name = ?2 AND profile IS NULL",
                params![updated, name],
            )?;
        }
        scrubbed += 1;
    }

    scrubbed += forget_skill_files_in_dir(&crate::agent::skills::get_skills_dir(), query)?;
    scrubbed +=
        forget_skill_files_in_dir(&crate::agent::skills::get_workspace_skills_dir(), query)?;

    Ok(scrubbed)
}

#[async_trait::async_trait]
impl Tool for ForgetMemoryTool {
    fn name(&self) -> &str {
        "forget_memory"
    }

    fn description(&self) -> &str {
        "Forget matching memory across cognitive, semantic, graph, shared, and research memory stores. Requires confirm=true."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Text, fact, entity, or marker to forget" },
                "confirm": { "type": "boolean", "description": "Must be true to perform deletion/tombstoning" },
                "userId": { "type": "string" },
                "sessionId": { "type": "string" },
                "agentId": { "type": "string" }
            },
            "required": ["query", "confirm"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let query = arguments["query"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing 'query'"))?
            .trim();
        if query.len() < 3 {
            return Err(anyhow!("query must be at least 3 characters"));
        }
        if !arguments["confirm"].as_bool().unwrap_or(false) {
            return Err(anyhow!("forget_memory requires confirm=true"));
        }

        let (uid, sid, aid) = scope_from_args(arguments);
        let scope = crate::tools::memory_extra::coordinator::MemoryScope::new(uid, sid, aid);
        let result = crate::tools::memory_extra::coordinator::MemoryCoordinator::default()
            .forget(query, &scope)
            .await?;
        Ok(result.raw)
    }
}

// ─── Tool: QueryFactHistoryTool ──────────────────────────────────

pub struct QueryFactHistoryTool;

#[async_trait::async_trait]
impl Tool for QueryFactHistoryTool {
    fn name(&self) -> &str {
        "query_fact_history"
    }

    fn description(&self) -> &str {
        "Query the chronological history of relations/facts involving a specific entity."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "entityName": { "type": "string", "description": "Name of the entity" },
                "relationType": { "type": "string", "description": "Optional relation type filter" },
                "userId": { "type": "string" },
                "sessionId": { "type": "string" },
                "agentId": { "type": "string" }
            },
            "required": ["entityName"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let entity_name = arguments["entityName"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing 'entityName'"))?;
        let relation_type = arguments.get("relationType").and_then(|v| v.as_str());
        let (uid, sid, aid) = scope_from_args(arguments);

        let results = with_db(|conn| {
            let sql = if let Some(_rel) = relation_type {
                "SELECT from_name, to_name, relation_type, valid_from, valid_until
                 FROM graph_edges
                 WHERE (from_name = ?1 OR to_name = ?1)
                   AND relation_type = ?2
                   AND (user_id = ?3 OR user_id = '*')
                   AND (session_id = ?4 OR session_id = '*')
                   AND (agent_id = ?5 OR agent_id = '*')
                 ORDER BY valid_from DESC"
            } else {
                "SELECT from_name, to_name, relation_type, valid_from, valid_until
                 FROM graph_edges
                 WHERE (from_name = ?1 OR to_name = ?1)
                   AND (user_id = ?2 OR user_id = '*')
                   AND (session_id = ?3 OR session_id = '*')
                   AND (agent_id = ?4 OR agent_id = '*')
                 ORDER BY valid_from DESC"
            };
            let mut stmt = conn.prepare(sql)?;
            let mut rows = if relation_type.is_some() {
                stmt.query(params![entity_name, relation_type, uid, sid, aid])?
            } else {
                stmt.query(params![entity_name, uid, sid, aid])?
            };
            let mut results = Vec::new();
            while let Some(row) = rows.next()? {
                results.push(json!({
                    "from": row.get::<_, String>(0)?,
                    "to": row.get::<_, String>(1)?,
                    "relationType": row.get::<_, String>(2)?,
                    "validFrom": row.get::<_, String>(3)?,
                    "validUntil": row.get::<_, Option<String>>(4)?,
                }));
            }
            Ok(results)
        })?;

        Ok(json!(results))
    }
}

// ─── Tool: QueryAsOfTool ─────────────────────────────────────────

pub struct QueryAsOfTool;

#[async_trait::async_trait]
impl Tool for QueryAsOfTool {
    fn name(&self) -> &str {
        "query_as_of"
    }

    fn description(&self) -> &str {
        "Query both Graph and Semantic memory states as of a specific point in time (ISO 8601 datetime)."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "asOf": { "type": "string", "description": "ISO 8601 datetime" },
                "userId": { "type": "string" },
                "sessionId": { "type": "string" },
                "agentId": { "type": "string" }
            },
            "required": ["asOf"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let as_of = arguments["asOf"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing 'asOf'"))?;
        let (uid, sid, aid) = scope_from_args(arguments);

        // Normalize timestamp
        let normalized = if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(as_of) {
            dt.with_timezone(&Utc)
                .format("%Y-%m-%dT%H:%M:%SZ")
                .to_string()
        } else if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(as_of, "%Y-%m-%dT%H:%M:%SZ") {
            dt.format("%Y-%m-%dT%H:%M:%SZ").to_string()
        } else if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(as_of, "%Y-%m-%d %H:%M:%S") {
            dt.format("%Y-%m-%dT%H:%M:%SZ").to_string()
        } else {
            return Err(anyhow!(
                "Invalid datetime format: '{}'. Expected RFC3339.",
                as_of
            ));
        };

        let graph_snapshot = with_db(|conn| {
            let mut entities = Vec::new();
            let mut stmt_nodes = conn.prepare(
                "SELECT name, entity_type, observations, created_at
                 FROM graph_nodes
                 WHERE created_at <= ?1
                   AND (?2 IS NULL OR user_id = ?2 OR user_id = '*')
                   AND (?3 IS NULL OR session_id = ?3 OR session_id = '*')
                   AND (?4 IS NULL OR agent_id = ?4 OR agent_id = '*')",
            )?;
            let mut node_rows = stmt_nodes.query(params![normalized, uid, sid, aid])?;
            while let Some(row) = node_rows.next()? {
                let obs_json: String = row.get(2)?;
                let observations: Vec<String> = serde_json::from_str(&obs_json).unwrap_or_default();
                entities.push(json!({
                    "name": row.get::<_, String>(0)?,
                    "entityType": row.get::<_, String>(1)?,
                    "observations": observations,
                    "createdAt": row.get::<_, String>(3)?,
                }));
            }

            let mut relations = Vec::new();
            let mut stmt_edges = conn.prepare(
                "SELECT from_name, to_name, relation_type, valid_from, valid_until
                 FROM graph_edges
                 WHERE valid_from <= ?1
                   AND (valid_until IS NULL OR valid_until > ?1)
                   AND (?2 IS NULL OR user_id = ?2 OR user_id = '*')
                   AND (?3 IS NULL OR session_id = ?3 OR session_id = '*')
                   AND (?4 IS NULL OR agent_id = ?4 OR agent_id = '*')",
            )?;
            let mut edge_rows = stmt_edges.query(params![normalized, uid, sid, aid])?;
            while let Some(row) = edge_rows.next()? {
                relations.push(json!({
                    "from": row.get::<_, String>(0)?,
                    "to": row.get::<_, String>(1)?,
                    "relationType": row.get::<_, String>(2)?,
                    "validFrom": row.get::<_, String>(3)?,
                    "validUntil": row.get::<_, Option<String>>(4)?,
                }));
            }
            Ok(json!({ "entities": entities, "relations": relations }))
        })?;

        let semantic_snapshot = with_db(|conn| {
            let mut stmt = conn.prepare(
                "SELECT node_id, raw_text, timestamp, importance
                 FROM semantic_metadata
                 WHERE valid_from <= ?1
                   AND (valid_until IS NULL OR valid_until > ?1)
                   AND (?2 IS NULL OR user_id = ?2 OR user_id = '*')
                   AND (?3 IS NULL OR session_id = ?3 OR session_id = '*')
                   AND (?4 IS NULL OR agent_id = ?4 OR agent_id = '*')",
            )?;
            let mut rows = stmt.query(params![normalized, uid, sid, aid])?;
            let mut facts = Vec::new();
            while let Some(row) = rows.next()? {
                facts.push(json!({
                    "nodeId": row.get::<_, String>(0)?,
                    "rawText": row.get::<_, String>(1)?,
                    "timestamp": row.get::<_, String>(2)?,
                    "importance": row.get::<_, f64>(3)?,
                }));
            }
            Ok(facts)
        })?;

        Ok(json!({
            "graph": graph_snapshot,
            "semantic": semantic_snapshot,
        }))
    }
}

// ─── SmartStoreTool (dedup + merge aware store) ──────────────────

pub struct SmartStoreTool;

#[async_trait::async_trait]
impl Tool for SmartStoreTool {
    fn name(&self) -> &str {
        "smart_store"
    }

    fn description(&self) -> &str {
        "Intelligently store or merge memories in Semantic and Graph layers using deduplication and decision logic."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "text": { "type": "string", "description": "Text statement to store in semantic memory" },
                "relation": {
                    "type": "object",
                    "properties": {
                        "from": { "type": "string" },
                        "to": { "type": "string" },
                        "relationType": { "type": "string" }
                    },
                    "description": "Graph relation to store"
                },
                "userId": { "type": "string" },
                "sessionId": { "type": "string" },
                "agentId": { "type": "string" }
            }
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let text = arguments.get("text").and_then(|v| v.as_str());
        let relation = arguments.get("relation");
        let (uid, sid, aid) = scope_from_args(arguments);

        // Handle relation input (Graph Layer)
        if let Some(rel) = relation {
            let from = rel["from"]
                .as_str()
                .ok_or_else(|| anyhow!("Relation missing 'from'"))?;
            let to = rel["to"]
                .as_str()
                .ok_or_else(|| anyhow!("Relation missing 'to'"))?;
            let rel_type = rel["relationType"]
                .as_str()
                .ok_or_else(|| anyhow!("Relation missing 'relationType'"))?;

            let exclusive_relations = [
                "lives_in",
                "current_job",
                "spouse",
                "has_status",
                "is_born_in",
                "located_in",
            ];

            if exclusive_relations.contains(&rel_type) {
                // Check for existing relation and supersede if needed
                let existing = with_db(|conn| -> Result<Option<String>> {
                    let mut stmt = conn.prepare(
                        "SELECT to_name FROM graph_edges
                         WHERE from_name = ?1 AND relation_type = ?2 AND valid_until IS NULL
                           AND (user_id = ?3 OR user_id = '*')
                           AND (session_id = ?4 OR session_id = '*')
                           AND (agent_id = ?5 OR agent_id = '*')",
                    )?;
                    let mut rows = stmt.query(params![from, rel_type, uid, sid, aid])?;
                    if let Some(row) = rows.next()? {
                        Ok(Some(row.get(0)?))
                    } else {
                        Ok(None)
                    }
                })?;

                if let Some(old_to) = existing {
                    if old_to != to {
                        with_db(|conn| {
                            conn.execute(
                                "UPDATE graph_edges SET valid_until = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
                                 WHERE from_name = ?1 AND to_name = ?2 AND relation_type = ?3 AND valid_until IS NULL
                                   AND user_id = ?4 AND session_id = ?5 AND agent_id = ?6",
                                params![from, old_to, rel_type, uid, sid, aid],
                            )?;
                            Ok(())
                        })?;

                        // Insert new edge via graph_memory tables directly
                        with_db(|conn| {
                            conn.execute(
                                "INSERT INTO graph_edges (from_name, to_name, relation_type, user_id, session_id, agent_id)
                                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                                params![from, to, rel_type, uid, sid, aid],
                            )?;
                            Ok(())
                        })?;

                        return Ok(json!({
                            "action": "superseded",
                            "layer": "graph",
                            "message": format!("Superseded '{} {} {}' -> '{} {} {}'", from, rel_type, old_to, to, rel_type, from),
                        }));
                    } else {
                        return Ok(json!({
                            "action": "no-op",
                            "layer": "graph",
                            "message": format!("Relation already exists: '{} {} {}'", from, rel_type, to),
                        }));
                    }
                }
            }

            let scope = crate::tools::memory_extra::coordinator::MemoryScope::new(uid, sid, aid);
            let result = crate::tools::memory_extra::coordinator::MemoryCoordinator::default()
                .write_graph_relation(from, rel_type, to, &scope)
                .await?;

            return Ok(json!({
                "action": if result.created { "add" } else { "no-op" },
                "layer": "graph",
                "message": format!("Created relation '{}->{} ({})'", from, to, rel_type),
            }));
        }

        // Handle text input (Semantic Layer)
        if let Some(t) = text {
            // Check for duplicates via FTS5
            let fts_matches = with_db(|conn| query_fts5(conn, t, 5, &uid, &sid, &aid))?;
            let best_match = fts_matches.into_iter().max_by(|a, b| {
                let sim_a = text_similarity(t, a["rawText"].as_str().unwrap_or(""));
                let sim_b = text_similarity(t, b["rawText"].as_str().unwrap_or(""));
                sim_a
                    .partial_cmp(&sim_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            if let Some(matched) = best_match {
                let node_id = matched["nodeId"].as_str().unwrap_or("");
                let existing_text = matched["rawText"].as_str().unwrap_or("");
                let similarity = text_similarity(t, existing_text);

                if similarity >= 0.98 {
                    return Ok(json!({
                        "action": "no-op",
                        "layer": "semantic",
                        "message": format!("Duplicate found (sim: {:.3}). No action needed.", similarity),
                        "winnerId": node_id,
                    }));
                } else if similarity >= 0.85 {
                    // Merge: keep the longer text
                    let merged = if t.len() >= existing_text.len() {
                        t.to_string()
                    } else {
                        existing_text.to_string()
                    };
                    let merged_embedding = semantic_embedding_blob(&merged);
                    with_db(|conn| {
                        conn.execute(
                            "UPDATE semantic_metadata SET raw_text = ?1, embedding = ?2 WHERE node_id = ?3 AND user_id = ?4 AND session_id = ?5 AND agent_id = ?6",
                            params![merged, merged_embedding, node_id, uid, sid, aid],
                        )?;
                        let _ = conn.execute(
                            "UPDATE semantic_fts SET raw_text = ?1 WHERE node_id = ?2",
                            params![merged, node_id],
                        );
                        Ok(())
                    })?;

                    return Ok(json!({
                        "action": "merge",
                        "layer": "semantic",
                        "message": format!("Merged with existing fact '{}'", node_id),
                        "winnerId": node_id,
                    }));
                }
            }

            let scope = crate::tools::memory_extra::coordinator::MemoryScope::new(uid, sid, aid);
            let written = crate::tools::memory_extra::coordinator::MemoryCoordinator::default()
                .write_semantic(t, -1.0, &scope)
                .await?;

            return Ok(json!({
                "action": "add",
                "layer": "semantic",
                "message": format!("Added new fact '{}'", written.id),
                "winnerId": written.id,
            }));
        }

        Err(anyhow!("Either text or relation must be provided"))
    }
}

// ─── Tool: ExtractAndStoreFactsTool ──────────────────────────────

pub struct ExtractAndStoreFactsTool;

#[async_trait::async_trait]
impl Tool for ExtractAndStoreFactsTool {
    fn name(&self) -> &str {
        "extract_and_store_facts"
    }

    fn description(&self) -> &str {
        "Extract facts from text using regular expressions and store them in the graph memory."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "text": { "type": "string", "description": "Text to extract facts from" },
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
        let (uid, sid, aid) = scope_from_args(arguments);

        let facts = extract_facts(text);
        let facts_extracted = facts.len();

        let mut entities_created = 0;
        let mut relations_created = 0;

        let scope = crate::tools::memory_extra::coordinator::MemoryScope::new(uid, sid, aid);
        let coordinator = crate::tools::memory_extra::coordinator::MemoryCoordinator::default();
        for fact in facts {
            let before_nodes = with_db(|conn| {
                let count = conn.query_row(
                    "SELECT COUNT(*) FROM graph_nodes WHERE (name = ?1 OR name = ?2) AND user_id = ?3 AND session_id = ?4 AND agent_id = ?5",
                    params![fact.from, fact.to, scope.user_id, scope.session_id, scope.agent_id],
                    |row| row.get::<_, i64>(0),
                )?;
                Ok(count)
            })?;

            let result = coordinator
                .write_graph_relation(&fact.from, &fact.relation, &fact.to, &scope)
                .await?;

            let after_nodes = with_db(|conn| {
                let count = conn.query_row(
                    "SELECT COUNT(*) FROM graph_nodes WHERE (name = ?1 OR name = ?2) AND user_id = ?3 AND session_id = ?4 AND agent_id = ?5",
                    params![fact.from, fact.to, scope.user_id, scope.session_id, scope.agent_id],
                    |row| row.get::<_, i64>(0),
                )?;
                Ok(count)
            })?;
            entities_created += (after_nodes - before_nodes).max(0) as i64;
            if result.created {
                relations_created += 1;
            }
        }

        Ok(json!({
            "factsExtracted": facts_extracted,
            "entitiesCreated": entities_created,
            "relationsCreated": relations_created,
            "status": format!("Extracted {} facts from text, created {} entities and {} relations", facts_extracted, entities_created, relations_created),
        }))
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct ExtractedFact {
    pub(crate) from: String,
    pub(crate) relation: String,
    pub(crate) to: String,
}

pub(crate) fn extract_facts(text: &str) -> Vec<ExtractedFact> {
    let mut facts = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for sentence in text.split(['.', '!', '?']) {
        let sentence = sentence.trim();
        if sentence.is_empty() {
            continue;
        }

        let mut carried_subject: Option<String> = None;
        for clause in split_fact_clauses(sentence) {
            if let Some(fact) = extract_fact_clause(&clause, carried_subject.as_deref()) {
                carried_subject = Some(fact.from.clone());
                let key = (
                    fact.from.to_lowercase(),
                    fact.relation.clone(),
                    fact.to.to_lowercase(),
                );
                if seen.insert(key) {
                    facts.push(fact);
                }
            }
        }
    }

    facts
}

fn split_fact_clauses(sentence: &str) -> Vec<String> {
    let normalized = sentence.replace(';', ".").replace(',', ".");
    let and_split = regex::Regex::new(r"\s+and\s+").unwrap();
    normalized
        .split('.')
        .flat_map(|part| and_split.split(part))
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn entity_pattern() -> &'static str {
    r"[A-Z][A-Za-z0-9_+#.-]*(?:\s+[A-Z][A-Za-z0-9_+#.-]*){0,5}"
}

fn extract_fact_clause(clause: &str, carried_subject: Option<&str>) -> Option<ExtractedFact> {
    let entity = entity_pattern();
    let direct_patterns = [
        (
            format!(r"^({entity})'?s\s+(?:favorite|preferred)\s+.+?\s+(?:is|are)\s+({entity})$"),
            "prefers",
        ),
        (
            format!(
                r"^({entity})\s+(?:is\s+|was\s+)?(?:built|written|implemented|made)\s+(?:with|in|using)\s+({entity})$"
            ),
            "built_with",
        ),
        (
            format!(r"^({entity})\s+(?:lives?|resides?)\s+in\s+({entity})$"),
            "lives_in",
        ),
        (
            format!(r"^({entity})\s+(?:(?:is|are|was|were|am)\s+)?(?:uses?|using)\s+({entity})$"),
            "uses",
        ),
        (
            format!(r"^({entity})\s+(?:depends\s+on|requires?)\s+({entity})$"),
            "depends_on",
        ),
        (
            format!(r"^({entity})\s+(?:prefers?|likes?|favou?rs?)\s+({entity})$"),
            "prefers",
        ),
        (
            format!(r"^({entity})\s+(?:is\s+a|is\s+an|is\s+the)\s+({entity})$"),
            "is_a",
        ),
        (
            format!(
                r"^({entity})\s+(?:(?:is|are|was|were|am)\s+)?(?:works?|working)\s+(?:on|with|at|for)\s+({entity})$"
            ),
            "works_with",
        ),
        (
            format!(r"^({entity})\s+(?:created?|built?|wrote?)\s+({entity})$"),
            "created",
        ),
    ];

    for (pattern, relation) in direct_patterns {
        let regex = regex::Regex::new(&pattern).unwrap();
        if let Some(caps) = regex.captures(clause) {
            return build_extracted_fact(caps.get(1)?.as_str(), relation, caps.get(2)?.as_str());
        }
    }

    let subject = carried_subject?;
    let carried_patterns = [
        (format!(r"^(?:uses?|using)\s+({entity})$"), "uses"),
        (
            format!(r"^(?:depends\s+on|requires?)\s+({entity})$"),
            "depends_on",
        ),
        (
            format!(r"^(?:prefers?|likes?|favou?rs?)\s+({entity})$"),
            "prefers",
        ),
        (
            format!(r"^(?:works?|working)\s+(?:on|with|at|for)\s+({entity})$"),
            "works_with",
        ),
    ];

    for (pattern, relation) in carried_patterns {
        let regex = regex::Regex::new(&pattern).unwrap();
        if let Some(caps) = regex.captures(clause) {
            return build_extracted_fact(subject, relation, caps.get(1)?.as_str());
        }
    }

    None
}

fn build_extracted_fact(from: &str, relation: &str, to: &str) -> Option<ExtractedFact> {
    let from = clean_entity(from);
    let to = clean_entity(to);
    if from.is_empty() || to.is_empty() || !is_valid_entity(&from) || !is_valid_entity(&to) {
        return None;
    }
    Some(ExtractedFact {
        from,
        relation: relation.to_string(),
        to,
    })
}

fn clean_entity(value: &str) -> String {
    value
        .trim()
        .trim_matches(|c: char| {
            !c.is_ascii_alphanumeric() && c != '+' && c != '#' && c != '.' && c != '-'
        })
        .trim_end_matches("'s")
        .trim_end_matches("’s")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

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

const COMMON_NOUNS: &[&str] = &[
    "app",
    "application",
    "code",
    "compiler",
    "computer",
    "database",
    "db",
    "developer",
    "engine",
    "engineer",
    "file",
    "framework",
    "hardware",
    "interpreter",
    "job",
    "language",
    "library",
    "machine",
    "program",
    "programmer",
    "project",
    "server",
    "software",
    "system",
    "thing",
    "things",
    "tool",
    "user",
    "work",
];

fn is_valid_entity(word: &str) -> bool {
    let lower = word.to_lowercase();
    if lower.is_empty() {
        return false;
    }
    if STOP_WORDS.binary_search(&lower.as_str()).is_ok() {
        return false;
    }
    if word == lower && COMMON_NOUNS.binary_search(&lower.as_str()).is_ok() {
        return false;
    }
    true
}

// ─── Tool: ProactiveRecallTool ───────────────────────────────────

pub struct ProactiveRecallTool;

#[async_trait::async_trait]
impl Tool for ProactiveRecallTool {
    fn name(&self) -> &str {
        "proactive_recall"
    }

    fn description(&self) -> &str {
        "Recall contextually relevant memories across semantic, graph, and episodic layers given a query context."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Search context" },
                "maxResults": { "type": "integer", "default": 10 },
                "userId": { "type": "string" },
                "sessionId": { "type": "string" },
                "agentId": { "type": "string" }
            },
            "required": ["query"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let query = arguments["query"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing 'query'"))?;
        let max_results = arguments
            .get("maxResults")
            .and_then(|v| v.as_u64())
            .unwrap_or(10) as usize;
        let (uid, sid, aid) = scope_from_args(arguments);

        // Extract keywords
        let keywords: Vec<String> = query
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c.is_whitespace() {
                    c
                } else {
                    ' '
                }
            })
            .collect::<String>()
            .split_whitespace()
            .map(|w| w.to_lowercase())
            .filter(|w| w.len() >= 3 && STOP_WORDS.binary_search(&w.as_str()).is_err())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        let mut items: Vec<Value> = Vec::new();

        // 1. Semantic memory via FTS5
        if !query.trim().is_empty() {
            if let Ok(fts_results) =
                with_db(|conn| query_fts5(conn, query, max_results, &uid, &sid, &aid))
            {
                for fact in fts_results {
                    let mut item = json!({
                        "layer": "semantic",
                        "content": fact["rawText"],
                        "confidence": 0.85,
                        "metadata": {
                            "nodeId": fact["nodeId"],
                            "timestamp": fact["timestamp"],
                            "importance": fact["importance"],
                        }
                    });
                    if let Some(c) = item["confidence"].as_f64() {
                        item["confidence"] = json!(c.min(1.0));
                    }
                    items.push(item);
                }
            }
        }

        // 2. Graph memory via LIKE search
        let query_pattern = format!("%{}%", query.to_lowercase());
        let graph_results = with_db(|conn| -> Result<Vec<Value>> {
            let mut entities = Vec::new();
            let mut stmt = conn.prepare(
                "SELECT name, entity_type, observations FROM graph_nodes
                 WHERE (LOWER(name) LIKE ?1 OR LOWER(entity_type) LIKE ?1 OR LOWER(observations) LIKE ?1)
                   AND (?2 IS NULL OR user_id = ?2 OR user_id = '*')
                   AND (?3 IS NULL OR session_id = ?3 OR session_id = '*')
                   AND (?4 IS NULL OR agent_id = ?4 OR agent_id = '*')"
            )?;
            let mut rows = stmt.query(params![query_pattern, uid, sid, aid])?;
            while let Some(row) = rows.next()? {
                let name: String = row.get(0)?;
                let entity_type: String = row.get(1)?;
                let obs_json: String = row.get(2)?;
                let observations: Vec<String> = serde_json::from_str(&obs_json).unwrap_or_default();
                let obs_str = if observations.is_empty() {
                    String::new()
                } else {
                    format!(" - Observations: {}", observations.join(", "))
                };
                entities.push(json!({
                    "name": name,
                    "entityType": entity_type,
                    "observations": observations,
                    "content": format!("Entity: {} ({}){}", name, entity_type, obs_str),
                }));
            }
            Ok(entities)
        })?;

        for entity in graph_results {
            let confidence = if keywords.is_empty() {
                0.6
            } else {
                let lower_name = entity["name"].as_str().unwrap_or("").to_lowercase();
                let lower_type = entity["entityType"].as_str().unwrap_or("").to_lowercase();
                let obs_text: String = entity["observations"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_str())
                            .collect::<Vec<_>>()
                            .join(" ")
                    })
                    .unwrap_or_default()
                    .to_lowercase();
                let match_count = keywords
                    .iter()
                    .filter(|kw| {
                        lower_name.contains(kw.as_str())
                            || lower_type.contains(kw.as_str())
                            || obs_text.contains(kw.as_str())
                    })
                    .count();
                (0.6 + 0.1 * match_count as f64).min(1.0)
            };
            items.push(json!({
                "layer": "graph",
                "content": entity["content"],
                "confidence": confidence,
                "metadata": {
                    "name": entity["name"],
                    "entityType": entity["entityType"],
                    "observations": entity["observations"],
                }
            }));
        }

        // 3. Episodic reflections
        if !query.trim().is_empty() {
            let pattern = format!("%{}%", query);
            let reflections = with_db(|conn| -> Result<Vec<Value>> {
                let mut stmt = conn.prepare(
                    "SELECT id, task_description, status, attempt_number, reflection, root_cause, solution_applied, created_at
                     FROM reflection_memory
                     WHERE (task_description LIKE ?1 OR reflection LIKE ?1 OR root_cause LIKE ?1)
                       AND (?2 IS NULL OR user_id = ?2 OR user_id = '*')
                       AND (?3 IS NULL OR session_id = ?3 OR session_id = '*')
                       AND (?4 IS NULL OR agent_id = ?4 OR agent_id = '*')
                     ORDER BY created_at DESC"
                )?;
                let mut rows = stmt.query(params![pattern, uid, sid, aid])?;
                let mut results = Vec::new();
                while let Some(row) = rows.next()? {
                    results.push(json!({
                        "id": row.get::<_, String>(0)?,
                        "taskDescription": row.get::<_, String>(1)?,
                        "status": row.get::<_, String>(2)?,
                        "attemptNumber": row.get::<_, i64>(3)?,
                        "reflection": row.get::<_, String>(4)?,
                        "rootCause": row.get::<_, Option<String>>(5)?,
                        "solutionApplied": row.get::<_, Option<String>>(6)?,
                        "createdAt": row.get::<_, String>(7)?,
                    }));
                }
                Ok(results)
            })?;

            for r in reflections {
                let confidence = if keywords.is_empty() {
                    0.6
                } else {
                    let text_to_check = format!(
                        "{} {} {} {}",
                        r["taskDescription"].as_str().unwrap_or(""),
                        r["reflection"].as_str().unwrap_or(""),
                        r["rootCause"].as_str().unwrap_or(""),
                        r["solutionApplied"].as_str().unwrap_or(""),
                    )
                    .to_lowercase();
                    let match_count = keywords
                        .iter()
                        .filter(|kw| text_to_check.contains(kw.as_str()))
                        .count();
                    (0.6 + 0.1 * match_count as f64).min(1.0)
                };
                items.push(json!({
                    "layer": "episodic",
                    "content": format!(
                        "Reflection on '{}' (Status: {}) | {} | Root Cause: {} | Solution: {}",
                        r["taskDescription"],
                        r["status"],
                        r["reflection"],
                        r["rootCause"].as_str().unwrap_or("None"),
                        r["solutionApplied"].as_str().unwrap_or("None"),
                    ),
                    "confidence": confidence,
                    "metadata": {
                        "id": r["id"],
                        "taskDescription": r["taskDescription"],
                        "status": r["status"],
                        "createdAt": r["createdAt"],
                    }
                }));
            }
        }

        // Sort by confidence descending
        items.sort_by(|a, b| {
            let ca = a["confidence"].as_f64().unwrap_or(0.0);
            let cb = b["confidence"].as_f64().unwrap_or(0.0);
            cb.partial_cmp(&ca).unwrap_or(std::cmp::Ordering::Equal)
        });
        items.truncate(max_results);

        Ok(json!(items))
    }
}
