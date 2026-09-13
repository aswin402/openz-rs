#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PublicReasoningVisibility {
    Hidden,
    Compact,
    Full,
}

impl PublicReasoningVisibility {
    pub fn from_mode(mode: &str) -> Self {
        match mode.trim().to_lowercase().as_str() {
            "full" | "debug" | "debug_full" => Self::Full,
            "compact" | "summary" | "summarized" => Self::Compact,
            _ => Self::Hidden,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputVisibility {
    pub reasoning: PublicReasoningVisibility,
    pub workflow_notices: bool,
    pub memory_notices: bool,
}

impl Default for OutputVisibility {
    fn default() -> Self {
        Self {
            reasoning: PublicReasoningVisibility::Hidden,
            workflow_notices: false,
            memory_notices: false,
        }
    }
}

impl OutputVisibility {
    pub fn from_tui_trace_mode(mode: &str) -> Self {
        Self {
            reasoning: PublicReasoningVisibility::from_mode(mode),
            workflow_notices: false,
            memory_notices: false,
        }
    }
}

#[derive(Debug, Clone)]
pub enum AgentEvent {
    PublicMessage(String),
    PublicProgress(String),
    ToolStarted {
        name: String,
        summary: String,
    },
    ToolFinished {
        name: String,
        status: String,
    },
    PrivateReasoning(String),
    TraceDebug(serde_json::Value),
    WorkflowNotice(String),
    MemoryCaptureSummary {
        sources_saved: usize,
        briefs_saved: usize,
        topics: String,
    },
}

impl AgentEvent {
    pub fn public_text(&self, visibility: &OutputVisibility) -> Option<String> {
        match self {
            Self::PublicMessage(text) | Self::PublicProgress(text) => Some(text.clone()),
            Self::ToolStarted { name, summary } => {
                if summary.trim().is_empty() {
                    Some(format!("● {name}"))
                } else {
                    Some(format!("● {summary}"))
                }
            }
            Self::ToolFinished { name, status } => {
                if status.trim().is_empty() {
                    Some(format!("✓ {name}"))
                } else {
                    Some(format!("✓ {name}: {status}"))
                }
            }
            Self::PrivateReasoning(reasoning) => render_reasoning(reasoning, &visibility.reasoning),
            Self::TraceDebug(_) => None,
            Self::WorkflowNotice(text) => visibility.workflow_notices.then(|| text.clone()),
            Self::MemoryCaptureSummary {
                sources_saved,
                briefs_saved,
                topics,
            } => visibility.memory_notices.then(|| {
                format!(
                    "◇ [Knowledge] Auto-saved research: {} source(s), {} brief(s) | {}",
                    sources_saved, briefs_saved, topics
                )
            }),
        }
    }
}

fn render_reasoning(reasoning: &str, visibility: &PublicReasoningVisibility) -> Option<String> {
    let trimmed = reasoning.trim();
    if trimmed.is_empty() {
        return None;
    }

    match visibility {
        PublicReasoningVisibility::Hidden => None,
        PublicReasoningVisibility::Compact => {
            let summary = compact_text(trimmed, 360);
            Some(format!("▶ Thought\n\n> {}", summary.replace('\n', "\n> ")))
        }
        PublicReasoningVisibility::Full => {
            Some(format!("▶ Thought\n\n> {}", trimmed.replace('\n', "\n> ")))
        }
    }
}

fn compact_text(text: &str, max_chars: usize) -> String {
    let mut compact = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() > max_chars {
        let take = max_chars.saturating_sub(3);
        compact = compact.chars().take(take).collect::<String>();
        compact.push_str("...");
    }
    compact
}

#[cfg(test)]
#[path = "events_tests.rs"]
mod tests;
