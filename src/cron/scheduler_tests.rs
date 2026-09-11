use super::*;
use crate::config::loader::CONFIG_DIR_OVERRIDE;
use crate::cron::{load_jobs_raw, save_jobs_raw, CronJob, CronJobStatus, CronNotifyPolicy};
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
