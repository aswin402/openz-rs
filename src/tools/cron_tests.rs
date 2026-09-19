use super::*;
use crate::config::loader::CONFIG_DIR_OVERRIDE;
use crate::cron::{
    load_jobs_raw, save_jobs_raw, CronJob, CronJobStatus, CronNotifyPolicy, CronRunRecord,
};

fn sample_job(id: &str) -> CronJob {
    CronJob {
        id: id.to_string(),
        schedule: "10s".to_string(),
        prompt: "say hi".to_string(),
        enabled: true,
        run_once: false,
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

#[tokio::test]
async fn pause_and_resume_job_toggle_enabled_status() {
    let temp_dir =
        std::env::temp_dir().join(format!("openz_cron_tools_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    CONFIG_DIR_OVERRIDE
        .scope(temp_dir.clone(), async {
            save_jobs_raw(&[sample_job("daily")]).unwrap();

            let pause = PauseJobTool;
            let paused = pause
                .call(&serde_json::json!({ "id": "daily" }))
                .await
                .unwrap();
            assert_eq!(paused["status"], "success");
            let jobs = load_jobs_raw().unwrap();
            assert!(!jobs[0].enabled);
            assert_eq!(jobs[0].status, CronJobStatus::Disabled);

            let resume = ResumeJobTool;
            let resumed = resume
                .call(&serde_json::json!({ "id": "daily" }))
                .await
                .unwrap();
            assert_eq!(resumed["status"], "success");
            let jobs = load_jobs_raw().unwrap();
            assert!(jobs[0].enabled);
            assert_eq!(jobs[0].status, CronJobStatus::Idle);
            assert!(jobs[0].next_run.is_none());
        })
        .await;

    let _ = std::fs::remove_dir_all(temp_dir);
}

#[tokio::test]
async fn run_job_now_rejects_unknown_job() {
    let temp_dir =
        std::env::temp_dir().join(format!("openz_cron_run_now_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    CONFIG_DIR_OVERRIDE
        .scope(temp_dir.clone(), async {
            save_jobs_raw(&[sample_job("daily")]).unwrap();
            let tool = RunJobNowTool;
            let err = tool
                .call(&serde_json::json!({ "id": "missing" }))
                .await
                .unwrap_err()
                .to_string();
            assert!(err.contains("Cron job with ID 'missing' not found"));
        })
        .await;

    let _ = std::fs::remove_dir_all(temp_dir);
}

#[tokio::test]
async fn run_job_now_rejects_running_job_without_provider_call() {
    let temp_dir = std::env::temp_dir().join(format!(
        "openz_cron_running_now_test_{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();

    CONFIG_DIR_OVERRIDE
        .scope(temp_dir.clone(), async {
            let mut job = sample_job("daily");
            job.status = CronJobStatus::Running;
            job.last_started_at = Some(chrono::Utc::now().to_rfc3339());
            save_jobs_raw(&[job]).unwrap();

            let tool = RunJobNowTool;
            let err = tool
                .call(&serde_json::json!({ "id": "daily" }))
                .await
                .unwrap_err()
                .to_string();
            assert!(err.contains("Cron job with ID 'daily' is already running"));
        })
        .await;

    let _ = std::fs::remove_dir_all(temp_dir);
}

#[tokio::test]
async fn get_job_logs_returns_structured_runs() {
    let temp_dir = std::env::temp_dir().join(format!(
        "openz_cron_logs_tool_test_{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();

    CONFIG_DIR_OVERRIDE
        .scope(temp_dir.clone(), async {
            crate::cron::append_cron_run_record(&CronRunRecord {
                run_id: "run-1".to_string(),
                job_id: "daily".to_string(),
                schedule: "10s".to_string(),
                started_at: "2026-08-20T08:00:00Z".to_string(),
                finished_at: Some("2026-08-20T08:00:01Z".to_string()),
                status: CronJobStatus::Success,
                log_path: Some("/tmp/daily.log".to_string()),
                summary: Some("done".to_string()),
                error: None,
            })
            .unwrap();

            let tool = GetJobLogsTool;
            let res = tool
                .call(&serde_json::json!({ "id": "daily", "limit": 5 }))
                .await
                .unwrap();
            assert_eq!(res["status"], "success");
            assert_eq!(res["runs"].as_array().unwrap().len(), 1);
            assert_eq!(res["runs"][0]["run_id"], "run-1");

            // Direct string get_job_logs
            let res = tool.call(&serde_json::json!("daily")).await.unwrap();
            assert_eq!(res["status"], "success");
            assert_eq!(res["runs"].as_array().unwrap().len(), 1);
        })
        .await;

    let _ = std::fs::remove_dir_all(temp_dir);
}

#[tokio::test]
async fn test_cron_tools_direct_strings_and_aliases() {
    let temp_dir = std::env::temp_dir().join(format!(
        "openz_cron_aliases_test_{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();

    CONFIG_DIR_OVERRIDE
        .scope(temp_dir.clone(), async {
            // 1. Schedule via aliases: job_id, cron, goal, runOnce
            let scheduler = ScheduleJobTool;
            let res = scheduler
                .call(&serde_json::json!({
                    "job_id": "backup_job",
                    "cron": "10m",
                    "goal": "run backup now",
                    "runOnce": "true"
                }))
                .await
                .unwrap();
            assert_eq!(res["status"], "success");
            assert_eq!(res["run_once"], true);

            // 2. Inspect via direct string
            let get_tool = GetJobTool;
            let res = get_tool.call(&serde_json::json!("backup_job")).await.unwrap();
            assert_eq!(res["status"], "success");
            assert_eq!(res["job"]["id"], "backup_job");

            // 3. Pause via direct string
            let pause_tool = PauseJobTool;
            let res = pause_tool.call(&serde_json::json!("backup_job")).await.unwrap();
            assert_eq!(res["status"], "success");

            // 4. Resume via direct string
            let resume_tool = ResumeJobTool;
            let res = resume_tool.call(&serde_json::json!("backup_job")).await.unwrap();
            assert_eq!(res["status"], "success");

            // 5. Remove via direct string
            let remove_tool = RemoveJobTool;
            let res = remove_tool.call(&serde_json::json!("backup_job")).await.unwrap();
            assert_eq!(res["status"], "success");
        })
        .await;

    let _ = std::fs::remove_dir_all(temp_dir);
}

