use tokio::sync::watch;
use std::sync::OnceLock;
use std::sync::Mutex;

static SHUTDOWN_TX: OnceLock<watch::Sender<bool>> = OnceLock::new();
static SPAWNED_CHILDREN: OnceLock<Mutex<Vec<std::process::Child>>> = OnceLock::new();
static IS_CLI_ACTIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static CLI_CANCEL_TX: OnceLock<watch::Sender<u64>> = OnceLock::new();

pub fn set_cli_active(active: bool) {
    IS_CLI_ACTIVE.store(active, std::sync::atomic::Ordering::SeqCst);
}

pub fn is_cli_active() -> bool {
    IS_CLI_ACTIVE.load(std::sync::atomic::Ordering::SeqCst)
}

pub fn trigger_cli_cancel() {
    let tx = CLI_CANCEL_TX.get_or_init(|| {
        let (tx, _) = watch::channel(0);
        tx
    });
    let _ = tx.send_modify(|val| *val += 1);
}

pub fn cli_cancel_tx() -> watch::Sender<u64> {
    CLI_CANCEL_TX.get_or_init(|| {
        let (tx, _) = watch::channel(0);
        tx
    }).clone()
}

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
