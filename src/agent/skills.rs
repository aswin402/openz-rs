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

