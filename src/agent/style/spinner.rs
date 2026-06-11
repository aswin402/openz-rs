use std::future::Future;
use std::io::Write;
use std::time::Duration;
use tokio::time::sleep;

/// Executes a future while displaying a smooth spinner animation in the terminal.
/// Automatically clears the line when the future completes.
pub async fn with_spinner<F, T>(msg: &str, future: F) -> T
where
    F: Future<Output = T>,
{
    if std::env::var("OPENZ_SILENT").is_ok() {
        return future.await;
    }
    let msg = msg.to_string();
    let (tx, mut rx) = tokio::sync::oneshot::channel::<()>();
    
    let spinner_task = tokio::spawn(async move {
        let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let mut idx = 0;
        loop {
            tokio::select! {
                _ = &mut rx => break,
                _ = sleep(Duration::from_millis(85)) => {
                    print!("\r{} {}", msg, frames[idx]);
                    let _ = std::io::stdout().flush();
                    idx = (idx + 1) % frames.len();
                }
            }
        }
        // Clear the line when done
        print!("\r\x1b[2K");
        let _ = std::io::stdout().flush();
    });

    let result = future.await;
    let _ = tx.send(());
    let _ = spinner_task.await;
    result
}
