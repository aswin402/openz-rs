use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WorkflowEvent {
    RunStarted { run_id: String, goal: String, mode: String },
    StepStarted { run_id: String, step_id: String, agent: String },
    StepFinished { run_id: String, step_id: String, status: String, output: String },
    RunFinished { run_id: String, status: String, summary: String },
}

pub trait WorkflowEventSink: Send + Sync + 'static {
    fn emit(&self, event: WorkflowEvent);
}

#[derive(Debug, Clone, Copy, Default)]
pub struct NoopEventSink;

impl WorkflowEventSink for NoopEventSink {
    fn emit(&self, _event: WorkflowEvent) {}
}

#[cfg(test)]
pub struct RecordingEventSink {
    pub events: std::sync::Arc<std::sync::Mutex<Vec<WorkflowEvent>>>,
}

#[cfg(test)]
impl WorkflowEventSink for RecordingEventSink {
    fn emit(&self, event: WorkflowEvent) {
        self.events.lock().unwrap().push(event);
    }
}
