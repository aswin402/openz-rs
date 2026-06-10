use serde::{Serialize, Deserialize};
use std::fs;
use std::path::PathBuf;
use anyhow::{Result, anyhow, Context};
use crate::config::resolve_path;
use crate::config::schema::Config;
use crate::providers::GenerationSettings;
use crate::session::Message;
use inquire::{Text, Confirm};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SubagentProfile {
    pub name: String,         // e.g. "twitter_researcher"
    pub description: String,  // What it does
    pub system_prompt: String,// System prompt tailored for its role
    pub model: String,        // Primary model to use (e.g. gpt-4o)
    pub fallbacks: Vec<String>, // Up to 3 fallback models
}

pub fn subagents_file_path() -> PathBuf {
    resolve_path("~/.openz/subagents.json")
}

pub fn load_profiles() -> Result<Vec<SubagentProfile>> {
    let path = subagents_file_path();
    let defaults = vec![
        SubagentProfile {
            name: "planner".to_string(),
            description: "Decomposes complex goals, manages workstreams, and tracks milestones.".to_string(),
            system_prompt: "You are a specialized Planner. Decompose the user's high-level task into clear, sequential milestones. Outline what needs to be checked, built, and verified at each stage.".to_string(),
            model: "gpt-4o-mini".to_string(),
            fallbacks: vec!["gpt-4o".to_string(), "claude-3-5-haiku".to_string()],
        },
        SubagentProfile {
            name: "researcher".to_string(),
            description: "Searches the web, reads files, and gathers project context.".to_string(),
            system_prompt: "You are a specialized Researcher. Conduct thorough web searches, analyze codebase directories, read relevant files, and retrieve external documentation to compile complete reference contexts.".to_string(),
            model: "gpt-4o-mini".to_string(),
            fallbacks: vec!["gpt-4o".to_string(), "claude-3-5-haiku".to_string()],
        },
        SubagentProfile {
            name: "architect".to_string(),
            description: "Designs system database schemas, directory layouts, and API contracts.".to_string(),
            system_prompt: "You are a specialized Architect. Design robust, performant system architectures, database tables, and API endpoints. Document your layouts clearly using structured schemas and Mermaid diagram definitions.".to_string(),
            model: "claude-3-5-sonnet".to_string(),
            fallbacks: vec!["gpt-4o".to_string(), "claude-3-5-haiku".to_string()],
        },
        SubagentProfile {
            name: "skill_creator".to_string(),
            description: "Writes specialized helper scripts and creates new native shell tools dynamically.".to_string(),
            system_prompt: "You are a specialized Skill Creator. Design and write automated bash or Python scripts to solve recurring workflow bottlenecks. Focus on robust error handling and type safety.".to_string(),
            model: "claude-3-5-sonnet".to_string(),
            fallbacks: vec!["gpt-4o".to_string(), "claude-3-5-haiku".to_string()],
        },
        SubagentProfile {
            name: "reviewer".to_string(),
            description: "Audits changed code files for security, logical bugs, and testing coverage.".to_string(),
            system_prompt: "You are a specialized Reviewer. Scan changed codebase files to identify security vulnerabilities, logical bugs, performance regressions, or style violations. Outline precise remediation recommendations.".to_string(),
            model: "claude-3-5-sonnet".to_string(),
            fallbacks: vec!["gpt-4o".to_string(), "claude-3-5-haiku".to_string()],
        },
        SubagentProfile {
            name: "code_auditor".to_string(),
            description: "Performs security audits on source code.".to_string(),
            system_prompt: "You are a specialized Code Auditor. Scan source code files, identify security vulnerabilities, potential exploits, insecure dependency usage, or coding flaws. Provide clear remediation advice and secure alternatives.".to_string(),
            model: "claude-3-5-sonnet".to_string(),
            fallbacks: vec!["gpt-4o".to_string(), "claude-3-5-haiku".to_string()],
        },
        SubagentProfile {
            name: "debugger".to_string(),
            description: "Diagnoses execution errors, reproduces bugs, and isolates root causes.".to_string(),
            system_prompt: "You are a specialized Debugger. Analyze system logs, stack traces, and failure modes to isolate root causes. Propose precise code changes to fix bugs and prevent regressions.".to_string(),
            model: "claude-3-5-sonnet".to_string(),
            fallbacks: vec!["gpt-4o".to_string(), "claude-3-5-haiku".to_string()],
        },
        SubagentProfile {
            name: "test_engineer".to_string(),
            description: "Designs QA test suites and writes unit, integration, and E2E tests.".to_string(),
            system_prompt: "You are a specialized Test Engineer. Write comprehensive unit, integration, and end-to-end test cases. Ensure high code coverage and robust validation of edge conditions.".to_string(),
            model: "gpt-4o-mini".to_string(),
            fallbacks: vec!["gpt-4o".to_string(), "claude-3-5-haiku".to_string()],
        },
        SubagentProfile {
            name: "devops_agent".to_string(),
            description: "Containerizes apps, drafts CI/CD pipelines, and manages infrastructure configs.".to_string(),
            system_prompt: "You are a specialized DevOps Agent. Write Dockerfiles, multi-stage builds, CI/CD workflow manifests (e.g. GitHub Actions), and infrastructure configuration scripts.".to_string(),
            model: "gpt-4o-mini".to_string(),
            fallbacks: vec!["gpt-4o".to_string(), "claude-3-5-haiku".to_string()],
        },
        SubagentProfile {
            name: "refactor_agent".to_string(),
            description: "Cleans up code complexity, applies patterns, and optimizes code structure.".to_string(),
            system_prompt: "You are a specialized Refactoring Agent. Analyze source code to identify complexity hotspots, code smells, or duplicate blocks. Refactor code for optimal maintainability and DRY principles.".to_string(),
            model: "claude-3-5-sonnet".to_string(),
            fallbacks: vec!["gpt-4o".to_string(), "claude-3-5-haiku".to_string()],
        },
        SubagentProfile {
            name: "memory_manager".to_string(),
            description: "Consolidates project facts, user preferences, and session context.".to_string(),
            system_prompt: "You are a specialized Memory Manager. Audit conversation transcripts to extract and save persistent project guidelines, user developer preferences, and critical decisions into markdown state files.".to_string(),
            model: "gpt-4o-mini".to_string(),
            fallbacks: vec!["gpt-4o".to_string(), "claude-3-5-haiku".to_string()],
        },
        SubagentProfile {
            name: "vision_agent".to_string(),
            description: "Analyzes wireframes, mockups, and UI visual layout aesthetics.".to_string(),
            system_prompt: "You are a specialized Vision Agent. Review UI screenshots, frontend layouts, wireframe assets, or image outputs to evaluate visual contrast, alignment, styling quality, and pixel-perfect aesthetics.".to_string(),
            model: "openrouter/google/gemini-2.5-flash".to_string(),
            fallbacks: vec!["openrouter/google/gemini-2.5-pro".to_string(), "gpt-4o-mini".to_string()],
        },
        SubagentProfile {
            name: "documentation_agent".to_string(),
            description: "Generates code docstrings, updates READMEs, and writes guides.".to_string(),
            system_prompt: "You are a specialized Documentation Agent. Maintain codebase clarity by writing docstrings, documenting module relations, updating README.md files, and writing onboarding guides.".to_string(),
            model: "gpt-4o-mini".to_string(),
            fallbacks: vec!["gpt-4o".to_string(), "claude-3-5-haiku".to_string()],
        },
        SubagentProfile {
            name: "self_improvement".to_string(),
            description: "Curates, updates, and refines agent memories and procedural skills.".to_string(),
            system_prompt: "You are a specialized Self-Improvement Agent. Analyze user queries, feedback, style complaints, and task transcripts. Refine long-term memory facts, create or update procedural skills, write reusable style guidelines, and organize them under ~/.openz/skills/ so the agent learns and grows.".to_string(),
            model: "claude-3-5-sonnet".to_string(),
            fallbacks: vec!["gpt-4o".to_string(), "claude-3-5-haiku".to_string()],
        },
        SubagentProfile {
            name: "skill_improvement".to_string(),
            description: "Audits, optimizes, and refines active agent skills inside ~/.openz/skills/.".to_string(),
            system_prompt: "You are a specialized Skill Improvement Agent. Your job is to audit, optimize, and refine active procedural skills inside ~/.openz/skills/. You have full access to read, list, add, and modify these markdown skill files using standard file tools. Read the existing files in ~/.openz/skills/, analyze compiler feedback, execution logs, or user styling preferences, and optimize, restructure, merge, or rewrite the skills to make the agent more accurate, efficient, and warning-free.".to_string(),
            model: "claude-3-5-sonnet".to_string(),
            fallbacks: vec!["gpt-4o".to_string(), "claude-3-5-haiku".to_string()],
        },
        SubagentProfile {
            name: "openz_maintainer".to_string(),
            description: "Diagnoses internal errors, performance bugs, or loop detections inside OpenZ itself.".to_string(),
            system_prompt: "You are a specialized OpenZ Maintainer Agent. Your job is to debug, fix, and maintain the OpenZ application and codebase. If there are internal errors, system crashes, loop detection events, or performance regressions, review the OpenZ codebase and log files, diagnose the root cause, write code fixes, run compilation checks, and ensure system health.".to_string(),
            model: "claude-3-5-sonnet".to_string(),
            fallbacks: vec!["gpt-4o".to_string(), "claude-3-5-haiku".to_string()],
        },
        SubagentProfile {
            name: "mcps_manager".to_string(),
            description: "Installs, configures, audits, and manages Model Context Protocol (MCP) servers and tools.".to_string(),
            system_prompt: "You are a specialized MCP Manager Agent. Your job is to install, configure, audit, and manage Model Context Protocol (MCP) servers and tools in OpenZ. You can read/write OpenZ configurations, verify system packages (node, npm, python, pip, uv, cargo), run installation commands for MCP package dependencies, and update the mcp_servers block in ~/.openz/config.json using standard tools. Use the 'exec_command' tool to test if dependencies are installed or to test run an MCP server.".to_string(),
            model: "claude-3-5-sonnet".to_string(),
            fallbacks: vec!["gpt-4o".to_string(), "claude-3-5-haiku".to_string()],
        }
    ];
 
    if !path.exists() {
        save_profiles(&defaults)?;
        return Ok(defaults);
    }
 
    let content = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read subagents file at {:?}", path))?;
    let mut loaded_profiles: Vec<SubagentProfile> = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse subagents file at {:?}", path))?;
 
    let mut migrated = false;
    for default_profile in defaults {
        if !loaded_profiles.iter().any(|p| p.name == default_profile.name) {
            loaded_profiles.push(default_profile);
            migrated = true;
        }
    }
 
    if migrated {
        save_profiles(&loaded_profiles)?;
    }
 
    Ok(loaded_profiles)
}

pub fn save_profiles(profiles: &[SubagentProfile]) -> Result<()> {
    let path = subagents_file_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(profiles)?;
    fs::write(&path, content)
        .with_context(|| format!("Failed to write subagents to {:?}", path))?;
    Ok(())
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
            _ => break, // Exit on Esc
        };

        match choice_idx {
            0 => {
                if let Err(e) = manage_menu(&config).await {
                    eprintln!("Error managing subagents: {}", e);
                }
            }
            1 => {
                if let Err(e) = create_menu(&config).await {
                    eprintln!("Error creating subagent: {}", e);
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
                println!("Primary Model: {}", profile.model);
                println!("Fallback Models: {:?}", profile.fallbacks);
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
                        modified.model = Text::new("Edit Primary Model:")
                            .with_initial_value(&profile.model)
                            .prompt()?;
                        
                        let mut fallbacks = Vec::new();
                        for idx in 1..=3 {
                            let default_val = profile.fallbacks.get(idx - 1).cloned().unwrap_or_default();
                            let fallback = Text::new(&format!("Edit Fallback Model {} (Leave empty to skip):", idx))
                                .with_initial_value(&default_val)
                                .prompt()?;
                            if !fallback.trim().is_empty() {
                                fallbacks.push(fallback.trim().to_string());
                            }
                        }
                        modified.fallbacks = fallbacks;

                        profiles[pos] = modified;
                        save_profiles(&profiles)?;
                        println!("✅ Subagent modified successfully.");
                    }
                    1 => {
                        let confirm = Confirm::new(&format!("Are you sure you want to delete {}?", profile.name))
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
                let model = Text::new("Enter Primary Model Name (e.g. gpt-4o-mini):").prompt()?;
                
                let mut fallbacks = Vec::new();
                for idx in 1..=3 {
                    let fallback = Text::new(&format!("Enter Fallback Model {} (Leave empty to skip):", idx)).prompt()?;
                    if !fallback.trim().is_empty() {
                        fallbacks.push(fallback.trim().to_string());
                    }
                }

                let new_profile = SubagentProfile {
                    name: name.trim().to_string(),
                    description,
                    system_prompt,
                    model,
                    fallbacks,
                };

                let mut profiles = load_profiles()?;
                profiles.push(new_profile);
                save_profiles(&profiles)?;
                println!("✅ Subagent manual creation complete.");
                break;
            }
            1 => {
                let task_description = Text::new("Describe the specific task or role you want this subagent to perform:").prompt()?;
                if task_description.trim().is_empty() {
                    return Err(anyhow!("Description cannot be empty."));
                }

                println!("🧠 Asking OpenZ to design this subagent for you...");
                let ai_designed = ask_openz_to_design(config, &task_description).await?;

                println!("\n--- AI Designed Subagent Proposed ---");
                println!("Name: {}", ai_designed.name);
                println!("Description: {}", ai_designed.description);
                println!("Primary Model: {}", ai_designed.model);
                println!("Fallback Models: {:?}", ai_designed.fallbacks);
                println!("System Prompt:\n{}", ai_designed.system_prompt);
                println!("------------------------------------");

                let save_choice = Confirm::new("Save this AI-designed subagent?").with_default(true).prompt()?;
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
          \"system_prompt\": \"Detailed system prompt containing instructions, rules, and formats for this agent\",\n\
          \"model\": \"gpt-4o-mini\",\n\
          \"fallbacks\": [\"gpt-4o\", \"claude-3-5-haiku\"]\n\
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

    let resp = provider.chat(system_prompt, &messages, &[], &settings).await?;
    let content = resp.content.ok_or_else(|| anyhow!("No design returned from AI"))?;

    // Parse JSON safely
    let cleaned_content = content
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim()
        .to_string();

    let ai_profile: SubagentProfile = serde_json::from_str(&cleaned_content)
        .with_context(|| format!("Failed to parse AI response as SubagentProfile. Response was: {}", content))?;

    Ok(ai_profile)
}
