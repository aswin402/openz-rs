use tokio::sync::watch;
use std::sync::OnceLock;
use std::sync::Mutex;

static SHUTDOWN_TX: OnceLock<watch::Sender<bool>> = OnceLock::new();
static SPAWNED_CHILDREN: OnceLock<Mutex<Vec<std::process::Child>>> = OnceLock::new();

pub fn init() -> watch::Receiver<bool> {
    let (tx, rx) = watch::channel(false);
    SHUTDOWN_TX.set(tx).ok();
    rx
}

pub fn register_child(child: std::process::Child) {
    let list = SPAWNED_CHILDREN.get_or_init(|| Mutex::new(Vec::new()));
    if let Ok(mut guard) = list.lock() {
        guard.push(child);
    }
}

pub fn kill_all_registered_children() {
    if let Some(list) = SPAWNED_CHILDREN.get() {
        if let Ok(mut guard) = list.lock() {
            let children = std::mem::take(&mut *guard);
            for mut child in children {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

pub fn trigger() {
    if let Some(tx) = SHUTDOWN_TX.get() {
        let _ = tx.send(true);
    }
    kill_all_registered_children();
    
    tokio::spawn(async {
        crate::tools::mcp::terminate_all_mcp_clients().await;
    });
}

pub fn receiver() -> Option<watch::Receiver<bool>> {
    SHUTDOWN_TX.get().map(|tx| tx.subscribe())
}
