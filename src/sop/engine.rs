use crate::config::schema::Config;
use crate::sop::{
    get_definition, load_instance, save_instance, substitute_template, SopDefinition, SopInstance,
    SopStatus, StepExecutionState,
};
use anyhow::Result;
use chrono::Utc;

pub async fn run_sop_instance(config: Config, instance_id: String) -> Result<()> {
    run_sop_instance_inner(config, instance_id, false).await
}

fn step_dependencies_satisfied(
    def: &SopDefinition,
    inst: &SopInstance,
    def_step: &crate::sop::SopStep,
) -> bool {
    let mut satisfied = true;
    for dep_name in &def_step.depends_on {
        match def.steps.iter().position(|s| &s.name == dep_name) {
            Some(dep_idx) => match inst.steps[dep_idx].status == "Completed" {
                true => {}
                false => {
                    satisfied = false;
                    break;
                }
            },
            None => eprintln!(
                "⚠️ SOP Step '{}' depends on non-existent step '{}'",
                def_step.name, dep_name
            ),
        }
    }
    satisfied
}

pub async fn run_sop_instance_inner(
    config: Config,
    instance_id: String,
    simulate: bool,
) -> Result<()> {
    let mut inst = load_instance(&instance_id)?;
    match inst.status == SopStatus::Completed {
        true => return Ok(()),
        false => {}
    }

    inst.status = SopStatus::Running;
    save_instance(&inst)?;

    let def = get_definition(&inst.sop_id)?
        .ok_or_else(|| anyhow::anyhow!("SOP definition '{}' not found", inst.sop_id))?;

    let mut any_failed = false;
    let (tx, mut rx) =
        tokio::sync::mpsc::channel::<(usize, Result<crate::agent::RunResult, String>)>(100);
    let mut active_tasks = 0usize;

    loop {
        let mut steps_to_spawn = Vec::new();
        match any_failed {
            true => {}
            false => {
                for (idx, def_step) in def.steps.iter().enumerate() {
                    let is_ready = inst.steps[idx].status == "Pending"
                        && step_dependencies_satisfied(&def, &inst, def_step);
                    match is_ready {
                        true => steps_to_spawn.push((idx, def_step.clone())),
                        false => {}
                    }
                }
            }
        }

        for (idx, def_step) in steps_to_spawn {
            {
                let step = &mut inst.steps[idx];
                step.status = "Running".to_string();
                step.started_at = Some(Utc::now().to_rfc3339());
            }
            save_instance(&inst)?;

            crate::channels::send_notification(&format!(
                "📋 [SOP: {}] Spawning Step {}/{} - '{}' in parallel...",
                inst.id,
                idx + 1,
                def.steps.len(),
                def_step.name
            ));

            let tx = tx.clone();
            let config_clone = config.clone();
            let prompt = substitute_template(&def_step.prompt_template, &inst.context);
            let session_key = format!("sop:{}:{}", inst.id, idx);

            match simulate {
                true => crate::channels::send_notification(&format!(
                    "🔍 [SOP: {}] Simulated Prompt for '{}':\n---\n{}\n---",
                    inst.id, def_step.name, prompt
                )),
                false => {}
            }

            active_tasks += 1;

            let def_step_clone = def_step.clone();
            tokio::spawn(async move {
                let run_result = match simulate {
                    true => {
                        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                        Ok(crate::agent::RunResult {
                            content: format!(
                                "[Simulated Output for Step: {}]",
                                def_step_clone.name
                            ),
                            tools_used: Vec::new(),
                            streamed: false,
                        })
                    }
                    false => {
                        match prepare_agent_and_prompt(&config_clone, &def_step_clone, &prompt)
                            .await
                        {
                            Ok((agent_loop, final_prompt)) => {
                                agent_loop.run(&final_prompt, &session_key).await
                            }
                            Err(e) => Err(e),
                        }
                    }
                };
                let mapped_result = match run_result {
                    Ok(res) => Ok(res),
                    Err(e) => Err(e.to_string()),
                };
                let _ = tx.send((idx, mapped_result)).await;
            });
        }

        match active_tasks > 0 {
            false => break,
            true => match rx.recv().await {
                Some((idx, res)) => {
                    active_tasks = active_tasks.saturating_sub(1);
                    let def_step = &def.steps[idx];
                    let now_str = Utc::now().to_rfc3339();

                    match res {
                        Ok(run_res) => {
                            let step = &mut inst.steps[idx];
                            step.status = "Completed".to_string();
                            step.completed_at = Some(now_str);
                            step.output = Some(run_res.content.clone());

                            let steps_obj = inst
                                .context
                                .get_mut("steps")
                                .and_then(|s| s.as_object_mut())
                                .ok_or_else(|| {
                                    anyhow::anyhow!(
                                        "Context 'steps' field is missing or not an object"
                                    )
                                })?;
                            steps_obj.insert(
                                def_step.name.clone(),
                                serde_json::json!({ "output": run_res.content }),
                            );

                            crate::channels::send_notification(&format!(
                                "✅ [SOP: {}] Step '{}' completed!",
                                inst.id, def_step.name
                            ));
                            inst.current_step_index = inst
                                .steps
                                .iter()
                                .filter(|s| s.status == "Completed")
                                .count();
                        }
                        Err(e) => {
                            let step = &mut inst.steps[idx];
                            step.status = "Failed".to_string();
                            step.completed_at = Some(now_str);
                            step.error = Some(e.clone());

                            any_failed = true;

                            crate::channels::send_notification(&format!(
                                "❌ [SOP: {}] Step '{}' failed: {}",
                                inst.id, def_step.name, e
                            ));
                        }
                    }
                    save_instance(&inst)?;
                }
                None => break,
            },
        }
    }

    let all_completed = inst.steps.iter().all(|s| s.status == "Completed");
    match all_completed {
        true => {
            inst.status = SopStatus::Completed;
            inst.current_step_index = inst.steps.len();
            inst.completed_at = Some(Utc::now().to_rfc3339());
            save_instance(&inst)?;

            crate::channels::send_notification(&format!(
                "✅ [SOP: {}] SOP completed successfully!",
                inst.id
            ));
            Ok(())
        }
        false => {
            inst.status = SopStatus::Failed;
            save_instance(&inst)?;

            crate::channels::send_notification(&format!(
                "❌ [SOP: {}] SOP failed to complete all steps.",
                inst.id
            ));
            anyhow::bail!("SOP instance failed to complete all steps")
        }
    }
}

fn create_sop_instance(
    sop_id: &str,
    prefix: &str,
    initial_payload: serde_json::Value,
) -> Result<SopInstance> {
    let def = get_definition(sop_id)?
        .ok_or_else(|| anyhow::anyhow!("SOP definition '{}' not found", sop_id))?;

    let instance_id = format!("{prefix}-{}", &uuid::Uuid::new_v4().to_string()[..8]);
    let steps = def
        .steps
        .iter()
        .map(|step| StepExecutionState {
            name: step.name.clone(),
            status: "Pending".to_string(),
            started_at: None,
            completed_at: None,
            output: None,
            error: None,
        })
        .collect();

    let inst = SopInstance {
        id: instance_id.clone(),
        sop_id: def.id,
        name: format!("{}-{instance_id}", def.name),
        status: SopStatus::Pending,
        current_step_index: 0,
        steps,
        context: serde_json::json!({
            "payload": initial_payload,
            "steps": {}
        }),
        started_at: Utc::now().to_rfc3339(),
        completed_at: None,
    };

    save_instance(&inst)?;
    Ok(inst)
}

pub async fn trigger_sop(
    config: Config,
    sop_id: String,
    initial_payload: serde_json::Value,
) -> Result<String> {
    trigger_sop_with_options(config, sop_id, initial_payload, false).await
}

pub async fn trigger_sop_with_options(
    config: Config,
    sop_id: String,
    initial_payload: serde_json::Value,
    wait: bool,
) -> Result<String> {
    let inst = create_sop_instance(&sop_id, "inst", initial_payload)?;
    let instance_id = inst.id;

    let config_clone = config.clone();
    let inst_id_clone = instance_id.clone();
    match wait {
        true => run_sop_instance(config_clone, inst_id_clone).await?,
        false => {
            tokio::spawn(async move {
                match run_sop_instance(config_clone, inst_id_clone).await {
                    Ok(()) => {}
                    Err(e) => tracing::error!("SOP background execution failed: {:?}", e),
                }
            });
        }
    }

    Ok(instance_id)
}

pub async fn trigger_sop_simulation(
    config: Config,
    sop_id: String,
    initial_payload: serde_json::Value,
) -> Result<String> {
    let inst = create_sop_instance(&sop_id, "sim", initial_payload)?;
    let instance_id = inst.id;

    crate::channels::send_notification(&format!(
        "🔬 Starting dry-run simulation for SOP '{}' (Instance ID: {})...",
        sop_id, instance_id
    ));

    run_sop_instance_inner(config, instance_id.clone(), true).await?;

    Ok(instance_id)
}

pub async fn resume_sop(config: Config, instance_id: String) -> Result<()> {
    resume_sop_with_options(config, instance_id, false).await
}

pub async fn resume_sop_with_options(
    config: Config,
    instance_id: String,
    wait: bool,
) -> Result<()> {
    let mut inst = load_instance(&instance_id)?;
    let can_resume = matches!(
        inst.status,
        SopStatus::Failed | SopStatus::Paused | SopStatus::Pending
    );
    match can_resume {
        false => anyhow::bail!("Only pending, failed, or paused SOP instances can be resumed"),
        true => {}
    }

    inst.status = SopStatus::Running;
    for step in &mut inst.steps {
        match step.status == "Failed" {
            true => {
                step.status = "Pending".to_string();
                step.error = None;
            }
            false => {}
        }
    }
    save_instance(&inst)?;

    let config_clone = config.clone();
    let inst_id_clone = instance_id;
    match wait {
        true => run_sop_instance(config_clone, inst_id_clone).await?,
        false => {
            tokio::spawn(async move {
                match run_sop_instance(config_clone, inst_id_clone).await {
                    Ok(()) => {}
                    Err(e) => tracing::error!("SOP background resume failed: {:?}", e),
                }
            });
        }
    }

    Ok(())
}

async fn prepare_agent_and_prompt(
    config: &Config,
    def_step: &crate::sop::SopStep,
    prompt: &str,
) -> Result<(crate::agent::AgentLoop, String)> {
    let matched_profile = def_step
        .agent
        .as_ref()
        .and_then(|agent_name| {
            crate::subagents::load_profiles()
                .ok()
                .and_then(|profiles| profiles.into_iter().find(|p| &p.name == agent_name))
        });

    match matched_profile {
        Some(profile) => {
            let mut config_override = config.clone();
            match profile.model {
                Some(m) => config_override.agents.defaults.model = m,
                None => {}
            }

            let agent_loop = crate::cli::build_agent_loop(config_override).await?;
            let subagent_prompt = format!(
                "You are a specialized subagent operating under the following profile guidelines:\n\n\
                {}\n\n\
                TASK:\n{}\n\n\
                When finished, provide a clear, concise summary of what you did and found.",
                profile.system_prompt, prompt
            );
            Ok((agent_loop, subagent_prompt))
        }
        None => {
            let agent_loop = crate::cli::build_agent_loop(config.clone()).await?;
            Ok((agent_loop, prompt.to_string()))
        }
    }
}
