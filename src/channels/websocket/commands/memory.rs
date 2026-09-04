//! Cognitive-memory WebSocket query and response builder.

use serde_json::Value;

pub(crate) async fn cognitive_memory_event() -> Value {
    let mut entities_count = 0i64;
    let mut relations_count = 0i64;
    let mut facts_count = 0i64;
    let working_memory_keys = crate::tools::memory_extra::working::active_working_memory_keys();
    let mut nodes: Vec<Value> = Vec::new();
    let mut edges: Vec<Value> = Vec::new();
    let mut facts: Vec<Value> = Vec::new();

    // graph_memory.db uses graph_nodes and graph_edges, plus code elements & calls.
    let graph_db = crate::config::loader::runtime_db_path("graph_memory.db");
    if graph_db.exists() {
        if let Ok(conn) = rusqlite::Connection::open(&graph_db) {
            let _ = conn
                .query_row("SELECT COUNT(*) FROM graph_nodes", [], |row| row.get(0))
                .map(|count: i64| entities_count = count);
            let _ = conn
                .query_row("SELECT COUNT(*) FROM graph_edges", [], |row| row.get(0))
                .map(|count: i64| relations_count = count);

            // Fetch every active graph node. The UI can filter locally and
            // should not silently lose older entities because of an endpoint limit.
            if let Ok(mut statement) =
                conn.prepare("SELECT name, entity_type, observations FROM graph_nodes")
            {
                if let Ok(rows) = statement.query_map([], |row| {
                    Ok(serde_json::json!({
                        "name": row.get::<_, String>(0)?,
                        "entity_type": row.get::<_, String>(1)?,
                        "observations": row.get::<_, String>(2)?,
                    }))
                }) {
                    nodes = rows.filter_map(|row| row.ok()).collect();
                }
            }

            // Fetch every active relation; expired historical edges are excluded.
            if let Ok(mut statement) = conn.prepare(
                "SELECT from_name, to_name, relation_type, confidence, valid_from
                 FROM graph_edges WHERE valid_until IS NULL",
            ) {
                if let Ok(rows) = statement.query_map([], |row| {
                    Ok(serde_json::json!({
                        "from_name": row.get::<_, String>(0)?,
                        "to_name": row.get::<_, String>(1)?,
                        "relation_type": row.get::<_, String>(2)?,
                        "confidence": row.get::<_, f64>(3)?,
                        "valid_from": row.get::<_, String>(4)?,
                    }))
                }) {
                    edges = rows.filter_map(|row| row.ok()).collect();
                }
            }

            // Fetch code elements if available.
            if let Ok(mut statement) = conn.prepare(
                "SELECT name, element_type, file_path, signature, start_line, end_line
                 FROM code_elements ORDER BY file_path, start_line",
            ) {
                if let Ok(rows) = statement.query_map([], |row| {
                    let name: String = row.get(0)?;
                    let element_type: String = row.get(1)?;
                    let file_path: String = row.get(2)?;
                    let signature: String = row.get(3)?;
                    let start_line: i64 = row.get(4)?;
                    let end_line: i64 = row.get(5)?;
                    Ok(serde_json::json!({
                        "name": name,
                        "entity_type": if element_type.is_empty() { "code" } else { element_type.as_str() },
                        "observations": format!(
                            "File: {} | Signature: {} | Lines: {}-{}",
                            file_path, signature, start_line, end_line
                        ),
                    }))
                }) {
                    nodes.extend(rows.filter_map(|row| row.ok()));
                }
            }

            // Fetch code calls if available.
            if let Ok(mut statement) = conn.prepare(
                "SELECT caller.name, callee.name, COALESCE(cc.call_site, 'calls')
                 FROM code_calls cc
                 JOIN code_elements caller ON caller.element_id = cc.caller_id
                 JOIN code_elements callee ON callee.element_id = cc.callee_id",
            ) {
                if let Ok(rows) = statement.query_map([], |row| {
                    Ok(serde_json::json!({
                        "from_name": row.get::<_, String>(0)?,
                        "to_name": row.get::<_, String>(1)?,
                        "relation_type": row.get::<_, String>(2)?,
                    }))
                }) {
                    edges.extend(rows.filter_map(|row| row.ok()));
                }
            }
        }
    }

    // memory.db uses cognitive_memory for stored facts/memories, plus skills
    // and bookmarks.
    let memory_db = crate::config::loader::runtime_db_path("memory.db");
    if memory_db.exists() {
        if let Ok(conn) = rusqlite::Connection::open(&memory_db) {
            let _ = conn
                .query_row("SELECT COUNT(*) FROM cognitive_memory", [], |row| row.get(0))
                .map(|count: i64| facts_count = count);

            // Skills are stored in the memory DB without a trigger column.
            if let Ok(mut statement) = conn.prepare(
                "SELECT name, content, profile, use_count FROM skills",
            ) {
                if let Ok(rows) = statement.query_map([], |row| {
                    let name: String = row.get(0)?;
                    let content: String = row.get(1).unwrap_or_default();
                    let profile: Option<String> = row.get(2).unwrap_or(None);
                    let use_count: i64 = row.get(3).unwrap_or(0);
                    Ok((name, content, profile, use_count))
                }) {
                    for (name, content, profile, use_count) in rows.filter_map(|row| row.ok()) {
                        nodes.push(serde_json::json!({
                            "name": name,
                            "entity_type": "skill",
                            "observations": format!(
                                "Profile: {} | Uses: {} | Content: {}",
                                profile.unwrap_or_else(|| "global".to_string()),
                                use_count,
                                content.chars().take(160).collect::<String>()
                            ),
                        }));
                    }
                }
            }

            // Fetch source bookmarks using the current memory schema.
            if let Ok(mut statement) = conn.prepare(
                "SELECT label, kind, uri, aliases, summary FROM source_bookmarks",
            ) {
                if let Ok(rows) = statement.query_map([], |row| {
                    let label: String = row.get(0)?;
                    let kind: String = row.get(1).unwrap_or_default();
                    let uri: String = row.get(2)?;
                    let aliases: String = row.get(3).unwrap_or_default();
                    let summary: String = row.get(4).unwrap_or_default();
                    Ok(serde_json::json!({
                        "name": label,
                        "entity_type": "link",
                        "observations": format!(
                            "Kind: {} | URI: {} | Aliases: {} | Summary: {}",
                            kind, uri, aliases, summary
                        ),
                    }))
                }) {
                    nodes.extend(rows.filter_map(|row| row.ok()));
                }
            }

            // Fetch every stored fact, newest first.
            if let Ok(mut statement) = conn.prepare(
                "SELECT text, timestamp, tags, importance FROM cognitive_memory ORDER BY timestamp DESC",
            ) {
                if let Ok(rows) = statement.query_map([], |row| {
                    Ok(serde_json::json!({
                        "text": row.get::<_, String>(0)?,
                        "timestamp": row.get::<_, String>(1)?,
                        "tags": row.get::<_, String>(2)?,
                        "importance": row.get::<_, f64>(3)?,
                    }))
                }) {
                    facts = rows.filter_map(|row| row.ok()).collect();
                }
            }
        }
    }

    let graph_db_path = graph_db.display().to_string();
    let memory_db_path = memory_db.display().to_string();

    // Counts describe the records actually sent to the WebUI, including code
    // entities/calls, skills, and bookmarks added above.
    entities_count = nodes
        .iter()
        .filter_map(|node| node.get("name").and_then(Value::as_str))
        .collect::<std::collections::HashSet<_>>()
        .len() as i64;
    relations_count = edges
        .iter()
        .filter_map(|edge| {
            Some((
                edge.get("from_name")?.as_str()?,
                edge.get("to_name")?.as_str()?,
                edge.get("relation_type")?.as_str()?,
            ))
        })
        .collect::<std::collections::HashSet<_>>()
        .len() as i64;
    facts_count = facts.len() as i64;

    super::super::protocol::cognitive_memory(
        entities_count,
        relations_count,
        facts_count,
        working_memory_keys,
        memory_db_path,
        graph_db_path,
        nodes,
        edges,
        facts,
    )
}
