use std::collections::{HashMap, HashSet};
use super::store::{ThoughtData, QualityReport};

// ─── Mermaid generation ──────────────────────────────────────────

pub fn generate_mermaid(thoughts: &[ThoughtData]) -> String {
    let mut mermaid = String::from("graph TD\n");
    mermaid.push_str("    classDef revision fill:#fafd7c,stroke:#d4b200,stroke-width:2px,color:#000;\n");
    mermaid.push_str("    classDef branch fill:#a1e887,stroke:#3b7a14,stroke-width:2px,color:#000;\n");
    mermaid.push_str("    classDef hypothesis fill:#d1b3ff,stroke:#6a3d9a,stroke-width:2px,color:#000;\n");
    mermaid.push_str("    classDef standard fill:#a5ccf7,stroke:#265c96,stroke-width:2px,color:#000;\n\n");

    for (i, t) in thoughts.iter().enumerate() {
        let id = format!("T{}", t.thought_number);
        let mut preview: String = t.thought.chars().take(30).collect();
        preview = preview.replace('\"', "'").replace('[', "(").replace(']', ")");
        if t.thought.len() > 30 { preview.push_str("..."); }

        let class = if t.is_revision.unwrap_or(false) { "revision" }
        else if t.branch_from_thought.is_some() { "branch" }
        else if t.hypothesis.is_some() { "hypothesis" }
        else { "standard" };

        mermaid.push_str(&format!("    {id}[\"T{num}: {preview}\"]\n", id = id, num = t.thought_number, preview = preview));
        mermaid.push_str(&format!("    class {id} {class}\n", id = id, class = class));

        if let Some(ref parents) = t.parent_thoughts {
            if !parents.is_empty() {
                for parent in parents {
                    mermaid.push_str(&format!("    T{parent} --> {id}\n", parent = parent, id = id));
                }
                continue;
            }
        }
        if let Some(branch_from) = t.branch_from_thought {
            mermaid.push_str(&format!("    T{branch_from} --> {id}\n", branch_from = branch_from, id = id));
        } else if t.is_revision.unwrap_or(false) {
            if let Some(revises) = t.revises_thought {
                mermaid.push_str(&format!("    T{revises} -.->|revises| {id}\n", revises = revises, id = id));
            }
        } else if i > 0 {
            let prev = thoughts[i - 1].thought_number;
            mermaid.push_str(&format!("    T{prev} --> {id}\n", prev = prev, id = id));
        }
    }
    mermaid
}

// ─── Loop / Cycle detection ──────────────────────────────────────

pub fn detect_loop(thoughts: &[ThoughtData]) -> Option<Vec<usize>> {
    let mut adj: HashMap<usize, Vec<usize>> = HashMap::new();
    for t in thoughts {
        let mut deps = Vec::new();
        if let Some(ref parents) = t.parent_thoughts { deps.extend(parents.iter().copied()); }
        if let Some(branch_from) = t.branch_from_thought { deps.push(branch_from); }
        if let Some(revises) = t.revises_thought { deps.push(revises); }
        adj.insert(t.thought_number, deps);
    }
    let mut visited = HashSet::new();
    let mut rec_stack = HashSet::new();
    let mut path = Vec::new();
    for &node in adj.keys() {
        if !visited.contains(&node) {
            if let Some(cycle) = dfs_cycle(node, &adj, &mut visited, &mut rec_stack, &mut path) {
                return Some(cycle);
            }
        }
    }
    None
}

fn dfs_cycle(
    node: usize,
    adj: &HashMap<usize, Vec<usize>>,
    visited: &mut HashSet<usize>,
    rec_stack: &mut HashSet<usize>,
    path: &mut Vec<usize>,
) -> Option<Vec<usize>> {
    visited.insert(node);
    rec_stack.insert(node);
    path.push(node);
    if let Some(neighbors) = adj.get(&node) {
        for &neighbor in neighbors {
            if !visited.contains(&neighbor) {
                if let Some(cycle) = dfs_cycle(neighbor, adj, visited, rec_stack, path) { return Some(cycle); }
            } else if rec_stack.contains(&neighbor) {
                if let Some(pos) = path.iter().position(|&x| x == neighbor) {
                    let mut cycle_path = path[pos..].to_vec();
                    cycle_path.push(neighbor);
                    return Some(cycle_path);
                }
            }
        }
    }
    rec_stack.remove(&node);
    path.pop();
    None
}

// ─── Quality calculation ─────────────────────────────────────────

pub fn analyze_quality(session_id: &str, thoughts: &[ThoughtData]) -> QualityReport {
    let total_thoughts = thoughts.len();

    let confidences: Vec<f64> = thoughts.iter().filter_map(|t| t.confidence_score).collect();
    let average_confidence = if confidences.is_empty() { 0.75 } else { confidences.iter().sum::<f64>() / confidences.len() as f64 };

    let mut assumed = HashSet::new();
    let mut verified = HashSet::new();
    let mut refuted = HashSet::new();

    for t in thoughts {
        if let Some(ref ass) = t.assumptions {
            for a in ass { assumed.insert(a.trim().to_lowercase()); }
        }
        if let Some(ref ver) = t.verified_assumptions {
            for v in ver {
                let vc = v.trim().to_lowercase();
                if vc.contains("refuted") || vc.contains("false") || vc.contains("falsified") {
                    let core = vc.replace("refuted:", "").replace("refuted", "").replace("false:", "").replace("false", "").replace("falsified:", "").replace("falsified", "").trim().to_string();
                    refuted.insert(core);
                } else {
                    let core = vc.replace("verified:", "").replace("verified", "").trim().to_string();
                    verified.insert(core);
                    verified.insert(vc);
                }
            }
        }
    }

    let assumptions_count = assumed.len();
    let verified_assumptions_count = assumed.iter().filter(|a| verified.contains(*a) || refuted.contains(*a)).count();
    let verified_assumptions_ratio = if assumptions_count == 0 { 1.0 } else { verified_assumptions_count as f64 / assumptions_count as f64 };

    let contradictions: Vec<String> = assumed.intersection(&refuted).map(|s| format!("Assumption '{}' is declared but refuted/falsified.", s)).collect();
    let contradictions_count = contradictions.len();
    let loop_path = detect_loop(thoughts);
    let loop_detected = loop_path.is_some();

    let mut score = average_confidence * 40.0 + verified_assumptions_ratio * 40.0;
    if total_thoughts > 0 { score += 20.0; }
    score -= (contradictions_count as f64 * 20.0).min(40.0);
    if loop_detected { score -= 30.0; }
    let quality_score = score.clamp(0.0, 100.0);

    let grade = if quality_score >= 90.0 { "A" } else if quality_score >= 80.0 { "B" } else if quality_score >= 70.0 { "C" } else if quality_score >= 60.0 { "D" } else { "F" }.to_string();

    QualityReport {
        session_id: session_id.to_string(),
        total_thoughts,
        average_confidence,
        assumptions_count,
        verified_assumptions_count,
        verified_assumptions_ratio,
        contradictions_count,
        contradictions,
        loop_detected,
        loop_path,
        quality_score,
        grade,
    }
}

// ─── Export Session as Markdown ──────────────────────────────────

pub fn export_session_as_markdown(session_id: &str, thoughts: &[ThoughtData]) -> String {
    let mut md = String::new();
    md.push_str(&format!("# Reasoning Session History - Session `{}`\n\n", session_id));
    for t in thoughts {
        let kind = if t.is_revision.unwrap_or(false) {
            "Revision"
        } else if t.branch_from_thought.is_some() {
            "Branch"
        } else {
            "Thought"
        };
        md.push_str(&format!("## {} {}\n", kind, t.thought_number));
        if let Some(ts) = t.timestamp {
            md.push_str(&format!("*Timestamp: {}*\n\n", ts.format("%Y-%m-%d %H:%M:%S UTC")));
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
            md.push_str(&format!("*Confidence Score: {}/5 ({:.0}%)*\n\n", (conf * 5.0).round(), conf * 100.0));
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
    md
}
