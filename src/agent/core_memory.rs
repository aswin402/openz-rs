use super::skills::ranker::tokenize_stemmed;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

pub const MAX_SOUL_CHARS: usize = 2_400;
pub const MAX_USER_PROFILE_CHARS: usize = 2_200;
pub const MAX_GLOBAL_MEMORY_CHARS: usize = 2_400;

static CORE_MEMORY_WRITE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryBulletMutation {
    #[serde(default)]
    pub operation: String,
    #[serde(default)]
    pub old_text: Option<String>,
    #[serde(default)]
    pub text: String,
}

pub fn get_soul_path() -> PathBuf {
    crate::config::runtime_data_dir().join("SOUL.md")
}

pub fn get_workspace_soul_path() -> PathBuf {
    crate::config::resolve_path(".openz/SOUL.md")
}

pub fn get_user_profile_path() -> PathBuf {
    crate::config::runtime_data_dir().join("USER.md")
}

pub fn get_global_memory_path() -> PathBuf {
    crate::config::runtime_data_dir().join("MEMORY.md")
}

fn truncate_chars_clean(input: &str, max_chars: usize) -> String {
    let trimmed = input.trim();
    match trimmed.chars().count() <= max_chars {
        true => trimmed.to_string(),
        false => trimmed.chars().take(max_chars).collect::<String>(),
    }
}

pub fn load_soul_profile() -> String {
    let global_path = get_soul_path();
    let workspace_path = get_workspace_soul_path();
    let global_text = global_path
        .is_file()
        .then(|| fs::read_to_string(&global_path).ok())
        .flatten()
        .unwrap_or_default();
    let workspace_text = (workspace_path != global_path && workspace_path.is_file())
        .then(|| fs::read_to_string(&workspace_path).ok())
        .flatten()
        .unwrap_or_default();

    let combined = match (
        global_text.trim().is_empty(),
        workspace_text.trim().is_empty(),
    ) {
        (true, true) => String::new(),
        (false, true) => global_text,
        (true, false) => workspace_text,
        (false, false) => format!("{}\n\n{}", global_text.trim(), workspace_text.trim()),
    };
    truncate_chars_clean(&combined, MAX_SOUL_CHARS)
}

pub fn load_user_profile() -> String {
    let path = get_user_profile_path();
    match path.is_file() {
        true => fs::read_to_string(path).unwrap_or_default(),
        false => String::new(),
    }
}

pub fn load_global_memory() -> String {
    let path = get_global_memory_path();
    match path.is_file() {
        true => fs::read_to_string(path).unwrap_or_default(),
        false => String::new(),
    }
}

pub(crate) fn clean_bullet_line(line: &str) -> Option<&str> {
    let l = line.trim();
    match l.starts_with('-') || l.starts_with('*') {
        true => Some(
            l[1..]
                .trim()
                .trim_start_matches("**")
                .trim_end_matches("**")
                .trim(),
        ),
        false => None,
    }
}

fn parse_existing_bullets(raw: &str) -> Vec<String> {
    let mut bullets: Vec<String> = Vec::new();
    for text in raw.lines().filter_map(clean_bullet_line) {
        let should_add = !text.is_empty()
            && !bullets.iter().any(|b| b.eq_ignore_ascii_case(text));
        let _ = should_add.then(|| bullets.push(text.to_string()));
    }
    bullets
}

fn bullet_topic_prefix(text: &str) -> Option<String> {
    text.split_once(':')
        .map(|(lhs, _)| lhs.trim().to_ascii_lowercase())
        .filter(|lhs| lhs.len() >= 3 && lhs.len() <= 36 && !lhs.contains("http"))
}

fn stemmed_jaccard_similarity(a: &str, b: &str) -> f32 {
    let set_a: HashSet<String> = tokenize_stemmed(a, true).into_iter().collect();
    let set_b: HashSet<String> = tokenize_stemmed(b, true).into_iter().collect();
    match set_a.len() >= 4 && set_b.len() >= 4 {
        false => 0.0,
        true => {
            let inter = set_a.intersection(&set_b).count() as f32;
            let union = set_a.union(&set_b).count().max(1) as f32;
            inter / union
        }
    }
}

fn bullets_represent_same_slot(existing: &str, candidate: &str) -> bool {
    match (bullet_topic_prefix(existing), bullet_topic_prefix(candidate)) {
        (Some(a), Some(b)) => a == b,
        _ => stemmed_jaccard_similarity(existing, candidate) >= 0.78,
    }
}

fn upsert_or_dedupe_bullet(bullets: &mut Vec<String>, raw_text: &str) {
    let text = raw_text.trim();
    match text.len() > 3 {
        false => return,
        true => {}
    }

    let exact_or_substring = bullets.iter().any(|b| {
        b.eq_ignore_ascii_case(text) || b.contains(text) || text.contains(b.as_str())
    });
    match exact_or_substring {
        true => return,
        false => {}
    }

    let slot_idx = bullets
        .iter()
        .position(|existing| bullets_represent_same_slot(existing, text));
    match slot_idx {
        Some(idx) => {
            bullets[idx] = text.to_string();
        }
        None => {
            bullets.push(text.to_string());
        }
    }
}

fn apply_single_mutation(bullets: &mut Vec<String>, mutation: &MemoryBulletMutation) {
    let op = mutation.operation.trim().to_ascii_lowercase();
    let clean_new = clean_bullet_line(&mutation.text)
        .unwrap_or_else(|| mutation.text.trim())
        .to_string();
    let clean_old = mutation
        .old_text
        .as_deref()
        .map(|s| clean_bullet_line(s).unwrap_or_else(|| s.trim()).to_string())
        .filter(|s| !s.is_empty());

    match op.as_str() {
        "remove" | "delete" | "forget" => {
            let target = clean_old.as_deref().unwrap_or(clean_new.as_str());
            let target_lower = target.to_ascii_lowercase();
            match target_lower.is_empty() {
                true => {}
                false => {
                    bullets.retain(|b| !b.to_ascii_lowercase().contains(&target_lower));
                }
            }
        }
        "replace" | "update" => {
            match clean_old {
                Some(old_needle) => {
                    let old_lower = old_needle.to_ascii_lowercase();
                    let pos = bullets
                        .iter()
                        .position(|b| b.to_ascii_lowercase().contains(&old_lower));
                    match (pos, clean_new.len() > 3) {
                        (Some(idx), true) => {
                            bullets[idx] = clean_new;
                        }
                        (Some(idx), false) => {
                            bullets.remove(idx);
                        }
                        (None, true) => upsert_or_dedupe_bullet(bullets, &clean_new),
                        (None, false) => {}
                    }
                }
                None => upsert_or_dedupe_bullet(bullets, &clean_new),
            }
        }
        _ => upsert_or_dedupe_bullet(bullets, &clean_new),
    }
}

fn parse_inline_line_as_mutation(line: &str) -> MemoryBulletMutation {
    let raw = clean_bullet_line(line).unwrap_or_else(|| line.trim());
    let lower = raw.to_ascii_lowercase();
    match (
        lower.strip_prefix("remove:").map(|_| &raw[7..]),
        lower.strip_prefix("replace:").map(|_| &raw[8..]),
    ) {
        (Some(rem), _) => MemoryBulletMutation {
            operation: "remove".to_string(),
            old_text: Some(rem.trim().to_string()),
            text: String::new(),
        },
        (None, Some(rep)) => match rep.split_once("=>") {
            Some((old_part, new_part)) => MemoryBulletMutation {
                operation: "replace".to_string(),
                old_text: Some(old_part.trim().to_string()),
                text: new_part.trim().to_string(),
            },
            None => MemoryBulletMutation {
                operation: "add".to_string(),
                old_text: None,
                text: rep.trim().to_string(),
            },
        },
        (None, None) => MemoryBulletMutation {
            operation: "add".to_string(),
            old_text: None,
            text: raw.to_string(),
        },
    }
}

fn render_bounded_markdown_section(
    header: &str,
    bullets: &[String],
    max_chars: usize,
) -> (String, Vec<String>) {
    let mut kept_rev: Vec<String> = Vec::new();
    let mut evicted: Vec<String> = Vec::new();
    let mut used_chars = header.chars().count();

    for bullet in bullets.iter().rev() {
        let line_len = bullet.chars().count() + 3;
        match used_chars + line_len <= max_chars {
            true => {
                used_chars += line_len;
                kept_rev.push(bullet.clone());
            }
            false => {
                evicted.push(bullet.clone());
            }
        }
    }
    kept_rev.reverse();
    evicted.reverse();

    let mut out = String::from(header);
    for bullet in &kept_rev {
        out.push_str(&format!("- {}\n", bullet));
    }
    (out, evicted)
}

pub fn apply_user_profile_mutations(
    inline_preferences: &str,
    structured_ops: &[MemoryBulletMutation],
) -> Result<Vec<String>> {
    let has_inline = !inline_preferences.trim().is_empty();
    let has_ops = !structured_ops.is_empty();
    match has_inline || has_ops {
        false => return Ok(Vec::new()),
        true => {}
    }

    let lock = CORE_MEMORY_WRITE_LOCK.get_or_init(|| Mutex::new(()));
    let _guard = lock.lock().ok();

    let path = get_user_profile_path();
    let _ = path.parent().map(fs::create_dir_all);
    let existing = path
        .is_file()
        .then(|| fs::read_to_string(&path).ok())
        .flatten()
        .unwrap_or_default();
    let mut bullets = parse_existing_bullets(&existing);

    for line in inline_preferences.trim().lines() {
        let trimmed = line.trim();
        let _ = (!trimmed.is_empty()).then(|| {
            let mut_op = parse_inline_line_as_mutation(trimmed);
            apply_single_mutation(&mut bullets, &mut_op);
        });
    }
    for op in structured_ops {
        apply_single_mutation(&mut bullets, op);
    }

    let header = "# User Profile & Preferences\n\nDurable user preferences, styling tastes, persona rules, and coding conventions across sessions.\n\n## Preferences\n";
    let (rendered, evicted) =
        render_bounded_markdown_section(header, &bullets, MAX_USER_PROFILE_CHARS);
    fs::write(path, rendered)?;
    Ok(evicted)
}

pub fn update_user_profile(new_preferences: &str) -> Result<()> {
    let _ = apply_user_profile_mutations(new_preferences, &[])?;
    Ok(())
}

pub fn apply_global_memory_mutations(
    inline_facts: &str,
    structured_ops: &[MemoryBulletMutation],
) -> Result<Vec<String>> {
    let has_inline = !inline_facts.trim().is_empty();
    let has_ops = !structured_ops.is_empty();
    match has_inline || has_ops {
        false => return Ok(Vec::new()),
        true => {}
    }

    let lock = CORE_MEMORY_WRITE_LOCK.get_or_init(|| Mutex::new(()));
    let _guard = lock.lock().ok();

    let path = get_global_memory_path();
    let _ = path.parent().map(fs::create_dir_all);
    let existing = path
        .is_file()
        .then(|| fs::read_to_string(&path).ok())
        .flatten()
        .unwrap_or_default();
    let mut bullets = parse_existing_bullets(&existing);

    for line in inline_facts.trim().lines() {
        let trimmed = line.trim();
        let _ = (!trimmed.is_empty()).then(|| {
            let mut_op = parse_inline_line_as_mutation(trimmed);
            apply_single_mutation(&mut bullets, &mut_op);
        });
    }
    for op in structured_ops {
        apply_single_mutation(&mut bullets, op);
    }

    let header = "# Global Agent Memory\n\nCurated cross-session environment facts, architectural invariants, and durable operational learnings.\n\n## Facts\n";
    let (rendered, evicted) =
        render_bounded_markdown_section(header, &bullets, MAX_GLOBAL_MEMORY_CHARS);
    fs::write(path, rendered)?;
    Ok(evicted)
}

pub fn update_global_memory(new_facts: &str) -> Result<Vec<String>> {
    apply_global_memory_mutations(new_facts, &[])
}
