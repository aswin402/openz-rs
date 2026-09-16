use super::*;

#[tokio::test]
async fn websocket_cron_commands_update_inventory_and_logs() {
    let temp_dir = std::env::temp_dir().join(format!(
        "openz_ws_cron_commands_test_{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();

    crate::config::loader::CONFIG_DIR_OVERRIDE
        .scope(temp_dir.clone(), async {
            let job = crate::cron::CronJob::new(
                "job-a".to_string(),
                "10s".to_string(),
                "say hi".to_string(),
                true,
                false,
            );
            crate::cron::save_jobs_raw(&[job]).unwrap();
            let config = crate::config::schema::Config::default();

            let missing =
                cron_update_event("pause_cron_job", &serde_json::json!({}), &config, None)
                    .await;
            assert_eq!(missing["event"], "error");

            let paused = cron_update_event(
                "pause_cron_job",
                &serde_json::json!({ "id": "job-a" }),
                &config,
                None,
            )
            .await;
            assert_eq!(paused["event"], "cron_jobs_updated");
            assert_eq!(paused["status"], "paused");
            assert_eq!(paused["inventory"]["counts"]["cronJobs"], 1);
            assert_eq!(paused["inventory"]["counts"]["activeCronJobs"], 0);

            let resumed = cron_update_event(
                "resume_cron_job",
                &serde_json::json!({ "id": "job-a" }),
                &config,
                None,
            )
            .await;
            assert_eq!(resumed["event"], "cron_jobs_updated");
            assert_eq!(resumed["status"], "resumed");
            assert_eq!(resumed["inventory"]["counts"]["activeCronJobs"], 1);

            crate::cron::append_cron_run_record(&crate::cron::CronRunRecord {
                run_id: "run-a".to_string(),
                job_id: "job-a".to_string(),
                schedule: "10s".to_string(),
                started_at: "2026-08-21T00:00:00Z".to_string(),
                finished_at: Some("2026-08-21T00:00:01Z".to_string()),
                status: crate::cron::CronJobStatus::Success,
                log_path: Some("/tmp/job-a.log".to_string()),
                summary: Some("ok".to_string()),
                error: None,
            })
            .unwrap();
            let logs = cron_logs_event(&serde_json::json!({ "id": "job-a", "limit": 5 }));
            assert_eq!(logs["event"], "cron_logs");
            assert_eq!(logs["runs"].as_array().unwrap().len(), 1);
            assert_eq!(logs["runs"][0]["job_id"], "job-a");

            let deleted = cron_update_event(
                "delete_cron_job",
                &serde_json::json!({ "id": "job-a" }),
                &config,
                None,
            )
            .await;
            assert_eq!(deleted["event"], "cron_jobs_updated");
            assert_eq!(deleted["status"], "deleted");
            assert_eq!(deleted["inventory"]["counts"]["cronJobs"], 0);
        })
        .await;

    let _ = std::fs::remove_dir_all(temp_dir);
}
