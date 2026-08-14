use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashSet;
use uuid::Uuid;

use super::events::{WorkflowEvent, WorkflowEventSink};
use super::result::{StepRunResult, StepStatus, WorkflowRunResult, WorkflowStatus};
use super::spec::{TerminationPolicy, WorkflowMode, WorkflowSpec, WorkflowStep};
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
    let policy = crate::grounding::step_execution_policy(workflow_goal, &step.goal, &step.agent);
    let grounding_guidance = if policy.require_sources {
        "Use web/docs/local tools when required for current, external, source-specific, uncertain, or high-stakes facts. Cite or name sources when used. If sources are unavailable or weak, say verification is incomplete."
    } else if policy.allow_nested_delegation {
        "Complete this step directly when possible, but nested delegation is permitted because this step explicitly asks for complex, delegated, research, scan, implementation, refactor, debug, or multi-source work."
    } else {
        "Complete this step directly when possible. Do not delegate or research for trivial/general-knowledge work unless the step explicitly asks for research, current facts, source-specific facts, or multi-source analysis."
    };

    format!(
        "Workflow goal: {workflow_goal}

Step id: {id}
Assigned agent: {agent}
Step goal: {goal}
Expected output: {expected}

Grounding guidance: {grounding_guidance}
Nested delegation allowed: {nested}
Live sources required: {sources}

Prior step results:
{prior}

Return only the requested deliverable plus real blockers.",
        id = step.id,
        agent = step.agent,
        goal = step.goal,
        expected = step.expected_output,
        nested = policy.allow_nested_delegation,
        sources = policy.require_sources,
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

    async fn run_step(
        &self,
        run_id: &str,
        spec: &WorkflowSpec,
        step: &WorkflowStep,
        prior_results: &[String],
    ) -> StepRunResult {
        self.sink.emit(WorkflowEvent::StepStarted {
            run_id: run_id.to_string(),
            step_id: step.id.clone(),
            agent: step.agent.clone(),
        });
        let started = std::time::Instant::now();
        match self.executor.execute_step(step, spec, prior_results).await {
            Ok(output) => {
                self.sink.emit(WorkflowEvent::StepFinished {
                    run_id: run_id.to_string(),
                    step_id: step.id.clone(),
                    status: "success".to_string(),
                    output: output.clone(),
                });
                StepRunResult {
                    step_id: step.id.clone(),
                    agent: step.agent.clone(),
                    status: StepStatus::Success,
                    output,
                    error: None,
                    duration_ms: started.elapsed().as_millis(),
                }
            }
            Err(err) => {
                let error = err.to_string();
                self.sink.emit(WorkflowEvent::StepFinished {
                    run_id: run_id.to_string(),
                    step_id: step.id.clone(),
                    status: "failed".to_string(),
                    output: error.clone(),
                });
                StepRunResult {
                    step_id: step.id.clone(),
                    agent: step.agent.clone(),
                    status: StepStatus::Failed,
                    output: String::new(),
                    error: Some(error),
                    duration_ms: started.elapsed().as_millis(),
                }
            }
        }
    }

    pub async fn run(
        &self,
        spec: WorkflowSpec,
        known_agents: &[String],
    ) -> Result<WorkflowRunResult> {
        validate_workflow_spec(&spec, known_agents)?;
        let ordered_steps = if matches!(
            &spec.mode,
            WorkflowMode::Sequential
                | WorkflowMode::Graph
                | WorkflowMode::ManagerWorker
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
            WorkflowMode::Sequential | WorkflowMode::Graph | WorkflowMode::ManagerWorker => {
                for step in
                    ordered_steps.expect("dependency-aware modes are ordered before run start")
                {
                    self.sink.emit(WorkflowEvent::StepStarted {
                        run_id: run_id.clone(),
                        step_id: step.id.clone(),
                        agent: step.agent.clone(),
                    });
                    let started = std::time::Instant::now();
                    match self
                        .executor
                        .execute_step(step, &spec, &prior_results)
                        .await
                    {
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
            WorkflowMode::SelectorGroup => {
                let group_agents = if spec.agents.is_empty() {
                    known_agents.to_vec()
                } else {
                    spec.agents
                        .iter()
                        .map(|agent| agent.name.clone())
                        .collect::<Vec<_>>()
                };
                let mut last_speaker: Option<String> = None;
                let mut next_agent_index = 0usize;
                for step in
                    ordered_steps.expect("selector group steps are ordered before run start")
                {
                    let candidates = selector_candidates(&group_agents, last_speaker.as_deref());
                    let selected_agent = if group_agents.is_empty() {
                        step.agent.clone()
                    } else {
                        let mut selected = None;
                        for offset in 0..group_agents.len() {
                            let index = (next_agent_index + offset) % group_agents.len();
                            let agent = &group_agents[index];
                            if candidates.iter().any(|candidate| candidate == agent) {
                                selected = Some((index, agent.clone()));
                                break;
                            }
                        }
                        let (index, agent) = selected.unwrap_or_else(|| {
                            let index = next_agent_index % group_agents.len();
                            (index, group_agents[index].clone())
                        });
                        next_agent_index = (index + 1) % group_agents.len();
                        agent
                    };
                    let mut selected_step = step.clone();
                    selected_step.agent = selected_agent;
                    let step_result = self
                        .run_step(&run_id, &spec, &selected_step, &prior_results)
                        .await;
                    if !matches!(step_result.status, StepStatus::Success) {
                        let summary = format!("workflow failed at step '{}'", step.id);
                        results.push(step_result);
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
                    prior_results.push(format!("{}: {}", step.id, step_result.output));
                    last_speaker = Some(step_result.agent.clone());
                    results.push(step_result);
                }
            }
            WorkflowMode::ReviewLoop => {
                let ordered_steps = ready_step_order(&spec)?;
                let reviewer_name = spec.review.reviewer.as_deref().unwrap_or("").trim();
                let reviewer_index = if !reviewer_name.is_empty() {
                    ordered_steps
                        .iter()
                        .position(|step| step.agent == reviewer_name)
                        .ok_or_else(|| {
                            anyhow::anyhow!(
                                "review loop reviewer '{}' has no matching workflow step",
                                reviewer_name
                            )
                        })?
                } else {
                    ordered_steps
                        .iter()
                        .position(|step| step.id.eq_ignore_ascii_case("review"))
                        .ok_or_else(|| {
                            anyhow::anyhow!("review loop needs a review step or review.reviewer")
                        })?
                };
                let reviewer_step = ordered_steps[reviewer_index];
                let implementation_steps = ordered_steps
                    .iter()
                    .enumerate()
                    .take_while(|(index, _)| *index < reviewer_index)
                    .map(|(_, step)| *step)
                    .collect::<Vec<_>>();
                let post_approval_steps = ordered_steps
                    .iter()
                    .enumerate()
                    .filter_map(|(index, step)| (index > reviewer_index).then_some(*step))
                    .collect::<Vec<_>>();
                let max_rounds = spec.termination.max_rounds.max(1);

                for round in 1..=max_rounds {
                    let round_note = format!("review round {round}/{max_rounds}");
                    for step in &implementation_steps {
                        let step_result = self.run_step(&run_id, &spec, step, &prior_results).await;
                        if !matches!(step_result.status, StepStatus::Success) {
                            let summary = format!("workflow failed at step '{}'", step.id);
                            results.push(step_result);
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
                        prior_results.push(format!(
                            "{} {}: {}",
                            round_note, step.id, step_result.output
                        ));
                        results.push(step_result);
                    }

                    let review_result = self
                        .run_step(&run_id, &spec, reviewer_step, &prior_results)
                        .await;
                    if !matches!(review_result.status, StepStatus::Success) {
                        let summary = format!("workflow failed at step '{}'", reviewer_step.id);
                        results.push(review_result);
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

                    let termination_status =
                        output_satisfies_termination(&review_result.output, &spec.termination);
                    prior_results.push(format!(
                        "{} {}: {}",
                        round_note, reviewer_step.id, review_result.output
                    ));
                    results.push(review_result);

                    match termination_status {
                        Some(WorkflowStatus::Success) => {
                            for step in &post_approval_steps {
                                let step_result =
                                    self.run_step(&run_id, &spec, step, &prior_results).await;
                                if !matches!(step_result.status, StepStatus::Success) {
                                    let summary = format!("workflow failed at step '{}'", step.id);
                                    results.push(step_result);
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
                                prior_results.push(format!("{}: {}", step.id, step_result.output));
                                results.push(step_result);
                            }

                            let summary = format!("review loop approved after {round} round(s)");
                            self.sink.emit(WorkflowEvent::RunFinished {
                                run_id: run_id.clone(),
                                status: "success".to_string(),
                                summary: summary.clone(),
                            });
                            return Ok(WorkflowRunResult {
                                run_id,
                                status: WorkflowStatus::Success,
                                summary,
                                steps: results,
                                sources: vec![],
                            });
                        }
                        Some(WorkflowStatus::Failed) => {
                            let summary = format!("review loop failed after {round} round(s)");
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
                        _ => {}
                    }
                }

                let summary = "review loop reached max_rounds".to_string();
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
            WorkflowMode::Parallel => {
                let executor = &self.executor;
                let spec_ref = &spec;
                let concurrency_limit = spec.steps.len().min(4).max(1);
                let indexed_steps = spec.steps.iter().enumerate().collect::<Vec<_>>();
                let mut completed = Vec::with_capacity(spec.steps.len());
                for chunk in indexed_steps.chunks(concurrency_limit) {
                    let mut batch = Vec::with_capacity(chunk.len());
                    for &(index, step) in chunk {
                        self.sink.emit(WorkflowEvent::StepStarted {
                            run_id: run_id.clone(),
                            step_id: step.id.clone(),
                            agent: step.agent.clone(),
                        });
                        batch.push(async move {
                            let started = std::time::Instant::now();
                            let step_result = match executor.execute_step(step, spec_ref, &[]).await
                            {
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
                    completed.extend(futures_util::future::join_all(batch).await);
                }
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
                            (
                                "failed",
                                format!("unexpected step status: {:?}", step_result.status),
                            )
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

pub fn selector_candidates(agents: &[String], last_speaker: Option<&str>) -> Vec<String> {
    let filtered = agents
        .iter()
        .filter(|agent| Some(agent.as_str()) != last_speaker)
        .cloned()
        .collect::<Vec<_>>();
    if filtered.is_empty() {
        agents.to_vec()
    } else {
        filtered
    }
}

pub fn output_satisfies_termination(
    output: &str,
    policy: &TerminationPolicy,
) -> Option<WorkflowStatus> {
    if let Some(keyword) = policy
        .failure_keyword
        .as_deref()
        .map(str::trim)
        .filter(|keyword| !keyword.is_empty())
    {
        if output.contains(keyword) {
            return Some(WorkflowStatus::Failed);
        }
    }
    if let Some(keyword) = policy
        .success_keyword
        .as_deref()
        .map(str::trim)
        .filter(|keyword| !keyword.is_empty())
    {
        if output.contains(keyword) {
            return Some(WorkflowStatus::Success);
        }
    }
    None
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
    use crate::orchestrator::events::{NoopEventSink, RecordingEventSink};
    use crate::orchestrator::spec::{AgentRef, WorkflowMode, WorkflowSpec, WorkflowStep};
    use async_trait::async_trait;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    };
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

    #[derive(Clone, Default)]
    struct CountingExecutor {
        count: Arc<AtomicUsize>,
    }

    impl CountingExecutor {
        fn count(&self) -> usize {
            self.count.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl StepExecutor for CountingExecutor {
        async fn execute_step(
            &self,
            step: &WorkflowStep,
            _spec: &WorkflowSpec,
            _prior_results: &[String],
        ) -> anyhow::Result<String> {
            self.count.fetch_add(1, Ordering::SeqCst);
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

    struct ScriptedExecutor {
        outputs: Arc<Mutex<Vec<String>>>,
        calls: Arc<Mutex<Vec<String>>>,
    }

    impl ScriptedExecutor {
        fn new(outputs: Vec<String>) -> Self {
            Self {
                outputs: Arc::new(Mutex::new(outputs)),
                calls: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn calls(&self) -> Arc<Mutex<Vec<String>>> {
            self.calls.clone()
        }
    }

    #[async_trait]
    impl StepExecutor for ScriptedExecutor {
        async fn execute_step(
            &self,
            step: &WorkflowStep,
            _spec: &WorkflowSpec,
            _prior_results: &[String],
        ) -> anyhow::Result<String> {
            self.calls.lock().unwrap().push(step.id.clone());
            if step.id == "review" {
                let mut outputs = self.outputs.lock().unwrap();
                if outputs.is_empty() {
                    anyhow::bail!("scripted executor has no output left");
                }
                Ok(outputs.remove(0))
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

    fn parallel_spec_with_two_steps() -> WorkflowSpec {
        WorkflowSpec {
            goal: "compare".to_string(),
            mode: WorkflowMode::Parallel,
            agents: vec![
                AgentRef {
                    name: "researcher".to_string(),
                    model: None,
                    tools: vec![],
                },
                AgentRef {
                    name: "reviewer".to_string(),
                    model: None,
                    tools: vec![],
                },
            ],
            steps: vec![
                WorkflowStep {
                    id: "research".to_string(),
                    agent: "researcher".to_string(),
                    goal: "Research".to_string(),
                    depends_on: vec![],
                    expected_output: "notes".to_string(),
                    max_retries: 0,
                },
                WorkflowStep {
                    id: "review".to_string(),
                    agent: "reviewer".to_string(),
                    goal: "Review".to_string(),
                    depends_on: vec![],
                    expected_output: "review".to_string(),
                    max_retries: 0,
                },
            ],
            termination: Default::default(),
            review: Default::default(),
            capabilities: Default::default(),
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

    #[test]
    fn step_prompt_instructs_direct_completion_for_trivial_steps() {
        let step = WorkflowStep {
            id: "plan".to_string(),
            agent: "planner".to_string(),
            goal: "Summarize hello".to_string(),
            depends_on: vec![],
            expected_output: "short summary".to_string(),
            max_retries: 0,
        };

        let prompt = build_step_prompt(&step, "Run smoke test workflow", &[]);

        assert!(prompt.contains("Complete this step directly when possible"));
        assert!(prompt.contains("Do not delegate"));
        assert!(prompt.contains("Do not research"));
    }

    #[test]
    fn step_prompt_allows_research_for_current_external_steps() {
        let step = WorkflowStep {
            id: "latest".to_string(),
            agent: "researcher".to_string(),
            goal: "Find the latest stable Rust version today".to_string(),
            depends_on: vec![],
            expected_output: "version with source".to_string(),
            max_retries: 0,
        };

        let prompt = build_step_prompt(&step, "Answer current software version question", &[]);

        assert!(prompt.contains("Use web/docs/local tools when required"));
        assert!(prompt.contains("source-specific"));
        assert!(prompt.contains("verification is incomplete"));
    }

    #[test]
    fn step_prompt_allows_delegation_when_policy_allows_it() {
        let step = WorkflowStep {
            id: "debug".to_string(),
            agent: "planner".to_string(),
            goal: "Debug the repo-wide workflow routing issue".to_string(),
            depends_on: vec![],
            expected_output: "root cause and fix plan".to_string(),
            max_retries: 0,
        };

        let prompt = build_step_prompt(&step, "Debug orchestration", &[]);

        assert!(prompt.contains("nested delegation is permitted"));
        assert!(prompt.contains("Nested delegation allowed: true"));
    }

    #[tokio::test]
    async fn sequential_runtime_executes_steps_in_dependency_order() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink = RecordingEventSink {
            events: events.clone(),
        };
        let runtime = WorkflowRuntime::new(FakeStepExecutor, sink);
        let spec = WorkflowSpec {
            goal: "ship".to_string(),
            mode: WorkflowMode::Sequential,
            agents: vec![AgentRef {
                name: "planner".to_string(),
                model: None,
                tools: vec![],
            }],
            steps: vec![
                WorkflowStep {
                    id: "a".to_string(),
                    agent: "planner".to_string(),
                    goal: "A".to_string(),
                    depends_on: vec![],
                    expected_output: "A".to_string(),
                    max_retries: 0,
                },
                WorkflowStep {
                    id: "b".to_string(),
                    agent: "planner".to_string(),
                    goal: "B".to_string(),
                    depends_on: vec!["a".to_string()],
                    expected_output: "B".to_string(),
                    max_retries: 0,
                },
            ],
            termination: Default::default(),
            review: Default::default(),
            capabilities: Default::default(),
        };

        let result = runtime
            .run(spec, &["planner".to_string()])
            .await
            .expect("workflow runs");
        assert_eq!(result.steps.len(), 2);
        assert_eq!(result.summary, "2 step(s) completed");
        assert!(matches!(
            result.status,
            crate::orchestrator::result::WorkflowStatus::Success
        ));
        assert!(events
            .lock()
            .unwrap()
            .iter()
            .any(|event| matches!(event, WorkflowEvent::RunStarted { .. })));
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

        assert!(err
            .to_string()
            .contains("workflow contains unresolved step dependencies"));
        assert!(events.lock().unwrap().is_empty());
        assert!(calls.lock().unwrap().is_empty());
    }

    fn review_loop_spec_with_success_keyword(
        success_keyword: &str,
        max_rounds: usize,
    ) -> WorkflowSpec {
        let mut workflow = spec(
            WorkflowMode::ReviewLoop,
            vec![step("implement", vec![]), step("review", vec!["implement"])],
        );
        workflow.agents.push(AgentRef {
            name: "reviewer".to_string(),
            model: None,
            tools: vec![],
        });
        workflow.steps[1].agent = "reviewer".to_string();
        workflow.termination.max_rounds = max_rounds;
        workflow.termination.success_keyword = Some(success_keyword.to_string());
        workflow.review.mode = crate::orchestrator::spec::ReviewMode::Required;
        workflow.review.reviewer = Some("reviewer".to_string());
        workflow
    }

    #[tokio::test]
    async fn review_loop_retries_until_approval_keyword() {
        let executor =
            ScriptedExecutor::new(vec!["needs changes".to_string(), "APPROVE".to_string()]);
        let runtime = WorkflowRuntime::new(executor, NoopEventSink);
        let spec = review_loop_spec_with_success_keyword("APPROVE", 3);

        let result = runtime
            .run(spec, &["planner".to_string(), "reviewer".to_string()])
            .await
            .unwrap();

        assert!(matches!(result.status, WorkflowStatus::Success));
        assert_eq!(result.steps.len(), 4);
        assert_eq!(result.summary, "review loop approved after 2 round(s)");
    }

    #[tokio::test]
    async fn review_loop_failure_keyword_stops_immediately() {
        let executor = ScriptedExecutor::new(vec!["BLOCKED".to_string()]);
        let runtime = WorkflowRuntime::new(executor, NoopEventSink);
        let mut spec = review_loop_spec_with_success_keyword("APPROVE", 3);
        spec.termination.failure_keyword = Some("BLOCKED".to_string());

        let result = runtime
            .run(spec, &["planner".to_string(), "reviewer".to_string()])
            .await
            .unwrap();

        assert!(matches!(result.status, WorkflowStatus::Failed));
        assert_eq!(result.steps.len(), 2);
        assert_eq!(result.summary, "review loop failed after 1 round(s)");
    }

    #[tokio::test]
    async fn review_loop_fails_when_max_rounds_reached() {
        let executor = ScriptedExecutor::new(vec![
            "needs changes".to_string(),
            "still needs changes".to_string(),
        ]);
        let runtime = WorkflowRuntime::new(executor, NoopEventSink);
        let spec = review_loop_spec_with_success_keyword("APPROVE", 2);

        let result = runtime
            .run(spec, &["planner".to_string(), "reviewer".to_string()])
            .await
            .unwrap();

        assert!(matches!(result.status, WorkflowStatus::Failed));
        assert_eq!(result.steps.len(), 4);
        assert_eq!(result.summary, "review loop reached max_rounds");
    }

    #[tokio::test]
    async fn review_loop_runs_post_review_steps_only_after_approval() {
        let executor =
            ScriptedExecutor::new(vec!["needs changes".to_string(), "APPROVE".to_string()]);
        let calls = executor.calls();
        let runtime = WorkflowRuntime::new(executor, NoopEventSink);
        let mut workflow = review_loop_spec_with_success_keyword("APPROVE", 3);
        workflow.steps.push(WorkflowStep {
            id: "publish".to_string(),
            agent: "planner".to_string(),
            goal: "Publish after approval".to_string(),
            depends_on: vec!["review".to_string()],
            expected_output: "published".to_string(),
            max_retries: 0,
        });

        let result = runtime
            .run(workflow, &["planner".to_string(), "reviewer".to_string()])
            .await
            .unwrap();

        assert!(matches!(result.status, WorkflowStatus::Success));
        assert_eq!(
            *calls.lock().unwrap(),
            vec!["implement", "review", "implement", "review", "publish"]
        );
    }

    #[tokio::test]
    async fn review_loop_rejects_missing_configured_reviewer_step() {
        let executor = ScriptedExecutor::new(vec!["APPROVE".to_string()]);
        let runtime = WorkflowRuntime::new(executor, NoopEventSink);
        let mut workflow = spec(WorkflowMode::ReviewLoop, vec![step("implement", vec![])]);
        workflow.agents.push(AgentRef {
            name: "reviewer".to_string(),
            model: None,
            tools: vec![],
        });
        workflow.review.mode = crate::orchestrator::spec::ReviewMode::Required;
        workflow.review.reviewer = Some("reviewer".to_string());
        workflow.termination.success_keyword = Some("APPROVE".to_string());

        let err = runtime
            .run(workflow, &["planner".to_string(), "reviewer".to_string()])
            .await
            .expect_err("configured reviewer must match a workflow step");

        assert!(err
            .to_string()
            .contains("review loop reviewer 'reviewer' has no matching workflow step"));
    }

    #[test]
    fn blank_termination_keywords_do_not_match() {
        let policy = TerminationPolicy {
            max_rounds: 1,
            success_keyword: Some("".to_string()),
            failure_keyword: Some("   ".to_string()),
        };

        assert!(output_satisfies_termination("any output", &policy).is_none());
    }

    #[tokio::test]
    async fn selector_group_uses_deterministic_round_robin() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let runtime = WorkflowRuntime::new(
            RecordingStepExecutor {
                calls: calls.clone(),
            },
            NoopEventSink,
        );
        let mut workflow = spec(
            WorkflowMode::SelectorGroup,
            vec![step("first", vec![]), step("second", vec!["first"])],
        );
        workflow.agents.push(AgentRef {
            name: "researcher".to_string(),
            model: None,
            tools: vec![],
        });
        workflow.agents.push(AgentRef {
            name: "reviewer".to_string(),
            model: None,
            tools: vec![],
        });
        workflow.steps.push(WorkflowStep {
            id: "third".to_string(),
            agent: "planner".to_string(),
            goal: "third".to_string(),
            depends_on: vec!["second".to_string()],
            expected_output: "third".to_string(),
            max_retries: 0,
        });

        let result = runtime
            .run(workflow, &["planner".to_string(), "researcher".to_string()])
            .await
            .unwrap();

        assert!(matches!(result.status, WorkflowStatus::Success));
        assert_eq!(*calls.lock().unwrap(), vec!["first", "second", "third"]);
        assert_eq!(
            result
                .steps
                .iter()
                .map(|step| step.agent.as_str())
                .collect::<Vec<_>>(),
            vec!["planner", "researcher", "reviewer"]
        );
    }

    #[tokio::test]
    async fn parallel_mode_runs_independent_steps() {
        let executor = CountingExecutor::default();
        let runtime = WorkflowRuntime::new(executor.clone(), NoopEventSink);
        let spec = parallel_spec_with_two_steps();

        let result = runtime
            .run(spec, &["researcher".to_string(), "reviewer".to_string()])
            .await
            .unwrap();

        assert!(matches!(result.status, WorkflowStatus::Success));
        assert_eq!(result.steps.len(), 2);
        assert_eq!(executor.count(), 2);
    }

    #[tokio::test]
    async fn parallel_mode_caps_concurrency_at_four() {
        let active = Arc::new(AtomicUsize::new(0));
        let max_active = Arc::new(AtomicUsize::new(0));
        let runtime = WorkflowRuntime::new(
            ConcurrentStepExecutor {
                active,
                max_active: max_active.clone(),
            },
            NoopEventSink,
        );
        let steps = (0..6)
            .map(|index| step(&format!("step-{index}"), vec![]))
            .collect::<Vec<_>>();

        runtime
            .run(
                spec(WorkflowMode::Parallel, steps),
                &["planner".to_string()],
            )
            .await
            .expect("workflow runs");

        assert_eq!(max_active.load(Ordering::SeqCst), 4);
    }

    #[test]
    fn selector_candidates_exclude_last_speaker_when_possible() {
        let candidates = selector_candidates(
            &["planner".to_string(), "researcher".to_string()],
            Some("planner"),
        );
        assert_eq!(candidates, vec!["researcher".to_string()]);
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
