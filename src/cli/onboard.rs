use anyhow::Result;
use inquire::{Password, PasswordDisplayMode, Select, Text};

use crate::channels::{provider_model_catalog, provider_models_by_name};
use crate::config::loader::{load_config, resolve_path, save_config};
use crate::config::provider_catalog;
use crate::config::schema::{Config, ProviderConfig};
use crate::println;

fn non_empty_trimmed_or(input: &str, default: &str) -> String {
    let trimmed = input.trim();
    match trimmed.is_empty() {
        true => default.to_string(),
        false => trimmed.to_string(),
    }
}

pub fn onboard_provider_names() -> Vec<&'static str> {
    provider_model_catalog()
        .iter()
        .map(|provider| provider.name)
        .collect()
}

pub fn default_onboard_model(provider_name: &str) -> &'static str {
    provider_models_by_name(provider_name)
        .and_then(|provider| provider.models.first().copied())
        .unwrap_or("default")
}

pub async fn handle_onboard() -> Result<()> {
    println!("=== Welcome to the OpenZ Setup Wizard ===");

    let providers = onboard_provider_names();
    let provider_name = Select::new("Choose an LLM provider:", providers).prompt()?;

    let is_local = provider_catalog::find_provider(provider_name)
        .is_some_and(|descriptor| descriptor.local);

    let api_key = match is_local {
        true => None,
        false => {
            let key = Password::new(&format!("Enter API Key for {}:", provider_name))
                .without_confirmation()
                .with_display_mode(PasswordDisplayMode::Masked)
                .prompt()?;
            let trimmed = key.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        }
    };

    let default_base = provider_catalog::default_api_base_for_provider(provider_name);
    let api_base_input =
        Text::new(&format!("Enter API Base URL [default: {}]:", default_base)).prompt()?;
    let api_base = Some(non_empty_trimmed_or(&api_base_input, default_base));

    let default_model = default_onboard_model(provider_name);
    let model_input = Text::new(&format!(
        "Enter LLM Model Name [default: {}]:",
        default_model
    ))
    .prompt()?;
    let model = non_empty_trimmed_or(&model_input, default_model);

    let bot_name_input = Text::new("Enter Bot Name [default: openz]:").prompt()?;
    let bot_name = non_empty_trimmed_or(&bot_name_input, "openz");

    let bot_icon_input = Text::new("Enter Bot Icon (Emoji/text) [default: ⚡]:").prompt()?;
    let bot_icon = non_empty_trimmed_or(&bot_icon_input, "⚡");

    let mut config = load_config().unwrap_or_else(|_| Config::default());

    config.set_provider_config(
        provider_name,
        ProviderConfig {
            api_key,
            api_key_env: None,
            api_key_file: None,
            api_base,
            default_model: Some(model.clone()),
            extra: std::collections::HashMap::new(),
        },
    );

    config.agents.defaults.provider = provider_name.to_string();
    config.agents.defaults.model = model;
    config.agents.defaults.bot_name = bot_name;
    config.agents.defaults.bot_icon = bot_icon;

    save_config(&config)?;

    let workspace = resolve_path(&config.agents.defaults.workspace);
    match workspace.exists() {
        true => {}
        false => std::fs::create_dir_all(&workspace)?,
    }

    println!(
        "\n✅ Onboarding complete! Settings saved to {:?}",
        crate::config::config_path()
    );
    println!("You can now run 'openz' to start chatting.");

    Ok(())
}

