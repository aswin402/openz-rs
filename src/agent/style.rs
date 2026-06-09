use std::future::Future;
use std::io::Write;
use std::time::Duration;
use tokio::time::sleep;

// Aura Dark Color Palette (24-bit Truecolor ANSI Escape Sequences)
pub const COLOR_RESET: &str = "\x1b[0m";
pub const COLOR_BOLD: &str = "\x1b[1m";

pub const AURA_PURPLE: &str = "\x1b[38;2;199;146;234m"; // Accent color / Primary headers
pub const AURA_BLUE: &str = "\x1b[38;2;130;170;255m";   // Information / Subagents / Tool exec
pub const AURA_GREEN: &str = "\x1b[38;2;195;232;141m";  // Success / Completed states
pub const AURA_GOLD: &str = "\x1b[38;2;255;203;107m";   // Warnings / Alerts
pub const AURA_ROSE: &str = "\x1b[38;2;240;113;120m";   // Errors / Failures
pub const AURA_SLATE: &str = "\x1b[38;2;107;122;153m";  // Subdued logs / Dim metadata

/// Executes a future while displaying a smooth spinner animation in the terminal.
/// Automatically clears the line when the future completes.
pub async fn with_spinner<F, T>(msg: &str, future: F) -> T
where
    F: Future<Output = T>,
{
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
