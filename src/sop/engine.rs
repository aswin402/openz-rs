use crate::config::schema::Config;
use crate::sop::{
    load_instance, save_instance, get_definition, substitute_template,
    SopInstance, SopStatus, StepExecutionState,
};
use anyhow::Result;
use chrono::Utc;

pub async fn run_sop_instance(config: Config, instance_id: String) -> Result<()> {
    run_sop_instance_inner(config, instance_id, false).await
}

pub async fn run_sop_instance_inner(config: Config, instance_id: String, simulate: bool) -> Result<()> {
    let mut inst = load_instance(&instance_id)?;
    if inst.status == SopStatus::Completed {
        return Ok(());
    }

    inst.status = SopStatus::Running;
    save_instance(&inst)?;

    let def = get_definition(&inst.sop_id)?
        .ok_or_else(|| anyhow::anyhow!("SOP definition '{}' not found", inst.sop_id))?;

    let mut any_failed = false;
    let (tx, mut rx) = tokio::sync::mpsc::channel::<(usize, Result<crate::agent::RunResult, String>)>(100);
    let mut active_tasks = 0;

    loop {
        // 1. Identify ready steps to spawn
        let mut steps_to_spawn = Vec::new();
        if !any_failed {
            for (idx, def_step) in def.steps.iter().enumerate() {
                let current_status = &inst.steps[idx].status;
                if current_status == "Pending" {
                    // Check dependencies
                    let mut deps_satisfied = true;
                    for dep_name in &def_step.depends_on {
                        if let Some(dep_idx) = def.steps.iter().position(|s| &s.name == dep_name) {
                            if inst.steps[dep_idx].status != "Completed" {
                                deps_satisfied = false;
                                break;
                            }
                        } else {
                            eprintln!("⚠️ SOP Step '{}' depends on non-existent step '{}'", def_step.name, dep_name);
                        }
                    }
                    if deps_satisfied {
                        steps_to_spawn.push((idx, def_step.clone()));
                    }
                }
            }
        }

        // 2. Spawn ready steps
        for (idx, def_step) in steps_to_spawn {
            // Mark step as running
            {
                let step = &mut inst.steps[idx];
                step.status = "Running".to_string();
                step.started_at = Some(Utc::now().to_rfc3339());
            }
            save_instance(&inst)?;

            crate::channels::cli::send_notification(&format!(
                "📋 [SOP: {}] Spawning Step {}/{} - '{}' in parallel...",
                inst.id, idx + 1, def.steps.len(), def_step.name
            ));

            let tx = tx.clone();
            let config_clone = config.clone();
            let prompt = substitute_template(&def_step.prompt_template, &inst.context);
            let session_key = format!("sop:{}:{}", inst.id, idx);

            if simulate {
                crate::channels::cli::send_notification(&format!(
                    "🔍 [SOP: {}] Simulated Prompt for '{}':\n---\n{}\n---",
                    inst.id, def_step.name, prompt
                ));
            }

            active_tasks += 1;

            let def_step_clone = def_step.clone();
            tokio::spawn(async move {
                let run_result = if simulate {
                    // MOCK RUN: Simulate small delay (50ms)
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                    Ok(crate::agent::RunResult {
                        content: format!("[Simulated Output for Step: {}]", def_step_clone.name),
                        tools_used: Vec::new(),
                        streamed: false,
                    })
                } else {
                    match prepare_agent_and_prompt(&config_clone, &def_step_clone, &prompt).await {
                        Ok((agent_loop, final_prompt)) => agent_loop.run(&final_prompt, &session_key).await,
                        Err(e) => Err(e),
                    }
                };
                let mapped_result = match run_result {
                    Ok(res) => Ok(res),
                    Err(e) => Err(e.to_string()),
                };
                let _ = tx.send((idx, mapped_result)).await;
            });
        }

        // 3. Wait for one task to finish if active
        if active_tasks > 0 {
            if let Some((idx, res)) = rx.recv().await {
                active_tasks -= 1;
                let def_step = &def.steps[idx];
                let now_str = Utc::now().to_rfc3339();

                match res {
                    Ok(run_res) => {
                        let step = &mut inst.steps[idx];
                        step.status = "Completed".to_string();
                        step.completed_at = Some(now_str.clone());
                        step.output = Some(run_res.content.clone());

                        // Update context
                        let steps_obj = inst.context.get_mut("steps")
                            .and_then(|s| s.as_object_mut())
                            .ok_or_else(|| anyhow::anyhow!("Context 'steps' field is missing or not an object"))?;
                        steps_obj.insert(
                            def_step.name.clone(),
                            serde_json::json!({ "output": run_res.content }),
                        );

                        crate::channels::cli::send_notification(&format!(
                            "✅ [SOP: {}] Step '{}' completed!",
                            inst.id, def_step.name
                        ));
                    }
                    Err(e) => {
                        let step = &mut inst.steps[idx];
                        step.status = "Failed".to_string();
                        step.completed_at = Some(now_str.clone());
                        step.error = Some(e.clone());

                        any_failed = true;

                        crate::channels::cli::send_notification(&format!(
                            "❌ [SOP: {}] Step '{}' failed: {}",
                            inst.id, def_step.name, e
                        ));
                    }
                }
                save_instance(&inst)?;
            }
        } else {
            break;
        }
    }

    // Mark SOP instance completed/failed
    let all_completed = inst.steps.iter().all(|s| s.status == "Completed");
    if all_completed {
        inst.status = SopStatus::Completed;
        inst.completed_at = Some(Utc::now().to_rfc3339());
        save_instance(&inst)?;

        crate::channels::cli::send_notification(&format!(
            "✅ [SOP: {}] SOP completed successfully!",
            inst.id
        ));
        Ok(())
    } else {
        inst.status = SopStatus::Failed;
        save_instance(&inst)?;

        crate::channels::cli::send_notification(&format!(
            "❌ [SOP: {}] SOP failed to complete all steps.",
            inst.id
        ));
        anyhow::bail!("SOP instance failed to complete all steps")
    }
}

pub async fn trigger_sop(
    config: Config,
    sop_id: String,
    initial_payload: serde_json::Value,
) -> Result<String> {
    let def = get_definition(&sop_id)?
        .ok_or_else(|| anyhow::anyhow!("SOP definition '{}' not found", sop_id))?;

    let instance_id = format!("inst-{}", &uuid::Uuid::new_v4().to_string()[..8]);

    let steps = def.steps.iter().map(|step| StepExecutionState {
        name: step.name.clone(),
        status: "Pending".to_string(),
        started_at: None,
        completed_at: None,
        output: None,
        error: None,
    }).collect();

    let now = Utc::now().to_rfc3339();
    let inst = SopInstance {
        id: instance_id.clone(),
        sop_id: def.id,
        name: format!("{}-{}", def.name, instance_id),
        status: SopStatus::Pending,
        current_step_index: 0,
        steps,
        context: serde_json::json!({
            "payload": initial_payload,
            "steps": {}
        }),
        started_at: now,
        completed_at: None,
    };

    save_instance(&inst)?;

    // Spawn execution in background
    let config_clone = config.clone();
    let inst_id_clone = instance_id.clone();
    tokio::spawn(async move {
        let _ = run_sop_instance(config_clone, inst_id_clone).await;
    });

    Ok(instance_id)
}

pub async fn trigger_sop_simulation(
    config: Config,
    sop_id: String,
    initial_payload: serde_json::Value,
) -> Result<String> {
    let def = get_definition(&sop_id)?
        .ok_or_else(|| anyhow::anyhow!("SOP definition '{}' not found", sop_id))?;

    let instance_id = format!("sim-{}", &uuid::Uuid::new_v4().to_string()[..8]);

    let steps = def.steps.iter().map(|step| StepExecutionState {
        name: step.name.clone(),
        status: "Pending".to_string(),
        started_at: None,
        completed_at: None,
        output: None,
        error: None,
    }).collect();

    let now = Utc::now().to_rfc3339();
    let inst = SopInstance {
        id: instance_id.clone(),
        sop_id: def.id,
        name: format!("{}-{}", def.name, instance_id),
        status: SopStatus::Pending,
        current_step_index: 0,
        steps,
        context: serde_json::json!({
            "payload": initial_payload,
            "steps": {}
        }),
        started_at: now,
        completed_at: None,
    };

    save_instance(&inst)?;

    crate::channels::cli::send_notification(&format!(
        "🔬 Starting dry-run simulation for SOP '{}' (Instance ID: {})...",
        sop_id, instance_id
    ));

    // Run synchronously so the user gets simulation log immediately on console
    run_sop_instance_inner(config, instance_id.clone(), true).await?;

    Ok(instance_id)
}

pub async fn resume_sop(config: Config, instance_id: String) -> Result<()> {
    let mut inst = load_instance(&instance_id)?;
    if inst.status != SopStatus::Failed && inst.status != SopStatus::Paused {
        anyhow::bail!("Only failed or paused SOP instances can be resumed");
    }

    inst.status = SopStatus::Running;
    for step in &mut inst.steps {
        if step.status == "Failed" {
            step.status = "Pending".to_string();
            step.error = None;
        }
    }
    save_instance(&inst)?;

    let config_clone = config.clone();
    let inst_id_clone = instance_id.clone();
    tokio::spawn(async move {
        let _ = run_sop_instance(config_clone, inst_id_clone).await;
    });

    Ok(())
}

async fn prepare_agent_and_prompt(
    config: &Config,
    def_step: &crate::sop::SopStep,
    prompt: &str,
) -> Result<(crate::agent::AgentLoop, String)> {
    if let Some(ref agent_name) = def_step.agent {
        if let Ok(profiles) = crate::subagents::load_profiles() {
            if let Some(profile) = profiles.into_iter().find(|p| &p.name == agent_name) {
                let mut config_override = config.clone();
                if let Some(ref m) = profile.model {
                    config_override.agents.defaults.model = m.clone();
                }
                
                let agent_loop = crate::cli::build_agent_loop(config_override).await?;
                let subagent_prompt = format!(
                    "You are a specialized subagent operating under the following profile guidelines:\n\n\
                    {}\n\n\
                    TASK:\n{}\n\n\
                    When finished, provide a clear, concise summary of what you did and found.",
                    profile.system_prompt, prompt
                );
                return Ok((agent_loop, subagent_prompt));
            }
        }
    }
    
    let agent_loop = crate::cli::build_agent_loop(config.clone()).await?;
    Ok((agent_loop, prompt.to_string()))
}
