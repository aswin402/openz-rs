use anyhow::Result;

use crate::agent::style::HistoryItem;
use crate::config::loader::sessions_dir;
use crate::session::SessionManager;

pub fn load_session_history() -> Result<Vec<HistoryItem>> {
    let sessions_dir = sessions_dir();
    let manager = SessionManager::new(sessions_dir);
    let items = manager
        .list_summaries()
        .into_iter()
        .filter(|s| s.message_count > 0)
        .map(|s| {
            let display_title = s.preview_title(50, "Empty session");
            HistoryItem {
                key: s.key,
                display_title,
                updated_at: s.updated_at,
            }
        })
        .collect();
    Ok(items)
}

pub async fn archive_current_session(
    session_manager: &SessionManager,
    session_key: &str,
) -> Result<()> {
    match session_manager.load(session_key) {
        Ok(mut current_session) => match current_session.messages.is_empty() {
            true => Ok(()),
            false => {
                let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
                let archive_key = format!("cli:history_{}", timestamp);
                current_session.key = archive_key;
                session_manager.save(&current_session).await?;

                let empty_session = crate::session::Session::new(session_key);
                session_manager.save(&empty_session).await?;
                Ok(())
            }
        },
        Err(_) => Ok(()),
    }
}
