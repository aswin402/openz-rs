use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const CONVENTIONAL_TYPES: &[&str] = &[
    "feat", "fix", "refactor", "perf", "test", "docs", "build", "ci", "chore", "style", "revert",
];

const PAST_TO_IMPERATIVE: &[(&str, &str)] = &[
    ("added", "add"),
    ("adding", "add"),
    ("adds", "add"),
    ("updated", "update"),
    ("updating", "update"),
    ("updates", "update"),
    ("fixed", "fix"),
    ("fixing", "fix"),
    ("fixes", "fix"),
    ("removed", "remove"),
    ("removing", "remove"),
    ("removes", "remove"),
    ("refactored", "refactor"),
    ("refactoring", "refactor"),
    ("implemented", "implement"),
    ("implementing", "implement"),
    ("optimized", "optimize"),
    ("optimizing", "optimize"),
];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitAnalysis {
    pub inferred_type: String,
    pub inferred_scope: Option<String>,
    pub files_changed: Vec<String>,
    pub additions: usize,
    pub deletions: usize,
    pub suggested_header: String,
    pub bullet_points: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidatedCommitMessage {
    pub subject: String,
    pub body_bullets: Vec<String>,
    pub formatted: String,
    pub subject_chars: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffHunkMap {
    pub files: Vec<String>,
    pub added_lines: Vec<(String, usize, String)>,
    pub total_added: usize,
    pub total_removed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewFinding {
    pub file: Option<String>,
    pub line: usize,
    pub severity: String,
    pub message: String,
}

fn infer_scope_from_paths(paths: &[String]) -> Option<String> {
    let mut bucket_counts: BTreeMap<String, usize> = BTreeMap::new();
    for path in paths {
        let parts: Vec<&str> = path
            .trim_start_matches("a/")
            .trim_start_matches("b/")
            .split('/')
            .filter(|s| !s.is_empty())
            .collect();
        let scope = match parts.as_slice() {
            ["crates", crate_name, ..] => Some((*crate_name).to_string()),
            ["src", module_dir, _rest, ..] => Some((*module_dir).to_string()),
            ["web", "src", "features", feat, ..] => Some(format!("web-{feat}")),
            ["web", ..] => Some("webui".to_string()),
            [single] => single
                .split('.')
                .next()
                .filter(|s| !s.is_empty())
                .map(|s| s.to_ascii_lowercase()),
            _ => None,
        };
        match scope {
            Some(s) => {
                *bucket_counts.entry(s).or_insert(0) += 1;
            }
            None => {}
        }
    }

    bucket_counts
        .into_iter()
        .max_by_key(|(_, count)| *count)
        .map(|(scope, _)| scope.replace('_', "-"))
}

fn infer_commit_type(paths: &[String], added_lines: &[String], deletions: usize) -> &'static str {
    let all_docs = !paths.is_empty()
        && paths
            .iter()
            .all(|p| p.ends_with(".md") || p.starts_with("docs/"));
    let all_tests = !paths.is_empty()
        && paths
            .iter()
            .all(|p| p.contains("test") || p.ends_with(".test.ts") || p.ends_with("_tests.rs"));
    let all_build = !paths.is_empty()
        && paths.iter().all(|p| {
            p.ends_with("Cargo.toml")
                || p.ends_with("package.json")
                || p.ends_with("justfile")
                || p.starts_with(".github/")
        });

    let joined_lower = added_lines.join("\n").to_ascii_lowercase();
    let has_fix_signal = joined_lower.contains("fix")
        || joined_lower.contains("bug")
        || joined_lower.contains("guard")
        || joined_lower.contains("prevent");
    let has_new_pub_symbol = added_lines.iter().any(|l| {
        let t = l.trim_start();
        t.starts_with("pub fn ")
            || t.starts_with("pub struct ")
            || t.starts_with("pub enum ")
            || t.starts_with("export function ")
            || t.starts_with("export const ")
    });

    match (
        all_docs,
        all_tests,
        all_build,
        has_new_pub_symbol,
        has_fix_signal,
        deletions > added_lines.len(),
    ) {
        (true, _, _, _, _, _) => "docs",
        (false, true, _, _, _, _) => "test",
        (false, false, true, _, _, _) => "build",
        (false, false, false, true, _, _) => "feat",
        (false, false, false, false, true, _) => "fix",
        (false, false, false, false, false, true) => "refactor",
        _ => "chore",
    }
}

/// Deterministically inspects a unified git diff to derive Conventional Commit metadata.
pub fn analyze_diff_for_commit(diff_text: &str, scope_override: Option<&str>) -> CommitAnalysis {
    let mut files: Vec<String> = Vec::new();
    let mut added_lines: Vec<String> = Vec::new();
    let mut additions = 0usize;
    let mut deletions = 0usize;

    for line in diff_text.lines() {
        match (
            line.strip_prefix("+++ b/"),
            line.starts_with("+++"),
            line.starts_with("---"),
            line.strip_prefix('+'),
            line.strip_prefix('-'),
        ) {
            (Some(file_path), _, _, _, _) => {
                let clean = file_path.trim().to_string();
                match clean != "/dev/null" && !files.contains(&clean) {
                    true => files.push(clean),
                    false => {}
                }
            }
            (None, false, _, Some(added), _) => {
                additions += 1;
                added_lines.push(added.to_string());
            }
            (None, _, false, None, Some(_)) => {
                deletions += 1;
            }
            _ => {}
        }
    }

    let inferred_type = infer_commit_type(&files, &added_lines, deletions).to_string();
    let inferred_scope = scope_override
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| infer_scope_from_paths(&files));

    let action_verb = match inferred_type.as_str() {
        "feat" => "add",
        "fix" => "fix",
        "docs" => "update",
        "test" => "cover",
        "refactor" => "streamline",
        _ => "update",
    };

    let primary_target = files
        .first()
        .and_then(|f| f.rsplit('/').next())
        .and_then(|name| name.split('.').next())
        .unwrap_or("workspace")
        .replace('_', " ");

    let raw_header = match &inferred_scope {
        Some(scope) => format!("{inferred_type}({scope}): {action_verb} {primary_target}"),
        None => format!("{inferred_type}: {action_verb} {primary_target}"),
    };

    let mut bullets = Vec::new();
    match files.len() > 1 {
        true => bullets.push(format!(
            "touch {} files (+{} / -{})",
            files.len(),
            additions,
            deletions
        )),
        false => {}
    }

    let validated = validate_and_format_commit(&raw_header, &bullets);
    CommitAnalysis {
        inferred_type,
        inferred_scope,
        files_changed: files,
        additions,
        deletions,
        suggested_header: validated.subject,
        bullet_points: validated.body_bullets,
    }
}

fn normalize_imperative_words(desc: &str) -> String {
    let mut words: Vec<String> = desc
        .trim()
        .trim_end_matches('.')
        .split_whitespace()
        .map(str::to_string)
        .collect();

    match words.first_mut() {
        Some(first) => {
            let lower = first.to_ascii_lowercase();
            let mapped = PAST_TO_IMPERATIVE
                .iter()
                .find(|(past, _)| *past == lower)
                .map(|(_, imp)| (*imp).to_string())
                .unwrap_or(lower);
            *first = mapped;
        }
        None => {}
    }

    words.join(" ")
}

fn truncate_at_word_boundary(text: &str, max_chars: usize) -> String {
    let char_count = text.chars().count();
    match char_count <= max_chars {
        true => text.to_string(),
        false => {
            let mut acc = String::new();
            for word in text.split_whitespace() {
                let next_len = match acc.is_empty() {
                    true => word.chars().count(),
                    false => acc.chars().count() + 1 + word.chars().count(),
                };
                match next_len <= max_chars {
                    true => {
                        match acc.is_empty() {
                            true => {}
                            false => acc.push(' '),
                        }
                        acc.push_str(word);
                    }
                    false => break,
                }
            }
            match acc.is_empty() {
                true => text.chars().take(max_chars).collect(),
                false => acc,
            }
        }
    }
}

/// Enforces `caveman_commit` invariants: Conventional Commit header `<= 50` chars,
/// imperative verb, stripped AI/co-author noise, concise bullets.
pub fn validate_and_format_commit(
    raw_subject: &str,
    raw_bullets: &[String],
) -> ValidatedCommitMessage {
    let first_line = raw_subject
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with("```"))
        .unwrap_or("chore: update workspace");

    let (prefix, rest) = match first_line.find(':') {
        Some(colon_idx) => {
            let cand_prefix = first_line[..colon_idx].trim();
            let base_type = cand_prefix
                .split('(')
                .next()
                .unwrap_or("")
                .trim_end_matches('!');
            let valid_cc = CONVENTIONAL_TYPES.contains(&base_type);
            match valid_cc {
                true => (cand_prefix.to_string(), first_line[colon_idx + 1..].trim()),
                false => ("chore".to_string(), first_line),
            }
        }
        None => ("chore".to_string(), first_line),
    };

    let normalized_desc = normalize_imperative_words(rest);
    let prefix_head = format!("{prefix}: ");
    let budget = 50usize.saturating_sub(prefix_head.chars().count()).max(12);
    let clamped_desc = truncate_at_word_boundary(&normalized_desc, budget);
    let subject = format!("{prefix_head}{clamped_desc}");
    let subject_chars = subject.chars().count();

    let body_bullets: Vec<String> = raw_bullets
        .iter()
        .map(|b| b.trim().trim_start_matches('-').trim_start_matches('*').trim())
        .filter(|b| {
            let lower = b.to_ascii_lowercase();
            !b.is_empty()
                && !lower.starts_with("co-authored-by:")
                && !lower.contains("generated with")
                && !lower.contains("ai assistant")
        })
        .take(4)
        .map(|b| format!("- {}", truncate_at_word_boundary(b, 70)))
        .collect();

    let formatted = match body_bullets.is_empty() {
        true => subject.clone(),
        false => format!("{subject}\n\n{}", body_bullets.join("\n")),
    };

    ValidatedCommitMessage {
        subject,
        body_bullets,
        formatted,
        subject_chars,
    }
}

fn parse_hunk_new_start_line(hunk_header: &str) -> Option<usize> {
    let plus_idx = hunk_header.find('+')?;
    let after_plus = &hunk_header[plus_idx + 1..];
    let digits: String = after_plus
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse::<usize>().ok()
}

/// Parses a unified diff into exact `(file, new_line_number, added_code)` tuples
/// for line-accurate `caveman_review` diagnostics.
pub fn analyze_diff_for_review(diff_text: &str) -> DiffHunkMap {
    let mut files = Vec::new();
    let mut added_lines = Vec::new();
    let mut current_file = String::from("unknown");
    let mut current_new_line = 1usize;
    let mut total_added = 0usize;
    let mut total_removed = 0usize;

    for line in diff_text.lines() {
        match (
            line.strip_prefix("+++ b/"),
            line.starts_with("@@ "),
            line.starts_with("+++"),
            line.starts_with("---"),
            line.strip_prefix('+'),
            line.strip_prefix('-'),
        ) {
            (Some(fp), _, _, _, _, _) => {
                current_file = fp.trim().to_string();
                match !files.contains(&current_file) {
                    true => files.push(current_file.clone()),
                    false => {}
                }
            }
            (None, true, _, _, _, _) => {
                current_new_line = parse_hunk_new_start_line(line).unwrap_or(1);
            }
            (None, false, false, _, Some(added), _) => {
                total_added += 1;
                added_lines.push((current_file.clone(), current_new_line, added.to_string()));
                current_new_line += 1;
            }
            (None, false, _, false, None, Some(_)) => {
                total_removed += 1;
            }
            _ => {
                current_new_line += 1;
            }
        }
    }

    DiffHunkMap {
        files,
        added_lines,
        total_added,
        total_removed,
    }
}

/// Formats structured or raw review findings into strict single-line `caveman_review` output:
/// `[file:]L<line>: <bug|security|perf|nit>: <problem>. <fix>.` or `LGTM`.
pub fn normalize_review_output(findings: &[ReviewFinding]) -> String {
    match findings.is_empty() {
        true => "LGTM".to_string(),
        false => findings
            .iter()
            .map(|f| {
                let sev = match f.severity.trim().to_ascii_lowercase().as_str() {
                    "bug" | "error" | "critical" => "bug",
                    "security" | "sec" | "vuln" => "security",
                    "perf" | "performance" => "perf",
                    "risk" | "warn" | "warning" => "risk",
                    _ => "nit",
                };
                let msg = f.message.trim().trim_end_matches('.');
                match f.file.as_deref().filter(|s| !s.trim().is_empty()) {
                    Some(file) => format!("{file}:L{}: {sev}: {msg}.", f.line),
                    None => format!("L{}: {sev}: {msg}.", f.line),
                }
            })
            .collect::<Vec<_>>()
            .join("\n"),
    }
}
