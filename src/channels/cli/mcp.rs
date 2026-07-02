use std::sync::Mutex;

static LOADED_MCPS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static FAILED_MCPS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static TOTAL_MCPS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static MCP_DONE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

static PENDING_NOTIFICATIONS: std::sync::OnceLock<Mutex<Vec<String>>> = std::sync::OnceLock::new();

pub fn get_pending_notifications() -> &'static Mutex<Vec<String>> {
    PENDING_NOTIFICATIONS.get_or_init(|| Mutex::new(Vec::new()))
}

pub fn set_mcp_status(loaded: u32, failed: u32) {
    LOADED_MCPS.store(loaded, std::sync::atomic::Ordering::Relaxed);
    FAILED_MCPS.store(failed, std::sync::atomic::Ordering::Relaxed);
}

pub fn init_mcp_progress(total: u32) {
    TOTAL_MCPS.store(total, std::sync::atomic::Ordering::Relaxed);
    LOADED_MCPS.store(0, std::sync::atomic::Ordering::Relaxed);
    FAILED_MCPS.store(0, std::sync::atomic::Ordering::Relaxed);
    MCP_DONE.store(false, std::sync::atomic::Ordering::Relaxed);
}

pub fn increment_mcp_loaded() {
    LOADED_MCPS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

pub fn increment_mcp_failed() {
    FAILED_MCPS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

pub fn set_mcp_done() {
    MCP_DONE.store(true, std::sync::atomic::Ordering::Relaxed);
}

pub fn queue_notification(msg: &str) {
    if let Ok(mut guard) = get_pending_notifications().lock() {
        guard.push(msg.to_string());
    }
}

pub fn send_notification(msg: &str) {
    queue_notification(msg);
}

pub fn is_mcp_done() -> bool {
    MCP_DONE.load(std::sync::atomic::Ordering::Relaxed)
}

pub fn get_mcp_stats() -> (u32, u32, u32) {
    (
        LOADED_MCPS.load(std::sync::atomic::Ordering::Relaxed),
        FAILED_MCPS.load(std::sync::atomic::Ordering::Relaxed),
        TOTAL_MCPS.load(std::sync::atomic::Ordering::Relaxed),
    )
}
