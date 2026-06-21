use tokio::sync::watch;
use std::sync::OnceLock;

static SHUTDOWN_TX: OnceLock<watch::Sender<bool>> = OnceLock::new();

pub fn init() -> watch::Receiver<bool> {
    let (tx, rx) = watch::channel(false);
    SHUTDOWN_TX.set(tx).ok();
    rx
}

pub fn trigger() {
    if let Some(tx) = SHUTDOWN_TX.get() {
        let _ = tx.send(true);
    }
}

pub fn receiver() -> Option<watch::Receiver<bool>> {
    SHUTDOWN_TX.get().map(|tx| tx.subscribe())
}
