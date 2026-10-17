use crate::memory::{scope_from_args, with_graph_db as with_db};
use crate::tools::memory_extra::codebase::STOP_WORDS;
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

        match arguments.get("factId").and_then(|v| v.as_str()) {
            Some(fact_id) => {
                parameter_provided = true;
                let updated = with_db(|conn| {
                    let rows = conn.execute(
                        "UPDATE semantic_metadata SET valid_until = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
                         WHERE node_id = ?1 AND user_id = ?2 AND session_id = ?3 AND agent_id = ?4 AND valid_until IS NULL",
                        params![fact_id, uid, sid, aid],
                    )?;
                    Ok(rows > 0)
                })?;
                let msg = match updated {
                    true => format!("Semantic fact '{}' invalidated successfully", fact_id),
                    false => format!(
                        "Semantic fact '{}' not found or already invalidated",
                        fact_id
                    ),
                };
                messages.push(msg);
            }
            None => {}
        }

        match (
            arguments.get("from").and_then(|v| v.as_str()),
            arguments.get("to").and_then(|v| v.as_str()),
            arguments.get("relationType").and_then(|v| v.as_str()),
        ) {
            (Some(from), Some(to), Some(rel_type)) => {
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
                let msg = match updated {
                    true => format!(
                        "Graph relation '{}->{} ({})' invalidated",
                        from, to, rel_type
                    ),
                    false => "Graph relation not found or already invalidated".to_string(),
                };
                messages.push(msg);
            }
            _ => {}
        }

        match parameter_provided {
            false => Err(anyhow!(
                "Either factId or all of (from, to, relationType) must be provided"
            )),
            true => Ok(json!({ "status": messages.join("\n") })),
        }
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

    match changed {
        true => (kept.join("\n"), true),
        false => (text.to_string(), false),
    }
}

fn session_files_dir() -> PathBuf {
    crate::config::loader::runtime_data_dir().join("sessions")
}

pub(crate) fn forget_session_metadata_memories(query: &str) -> Result<i64> {
    let sessions_dir = session_files_dir();
    match sessions_dir.exists() {
        false => return Ok(0),
        true => {}
    }

    let mut scrubbed = 0i64;
    for entry in fs::read_dir(sessions_dir)? {
        let path = entry?.path();
        match path.extension().and_then(|s| s.to_str()) == Some("json") {
            false => continue,
            true => {}
        }

        let content = fs::read_to_string(&path)?;
        let mut value: Value = match serde_json::from_str(&content) {
            Ok(value) => value,
            Err(_) => continue,
        };

        let memory = match value
            .get_mut("metadata")
            .and_then(|metadata| metadata.get_mut("memory"))
        {
            Some(memory) => memory,
            None => continue,
        };
        let memory_text = match memory.as_str() {
            Some(text) => text,
            None => continue,
        };

        let (updated, changed) = scrub_lines_containing(memory_text, query);
        match changed {
            false => continue,
            true => {}
        }

        *memory = Value::String(updated);
        fs::write(&path, serde_json::to_string_pretty(&value)?)?;
        scrubbed += 1;
    }

    Ok(scrubbed)
}

fn scrub_skill_file(path: &Path, query: &str) -> Result<bool> {
    match path.is_file() {
        false => return Ok(false),
        true => {}
    }
    let content = fs::read_to_string(path)?;
    let (updated, changed) = scrub_lines_containing(&content, query);
    match changed {
        true => fs::write(path, updated)?,
        false => {}
    }
    Ok(changed)
}

fn forget_skill_files_in_dir(dir: &Path, query: &str) -> Result<i64> {
    match dir.exists() && dir.is_dir() {
        false => return Ok(0),
        true => {}
    }

    let mut scrubbed = 0i64;
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        let is_md_file = path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("md");
        match (is_md_file, path.is_dir()) {
            (true, _) => match scrub_skill_file(&path, query)? {
                true => scrubbed += 1,
                false => {}
            },
            (false, true) => match scrub_skill_file(&path.join("SKILL.md"), query)? {
                true => scrubbed += 1,
                false => {}
            },
            (false, false) => {}
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
        match changed {
            false => continue,
            true => {}
        }

        match (updated.trim().is_empty(), profile) {
            (true, Some(profile)) => {
                conn.execute(
                    "DELETE FROM skills WHERE name = ?1 AND profile = ?2",
                    params![name, profile],
                )?;
            }
            (true, None) => {
                conn.execute(
                    "DELETE FROM skills WHERE name = ?1 AND profile IS NULL",
                    params![name],
                )?;
            }
            (false, Some(profile)) => {
                conn.execute(
                    "UPDATE skills SET content = ?1 WHERE name = ?2 AND profile = ?3",
                    params![updated, name, profile],
                )?;
            }
            (false, None) => {
                conn.execute(
                    "UPDATE skills SET content = ?1 WHERE name = ?2 AND profile IS NULL",
                    params![updated, name],
                )?;
            }
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
        match query.len() < 3 {
            true => return Err(anyhow!("query must be at least 3 characters")),
            false => {}
        }
        match arguments["confirm"].as_bool().unwrap_or(false) {
            false => return Err(anyhow!("forget_memory requires confirm=true")),
            true => {}
        }

        let (uid, sid, aid) = scope_from_args(arguments);
        let scope = crate::tools::memory_extra::coordinator::MemoryScope::new(uid, sid, aid);
        let result = crate::tools::memory_extra::coordinator::MemoryCoordinator
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
            let sql = match relation_type {
                Some(_) => {
                    "SELECT from_name, to_name, relation_type, valid_from, valid_until
                     FROM graph_edges
                     WHERE (from_name = ?1 OR to_name = ?1)
                       AND relation_type = ?2
                       AND (user_id = ?3 OR user_id = '*')
                       AND (session_id = ?4 OR session_id = '*')
                       AND (agent_id = ?5 OR agent_id = '*')
                     ORDER BY valid_from DESC"
                }
                None => {
                    "SELECT from_name, to_name, relation_type, valid_from, valid_until
                     FROM graph_edges
                     WHERE (from_name = ?1 OR to_name = ?1)
                       AND (user_id = ?2 OR user_id = '*')
                       AND (session_id = ?3 OR session_id = '*')
                       AND (agent_id = ?4 OR agent_id = '*')
                     ORDER BY valid_from DESC"
                }
            };
            let mut stmt = conn.prepare(sql)?;
            let mut rows = match relation_type.is_some() {
                true => stmt.query(params![entity_name, relation_type, uid, sid, aid])?,
                false => stmt.query(params![entity_name, uid, sid, aid])?,
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

fn normalize_as_of_timestamp(as_of: &str) -> Result<String> {
    chrono::DateTime::parse_from_rfc3339(as_of)
        .map(|dt| dt.with_timezone(&Utc).format("%Y-%m-%dT%H:%M:%SZ").to_string())
        .or_else(|_| {
            chrono::NaiveDateTime::parse_from_str(as_of, "%Y-%m-%dT%H:%M:%SZ")
                .map(|dt| dt.format("%Y-%m-%dT%H:%M:%SZ").to_string())
        })
        .or_else(|_| {
            chrono::NaiveDateTime::parse_from_str(as_of, "%Y-%m-%d %H:%M:%S")
                .map(|dt| dt.format("%Y-%m-%dT%H:%M:%SZ").to_string())
        })
        .map_err(|_| anyhow!("Invalid datetime format: '{}'. Expected RFC3339.", as_of))
}

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
        let normalized = normalize_as_of_timestamp(as_of)?;

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

        match relation {
            Some(rel) => {
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

                match exclusive_relations.contains(&rel_type) {
                    true => {
                        let existing = with_db(|conn| -> Result<Option<String>> {
                            let mut stmt = conn.prepare(
                                "SELECT to_name FROM graph_edges
                                 WHERE from_name = ?1 AND relation_type = ?2 AND valid_until IS NULL
                                   AND (user_id = ?3 OR user_id = '*')
                                   AND (session_id = ?4 OR session_id = '*')
                                   AND (agent_id = ?5 OR agent_id = '*')",
                            )?;
                            let mut rows = stmt.query(params![from, rel_type, uid, sid, aid])?;
                            match rows.next()? {
                                Some(row) => Ok(Some(row.get(0)?)),
                                None => Ok(None),
                            }
                        })?;

                        match existing {
                            Some(old_to) => match old_to != to {
                                true => {
                                    with_db(|conn| {
                                        conn.execute(
                                            "UPDATE graph_edges SET valid_until = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
                                             WHERE from_name = ?1 AND to_name = ?2 AND relation_type = ?3 AND valid_until IS NULL
                                               AND user_id = ?4 AND session_id = ?5 AND agent_id = ?6",
                                            params![from, old_to, rel_type, uid, sid, aid],
                                        )?;
                                        Ok(())
                                    })?;

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
                                }
                                false => {
                                    return Ok(json!({
                                        "action": "no-op",
                                        "layer": "graph",
                                        "message": format!("Relation already exists: '{} {} {}'", from, rel_type, to),
                                    }));
                                }
                            },
                            None => {}
                        }
                    }
                    false => {}
                }

                let scope =
                    crate::tools::memory_extra::coordinator::MemoryScope::new(uid, sid, aid);
                let result = crate::tools::memory_extra::coordinator::MemoryCoordinator
                    .write_graph_relation(from, rel_type, to, &scope)
                    .await?;

                let action = match result.created {
                    true => "add",
                    false => "no-op",
                };
                return Ok(json!({
                    "action": action,
                    "layer": "graph",
                    "message": format!("Created relation '{}->{} ({})'", from, to, rel_type),
                }));
            }
            None => {}
        }

        match text {
            Some(t) => {
                let fts_matches = with_db(|conn| query_fts5(conn, t, 5, &uid, &sid, &aid))?;
                let best_match = fts_matches.into_iter().max_by(|a, b| {
                    let sim_a = text_similarity(t, a["rawText"].as_str().unwrap_or(""));
                    let sim_b = text_similarity(t, b["rawText"].as_str().unwrap_or(""));
                    sim_a
                        .partial_cmp(&sim_b)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });

                match best_match {
                    Some(matched) => {
                        let node_id = matched["nodeId"].as_str().unwrap_or("");
                        let existing_text = matched["rawText"].as_str().unwrap_or("");
                        let similarity = text_similarity(t, existing_text);

                        match (similarity >= 0.98, similarity >= 0.85) {
                            (true, _) => {
                                return Ok(json!({
                                    "action": "no-op",
                                    "layer": "semantic",
                                    "message": format!("Duplicate found (sim: {:.3}). No action needed.", similarity),
                                    "winnerId": node_id,
                                }));
                            }
                            (false, true) => {
                                let merged = match t.len() >= existing_text.len() {
                                    true => t.to_string(),
                                    false => existing_text.to_string(),
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
                            (false, false) => {}
                        }
                    }
                    None => {}
                }

                let scope =
                    crate::tools::memory_extra::coordinator::MemoryScope::new(uid, sid, aid);
                let written = crate::tools::memory_extra::coordinator::MemoryCoordinator
                    .write_semantic(t, -1.0, &scope)
                    .await?;

                Ok(json!({
                    "action": "add",
                    "layer": "semantic",
                    "message": format!("Added new fact '{}'", written.id),
                    "winnerId": written.id,
                }))
            }
            None => Err(anyhow!("Either text or relation must be provided")),
        }
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
        "Extract facts from text and store them in the graph memory."
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
        let coordinator = crate::tools::memory_extra::coordinator::MemoryCoordinator;
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
            match result.created {
                true => relations_created += 1,
                false => {}
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
        match sentence.is_empty() {
            true => continue,
            false => {}
        }

        let mut carried_subject: Option<String> = None;
        let mut carried_object: Option<String> = None;
        for clause in split_fact_clauses(sentence) {
            let extracted = extract_fact_clause_multi(
                &clause,
                carried_subject.as_deref(),
                carried_object.as_deref(),
            );
            match extracted.is_empty() {
                true => continue,
                false => {}
            }

            match extracted.first() {
                Some(first) => {
                    carried_subject = Some(first.from.clone());
                    carried_object = Some(first.to.clone());
                }
                None => {}
            }
            match extracted.iter().find(|fact| fact.relation == "created") {
                Some(created) => {
                    carried_object = Some(created.to.clone());
                }
                None => {}
            }

            for fact in extracted {
                let key = (
                    fact.from.to_lowercase(),
                    fact.relation.clone(),
                    fact.to.to_lowercase(),
                );
                match seen.insert(key) {
                    true => facts.push(fact),
                    false => {}
                }
            }
        }
    }

    facts
}

fn split_on_whitespace_and(part: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    for word in part.split_whitespace() {
        match word == "and" {
            true => {
                match current.is_empty() {
                    false => {
                        segments.push(current.join(" "));
                        current.clear();
                    }
                    true => {}
                }
            }
            false => current.push(word),
        }
    }
    match current.is_empty() {
        false => segments.push(current.join(" ")),
        true => {}
    }
    segments
}

fn split_fact_clauses(sentence: &str) -> Vec<String> {
    let normalized = sentence.replace([';', ','], ".");
    normalized
        .split('.')
        .flat_map(split_on_whitespace_and)
        .map(|s| s.trim().to_string())
        .filter(|part| !part.is_empty())
        .collect()
}

fn is_entity_word(word: &str) -> bool {
    let mut chars = word.chars();
    let first = match chars.next() {
        Some(ch) => ch,
        None => return false,
    };
    first.is_ascii_uppercase()
        && chars.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '+' | '#' | '.' | '-'))
}

fn is_entity_phrase(text: &str) -> bool {
    let words: Vec<&str> = text.split_whitespace().collect();
    !words.is_empty() && words.len() <= 6 && words.iter().copied().all(is_entity_word)
}

const BUILD_VERBS: &[&str] = &["created", "create", "built", "build", "wrote", "write", "made"];
const BUILD_PREPS: &[&str] = &["with", "in", "using"];
const CARRIED_BUILD_VERBS: &[&str] = &[
    "built",
    "build",
    "wrote",
    "write",
    "made",
    "implemented",
];
const PRONOUN_TARGETS: &[&str] = &["it", "that", "this"];

fn parse_built_product_clause(words: &[&str]) -> Option<Vec<ExtractedFact>> {
    let verb_idx = words
        .iter()
        .position(|w| BUILD_VERBS.contains(w))
        .filter(|&idx| idx > 0)?;
    let prep_rel_idx = words[verb_idx + 1..]
        .iter()
        .position(|w| BUILD_PREPS.contains(w))
        .filter(|&idx| idx > 0)?;
    let prep_idx = verb_idx + 1 + prep_rel_idx;
    match prep_idx + 1 < words.len() {
        false => return None,
        true => {}
    }
    let subject = words[..verb_idx].join(" ");
    let product = words[verb_idx + 1..prep_idx].join(" ");
    let tooling = words[prep_idx + 1..].join(" ");
    match is_entity_phrase(&subject) && is_entity_phrase(&product) && is_entity_phrase(&tooling) {
        false => None,
        true => {
            let mut facts = Vec::new();
            match build_extracted_fact(&subject, "created", &product) {
                Some(fact) => facts.push(fact),
                None => {}
            }
            match build_extracted_fact(&product, "built_with", &tooling) {
                Some(fact) => facts.push(fact),
                None => {}
            }
            Some(facts)
        }
    }
}

fn parse_carried_build_clause(words: &[&str], carried_object: Option<&str>) -> Option<Vec<ExtractedFact>> {
    let object = carried_object?;
    match words.len() >= 4
        && CARRIED_BUILD_VERBS.contains(&words[0])
        && PRONOUN_TARGETS.contains(&words[1])
        && BUILD_PREPS.contains(&words[2])
    {
        false => None,
        true => {
            let tooling = words[3..].join(" ");
            match is_entity_phrase(&tooling) {
                true => Some(
                    build_extracted_fact(object, "built_with", &tooling)
                        .into_iter()
                        .collect(),
                ),
                false => None,
            }
        }
    }
}

fn parse_possessive_preference_clause(words: &[&str]) -> Option<ExtractedFact> {
    let fav_idx = words
        .iter()
        .position(|&w| w == "favorite" || w == "preferred")
        .filter(|&idx| idx > 0)?;
    let cop_rel_idx = words[fav_idx + 1..]
        .iter()
        .position(|&w| w == "is" || w == "are")
        .filter(|&idx| idx > 0)?;
    let cop_idx = fav_idx + 1 + cop_rel_idx;
    match cop_idx + 1 < words.len() {
        false => return None,
        true => {}
    }
    let raw_subject = words[..fav_idx].join(" ");
    let subject = raw_subject
        .trim_end_matches("'s")
        .trim_end_matches("’s")
        .trim_end_matches('s')
        .to_string();
    let subject_candidate = raw_subject
        .strip_suffix("'s")
        .or_else(|| raw_subject.strip_suffix("’s"))
        .or_else(|| raw_subject.strip_suffix('s'))
        .unwrap_or(&subject);
    let target = words[cop_idx + 1..].join(" ");
    match is_entity_phrase(subject_candidate) && is_entity_phrase(&target) {
        true => build_extracted_fact(subject_candidate, "prefers", &target),
        false => None,
    }
}

const DIRECT_CONNECTOR_RULES: &[(&str, &str)] = &[
    (" is built with ", "built_with"),
    (" was built with ", "built_with"),
    (" built with ", "built_with"),
    (" is built in ", "built_with"),
    (" was built in ", "built_with"),
    (" built in ", "built_with"),
    (" is built using ", "built_with"),
    (" was built using ", "built_with"),
    (" built using ", "built_with"),
    (" is written with ", "built_with"),
    (" was written with ", "built_with"),
    (" written with ", "built_with"),
    (" is written in ", "built_with"),
    (" was written in ", "built_with"),
    (" written in ", "built_with"),
    (" is written using ", "built_with"),
    (" was written using ", "built_with"),
    (" written using ", "built_with"),
    (" is implemented with ", "built_with"),
    (" was implemented with ", "built_with"),
    (" implemented with ", "built_with"),
    (" is implemented in ", "built_with"),
    (" was implemented in ", "built_with"),
    (" implemented in ", "built_with"),
    (" is implemented using ", "built_with"),
    (" was implemented using ", "built_with"),
    (" implemented using ", "built_with"),
    (" is made with ", "built_with"),
    (" was made with ", "built_with"),
    (" made with ", "built_with"),
    (" is made in ", "built_with"),
    (" was made in ", "built_with"),
    (" made in ", "built_with"),
    (" is made using ", "built_with"),
    (" was made using ", "built_with"),
    (" made using ", "built_with"),
    (" lives in ", "lives_in"),
    (" live in ", "lives_in"),
    (" resides in ", "lives_in"),
    (" reside in ", "lives_in"),
    (" is using ", "uses"),
    (" are using ", "uses"),
    (" was using ", "uses"),
    (" were using ", "uses"),
    (" am using ", "uses"),
    (" uses ", "uses"),
    (" use ", "uses"),
    (" using ", "uses"),
    (" depends on ", "depends_on"),
    (" requires ", "depends_on"),
    (" require ", "depends_on"),
    (" prefers ", "prefers"),
    (" prefer ", "prefers"),
    (" likes ", "prefers"),
    (" like ", "prefers"),
    (" favors ", "prefers"),
    (" favor ", "prefers"),
    (" favours ", "prefers"),
    (" favour ", "prefers"),
    (" is a ", "is_a"),
    (" is an ", "is_a"),
    (" is the ", "is_a"),
    (" is working on ", "works_with"),
    (" are working on ", "works_with"),
    (" was working on ", "works_with"),
    (" were working on ", "works_with"),
    (" am working on ", "works_with"),
    (" works on ", "works_with"),
    (" work on ", "works_with"),
    (" working on ", "works_with"),
    (" is working with ", "works_with"),
    (" are working with ", "works_with"),
    (" was working with ", "works_with"),
    (" were working with ", "works_with"),
    (" am working with ", "works_with"),
    (" works with ", "works_with"),
    (" work with ", "works_with"),
    (" working with ", "works_with"),
    (" is working at ", "works_with"),
    (" are working at ", "works_with"),
    (" was working at ", "works_with"),
    (" were working at ", "works_with"),
    (" am working at ", "works_with"),
    (" works at ", "works_with"),
    (" work at ", "works_with"),
    (" working at ", "works_with"),
    (" is working for ", "works_with"),
    (" are working for ", "works_with"),
    (" was working for ", "works_with"),
    (" were working for ", "works_with"),
    (" am working for ", "works_with"),
    (" works for ", "works_with"),
    (" work for ", "works_with"),
    (" working for ", "works_with"),
    (" created ", "created"),
    (" create ", "created"),
    (" built ", "created"),
    (" build ", "created"),
    (" wrote ", "created"),
    (" write ", "created"),
];

const CARRIED_CONNECTOR_RULES: &[(&str, &str)] = &[
    ("uses ", "uses"),
    ("use ", "uses"),
    ("using ", "uses"),
    ("depends on ", "depends_on"),
    ("requires ", "depends_on"),
    ("require ", "depends_on"),
    ("prefers ", "prefers"),
    ("prefer ", "prefers"),
    ("likes ", "prefers"),
    ("like ", "prefers"),
    ("favors ", "prefers"),
    ("favor ", "prefers"),
    ("favours ", "prefers"),
    ("favour ", "prefers"),
    ("works on ", "works_with"),
    ("work on ", "works_with"),
    ("working on ", "works_with"),
    ("works with ", "works_with"),
    ("work with ", "works_with"),
    ("working with ", "works_with"),
    ("works at ", "works_with"),
    ("work at ", "works_with"),
    ("working at ", "works_with"),
    ("works for ", "works_with"),
    ("work for ", "works_with"),
    ("working for ", "works_with"),
];

fn extract_fact_clause_multi(
    clause: &str,
    carried_subject: Option<&str>,
    carried_object: Option<&str>,
) -> Vec<ExtractedFact> {
    let words: Vec<&str> = clause.split_whitespace().collect();
    let normalized = words.join(" ");

    match parse_built_product_clause(&words) {
        Some(facts) => return facts,
        None => {}
    }

    match parse_carried_build_clause(&words, carried_object) {
        Some(facts) => return facts,
        None => {}
    }

    extract_fact_clause(&normalized, &words, carried_subject)
        .into_iter()
        .collect()
}

fn extract_fact_clause(
    normalized: &str,
    words: &[&str],
    carried_subject: Option<&str>,
) -> Option<ExtractedFact> {
    match parse_possessive_preference_clause(words) {
        Some(fact) => return Some(fact),
        None => {}
    }

    for &(connector, relation) in DIRECT_CONNECTOR_RULES {
        match normalized.find(connector) {
            Some(idx) => {
                let left = &normalized[..idx];
                let right = &normalized[idx + connector.len()..];
                match is_entity_phrase(left) && is_entity_phrase(right) {
                    true => match build_extracted_fact(left, relation, right) {
                        Some(fact) => return Some(fact),
                        None => {}
                    },
                    false => {}
                }
            }
            None => {}
        }
    }

    let subject = carried_subject?;
    for &(prefix, relation) in CARRIED_CONNECTOR_RULES {
        match normalized.strip_prefix(prefix) {
            Some(right) => match is_entity_phrase(right) {
                true => match build_extracted_fact(subject, relation, right) {
                    Some(fact) => return Some(fact),
                    None => {}
                },
                false => {}
            },
            None => {}
        }
    }

    None
}

fn build_extracted_fact(from: &str, relation: &str, to: &str) -> Option<ExtractedFact> {
    let from = clean_entity(from);
    let to = clean_entity(to);
    match from.is_empty() || to.is_empty() || !is_valid_entity(&from) || !is_valid_entity(&to) {
        true => None,
        false => Some(ExtractedFact {
            from,
            relation: relation.to_string(),
            to,
        }),
    }
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
    !lower.is_empty()
        && STOP_WORDS.binary_search(&lower.as_str()).is_err()
        && !(word == lower && COMMON_NOUNS.binary_search(&lower.as_str()).is_ok())
}

fn keyword_match_count(keywords: &[String], text: &str) -> usize {
    keywords
        .iter()
        .filter(|kw| text.contains(kw.as_str()))
        .count()
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
            .map(|c| match c.is_alphanumeric() || c.is_whitespace() {
                true => c,
                false => ' ',
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
        match query.trim().is_empty() {
            false => {
                match with_db(|conn| query_fts5(conn, query, max_results, &uid, &sid, &aid)) {
                    Ok(fts_results) => {
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
                            match item["confidence"].as_f64() {
                                Some(c) => {
                                    item["confidence"] = json!(c.min(1.0));
                                }
                                None => {}
                            }
                            items.push(item);
                        }
                    }
                    Err(_) => {}
                }
            }
            true => {}
        }

        // 2. Graph memory via scoped keyword ranking over entities and relations.
        let graph_results = with_db(|conn| -> Result<Vec<Value>> {
            let mut results = Vec::new();

            let mut node_stmt = conn.prepare(
                "SELECT name, entity_type, observations FROM graph_nodes
                 WHERE (?1 IS NULL OR user_id = ?1 OR user_id = '*')
                   AND (?2 IS NULL OR session_id = ?2 OR session_id = '*')
                   AND (?3 IS NULL OR agent_id = ?3 OR agent_id = '*')",
            )?;
            let mut node_rows = node_stmt.query(params![uid, sid, aid])?;
            while let Some(row) = node_rows.next()? {
                let name: String = row.get(0)?;
                let entity_type: String = row.get(1)?;
                let obs_json: String = row.get(2)?;
                let observations: Vec<String> = serde_json::from_str(&obs_json).unwrap_or_default();
                let searchable =
                    format!("{} {} {}", name, entity_type, observations.join(" ")).to_lowercase();
                let match_count = keyword_match_count(&keywords, &searchable);
                match match_count > 0 {
                    true => {
                        let obs_str = match observations.is_empty() {
                            true => String::new(),
                            false => format!(" - Observations: {}", observations.join(", ")),
                        };
                        results.push(json!({
                            "kind": "entity",
                            "name": name,
                            "entityType": entity_type,
                            "observations": observations,
                            "content": format!("Entity: {} ({}){}", name, entity_type, obs_str),
                            "matchCount": match_count,
                        }));
                    }
                    false => {}
                }
            }

            let mut edge_stmt = conn.prepare(
                "SELECT from_name, relation_type, to_name FROM graph_edges
                 WHERE valid_until IS NULL
                   AND (?1 IS NULL OR user_id = ?1 OR user_id = '*')
                   AND (?2 IS NULL OR session_id = ?2 OR session_id = '*')
                   AND (?3 IS NULL OR agent_id = ?3 OR agent_id = '*')",
            )?;
            let mut edge_rows = edge_stmt.query(params![uid, sid, aid])?;
            while let Some(row) = edge_rows.next()? {
                let from_name: String = row.get(0)?;
                let relation_type: String = row.get(1)?;
                let to_name: String = row.get(2)?;
                let relation_words = relation_type.replace('_', " ");
                let searchable = format!(
                    "{} {} {} {}",
                    from_name, relation_type, relation_words, to_name
                )
                .to_lowercase();
                let match_count = keyword_match_count(&keywords, &searchable);
                match match_count > 0 {
                    true => {
                        results.push(json!({
                            "kind": "relation",
                            "from": from_name,
                            "relationType": relation_type,
                            "to": to_name,
                            "content": format!("Relation: {} -[{}]-> {}", from_name, relation_type, to_name),
                            "matchCount": match_count,
                        }));
                    }
                    false => {}
                }
            }

            Ok(results)
        })?;

        for graph_item in graph_results {
            let match_count = graph_item["matchCount"].as_u64().unwrap_or(0) as f64;
            let confidence = (0.6 + 0.1 * match_count).min(1.0);
            let metadata = match graph_item["kind"] == "relation" {
                true => json!({
                    "kind": "relation",
                    "from": graph_item["from"],
                    "relationType": graph_item["relationType"],
                    "to": graph_item["to"],
                    "matchCount": graph_item["matchCount"],
                }),
                false => json!({
                    "kind": "entity",
                    "name": graph_item["name"],
                    "entityType": graph_item["entityType"],
                    "observations": graph_item["observations"],
                    "matchCount": graph_item["matchCount"],
                }),
            };
            items.push(json!({
                "layer": "graph",
                "content": graph_item["content"],
                "confidence": confidence,
                "metadata": metadata,
            }));
        }

        // 3. Episodic reflections
        match query.trim().is_empty() {
            false => {
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
                    let confidence = match keywords.is_empty() {
                        true => 0.6,
                        false => {
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
                        }
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
            true => {}
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

#[cfg(test)]
#[path = "facts_tests.rs"]
mod tests;
