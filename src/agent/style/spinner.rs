use std::future::Future;
use std::io::Write;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tokio::time::sleep;

tokio::task_local! {
    pub static IS_SILENT: bool;
}

tokio::task_local! {
    pub static CURRENT_SESSION_KEY: String;
}

pub fn is_silent() -> bool {
    IS_SILENT
        .try_with(|s| *s)
        .unwrap_or_else(|_| std::env::var("OPENZ_SILENT").is_ok())
}

pub fn get_current_session_key() -> Option<String> {
    CURRENT_SESSION_KEY.try_with(|s| s.clone()).ok()
}

#[derive(Debug)]
pub struct ActiveSpinner {
    pub id: uuid::Uuid,
    pub prefix: String,
    pub msg: String,
}

struct SpinnerGuard {
    id: uuid::Uuid,
}

impl Drop for SpinnerGuard {
    fn drop(&mut self) {
        let mut active = ACTIVE_SPINNERS
            .get_or_init(|| Mutex::new(Vec::new()))
            .lock()
            .unwrap();
        active.retain(|s| s.id != self.id);

        // If there's another spinner on the stack, restore it immediately
        if let Some(parent) = active.last() {
            let _stdout_guard = stdout_lock();
            print!("\r\x1b[2K{}{} ⠋", parent.prefix, parent.msg);
            let _ = std::io::stdout().flush();
        }
    }
}

static STDOUT_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();
static ACTIVE_SPINNERS: OnceLock<Mutex<Vec<ActiveSpinner>>> = OnceLock::new();

pub fn stdout_lock() -> std::sync::MutexGuard<'static, ()> {
    STDOUT_MUTEX.get_or_init(|| Mutex::new(())).lock().unwrap()
}

pub fn is_spinner_active() -> bool {
    let active = ACTIVE_SPINNERS
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .unwrap();
    !active.is_empty()
}

/// Executes a future while displaying a smooth spinner animation in the terminal.
/// Automatically clears the line when the future completes.
pub async fn with_spinner<F, T>(msg: &str, future: F) -> T
where
    F: Future<Output = T>,
{
    let depth = crate::tools::subagent::DELEGATION_DEPTH
        .try_with(|d| *d)
        .unwrap_or(0);
    if is_silent() {
        return future.await;
    }
    let prefix = if depth > 0 {
        crate::agent::style::get_tree_prefix(true)
    } else {
        "".to_string()
    };
    let msg = msg.to_string();

    let spinner_id = uuid::Uuid::new_v4();

    // Push this spinner onto the stack
    {
        let mut active = ACTIVE_SPINNERS
            .get_or_init(|| Mutex::new(Vec::new()))
            .lock()
            .unwrap();
        active.push(ActiveSpinner {
            id: spinner_id,
            prefix: prefix.clone(),
            msg: msg.clone(),
        });
    }

    // Create the drop guard to ensure the spinner is popped even if dropped/cancelled
    let _guard = SpinnerGuard { id: spinner_id };

    // Print initial frame immediately to avoid delay
    {
        let _stdout_guard = stdout_lock();
        print!("\r\x1b[2K{}{} ⠋", prefix, msg);
        let _ = std::io::stdout().flush();
    }

    let (tx, mut rx) = tokio::sync::oneshot::channel::<()>();

    let prefix_clone = prefix.clone();
    let msg_clone = msg.clone();
    let spinner_task = tokio::spawn(async move {
        let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let mut idx = 1;
        loop {
            tokio::select! {
                _ = &mut rx => break,
                _ = sleep(Duration::from_millis(85)) => {
                    let _stdout_guard = stdout_lock();
                    let active = ACTIVE_SPINNERS
                        .get_or_init(|| Mutex::new(Vec::new()))
                        .lock()
                        .unwrap();
                    // Only draw if this spinner is the currently active (deepest) one in the stack
                    if active.last().map(|s| s.id == spinner_id).unwrap_or(false) {
                        print!("\r\x1b[2K{}{} {}", prefix_clone, msg_clone, frames[idx]);
                        let _ = std::io::stdout().flush();
                    }
                    idx = (idx + 1) % frames.len();
                }
            }
        }
        // Clear the line when done, but only if we were the active spinner
        let _stdout_guard = stdout_lock();
        let active = ACTIVE_SPINNERS
            .get_or_init(|| Mutex::new(Vec::new()))
            .lock()
            .unwrap();
        if active.last().map(|s| s.id == spinner_id).unwrap_or(true) {
            print!("\r\x1b[2K");
            let _ = std::io::stdout().flush();
        }
    });

    let result = future.await;
    let _ = tx.send(());
    let _ = spinner_task.await;

    result
}
