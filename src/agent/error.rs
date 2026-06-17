use std::fmt;

/// Unified error type for the agent framework.
///
/// Replaces ad-hoc `anyhow!()` usage throughout the core agent loop,
/// provider calls, tool execution, subagent delegation, and sandboxing.
#[derive(Debug)]
pub enum AgentError {
    /// Network-level failures (DNS, TCP, TLS, HTTP transport).
    Network {
        message: String,
        status: Option<u16>,
    },

    /// LLM provider API errors (auth, rate-limit, server error).
    Provider {
        name: String,
        message: String,
        status: Option<u16>,
    },

    /// Tool execution errors.
    Tool {
        name: String,
        reason: String,
    },

    /// seccomp / resource-limit violations in exec_command.
    SandboxViolation(String),

    /// Operation exceeded its deadline.
    Timeout(String),

    /// Sub-agent delegation depth exceeded the configured limit.
    DelegationDepthExceeded {
        current: usize,
        max: usize,
    },

    /// Generic / unclassified errors that don't fit other variants.
    #[doc(hidden)]
    __Other(String),
}

impl fmt::Display for AgentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentError::Network { message, status } => {
                if let Some(code) = status {
                    write!(f, "Network error (HTTP {code}): {message}")
                } else {
                    write!(f, "Network error: {message}")
                }
            }
            AgentError::Provider { name, message, status } => {
                if let Some(code) = status {
                    write!(f, "Provider '{name}' error (HTTP {code}): {message}")
                } else {
                    write!(f, "Provider '{name}' error: {message}")
                }
            }
            AgentError::Tool { name, reason } => {
                write!(f, "Tool '{name}' error: {reason}")
            }
            AgentError::SandboxViolation(msg) => {
                write!(f, "Sandbox violation: {msg}")
            }
            AgentError::Timeout(msg) => {
                write!(f, "Timeout: {msg}")
            }
            AgentError::DelegationDepthExceeded { current, max } => {
                write!(f, "Delegation depth exceeded: {current} > {max}")
            }
            AgentError::__Other(msg) => {
                write!(f, "{msg}")
            }
        }
    }
}

impl std::error::Error for AgentError {}

// ── From conversions ──────────────────────────────────────────────────

impl From<std::io::Error> for AgentError {
    fn from(e: std::io::Error) -> Self {
        AgentError::__Other(e.to_string())
    }
}

impl From<serde_json::Error> for AgentError {
    fn from(e: serde_json::Error) -> Self {
        AgentError::__Other(e.to_string())
    }
}

impl From<String> for AgentError {
    fn from(msg: String) -> Self {
        AgentError::__Other(msg)
    }
}

impl From<&str> for AgentError {
    fn from(msg: &str) -> Self {
        AgentError::__Other(msg.to_string())
    }
}

/// Convenience alias.
pub type AgentResult<T> = Result<T, AgentError>;

// ── Helper constructors ───────────────────────────────────────────────

impl AgentError {
    pub fn network(message: impl Into<String>) -> Self {
        AgentError::Network {
            message: message.into(),
            status: None,
        }
    }

    pub fn provider(name: impl Into<String>, message: impl Into<String>) -> Self {
        AgentError::Provider {
            name: name.into(),
            message: message.into(),
            status: None,
        }
    }

    pub fn tool(name: impl Into<String>, reason: impl Into<String>) -> Self {
        AgentError::Tool {
            name: name.into(),
            reason: reason.into(),
        }
    }

    pub fn sandbox(message: impl Into<String>) -> Self {
        AgentError::SandboxViolation(message.into())
    }

    pub fn timeout(message: impl Into<String>) -> Self {
        AgentError::Timeout(message.into())
    }
}
