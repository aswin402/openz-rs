//! Interactive terminal wizards and menus for subagent creation, management, and AI design.

use super::{load_profiles, save_profiles, SubagentProfile};
use crate::config::schema::Config;
use crate::providers::GenerationSettings;
use crate::session::Message;
use anyhow::{anyhow, Context, Result};
use inquire::{Confirm, Text};

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
            _ => break, // Exit on Esc
        };

        match choice_idx {
            0 => {
                if let Err(e) = manage_menu(&config).await {
                    tracing::error!("Error managing subagents: {}", e);
                }
            }
            1 => {
                if let Err(e) = create_menu(&config).await {
                    tracing::error!("Error creating subagent: {}", e);
                }
            }
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
    if profiles.is_empty() {
        println!("No subagents currently configured.");
        return Ok(());
    }

    let active_mdl = config.agents.defaults.model.clone();
    let mut subagent_names: Vec<String> = profiles.iter().map(|p| p.name.clone()).collect();
    subagent_names.push("Back".to_string());

    loop {
        let name_choice_idx = match crate::agent::style::select_menu_custom(
            "Select a subagent to manage:",
            &subagent_names,
            &active_mdl,
            Some("List Subagents"),
            true,
        ) {
            Ok(Some(idx)) => idx,
            _ => break, // Go back on Esc
        };

        if name_choice_idx == profiles.len() {
            break; // Back option
        }

        let name_choice = &subagent_names[name_choice_idx];

        if let Some(pos) = profiles.iter().position(|p| p.name == *name_choice) {
            loop {
                let profile = &profiles[pos];
                println!("\n--- Subagent Details: {} ---", profile.name);
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
                    _ => break, // Go back on Esc
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
                        if let Some(selected_model) = prompt_choose_model(
                            "Edit Primary Model:",
                            profile.model.as_deref().unwrap_or(""),
                            config,
                        )
                        .await?
                        {
                            modified.model = Some(selected_model);
                        }

                        let mut fallbacks = Vec::new();
                        for idx in 1..=3 {
                            let default_val = profile
                                .fallbacks
                                .as_ref()
                                .and_then(|f| f.get(idx - 1).cloned())
                                .unwrap_or_default();
                            let label = format!("Edit Fallback Model {} (Exit/Esc to skip):", idx);
                            if let Some(fallback) =
                                prompt_choose_model(&label, &default_val, config).await?
                            {
                                fallbacks.push(fallback);
                            }
                        }
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
                        if confirm {
                            profiles.remove(pos);
                            save_profiles(&profiles)?;
                            println!("✅ Subagent deleted successfully.");
                            subagent_names = profiles.iter().map(|p| p.name.clone()).collect();
                            subagent_names.push("Back".to_string());
                            break;
                        }
                    }
                    _ => break,
                }
            }
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
            _ => break, // Go back on Esc
        };

        if choice_idx == 2 {
            break; // Back option
        }

        match choice_idx {
            0 => {
                let name = Text::new("Enter Subagent Name (snake_case):").prompt()?;
                if name.trim().is_empty() {
                    return Err(anyhow!("Name cannot be empty."));
                }

                let description = Text::new("Enter Description:").prompt()?;
                let system_prompt = Text::new("Enter System Prompt:").prompt()?;
                let model = prompt_choose_model(
                    "Choose Primary Model (Enter/Esc for default):",
                    &config.agents.defaults.model,
                    config,
                )
                .await?;

                let mut fallbacks = Vec::new();
                for idx in 1..=3 {
                    let label = format!("Choose Fallback Model {} (Exit/Esc to skip):", idx);
                    if let Some(fallback) = prompt_choose_model(&label, "", config).await? {
                        fallbacks.push(fallback);
                    }
                }
                let fallbacks_opt = if fallbacks.is_empty() {
                    None
                } else {
                    Some(fallbacks)
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
                if task_description.trim().is_empty() {
                    return Err(anyhow!("Description cannot be empty."));
                }

                println!("🧠 Asking OpenZ to design this subagent for you...");
                let ai_designed = ask_openz_to_design(config, &task_description).await?;

                println!("\n--- AI Designed Subagent Proposed ---");
                println!("Name: {}", ai_designed.name);
                println!("Description: {}", ai_designed.description);
                println!(
                    "Primary Model: {}",
                    ai_designed.model.as_deref().unwrap_or("(default)")
                );
                let fallbacks_display = ai_designed
                    .fallbacks
                    .clone()
                    .unwrap_or_else(|| config.get_dynamic_fallbacks(&ai_designed.name));
                println!("Fallback Models: {:?}", fallbacks_display);
                println!("System Prompt:\n{}", ai_designed.system_prompt);
                println!("------------------------------------");

                let save_choice = Confirm::new("Save this AI-designed subagent?")
                    .with_default(true)
                    .prompt()?;
                if save_choice {
                    let mut profiles = load_profiles()?;
                    profiles.push(ai_designed);
                    save_profiles(&profiles)?;
                    println!("✅ AI-designed subagent saved successfully.");
                }
                break;
            }
            _ => {}
        }
    }

    Ok(())
}

async fn ask_openz_to_design(config: &Config, task_description: &str) -> Result<SubagentProfile> {
    // 1. Build provider
    let provider = crate::cli::build_agent_loop(config.clone()).await?.provider;

    // 2. Query LLM to generate profile JSON
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

    // Parse JSON safely
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

    if provider_list.is_empty() {
        println!(
            "{}⚠️ No LLM providers configured! Please run 'openz configure' first.{}",
            crate::agent::style::colors::AURA_GOLD,
            crate::agent::style::colors::COLOR_RESET
        );
        return Ok(None);
    }

    let mut provider_options: Vec<String> = provider_list
        .iter()
        .map(|p| format!("{} ({})", p.display, p.models.len()))
        .collect();
    provider_options.push("Exit".to_string());

    match crate::agent::style::select_menu_custom(
        prompt_label,
        &provider_options,
        current_model,
        Some("Select Provider"),
        true,
    )? {
        Some(prov_idx) => {
            if prov_idx == provider_list.len() {
                return Ok(None);
            }
            let prov_info = provider_list[prov_idx];

            let mut model_options =
                match crate::channels::fetch_provider_models(prov_info.name, config).await {
                    Some(models) => models,
                    None => prov_info.models.iter().map(|&m| m.to_string()).collect(),
                };
            model_options.push("Type manually (Custom Model)".to_string());
            model_options.push("Exit".to_string());

            match crate::agent::style::select_menu_custom(
                &format!("Choose a model from {} ({}):", prov_info.display, prov_info.models.len()),
                &model_options,
                current_model,
                None,
                false,
            )? {
                Some(model_idx) => {
                    if model_idx == model_options.len() - 1 {
                        return Ok(None);
                    }
                    let mut final_model = if model_idx == model_options.len() - 2 {
                        let custom_model = inquire::Text::new("Enter custom model name:")
                            .with_initial_value(current_model)
                            .prompt()?;
                        if custom_model.trim().is_empty() {
                            return Ok(None);
                        }
                        custom_model.trim().to_string()
                    } else {
                        model_options[model_idx].clone()
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

                    if !has_provider_prefix {
                        final_model = format!("{}/{}", prov_info.name, final_model);
                    }

                    Ok(Some(final_model))
                }
                None => Ok(None),
            }
        }
        None => Ok(None),
    }
}
