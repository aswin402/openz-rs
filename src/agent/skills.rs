use anyhow::Result;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Skill {
    pub name: String,
    pub content: String,
}

pub fn get_skills_dir() -> PathBuf {
    crate::config::resolve_path("~/.openz/skills")
}

pub fn load_skills() -> Result<Vec<Skill>> {
    let mut skills_map = std::collections::HashMap::new();

    // 1. Load from global directory
    let global_dir = get_skills_dir();
    if global_dir.exists() {
        for entry in fs::read_dir(global_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("md") {
                let name = path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string();
                let content = fs::read_to_string(&path)?;
                skills_map.insert(name.clone(), Skill { name, content });
            }
        }
    }

    // 2. Load from local workspace directory (./skills)
    let local_dir = std::path::Path::new("skills");
    if local_dir.exists() && local_dir.is_dir() {
        for entry in fs::read_dir(local_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("md") {
                let name = path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string();
                let content = fs::read_to_string(&path)?;
                // Local skills override/shadow global skills of the same name
                skills_map.insert(name.clone(), Skill { name, content });
            }
        }
    }

    Ok(skills_map.into_values().collect())
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SkillStats {
    pub use_count: u32,
    pub last_used: String,
}

fn load_stats() -> std::collections::HashMap<String, SkillStats> {
    let path = get_skills_dir().join("stats.json");
    if path.exists() {
        if let Ok(content) = fs::read_to_string(path) {
            if let Ok(stats) = serde_json::from_str(&content) {
                return stats;
            }
        }
    }
    std::collections::HashMap::new()
}

fn save_stats(stats: &std::collections::HashMap<String, SkillStats>) {
    let path = get_skills_dir().join("stats.json");
    if let Ok(content) = serde_json::to_string_pretty(stats) {
        let _ = fs::write(path, content);
    }
}

pub fn archive_stale_skills() -> Result<()> {
    let skills_dir = get_skills_dir();
    if !skills_dir.exists() {
        return Ok(());
    }

    let archive_dir = skills_dir.join("archive");
    let stats = load_stats();
    let now = chrono::Utc::now();
    let stale_duration = chrono::Duration::days(30);

    for entry in fs::read_dir(&skills_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("md") {
            let name = path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            if name.is_empty() {
                continue;
            }

            let mut is_stale = false;
            if let Some(stat) = stats.get(name) {
                if let Ok(last_used_date) = chrono::DateTime::parse_from_rfc3339(&stat.last_used) {
                    let last_used_utc = last_used_date.with_timezone(&chrono::Utc);
                    if now.signed_duration_since(last_used_utc) > stale_duration {
                        is_stale = true;
                    }
                }
            } else {
                // If it's not in stats, check the file modification date
                if let Ok(metadata) = path.metadata() {
                    if let Ok(modified) = metadata.modified() {
                        let modified_utc: chrono::DateTime<chrono::Utc> = modified.into();
                        if now.signed_duration_since(modified_utc) > stale_duration {
                            is_stale = true;
                        }
                    }
                }
            }

            if is_stale {
                fs::create_dir_all(&archive_dir)?;
                let dest_path = archive_dir.join(format!("{}.md", name));
                let _ = fs::rename(&path, &dest_path);
                let bak_path = skills_dir.join(format!("{}.md.bak", name));
                if bak_path.exists() {
                    let _ = fs::rename(&bak_path, archive_dir.join(format!("{}.md.bak", name)));
                }
                
                // Print TUI notification using helper if possible, or just standard log
                let aura_blue = "\x1b[38;2;96;165;250m";
                let color_reset = "\x1b[0m";
                crate::channels::cli::send_notification(&format!(
                    "{}◇ [Self-Improvement] Skill '{}' archived due to 30 days of inactivity.{}",
                    aura_blue, name, color_reset
                ));
            }
        }
    }

    Ok(())
}

pub fn load_relevant_skills(user_content: &str, session_messages: &[crate::session::Message]) -> Result<Vec<Skill>> {
    let all_skills = load_skills()?;
    if all_skills.is_empty() {
        return Ok(Vec::new());
    }

    // Combine recent user prompt + last 3 messages into search context
    let mut search_context = user_content.to_lowercase();
    for msg in session_messages.iter().rev().take(3) {
        search_context.push_str(" ");
        search_context.push_str(&msg.content.to_lowercase());
    }

    let mut relevant = Vec::new();
    for skill in all_skills {
        // Simple and robust keyword match:
        // 1. Check if the skill name has any word matching in the query context
        let name_words: Vec<&str> = skill.name.split('_').collect();
        let name_match = name_words.iter().any(|word| {
            word.len() > 2 && search_context.contains(word)
        });

        // 2. Or if the query context contains the skill name directly
        let name_exact_match = search_context.contains(&skill.name.to_lowercase());

        if name_match || name_exact_match {
            relevant.push(skill);
        }
    }

    // Track usage metrics for successfully loaded relevant skills
    if !relevant.is_empty() {
        let mut stats = load_stats();
        let now = chrono::Utc::now().to_rfc3339();
        for skill in &relevant {
            let entry = stats.entry(skill.name.clone()).or_insert(SkillStats {
                use_count: 0,
                last_used: now.clone(),
            });
            entry.use_count += 1;
            entry.last_used = now.clone();
        }
        save_stats(&stats);
    }

    Ok(relevant)
}

pub fn save_skill(name: &str, content: &str) -> Result<()> {
    let local_dir = std::path::Path::new("skills");
    let dir = if local_dir.exists() && local_dir.is_dir() {
        local_dir.to_path_buf()
    } else {
        get_skills_dir()
    };
    fs::create_dir_all(&dir)?;

    // Normalize skill name to snake_case / lowercase with underscores/hyphens
    let safe_name = name.to_lowercase()
        .replace(|c: char| !c.is_alphanumeric() && c != '_' && c != '-', "_");

    let path = dir.join(format!("{}.md", safe_name));
    
    // Create backup if file already exists to prevent data corruption by LLM
    if path.exists() {
        let backup_path = dir.join(format!("{}.md.bak", safe_name));
        let _ = fs::copy(&path, &backup_path);
    }

    fs::write(path, content)?;
    Ok(())
}

pub fn delete_skill(name: &str) -> Result<()> {
    let safe_name = name.to_lowercase()
        .replace(|c: char| !c.is_alphanumeric() && c != '_' && c != '-', "_");
    let filename = format!("{}.md", safe_name);

    // Delete from global if exists
    let global_path = get_skills_dir().join(&filename);
    if global_path.exists() {
        fs::remove_file(global_path)?;
    }

    // Delete from local if exists
    let local_path = std::path::Path::new("skills").join(&filename);
    if local_path.exists() {
        fs::remove_file(local_path)?;
    }

    Ok(())
}

pub fn clear_skills() -> Result<()> {
    // Clear global directory
    let global_dir = get_skills_dir();
    if global_dir.exists() {
        fs::remove_dir_all(&global_dir)?;
        fs::create_dir_all(&global_dir)?;
    }

    // Clear local directory
    let local_dir = std::path::Path::new("skills");
    if local_dir.exists() && local_dir.is_dir() {
        fs::remove_dir_all(local_dir)?;
        fs::create_dir_all(local_dir)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_save_and_load_skills() {
        let skill_name = "test_temp_skill_12345";
        let skill_content = "# Test Content\n- Rule 1";
        
        let res = save_skill(skill_name, skill_content);
        assert!(res.is_ok());

        let skills = load_skills().expect("Failed to load skills");
        let found = skills.iter().find(|s| s.name == skill_name);
        assert!(found.is_some());
        assert_eq!(found.unwrap().content, skill_content);

        let del_res = delete_skill(skill_name);
        assert!(del_res.is_ok());

        let skills_after = load_skills().expect("Failed to load skills");
        let found_after = skills_after.iter().find(|s| s.name == skill_name);
        assert!(found_after.is_none());
    }
}

