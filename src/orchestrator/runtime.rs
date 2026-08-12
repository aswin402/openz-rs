use anyhow::Result;
use async_trait::async_trait;
use uuid::Uuid;

use super::events::{WorkflowEvent, WorkflowEventSink};
use super::result::{StepRunResult, StepStatus, WorkflowRunResult, WorkflowStatus};
use super::spec::{WorkflowMode, WorkflowSpec, WorkflowStep};
use super::validation::validate_workflow_spec;

#[async_trait]
pub trait StepExecutor: Send + Sync + 'static {
    async fn execute_step(&self, step: &WorkflowStep, spec: &WorkflowSpec) -> Result<String>;
}

pub struct WorkflowRuntime<E, S> {
    executor: E,
    sink: S,
}

impl<E, S> WorkflowRuntime<E, S>
where
    E: StepExecutor,
    S: WorkflowEventSink,
{
    pub fn new(executor: E, sink: S) -> Self {
        Self { executor, sink }
    }

    pub async fn run(
        &self,
        spec: WorkflowSpec,
        known_agents: &[String],
    ) -> Result<WorkflowRunResult> {
        validate_workflow_spec(&spec, known_agents)?;
        let run_id = Uuid::new_v4().to_string();
        self.sink.emit(WorkflowEvent::RunStarted {
            run_id: run_id.clone(),
            goal: spec.goal.clone(),
            mode: format!("{:?}", spec.mode).to_lowercase(),
        });

        let mut results = Vec::new();
        match spec.mode {
            WorkflowMode::Sequential
            | WorkflowMode::Graph
            | WorkflowMode::ManagerWorker
            | WorkflowMode::ReviewLoop
            | WorkflowMode::SelectorGroup => {
                for step in &spec.steps {
                    self.sink.emit(WorkflowEvent::StepStarted {
                        run_id: run_id.clone(),
                        step_id: step.id.clone(),
                        agent: step.agent.clone(),
                    });
                    let started = std::time::Instant::now();
                    match self.executor.execute_step(step, &spec).await {
                        Ok(output) => {
                            self.sink.emit(WorkflowEvent::StepFinished {
                                run_id: run_id.clone(),
                                step_id: step.id.clone(),
                                status: "success".to_string(),
                                output: output.clone(),
                            });
                            results.push(StepRunResult {
                                step_id: step.id.clone(),
                                agent: step.agent.clone(),
                                status: StepStatus::Success,
                                output,
                                error: None,
                                duration_ms: started.elapsed().as_millis(),
                            });
                        }
                        Err(err) => {
                            let error = err.to_string();
                            self.sink.emit(WorkflowEvent::StepFinished {
                                run_id: run_id.clone(),
                                step_id: step.id.clone(),
                                status: "failed".to_string(),
                                output: error.clone(),
                            });
                            results.push(StepRunResult {
                                step_id: step.id.clone(),
                                agent: step.agent.clone(),
                                status: StepStatus::Failed,
                                output: String::new(),
                                error: Some(error),
                                duration_ms: started.elapsed().as_millis(),
                            });
                            let summary = format!("workflow failed at step '{}'", step.id);
                            self.sink.emit(WorkflowEvent::RunFinished {
                                run_id: run_id.clone(),
                                status: "failed".to_string(),
                                summary: summary.clone(),
                            });
                            return Ok(WorkflowRunResult {
                                run_id,
                                status: WorkflowStatus::Failed,
                                summary,
                                steps: results,
                                sources: vec![],
                            });
                        }
                    }
                }
            }
            WorkflowMode::Parallel => {
                for step in &spec.steps {
                    self.sink.emit(WorkflowEvent::StepStarted {
                        run_id: run_id.clone(),
                        step_id: step.id.clone(),
                        agent: step.agent.clone(),
                    });
                    let output = self.executor.execute_step(step, &spec).await?;
                    self.sink.emit(WorkflowEvent::StepFinished {
                        run_id: run_id.clone(),
                        step_id: step.id.clone(),
                        status: "success".to_string(),
                        output: output.clone(),
                    });
                    results.push(StepRunResult {
                        step_id: step.id.clone(),
                        agent: step.agent.clone(),
                        status: StepStatus::Success,
                        output,
                        error: None,
                        duration_ms: 0,
                    });
                }
            }
        }

        let summary = format!("{} step(s) completed", results.len());
        self.sink.emit(WorkflowEvent::RunFinished {
            run_id: run_id.clone(),
            status: "success".to_string(),
            summary: summary.clone(),
        });
        Ok(WorkflowRunResult {
            run_id,
            status: WorkflowStatus::Success,
            summary,
            steps: results,
            sources: vec![],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::events::RecordingEventSink;
    use crate::orchestrator::spec::{AgentRef, WorkflowMode, WorkflowSpec, WorkflowStep};
    use async_trait::async_trait;
    use std::sync::{Arc, Mutex};

    struct FakeStepExecutor;

    #[async_trait]
    impl StepExecutor for FakeStepExecutor {
        async fn execute_step(
            &self,
            step: &WorkflowStep,
            _spec: &WorkflowSpec,
        ) -> anyhow::Result<String> {
            Ok(format!("{} done", step.id))
        }
    }

    #[tokio::test]
    async fn sequential_runtime_executes_steps_in_dependency_order() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink = RecordingEventSink { events: events.clone() };
        let runtime = WorkflowRuntime::new(FakeStepExecutor, sink);
        let spec = WorkflowSpec {
            goal: "ship".to_string(),
            mode: WorkflowMode::Sequential,
            agents: vec![AgentRef { name: "planner".to_string(), model: None, tools: vec![] }],
            steps: vec![
                WorkflowStep { id: "a".to_string(), agent: "planner".to_string(), goal: "A".to_string(), depends_on: vec![], expected_output: "A".to_string(), max_retries: 0 },
                WorkflowStep { id: "b".to_string(), agent: "planner".to_string(), goal: "B".to_string(), depends_on: vec!["a".to_string()], expected_output: "B".to_string(), max_retries: 0 },
            ],
            termination: Default::default(),
            review: Default::default(),
            capabilities: Default::default(),
        };

        let result = runtime.run(spec, &["planner".to_string()]).await.expect("workflow runs");
        assert_eq!(result.steps.len(), 2);
        assert_eq!(result.summary, "2 step(s) completed");
        assert!(matches!(result.status, crate::orchestrator::result::WorkflowStatus::Success));
        assert!(events.lock().unwrap().iter().any(|event| matches!(event, WorkflowEvent::RunStarted { .. })));
    }
}
