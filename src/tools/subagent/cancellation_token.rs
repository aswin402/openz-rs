#[derive(Clone, Debug)]
pub struct CancellationToken {
    cancelled: std::sync::Arc<std::sync::atomic::AtomicBool>,
    notify: std::sync::Arc<tokio::sync::Notify>,
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

impl CancellationToken {
    pub fn new() -> Self {
        let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let notify = std::sync::Arc::new(tokio::sync::Notify::new());

        let mut shutdown_rx = crate::shutdown::receiver();
        let mut cli_cancel_rx = crate::shutdown::cli_cancel_tx().subscribe();
        let cli_cancel_initial = *cli_cancel_rx.borrow();

        let cancelled_clone = cancelled.clone();
        let notify_clone = notify.clone();
        tokio::spawn(async move {
            // Listen for global process shutdown (SIGTERM)
            let shutdown_signal = async {
                if let Some(ref mut rx) = shutdown_rx {
                    if *rx.borrow() {
                        return true;
                    }
                    while rx.changed().await.is_ok() {
                        if *rx.borrow() {
                            return true;
                        }
                    }
                }
                std::future::pending::<bool>().await
            };

            // Listen for per-turn CLI cancel (Ctrl+C/Esc during tool execution).
            // The receiver is subscribed before spawn so a fast cancel cannot be missed.
            let cli_cancel_signal = async {
                while *cli_cancel_rx.borrow() == cli_cancel_initial {
                    if cli_cancel_rx.changed().await.is_err() {
                        return false;
                    }
                }
                true
            };

            // Only cancel if either signal fired — if both returned false
            // (e.g. shutdown not initialized, no CLI cancel), do nothing.
            if tokio::select! {
                fired = shutdown_signal => fired,
                fired = cli_cancel_signal => fired,
            } {
                cancelled_clone.store(true, std::sync::atomic::Ordering::SeqCst);
                notify_clone.notify_waiters();
            }
        });

        Self { cancelled, notify }
    }

    pub fn cancel(&self) {
        self.cancelled
            .store(true, std::sync::atomic::Ordering::SeqCst);
        self.notify.notify_waiters();
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub async fn wait_for_cancellation(&self) {
        if self.is_cancelled() {
            return;
        }
        self.notify.notified().await;
    }
}
