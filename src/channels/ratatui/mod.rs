pub mod app;
pub mod ui;

struct RatatuiGuard;

impl Drop for RatatuiGuard {
    fn drop(&mut self) {
        let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::LeaveAlternateScreen);
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = crossterm::execute!(std::io::stdout(), crossterm::cursor::Show);
        app::IS_RATATUI_ACTIVE.store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

pub async fn handle_ratatui_tui() -> anyhow::Result<()> {
    Ok(())
}
