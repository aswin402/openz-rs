use crate::config::schema::Config;
use crate::sop::{
    load_instance, save_instance, get_definition, substitute_template,
    SopInstance, SopStatus, StepExecutionState,
};
use anyhow::Result;
use chrono::Utc;

pub async fn run_sop_instance(config: Config, instance_id: String) -> Result<()> {
    let mut inst = load_instance(&instance_id)?;
    if inst.status == SopStatus::Completed {
        return Ok(());
    }

    inst.status = SopStatus::Running;
    save_instance(&inst)?;

    let def = get_definition(&inst.sop_id)?
        .ok_or_else(|| anyhow::anyhow!("SOP definition '{}' not found", inst.sop_id))?;

    while inst.current_step_index < def.steps.len() {
        let idx = inst.current_step_index;
        let def_step = &def.steps[idx];

        // 1. Mark step as running
        {
            let step = &mut inst.steps[idx];
            step.status = "Running".to_string();
            step.started_at = Some(Utc::now().to_rfc3339());
        }
        save_instance(&inst)?;

        // 2. Prepare prompt template
        let prompt = substitute_template(&def_step.prompt_template, &inst.context);

        crate::channels::cli::send_notification(&format!(
            "📋 [SOP: {}] Executing Step {}/{} - '{}'...",
            inst.id, idx + 1, def.steps.len(), def_step.name
        ));

        // 3. Build Agent and run the step
        let agent_loop = crate::cli::build_agent_loop(config.clone()).await?;
        let session_key = format!("sop:{}:{}", inst.id, idx);
        
        let run_result = agent_loop.run(&prompt, &session_key).await;

        // 4. Update state based on execution result
        match run_result {
            Ok(res) => {
                let now_str = Utc::now().to_rfc3339();
                let step = &mut inst.steps[idx];
                step.status = "Completed".to_string();
                step.completed_at = Some(now_str.clone());
                step.output = Some(res.content.clone());

                // Update context
                let steps_obj = inst.context.get_mut("steps")
                    .and_then(|s| s.as_object_mut())
                    .expect("Context steps should be an object");
                steps_obj.insert(
                    def_step.name.clone(),
                    serde_json::json!({ "output": res.content }),
                );

                inst.current_step_index += 1;
                save_instance(&inst)?;
            }
            Err(e) => {
                let now_str = Utc::now().to_rfc3339();
                let step = &mut inst.steps[idx];
                step.status = "Failed".to_string();
                step.completed_at = Some(now_str.clone());
                step.error = Some(e.to_string());

                inst.status = SopStatus::Failed;
                save_instance(&inst)?;

                crate::channels::cli::send_notification(&format!(
                    "❌ [SOP: {}] Step '{}' failed: {}",
                    inst.id, def_step.name, e
                ));
                return Err(e);
            }
        }
    }

    // Mark SOP instance completed
    inst.status = SopStatus::Completed;
    inst.completed_at = Some(Utc::now().to_rfc3339());
    save_instance(&inst)?;

    crate::channels::cli::send_notification(&format!(
        "✅ [SOP: {}] SOP completed successfully!",
        inst.id
    ));

    Ok(())
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

pub async fn resume_sop(config: Config, instance_id: String) -> Result<()> {
    let mut inst = load_instance(&instance_id)?;
    if inst.status != SopStatus::Failed && inst.status != SopStatus::Paused {
        anyhow::bail!("Only failed or paused SOP instances can be resumed");
    }

    inst.status = SopStatus::Running;
    if let Some(step) = inst.steps.get_mut(inst.current_step_index) {
        step.status = "Pending".to_string();
        step.error = None;
    }
    save_instance(&inst)?;

    let config_clone = config.clone();
    let inst_id_clone = instance_id.clone();
    tokio::spawn(async move {
        let _ = run_sop_instance(config_clone, inst_id_clone).await;
    });

    Ok(())
}
