use crate::memory::{scope_from_args, with_graph_db as with_db};
use crate::tools::Tool;
use anyhow::{anyhow, Result};
use rusqlite::params;
use serde_json::{json, Value};

// ─── Extraction Helpers ─────────────────────────────────────────

pub(crate) fn extract_string_field(val: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(v) = val.get(*key) {
            if let Some(s) = v.as_str() {
                let trimmed = s.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            } else if v.is_number() || v.is_boolean() {
                let s = v.to_string();
                let trimmed = s.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
    }
    None
}

pub(crate) fn extract_string_list_field(val: &Value, keys: &[&str]) -> Vec<String> {
    for key in keys {
        if let Some(v) = val.get(*key) {
            if let Some(arr) = v.as_array() {
                let list: Vec<String> = arr
                    .iter()
                    .filter_map(|item| {
                        if let Some(s) = item.as_str() {
                            let trimmed = s.trim();
                            if !trimmed.is_empty() {
                                Some(trimmed.to_string())
                            } else {
                                None
                            }
                        } else if item.is_number() || item.is_boolean() {
                            Some(item.to_string())
                        } else {
                            None
                        }
                    })
                    .collect();
                if !list.is_empty() {
                    return list;
                }
            } else if let Some(s) = v.as_str() {
                let trimmed = s.trim();
                if !trimmed.is_empty() {
                    if trimmed.contains(',') {
                        return trimmed
                            .split(',')
                            .map(|part| part.trim().to_string())
                            .filter(|p| !p.is_empty())
                            .collect();
                    } else {
                        return vec![trimmed.to_string()];
                    }
                }
            }
        }
    }
    Vec::new()
}

// ─── Tool 1: CreateEntitiesTool ─────────────────────────────────

pub struct CreateEntitiesTool;

#[async_trait::async_trait]
impl Tool for CreateEntitiesTool {
    fn name(&self) -> &str {
        "create_entities"
    }

    fn description(&self) -> &str {
        "Create multiple new entities in the knowledge graph. Each entity must have a name and entity_type. Merges observations for existing entities."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "entities": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "name": { "type": "string", "description": "The name of the entity" },
                            "entityType": { "type": "string", "description": "The type of the entity (aliases: entity_type, type)" },
                            "observations": { "type": "array", "items": { "type": "string" }, "description": "An array of observation contents (alias: observation)" }
                        },
                        "required": ["name"]
                    },
                    "description": "Array of entities to create (or single entity object)"
                },
                "userId": { "type": "string" },
                "sessionId": { "type": "string" },
                "agentId": { "type": "string" }
            },
            "required": ["entities"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let entities: Vec<Value> = if let Some(arr) = arguments
            .get("entities")
            .or_else(|| arguments.get("entity"))
            .or_else(|| arguments.get("nodes"))
            .or_else(|| arguments.get("items"))
            .and_then(|v| v.as_array())
        {
            arr.clone()
        } else if let Some(obj) = arguments
            .get("entity")
            .or_else(|| arguments.get("node"))
            .and_then(|v| v.as_object())
        {
            vec![Value::Object(obj.clone())]
        } else if arguments.get("name").is_some()
            || arguments.get("entity_name").is_some()
            || arguments.get("entityName").is_some()
        {
            vec![arguments.clone()]
        } else {
            return Err(anyhow!("Missing 'entities' array or entity object"));
        };
        let (user_id, session_id, agent_id) = scope_from_args(arguments);

        let mut created = Vec::new();
        with_db(|conn| {
            let tx = conn.unchecked_transaction()?;
            for entity in &entities {
                let name = extract_string_field(
                    entity,
                    &["name", "entity_name", "entityName", "title", "id"],
                )
                .ok_or_else(|| anyhow!("Entity missing 'name'"))?;
                let entity_type = extract_string_field(
                    entity,
                    &["entityType", "entity_type", "type", "kind", "category"],
                )
                .unwrap_or_else(|| "Entity".to_string());
                let obs = extract_string_list_field(
                    entity,
                    &["observations", "observation", "contents", "content", "notes"],
                );
                let obs_json = serde_json::to_string(&obs)?;

                let existing_obs: Option<String> = tx
                    .query_row(
                        "SELECT observations FROM graph_nodes WHERE name = ?1 AND user_id = ?2 AND session_id = ?3 AND agent_id = ?4",
                        params![name, user_id, session_id, agent_id],
                        |row| row.get(0),
                    )
                    .ok();

                if let Some(existing_obs_json) = existing_obs {
                    let mut current_obs: Vec<String> =
                        serde_json::from_str(&existing_obs_json).unwrap_or_default();
                    for o in &obs {
                        if !current_obs.contains(o) {
                            current_obs.push(o.clone());
                        }
                    }
                    let updated_obs_json = serde_json::to_string(&current_obs)?;
                    tx.execute(
                        "UPDATE graph_nodes SET observations = ?1, entity_type = CASE WHEN ?2 != 'Entity' THEN ?2 ELSE entity_type END WHERE name = ?3 AND user_id = ?4 AND session_id = ?5 AND agent_id = ?6",
                        params![updated_obs_json, entity_type, name, user_id, session_id, agent_id],
                    )?;
                    created.push(json!({
                        "name": name,
                        "entityType": entity_type,
                        "observations": current_obs
                    }));
                } else {
                    tx.execute(
                        "INSERT INTO graph_nodes (name, entity_type, observations, user_id, session_id, agent_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                        params![name, entity_type, obs_json, user_id, session_id, agent_id],
                    )?;
                    created.push(json!({
                        "name": name,
                        "entityType": entity_type,
                        "observations": obs
                    }));
                }
            }
            tx.commit()?;
            Ok(())
        })?;

        Ok(json!({ "result": created }))
    }
}

// ─── Tool 2: CreateRelationsTool ────────────────────────────────

pub struct CreateRelationsTool;

#[async_trait::async_trait]
impl Tool for CreateRelationsTool {
    fn name(&self) -> &str {
        "create_relations"
    }

    fn description(&self) -> &str {
        "Create multiple new relations between entities in the knowledge graph. Relations should be in active voice."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "relations": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "from": { "type": "string", "description": "The name of the source entity (alias: source)" },
                            "to": { "type": "string", "description": "The name of the target entity (alias: target)" },
                            "relationType": { "type": "string", "description": "The type of relation (aliases: relation_type, relation, type)" }
                        },
                        "required": ["from", "to", "relationType"]
                    }
                },
                "userId": { "type": "string" },
                "sessionId": { "type": "string" },
                "agentId": { "type": "string" }
            },
            "required": ["relations"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let relations: Vec<Value> = if let Some(arr) = arguments
            .get("relations")
            .or_else(|| arguments.get("relation"))
            .or_else(|| arguments.get("edges"))
            .or_else(|| arguments.get("links"))
            .or_else(|| arguments.get("relationships"))
            .and_then(|v| v.as_array())
        {
            arr.clone()
        } else if let Some(obj) = arguments
            .get("relation")
            .or_else(|| arguments.get("edge"))
            .or_else(|| arguments.get("link"))
            .and_then(|v| v.as_object())
        {
            vec![Value::Object(obj.clone())]
        } else if (arguments.get("from").is_some() || arguments.get("source").is_some())
            && (arguments.get("to").is_some() || arguments.get("target").is_some())
        {
            vec![arguments.clone()]
        } else {
            return Err(anyhow!("Missing 'relations' array or relation object"));
        };
        let (user_id, session_id, agent_id) = scope_from_args(arguments);

        let mut created = Vec::new();
        with_db(|conn| {
            let tx = conn.unchecked_transaction()?;
            for rel in &relations {
                let from = extract_string_field(
                    rel,
                    &["from", "source", "from_name", "fromName", "start", "origin"],
                )
                .ok_or_else(|| anyhow!("Relation missing 'from'"))?;
                let to = extract_string_field(
                    rel,
                    &["to", "target", "to_name", "toName", "end", "destination"],
                )
                .ok_or_else(|| anyhow!("Relation missing 'to'"))?;
                let rel_type = extract_string_field(
                    rel,
                    &[
                        "relationType",
                        "relation_type",
                        "relation",
                        "type",
                        "predicate",
                        "label",
                        "name",
                    ],
                )
                .unwrap_or_else(|| "relates_to".to_string());

                let exists: bool = tx.query_row(
                    "SELECT EXISTS(SELECT 1 FROM graph_edges WHERE from_name = ?1 AND to_name = ?2 AND relation_type = ?3 AND user_id = ?4 AND session_id = ?5 AND agent_id = ?6 AND valid_until IS NULL)",
                    params![from, to, rel_type, user_id, session_id, agent_id],
                    |row| row.get(0),
                )?;

                if !exists {
                    tx.execute(
                        "INSERT INTO graph_edges (from_name, to_name, relation_type, user_id, session_id, agent_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                        params![from, to, rel_type, user_id, session_id, agent_id],
                    )?;
                    created.push(json!({
                        "from": from,
                        "to": to,
                        "relationType": rel_type
                    }));
                }
            }
            tx.commit()?;
            Ok(())
        })?;

        Ok(json!({ "result": created }))
    }
}

// ─── Tool 3: AddObservationsTool ────────────────────────────────

pub struct AddObservationsTool;

#[async_trait::async_trait]
impl Tool for AddObservationsTool {
    fn name(&self) -> &str {
        "add_observations"
    }

    fn description(&self) -> &str {
        "Add new observations to existing entities in the knowledge graph. Automatically creates the entity if not found."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "observations": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "entityName": { "type": "string", "description": "The name of the entity (aliases: entity_name, name)" },
                            "contents": { "type": "array", "items": { "type": "string" }, "description": "Array or single string of observations (aliases: content, observation, notes)" }
                        },
                        "required": ["entityName", "contents"]
                    }
                },
                "userId": { "type": "string" },
                "sessionId": { "type": "string" },
                "agentId": { "type": "string" }
            },
            "required": ["observations"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let observations: Vec<Value> = if let Some(arr) = arguments
            .get("observations")
            .or_else(|| arguments.get("observation"))
            .or_else(|| arguments.get("items"))
            .and_then(|v| v.as_array())
        {
            arr.clone()
        } else if let Some(obj) = arguments
            .get("observation")
            .or_else(|| arguments.get("item"))
            .and_then(|v| v.as_object())
        {
            vec![Value::Object(obj.clone())]
        } else if arguments.get("entityName").is_some()
            || arguments.get("entity_name").is_some()
            || arguments.get("name").is_some()
            || arguments.get("entity").is_some()
        {
            vec![arguments.clone()]
        } else {
            return Err(anyhow!("Missing 'observations' array or observation object"));
        };
        let (user_id, session_id, agent_id) = scope_from_args(arguments);

        let mut results = Vec::new();
        with_db(|conn| {
            let tx = conn.unchecked_transaction()?;
            for obs in &observations {
                let entity_name = extract_string_field(
                    obs,
                    &["entityName", "entity_name", "name", "entity", "id"],
                )
                .ok_or_else(|| anyhow!("Missing 'entityName'"))?;
                let contents = extract_string_list_field(
                    obs,
                    &["contents", "content", "observations", "observation", "notes", "text"],
                );

                let current_obs_str: Option<String> = tx
                    .query_row(
                        "SELECT observations FROM graph_nodes WHERE name = ?1 AND user_id = ?2 AND session_id = ?3 AND agent_id = ?4",
                        params![entity_name, user_id, session_id, agent_id],
                        |row| row.get(0),
                    )
                    .ok();

                match current_obs_str {
                    Some(obs_json) => {
                        let mut current_obs: Vec<String> =
                            serde_json::from_str(&obs_json).unwrap_or_default();
                        let mut added = Vec::new();
                        for content in &contents {
                            if !current_obs.contains(content) {
                                current_obs.push(content.clone());
                                added.push(content.clone());
                            }
                        }
                        let new_obs_json = serde_json::to_string(&current_obs)?;
                        tx.execute(
                            "UPDATE graph_nodes SET observations = ?1 WHERE name = ?2 AND user_id = ?3 AND session_id = ?4 AND agent_id = ?5",
                            params![new_obs_json, entity_name, user_id, session_id, agent_id],
                        )?;
                        results.push(json!({ "entityName": entity_name, "addedObservations": added }));
                    }
                    None => {
                        let new_obs_json = serde_json::to_string(&contents)?;
                        tx.execute(
                            "INSERT INTO graph_nodes (name, entity_type, observations, user_id, session_id, agent_id) VALUES (?1, 'Entity', ?2, ?3, ?4, ?5)",
                            params![entity_name, new_obs_json, user_id, session_id, agent_id],
                        )?;
                        results.push(json!({ "entityName": entity_name, "addedObservations": contents }));
                    }
                }
            }
            tx.commit()?;
            Ok(())
        })?;

        Ok(json!({ "result": results }))
    }
}

// ─── Tool 4: DeleteEntitiesTool ─────────────────────────────────

pub struct DeleteEntitiesTool;

#[async_trait::async_trait]
impl Tool for DeleteEntitiesTool {
    fn name(&self) -> &str {
        "delete_entities"
    }

    fn description(&self) -> &str {
        "Delete multiple entities and their associated relations from the knowledge graph."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "entityNames": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Array of entity names to delete (aliases: entity_names, names, entities)"
                },
                "userId": { "type": "string" },
                "sessionId": { "type": "string" },
                "agentId": { "type": "string" }
            },
            "required": ["entityNames"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let names = extract_string_list_field(
            arguments,
            &["entityNames", "entity_names", "names", "name", "entities", "entity", "entity_name"],
        );
        if names.is_empty() {
            return Ok(json!({ "status": "deleted", "count": 0 }));
        }
        let (user_id, session_id, agent_id) = scope_from_args(arguments);

        with_db(|conn| {
            let tx = conn.unchecked_transaction()?;
            for name in &names {
                tx.execute(
                    "DELETE FROM graph_nodes WHERE name = ?1 AND user_id = ?2 AND session_id = ?3 AND agent_id = ?4",
                    params![name, user_id, session_id, agent_id],
                )?;
                tx.execute(
                    "UPDATE graph_edges SET valid_until = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE (from_name = ?1 OR to_name = ?1) AND user_id = ?2 AND session_id = ?3 AND agent_id = ?4 AND valid_until IS NULL",
                    params![name, user_id, session_id, agent_id],
                )?;
            }
            tx.commit()?;
            Ok(())
        })?;

        Ok(json!({ "status": "deleted", "count": names.len() }))
    }
}

// ─── Tool 5: DeleteObservationsTool ─────────────────────────────

pub struct DeleteObservationsTool;

#[async_trait::async_trait]
impl Tool for DeleteObservationsTool {
    fn name(&self) -> &str {
        "delete_observations"
    }

    fn description(&self) -> &str {
        "Delete specific observations from entities in the knowledge graph."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "deletions": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "entityName": { "type": "string", "description": "The name of the entity (aliases: entity_name, name)" },
                            "observations": { "type": "array", "items": { "type": "string" }, "description": "Array or single string of observations to delete (aliases: observation, contents)" }
                        },
                        "required": ["entityName", "observations"]
                    }
                },
                "userId": { "type": "string" },
                "sessionId": { "type": "string" },
                "agentId": { "type": "string" }
            },
            "required": ["deletions"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let deletions: Vec<Value> = if let Some(arr) = arguments
            .get("deletions")
            .or_else(|| arguments.get("deletion"))
            .or_else(|| arguments.get("observations"))
            .or_else(|| arguments.get("items"))
            .and_then(|v| v.as_array())
        {
            arr.clone()
        } else if let Some(obj) = arguments.get("deletion").and_then(|v| v.as_object()) {
            vec![Value::Object(obj.clone())]
        } else if arguments.get("entityName").is_some()
            || arguments.get("entity_name").is_some()
            || arguments.get("name").is_some()
            || arguments.get("entity").is_some()
        {
            vec![arguments.clone()]
        } else {
            return Err(anyhow!("Missing 'deletions' array or deletion object"));
        };
        let (user_id, session_id, agent_id) = scope_from_args(arguments);

        with_db(|conn| {
            let tx = conn.unchecked_transaction()?;
            for del in &deletions {
                let entity_name = extract_string_field(
                    del,
                    &["entityName", "entity_name", "name", "entity"],
                )
                .ok_or_else(|| anyhow!("Missing 'entityName'"))?;
                let to_remove = extract_string_list_field(
                    del,
                    &["observations", "observation", "contents", "content", "notes", "text"],
                );

                let current_obs_str: Option<String> = tx
                    .query_row(
                        "SELECT observations FROM graph_nodes WHERE name = ?1 AND user_id = ?2 AND session_id = ?3 AND agent_id = ?4",
                        params![entity_name, user_id, session_id, agent_id],
                        |row| row.get(0),
                    )
                    .ok();

                if let Some(obs_json) = current_obs_str {
                    let current_obs: Vec<String> = serde_json::from_str(&obs_json)?;
                    let filtered: Vec<String> = current_obs
                        .into_iter()
                        .filter(|o| !to_remove.contains(o))
                        .collect();
                    let new_obs_json = serde_json::to_string(&filtered)?;
                    tx.execute(
                        "UPDATE graph_nodes SET observations = ?1 WHERE name = ?2 AND user_id = ?3 AND session_id = ?4 AND agent_id = ?5",
                        params![new_obs_json, entity_name, user_id, session_id, agent_id],
                    )?;
                }
            }
            tx.commit()?;
            Ok(())
        })?;

        Ok(json!({ "status": "observations deleted" }))
    }
}

// ─── Tool 6: DeleteRelationsTool ────────────────────────────────

pub struct DeleteRelationsTool;

#[async_trait::async_trait]
impl Tool for DeleteRelationsTool {
    fn name(&self) -> &str {
        "delete_relations"
    }

    fn description(&self) -> &str {
        "Delete multiple relations from the knowledge graph. If relationType is omitted, deletes all relations between source and target."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "relations": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "from": { "type": "string", "description": "The source entity (alias: source)" },
                            "to": { "type": "string", "description": "The target entity (alias: target)" },
                            "relationType": { "type": "string", "description": "Optional relation type. If omitted, all relations between from and to are removed." }
                        },
                        "required": ["from", "to"]
                    }
                },
                "userId": { "type": "string" },
                "sessionId": { "type": "string" },
                "agentId": { "type": "string" }
            },
            "required": ["relations"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let relations: Vec<Value> = if let Some(arr) = arguments
            .get("relations")
            .or_else(|| arguments.get("relation"))
            .or_else(|| arguments.get("edges"))
            .or_else(|| arguments.get("links"))
            .or_else(|| arguments.get("relationships"))
            .and_then(|v| v.as_array())
        {
            arr.clone()
        } else if let Some(obj) = arguments.get("relation").and_then(|v| v.as_object()) {
            vec![Value::Object(obj.clone())]
        } else if (arguments.get("from").is_some() || arguments.get("source").is_some())
            && (arguments.get("to").is_some() || arguments.get("target").is_some())
        {
            vec![arguments.clone()]
        } else {
            return Err(anyhow!("Missing 'relations' array or relation object"));
        };
        let (user_id, session_id, agent_id) = scope_from_args(arguments);

        with_db(|conn| {
            let tx = conn.unchecked_transaction()?;
            for rel in &relations {
                let from = extract_string_field(
                    rel,
                    &["from", "source", "from_name", "fromName", "start"],
                )
                .ok_or_else(|| anyhow!("Missing 'from'"))?;
                let to = extract_string_field(
                    rel,
                    &["to", "target", "to_name", "toName", "end", "destination"],
                )
                .ok_or_else(|| anyhow!("Missing 'to'"))?;
                let rel_type = extract_string_field(
                    rel,
                    &[
                        "relationType",
                        "relation_type",
                        "relation",
                        "type",
                        "predicate",
                        "label",
                    ],
                );

                if let Some(rt) = rel_type {
                    tx.execute(
                        "UPDATE graph_edges SET valid_until = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE from_name = ?1 AND to_name = ?2 AND relation_type = ?3 AND user_id = ?4 AND session_id = ?5 AND agent_id = ?6 AND valid_until IS NULL",
                        params![from, to, rt, user_id, session_id, agent_id],
                    )?;
                } else {
                    tx.execute(
                        "UPDATE graph_edges SET valid_until = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE from_name = ?1 AND to_name = ?2 AND user_id = ?3 AND session_id = ?4 AND agent_id = ?5 AND valid_until IS NULL",
                        params![from, to, user_id, session_id, agent_id],
                    )?;
                }
            }
            tx.commit()?;
            Ok(())
        })?;

        Ok(json!({ "status": "relations deleted" }))
    }
}

// ─── Tool 7: ReadGraphTool ──────────────────────────────────────

pub struct ReadGraphTool;

#[async_trait::async_trait]
impl Tool for ReadGraphTool {
    fn name(&self) -> &str {
        "read_graph"
    }

    fn description(&self) -> &str {
        "Read the entire knowledge graph."
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
        let (user_id, session_id, agent_id) = scope_from_args(arguments);

        with_db(|conn| {
            let mut stmt_nodes = conn.prepare(
                "SELECT name, entity_type, observations FROM graph_nodes WHERE (?1 IS NULL OR user_id = ?1 OR user_id = '*') AND (?2 IS NULL OR session_id = ?2 OR session_id = '*') AND (?3 IS NULL OR agent_id = ?3 OR agent_id = '*')"
            )?;
            let mut node_rows = stmt_nodes.query(params![user_id, session_id, agent_id])?;
            let mut entities = Vec::new();
            while let Some(row) = node_rows.next()? {
                let name: String = row.get(0)?;
                let entity_type: String = row.get(1)?;
                let obs_json: String = row.get(2)?;
                let observations: Vec<String> = serde_json::from_str(&obs_json).unwrap_or_default();
                entities.push(json!({ "name": name, "entityType": entity_type, "observations": observations }));
            }

            let mut stmt_edges = conn.prepare(
                "SELECT from_name, to_name, relation_type FROM graph_edges WHERE (?1 IS NULL OR user_id = ?1 OR user_id = '*') AND (?2 IS NULL OR session_id = ?2 OR session_id = '*') AND (?3 IS NULL OR agent_id = ?3 OR agent_id = '*') AND valid_until IS NULL"
            )?;
            let mut edge_rows = stmt_edges.query(params![user_id, session_id, agent_id])?;
            let mut relations = Vec::new();
            while let Some(row) = edge_rows.next()? {
                relations.push(json!({ "from": row.get::<_, String>(0)?, "to": row.get::<_, String>(1)?, "relationType": row.get::<_, String>(2)? }));
            }

            Ok(json!({ "entities": entities, "relations": relations }))
        })
    }
}

// ─── Tool 8: SearchNodesTool ────────────────────────────────────

pub struct SearchNodesTool;

#[async_trait::async_trait]
impl Tool for SearchNodesTool {
    fn name(&self) -> &str {
        "search_nodes"
    }

    fn description(&self) -> &str {
        "Search for nodes in the knowledge graph based on a query."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Search query to match against entity names, types, and observation content (aliases: q, search, term, pattern)" },
                "userId": { "type": "string" },
                "sessionId": { "type": "string" },
                "agentId": { "type": "string" }
            },
            "required": ["query"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let query = extract_string_field(
            arguments,
            &["query", "q", "search", "term", "pattern", "keyword", "text"],
        )
        .unwrap_or_default();
        let (user_id, session_id, agent_id) = scope_from_args(arguments);
        let query_pattern = format!("%{}%", query.to_lowercase());

        with_db(|conn| {
            let mut stmt_nodes = conn.prepare(
                "SELECT name, entity_type, observations FROM graph_nodes WHERE (LOWER(name) LIKE ?1 OR LOWER(entity_type) LIKE ?1 OR LOWER(observations) LIKE ?1) AND (?2 IS NULL OR user_id = ?2 OR user_id = '*') AND (?3 IS NULL OR session_id = ?3 OR session_id = '*') AND (?4 IS NULL OR agent_id = ?4 OR agent_id = '*')"
            )?;
            let mut node_rows =
                stmt_nodes.query(params![query_pattern, user_id, session_id, agent_id])?;
            let mut entities = Vec::new();
            while let Some(row) = node_rows.next()? {
                let name: String = row.get(0)?;
                let entity_type: String = row.get(1)?;
                let obs_json: String = row.get(2)?;
                let observations: Vec<String> = serde_json::from_str(&obs_json).unwrap_or_default();
                entities.push(json!({ "name": name, "entityType": entity_type, "observations": observations }));
            }

            let mut stmt_edges = conn.prepare(
                "SELECT DISTINCT from_name, to_name, relation_type FROM graph_edges WHERE (from_name IN (SELECT name FROM graph_nodes WHERE (LOWER(name) LIKE ?1 OR LOWER(entity_type) LIKE ?1 OR LOWER(observations) LIKE ?1)) OR to_name IN (SELECT name FROM graph_nodes WHERE (LOWER(name) LIKE ?1 OR LOWER(entity_type) LIKE ?1 OR LOWER(observations) LIKE ?1))) AND (?2 IS NULL OR user_id = ?2 OR user_id = '*') AND (?3 IS NULL OR session_id = ?3 OR session_id = '*') AND (?4 IS NULL OR agent_id = ?4 OR agent_id = '*') AND valid_until IS NULL"
            )?;
            let mut edge_rows =
                stmt_edges.query(params![query_pattern, user_id, session_id, agent_id])?;
            let mut relations = Vec::new();
            while let Some(row) = edge_rows.next()? {
                relations.push(json!({ "from": row.get::<_, String>(0)?, "to": row.get::<_, String>(1)?, "relationType": row.get::<_, String>(2)? }));
            }

            Ok(json!({ "entities": entities, "relations": relations }))
        })
    }
}

// ─── Tool 9: OpenNodesTool ──────────────────────────────────────

pub struct OpenNodesTool;

#[async_trait::async_trait]
impl Tool for OpenNodesTool {
    fn name(&self) -> &str {
        "open_nodes"
    }

    fn description(&self) -> &str {
        "Open specific nodes in the knowledge graph by their names."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "names": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "An array or single string of entity names to retrieve (aliases: name, entityNames, entity_names, entities)"
                },
                "userId": { "type": "string" },
                "sessionId": { "type": "string" },
                "agentId": { "type": "string" }
            },
            "required": ["names"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let names = extract_string_list_field(
            arguments,
            &["names", "name", "entityNames", "entity_names", "entities", "entity", "nodes"],
        );
        if names.is_empty() {
            return Ok(json!({ "entities": [], "relations": [] }));
        }
        let (user_id, session_id, agent_id) = scope_from_args(arguments);

        with_db(|conn| {
            let mut entities = Vec::new();
            for name in &names {
                let mut stmt = conn.prepare(
                    "SELECT entity_type, observations FROM graph_nodes WHERE name = ?1 AND (?2 IS NULL OR user_id = ?2 OR user_id = '*') AND (?3 IS NULL OR session_id = ?3 OR session_id = '*') AND (?4 IS NULL OR agent_id = ?4 OR agent_id = '*')"
                )?;
                let mut rows = stmt.query(params![name, user_id, session_id, agent_id])?;
                if let Some(row) = rows.next()? {
                    let entity_type: String = row.get(0)?;
                    let obs_json: String = row.get(1)?;
                    let observations: Vec<String> =
                        serde_json::from_str(&obs_json).unwrap_or_default();
                    entities.push(json!({ "name": name, "entityType": entity_type, "observations": observations }));
                }
            }

            let mut relations = Vec::new();
            if !names.is_empty() {
                let placeholders: Vec<String> =
                    (0..names.len()).map(|i| format!("?{}", i + 4)).collect();
                let placeholders_str = placeholders.join(", ");
                let sql = format!(
                    "SELECT DISTINCT from_name, to_name, relation_type FROM graph_edges WHERE (from_name IN ({0}) OR to_name IN ({0})) AND (?1 IS NULL OR user_id = ?1 OR user_id = '*') AND (?2 IS NULL OR session_id = ?2 OR session_id = '*') AND (?3 IS NULL OR agent_id = ?3 OR agent_id = '*') AND valid_until IS NULL",
                    placeholders_str
                );
                let mut stmt_edges = conn.prepare(&sql)?;
                let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> =
                    vec![Box::new(user_id), Box::new(session_id), Box::new(agent_id)];
                for name in &names {
                    param_values.push(Box::new(name.clone()));
                }
                let params_refs: Vec<&dyn rusqlite::types::ToSql> =
                    param_values.iter().map(|p| p.as_ref()).collect();
                let mut edge_rows = stmt_edges.query(params_refs.as_slice())?;
                while let Some(row) = edge_rows.next()? {
                    relations.push(json!({ "from": row.get::<_, String>(0)?, "to": row.get::<_, String>(1)?, "relationType": row.get::<_, String>(2)? }));
                }
            }

            Ok(json!({ "entities": entities, "relations": relations }))
        })
    }
}

