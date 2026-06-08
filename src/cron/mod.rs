pub mod scheduler;

use serde::{Serialize, Deserialize};
use std::fs;
use std::path::PathBuf;
use anyhow::{Result, Context};
use crate::config::resolve_path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJob {
    pub id: String,
    pub schedule: String, // e.g. "1m", "5m", "1h", "1d"
    pub prompt: String,
    pub enabled: bool,
    pub last_run: Option<String>,
    pub next_run: Option<String>,
}

pub fn cron_file_path() -> PathBuf {
    resolve_path("~/.openz/cron_jobs.json")
}

pub fn load_jobs() -> Result<Vec<CronJob>> {
    let path = cron_file_path();
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read cron jobs file at {:?}", path))?;
    let jobs: Vec<CronJob> = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse cron jobs file at {:?}", path))?;
    Ok(jobs)
}

pub fn save_jobs(jobs: &[CronJob]) -> Result<()> {
    let path = cron_file_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(jobs)?;
    fs::write(&path, content)
        .with_context(|| format!("Failed to write cron jobs to {:?}", path))?;
    Ok(())
}

pub fn parse_schedule(s: &str) -> Option<chrono::Duration> {
    let s = s.trim();
    if s.len() < 2 { return None; }
    let (num_str, unit) = s.split_at(s.len() - 1);
    let num: i64 = num_str.parse().ok()?;
    match unit {
        "s" => Some(chrono::Duration::seconds(num)),
        "m" => Some(chrono::Duration::minutes(num)),
        "h" => Some(chrono::Duration::hours(num)),
        "d" => Some(chrono::Duration::days(num)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_schedule() {
        assert_eq!(parse_schedule("10s"), Some(chrono::Duration::seconds(10)));
        assert_eq!(parse_schedule("5m"), Some(chrono::Duration::minutes(5)));
        assert_eq!(parse_schedule("2h"), Some(chrono::Duration::hours(2)));
        assert_eq!(parse_schedule("1d"), Some(chrono::Duration::days(1)));
        assert_eq!(parse_schedule("invalid"), None);
        assert_eq!(parse_schedule(""), None);
    }
}
