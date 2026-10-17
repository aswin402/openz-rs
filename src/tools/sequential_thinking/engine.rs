use super::store::{QualityReport, ThoughtData};
use std::collections::{HashMap, HashSet};

// ─── Mermaid generation ──────────────────────────────────────────

pub(crate) fn classify_thought_node(t: &ThoughtData) -> (&'static str, &'static str) {
    [
        (t.is_revision.unwrap_or(false), ("revision", "Revision")),
        (t.branch_from_thought.is_some(), ("branch", "Branch")),
        (t.hypothesis.is_some(), ("hypothesis", "Thought")),
    ]
    .into_iter()
    .find_map(|(matched, pair)| matched.then_some(pair))
    .unwrap_or(("standard", "Thought"))
}

pub fn generate_mermaid(thoughts: &[ThoughtData]) -> String {
    let mut mermaid = String::from("graph TD\n");
    mermaid.push_str(
        "    classDef revision fill:#fafd7c,stroke:#d4b200,stroke-width:2px,color:#000;\n",
    );
    mermaid
        .push_str("    classDef branch fill:#a1e887,stroke:#3b7a14,stroke-width:2px,color:#000;\n");
    mermaid.push_str(
        "    classDef hypothesis fill:#d1b3ff,stroke:#6a3d9a,stroke-width:2px,color:#000;\n",
    );
    mermaid.push_str(
        "    classDef standard fill:#a5ccf7,stroke:#265c96,stroke-width:2px,color:#000;\n\n",
    );

    for (i, t) in thoughts.iter().enumerate() {
        let id = format!("T{}", t.thought_number);
        let mut preview: String = t.thought.chars().take(30).collect();
        preview = preview
            .replace('\"', "'")
            .replace('[', "(")
            .replace(']', ")");
        match t.thought.len() > 30 {
            true => preview.push_str("..."),
            false => {}
        }

        let (class, _) = classify_thought_node(t);

        mermaid.push_str(&format!(
            "    {id}[\"T{num}: {preview}\"]\n",
            id = id,
            num = t.thought_number,
            preview = preview
        ));
        mermaid.push_str(&format!("    class {id} {class}\n", id = id, class = class));

        let non_empty_parents = t.parent_thoughts.as_ref().filter(|p| !p.is_empty());
        match non_empty_parents {
            Some(parents) => {
                for parent in parents {
                    mermaid.push_str(&format!(
                        "    T{parent} --> {id}\n",
                        parent = parent,
                        id = id
                    ));
                }
                continue;
            }
            None => {}
        }

        match (
            t.branch_from_thought,
            t.is_revision.unwrap_or(false),
            t.revises_thought,
            i > 0,
        ) {
            (Some(branch_from), _, _, _) => {
                mermaid.push_str(&format!(
                    "    T{branch_from} --> {id}\n",
                    branch_from = branch_from,
                    id = id
                ));
            }
            (None, true, Some(revises), _) => {
                mermaid.push_str(&format!(
                    "    T{revises} -.->|revises| {id}\n",
                    revises = revises,
                    id = id
                ));
            }
            (None, true, None, _) => {}
            (None, false, _, true) => {
                let prev = thoughts[i - 1].thought_number;
                mermaid.push_str(&format!("    T{prev} --> {id}\n", prev = prev, id = id));
            }
            (None, false, _, false) => {}
        }
    }
    mermaid
}

// ─── Loop / Cycle detection ──────────────────────────────────────

pub fn detect_loop(thoughts: &[ThoughtData]) -> Option<Vec<usize>> {
    let mut adj: HashMap<usize, Vec<usize>> = HashMap::new();
    for t in thoughts {
        let mut deps = Vec::new();
        match &t.parent_thoughts {
            Some(parents) => deps.extend(parents.iter().copied()),
            None => {}
        }
        match t.branch_from_thought {
            Some(branch_from) => deps.push(branch_from),
            None => {}
        }
        match t.revises_thought {
            Some(revises) => deps.push(revises),
            None => {}
        }
        adj.insert(t.thought_number, deps);
    }
    let mut visited = HashSet::new();
    let mut rec_stack = HashSet::new();
    let mut path = Vec::new();
    adj.keys().copied().find_map(|node| {
        (!visited.contains(&node))
            .then(|| dfs_cycle(node, &adj, &mut visited, &mut rec_stack, &mut path))
            .flatten()
    })
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
    let found = adj.get(&node).and_then(|neighbors| {
        neighbors.iter().copied().find_map(|neighbor| {
            match (visited.contains(&neighbor), rec_stack.contains(&neighbor)) {
                (false, _) => dfs_cycle(neighbor, adj, visited, rec_stack, path),
                (true, true) => path.iter().position(|&x| x == neighbor).map(|pos| {
                    let mut cycle_path = path[pos..].to_vec();
                    cycle_path.push(neighbor);
                    cycle_path
                }),
                (true, false) => None,
            }
        })
    });
    match found {
        Some(cycle) => Some(cycle),
        None => {
            rec_stack.remove(&node);
            path.pop();
            None
        }
    }
}

// ─── Quality calculation ─────────────────────────────────────────

const GRADE_THRESHOLDS: &[(f64, &str)] = &[(90.0, "A"), (80.0, "B"), (70.0, "C"), (60.0, "D")];

pub fn analyze_quality(session_id: &str, thoughts: &[ThoughtData]) -> QualityReport {
    let total_thoughts = thoughts.len();

    let confidences: Vec<f64> = thoughts.iter().filter_map(|t| t.confidence_score).collect();
    let average_confidence = match confidences.is_empty() {
        true => 0.75,
        false => confidences.iter().sum::<f64>() / confidences.len() as f64,
    };

    let mut assumed = HashSet::new();
    let mut verified = HashSet::new();
    let mut refuted = HashSet::new();

    for t in thoughts {
        match &t.assumptions {
            Some(ass) => {
                for a in ass {
                    assumed.insert(a.trim().to_lowercase());
                }
            }
            None => {}
        }
        match &t.verified_assumptions {
            Some(ver) => {
                for v in ver {
                    let vc = v.trim().to_lowercase();
                    let is_refuted =
                        vc.contains("refuted") || vc.contains("false") || vc.contains("falsified");
                    match is_refuted {
                        true => {
                            let core = vc
                                .replace("refuted:", "")
                                .replace("refuted", "")
                                .replace("false:", "")
                                .replace("false", "")
                                .replace("falsified:", "")
                                .replace("falsified", "")
                                .trim()
                                .to_string();
                            refuted.insert(core);
                        }
                        false => {
                            let core = vc
                                .replace("verified:", "")
                                .replace("verified", "")
                                .trim()
                                .to_string();
                            verified.insert(core);
                            verified.insert(vc);
                        }
                    }
                }
            }
            None => {}
        }
    }

    let assumptions_count = assumed.len();
    let verified_assumptions_count = assumed
        .iter()
        .filter(|a| verified.contains(*a) || refuted.contains(*a))
        .count();
    let verified_assumptions_ratio = match assumptions_count {
        0 => 1.0,
        n => verified_assumptions_count as f64 / n as f64,
    };

    let contradictions: Vec<String> = assumed
        .intersection(&refuted)
        .map(|s| format!("Assumption '{}' is declared but refuted/falsified.", s))
        .collect();
    let contradictions_count = contradictions.len();
    let loop_path = detect_loop(thoughts);
    let loop_detected = loop_path.is_some();

    let mut score = average_confidence * 40.0 + verified_assumptions_ratio * 40.0;
    score += f64::from(u8::from(total_thoughts > 0)) * 20.0;
    score -= (contradictions_count as f64 * 20.0).min(40.0);
    score -= f64::from(u8::from(loop_detected)) * 30.0;
    let quality_score = score.clamp(0.0, 100.0);

    let grade = GRADE_THRESHOLDS
        .iter()
        .find_map(|&(min_score, g)| (quality_score >= min_score).then_some(g))
        .unwrap_or("F")
        .to_string();

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

fn append_markdown_bullet_section(md: &mut String, heading: &str, icon: &str, items: Option<&Vec<String>>) {
    match items.filter(|list| !list.is_empty()) {
        Some(list) => {
            md.push_str(&format!("### {heading}\n"));
            for item in list {
                md.push_str(&format!("- {icon} {item}\n"));
            }
            md.push('\n');
        }
        None => {}
    }
}

fn append_markdown_callout(md: &mut String, label: &str, value: Option<&String>) {
    match value {
        Some(v) => md.push_str(&format!("> **{label}:** {v}\n\n")),
        None => {}
    }
}

pub fn export_session_as_markdown(session_id: &str, thoughts: &[ThoughtData]) -> String {
    let mut md = String::new();
    md.push_str(&format!(
        "# Reasoning Session History - Session `{}`\n\n",
        session_id
    ));
    for t in thoughts {
        let (_, kind) = classify_thought_node(t);
        md.push_str(&format!("## {} {}\n", kind, t.thought_number));
        match t.timestamp {
            Some(ts) => md.push_str(&format!(
                "*Timestamp: {}*\n\n",
                ts.format("%Y-%m-%d %H:%M:%S UTC")
            )),
            None => {}
        }
        md.push_str(&format!("{}\n\n", t.thought));
        append_markdown_bullet_section(&mut md, "Assumptions", "🤔", t.assumptions.as_ref());
        append_markdown_bullet_section(
            &mut md,
            "Verified Assumptions",
            "✅",
            t.verified_assumptions.as_ref(),
        );
        match t.confidence_score {
            Some(conf) => md.push_str(&format!(
                "*Confidence Score: {}/5 ({:.0}%)*\n\n",
                (conf * 5.0).round(),
                conf * 100.0
            )),
            None => {}
        }
        append_markdown_callout(&mut md, "🧐 Self-Criticism", t.criticism.as_ref());
        append_markdown_callout(&mut md, "🔬 Hypothesis", t.hypothesis.as_ref());
        append_markdown_callout(&mut md, "🧪 Verification", t.verification_method.as_ref());
        md.push_str("---\n\n");
    }
    md
}
