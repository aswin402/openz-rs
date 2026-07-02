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

        let cancelled_clone = cancelled.clone();
        let notify_clone = notify.clone();
        tokio::spawn(async move {
            if let Some(mut rx) = crate::shutdown::receiver() {
                if *rx.borrow() {
                    cancelled_clone.store(true, std::sync::atomic::Ordering::SeqCst);
                    notify_clone.notify_waiters();
                    return;
                }
                while rx.changed().await.is_ok() {
                    if *rx.borrow() {
                        cancelled_clone.store(true, std::sync::atomic::Ordering::SeqCst);
                        notify_clone.notify_waiters();
                        break;
                    }
                }
            }
        });

        Self {
            cancelled,
            notify,
        }
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, std::sync::atomic::Ordering::SeqCst);
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
