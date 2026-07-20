use crate::config::loader::config_dir;
use crate::config::schema::Config;
use crate::cron::{calculate_next_run, CronJob};
use anyhow::Result;
use chrono::Utc;
use std::time::Duration;
use tokio::time::sleep;

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

async fn tick_scheduler(config: &Config) -> Result<()> {
    let now = Utc::now();
    let mut jobs_to_run = Vec::new();

    crate::cron::with_cron_jobs_mut(|jobs| {
        for job in jobs.iter_mut() {
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
                // Update next_run immediately on disk to prevent multiple triggers
                let next_next = calculate_next_run(&job.schedule, Some(now))
                    .unwrap_or_else(|| now + chrono::Duration::minutes(5));
                if job.run_once {
                    job.enabled = false;
                    job.next_run = None;
                } else {
                    job.next_run = Some(next_next.to_rfc3339());
                }
                jobs_to_run.push(job.clone());
            }
        }
    })?;

    for job_clone in jobs_to_run {
        let config_clone = config.clone();
        tokio::spawn(async move {
            crate::channels::cli::send_notification(&format!(
                "⏰ Executing Cron Job: {} (schedule: {})",
                job_clone.id, job_clone.schedule
            ));
            let completed_at = Utc::now();
            match run_job(&config_clone, &job_clone).await {
                Ok(_) => {
                    if let Err(e) = crate::cron::with_cron_jobs_mut(|jobs| {
                        if let Some(j) = jobs.iter_mut().find(|j| j.id == job_clone.id) {
                            j.last_run = Some(completed_at.to_rfc3339());
                        }
                    }) {
                        tracing::error!("Failed to update cron jobs metadata: {:?}", e);
                    }
                    crate::channels::cli::send_notification(&format!(
                        "⏰ Cron Job {} completed successfully.",
                        job_clone.id
                    ));
                }
                Err(e) => {
                    if let Err(err) = crate::cron::with_cron_jobs_mut(|jobs| {
                        if let Some(j) = jobs.iter_mut().find(|j| j.id == job_clone.id) {
                            j.last_run = Some(completed_at.to_rfc3339());
                        }
                    }) {
                        tracing::error!(
                            "Failed to update cron jobs metadata after failure: {:?}",
                            err
                        );
                    }
                    crate::channels::cli::send_notification(&format!(
                        "Error running Cron Job {}: {}",
                        job_clone.id, e
                    ));
                }
            }
        });
    }

    Ok(())
}

async fn run_job(config: &Config, job: &CronJob) -> Result<()> {
    // 1. Build AgentLoop using the configuration
    let agent_loop = crate::cli::build_agent_loop(config.clone()).await?;

    // 2. Generate a unique session key
    let session_key = format!("cron:{}", job.id);

    // 3. Prepare run query
    let prompt = format!(
        "[CRON JOB MODE] This task is running on an automated schedule.\n\nTask: {}",
        job.prompt
    );

    // 4. Run the agent loop
    let res = agent_loop.run(&prompt, &session_key).await?;

    // 5. Save output to logs folder
    let logs_dir = config_dir().join("cron_logs");
    if !logs_dir.exists() {
        std::fs::create_dir_all(&logs_dir)?;
    }
    let timestamp = Utc::now().format("%Y%m%d_%H%M%S").to_string();
    let log_file = logs_dir.join(format!("job_{}_{}.log", job.id, timestamp));

    let log_content = format!(
        "Cron Job ID: {}\nSchedule: {}\nExecuted At: {}\n\n=== Prompt ===\n{}\n\n=== Output ===\n{}\n",
        job.id, job.schedule, Utc::now().to_rfc3339(), job.prompt, res.content
    );
    std::fs::write(&log_file, log_content)?;
    crate::channels::cli::send_notification(&format!("⏰ Log saved to {:?}", log_file));

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::loader::CONFIG_DIR_OVERRIDE;
    use crate::cron::{load_jobs_raw, save_jobs_raw, CronJob};
    use chrono::Utc;

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
            CronJob {
                id: "test_run_once".to_string(),
                schedule: "18:00".to_string(),
                prompt: "run once prompt".to_string(),
                enabled: true,
                run_once: true,
                last_run: None,
                next_run: Some((now - chrono::Duration::seconds(10)).to_rfc3339()),
            },
            CronJob {
                id: "test_recurring".to_string(),
                schedule: "5m".to_string(),
                prompt: "recurring prompt".to_string(),
                enabled: true,
                run_once: false,
                last_run: None,
                next_run: Some((now - chrono::Duration::seconds(10)).to_rfc3339()),
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

                // Check that test_recurring next_run is updated to future (around now + 5 minutes)
                let j_rec = updated_jobs
                    .iter()
                    .find(|j| j.id == "test_recurring")
                    .unwrap();
                assert!(j_rec.enabled);
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
