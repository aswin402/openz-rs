use anyhow::{anyhow, Result};
use chrono::Utc;
use rusqlite::Connection;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, OnceLock};

use super::engine::{analyze_quality, generate_mermaid};
use super::store::{
    get_db_path, MemoryThoughtStore, SqliteThoughtStore, ThoughtData, ThoughtStore, ToolResult,
};
use crate::tools::Tool;

// ─── Engine (shared mutable state) ───────────────────────────────

pub(crate) struct SequentialThinkingEngine {
    pub(crate) store: Box<dyn ThoughtStore>,
    pub(crate) current_session_id: String,
    pub(crate) thought_history: Vec<ThoughtData>,
    pub(crate) branches: HashMap<String, Vec<ThoughtData>>,
}

static ENGINE: OnceLock<Arc<tokio::sync::Mutex<SequentialThinkingEngine>>> = OnceLock::new();

pub(crate) fn get_engine() -> &'static Arc<tokio::sync::Mutex<SequentialThinkingEngine>> {
    ENGINE.get_or_init(|| {
        let db_path = get_db_path();
        if let Some(parent) = db_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let store: Box<dyn ThoughtStore> = match Connection::open(&db_path) {
            Ok(conn) => match SqliteThoughtStore::new(conn) {
                Ok(s) => Box::new(s),
                Err(_) => Box::new(MemoryThoughtStore::new()),
            },
            Err(_) => Box::new(MemoryThoughtStore::new()),
        };
        Arc::new(tokio::sync::Mutex::new(SequentialThinkingEngine {
            store,
            current_session_id: String::new(),
            thought_history: Vec::new(),
            branches: HashMap::new(),
        }))
    })
}

impl SequentialThinkingEngine {
    pub(crate) fn load_session(&mut self, session_id: &str) -> Result<(), String> {
        if self.current_session_id != session_id {
            let thoughts = self.store.load_session(session_id)?;
            self.current_session_id = session_id.to_string();
            self.thought_history = thoughts;
            self.branches.clear();
            for t in &self.thought_history {
                if let (Some(_), Some(branch_id)) = (t.branch_from_thought, t.branch_id.as_ref()) {
                    self.branches
                        .entry(branch_id.clone())
                        .or_default()
                        .push(t.clone());
                }
            }
        }
        Ok(())
    }

    pub(crate) fn process_thought(&mut self, mut input: ThoughtData) -> Result<ToolResult, String> {
        let session_id = match input.session_id.as_ref() {
            Some(id) if !id.trim().is_empty() => id.clone(),
            _ => {
                let generated = uuid::Uuid::new_v4().to_string();
                input.session_id = Some(generated.clone());
                generated
            }
        };
        self.load_session(&session_id)?;
        if input.thought_number > input.total_thoughts {
            input.total_thoughts = input.thought_number;
        }
        if input.timestamp.is_none() {
            input.timestamp = Some(Utc::now());
        }
        self.store.save_thought(&session_id, &input)?;
        if let (Some(_), Some(branch_id)) = (input.branch_from_thought, input.branch_id.as_ref()) {
            self.branches
                .entry(branch_id.clone())
                .or_default()
                .push(input.clone());
        }

        let thought_number = input.thought_number;
        let total_thoughts = input.total_thoughts;
        let next_thought_needed = input.next_thought_needed;
        let left_to_be_done = input.left_to_be_done.clone().unwrap_or_default();

        self.thought_history.push(input);

        let branches = self.branches.keys().cloned().collect::<Vec<String>>();
        let confidence_history = self
            .thought_history
            .iter()
            .map(|t| t.confidence_score)
            .collect();
        let thought_graph_mermaid = generate_mermaid(&self.thought_history);

        Ok(ToolResult {
            thought_number,
            total_thoughts,
            next_thought_needed,
            branches,
            thought_history_length: self.thought_history.len(),
            thought_graph_mermaid,
            confidence_history,
            left_to_be_done,
            session_id,
        })
    }
}

// ─── Input Normalization Helpers ─────────────────────────────────

fn get_string_field(map: &serde_json::Map<String, Value>, aliases: &[&str]) -> Option<String> {
    for &k in aliases {
        if let Some(v) = map.get(k) {
            if let Some(s) = v.as_str() {
                let trimmed = s.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            } else if v.is_number() || v.is_boolean() {
                return Some(v.to_string());
            }
        }
    }
    None
}

fn get_usize_field(map: &serde_json::Map<String, Value>, aliases: &[&str]) -> Option<usize> {
    for &k in aliases {
        if let Some(v) = map.get(k) {
            if let Some(n) = v.as_u64() {
                return Some(n as usize);
            } else if let Some(n) = v.as_i64() {
                if n >= 0 {
                    return Some(n as usize);
                }
            } else if let Some(s) = v.as_str() {
                if let Ok(n) = s.trim().parse::<usize>() {
                    return Some(n);
                }
            }
        }
    }
    None
}

fn get_f64_field(map: &serde_json::Map<String, Value>, aliases: &[&str]) -> Option<f64> {
    for &k in aliases {
        if let Some(v) = map.get(k) {
            if let Some(f) = v.as_f64() {
                return Some(f);
            } else if let Some(s) = v.as_str() {
                if let Ok(f) = s.trim().parse::<f64>() {
                    return Some(f);
                }
            }
        }
    }
    None
}

fn get_bool_field(map: &serde_json::Map<String, Value>, aliases: &[&str]) -> Option<bool> {
    for &k in aliases {
        if let Some(v) = map.get(k) {
            if let Some(b) = v.as_bool() {
                return Some(b);
            } else if let Some(s) = v.as_str() {
                match s.trim().to_lowercase().as_str() {
                    "true" | "1" | "yes" | "y" => return Some(true),
                    "false" | "0" | "no" | "n" => return Some(false),
                    _ => {}
                }
            } else if let Some(n) = v.as_i64() {
                return Some(n != 0);
            }
        }
    }
    None
}

fn get_string_vec_field(map: &serde_json::Map<String, Value>, aliases: &[&str]) -> Option<Vec<String>> {
    for &k in aliases {
        if let Some(v) = map.get(k) {
            if let Some(arr) = v.as_array() {
                let items: Vec<String> = arr
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
                if !items.is_empty() {
                    return Some(items);
                }
            } else if let Some(s) = v.as_str() {
                let trimmed = s.trim();
                if !trimmed.is_empty() {
                    return Some(vec![trimmed.to_string()]);
                }
            }
        }
    }
    None
}

fn get_usize_vec_field(map: &serde_json::Map<String, Value>, aliases: &[&str]) -> Option<Vec<usize>> {
    for &k in aliases {
        if let Some(v) = map.get(k) {
            if let Some(arr) = v.as_array() {
                let items: Vec<usize> = arr
                    .iter()
                    .filter_map(|item| {
                        if let Some(n) = item.as_u64() {
                            Some(n as usize)
                        } else if let Some(s) = item.as_str() {
                            s.trim().parse::<usize>().ok()
                        } else {
                            None
                        }
                    })
                    .collect();
                if !items.is_empty() {
                    return Some(items);
                }
            } else if let Some(n) = v.as_u64() {
                return Some(vec![n as usize]);
            } else if let Some(s) = v.as_str() {
                if let Ok(n) = s.trim().parse::<usize>() {
                    return Some(vec![n]);
                }
            }
        }
    }
    None
}

fn parse_thought_data(arguments: &Value, current_len: usize) -> Result<ThoughtData> {
    match arguments {
        Value::String(s) => {
            let thought = s.trim().to_string();
            if thought.is_empty() {
                return Err(anyhow!("Missing 'thought' parameter"));
            }
            let thought_number = current_len + 1;
            let total_thoughts = std::cmp::max(thought_number + 2, 3);
            let next_thought_needed = thought_number < total_thoughts;
            Ok(ThoughtData {
                thought,
                thought_number,
                total_thoughts,
                next_thought_needed,
                is_revision: None,
                revises_thought: None,
                branch_from_thought: None,
                branch_id: None,
                needs_more_thoughts: None,
                parent_thoughts: None,
                assumptions: None,
                verified_assumptions: None,
                confidence_score: None,
                criticism: None,
                hypothesis: None,
                verification_method: None,
                left_to_be_done: None,
                timestamp: None,
                session_id: None,
            })
        }
        Value::Object(map) => {
            let thought = get_string_field(
                map,
                &[
                    "thought",
                    "content",
                    "text",
                    "step",
                    "thinking",
                    "message",
                    "description",
                ],
            )
            .ok_or_else(|| anyhow!("Missing 'thought' parameter"))?;

            let thought_number = get_usize_field(
                map,
                &[
                    "thoughtNumber",
                    "thought_number",
                    "thought_num",
                    "number",
                    "step_number",
                    "stepNumber",
                ],
            )
            .filter(|&n| n > 0)
            .unwrap_or(current_len + 1);

            let total_thoughts = get_usize_field(
                map,
                &[
                    "totalThoughts",
                    "total_thoughts",
                    "estimated_thoughts",
                    "total_steps",
                    "totalSteps",
                ],
            )
            .filter(|&n| n > 0)
            .unwrap_or_else(|| std::cmp::max(thought_number + 2, 3));

            let next_thought_needed = get_bool_field(
                map,
                &[
                    "nextThoughtNeeded",
                    "next_thought_needed",
                    "next_thought",
                    "nextThought",
                    "more_thoughts",
                ],
            )
            .unwrap_or(thought_number < total_thoughts);

            let is_revision = get_bool_field(map, &["isRevision", "is_revision", "revision"]);
            let revises_thought =
                get_usize_field(map, &["revisesThought", "revises_thought", "revises"]);
            let branch_from_thought = get_usize_field(
                map,
                &[
                    "branchFromThought",
                    "branch_from_thought",
                    "branch_from",
                    "branchFrom",
                ],
            );
            let branch_id = get_string_field(map, &["branchId", "branch_id", "branch"]);
            let needs_more_thoughts =
                get_bool_field(map, &["needsMoreThoughts", "needs_more_thoughts"]);
            let parent_thoughts =
                get_usize_vec_field(map, &["parentThoughts", "parent_thoughts", "parents"]);
            let assumptions = get_string_vec_field(map, &["assumptions"]);
            let verified_assumptions = get_string_vec_field(
                map,
                &["verifiedAssumptions", "verified_assumptions", "verified"],
            );
            let confidence_score = get_f64_field(
                map,
                &["confidenceScore", "confidence_score", "confidence", "score"],
            );
            let criticism =
                get_string_field(map, &["criticism", "self_criticism", "critique"]);
            let hypothesis = get_string_field(map, &["hypothesis"]);
            let verification_method = get_string_field(
                map,
                &[
                    "verificationMethod",
                    "verification_method",
                    "verification",
                ],
            );
            let left_to_be_done = get_string_vec_field(
                map,
                &["leftToBeDone", "left_to_be_done", "todo", "todos", "open_todos"],
            );
            let session_id =
                get_string_field(map, &["sessionId", "session_id", "session", "id"]);

            Ok(ThoughtData {
                thought,
                thought_number,
                total_thoughts,
                next_thought_needed,
                is_revision,
                revises_thought,
                branch_from_thought,
                branch_id,
                needs_more_thoughts,
                parent_thoughts,
                assumptions,
                verified_assumptions,
                confidence_score,
                criticism,
                hypothesis,
                verification_method,
                left_to_be_done,
                timestamp: None,
                session_id,
            })
        }
        _ => Err(anyhow!("Invalid arguments: expected object or string")),
    }
}

// ─── Tool 1: SequentialThinkingTool ──────────────────────────────

pub struct SequentialThinkingTool;

#[async_trait::async_trait]
impl Tool for SequentialThinkingTool {
    fn name(&self) -> &str {
        "sequentialthinking"
    }

    fn description(&self) -> &str {
        "A detailed tool for dynamic and reflective problem-solving through thoughts. Supports branching, revisions, Graph of Thoughts (GoT) merging, and Clear Thought parameters."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "thought": { "type": "string", "description": "Your current thinking step" },
                "nextThoughtNeeded": { "type": "boolean", "description": "Whether another thought step is needed" },
                "thoughtNumber": { "type": "integer", "description": "Current thought number (starts at 1)" },
                "totalThoughts": { "type": "integer", "description": "Estimated total thoughts needed" },
                "isRevision": { "type": "boolean", "description": "Whether this revises previous thinking" },
                "revisesThought": { "type": "integer", "description": "Which thought number is being revised" },
                "branchFromThought": { "type": "integer", "description": "Thought number this branch originates from" },
                "branchId": { "type": "string", "description": "Identifier for the current branch" },
                "needsMoreThoughts": { "type": "boolean", "description": "Request to add more thoughts" },
                "parentThoughts": { "type": "array", "items": { "type": "integer" }, "description": "Multiple parent thought numbers for GoT merging" },
                "assumptions": { "type": "array", "items": { "type": "string" }, "description": "Assumptions made in this step" },
                "verifiedAssumptions": { "type": "array", "items": { "type": "string" }, "description": "Assumptions verified or refuted" },
                "confidenceScore": { "type": "number", "description": "Confidence in this reasoning line (0.0 to 1.0)" },
                "criticism": { "type": "string", "description": "Self-criticism of previous thoughts" },
                "hypothesis": { "type": "string", "description": "Hypothesis to be tested" },
                "verificationMethod": { "type": "string", "description": "Method to verify the hypothesis" },
                "leftToBeDone": { "type": "array", "items": { "type": "string" }, "description": "Items/tasks left to be done" },
                "sessionId": { "type": "string", "description": "Session identifier for the thinking session" }
            },
            "required": ["thought", "nextThoughtNeeded", "thoughtNumber", "totalThoughts"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let engine = get_engine();
        let mut guard = engine.lock().await;

        if let Value::Object(map) = arguments {
            if let Some(sid) = get_string_field(map, &["sessionId", "session_id", "session", "id"]) {
                let _ = guard.load_session(&sid);
            }
        }

        let current_len = guard.thought_history.len();
        let mut thought_data = parse_thought_data(arguments, current_len)?;
        if thought_data.session_id.is_none() && !guard.current_session_id.is_empty() {
            thought_data.session_id = Some(guard.current_session_id.clone());
        }

        let result = guard
            .process_thought(thought_data)
            .map_err(|e| anyhow!("{}", e))?;
        Ok(serde_json::to_value(result).unwrap_or(Value::Null))
    }
}

// ─── Tool 2: AnalyzeGraphTool ────────────────────────────────────

pub struct AnalyzeGraphTool;

#[async_trait::async_trait]
impl Tool for AnalyzeGraphTool {
    fn name(&self) -> &str {
        "analyze_graph"
    }

    fn description(&self) -> &str {
        "Query and analyze the thought graph of a thinking session. Supports low_confidence, contradictions, unverified_assumptions, dead_branches, summary_stats, and quality_report queries."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "enum": ["low_confidence", "contradictions", "unverified_assumptions", "dead_branches", "summary_stats", "quality_report"],
                    "description": "The type of analysis/query to run against the thought graph"
                },
                "confidenceThreshold": { "type": "number", "default": 0.5, "description": "Confidence threshold for low_confidence filter" },
                "sessionId": { "type": "string", "description": "Session identifier to analyze (defaults to active session)" }
            },
            "required": ["query"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let engine = get_engine();
        let mut guard = engine.lock().await;

        let (query_arg, session_id_arg, threshold_arg) = match arguments {
            Value::String(s) => (Some(s.trim().to_string()), None, None),
            Value::Object(map) => {
                let q = get_string_field(map, &["query", "type", "mode", "action", "analysis"]);
                let sid = get_string_field(map, &["sessionId", "session_id", "session", "id"]);
                let th = get_f64_field(
                    map,
                    &[
                        "confidenceThreshold",
                        "confidence_threshold",
                        "threshold",
                        "confidence",
                    ],
                );
                (q, sid, th)
            }
            _ => (None, None, None),
        };

        let session_id = session_id_arg
            .or_else(|| {
                if !guard.current_session_id.is_empty() {
                    Some(guard.current_session_id.clone())
                } else {
                    guard
                        .store
                        .list_sessions()
                        .ok()
                        .and_then(|list| list.into_iter().next().map(|s| s.id))
                }
            });

        let session_id = match session_id {
            Some(id) if !id.trim().is_empty() => id,
            _ => {
                return Ok(json!({
                    "status": "no_session",
                    "message": "No active thinking session found. Create thoughts using sequentialthinking first."
                }));
            }
        };

        if let Err(e) = guard.load_session(&session_id) {
            return Ok(json!({
                "status": "not_found",
                "sessionId": session_id,
                "error": format!("Session not found: {}", e)
            }));
        }

        let raw_query = query_arg.unwrap_or_else(|| "summary_stats".to_string());
        let normalized = raw_query.trim().to_lowercase().replace('-', "_");
        let query = match normalized.as_str() {
            "low_confidence" | "low" | "confidence" => "low_confidence",
            "contradictions" | "contradiction" | "conflicts" => "contradictions",
            "unverified_assumptions" | "unverified" | "assumptions" => "unverified_assumptions",
            "dead_branches" | "dead" | "dead_branch" => "dead_branches",
            "summary_stats" | "summary" | "stats" => "summary_stats",
            "quality_report" | "quality" | "report" => "quality_report",
            _ => "summary_stats",
        };

        match query {
            "low_confidence" => {
                let threshold = threshold_arg.unwrap_or(0.5);
                let low: Vec<ThoughtData> = guard
                    .thought_history
                    .iter()
                    .filter(|t| t.confidence_score.map(|c| c <= threshold).unwrap_or(false))
                    .cloned()
                    .collect();
                Ok(json!(low))
            }
            "contradictions" => {
                let mut assumed = HashSet::new();
                let mut refuted = HashSet::new();
                for t in &guard.thought_history {
                    if let Some(ref ass) = t.assumptions {
                        for a in ass {
                            assumed.insert(a.trim().to_lowercase());
                        }
                    }
                    if let Some(ref ver) = t.verified_assumptions {
                        for v in ver {
                            let vc = v.trim().to_lowercase();
                            if vc.contains("refuted")
                                || vc.contains("false")
                                || vc.contains("falsified")
                            {
                                refuted.insert(
                                    vc.replace("refuted:", "")
                                        .replace("refuted", "")
                                        .replace("false:", "")
                                        .replace("false", "")
                                        .replace("falsified:", "")
                                        .replace("falsified", "")
                                        .trim()
                                        .to_string(),
                                );
                            }
                        }
                    }
                }
                let contradictions: Vec<String> = assumed
                    .intersection(&refuted)
                    .map(|s| {
                        format!(
                            "Assumption '{}' is assumed but has been refuted/falsified.",
                            s
                        )
                    })
                    .collect();
                Ok(json!(contradictions))
            }
            "unverified_assumptions" => {
                let mut assumed = HashSet::new();
                let mut verified = HashSet::new();
                for t in &guard.thought_history {
                    if let Some(ref ass) = t.assumptions {
                        for a in ass {
                            assumed.insert(a.clone());
                        }
                    }
                    if let Some(ref ver) = t.verified_assumptions {
                        for v in ver {
                            let vc = v
                                .replace("verified:", "")
                                .replace("refuted:", "")
                                .replace("false:", "")
                                .trim()
                                .to_string();
                            verified.insert(vc);
                            verified.insert(v.clone());
                        }
                    }
                }
                Ok(json!(assumed
                    .into_iter()
                    .filter(|a| !verified.contains(a))
                    .collect::<Vec<String>>()))
            }
            "dead_branches" => {
                if guard.thought_history.is_empty() {
                    return Ok(json!([]));
                }
                let last = &guard.thought_history[guard.thought_history.len() - 1];
                let mut main_chain = HashSet::new();
                let mut queue = vec![last.thought_number];
                while let Some(tn) = queue.pop() {
                    if main_chain.insert(tn) {
                        if let Some(t) = guard
                            .thought_history
                            .iter()
                            .find(|x| x.thought_number == tn)
                        {
                            if let Some(ref parents) = t.parent_thoughts {
                                queue.extend(parents.iter().copied());
                            }
                            if let Some(bf) = t.branch_from_thought {
                                queue.push(bf);
                            }
                            if let Some(rev) = t.revises_thought {
                                queue.push(rev);
                            }
                            if t.parent_thoughts.is_none()
                                && t.branch_from_thought.is_none()
                                && !t.is_revision.unwrap_or(false)
                                && t.thought_number > 1
                            {
                                queue.push(t.thought_number - 1);
                            }
                        }
                    }
                }
                let dead: Vec<ThoughtData> = guard
                    .thought_history
                    .iter()
                    .filter(|t| !main_chain.contains(&t.thought_number))
                    .cloned()
                    .collect();
                Ok(json!(dead))
            }
            "summary_stats" => {
                let report = analyze_quality(&session_id, &guard.thought_history);
                Ok(json!({
                    "sessionId": session_id,
                    "totalThoughts": guard.thought_history.len(),
                    "averageConfidence": report.average_confidence,
                    "branchesCount": guard.branches.len(),
                    "totalAssumptions": report.assumptions_count,
                    "totalVerifiedAssumptions": report.verified_assumptions_count,
                    "qualityScore": report.quality_score,
                    "grade": report.grade,
                }))
            }
            "quality_report" => {
                let report = analyze_quality(&session_id, &guard.thought_history);
                Ok(json!(report))
            }
            _ => Err(anyhow!("Unknown query type: {}", query)),
        }
    }
}

// ─── Tool 3: ExportSessionTool ───────────────────────────────────

pub struct ExportSessionTool;

#[async_trait::async_trait]
impl Tool for ExportSessionTool {
    fn name(&self) -> &str {
        "export_session"
    }

    fn description(&self) -> &str {
        "Export the reasoning session in various formats: mermaid graph, JSON Graph, markdown report, or Graphviz DOT format."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "format": {
                    "type": "string", "enum": ["mermaid", "json", "markdown", "dot"],
                    "description": "The target export format"
                },
                "sessionId": { "type": "string", "description": "Session to export (defaults to active session)" }
            },
            "required": ["format"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let engine = get_engine();
        let mut guard = engine.lock().await;

        let (format_arg, session_id_arg) = match arguments {
            Value::String(s) => {
                let lower = s.trim().to_lowercase();
                if matches!(
                    lower.as_str(),
                    "mermaid" | "json" | "markdown" | "md" | "dot" | "graph"
                ) {
                    (Some(lower), None)
                } else {
                    (None, Some(s.trim().to_string()))
                }
            }
            Value::Object(map) => {
                let fmt = get_string_field(map, &["format", "type", "export_format", "as"]);
                let sid = get_string_field(map, &["sessionId", "session_id", "session", "id"]);
                (fmt, sid)
            }
            _ => (None, None),
        };

        let session_id = session_id_arg
            .or_else(|| {
                if !guard.current_session_id.is_empty() {
                    Some(guard.current_session_id.clone())
                } else {
                    guard
                        .store
                        .list_sessions()
                        .ok()
                        .and_then(|list| list.into_iter().next().map(|s| s.id))
                }
            });

        let session_id = match session_id {
            Some(id) if !id.trim().is_empty() => id,
            _ => {
                return Ok(json!({
                    "status": "no_session",
                    "message": "No thinking sessions found to export. Create thoughts using sequentialthinking first."
                }));
            }
        };

        if let Err(e) = guard.load_session(&session_id) {
            return Ok(json!({
                "status": "not_found",
                "sessionId": session_id,
                "error": format!("Session not found: {}", e)
            }));
        }

        let raw_format = format_arg.unwrap_or_else(|| "markdown".to_string());
        let normalized = raw_format.trim().to_lowercase().replace('-', "_");
        let format = match normalized.as_str() {
            "mermaid" | "graph" => "mermaid",
            "json" => "json",
            "markdown" | "md" | "text" => "markdown",
            "dot" | "graphviz" => "dot",
            _ => "markdown",
        };

        match format {
            "mermaid" => {
                let mermaid_graph = generate_mermaid(&guard.thought_history);
                Ok(json!({ "format": "mermaid", "sessionId": session_id, "data": mermaid_graph }))
            }
            "json" => {
                let mut nodes = Vec::new();
                let mut edges = Vec::new();
                for (i, t) in guard.thought_history.iter().enumerate() {
                    nodes.push(json!({
                        "id": format!("T{}", t.thought_number), "thoughtNumber": t.thought_number,
                        "thought": t.thought, "confidenceScore": t.confidence_score, "timestamp": t.timestamp,
                    }));
                    if let Some(ref parents) = t.parent_thoughts {
                        for p in parents {
                            edges.push(json!({ "source": format!("T{}", p), "target": format!("T{}", t.thought_number), "type": "parent" }));
                        }
                        continue;
                    }
                    if let Some(bf) = t.branch_from_thought {
                        edges.push(json!({ "source": format!("T{}", bf), "target": format!("T{}", t.thought_number), "type": "branch" }));
                    } else if t.is_revision.unwrap_or(false) {
                        if let Some(rev) = t.revises_thought {
                            edges.push(json!({ "source": format!("T{}", rev), "target": format!("T{}", t.thought_number), "type": "revision" }));
                        }
                    } else if i > 0 {
                        edges.push(json!({ "source": format!("T{}", guard.thought_history[i - 1].thought_number), "target": format!("T{}", t.thought_number), "type": "standard" }));
                    }
                }
                Ok(
                    json!({ "format": "json", "sessionId": session_id, "data": { "nodes": nodes, "edges": edges } }),
                )
            }
            "markdown" => {
                let mut md = String::new();
                md.push_str(&format!(
                    "# Reasoning Session History - Session `{}`\n\n",
                    session_id
                ));
                for t in &guard.thought_history {
                    let kind = if t.is_revision.unwrap_or(false) {
                        "Revision"
                    } else if t.branch_from_thought.is_some() {
                        "Branch"
                    } else {
                        "Thought"
                    };
                    md.push_str(&format!("## {} {}\n", kind, t.thought_number));
                    if let Some(ts) = t.timestamp {
                        md.push_str(&format!(
                            "*Timestamp: {}*\n\n",
                            ts.format("%Y-%m-%d %H:%M:%S UTC")
                        ));
                    }
                    md.push_str(&format!("{}\n\n", t.thought));
                    if let Some(ref ass) = t.assumptions {
                        if !ass.is_empty() {
                            md.push_str("### Assumptions\n");
                            for a in ass {
                                md.push_str(&format!("- 🤔 {}\n", a));
                            }
                            md.push('\n');
                        }
                    }
                    if let Some(ref ver) = t.verified_assumptions {
                        if !ver.is_empty() {
                            md.push_str("### Verified Assumptions\n");
                            for v in ver {
                                md.push_str(&format!("- ✅ {}\n", v));
                            }
                            md.push('\n');
                        }
                    }
                    if let Some(conf) = t.confidence_score {
                        md.push_str(&format!(
                            "*Confidence Score: {}/5 ({:.0}%)*\n\n",
                            (conf * 5.0).round(),
                            conf * 100.0
                        ));
                    }
                    if let Some(ref c) = t.criticism {
                        md.push_str(&format!("> **🧐 Self-Criticism:** {}\n\n", c));
                    }
                    if let Some(ref h) = t.hypothesis {
                        md.push_str(&format!("> **🔬 Hypothesis:** {}\n\n", h));
                    }
                    if let Some(ref vm) = t.verification_method {
                        md.push_str(&format!("> **🧪 Verification:** {}\n\n", vm));
                    }
                    md.push_str("---\n\n");
                }
                Ok(json!({ "format": "markdown", "sessionId": session_id, "data": md }))
            }
            "dot" => {
                let mut dot = String::from(
                    "digraph G {\n  node [shape=box, style=filled, fontname=\"Arial\"];\n",
                );
                for (i, t) in guard.thought_history.iter().enumerate() {
                    let id = format!("T{}", t.thought_number);
                    let preview: String = t.thought.chars().take(20).collect();
                    let color = if t.is_revision.unwrap_or(false) {
                        "\"#fafd7c\""
                    } else if t.branch_from_thought.is_some() {
                        "\"#a1e887\""
                    } else {
                        "\"#a5ccf7\""
                    };
                    dot.push_str(&format!(
                        "  {} [label=\"T{}: {}...\", fillcolor={}];\n",
                        id, t.thought_number, preview, color
                    ));
                    if let Some(ref parents) = t.parent_thoughts {
                        for p in parents {
                            dot.push_str(&format!("  T{} -> {};\n", p, id));
                        }
                        continue;
                    }
                    if let Some(bf) = t.branch_from_thought {
                        dot.push_str(&format!("  T{} -> {};\n", bf, id));
                    } else if t.is_revision.unwrap_or(false) {
                        if let Some(rev) = t.revises_thought {
                            dot.push_str(&format!(
                                "  T{} -> {} [style=dotted, label=\"revises\"];\n",
                                rev, id
                            ));
                        }
                    } else if i > 0 {
                        dot.push_str(&format!(
                            "  T{} -> {};\n",
                            guard.thought_history[i - 1].thought_number,
                            id
                        ));
                    }
                }
                dot.push_str("}\n");
                Ok(json!({ "format": "dot", "sessionId": session_id, "data": dot }))
            }
            _ => Err(anyhow!("Unknown format: {}", format)),
        }
    }
}

// ─── Tool 4: SummarizeReasoningTool ──────────────────────────────

pub struct SummarizeReasoningTool;

#[async_trait::async_trait]
impl Tool for SummarizeReasoningTool {
    fn name(&self) -> &str {
        "summarize_reasoning"
    }

    fn description(&self) -> &str {
        "Retrieve a structured summary and timeline of the reasoning chain for the specified session."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "sessionId": { "type": "string", "description": "Session to summarize (defaults to active session)" }
            }
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let engine = get_engine();
        let mut guard = engine.lock().await;

        let session_id_arg = match arguments {
            Value::String(s) => Some(s.trim().to_string()),
            Value::Object(map) => {
                get_string_field(map, &["sessionId", "session_id", "session", "id"])
            }
            _ => None,
        };

        let session_id = session_id_arg
            .or_else(|| {
                if !guard.current_session_id.is_empty() {
                    Some(guard.current_session_id.clone())
                } else {
                    guard
                        .store
                        .list_sessions()
                        .ok()
                        .and_then(|list| list.into_iter().next().map(|s| s.id))
                }
            });

        let session_id = match session_id {
            Some(id) if !id.trim().is_empty() => id,
            _ => {
                return Ok(json!({
                    "status": "no_session",
                    "message": "No thinking sessions found to summarize. Create thoughts using sequentialthinking first."
                }));
            }
        };

        if let Err(e) = guard.load_session(&session_id) {
            return Ok(json!({
                "status": "not_found",
                "sessionId": session_id,
                "error": format!("Session not found: {}", e)
            }));
        }

        let total_thoughts = guard.thought_history.len();
        let total_branches = guard.branches.len();

        let confidences: Vec<f64> = guard
            .thought_history
            .iter()
            .filter_map(|t| t.confidence_score)
            .collect();
        let average_confidence = if confidences.is_empty() {
            0.0
        } else {
            confidences.iter().sum::<f64>() / confidences.len() as f64
        };

        let mut merge_points = Vec::new();
        for t in &guard.thought_history {
            if let Some(ref parents) = t.parent_thoughts {
                if parents.len() > 1 {
                    merge_points.push(t.thought_number);
                }
            }
        }

        let mut assumed = HashSet::new();
        let mut verified = HashSet::new();
        for t in &guard.thought_history {
            if let Some(ref ass) = t.assumptions {
                for a in ass {
                    assumed.insert(a.clone());
                }
            }
            if let Some(ref ver) = t.verified_assumptions {
                for v in ver {
                    let vc = v
                        .replace("verified:", "")
                        .replace("refuted:", "")
                        .replace("false:", "")
                        .trim()
                        .to_string();
                    verified.insert(vc);
                    verified.insert(v.clone());
                }
            }
        }
        let unverified_assumptions: Vec<String> = assumed
            .into_iter()
            .filter(|a| !verified.contains(a))
            .collect();

        let open_todos = guard
            .thought_history
            .last()
            .and_then(|t| t.left_to_be_done.clone())
            .unwrap_or_default();

        let mut parts = Vec::new();
        for t in &guard.thought_history {
            let mut part = format!("T{}", t.thought_number);
            if let Some(bf) = t.branch_from_thought {
                let bid = t.branch_id.as_deref().unwrap_or("unknown");
                part = format!("{}(branch:{}, from:T{})", part, bid, bf);
            } else if let Some(ref parents) = t.parent_thoughts {
                if parents.len() > 1 {
                    let p_str: Vec<String> = parents.iter().map(|p| format!("T{}", p)).collect();
                    part = format!("{}(merge:{})", part, p_str.join("+"));
                }
            } else if t.is_revision.unwrap_or(false) {
                if let Some(rev) = t.revises_thought {
                    part = format!("{}(revises:T{})", part, rev);
                }
            }
            parts.push(part);
        }

        Ok(json!({
            "sessionId": session_id, "totalThoughts": total_thoughts, "totalBranches": total_branches,
            "mergePoints": merge_points, "averageConfidence": average_confidence,
            "unverifiedAssumptions": unverified_assumptions, "openTodos": open_todos,
            "timeline": parts.join(" → "),
        }))
    }
}

// ─── Tool 5: TemplatesTool ───────────────────────────────────────

pub struct TemplatesTool;

#[async_trait::async_trait]
impl Tool for TemplatesTool {
    fn name(&self) -> &str {
        "reasoning_templates"
    }

    fn description(&self) -> &str {
        "Retrieve pre-structured reasoning templates to guide complex thinking processes. Includes divide-and-conquer, hypothesis testing, and devil's advocate reasoning."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "template": {
                    "type": "string", "enum": ["divide-and-conquer", "hypothesis-test", "devils-advocate", "all"],
                    "default": "all", "description": "The reasoning template to retrieve"
                }
            }
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let raw_name = match arguments {
            Value::String(s) => s.trim().to_string(),
            Value::Object(map) => {
                get_string_field(map, &["template", "name", "type", "template_name", "id"])
                    .unwrap_or_else(|| "all".to_string())
            }
            _ => "all".to_string(),
        };

        let normalized = raw_name.trim().to_lowercase().replace('_', "-");
        let template_name = match normalized.as_str() {
            "divide-and-conquer" | "divide_and_conquer" | "divide" => "divide-and-conquer",
            "hypothesis-test" | "hypothesis_test" | "hypothesis" => "hypothesis-test",
            "devils-advocate" | "devils_advocate" | "devil" => "devils-advocate",
            _ => "all",
        };

        let divide_and_conquer = json!({
            "name": "Divide and Conquer", "id": "divide-and-conquer",
            "description": "Decompose a large, complex problem into smaller, independent sub-problems.",
            "recommendedSteps": [
                { "step": 1, "title": "Problem Scope & Boundary Analysis", "description": "Define the problem, inputs, outputs, and constraints.", "propertiesToSet": ["assumptions"] },
                { "step": 2, "title": "Decomposition Strategy", "description": "Divide into smaller sub-problems. Formulate a hypothesis for combining results.", "propertiesToSet": ["hypothesis"] },
                { "step": 3, "title": "Sub-problem Exploration & Branching", "description": "Spawn branches for each sub-problem.", "propertiesToSet": ["branchId", "branchFromThought"] },
                { "step": 4, "title": "Synthesis & Solution Merge", "description": "Merge branches and synthesize results.", "propertiesToSet": ["parentThoughts", "verifiedAssumptions"] }
            ]
        });

        let hypothesis_test = json!({
            "name": "Hypothesis Testing", "id": "hypothesis-test",
            "description": "Establish a testable hypothesis, identify assumptions, design verification, and evaluate.",
            "recommendedSteps": [
                { "step": 1, "title": "Hypothesis Formulation", "description": "Define a testable, falsifiable hypothesis.", "propertiesToSet": ["hypothesis", "verificationMethod"] },
                { "step": 2, "title": "Assumption Mapping", "description": "List all assumptions required for the hypothesis.", "propertiesToSet": ["assumptions"] },
                { "step": 3, "title": "Evidence Gathering & Verification", "description": "Verify assumptions using the defined method.", "propertiesToSet": ["verifiedAssumptions", "confidenceScore"] },
                { "step": 4, "title": "Synthesis / Backtracking", "description": "Confirm or refute the hypothesis. Revise if needed.", "propertiesToSet": ["isRevision", "revisesThought", "criticism"] }
            ]
        });

        let devils_advocate = json!({
            "name": "Devil's Advocate", "id": "devils-advocate",
            "description": "Identify biases, challenge assumptions, find edge cases and failure modes.",
            "recommendedSteps": [
                { "step": 1, "title": "Proposed Solution", "description": "State the current preferred solution.", "propertiesToSet": ["thought"] },
                { "step": 2, "title": "Assumption Enumeration", "description": "List every supporting assumption.", "propertiesToSet": ["assumptions"] },
                { "step": 3, "title": "Adversarial Challenge", "description": "Challenge each assumption. Describe failure modes.", "propertiesToSet": ["criticism"] },
                { "step": 4, "title": "Solution Hardening", "description": "Revise to address criticisms.", "propertiesToSet": ["isRevision", "revisesThought", "leftToBeDone"] }
            ]
        });

        match template_name {
            "divide-and-conquer" => Ok(divide_and_conquer),
            "hypothesis-test" => Ok(hypothesis_test),
            "devils-advocate" => Ok(devils_advocate),
            _ => {
                Ok(json!({ "templates": [divide_and_conquer, hypothesis_test, devils_advocate] }))
            }
        }
    }
}
