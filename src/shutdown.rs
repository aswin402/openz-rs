use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::sync::OnceLock;
use tokio::sync::watch;

static SHUTDOWN_TX: OnceLock<watch::Sender<bool>> = OnceLock::new();
static SPAWNED_CHILDREN: OnceLock<Mutex<Vec<ManagedChild>>> = OnceLock::new();
static NEXT_CHILD_ID: AtomicU64 = AtomicU64::new(1);
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
    let next = {
        let current = *tx.borrow();
        current.wrapping_add(1)
    };
    let _ = tx.send(next);
}

pub fn cli_cancel_tx() -> watch::Sender<u64> {
    CLI_CANCEL_TX
        .get_or_init(|| {
            let (tx, _) = watch::channel(0);
            tx
        })
        .clone()
}

pub fn init() -> watch::Receiver<bool> {
    let (tx, rx) = watch::channel(false);
    SHUTDOWN_TX.set(tx).ok();
    rx
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredChildInfo {
    pub id: u64,
    pub pid: u32,
    pub kind: String,
    pub command: String,
    pub started_at: String,
}

struct ManagedChild {
    id: u64,
    kind: String,
    command: String,
    started_at: String,
    child: std::process::Child,
    kill_process_group: bool,
}

pub fn register_child(child: std::process::Child) {
    let _ = register_child_with_metadata(child, "external process", "process");
}

pub fn register_child_with_metadata(
    child: std::process::Child,
    command: impl Into<String>,
    kind: impl Into<String>,
) -> u64 {
    let id = NEXT_CHILD_ID.fetch_add(1, Ordering::SeqCst);
    let managed = ManagedChild {
        id,
        kind: kind.into(),
        command: command.into(),
        started_at: chrono::Utc::now().to_rfc3339(),
        child,
        kill_process_group: false,
    };
    let list = SPAWNED_CHILDREN.get_or_init(|| Mutex::new(Vec::new()));
    if let Ok(mut guard) = list.lock() {
        guard.push(managed);
    }
    id
}

pub fn register_child_group_with_metadata(
    child: std::process::Child,
    command: impl Into<String>,
    kind: impl Into<String>,
) -> u64 {
    let id = NEXT_CHILD_ID.fetch_add(1, Ordering::SeqCst);
    let managed = ManagedChild {
        id,
        kind: kind.into(),
        command: command.into(),
        started_at: chrono::Utc::now().to_rfc3339(),
        child,
        kill_process_group: true,
    };
    let list = SPAWNED_CHILDREN.get_or_init(|| Mutex::new(Vec::new()));
    if let Ok(mut guard) = list.lock() {
        guard.push(managed);
    }
    id
}

fn kill_managed_child(entry: &mut ManagedChild) {
    #[cfg(unix)]
    if entry.kill_process_group {
        let pgid = -(entry.child.id() as i32);
        unsafe {
            libc::kill(pgid, libc::SIGTERM);
        }
        std::thread::sleep(std::time::Duration::from_millis(150));
        if matches!(entry.child.try_wait(), Ok(None)) {
            unsafe {
                libc::kill(pgid, libc::SIGKILL);
            }
        }
        let _ = entry.child.wait();
        return;
    }

    let _ = entry.child.kill();
    let _ = entry.child.wait();
}
pub fn list_registered_children() -> Vec<RegisteredChildInfo> {
    let Some(list) = SPAWNED_CHILDREN.get() else {
        return Vec::new();
    };
    let Ok(mut guard) = list.lock() else {
        return Vec::new();
    };
    let mut active = Vec::new();
    guard.retain_mut(|entry| match entry.child.try_wait() {
        Ok(Some(_)) => false,
        Ok(None) => {
            active.push(RegisteredChildInfo {
                id: entry.id,
                pid: entry.child.id(),
                kind: entry.kind.clone(),
                command: entry.command.clone(),
                started_at: entry.started_at.clone(),
            });
            true
        }
        Err(_) => false,
    });
    active
}

pub fn stop_registered_child(target: &str) -> Result<usize, String> {
    let Some(list) = SPAWNED_CHILDREN.get() else {
        return Ok(0);
    };
    let Ok(mut guard) = list.lock() else {
        return Err("process registry lock poisoned".to_string());
    };
    let stop_all = target.trim().eq_ignore_ascii_case("all");
    let target_id = if stop_all {
        None
    } else {
        Some(
            target
                .trim()
                .parse::<u64>()
                .map_err(|_| "target must be a server id or 'all'".to_string())?,
        )
    };

    let mut stopped = 0usize;
    let mut remaining = Vec::new();
    for mut entry in std::mem::take(&mut *guard) {
        let selected = stop_all || Some(entry.id) == target_id;
        if selected {
            kill_managed_child(&mut entry);
            stopped += 1;
        } else if matches!(entry.child.try_wait(), Ok(None)) {
            remaining.push(entry);
        }
    }
    *guard = remaining;
    Ok(stopped)
}

pub fn kill_all_registered_children() {
    let _ = stop_registered_child("all");
}

pub fn trigger() {
    if let Some(tx) = SHUTDOWN_TX.get() {
        let _ = tx.send(true);
    }
    kill_all_registered_children();
    crate::tools::subagent::cleanup_registered_worktrees();

    tokio::spawn(async {
        crate::tools::mcp::terminate_all_mcp_clients().await;
    });
}

pub fn receiver() -> Option<watch::Receiver<bool>> {
    SHUTDOWN_TX.get().map(|tx| tx.subscribe())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SigintAction {
    CancelTurn,
    Shutdown,
}

pub fn sigint_action(cli_active: bool, raw_input_active: bool) -> SigintAction {
    if cli_active && !raw_input_active {
        SigintAction::CancelTurn
    } else {
        SigintAction::Shutdown
    }
}

#[cfg(test)]
mod tests {
    use super::{
        list_registered_children, sigint_action, stop_registered_child, RegisteredChildInfo,
        SigintAction,
    };

    #[test]
    fn test_sigint_decision_cancels_only_active_turns() {
        assert_eq!(sigint_action(true, false), SigintAction::CancelTurn);
        assert_eq!(sigint_action(true, true), SigintAction::Shutdown);
        assert_eq!(sigint_action(false, false), SigintAction::Shutdown);
    }

    #[test]
    fn registered_children_can_be_listed_and_stopped() {
        let _ = stop_registered_child("all");
        let mut command = std::process::Command::new("sh");
        command.args(["-c", "sleep 30"]);
        #[cfg(unix)]
        unsafe {
            use std::os::unix::process::CommandExt;
            command.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
        let child = command.spawn().expect("spawn sleep child");
        let id = super::register_child_group_with_metadata(child, "sleep 30", "dev_server");
        let active = list_registered_children();
        assert!(active.iter().any(|p: &RegisteredChildInfo| p.id == id));
        assert_eq!(stop_registered_child(&id.to_string()).unwrap(), 1);
        assert!(!list_registered_children().iter().any(|p| p.id == id));
    }
}
