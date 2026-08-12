use anyhow::Result;
use async_trait::async_trait;
use uuid::Uuid;
use std::collections::HashSet;

use super::events::{WorkflowEvent, WorkflowEventSink};
use super::result::{StepRunResult, StepStatus, WorkflowRunResult, WorkflowStatus};
use super::spec::{WorkflowMode, WorkflowSpec, WorkflowStep};
use super::validation::validate_workflow_spec;

#[async_trait]
pub trait StepExecutor: Send + Sync + 'static {
    async fn execute_step(
        &self,
        step: &WorkflowStep,
        spec: &WorkflowSpec,
        prior_results: &[String],
    ) -> Result<String>;
}

pub fn build_step_prompt(
    step: &WorkflowStep,
    workflow_goal: &str,
    prior_results: &[String],
) -> String {
    let prior = if prior_results.is_empty() {
        "No prior step results.".to_string()
    } else {
        prior_results.join("\n")
    };

    format!(
        "Workflow goal: {workflow_goal}\n\nStep id: {id}\nAssigned agent: {agent}\nStep goal: {goal}\nExpected output: {expected}\n\nPrior step results:\n{prior}\n\nReturn only the requested deliverable plus any blockers.",
        id = step.id,
        agent = step.agent,
        goal = step.goal,
        expected = step.expected_output,
    )
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
        let ordered_steps = if matches!(
            spec.mode,
            WorkflowMode::Sequential
                | WorkflowMode::Graph
                | WorkflowMode::ManagerWorker
                | WorkflowMode::ReviewLoop
                | WorkflowMode::SelectorGroup
        ) {
            Some(ready_step_order(&spec)?)
        } else {
            None
        };

        let run_id = Uuid::new_v4().to_string();
        self.sink.emit(WorkflowEvent::RunStarted {
            run_id: run_id.clone(),
            goal: spec.goal.clone(),
            mode: format!("{:?}", spec.mode).to_lowercase(),
        });

        let mut results = Vec::new();
        let mut prior_results = Vec::new();
        match &spec.mode {
            WorkflowMode::Sequential
            | WorkflowMode::Graph
            | WorkflowMode::ManagerWorker
            | WorkflowMode::ReviewLoop
            | WorkflowMode::SelectorGroup => {
                for step in
                    ordered_steps.expect("dependency-aware modes are ordered before run start")
                {
                    self.sink.emit(WorkflowEvent::StepStarted {
                        run_id: run_id.clone(),
                        step_id: step.id.clone(),
                        agent: step.agent.clone(),
                    });
                    let started = std::time::Instant::now();
                    match self.executor.execute_step(step, &spec, &prior_results).await {
                        Ok(output) => {
                            self.sink.emit(WorkflowEvent::StepFinished {
                                run_id: run_id.clone(),
                                step_id: step.id.clone(),
                                status: "success".to_string(),
                                output: output.clone(),
                            });
                            prior_results.push(format!("{}: {}", step.id, output));
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
                let mut tasks = Vec::with_capacity(spec.steps.len());
                let executor = &self.executor;
                let spec_ref = &spec;
                for (index, step) in spec.steps.iter().enumerate() {
                    self.sink.emit(WorkflowEvent::StepStarted {
                        run_id: run_id.clone(),
                        step_id: step.id.clone(),
                        agent: step.agent.clone(),
                    });
                    tasks.push(async move {
                        let started = std::time::Instant::now();
                        let step_result = match executor.execute_step(step, spec_ref, &[]).await {
                            Ok(output) => StepRunResult {
                                step_id: step.id.clone(),
                                agent: step.agent.clone(),
                                status: StepStatus::Success,
                                output,
                                error: None,
                                duration_ms: started.elapsed().as_millis(),
                            },
                            Err(err) => StepRunResult {
                                step_id: step.id.clone(),
                                agent: step.agent.clone(),
                                status: StepStatus::Failed,
                                output: String::new(),
                                error: Some(err.to_string()),
                                duration_ms: started.elapsed().as_millis(),
                            },
                        };
                        (index, step, step_result)
                    });
                }

                let mut completed = futures_util::future::join_all(tasks).await;
                completed.sort_by_key(|(index, _, _)| *index);

                let mut first_failure_step = None;
                for (_, step, step_result) in completed {
                    let (status, event_output) = match &step_result.status {
                        StepStatus::Success => ("success", step_result.output.clone()),
                        StepStatus::Failed => {
                            if first_failure_step.is_none() {
                                first_failure_step = Some(step.id.clone());
                            }
                            (
                                "failed",
                                step_result
                                    .error
                                    .clone()
                                    .unwrap_or_else(|| "step failed".to_string()),
                            )
                        }
                        _ => {
                            if first_failure_step.is_none() {
                                first_failure_step = Some(step.id.clone());
                            }
                            ("failed", format!("unexpected step status: {:?}", step_result.status))
                        }
                    };
                    self.sink.emit(WorkflowEvent::StepFinished {
                        run_id: run_id.clone(),
                        step_id: step.id.clone(),
                        status: status.to_string(),
                        output: event_output,
                    });
                    results.push(step_result);
                }

                if let Some(step_id) = first_failure_step {
                    let summary = format!("workflow failed at step '{}'", step_id);
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

fn ready_step_order(spec: &WorkflowSpec) -> Result<Vec<&WorkflowStep>> {
    let mut completed = HashSet::new();
    let mut ordered = Vec::with_capacity(spec.steps.len());

    while ordered.len() < spec.steps.len() {
        let next = spec.steps.iter().find(|step| {
            !completed.contains(step.id.as_str())
                && step
                    .depends_on
                    .iter()
                    .all(|dependency| completed.contains(dependency.as_str()))
        });

        let Some(step) = next else {
            anyhow::bail!("workflow contains unresolved step dependencies");
        };

        completed.insert(step.id.as_str());
        ordered.push(step);
    }

    Ok(ordered)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::events::RecordingEventSink;
    use crate::orchestrator::spec::{AgentRef, WorkflowMode, WorkflowSpec, WorkflowStep};
    use async_trait::async_trait;
    use std::sync::{atomic::{AtomicUsize, Ordering}, Arc, Mutex};
    use std::time::Duration;

    struct FakeStepExecutor;

    #[async_trait]
    impl StepExecutor for FakeStepExecutor {
        async fn execute_step(
            &self,
            step: &WorkflowStep,
            _spec: &WorkflowSpec,
            _prior_results: &[String],
        ) -> anyhow::Result<String> {
            Ok(format!("{} done", step.id))
        }
    }

    struct RecordingStepExecutor {
        calls: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait]
    impl StepExecutor for RecordingStepExecutor {
        async fn execute_step(
            &self,
            step: &WorkflowStep,
            _spec: &WorkflowSpec,
            _prior_results: &[String],
        ) -> anyhow::Result<String> {
            self.calls.lock().unwrap().push(step.id.clone());
            Ok(format!("{} done", step.id))
        }
    }

    struct PriorLengthRecordingStepExecutor {
        prior_lengths: Arc<Mutex<Vec<usize>>>,
    }

    #[async_trait]
    impl StepExecutor for PriorLengthRecordingStepExecutor {
        async fn execute_step(
            &self,
            step: &WorkflowStep,
            _spec: &WorkflowSpec,
            prior_results: &[String],
        ) -> anyhow::Result<String> {
            self.prior_lengths.lock().unwrap().push(prior_results.len());
            Ok(format!("{} done", step.id))
        }
    }

    struct ConcurrentStepExecutor {
        active: Arc<AtomicUsize>,
        max_active: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl StepExecutor for ConcurrentStepExecutor {
        async fn execute_step(
            &self,
            step: &WorkflowStep,
            _spec: &WorkflowSpec,
            _prior_results: &[String],
        ) -> anyhow::Result<String> {
            let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
            let mut observed = self.max_active.load(Ordering::SeqCst);
            while active > observed {
                match self.max_active.compare_exchange(
                    observed,
                    active,
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                ) {
                    Ok(_) => break,
                    Err(current) => observed = current,
                }
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
            self.active.fetch_sub(1, Ordering::SeqCst);
            Ok(format!("{} done", step.id))
        }
    }

    struct FailNamedStepExecutor {
        fail_step_id: String,
    }

    #[async_trait]
    impl StepExecutor for FailNamedStepExecutor {
        async fn execute_step(
            &self,
            step: &WorkflowStep,
            _spec: &WorkflowSpec,
            _prior_results: &[String],
        ) -> anyhow::Result<String> {
            if step.id == self.fail_step_id {
                Err(anyhow::anyhow!("{} failed", step.id))
            } else {
                Ok(format!("{} done", step.id))
            }
        }
    }

    struct FailingStepExecutor;

    #[async_trait]
    impl StepExecutor for FailingStepExecutor {
        async fn execute_step(
            &self,
            _step: &WorkflowStep,
            _spec: &WorkflowSpec,
            _prior_results: &[String],
        ) -> anyhow::Result<String> {
            Err(anyhow::anyhow!("executor failed"))
        }
    }

    fn step(id: &str, depends_on: Vec<&str>) -> WorkflowStep {
        WorkflowStep {
            id: id.to_string(),
            agent: "planner".to_string(),
            goal: id.to_string(),
            depends_on: depends_on.into_iter().map(str::to_string).collect(),
            expected_output: id.to_string(),
            max_retries: 0,
        }
    }

    fn spec(mode: WorkflowMode, steps: Vec<WorkflowStep>) -> WorkflowSpec {
        WorkflowSpec {
            goal: "ship".to_string(),
            mode,
            agents: vec![AgentRef {
                name: "planner".to_string(),
                model: None,
                tools: vec![],
            }],
            steps,
            termination: Default::default(),
            review: Default::default(),
            capabilities: Default::default(),
        }
    }

    #[test]
    fn builds_step_prompt_with_expected_output_and_prior_results() {
        let step = WorkflowStep {
            id: "review".to_string(),
            agent: "reviewer".to_string(),
            goal: "Review implementation".to_string(),
            depends_on: vec!["build".to_string()],
            expected_output: "approve or concrete change list".to_string(),
            max_retries: 0,
        };
        let prompt = build_step_prompt(
            &step,
            "Ship workflow runtime",
            &["build: implementation completed".to_string()],
        );
        assert!(prompt.contains("Workflow goal: Ship workflow runtime"));
        assert!(prompt.contains("Step id: review"));
        assert!(prompt.contains("Expected output: approve or concrete change list"));
        assert!(prompt.contains("build: implementation completed"));
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
    #[tokio::test]
    async fn sequential_runtime_executes_reversed_declarations_in_dependency_order() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let runtime = WorkflowRuntime::new(
            RecordingStepExecutor {
                calls: calls.clone(),
            },
            RecordingEventSink {
                events: Arc::new(Mutex::new(Vec::new())),
            },
        );
        let workflow = spec(
            WorkflowMode::Sequential,
            vec![step("b", vec!["a"]), step("a", vec![])],
        );

        runtime
            .run(workflow, &["planner".to_string()])
            .await
            .expect("workflow runs");

        assert_eq!(*calls.lock().unwrap(), vec!["a", "b"]);
    }

    #[tokio::test]
    async fn sequential_runtime_rejects_cyclic_dependencies_before_run_start() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let events = Arc::new(Mutex::new(Vec::new()));
        let runtime = WorkflowRuntime::new(
            RecordingStepExecutor {
                calls: calls.clone(),
            },
            RecordingEventSink {
                events: events.clone(),
            },
        );
        let workflow = spec(
            WorkflowMode::Sequential,
            vec![step("a", vec!["b"]), step("b", vec!["a"])],
        );

        let err = runtime
            .run(workflow, &["planner".to_string()])
            .await
            .expect_err("cyclic dependencies fail before run starts");

        assert!(
            err.to_string()
                .contains("workflow contains unresolved step dependencies")
        );
        assert!(events.lock().unwrap().is_empty());
        assert!(calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn parallel_runtime_passes_empty_prior_results_to_each_step() {
        let prior_lengths = Arc::new(Mutex::new(Vec::new()));
        let runtime = WorkflowRuntime::new(
            PriorLengthRecordingStepExecutor {
                prior_lengths: prior_lengths.clone(),
            },
            RecordingEventSink {
                events: Arc::new(Mutex::new(Vec::new())),
            },
        );

        runtime
            .run(
                spec(
                    WorkflowMode::Parallel,
                    vec![step("a", vec![]), step("b", vec![])],
                ),
                &["planner".to_string()],
            )
            .await
            .expect("workflow runs");

        assert_eq!(*prior_lengths.lock().unwrap(), vec![0, 0]);
    }

    #[tokio::test]
    async fn parallel_runtime_starts_steps_concurrently() {
        let active = Arc::new(AtomicUsize::new(0));
        let max_active = Arc::new(AtomicUsize::new(0));
        let runtime = WorkflowRuntime::new(
            ConcurrentStepExecutor {
                active,
                max_active: max_active.clone(),
            },
            RecordingEventSink {
                events: Arc::new(Mutex::new(Vec::new())),
            },
        );

        runtime
            .run(
                spec(
                    WorkflowMode::Parallel,
                    vec![step("a", vec![]), step("b", vec![])],
                ),
                &["planner".to_string()],
            )
            .await
            .expect("workflow runs");

        assert!(
            max_active.load(Ordering::SeqCst) >= 2,
            "parallel steps should overlap in execution"
        );
    }

    #[tokio::test]
    async fn parallel_runtime_reports_all_completed_results_when_one_step_fails() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let runtime = WorkflowRuntime::new(
            FailNamedStepExecutor {
                fail_step_id: "a".to_string(),
            },
            RecordingEventSink {
                events: events.clone(),
            },
        );

        let result = runtime
            .run(
                spec(
                    WorkflowMode::Parallel,
                    vec![step("a", vec![]), step("b", vec![])],
                ),
                &["planner".to_string()],
            )
            .await
            .expect("workflow returns failed result");

        assert!(matches!(result.status, WorkflowStatus::Failed));
        assert_eq!(result.steps.len(), 2);
        assert!(matches!(result.steps[0].status, StepStatus::Failed));
        assert!(matches!(result.steps[1].status, StepStatus::Success));
        let finished_steps = events
            .lock()
            .unwrap()
            .iter()
            .filter(|event| matches!(event, WorkflowEvent::StepFinished { .. }))
            .count();
        assert_eq!(finished_steps, 2);
    }

    #[tokio::test]
    async fn parallel_runtime_emits_failure_lifecycle_events() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let runtime = WorkflowRuntime::new(
            FailingStepExecutor,
            RecordingEventSink {
                events: events.clone(),
            },
        );

        let result = runtime
            .run(
                spec(WorkflowMode::Parallel, vec![step("a", vec![])]),
                &["planner".to_string()],
            )
            .await
            .expect("failed workflow returns a result");

        assert!(matches!(result.status, WorkflowStatus::Failed));
        assert_eq!(result.summary, "workflow failed at step 'a'");
        assert!(matches!(
            &events.lock().unwrap()[..],
            [
                WorkflowEvent::RunStarted { .. },
                WorkflowEvent::StepStarted { step_id, .. },
                WorkflowEvent::StepFinished { status, output, .. },
                WorkflowEvent::RunFinished { status: run_status, summary, .. },
            ] if step_id == "a"
                && status == "failed"
                && output == "executor failed"
                && run_status == "failed"
                && summary == "workflow failed at step 'a'"
        ));
    }

}
