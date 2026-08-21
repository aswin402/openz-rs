use crate::config::loader::config_dir;
use crate::config::schema::Config;
use crate::cron::{
    CronJob, CronJobStatus, CronNotifyPolicy, CronRunRecord, append_cron_run_record,
    calculate_next_run,
};
use anyhow::{Result, anyhow};
use chrono::Utc;
use std::time::Duration;
use tokio::time::sleep;

const CRON_RUNNING_LEASE_SECS: i64 = 60 * 60;
const STALE_RUNNING_ERROR: &str = "Cron job marked failed after stale running lease expired.";

pub fn start_scheduler(config: Config) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        tracing::info!("Cron scheduler background service started...");
        let mut shutdown_rx = match crate::shutdown::receiver() {
            Some(rx) => rx,
            None => {
                let (_, rx) = tokio::sync::watch::channel(false);
                rx
            }
        };

        loop {
            if *shutdown_rx.borrow() {
                break;
            }

            if let Err(e) = tick_scheduler(&config).await {
                crate::channels::cli::send_notification(&format!(
                    "Error in cron scheduler tick: {}",
                    e
                ));
            }

            tokio::select! {
                biased;
                _ = shutdown_rx.changed() => {
                    break;
                }
                _ = sleep(Duration::from_secs(10)) => {}
            }
        }
    })
}

fn running_state_is_stale(job: &CronJob, now: chrono::DateTime<Utc>) -> bool {
    job.last_started_at
        .as_deref()
        .and_then(|dt| dt.parse::<chrono::DateTime<Utc>>().ok())
        .map(|started_at| {
            now.signed_duration_since(started_at).num_seconds() > CRON_RUNNING_LEASE_SECS
        })
        .unwrap_or(true)
}

fn mark_stale_running_failure(job: &mut CronJob, now: chrono::DateTime<Utc>) {
    job.status = CronJobStatus::Failed;
    job.last_run = Some(now.to_rfc3339());
    job.last_finished_at = Some(now.to_rfc3339());
    job.updated_at = Some(now.to_rfc3339());
    job.run_count = job.run_count.saturating_add(1);
    job.failure_count = job.failure_count.saturating_add(1);
    job.last_error = Some(STALE_RUNNING_ERROR.to_string());
    job.last_log_path = None;
}

fn mark_job_running(job: &mut CronJob, started_at: chrono::DateTime<Utc>) {
    job.status = CronJobStatus::Running;
    job.last_started_at = Some(started_at.to_rfc3339());
    job.updated_at = Some(started_at.to_rfc3339());
    job.last_error = None;
}

fn should_notify_cron(quiet: bool, notify_on: &CronNotifyPolicy, status: &CronJobStatus) -> bool {
    if quiet {
        return false;
    }
    match notify_on {
        CronNotifyPolicy::Never => false,
        CronNotifyPolicy::Failure => matches!(status, CronJobStatus::Failed),
        CronNotifyPolicy::Always => true,
    }
}

fn failed_run_record(
    job: &CronJob,
    started_at: chrono::DateTime<Utc>,
    error: &anyhow::Error,
) -> CronRunRecord {
    let finished_at = Utc::now();
    CronRunRecord {
        run_id: format!("{}_{}", job.id, uuid::Uuid::new_v4()),
        job_id: job.id.clone(),
        schedule: job.schedule.clone(),
        started_at: started_at.to_rfc3339(),
        finished_at: Some(finished_at.to_rfc3339()),
        status: CronJobStatus::Failed,
        log_path: None,
        summary: None,
        error: Some(error.to_string()),
    }
}

fn update_job_success(
    job_id: &str,
    completed_at: chrono::DateTime<Utc>,
    run_record: &CronRunRecord,
) -> Result<()> {
    crate::cron::with_cron_jobs_mut(|jobs| {
        if let Some(j) = jobs.iter_mut().find(|j| j.id == job_id) {
            j.status = if j.enabled {
                CronJobStatus::Idle
            } else {
                CronJobStatus::Disabled
            };
            j.last_run = Some(completed_at.to_rfc3339());
            j.last_finished_at = Some(completed_at.to_rfc3339());
            j.updated_at = Some(completed_at.to_rfc3339());
            j.run_count = j.run_count.saturating_add(1);
            j.last_error = None;
            j.last_log_path = run_record.log_path.clone();
        }
    })?;
    Ok(())
}

fn update_job_failure(
    job_id: &str,
    completed_at: chrono::DateTime<Utc>,
    error: &anyhow::Error,
) -> Result<()> {
    crate::cron::with_cron_jobs_mut(|jobs| {
        if let Some(j) = jobs.iter_mut().find(|j| j.id == job_id) {
            j.status = CronJobStatus::Failed;
            j.last_run = Some(completed_at.to_rfc3339());
            j.last_finished_at = Some(completed_at.to_rfc3339());
            j.updated_at = Some(completed_at.to_rfc3339());
            j.run_count = j.run_count.saturating_add(1);
            j.failure_count = j.failure_count.saturating_add(1);
            j.last_error = Some(error.to_string());
            j.last_log_path = None;
        }
    })?;
    Ok(())
}

pub async fn run_single_job_now(config: &Config, job: CronJob) -> Result<CronRunRecord> {
    let started_at = Utc::now();
    crate::cron::with_cron_jobs_mut(|jobs| -> Result<()> {
        let j = jobs
            .iter_mut()
            .find(|j| j.id == job.id)
            .ok_or_else(|| anyhow!("Cron job with ID '{}' not found.", job.id))?;
        if matches!(j.status, CronJobStatus::Running) {
            if !running_state_is_stale(j, started_at) {
                return Err(anyhow!("Cron job with ID '{}' is already running.", job.id));
            }
            mark_stale_running_failure(j, started_at);
        }
        mark_job_running(j, started_at);
        Ok(())
    })??;

    match run_job(config, &job, started_at).await {
        Ok(record) => {
            let completed_at = record
                .finished_at
                .as_deref()
                .and_then(|dt| dt.parse::<chrono::DateTime<Utc>>().ok())
                .unwrap_or_else(Utc::now);
            if let Err(e) = append_cron_run_record(&record) {
                tracing::error!(job_id = %job.id, error = ?e, "failed to append manual cron run record");
                update_job_failure(&job.id, completed_at, &e)?;
                return Err(e);
            }
            update_job_success(&job.id, completed_at, &record)?;
            Ok(record)
        }
        Err(e) => {
            let run_record = failed_run_record(&job, started_at, &e);
            let _ = append_cron_run_record(&run_record);
            let completed_at = run_record
                .finished_at
                .as_deref()
                .and_then(|dt| dt.parse::<chrono::DateTime<Utc>>().ok())
                .unwrap_or_else(Utc::now);
            update_job_failure(&job.id, completed_at, &e)?;
            Err(e)
        }
    }
}

async fn tick_scheduler(config: &Config) -> Result<()> {
    let now = Utc::now();
    let mut jobs_to_run = Vec::new();

    crate::cron::with_cron_jobs_mut(|jobs| {
        for job in jobs.iter_mut() {
            if matches!(job.status, CronJobStatus::Running) {
                if !running_state_is_stale(job, now) {
                    continue;
                }
                mark_stale_running_failure(job, now);
            }
            if !job.enabled {
                continue;
            }

            let next_run = match &job.next_run {
                Some(dt_str) => match dt_str.parse::<chrono::DateTime<Utc>>() {
                    Ok(dt) => dt,
                    Err(_) => {
                        let next = calculate_next_run(&job.schedule, None)
                            .unwrap_or_else(|| now + chrono::Duration::minutes(5));
                        job.next_run = Some(next.to_rfc3339());
                        next
                    }
                },
                None => {
                    let next = calculate_next_run(&job.schedule, None)
                        .unwrap_or_else(|| now + chrono::Duration::minutes(5));
                    job.next_run = Some(next.to_rfc3339());
                    next
                }
            };

            if now >= next_run {
                let started_at = Utc::now();
                let next_next = calculate_next_run(&job.schedule, Some(now))
                    .unwrap_or_else(|| now + chrono::Duration::minutes(5));
                if job.run_once {
                    job.enabled = false;
                    job.next_run = None;
                } else {
                    job.next_run = Some(next_next.to_rfc3339());
                }
                mark_job_running(job, started_at);
                jobs_to_run.push(job.clone());
            }
        }
    })?;

    for job_clone in jobs_to_run {
        let config_clone = config.clone();
        tokio::spawn(async move {
            tracing::info!(
                job_id = %job_clone.id,
                schedule = %job_clone.schedule,
                "cron job started"
            );
            let started_at = job_clone
                .last_started_at
                .as_deref()
                .and_then(|dt| dt.parse::<chrono::DateTime<Utc>>().ok())
                .unwrap_or_else(Utc::now);
            match run_job(&config_clone, &job_clone, started_at).await {
                Ok(run_record) => {
                    let completed_at = run_record
                        .finished_at
                        .as_deref()
                        .and_then(|dt| dt.parse::<chrono::DateTime<Utc>>().ok())
                        .unwrap_or_else(Utc::now);
                    match append_cron_run_record(&run_record) {
                        Ok(()) => {
                            if let Err(e) =
                                update_job_success(&job_clone.id, completed_at, &run_record)
                            {
                                tracing::error!("Failed to update cron jobs metadata: {:?}", e);
                            }
                            tracing::info!(job_id = %job_clone.id, "cron job completed successfully");
                            if should_notify_cron(
                                job_clone.quiet,
                                &job_clone.notify_on,
                                &CronJobStatus::Success,
                            ) {
                                crate::channels::cli::send_notification(&format!(
                                    "Cron Job {} completed successfully.",
                                    job_clone.id
                                ));
                            }
                        }
                        Err(e) => {
                            tracing::error!(job_id = %job_clone.id, error = ?e, "failed to append cron run record");
                            if let Err(err) = update_job_failure(&job_clone.id, completed_at, &e) {
                                tracing::error!(
                                    "Failed to update cron jobs metadata after run-record append failure: {:?}",
                                    err
                                );
                            }
                            if should_notify_cron(
                                job_clone.quiet,
                                &job_clone.notify_on,
                                &CronJobStatus::Failed,
                            ) {
                                crate::channels::cli::send_notification(&format!(
                                    "Error running Cron Job {}: {}",
                                    job_clone.id, e
                                ));
                            }
                        }
                    }
                }
                Err(e) => {
                    let run_record = failed_run_record(&job_clone, started_at, &e);
                    let append_err = append_cron_run_record(&run_record).err();
                    if let Some(err) = &append_err {
                        tracing::error!(job_id = %job_clone.id, error = ?err, "failed to append failed cron run record");
                    }
                    let completed_at = run_record
                        .finished_at
                        .as_deref()
                        .and_then(|dt| dt.parse::<chrono::DateTime<Utc>>().ok())
                        .unwrap_or_else(Utc::now);
                    if let Err(err) = update_job_failure(&job_clone.id, completed_at, &e) {
                        tracing::error!(
                            "Failed to update cron jobs metadata after failure: {:?}",
                            err
                        );
                    }
                    tracing::error!(job_id = %job_clone.id, error = ?e, "cron job failed");
                    if should_notify_cron(
                        job_clone.quiet,
                        &job_clone.notify_on,
                        &CronJobStatus::Failed,
                    ) {
                        crate::channels::cli::send_notification(&format!(
                            "Error running Cron Job {}: {}",
                            job_clone.id, e
                        ));
                    }
                }
            }
        });
    }

    Ok(())
}

async fn run_job(
    config: &Config,
    job: &CronJob,
    started_at: chrono::DateTime<Utc>,
) -> Result<CronRunRecord> {
    let agent_loop = crate::cli::build_agent_loop(config.clone()).await?;

    let session_key = format!("cron:{}", job.id);
    let prompt = format!(
        "[CRON JOB MODE] This task is running on an automated schedule.

Task: {}",
        job.prompt
    );

    let res = agent_loop.run(&prompt, &session_key).await?;

    let logs_dir = config_dir().join("cron_logs");
    if !logs_dir.exists() {
        std::fs::create_dir_all(&logs_dir)?;
    }
    let timestamp = Utc::now().format("%Y%m%d_%H%M%S%.3f").to_string();
    let run_id = format!("{}_{}", job.id, uuid::Uuid::new_v4());
    let log_file = logs_dir.join(format!("job_{}_{}_{}.log", job.id, timestamp, run_id));
    let finished_at = Utc::now();

    let log_content = format!(
        "Cron Job ID: {}
Schedule: {}
Executed At: {}

=== Prompt ===
{}

=== Output ===
{}
",
        job.id,
        job.schedule,
        finished_at.to_rfc3339(),
        job.prompt,
        res.content
    );
    std::fs::write(&log_file, log_content)?;
    tracing::info!(job_id = %job.id, log_path = %log_file.display(), "cron job log saved");

    Ok(CronRunRecord {
        run_id,
        job_id: job.id.clone(),
        schedule: job.schedule.clone(),
        started_at: started_at.to_rfc3339(),
        finished_at: Some(finished_at.to_rfc3339()),
        status: CronJobStatus::Success,
        log_path: Some(log_file.to_string_lossy().to_string()),
        summary: Some(res.content.chars().take(500).collect()),
        error: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::loader::CONFIG_DIR_OVERRIDE;
    use crate::cron::{CronJob, CronJobStatus, CronNotifyPolicy, load_jobs_raw, save_jobs_raw};
    use chrono::Utc;

    #[test]
    fn cron_should_notify_policy_matches_quiet_defaults() {
        assert!(!should_notify_cron(
            false,
            &CronNotifyPolicy::Never,
            &CronJobStatus::Success
        ));
        assert!(!should_notify_cron(
            true,
            &CronNotifyPolicy::Always,
            &CronJobStatus::Success
        ));
        assert!(should_notify_cron(
            false,
            &CronNotifyPolicy::Always,
            &CronJobStatus::Success
        ));
        assert!(should_notify_cron(
            false,
            &CronNotifyPolicy::Failure,
            &CronJobStatus::Failed
        ));
        assert!(!should_notify_cron(
            false,
            &CronNotifyPolicy::Failure,
            &CronJobStatus::Success
        ));
    }

    #[tokio::test]
    async fn stale_running_job_is_recovered_before_next_due_run() {
        let temp_dir = std::env::temp_dir().join(format!(
            "openz_cron_stale_running_test_{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let config_dir = temp_dir.clone();

        let config = Config::default();
        let now = Utc::now();
        let stale_started = now - chrono::Duration::seconds(CRON_RUNNING_LEASE_SECS + 1);
        let fresh_started = now - chrono::Duration::seconds(5);

        let jobs = vec![
            {
                let mut job = CronJob::new(
                    "stale".to_string(),
                    "5m".to_string(),
                    "stale prompt".to_string(),
                    true,
                    false,
                );
                job.status = CronJobStatus::Running;
                job.last_started_at = Some(stale_started.to_rfc3339());
                job.next_run = Some((now - chrono::Duration::seconds(10)).to_rfc3339());
                job
            },
            {
                let mut job = CronJob::new(
                    "fresh".to_string(),
                    "5m".to_string(),
                    "fresh prompt".to_string(),
                    true,
                    false,
                );
                job.status = CronJobStatus::Running;
                job.last_started_at = Some(fresh_started.to_rfc3339());
                job.next_run = Some((now - chrono::Duration::seconds(10)).to_rfc3339());
                job
            },
        ];

        CONFIG_DIR_OVERRIDE
            .scope(config_dir, async move {
                save_jobs_raw(&jobs).unwrap();
                tick_scheduler(&config).await.unwrap();

                let updated_jobs = load_jobs_raw().unwrap();
                let stale = updated_jobs.iter().find(|j| j.id == "stale").unwrap();
                assert_eq!(stale.status, CronJobStatus::Running);
                assert_eq!(stale.failure_count, 1);
                assert_ne!(stale.last_started_at, Some(stale_started.to_rfc3339()));
                let fresh = updated_jobs.iter().find(|j| j.id == "fresh").unwrap();
                assert_eq!(fresh.status, CronJobStatus::Running);
                assert_eq!(fresh.failure_count, 0);
                assert_eq!(fresh.last_started_at, Some(fresh_started.to_rfc3339()));
            })
            .await;

        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[tokio::test]
    async fn stale_disabled_run_once_job_is_marked_failed_without_rerun() {
        let temp_dir = std::env::temp_dir().join(format!(
            "openz_cron_stale_run_once_test_{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let config_dir = temp_dir.clone();

        let config = Config::default();
        let now = Utc::now();
        let stale_started = now - chrono::Duration::seconds(CRON_RUNNING_LEASE_SECS + 1);

        let jobs = vec![{
            let mut job = CronJob::new(
                "stale_once".to_string(),
                "5m".to_string(),
                "stale once prompt".to_string(),
                false,
                true,
            );
            job.status = CronJobStatus::Running;
            job.last_started_at = Some(stale_started.to_rfc3339());
            job.next_run = None;
            job
        }];

        CONFIG_DIR_OVERRIDE
            .scope(config_dir, async move {
                save_jobs_raw(&jobs).unwrap();
                tick_scheduler(&config).await.unwrap();

                let updated_jobs = load_jobs_raw().unwrap();
                let job = updated_jobs.iter().find(|j| j.id == "stale_once").unwrap();
                assert!(!job.enabled);
                assert_eq!(job.status, CronJobStatus::Failed);
                assert_eq!(job.failure_count, 1);
                assert_eq!(job.last_error.as_deref(), Some(STALE_RUNNING_ERROR));
            })
            .await;

        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[tokio::test]
    async fn test_tick_scheduler_updates_disk_immediately() {
        let temp_dir =
            std::env::temp_dir().join(format!("openz_cron_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let config_dir = temp_dir.clone();

        let config = Config::default();
        let now = Utc::now();

        // Define a run_once job and a recurring job
        let jobs = vec![
            {
                let mut job = CronJob::new(
                    "test_run_once".to_string(),
                    "18:00".to_string(),
                    "run once prompt".to_string(),
                    true,
                    true,
                );
                job.next_run = Some((now - chrono::Duration::seconds(10)).to_rfc3339());
                job
            },
            {
                let mut job = CronJob::new(
                    "test_recurring".to_string(),
                    "5m".to_string(),
                    "recurring prompt".to_string(),
                    true,
                    false,
                );
                job.next_run = Some((now - chrono::Duration::seconds(10)).to_rfc3339());
                job
            },
        ];

        // Run inside CONFIG_DIR_OVERRIDE scope
        CONFIG_DIR_OVERRIDE
            .scope(config_dir, async move {
                // Save the initial jobs
                save_jobs_raw(&jobs).unwrap();

                // Run one tick of the scheduler
                tick_scheduler(&config).await.unwrap();

                // Load the jobs back from disk
                let updated_jobs = load_jobs_raw().unwrap();

                // Check that test_run_once has been disabled and next_run is None
                let j_once = updated_jobs
                    .iter()
                    .find(|j| j.id == "test_run_once")
                    .unwrap();
                assert!(!j_once.enabled);
                assert!(j_once.next_run.is_none());
                assert_eq!(j_once.status, CronJobStatus::Running);
                assert!(j_once.last_started_at.is_some());

                // Check that test_recurring next_run is updated to future (around now + 5 minutes)
                let j_rec = updated_jobs
                    .iter()
                    .find(|j| j.id == "test_recurring")
                    .unwrap();
                assert!(j_rec.enabled);
                assert_eq!(j_rec.status, CronJobStatus::Running);
                assert!(j_rec.last_started_at.is_some());
                let next_dt = j_rec
                    .next_run
                    .as_ref()
                    .unwrap()
                    .parse::<chrono::DateTime<Utc>>()
                    .unwrap();
                assert!(next_dt > now);
                assert!(next_dt <= now + chrono::Duration::minutes(6));
            })
            .await;

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
