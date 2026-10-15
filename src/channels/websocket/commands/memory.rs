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
            Ok(serde_json::json!({
                "name": row.get::<_, String>(0)?,
                "entity_type": row.get::<_, String>(1)?,
                "observations": row.get::<_, String>(2)?,
            }))
        }) {
            Ok(rows) => *nodes = rows.filter_map(|row| row.ok()).collect(),
            Err(_) => {}
        },
        Err(_) => {}
    }

    match conn.prepare(
        "SELECT from_name, to_name, relation_type, confidence, valid_from
         FROM graph_edges WHERE valid_until IS NULL",
    ) {
        Ok(mut statement) => match statement.query_map([], |row| {
            Ok(serde_json::json!({
                "from_name": row.get::<_, String>(0)?,
                "to_name": row.get::<_, String>(1)?,
                "relation_type": row.get::<_, String>(2)?,
                "confidence": row.get::<_, f64>(3)?,
                "valid_from": row.get::<_, String>(4)?,
            }))
        }) {
            Ok(rows) => *edges = rows.filter_map(|row| row.ok()).collect(),
            Err(_) => {}
        },
        Err(_) => {}
    }

    match conn.prepare(
        "SELECT name, element_type, file_path, signature, start_line, end_line
         FROM code_elements ORDER BY file_path, start_line",
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
    nodes: &mut Vec<Value>,
    facts: &mut Vec<Value>,
) {
    let conn = match memory_db.exists() {
        true => match rusqlite::Connection::open(memory_db) {
            Ok(conn) => conn,
            Err(_) => return,
        },
        false => return,
    };

    match conn.prepare("SELECT name, content, profile, use_count FROM skills") {
        Ok(mut statement) => match statement.query_map([], |row| {
            let name: String = row.get(0)?;
            let content: String = row.get(1).unwrap_or_default();
            let profile: Option<String> = row.get(2).unwrap_or(None);
            let use_count: i64 = row.get(3).unwrap_or(0);
            Ok((name, content, profile, use_count))
        }) {
            Ok(rows) => {
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
            Err(_) => {}
        },
        Err(_) => {}
    };

    match conn.prepare("SELECT label, kind, uri, aliases, summary FROM source_bookmarks") {
        Ok(mut statement) => match statement.query_map([], |row| {
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
            Ok(rows) => nodes.extend(rows.filter_map(|row| row.ok())),
            Err(_) => {}
        },
        Err(_) => {}
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
    collect_memory_db_records(&memory_db, &mut nodes, &mut facts);

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
