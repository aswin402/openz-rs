pub mod scheduler;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CronJobStatus {
    Idle,
    Running,
    Success,
    Failed,
    Disabled,
}

fn default_cron_job_status() -> CronJobStatus {
    CronJobStatus::Idle
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CronNotifyPolicy {
    Never,
    Failure,
    Always,
}

fn default_cron_notify_policy() -> CronNotifyPolicy {
    CronNotifyPolicy::Failure
}

fn default_quiet() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJob {
    pub id: String,
    pub schedule: String, // e.g. "1m", "5m", "1h", "1d"
    pub prompt: String,
    pub enabled: bool,
    #[serde(default)]
    pub run_once: bool,
    pub last_run: Option<String>,
    pub next_run: Option<String>,
    #[serde(default = "default_cron_job_status")]
    pub status: CronJobStatus,
    #[serde(default = "default_quiet")]
    pub quiet: bool,
    #[serde(default = "default_cron_notify_policy")]
    pub notify_on: CronNotifyPolicy,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub last_started_at: Option<String>,
    #[serde(default)]
    pub last_finished_at: Option<String>,
    #[serde(default)]
    pub last_error: Option<String>,
    #[serde(default)]
    pub last_log_path: Option<String>,
    #[serde(default)]
    pub run_count: u64,
    #[serde(default)]
    pub failure_count: u64,
}

impl CronJob {
    pub fn new(
        id: String,
        schedule: String,
        prompt: String,
        enabled: bool,
        run_once: bool,
    ) -> Self {
        Self {
            id,
            schedule,
            prompt,
            enabled,
            run_once,
            last_run: None,
            next_run: None,
            status: CronJobStatus::Idle,
            quiet: true,
            notify_on: CronNotifyPolicy::Failure,
            created_at: None,
            updated_at: None,
            last_started_at: None,
            last_finished_at: None,
            last_error: None,
            last_log_path: None,
            run_count: 0,
            failure_count: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CronRunRecord {
    pub run_id: String,
    pub job_id: String,
    pub schedule: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub status: CronJobStatus,
    pub log_path: Option<String>,
    pub summary: Option<String>,
    pub error: Option<String>,
}

pub fn cron_file_path() -> PathBuf {
    crate::config::loader::config_dir().join("cron_jobs.json")
}

pub fn cron_runs_file_path() -> PathBuf {
    crate::config::loader::config_dir().join("cron_runs.jsonl")
}

pub fn append_cron_run_record(record: &CronRunRecord) -> Result<()> {
    let path = cron_runs_file_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut line = serde_json::to_string(record)?;
    line.push('\n');
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("Failed to open cron run log at {:?}", path))?;
    file.write_all(line.as_bytes())?;
    Ok(())
}

pub fn load_cron_run_records(job_id: Option<&str>, limit: usize) -> Result<Vec<CronRunRecord>> {
    let path = cron_runs_file_path();
    if !path.exists() || limit == 0 {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read cron run log at {:?}", path))?;
    let mut records = Vec::new();
    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let record: CronRunRecord = serde_json::from_str(line)
            .with_context(|| format!("Failed to parse cron run record from {:?}", path))?;
        if job_id.map(|wanted| wanted == record.job_id).unwrap_or(true) {
            records.push(record);
        }
    }

    if records.len() > limit {
        Ok(records.split_off(records.len() - limit))
    } else {
        Ok(records)
    }
}

pub struct FileLock {
    lock_path: PathBuf,
}

impl FileLock {
    pub fn acquire(lock_path: PathBuf) -> Self {
        loop {
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&lock_path)
            {
                Ok(_) => {
                    return FileLock { lock_path };
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    if let Ok(metadata) = std::fs::metadata(&lock_path) {
                        if let Ok(modified) = metadata.modified() {
                            if let Ok(elapsed) = modified.elapsed() {
                                if elapsed.as_secs() > 10 {
                                    let _ = std::fs::remove_file(&lock_path);
                                    continue;
                                }
                            }
                        }
                    }
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                Err(_) => {
                    if let Some(parent) = lock_path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
            }
        }
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.lock_path);
    }
}

pub fn acquire_cron_lock() -> FileLock {
    let lock_path = crate::config::loader::config_dir().join("cron_jobs.lock");
    FileLock::acquire(lock_path)
}

pub fn load_jobs_raw() -> Result<Vec<CronJob>> {
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

pub fn save_jobs_raw(jobs: &[CronJob]) -> Result<()> {
    let path = cron_file_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(jobs)?;
    fs::write(&path, content)
        .with_context(|| format!("Failed to write cron jobs to {:?}", path))?;
    Ok(())
}

pub fn load_jobs() -> Result<Vec<CronJob>> {
    let _lock = acquire_cron_lock();
    load_jobs_raw()
}

pub fn save_jobs(jobs: &[CronJob]) -> Result<()> {
    let _lock = acquire_cron_lock();
    save_jobs_raw(jobs)
}

pub fn with_cron_jobs_mut<F, R>(f: F) -> Result<R>
where
    F: FnOnce(&mut Vec<CronJob>) -> R,
{
    let _lock = acquire_cron_lock();
    let mut jobs = load_jobs_raw()?;
    let res = f(&mut jobs);
    save_jobs_raw(&jobs)?;
    Ok(res)
}

pub fn parse_schedule(s: &str) -> Option<chrono::Duration> {
    let s = s.trim();
    if s.len() < 2 {
        return None;
    }
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

use chrono::{Local, TimeZone, Utc};
use std::str::FromStr;

pub fn calculate_next_run(
    s: &str,
    last_run: Option<chrono::DateTime<Utc>>,
) -> Option<chrono::DateTime<Utc>> {
    let now = Utc::now();
    let base_time = last_run.unwrap_or(now);

    // 1. Try simple duration parsing
    if let Some(duration) = parse_schedule(s) {
        return Some(base_time + duration);
    }

    let s_clean = s.trim();

    // 2. Try a plain local clock time, e.g. "18:00" or "18:00:30".
    // Users read wall-clock times in the TUI, so store the next local occurrence as UTC.
    if let Some(next_local_time) = next_local_clock_time(s_clean) {
        return Some(next_local_time);
    }

    // 3. Try standard Unix cron parsing (5-field or 6-field) in local time.
    let cron_str = if s_clean.split_whitespace().count() == 5 {
        format!("0 {}", s_clean)
    } else {
        s_clean.to_string()
    };

    if let Ok(schedule) = cron::Schedule::from_str(&cron_str) {
        return schedule
            .upcoming(Local)
            .next()
            .map(|dt| dt.with_timezone(&Utc));
    }

    None
}

fn next_local_clock_time(s: &str) -> Option<chrono::DateTime<Utc>> {
    let time = chrono::NaiveTime::parse_from_str(s, "%H:%M")
        .or_else(|_| chrono::NaiveTime::parse_from_str(s, "%H:%M:%S"))
        .ok()?;
    let now = Local::now();
    let today = now.date_naive().and_time(time);
    let mut candidate = Local.from_local_datetime(&today).earliest()?;
    if candidate <= now {
        let tomorrow = now.date_naive().succ_opt()?.and_time(time);
        candidate = Local.from_local_datetime(&tomorrow).earliest()?;
    }
    Some(candidate.with_timezone(&Utc))
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;

