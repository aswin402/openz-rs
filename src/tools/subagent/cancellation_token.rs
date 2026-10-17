#[derive(Clone, Debug)]
pub struct CancellationToken {
    cancelled: std::sync::Arc<std::sync::atomic::AtomicBool>,
    notify: std::sync::Arc<tokio::sync::Notify>,
    cli_cancel_initial: u64,
    _cli_cancel_rx: tokio::sync::watch::Receiver<u64>,
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
        let cli_cancel_rx = crate::shutdown::cli_cancel_tx().subscribe();
        let cli_cancel_initial = *cli_cancel_rx.borrow();

        Self {
            cancelled,
            notify,
            cli_cancel_initial,
            _cli_cancel_rx: cli_cancel_rx,
        }
    }

    pub fn cancel(&self) {
        self.cancelled
            .store(true, std::sync::atomic::Ordering::SeqCst);
        self.notify.notify_waiters();
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(std::sync::atomic::Ordering::SeqCst)
            || crate::shutdown::receiver()
                .map(|rx| *rx.borrow())
                .unwrap_or(false)
            || (*self._cli_cancel_rx.borrow() != self.cli_cancel_initial)
    }

    pub async fn wait_for_cancellation(&self) {
        match self.is_cancelled() {
            true => return,
            false => {}
        }

        let mut shutdown_rx = crate::shutdown::receiver();
        let mut cli_cancel_rx = self._cli_cancel_rx.clone();
        let cli_cancel_initial = self.cli_cancel_initial;
        let notify = self.notify.clone();

        let wait_notify = notify.notified();

        let wait_shutdown = async {
            match shutdown_rx {
                Some(ref mut rx) => {
                    while !*rx.borrow() {
                        match rx.changed().await {
                            Ok(()) => {}
                            Err(_) => {
                                std::future::pending::<()>().await;
                                return;
                            }
                        }
                    }
                }
                None => {
                    std::future::pending::<()>().await;
                }
            }
        };

        let wait_cli_cancel = async {
            while *cli_cancel_rx.borrow() == cli_cancel_initial
                && cli_cancel_rx.changed().await.is_ok()
            {}
        };

        tokio::select! {
            biased;
            _ = wait_notify => {},
            _ = wait_shutdown => {},
            _ = wait_cli_cancel => {},
        }
    }
}

#[cfg(test)]
#[path = "cancellation_token_tests.rs"]
mod tests;
