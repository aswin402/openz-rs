//! Cognitive-memory WebSocket query and response builder.

use serde_json::Value;

fn collect_graph_db_nodes_and_edges(
    graph_db: &std::path::Path,
    nodes: &mut Vec<Value>,
    edges: &mut Vec<Value>,
) {
    let conn = match graph_db.exists() {
        true => match rusqlite::Connection::open(graph_db) {
            Ok(conn) => conn,
            Err(_) => return,
        },
        false => return,
    };

    match conn.prepare("SELECT name, entity_type, observations FROM graph_nodes") {
        Ok(mut statement) => match statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        }) {
            Ok(rows) => {
                *nodes = rows
                    .filter_map(|row| row.ok())
                    .filter(|(name, entity_type, observations)| {
                        let is_url = name.starts_with("http://") || name.starts_with("https://");
                        let is_scraper = entity_type == "Entity"
                            && observations.starts_with("[\"source: http");
                        !is_url && !is_scraper
                    })
                    .map(|(name, entity_type, observations)| {
                        serde_json::json!({
                            "name": name,
                            "entity_type": entity_type,
                            "observations": observations,
                        })
                    })
                    .collect();
            }
            Err(_) => {}
        },
        Err(_) => {}
    }

    let valid_node_names: std::collections::HashSet<String> = nodes
        .iter()
        .filter_map(|n| n.get("name").and_then(Value::as_str).map(str::to_string))
        .collect();

    match conn.prepare(
        "SELECT from_name, to_name, relation_type, confidence, valid_from
         FROM graph_edges WHERE valid_until IS NULL",
    ) {
        Ok(mut statement) => match statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, f64>(3)?,
                row.get::<_, String>(4)?,
            ))
        }) {
            Ok(rows) => {
                *edges = rows
                    .filter_map(|row| row.ok())
                    .filter(|(from_name, to_name, _, _, _)| {
                        valid_node_names.contains(from_name) && valid_node_names.contains(to_name)
                    })
                    .map(|(from_name, to_name, relation_type, confidence, valid_from)| {
                        serde_json::json!({
                            "from_name": from_name,
                            "to_name": to_name,
                            "relation_type": relation_type,
                            "confidence": confidence,
                            "valid_from": valid_from,
                            "provenance": "extracted",
                            "source": "graph_memory.db",
                        })
                    })
                    .collect();
            }
            Err(_) => {}
        },
        Err(_) => {}
    }

    match conn.prepare(
        "SELECT name, element_type, file_path, signature, start_line, end_line
         FROM code_elements WHERE file_path NOT LIKE '/tmp/%' ORDER BY file_path, start_line",
    ) {
        Ok(mut statement) => match statement.query_map([], |row| {
            let name: String = row.get(0)?;
            let element_type: String = row.get(1)?;
            let file_path: String = row.get(2)?;
            let signature: String = row.get(3)?;
            let start_line: i64 = row.get(4)?;
            let end_line: i64 = row.get(5)?;
            let entity_type = match element_type.is_empty() {
                true => "code",
                false => element_type.as_str(),
            };
            Ok(serde_json::json!({
                "name": name,
                "entity_type": entity_type,
                "observations": format!(
                    "File: {} | Signature: {} | Lines: {}-{}",
                    file_path, signature, start_line, end_line
                ),
            }))
        }) {
            Ok(rows) => nodes.extend(rows.filter_map(|row| row.ok())),
            Err(_) => {}
        },
        Err(_) => {}
    }

    match conn.prepare(
        "SELECT caller.name, callee.name, COALESCE(cc.call_site, 'calls')
         FROM code_calls cc
         JOIN code_elements caller ON caller.element_id = cc.caller_id
         JOIN code_elements callee ON callee.element_id = cc.callee_id",
    ) {
        Ok(mut statement) => match statement.query_map([], |row| {
            Ok(serde_json::json!({
                "from_name": row.get::<_, String>(0)?,
                "to_name": row.get::<_, String>(1)?,
                "relation_type": row.get::<_, String>(2)?,
                "provenance": "extracted",
                "source": "code_calls",
            }))
        }) {
            Ok(rows) => edges.extend(rows.filter_map(|row| row.ok())),
            Err(_) => {}
        },
        Err(_) => {}
    };
}

fn collect_memory_db_records(
    memory_db: &std::path::Path,
    facts: &mut Vec<Value>,
) {
    let conn = match memory_db.exists() {
        true => match rusqlite::Connection::open(memory_db) {
            Ok(conn) => conn,
            Err(_) => return,
        },
        false => return,
    };

    match conn.prepare(
        "SELECT text, timestamp, tags, importance FROM cognitive_memory ORDER BY timestamp DESC",
    ) {
        Ok(mut statement) => match statement.query_map([], |row| {
            Ok(serde_json::json!({
                "text": row.get::<_, String>(0)?,
                "timestamp": row.get::<_, String>(1)?,
                "tags": row.get::<_, String>(2)?,
                "importance": row.get::<_, f64>(3)?,
            }))
        }) {
            Ok(rows) => *facts = rows.filter_map(|row| row.ok()).collect(),
            Err(_) => {}
        },
        Err(_) => {}
    };
}

pub(crate) async fn cognitive_memory_event() -> Value {
    let working_memory_keys = crate::tools::memory_extra::working::active_working_memory_keys();
    let mut nodes: Vec<Value> = Vec::new();
    let mut edges: Vec<Value> = Vec::new();
    let mut facts: Vec<Value> = Vec::new();

    let graph_db = crate::config::loader::runtime_db_path("graph_memory.db");
    collect_graph_db_nodes_and_edges(&graph_db, &mut nodes, &mut edges);

    let memory_db = crate::config::loader::runtime_db_path("memory.db");
    collect_memory_db_records(&memory_db, &mut facts);

    let graph_db_path = graph_db.display().to_string();
    let memory_db_path = memory_db.display().to_string();

    let entities_count = nodes
        .iter()
        .filter_map(|node| node.get("name").and_then(Value::as_str))
        .collect::<std::collections::HashSet<_>>()
        .len() as i64;
    let relations_count = edges
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
    let facts_count = facts.len() as i64;

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
