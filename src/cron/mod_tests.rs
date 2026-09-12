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

#[test]
fn test_calculate_next_run() {
    let now = Utc::now();
    // Test duration
    let next = calculate_next_run("5m", Some(now));
    assert!(next.is_some());
    assert_eq!(next.unwrap(), now + chrono::Duration::minutes(5));

    // Test standard local-time cron (every minute)
    let next_cron = calculate_next_run("* * * * *", Some(now));
    assert!(next_cron.is_some());
    assert!(next_cron.unwrap() > now);

    // Test local wall-clock time accepted for one-shot style prompts.
    let next_clock = calculate_next_run("18:00", None);
    assert!(next_clock.is_some());
    let next_clock = next_clock.unwrap();
    assert!(next_clock > Utc::now());
    assert!(next_clock <= Utc::now() + chrono::Duration::days(1));
}

#[test]
fn cron_job_deserializes_run_once_default() {
    let job: CronJob = serde_json::from_value(serde_json::json!({
        "id": "legacy",
        "schedule": "5m",
        "prompt": "do work",
        "enabled": true,
        "last_run": null,
        "next_run": null
    }))
    .unwrap();
    assert!(!job.run_once);
}

#[test]
fn cron_job_deserializes_inventory_defaults() {
    let job: CronJob = serde_json::from_value(serde_json::json!({
        "id": "legacy",
        "schedule": "5m",
        "prompt": "do work",
        "enabled": true,
        "last_run": null,
        "next_run": null
    }))
    .unwrap();

    assert!(!job.run_once);
    assert_eq!(job.status, CronJobStatus::Idle);
    assert!(job.quiet);
    assert_eq!(job.notify_on, CronNotifyPolicy::Failure);
    assert_eq!(job.run_count, 0);
    assert_eq!(job.failure_count, 0);
    assert!(job.created_at.is_none());
    assert!(job.updated_at.is_none());
    assert!(job.last_started_at.is_none());
    assert!(job.last_finished_at.is_none());
    assert!(job.last_error.is_none());
    assert!(job.last_log_path.is_none());
}

#[tokio::test]
async fn cron_run_records_append_and_filter_by_job_id() {
    let temp_dir =
        std::env::temp_dir().join(format!("openz_cron_runs_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    crate::config::loader::CONFIG_DIR_OVERRIDE
        .scope(temp_dir.clone(), async {
            let first = CronRunRecord {
                run_id: "run-a".to_string(),
                job_id: "job-a".to_string(),
                schedule: "10s".to_string(),
                started_at: "2026-08-20T08:00:00Z".to_string(),
                finished_at: Some("2026-08-20T08:00:01Z".to_string()),
                status: CronJobStatus::Success,
                log_path: Some("/tmp/job-a.log".to_string()),
                summary: Some("ok".to_string()),
                error: None,
            };
            let second = CronRunRecord {
                run_id: "run-b".to_string(),
                job_id: "job-b".to_string(),
                schedule: "1m".to_string(),
                started_at: "2026-08-20T08:01:00Z".to_string(),
                finished_at: Some("2026-08-20T08:01:01Z".to_string()),
                status: CronJobStatus::Failed,
                log_path: Some("/tmp/job-b.log".to_string()),
                summary: None,
                error: Some("boom".to_string()),
            };

            append_cron_run_record(&first).unwrap();
            append_cron_run_record(&second).unwrap();

            let only_a = load_cron_run_records(Some("job-a"), 10).unwrap();
            assert_eq!(only_a.len(), 1);
            assert_eq!(only_a[0].run_id, "run-a");
            assert_eq!(only_a[0].status, CronJobStatus::Success);

            let all = load_cron_run_records(None, 10).unwrap();
            assert_eq!(all.len(), 2);
            assert_eq!(all[0].run_id, "run-a");
            assert_eq!(all[1].run_id, "run-b");
        })
        .await;

    let _ = std::fs::remove_dir_all(temp_dir);
}
