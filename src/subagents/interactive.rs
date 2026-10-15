//! Interactive terminal wizards and menus for subagent creation, management, and AI design.

use super::{load_profiles, save_profiles, SubagentProfile};
use crate::config::schema::Config;
use crate::providers::GenerationSettings;
use crate::session::Message;
use anyhow::{anyhow, Context, Result};
use inquire::{Confirm, Text};

fn build_subagent_menu_names(profiles: &[SubagentProfile]) -> Vec<String> {
    let mut names: Vec<String> = profiles.iter().map(|p| p.name.clone()).collect();
    names.push("Back".to_string());
    names
}

fn print_subagent_summary(profile: &SubagentProfile, config: &Config, heading: &str) {
    println!("\n--- {} ---", heading);
    println!("Name: {}", profile.name);
    println!("Description: {}", profile.description);
    println!(
        "Primary Model: {}",
        profile.model.as_deref().unwrap_or("(default)")
    );
    let fallbacks_display = profile
        .fallbacks
        .clone()
        .unwrap_or_else(|| config.get_dynamic_fallbacks(&profile.name));
    println!("Fallback Models: {:?}", fallbacks_display);
    println!("System Prompt:\n{}", profile.system_prompt);
    println!("------------------------------------");
}

async fn prompt_fallback_models(
    existing: Option<&[String]>,
    verb: &str,
    config: &Config,
) -> Result<Vec<String>> {
    let mut fallbacks = Vec::new();
    for idx in 1..=3 {
        let default_val = existing
            .and_then(|f| f.get(idx - 1).cloned())
            .unwrap_or_default();
        let label = format!("{verb} Fallback Model {idx} (Exit/Esc to skip):");
        match prompt_choose_model(&label, &default_val, config).await? {
            Some(fallback) => fallbacks.push(fallback),
            None => {}
        }
    }
    Ok(fallbacks)
}

pub async fn run_subagent_manager(config: Config) -> Result<()> {
    let _ = load_profiles()?;
    let active_mdl = config.agents.defaults.model.clone();

    loop {
        let choices = vec![
            "List / Manage Subagents".to_string(),
            "Create New Subagent".to_string(),
            "Exit".to_string(),
        ];

        let choice_idx = match crate::agent::style::select_menu_custom(
            "Choose an option:",
            &choices,
            &active_mdl,
            Some("OpenZ Subagent Manager"),
            true,
        ) {
            Ok(Some(idx)) => idx,
            _ => break,
        };

        match choice_idx {
            0 => match manage_menu(&config).await {
                Ok(()) => {}
                Err(e) => tracing::error!("Error managing subagents: {}", e),
            },
            1 => match create_menu(&config).await {
                Ok(()) => {}
                Err(e) => tracing::error!("Error creating subagent: {}", e),
            },
            _ => {
                println!("Exiting subagent manager.");
                break;
            }
        }
    }
    Ok(())
}

async fn manage_menu(config: &Config) -> Result<()> {
    let mut profiles = load_profiles()?;
    match profiles.is_empty() {
        true => {
            println!("No subagents currently configured.");
            return Ok(());
        }
        false => {}
    }

    let active_mdl = config.agents.defaults.model.clone();
    let mut subagent_names = build_subagent_menu_names(&profiles);

    loop {
        let name_choice_idx = match crate::agent::style::select_menu_custom(
            "Select a subagent to manage:",
            &subagent_names,
            &active_mdl,
            Some("List Subagents"),
            true,
        ) {
            Ok(Some(idx)) => idx,
            _ => break,
        };

        match name_choice_idx == profiles.len() {
            true => break,
            false => {}
        }

        let name_choice = &subagent_names[name_choice_idx];

        match profiles.iter().position(|p| p.name == *name_choice) {
            Some(pos) => loop {
                let profile = &profiles[pos];
                print_subagent_summary(
                    profile,
                    config,
                    &format!("Subagent Details: {}", profile.name),
                );

                let options = vec![
                    "Modify Subagent".to_string(),
                    "Delete Subagent".to_string(),
                    "Back".to_string(),
                ];

                let action_idx = match crate::agent::style::select_menu_custom(
                    "Select action:",
                    &options,
                    &active_mdl,
                    Some(&format!("Manage: {}", profile.name)),
                    true,
                ) {
                    Ok(Some(idx)) => idx,
                    _ => break,
                };

                match action_idx {
                    0 => {
                        let mut modified = profile.clone();
                        modified.description = Text::new("Edit Description:")
                            .with_initial_value(&profile.description)
                            .prompt()?;
                        modified.system_prompt = Text::new("Edit System Prompt:")
                            .with_initial_value(&profile.system_prompt)
                            .prompt()?;
                        match prompt_choose_model(
                            "Edit Primary Model:",
                            profile.model.as_deref().unwrap_or(""),
                            config,
                        )
                        .await?
                        {
                            Some(selected_model) => modified.model = Some(selected_model),
                            None => {}
                        }

                        let fallbacks =
                            prompt_fallback_models(profile.fallbacks.as_deref(), "Edit", config)
                                .await?;
                        modified.fallbacks = Some(fallbacks);

                        profiles[pos] = modified;
                        save_profiles(&profiles)?;
                        println!("✅ Subagent modified successfully.");
                    }
                    1 => {
                        let confirm = Confirm::new(&format!(
                            "Are you sure you want to delete {}?",
                            profile.name
                        ))
                        .with_default(false)
                        .prompt()?;
                        match confirm {
                            true => {
                                profiles.remove(pos);
                                save_profiles(&profiles)?;
                                println!("✅ Subagent deleted successfully.");
                                subagent_names = build_subagent_menu_names(&profiles);
                                break;
                            }
                            false => {}
                        }
                    }
                    _ => break,
                }
            },
            None => {}
        }
    }

    Ok(())
}

async fn create_menu(config: &Config) -> Result<()> {
    let creation_types = vec![
        "Create Manually".to_string(),
        "Create with AI (Ask OpenZ)".to_string(),
        "Back".to_string(),
    ];
    let active_mdl = config.agents.defaults.model.clone();

    loop {
        let choice_idx = match crate::agent::style::select_menu_custom(
            "How would you like to create the subagent?",
            &creation_types,
            &active_mdl,
            Some("Create Subagent"),
            true,
        ) {
            Ok(Some(idx)) => idx,
            _ => break,
        };

        match choice_idx {
            0 => {
                let name = Text::new("Enter Subagent Name (snake_case):").prompt()?;
                match name.trim().is_empty() {
                    true => return Err(anyhow!("Name cannot be empty.")),
                    false => {}
                }

                let description = Text::new("Enter Description:").prompt()?;
                let system_prompt = Text::new("Enter System Prompt:").prompt()?;
                let model = prompt_choose_model(
                    "Choose Primary Model (Enter/Esc for default):",
                    &config.agents.defaults.model,
                    config,
                )
                .await?;

                let fallbacks = prompt_fallback_models(None, "Choose", config).await?;
                let fallbacks_opt = match fallbacks.is_empty() {
                    true => None,
                    false => Some(fallbacks),
                };

                let new_profile = SubagentProfile {
                    name: name.trim().to_string(),
                    description,
                    system_prompt,
                    model,
                    fallbacks: fallbacks_opt,
                    extra: serde_json::Map::new(),
                };

                let mut profiles = load_profiles()?;
                profiles.push(new_profile);
                save_profiles(&profiles)?;
                println!("✅ Subagent manual creation complete.");
                break;
            }
            1 => {
                let task_description = Text::new(
                    "Describe the specific task or role you want this subagent to perform:",
                )
                .prompt()?;
                match task_description.trim().is_empty() {
                    true => return Err(anyhow!("Description cannot be empty.")),
                    false => {}
                }

                println!("🧠 Asking OpenZ to design this subagent for you...");
                let ai_designed = ask_openz_to_design(config, &task_description).await?;

                print_subagent_summary(&ai_designed, config, "AI Designed Subagent Proposed");

                let save_choice = Confirm::new("Save this AI-designed subagent?")
                    .with_default(true)
                    .prompt()?;
                match save_choice {
                    true => {
                        let mut profiles = load_profiles()?;
                        profiles.push(ai_designed);
                        save_profiles(&profiles)?;
                        println!("✅ AI-designed subagent saved successfully.");
                    }
                    false => {}
                }
                break;
            }
            _ => break,
        }
    }

    Ok(())
}

async fn ask_openz_to_design(config: &Config, task_description: &str) -> Result<SubagentProfile> {
    let provider = crate::cli::build_agent_loop(config.clone()).await?.provider;

    let system_prompt = "You are a specialized agent creator. Given a user's description of a task, design a custom subagent. \
        Return the output as a clean JSON block exactly matching this schema:\n\
        {\n\
          \"name\": \"snake_case_name\",\n\
          \"description\": \"One sentence summary of the subagent's role\",\n\
          \"system_prompt\": \"Detailed system prompt containing instructions, rules, and formats for this agent\"\n\
        }\n\
        Do not return any conversational text or markdown blocks, only the raw JSON.";

    let prompt = format!("Task description: {}", task_description);
    let messages = vec![Message {
        role: "user".to_string(),
        content: prompt,
        timestamp: Some(chrono::Utc::now().to_rfc3339()),
        extra: serde_json::Map::new(),
    }];

    let settings = GenerationSettings {
        temperature: 0.2,
        max_tokens: 1024,
        reasoning_effort: None,
    };

    let resp = provider
        .chat(system_prompt, &messages, &[], &settings)
        .await?;
    let content = resp
        .content
        .ok_or_else(|| anyhow!("No design returned from AI"))?;

    let cleaned_content = content
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim()
        .to_string();

    let ai_profile: SubagentProfile =
        serde_json::from_str(&cleaned_content).with_context(|| {
            format!(
                "Failed to parse AI response as SubagentProfile. Response was: {}",
                content
            )
        })?;

    Ok(ai_profile)
}

async fn prompt_choose_model(
    prompt_label: &str,
    current_model: &str,
    config: &Config,
) -> Result<Option<String>> {
    // Keep subagent model selection aligned with the canonical catalog while
    // preserving its existing exclusion of custom Mivi and auto-start Ollama.
    let provider_list = crate::channels::provider_model_catalog()
        .iter()
        .filter(|provider| !matches!(provider.name, "mivi" | "ollama_local"))
        .filter(|provider| config.is_provider_configured(provider.name))
        .collect::<Vec<_>>();

    match provider_list.is_empty() {
        true => {
            println!(
                "{}⚠️ No LLM providers configured! Please run 'openz configure' first.{}",
                crate::agent::style::colors::AURA_GOLD,
                crate::agent::style::colors::COLOR_RESET
            );
            return Ok(None);
        }
        false => {}
    }

    let mut provider_options: Vec<String> = provider_list
        .iter()
        .map(|p| format!("{} ({})", p.display, p.models.len()))
        .collect();
    provider_options.push("Exit".to_string());

    let prov_idx = match crate::agent::style::select_menu_custom(
        prompt_label,
        &provider_options,
        current_model,
        Some("Select Provider"),
        true,
    )? {
        Some(idx) => idx,
        None => return Ok(None),
    };

    let prov_info = match provider_list.get(prov_idx) {
        Some(info) => *info,
        None => return Ok(None),
    };

    let mut model_options =
        match crate::channels::fetch_provider_models(prov_info.name, config).await {
            Some(models) => models,
            None => prov_info.models.iter().map(|&m| m.to_string()).collect(),
        };
    model_options.push("Type manually (Custom Model)".to_string());
    model_options.push("Exit".to_string());

    let model_idx = match crate::agent::style::select_menu_custom(
        &format!(
            "Choose a model from {} ({}):",
            prov_info.display,
            prov_info.models.len()
        ),
        &model_options,
        current_model,
        None,
        false,
    )? {
        Some(idx) => idx,
        None => return Ok(None),
    };

    match model_idx == model_options.len() - 1 {
        true => return Ok(None),
        false => {}
    }

    let mut final_model = match model_idx == model_options.len() - 2 {
        true => {
            let custom_model = inquire::Text::new("Enter custom model name:")
                .with_initial_value(current_model)
                .prompt()?;
            match custom_model.trim().is_empty() {
                true => return Ok(None),
                false => custom_model.trim().to_string(),
            }
        }
        false => model_options[model_idx].clone(),
    };

    let descriptor = crate::config::provider_catalog::find_provider(prov_info.name);
    let has_provider_prefix = descriptor
        .map(|descriptor| {
            descriptor
                .model_prefixes
                .iter()
                .any(|prefix| final_model.starts_with(prefix))
        })
        .unwrap_or(false);

    match has_provider_prefix {
        true => {}
        false => {
            final_model = format!("{}/{}", prov_info.name, final_model);
        }
    }

    Ok(Some(final_model))
}
