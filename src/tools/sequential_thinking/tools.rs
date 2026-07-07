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
            Some(id) => id.clone(),
            None => {
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
        let thought_data: ThoughtData = serde_json::from_value(arguments.clone())
            .map_err(|e| anyhow!("Invalid arguments: {}", e))?;

        let engine = get_engine();
        let mut guard = engine.lock().await;
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

        let session_id = arguments["sessionId"]
            .as_str()
            .map(String::from)
            .unwrap_or_else(|| guard.current_session_id.clone());
        if session_id.is_empty() {
            return Err(anyhow!("No active session and no sessionId provided"));
        }
        guard
            .load_session(&session_id)
            .map_err(|e| anyhow!("{}", e))?;

        let query = arguments["query"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing query parameter"))?;

        match query {
            "low_confidence" => {
                let threshold = arguments["confidenceThreshold"].as_f64().unwrap_or(0.5);
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

        let session_id = arguments["sessionId"]
            .as_str()
            .map(String::from)
            .unwrap_or_else(|| guard.current_session_id.clone());
        if session_id.is_empty() {
            return Err(anyhow!("No active session and no sessionId provided"));
        }
        guard
            .load_session(&session_id)
            .map_err(|e| anyhow!("{}", e))?;

        let format = arguments["format"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing format parameter"))?;

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

        let session_id = arguments["sessionId"]
            .as_str()
            .map(String::from)
            .unwrap_or_else(|| guard.current_session_id.clone());
        if session_id.is_empty() {
            return Err(anyhow!("No active session and no sessionId provided"));
        }
        guard
            .load_session(&session_id)
            .map_err(|e| anyhow!("{}", e))?;

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
        let template_name = arguments["template"].as_str().unwrap_or("all");

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
            "all" | _ => {
                Ok(json!({ "templates": [divide_and_conquer, hypothesis_test, devils_advocate] }))
            }
        }
    }
}
