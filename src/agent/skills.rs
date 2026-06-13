use anyhow::Result;
use std::fs;
use std::path::PathBuf;
use rusqlite::{Connection, params};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Skill {
    pub name: String,
    pub content: String,
}

pub fn get_skills_dir() -> PathBuf {
    crate::config::resolve_path("~/.openz/skills")
}

pub fn get_db_path() -> PathBuf {
    crate::config::resolve_path("~/.openz/memory.db")
}

pub fn get_connection() -> Result<Connection> {
    let path = get_db_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(path)?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS skills (
            name TEXT NOT NULL,
            content TEXT NOT NULL,
            profile TEXT,
            use_count INTEGER DEFAULT 0,
            last_used TEXT,
            created_at TEXT,
            PRIMARY KEY (name, profile)
        )",
        [],
    )?;
    let _ = migrate_old_skills_to_db(&conn);
    Ok(conn)
}

fn migrate_old_skills_to_db(conn: &Connection) -> Result<()> {
    let global_dir = get_skills_dir();
    if global_dir.exists() && global_dir.is_dir() {
        for entry in fs::read_dir(&global_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("md") {
                let name = path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string();
                if name.is_empty() || name == "INDEX" || name.ends_with(".bak") {
                    continue;
                }
                if let Ok(content) = fs::read_to_string(&path) {
                    let now = chrono::Utc::now().to_rfc3339();
                    let _ = conn.execute(
                        "INSERT OR IGNORE INTO skills (name, content, profile, created_at, last_used)
                         VALUES (?1, ?2, NULL, ?3, ?3)",
                        params![name, content, now],
                    );
                }
            }
        }
    }
    Ok(())
}

pub fn load_skills() -> Result<Vec<Skill>> {
    load_skills_with_profile(None)
}

pub fn load_skills_with_profile(profile_name: Option<&str>) -> Result<Vec<Skill>> {
    let mut skills_map = std::collections::HashMap::new();

    // 1. Load from SQLite database (global and profile-specific)
    if let Ok(conn) = get_connection() {
        let mut stmt = conn.prepare("SELECT name, content FROM skills WHERE profile IS NULL OR profile = ?")?;
        let profile_str = profile_name.unwrap_or("");
        let rows = stmt.query_map(params![profile_str], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
            ))
        })?;

        for r in rows.flatten() {
            let (name, content) = r;
            skills_map.insert(name.clone(), Skill { name, content });
        }
    }

    // 2. Load from local workspace directory (./skills) to support workspace overrides
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
                skills_map.insert(name.clone(), Skill { name, content });
            }
        }
    }

    Ok(skills_map.into_values().collect())
}

pub fn archive_stale_skills() -> Result<()> {
    if let Ok(conn) = get_connection() {
        let now = chrono::Utc::now();
        let stale_duration = chrono::Duration::days(30);

        let mut stmt = conn.prepare("SELECT name, profile, last_used, content FROM skills")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;

        let mut to_delete = Vec::new();
        for r in rows.flatten() {
            let (name, profile, last_used_opt, content) = r;
            if let Some(last_used_str) = last_used_opt {
                if let Ok(last_used_date) = chrono::DateTime::parse_from_rfc3339(&last_used_str) {
                    let last_used_utc = last_used_date.with_timezone(&chrono::Utc);
                    if now.signed_duration_since(last_used_utc) > stale_duration {
                        let archive_dir = get_skills_dir().join("archive");
                        fs::create_dir_all(&archive_dir)?;
                        let dest_path = archive_dir.join(format!("{}.md", name));
                        let _ = fs::write(dest_path, content);

                        to_delete.push((name, profile));
                    }
                }
            }
        }

        for (name, profile) in to_delete {
            let _ = conn.execute(
                "DELETE FROM skills WHERE name = ?1 AND profile IS ?2",
                params![name, profile],
            );
            
            let aura_blue = "\x1b[38;2;96;165;250m";
            let color_reset = "\x1b[0m";
            crate::channels::cli::send_notification(&format!(
                "{}◇ [Self-Improvement] Skill '{}' archived to filesystem due to 30 days of database inactivity.{}",
                aura_blue, name, color_reset
            ));
        }
    }
    Ok(())
}

pub fn load_relevant_skills(user_content: &str, session_messages: &[crate::session::Message]) -> Result<Vec<Skill>> {
    load_relevant_skills_with_profile(user_content, session_messages, None)
}

pub fn load_relevant_skills_with_profile(user_content: &str, session_messages: &[crate::session::Message], profile_name: Option<&str>) -> Result<Vec<Skill>> {
    let all_skills = load_skills_with_profile(profile_name)?;
    if all_skills.is_empty() {
        return Ok(Vec::new());
    }

    let mut search_context = user_content.to_lowercase();
    for msg in session_messages.iter().rev().take(3) {
        search_context.push_str(" ");
        search_context.push_str(&msg.content.to_lowercase());
    }

    let mut relevant = Vec::new();
    for skill in all_skills {
        let name_words: Vec<&str> = skill.name.split('_').collect();
        let name_match = name_words.iter().any(|word| {
            word.len() > 2 && search_context.contains(word)
        });

        let name_exact_match = search_context.contains(&skill.name.to_lowercase());

        if name_match || name_exact_match {
            relevant.push(skill);
        }
    }

    if !relevant.is_empty() {
        if let Ok(conn) = get_connection() {
            let now = chrono::Utc::now().to_rfc3339();
            for skill in &relevant {
                let _ = conn.execute(
                    "UPDATE skills SET use_count = use_count + 1, last_used = ?1 WHERE name = ?2",
                    params![now, skill.name],
                );
            }
        }
    }

    Ok(relevant)
}

pub fn save_skill(name: &str, content: &str) -> Result<()> {
    let safe_name = name.to_lowercase()
        .replace(|c: char| !c.is_alphanumeric() && c != '_' && c != '-', "_");

    let conn = get_connection()?;
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO skills (name, content, profile, created_at, last_used)
         VALUES (?1, ?2, NULL, ?3, ?3)
         ON CONFLICT(name, profile) DO UPDATE SET content = ?2, last_used = ?3",
        params![safe_name, content, now],
    )?;

    let local_dir = std::path::Path::new("skills");
    if local_dir.exists() && local_dir.is_dir() {
        let path = local_dir.join(format!("{}.md", safe_name));
        fs::write(path, content)?;
    }

    Ok(())
}

pub fn delete_skill(name: &str) -> Result<()> {
    let safe_name = name.to_lowercase()
        .replace(|c: char| !c.is_alphanumeric() && c != '_' && c != '-', "_");

    let conn = get_connection()?;
    conn.execute("DELETE FROM skills WHERE name = ?1", params![safe_name])?;

    let local_path = std::path::Path::new("skills").join(format!("{}.md", safe_name));
    if local_path.exists() {
        fs::remove_file(local_path)?;
    }
    Ok(())
}

pub fn clear_skills() -> Result<()> {
    let conn = get_connection()?;
    conn.execute("DELETE FROM skills", [])?;

    let local_dir = std::path::Path::new("skills");
    if local_dir.exists() && local_dir.is_dir() {
        fs::remove_dir_all(local_dir)?;
        fs::create_dir_all(local_dir)?;
    }

    Ok(())
}

pub fn save_subagent_skill(profile: &str, name: &str, content: &str) -> Result<()> {
    let safe_name = name.to_lowercase()
        .replace(|c: char| !c.is_alphanumeric() && c != '_' && c != '-', "_");

    let conn = get_connection()?;
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO skills (name, content, profile, created_at, last_used)
         VALUES (?1, ?2, ?3, ?4, ?4)
         ON CONFLICT(name, profile) DO UPDATE SET content = ?2, last_used = ?4",
        params![safe_name, content, profile, now],
    )?;
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
