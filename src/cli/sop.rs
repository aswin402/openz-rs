use crate::agent::style::colors::*;
use crate::cli::args::SopAction;
use crate::config::loader::load_config;
use crate::println;
use anyhow::Result;

fn parse_sop_payload(payload: Option<String>) -> Result<serde_json::Value> {
    let raw = match payload {
        Some(raw) => raw,
        None => return Ok(serde_json::json!({})),
    };
    let trimmed = raw.trim();
    match trimmed.starts_with('{') || trimmed.starts_with('[') {
        true => Ok(serde_json::from_str(trimmed)?),
        false => {
            let path = std::path::Path::new(trimmed);
            match path.exists() {
                true => {
                    let content = std::fs::read_to_string(path)?;
                    Ok(serde_json::from_str(&content)?)
                }
                false => {
                    anyhow::bail!("Payload must be a valid JSON string or path to a JSON file")
                }
            }
        }
    }
}

pub async fn handle_sop(action: SopAction) -> Result<()> {
    match action {
        SopAction::List => {
            let defs = crate::sop::load_definitions()?;
            println!(
                "\n{}📋 Available Standard Operating Procedures (SOPs):{}\n",
                COLOR_BOLD, COLOR_RESET
            );
            match defs.is_empty() {
                true => println!("No SOP definitions found."),
                false => {
                    for def in defs {
                        println!("{}• ID:{} {}", RED_ORANGE, COLOR_RESET, def.id);
                        println!("  {}Name:{} {}", COLOR_BOLD, COLOR_RESET, def.name);
                        println!(
                            "  {}Description:{} {}",
                            COLOR_BOLD, COLOR_RESET, def.description
                        );
                        println!("  {}Steps:{}", COLOR_BOLD, COLOR_RESET);
                        for (i, step) in def.steps.iter().enumerate() {
                            let deps_str = match step.depends_on.is_empty() {
                                true => String::new(),
                                false => format!(" [Depends on: {}]", step.depends_on.join(", ")),
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
        }
        SopAction::Instances => {
            let instances = crate::sop::list_instances()?;
            println!(
                "\n{}📋 SOP Execution Instances:{}\n",
                COLOR_BOLD, COLOR_RESET
            );
            match instances.is_empty() {
                true => println!("No SOP instances executed yet."),
                false => {
                    for inst in instances {
                        let status_color = match inst.status {
                            crate::sop::SopStatus::Completed => EMERALD_GREEN,
                            crate::sop::SopStatus::Failed => ERROR_RED,
                            crate::sop::SopStatus::Running => LIGHT_WHITE,
                            _ => COLOR_RESET,
                        };
                        let completed_count = inst
                            .steps
                            .iter()
                            .filter(|s| s.status == "Completed")
                            .count();
                        let display_step = match inst.current_step_index > 0 {
                            true => inst.current_step_index,
                            false => completed_count,
                        };
                        println!("{}• Instance ID:{} {}", RED_ORANGE, COLOR_RESET, inst.id);
                        println!("  {}SOP ID:{} {}", COLOR_BOLD, COLOR_RESET, inst.sop_id);
                        println!(
                            "  {}Status:{} {:?}{}",
                            COLOR_BOLD, status_color, inst.status, COLOR_RESET
                        );
                        println!(
                            "  {}Steps Completed:{} {}/{}",
                            COLOR_BOLD,
                            COLOR_RESET,
                            display_step,
                            inst.steps.len()
                        );
                        println!(
                            "  {}Started At:{} {}",
                            COLOR_BOLD, COLOR_RESET, inst.started_at
                        );
                        match inst.completed_at {
                            Some(ref completed) => {
                                println!("  {}Completed At:{} {}", COLOR_BOLD, COLOR_RESET, completed);
                            }
                            None => {}
                        }
                        println!();
                    }
                }
            }
        }
        SopAction::Trigger {
            sop_id,
            payload,
            wait,
        } => {
            let config = load_config()?;
            let payload_value = parse_sop_payload(payload)?;

            match wait {
                true => println!("Executing SOP '{}' synchronously...", sop_id),
                false => println!("Triggering SOP '{}' in background...", sop_id),
            }

            match crate::sop::engine::trigger_sop_with_options(
                config,
                sop_id.clone(),
                payload_value,
                wait,
            )
            .await
            {
                Ok(instance_id) => match wait {
                    true => {
                        println!(
                            "{}✓ SOP execution finished!{}",
                            EMERALD_GREEN, COLOR_RESET
                        );
                        println!("Instance ID: {}", instance_id);
                        match crate::sop::load_instance(&instance_id) {
                            Ok(inst) => {
                                println!("Status: {:?}", inst.status);
                                println!(
                                    "Steps completed: {}/{}",
                                    inst.current_step_index,
                                    inst.steps.len()
                                );
                            }
                            Err(_) => {}
                        }
                    }
                    false => {
                        println!(
                            "{}✓ SOP successfully triggered!{}",
                            EMERALD_GREEN, COLOR_RESET
                        );
                        println!("Instance ID: {}", instance_id);
                    }
                },
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
            match wait {
                true => println!("Resuming SOP instance '{}' synchronously...", instance_id),
                false => println!("Resuming SOP instance '{}' in background...", instance_id),
            }

            match crate::sop::engine::resume_sop_with_options(config, instance_id.clone(), wait)
                .await
            {
                Ok(_) => match wait {
                    true => {
                        println!(
                            "{}✓ SOP instance resumed and execution completed!{}",
                            EMERALD_GREEN, COLOR_RESET
                        );
                        match crate::sop::load_instance(&instance_id) {
                            Ok(inst) => {
                                println!("Status: {:?}", inst.status);
                                println!(
                                    "Steps completed: {}/{}",
                                    inst.current_step_index,
                                    inst.steps.len()
                                );
                            }
                            Err(_) => {}
                        }
                    }
                    false => {
                        println!(
                            "{}✓ SOP instance resume initiated successfully!{}",
                            EMERALD_GREEN, COLOR_RESET
                        );
                    }
                },
                Err(e) => {
                    eprintln!("{}❌ Failed to resume SOP: {}{}", ERROR_RED, e, COLOR_RESET);
                }
            }
        }
        SopAction::Simulate { sop_id, payload } => {
            let config = load_config()?;
            let payload_value = parse_sop_payload(payload)?;

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

