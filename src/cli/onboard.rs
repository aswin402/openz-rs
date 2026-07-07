use anyhow::Result;
use inquire::{Password, PasswordDisplayMode, Select, Text};

use crate::config::loader::{load_config, resolve_path, save_config};
use crate::config::schema::{Config, ProviderConfig};
use crate::println;

pub async fn handle_onboard() -> Result<()> {
    println!("=== Welcome to the OpenZ Setup Wizard ===");

    let providers = vec![
        "anthropic",
        "openai",
        "openrouter",
        "deepseek",
        "groq",
        "ollama",
        "minimax",
    ];
    let provider_name = Select::new("Choose an LLM provider:", providers).prompt()?;

    let mut api_key = None;
    if provider_name != "ollama" {
        let key = Password::new(&format!("Enter API Key for {}:", provider_name))
            .without_confirmation()
            .with_display_mode(PasswordDisplayMode::Masked)
            .prompt()?;
        if !key.trim().is_empty() {
            api_key = Some(key.trim().to_string());
        }
    }

    let default_base = match provider_name {
        "anthropic" => "https://api.anthropic.com",
        "openai" => "https://api.openai.com/v1",
        "openrouter" => "https://openrouter.ai/api/v1",
        "deepseek" => "https://api.deepseek.com/v1",
        "groq" => "https://api.groq.com/openai/v1",
        "ollama" => "http://localhost:11434/v1",
        "minimax" => "https://api.minimax.io/v1",
        _ => "",
    };

    let api_base_input =
        Text::new(&format!("Enter API Base URL [default: {}]:", default_base)).prompt()?;
    let api_base = if api_base_input.trim().is_empty() {
        Some(default_base.to_string())
    } else {
        Some(api_base_input.trim().to_string())
    };

    let default_model = match provider_name {
        "anthropic" => "claude-3-5-sonnet-20241022",
        "openai" => "gpt-4o",
        "openrouter" => "google/gemini-2.5-pro",
        "deepseek" => "deepseek-chat",
        "groq" => "llama3-70b-8192",
        "ollama" => "llama3",
        "minimax" => "MiniMax-M3",
        _ => "",
    };

    let model_input = Text::new(&format!(
        "Enter LLM Model Name [default: {}]:",
        default_model
    ))
    .prompt()?;
    let model = if model_input.trim().is_empty() {
        default_model.to_string()
    } else {
        model_input.trim().to_string()
    };

    let bot_name = Text::new("Enter Bot Name [default: openz]:").prompt()?;
    let bot_name = if bot_name.trim().is_empty() {
        "openz".to_string()
    } else {
        bot_name.trim().to_string()
    };

    let bot_icon = Text::new("Enter Bot Icon (Emoji/text) [default: ⚡]:").prompt()?;
    let bot_icon = if bot_icon.trim().is_empty() {
        "⚡".to_string()
    } else {
        bot_icon.trim().to_string()
    };

    let mut config = load_config().unwrap_or_else(|_| Config::default());

    let p_config = Some(ProviderConfig {
        api_key,
        api_base,
        extra: std::collections::HashMap::new(),
    });

    match provider_name {
        "anthropic" => config.providers.anthropic = p_config,
        "openai" => config.providers.openai = p_config,
        "openrouter" => config.providers.openrouter = p_config,
        "deepseek" => config.providers.deepseek = p_config,
        "groq" => config.providers.groq = p_config,
        "ollama" => config.providers.ollama = p_config,
        "minimax" => config.providers.minimax = p_config,
        _ => {}
    }

    config.agents.defaults.provider = provider_name.to_string();
    config.agents.defaults.model = model;
    config.agents.defaults.bot_name = bot_name;
    config.agents.defaults.bot_icon = bot_icon;

    save_config(&config)?;

    let workspace = resolve_path(&config.agents.defaults.workspace);
    if !workspace.exists() {
        std::fs::create_dir_all(&workspace)?;
    }

    println!(
        "\n✅ Onboarding complete! Settings saved to {:?}",
        crate::config::config_path()
    );
    println!("You can now run 'openz agent' to start chatting.");

    Ok(())
}
