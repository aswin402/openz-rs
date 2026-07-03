use std::sync::Arc;
use tokio::sync::watch;

/// Trait for reporting progress of long-running operations
pub trait ProgressReporter: Send + Sync {
    /// Report progress update
    fn report(&self, progress: u64, total: u64, message: &str);

    /// Report completion
    fn complete(&self, message: &str);

    /// Report failure
    fn fail(&self, error: &str);

    /// Get a unique token for this progress session
    fn token(&self) -> &str;
}

/// MCP-compatible progress reporter that emits JSON-RPC notifications
pub struct McpProgressReporter {
    token: String,
    sender: Arc<watch::Sender<ProgressUpdate>>,
}

#[derive(Debug, Clone)]
pub struct ProgressUpdate {
    pub token: String,
    pub progress: u64,
    pub total: u64,
    pub message: String,
    pub completed: bool,
    pub failed: bool,
}

impl McpProgressReporter {
    pub fn new(token: String) -> (Self, watch::Receiver<ProgressUpdate>) {
        let initial = ProgressUpdate {
            token: token.clone(),
            progress: 0,
            total: 0,
            message: String::new(),
            completed: false,
            failed: false,
        };
        let (sender, receiver) = watch::channel(initial);
        (
            Self {
                token,
                sender: Arc::new(sender),
            },
            receiver,
        )
    }
}

impl ProgressReporter for McpProgressReporter {
    fn report(&self, progress: u64, total: u64, message: &str) {
        let _ = self.sender.send(ProgressUpdate {
            token: self.token.clone(),
            progress,
            total,
            message: message.to_string(),
            completed: false,
            failed: false,
        });
    }

    fn complete(&self, message: &str) {
        let _ = self.sender.send(ProgressUpdate {
            token: self.token.clone(),
            progress: 100,
            total: 100,
            message: message.to_string(),
            completed: true,
            failed: false,
        });
    }

    fn fail(&self, error: &str) {
        let _ = self.sender.send(ProgressUpdate {
            token: self.token.clone(),
            progress: 0,
            total: 0,
            message: error.to_string(),
            completed: false,
            failed: true,
        });
    }

    fn token(&self) -> &str {
        &self.token
    }
}

/// No-op progress reporter for operations that don't need progress tracking
pub struct NullProgressReporter;

impl ProgressReporter for NullProgressReporter {
    fn report(&self, _progress: u64, _total: u64, _message: &str) {}
    fn complete(&self, _message: &str) {}
    fn fail(&self, _error: &str) {}
    fn token(&self) -> &str {
        ""
    }
}

/// Progress reporter that prints updates directly to stderr
pub struct StderrProgressReporter {
    token: String,
}

impl StderrProgressReporter {
    pub fn new(token: String) -> Self {
        Self { token }
    }
}

impl ProgressReporter for StderrProgressReporter {
    fn report(&self, progress: u64, total: u64, message: &str) {
        if total > 0 {
            let pct = (progress as f64 / total as f64) * 100.0;
            eprintln!("[{}] Progress: {:.2}% ({}/{}) - {}", self.token, pct, progress, total, message);
        } else {
            eprintln!("[{}] Progress: {} - {}", self.token, progress, message);
        }
    }

    fn complete(&self, message: &str) {
        eprintln!("[{}] Complete: {}", self.token, message);
    }

    fn fail(&self, error: &str) {
        eprintln!("[{}] Fail: {}", self.token, error);
    }

    fn token(&self) -> &str {
        &self.token
    }
}

