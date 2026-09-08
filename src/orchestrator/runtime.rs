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
        "Complete the step directly when possible. Do not delegate or research for trivial/general-knowledge tasks. Use web only if required by the step. Use nested delegation only when the step explicitly asks for delegation or clearly needs another specialist."
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
                let concurrency_limit = spec.steps.len().clamp(1, 4);
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
#[path = "runtime_tests.rs"]
mod tests;
