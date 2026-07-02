use crate::tools::Tool;
use crate::agent::style::*;
use crate::config::schema::Config;
use crate::providers::LLMProvider;
use crate::subagents::SubagentProfile;
use anyhow::{Result, anyhow};
use std::sync::Arc;
use serde_json::Value;

pub struct OptimizeSubagentTool {
    pub config: Config,
    pub parent_provider: Arc<dyn LLMProvider>,
}

#[async_trait::async_trait]
impl Tool for OptimizeSubagentTool {
    fn name(&self) -> &str {
        "optimize_subagent"
    }

    fn description(&self) -> &str {
        "Optimize a specialized subagent's system prompt using AI based on feedback logs or execution errors."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "subagent_name": {
                    "type": "string",
                    "description": "The name of the subagent to optimize (e.g. 'researcher', 'architect', 'reviewer')"
                },
                "feedback": {
                    "type": "string",
                    "description": "Details about the error, feedback, failed logs, or missing guidelines that occurred."
                }
            },
            "required": ["subagent_name", "feedback"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let subagent_name = arguments.get("subagent_name").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'subagent_name' argument"))?;
        let feedback = arguments.get("feedback").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'feedback' argument"))?;

        let mut profiles = crate::subagents::load_profiles()?;
        let pos = profiles.iter().position(|p| p.name == subagent_name)
            .ok_or_else(|| anyhow!("Subagent '{}' not found", subagent_name))?;

        let profile = &profiles[pos];

        let system_prompt_sum = "You are an expert prompt engineer. Optimize system prompts for specialized subagents. \
            Analyze the failed case feedback, and rewrite the subagent's system prompt to address the issue. \
            Ensure the prompt remains clear, structured, and focused. Return only the optimized system prompt, with no conversational text or markdown blocks.";

        let user_prompt = format!(
            "Subagent: {}\n\
            Current System Prompt:\n{}\n\n\
            Execution Feedback/Error:\n{}\n\n\
            Please return only the rewritten, optimized system prompt.",
            subagent_name, profile.system_prompt, feedback
        );

        let messages = vec![crate::session::Message {
            role: "user".to_string(),
            content: user_prompt,
            timestamp: Some(chrono::Utc::now().to_rfc3339()),
            extra: serde_json::Map::new(),
        }];

        let settings = crate::providers::GenerationSettings {
            temperature: 0.2,
            max_tokens: 1024,
            reasoning_effort: None,
        };

        let spinner_msg = format!(
            "{}▸ [Prompt-Optimize] Asking OpenZ to optimize subagent prompt for '{}'...{}",
            AURA_PURPLE,
            subagent_name,
            COLOR_RESET
        );
        let chat_fut = self.parent_provider.chat(system_prompt_sum, &messages, &[], &settings);
        let resp = with_spinner(&spinner_msg, chat_fut).await?;
        let content = resp.content.ok_or_else(|| anyhow!("Failed to generate optimized prompt from AI"))?;

        let clean_prompt = content.trim().to_string();
        if clean_prompt.is_empty() {
            return Err(anyhow!("Received empty optimized prompt from AI"));
        }

        profiles[pos].system_prompt = clean_prompt.clone();
        crate::subagents::save_profiles(&profiles)?;

        crate::tui_println!("{}✓ [Prompt-Optimize] Optimized prompt for '{}' saved successfully.{}", EMERALD_GREEN, subagent_name, COLOR_RESET);

        Ok(serde_json::json!({
            "status": "success",
            "message": format!("Successfully optimized subagent '{}'", subagent_name),
            "new_system_prompt": clean_prompt
        }))
    }
}

pub struct CreateSubagentTool {
    pub config: Config,
}

#[async_trait::async_trait]
impl Tool for CreateSubagentTool {
    fn name(&self) -> &str {
        "create_subagent"
    }

    fn description(&self) -> &str {
        "Create and save a new custom specialized subagent profile. The new subagent will be saved to the database and dynamically registered for future tasks."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Unique name for the subagent in lowercase alphanumeric/underscore format (e.g. 'twitter_researcher')"
                },
                "description": {
                    "type": "string",
                    "description": "A short summary of what this subagent is specialized in."
                },
                "system_prompt": {
                    "type": "string",
                    "description": "The detailed instructions and guidelines that define how this subagent operates."
                },
                "model": {
                    "type": "string",
                    "description": "Optional: The primary model to run (e.g. 'gpt-4o-mini', 'claude-3-5-sonnet', 'gpt-4o'). Default is 'gpt-4o-mini'."
                },
                "fallbacks": {
                    "type": "array",
                    "items": {
                        "type": "string"
                    },
                    "description": "Optional: Up to 3 fallback models to try if the primary model fails."
                }
            },
            "required": ["name", "description", "system_prompt"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let name = arguments.get("name").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'name' argument"))?.trim().to_string();
        let description = arguments.get("description").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'description' argument"))?.trim().to_string();
        let system_prompt = arguments.get("system_prompt").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'system_prompt' argument"))?.trim().to_string();
        let model = arguments.get("model").and_then(|v| v.as_str()).map(|s| s.trim().to_string());

        let mut fallbacks = Vec::new();
        if let Some(arr) = arguments.get("fallbacks").and_then(|v| v.as_array()) {
            for v in arr {
                if let Some(s) = v.as_str() {
                    let s_trimmed = s.trim().to_string();
                    if !s_trimmed.is_empty() {
                        fallbacks.push(s_trimmed);
                    }
                }
            }
        }
        let fallbacks_opt = if fallbacks.is_empty() {
            None
        } else {
            Some(fallbacks)
        };

        // Validate name format: starts with a letter, lowercase alphanumeric and underscore only
        if name.is_empty() || !name.chars().next().unwrap().is_ascii_alphabetic() || name.chars().any(|c| !c.is_ascii_lowercase() && !c.is_ascii_digit() && c != '_') {
            return Err(anyhow!("Subagent name must start with a letter and contain only lowercase alphanumeric characters and underscores."));
        }

        // Do not allow overwriting default subagents
        let defaults = [
            "planner", "researcher", "architect", "skill_creator", "reviewer",
            "code_auditor", "debugger", "test_engineer", "devops_agent",
            "refactor_agent", "memory_manager", "vision_agent", "documentation_agent",
            "self_improvement", "skill_improvement", "openz_maintainer", "mcps_manager",
            "git_ops_agent", "ast_searcher", "database_specialist", "browser_operator",
            "dependency_manager", "frontend_architect", "docs_lookup_agent",
            "document_compiler", "presentation_designer", "code_synthesizer",
            "summarizer_agent", "media_designer", "openz_coordinator",
            "sop_designer", "api_integrator", "performance_tuner", "communication_manager",
            "automation_agent", "coding_agent"
        ];
        if defaults.contains(&name.as_str()) {
            return Err(anyhow!("Cannot overwrite default subagent '{}'", name));
        }

        let mut profiles = crate::subagents::load_profiles()?;
        let profile = SubagentProfile {
            name: name.clone(),
            description,
            system_prompt,
            model,
            fallbacks: fallbacks_opt,
            extra: serde_json::Map::new(),
        };

        if let Some(pos) = profiles.iter().position(|p| p.name == name) {
            profiles[pos] = profile;
        } else {
            profiles.push(profile);
        }

        crate::subagents::save_profiles(&profiles)?;

        crate::tui_println!("{}✓ Custom subagent '{}' created and saved.{}", EMERALD_GREEN, name, COLOR_RESET);

        Ok(serde_json::json!({
            "status": "success",
            "message": format!("Successfully created/updated subagent '{}'", name)
        }))
    }
}

pub struct DeleteSubagentTool;

#[async_trait::async_trait]
impl Tool for DeleteSubagentTool {
    fn name(&self) -> &str {
        "delete_subagent"
    }

    fn description(&self) -> &str {
        "Delete a custom subagent profile. Crucial: Default subagents cannot be deleted."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "The name of the custom subagent to delete (e.g. 'twitter_researcher')"
                }
            },
            "required": ["name"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let name = arguments.get("name").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'name' argument"))?.trim().to_string();

        let defaults = [
            "planner", "researcher", "architect", "skill_creator", "reviewer",
            "code_auditor", "debugger", "test_engineer", "devops_agent",
            "refactor_agent", "memory_manager", "vision_agent", "documentation_agent",
            "self_improvement", "skill_improvement", "openz_maintainer", "mcps_manager",
            "git_ops_agent", "ast_searcher", "database_specialist", "browser_operator",
            "dependency_manager", "frontend_architect", "docs_lookup_agent",
            "document_compiler", "presentation_designer", "code_synthesizer",
            "summarizer_agent", "media_designer", "openz_coordinator",
            "sop_designer", "api_integrator", "performance_tuner", "communication_manager",
            "automation_agent", "coding_agent"
        ];
        if defaults.contains(&name.as_str()) {
            return Err(anyhow!("Cannot delete default subagent '{}'", name));
        }

        let mut profiles = crate::subagents::load_profiles()?;
        let pos = profiles.iter().position(|p| p.name == name)
            .ok_or_else(|| anyhow!("Custom subagent '{}' not found", name))?;

        profiles.remove(pos);
        crate::subagents::save_profiles(&profiles)?;

        crate::tui_println!("{}✓ Custom subagent '{}' deleted.{}", EMERALD_GREEN, name, COLOR_RESET);

        Ok(serde_json::json!({
            "status": "success",
            "message": format!("Successfully deleted custom subagent '{}'", name)
        }))
    }
}
