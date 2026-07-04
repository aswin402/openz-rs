use crate::tools::graph_memory::{scope_from_args, with_db};
use crate::tools::Tool;
use anyhow::{anyhow, Result};
use rusqlite::params;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet, VecDeque};

// ─── Tool: LogRepositoryEvolutionTool ────────────────────────────

pub struct LogRepositoryEvolutionTool;

#[async_trait::async_trait]
impl Tool for LogRepositoryEvolutionTool {
    fn name(&self) -> &str { "log_repository_evolution" }

    fn description(&self) -> &str {
        "Log file changes, refactoring records, commits, versions, and bug status metrics."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "filePath": { "type": "string" },
                "version": { "type": "string" },
                "commitHash": { "type": "string" },
                "author": { "type": "string" },
                "changeType": { "type": "string", "description": "e.g. added, modified, refactored, deleted" },
                "summary": { "type": "string" },
                "bugIntroduced": { "type": "boolean" },
                "bugFixed": { "type": "boolean" }
            },
            "required": ["filePath", "version", "changeType", "summary"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let file_path = arguments["filePath"].as_str().ok_or_else(|| anyhow!("Missing filePath"))?;
        let version = arguments["version"].as_str().ok_or_else(|| anyhow!("Missing version"))?;
        let commit_hash = arguments["commitHash"].as_str().unwrap_or("");
        let author = arguments["author"].as_str().unwrap_or("");
        let change_type = arguments["changeType"].as_str().ok_or_else(|| anyhow!("Missing changeType"))?;
        let summary = arguments["summary"].as_str().ok_or_else(|| anyhow!("Missing summary"))?;
        let bug_introduced = arguments["bugIntroduced"].as_bool().unwrap_or(false) as i32;
        let bug_fixed = arguments["bugFixed"].as_bool().unwrap_or(false) as i32;

        with_db(|conn| {
            conn.execute(
                "INSERT INTO repo_evolution (file_path, version, commit_hash, author, change_type, summary, bug_introduced, bug_fixed) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![file_path, version, commit_hash, author, change_type, summary, bug_introduced, bug_fixed],
            )?;
            Ok(json!({ "status": "logged" }))
        })
    }
}

// ─── Tool: QueryRepositoryEvolutionTool ──────────────────────────

pub struct QueryRepositoryEvolutionTool;

#[async_trait::async_trait]
impl Tool for QueryRepositoryEvolutionTool {
    fn name(&self) -> &str { "query_repository_evolution" }

    fn description(&self) -> &str {
        "Query repository file history and change statistics."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "filePath": { "type": "string", "description": "Optional filter by file path" }
            }
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let file_path = arguments["filePath"].as_str();

        with_db(|conn| {
            if let Some(fp) = file_path {
                let mut stmt = conn.prepare("SELECT * FROM repo_evolution WHERE file_path = ?1 ORDER BY created_at DESC")?;
                let rows = stmt.query_map(params![fp], map_repo_row)?;
                let mut entries = Vec::new();
                for r in rows { entries.push(r?); }
                Ok(json!({ "entries": entries, "totalCount": entries.len() }))
            } else {
                let mut stmt = conn.prepare(
                    "SELECT file_path, COUNT(*) as changes, SUM(bug_introduced) as bugs_introduced, SUM(bug_fixed) as bugs_fixed FROM repo_evolution GROUP BY file_path ORDER BY changes DESC"
                )?;
                let rows = stmt.query_map([], |row| {
                    Ok(json!({
                        "filePath": row.get::<_, String>(0)?,
                        "changes": row.get::<_, i64>(1)?,
                        "bugsIntroduced": row.get::<_, i64>(2)?,
                        "bugsFixed": row.get::<_, i64>(3)?,
                    }))
                })?;
                let mut stats = Vec::new();
                for r in rows { stats.push(r?); }
                Ok(json!({ "statistics": stats }))
            }
        })
    }
}

fn map_repo_row(row: &rusqlite::Row) -> rusqlite::Result<Value> {
    Ok(json!({
        "id": row.get::<_, i64>(0)?,
        "filePath": row.get::<_, String>(1)?,
        "version": row.get::<_, String>(2)?,
        "commitHash": row.get::<_, String>(3)?,
        "author": row.get::<_, String>(4)?,
        "changeType": row.get::<_, String>(5)?,
        "summary": row.get::<_, String>(6)?,
        "bugIntroduced": row.get::<_, bool>(7)?,
        "bugFixed": row.get::<_, bool>(8)?,
        "createdAt": row.get::<_, String>(9)?,
    }))
}

// ─── Tool: TraverseGraphTool ─────────────────────────────────────

pub struct TraverseGraphTool;

#[async_trait::async_trait]
impl Tool for TraverseGraphTool {
    fn name(&self) -> &str { "traverse_graph" }

    fn description(&self) -> &str {
        "Traverse nodes and edges from a start entity using BFS up to a maximum depth."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "startEntity": { "type": "string" },
                "maxDepth": { "type": "integer", "default": 2 },
                "userId": { "type": "string" },
                "sessionId": { "type": "string" },
                "agentId": { "type": "string" }
            },
            "required": ["startEntity"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let start = arguments["startEntity"].as_str().ok_or_else(|| anyhow!("Missing startEntity"))?;
        let max_depth = arguments["maxDepth"].as_i64().unwrap_or(2) as usize;
        let (uid, sid, aid) = scope_from_args(arguments);

        with_db(|conn| {
            let mut visited = HashSet::new();
            let mut nodes = Vec::new();
            let mut edges = Vec::new();
            let mut queue = VecDeque::new();
            queue.push_back((start.to_string(), 0usize));

            while let Some((current, depth)) = queue.pop_front() {
                if !visited.insert(current.clone()) || depth > max_depth { continue; }

                // Get node info
                if let Ok(node) = conn.query_row(
                    "SELECT name, entity_type, observations FROM graph_nodes WHERE name = ?1 AND (?2 IS NULL OR user_id = ?2 OR user_id = '*') AND (?3 IS NULL OR session_id = ?3 OR session_id = '*') AND (?4 IS NULL OR agent_id = ?4 OR agent_id = '*')",
                    params![current, uid, sid, aid],
                    |row| {
                        let name: String = row.get(0)?;
                        let etype: String = row.get(1)?;
                        let obs: String = row.get(2)?;
                        Ok(json!({ "name": name, "entityType": etype, "observations": serde_json::from_str::<Vec<String>>(&obs).unwrap_or_default() }))
                    }
                ) {
                    nodes.push(node);
                }

                if depth >= max_depth { continue; }

                // Get neighbors
                let mut stmt = conn.prepare(
                    "SELECT from_name, to_name, relation_type FROM graph_edges WHERE (from_name = ?1 OR to_name = ?1) AND (?2 IS NULL OR user_id = ?2 OR user_id = '*') AND (?3 IS NULL OR session_id = ?3 OR session_id = '*') AND (?4 IS NULL OR agent_id = ?4 OR agent_id = '*') AND valid_until IS NULL"
                )?;
                let rows = stmt.query_map(params![current, uid, sid, aid], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
                })?;
                for r in rows {
                    let (from, to, rel_type) = r?;
                    edges.push(json!({ "from": from.clone(), "to": to.clone(), "relationType": rel_type }));
                    let neighbor = if from == current { &to } else { &from };
                    if !visited.contains(neighbor) {
                        queue.push_back((neighbor.clone(), depth + 1));
                    }
                }
            }

            Ok(json!({ "nodes": nodes, "edges": edges, "maxDepth": max_depth }))
        })
    }
}

// ─── Tool: FindPathTool ──────────────────────────────────────────

pub struct FindPathTool;

#[async_trait::async_trait]
impl Tool for FindPathTool {
    fn name(&self) -> &str { "find_path" }

    fn description(&self) -> &str {
        "Find the shortest path and relations between two entity nodes using BFS."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "startEntity": { "type": "string" },
                "targetEntity": { "type": "string" },
                "userId": { "type": "string" },
                "sessionId": { "type": "string" },
                "agentId": { "type": "string" }
            },
            "required": ["startEntity", "targetEntity"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let start = arguments["startEntity"].as_str().ok_or_else(|| anyhow!("Missing startEntity"))?;
        let target = arguments["targetEntity"].as_str().ok_or_else(|| anyhow!("Missing targetEntity"))?;
        let (uid, sid, aid) = scope_from_args(arguments);

        with_db(|conn| {
            // BFS tracking parents to reconstruct path
            let mut parent: HashMap<String, (String, String)> = HashMap::new(); // child -> (parent, relation_type)
            let mut queue = VecDeque::new();
            queue.push_back(start.to_string());
            parent.insert(start.to_string(), (String::new(), String::new()));

            let mut found = false;
            while let Some(current) = queue.pop_front() {
                if current == target { found = true; break; }

                let mut stmt = conn.prepare(
                    "SELECT from_name, to_name, relation_type FROM graph_edges WHERE (from_name = ?1 OR to_name = ?1) AND (?2 IS NULL OR user_id = ?2 OR user_id = '*') AND (?3 IS NULL OR session_id = ?3 OR session_id = '*') AND (?4 IS NULL OR agent_id = ?4 OR agent_id = '*') AND valid_until IS NULL"
                )?;
                let rows = stmt.query_map(params![current, uid, sid, aid], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
                })?;
                for r in rows {
                    let (from, to, rel_type) = r?;
                    let neighbor = if from == current { &to } else { &from };
                    if !parent.contains_key(neighbor) {
                        parent.insert(neighbor.clone(), (current.clone(), rel_type));
                        queue.push_back(neighbor.clone());
                    }
                }
            }

            if !found {
                return Ok(json!({ "found": false, "path": [] }));
            }

            // Reconstruct path
            let mut path = Vec::new();
            let mut current = target.to_string();
            while current != start {
                if let Some((p, rel)) = parent.get(&current) {
                    path.push(json!({ "from": p, "to": current, "relationType": rel }));
                    current = p.clone();
                } else { break; }
            }
            path.reverse();

            Ok(json!({ "found": true, "pathLength": path.len(), "path": path }))
        })
    }
}

// ─── Tool: AnalyzeGraphCommunitiesTool ──────────────────────────

pub struct AnalyzeGraphCommunitiesTool;

#[async_trait::async_trait]
impl Tool for AnalyzeGraphCommunitiesTool {
    fn name(&self) -> &str { "analyze_graph_communities" }

    fn description(&self) -> &str {
        "Cluster the entity-relation graph into weakly connected communities with summaries."
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

        with_db(|conn| {
            // Collect all node names
            let mut stmt = conn.prepare(
                "SELECT name FROM graph_nodes WHERE (?1 IS NULL OR user_id = ?1 OR user_id = '*') AND (?2 IS NULL OR session_id = ?2 OR session_id = '*') AND (?3 IS NULL OR agent_id = ?3 OR agent_id = '*')"
            )?;
            let names: Vec<String> = stmt.query_map(params![uid, sid, aid], |r| r.get(0))?
                .filter_map(|r| r.ok()).collect();

            // Union-Find
            let mut parent: HashMap<String, String> = HashMap::new();
            for n in &names { parent.insert(n.clone(), n.clone()); }

            fn find(p: &mut HashMap<String, String>, x: &str) -> String {
                let px = p.get(x).cloned().unwrap_or_default();
                if px != x {
                    let root = find(p, &px);
                    p.insert(x.to_string(), root.clone());
                    root
                } else { x.to_string() }
            }

            let mut edge_stmt = conn.prepare(
                "SELECT from_name, to_name FROM graph_edges WHERE valid_until IS NULL AND (?1 IS NULL OR user_id = ?1 OR user_id = '*') AND (?2 IS NULL OR session_id = ?2 OR session_id = '*') AND (?3 IS NULL OR agent_id = ?3 OR agent_id = '*')"
            )?;
            let edges = edge_stmt.query_map(params![uid, sid, aid], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })?;
            for (from, to) in edges.flatten() {
                let rf = find(&mut parent, &from);
                let rt = find(&mut parent, &to);
                if rf != rt { parent.insert(rf, rt); }
            }

            // Group by root
            let mut communities: HashMap<String, Vec<String>> = HashMap::new();
            for n in &names {
                let root = find(&mut parent, n);
                communities.entry(root).or_default().push(n.clone());
            }

            let mut result = Vec::new();
            for (_, members) in communities {
                if members.len() < 2 { continue; }
                let summary = format!("{} entities: {}", members.len(), members.join(", "));
                result.push(json!({ "size": members.len(), "members": members, "summary": summary }));
            }
            result.sort_by(|a, b| b["size"].as_i64().cmp(&a["size"].as_i64()));

            Ok(json!({ "totalCommunities": result.len(), "communities": result }))
        })
    }
}

// ─── Tool: DetectAndResolveConflictsTool ─────────────────────────

pub struct DetectAndResolveConflictsTool;

const EXCLUSIVE_RELATIONS: &[&str] = &["lives_in", "current_job", "spouse", "has_status", "is_born_in", "located_in"];

#[async_trait::async_trait]
impl Tool for DetectAndResolveConflictsTool {
    fn name(&self) -> &str { "detect_and_resolve_conflicts" }

    fn description(&self) -> &str {
        "Detect and resolve contradictions or conflicts in graph relations and semantic memories."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "strategy": { "type": "string", "default": "recency", "enum": ["recency"] },
                "dryRun": { "type": "boolean", "default": true },
                "semanticThreshold": { "type": "number", "default": 0.85 },
                "userId": { "type": "string" },
                "sessionId": { "type": "string" },
                "agentId": { "type": "string" }
            }
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let strategy = arguments["strategy"].as_str().unwrap_or("recency");
        let dry_run = arguments["dryRun"].as_bool().unwrap_or(true);
        let (uid, sid, aid) = scope_from_args(arguments);

        with_db(|conn| {
            let mut conflicts = Vec::new();
            let mut resolved = 0i64;

            for &rel_type in EXCLUSIVE_RELATIONS {
                // Find entities with multiple current edges of the same exclusive type
                let mut stmt = conn.prepare(
                    "SELECT from_name, COUNT(*) as cnt FROM graph_edges WHERE relation_type = ?1 AND valid_until IS NULL AND (?2 IS NULL OR user_id = ?2 OR user_id = '*') AND (?3 IS NULL OR session_id = ?3 OR session_id = '*') AND (?4 IS NULL OR agent_id = ?4 OR agent_id = '*') GROUP BY from_name HAVING cnt > 1"
                )?;
                let rows = stmt.query_map(params![rel_type, uid, sid, aid], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
                })?;
                for r in rows {
                    let (entity, count) = r?;
                    let description = format!("Entity '{}' has {} '{}' relations (exclusive type allows 1)", entity, count, rel_type);

                    if !dry_run && strategy == "recency" {
                        // Keep the most recent, expire others
                        conn.execute(
                            "UPDATE graph_edges SET valid_until = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE rowid NOT IN (SELECT rowid FROM graph_edges WHERE from_name = ?1 AND relation_type = ?2 AND valid_until IS NULL ORDER BY created_at DESC LIMIT 1) AND from_name = ?3 AND relation_type = ?4 AND valid_until IS NULL",
                            params![entity, rel_type, entity, rel_type],
                        )?;
                        resolved += count - 1;
                    }

                    conflicts.push(json!({ "entity": entity, "relationType": rel_type, "count": count, "description": description }));
                }
            }

            Ok(json!({
                "conflictsFound": conflicts.len() as i64,
                "conflicts": conflicts,
                "resolved": if dry_run { 0 } else { resolved },
                "dryRun": dry_run,
                "strategy": strategy,
            }))
        })
    }
}
