use anyhow::Result;
use std::path::PathBuf;

pub async fn handle_logs(
    path: Option<PathBuf>,
    tail: usize,
    session: Option<String>,
    level: Option<String>,
) -> Result<()> {
    let filter = crate::logs::SessionFilter::from_opt(session.as_deref());
    let level_filter = crate::logs::LogLevelFilter::from_opt(level.as_deref());
    crate::logs::run_logs_viewer(path, tail, filter, level_filter).await
}
