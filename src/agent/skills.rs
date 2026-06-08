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
    let dir = get_skills_dir();
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut skills = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("md") {
            let name = path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            let content = fs::read_to_string(&path)?;
            skills.push(Skill { name, content });
        }
    }
    Ok(skills)
}

pub fn save_skill(name: &str, content: &str) -> Result<()> {
    let dir = get_skills_dir();
    fs::create_dir_all(&dir)?;
    
    // Normalize skill name to snake_case / lowercase with underscores/hyphens
    let safe_name = name.to_lowercase()
        .replace(|c: char| !c.is_alphanumeric() && c != '_' && c != '-', "_");
    
    let path = dir.join(format!("{}.md", safe_name));
    fs::write(path, content)?;
    Ok(())
}

pub fn delete_skill(name: &str) -> Result<()> {
    let dir = get_skills_dir();
    let safe_name = name.to_lowercase()
        .replace(|c: char| !c.is_alphanumeric() && c != '_' && c != '-', "_");
    let path = dir.join(format!("{}.md", safe_name));
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

pub fn clear_skills() -> Result<()> {
    let dir = get_skills_dir();
    if dir.exists() {
        fs::remove_dir_all(&dir)?;
        fs::create_dir_all(&dir)?;
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

