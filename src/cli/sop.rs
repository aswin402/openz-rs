use crate::agent::style::colors::*;
use crate::cli::args::SopAction;
use crate::config::loader::load_config;
use crate::println;
use anyhow::Result;

pub async fn handle_sop(action: SopAction) -> Result<()> {
    match action {
        SopAction::List => {
            let defs = crate::sop::load_definitions()?;
            println!(
                "\n{}📋 Available Standard Operating Procedures (SOPs):{}\n",
                COLOR_BOLD, COLOR_RESET
            );
            if defs.is_empty() {
                println!("No SOP definitions found.");
            } else {
                for def in defs {
                    println!("{}• ID:{} {}", AURA_PURPLE, COLOR_RESET, def.id);
                    println!("  {}Name:{} {}", COLOR_BOLD, COLOR_RESET, def.name);
                    println!(
                        "  {}Description:{} {}",
                        COLOR_BOLD, COLOR_RESET, def.description
                    );
                    println!("  {}Steps:{}", COLOR_BOLD, COLOR_RESET);
                    for (i, step) in def.steps.iter().enumerate() {
                        let deps_str = if step.depends_on.is_empty() {
                            String::new()
                        } else {
                            format!(" [Depends on: {}]", step.depends_on.join(", "))
                        };
                        println!(
                            "    {}. {}{}: {}",
                            i + 1,
                            step.name,
                            deps_str,
                            step.description
                        );
                    }
                    println!();
                }
            }
        }
        SopAction::Instances => {
            let instances = crate::sop::list_instances()?;
            println!(
                "\n{}📋 SOP Execution Instances:{}\n",
                COLOR_BOLD, COLOR_RESET
            );
            if instances.is_empty() {
                println!("No SOP instances executed yet.");
            } else {
                for inst in instances {
                    let status_color = match inst.status {
                        crate::sop::SopStatus::Completed => EMERALD_GREEN,
                        crate::sop::SopStatus::Failed => ERROR_RED,
                        crate::sop::SopStatus::Running => LIGHT_WHITE,
                        _ => COLOR_RESET,
                    };
                    println!("{}• Instance ID:{} {}", AURA_PURPLE, COLOR_RESET, inst.id);
                    println!("  {}SOP ID:{} {}", COLOR_BOLD, COLOR_RESET, inst.sop_id);
                    println!(
                        "  {}Status:{} {:?}{}",
                        COLOR_BOLD, status_color, inst.status, COLOR_RESET
                    );
                    println!(
                        "  {}Current Step:{} {}/{}",
                        COLOR_BOLD,
                        COLOR_RESET,
                        inst.current_step_index,
                        inst.steps.len()
                    );
                    println!(
                        "  {}Started At:{} {}",
                        COLOR_BOLD, COLOR_RESET, inst.started_at
                    );
                    if let Some(ref completed) = inst.completed_at {
                        println!("  {}Completed At:{} {}", COLOR_BOLD, COLOR_RESET, completed);
                    }
                    println!();
                }
            }
        }
        SopAction::Trigger { sop_id, payload, wait } => {
            let config = load_config()?;
            let payload_value = if let Some(p) = payload {
                let p_trimmed = p.trim();
                if p_trimmed.starts_with('{') || p_trimmed.starts_with('[') {
                    serde_json::from_str(p_trimmed)?
                } else {
                    // Try parsing as file path
                    let path = std::path::Path::new(p_trimmed);
                    if path.exists() {
                        let content = std::fs::read_to_string(path)?;
                        serde_json::from_str(&content)?
                    } else {
                        anyhow::bail!("Payload must be a valid JSON string or path to a JSON file");
                    }
                }
            } else {
                serde_json::json!({})
            };

            if wait {
                println!("Executing SOP '{}' synchronously...", sop_id);
            } else {
                println!("Triggering SOP '{}' in background...", sop_id);
            }

            match crate::sop::engine::trigger_sop_with_options(config, sop_id.clone(), payload_value, wait).await {
                Ok(instance_id) => {
                    if wait {
                        println!(
                            "{}✓ SOP execution finished!{}",
                            EMERALD_GREEN, COLOR_RESET
                        );
                        if let Ok(inst) = crate::sop::load_instance(&instance_id) {
                            println!("Instance ID: {}", instance_id);
                            println!("Status: {:?}", inst.status);
                            println!("Steps completed: {}/{}", inst.current_step_index, inst.steps.len());
                        } else {
                            println!("Instance ID: {}", instance_id);
                        }
                    } else {
                        println!(
                            "{}✓ SOP successfully triggered!{}",
                            EMERALD_GREEN, COLOR_RESET
                        );
                        println!("Instance ID: {}", instance_id);
                    }
                }
                Err(e) => {
                    eprintln!(
                        "{}❌ Failed to execute SOP: {}{}",
                        ERROR_RED, e, COLOR_RESET
                    );
                }
            }
        }
        SopAction::Resume { instance_id, wait } => {
            let config = load_config()?;
            if wait {
                println!("Resuming SOP instance '{}' synchronously...", instance_id);
            } else {
                println!("Resuming SOP instance '{}' in background...", instance_id);
            }

            match crate::sop::engine::resume_sop_with_options(config, instance_id.clone(), wait).await {
                Ok(_) => {
                    if wait {
                        println!(
                            "{}✓ SOP instance resumed and execution completed!{}",
                            EMERALD_GREEN, COLOR_RESET
                        );
                        if let Ok(inst) = crate::sop::load_instance(&instance_id) {
                            println!("Status: {:?}", inst.status);
                            println!("Steps completed: {}/{}", inst.current_step_index, inst.steps.len());
                        }
                    } else {
                        println!(
                            "{}✓ SOP instance resume initiated successfully!{}",
                            EMERALD_GREEN, COLOR_RESET
                        );
                    }
                }
                Err(e) => {
                    eprintln!("{}❌ Failed to resume SOP: {}{}", ERROR_RED, e, COLOR_RESET);
                }
            }
        }
        SopAction::Simulate { sop_id, payload } => {
            let config = load_config()?;
            let payload_value = if let Some(p) = payload {
                let p_trimmed = p.trim();
                if p_trimmed.starts_with('{') || p_trimmed.starts_with('[') {
                    serde_json::from_str(p_trimmed)?
                } else {
                    let path = std::path::Path::new(p_trimmed);
                    if path.exists() {
                        let content = std::fs::read_to_string(path)?;
                        serde_json::from_str(&content)?
                    } else {
                        anyhow::bail!("Payload must be a valid JSON string or path to a JSON file");
                    }
                }
            } else {
                serde_json::json!({})
            };

            println!("Simulating SOP '{}'...", sop_id);
            match crate::sop::engine::trigger_sop_simulation(config, sop_id.clone(), payload_value)
                .await
            {
                Ok(instance_id) => {
                    println!(
                        "{}✓ SOP simulation finished successfully!{}",
                        EMERALD_GREEN, COLOR_RESET
                    );
                    println!("Simulated Instance ID: {}", instance_id);
                }
                Err(e) => {
                    eprintln!(
                        "{}❌ Failed to simulate SOP: {}{}",
                        ERROR_RED, e, COLOR_RESET
                    );
                }
            }
        }
    }
    Ok(())
}
