use crate::tools::Tool;
use crate::tools::ToolRegistry;
use crate::agent::style::*;
use crate::agent::AgentLoop;
use crate::config::schema::Config;
use crate::providers::LLMProvider;
use crate::providers::openai::OpenAIProvider;
use crate::providers::anthropic::AnthropicProvider;
use crate::session::SessionManager;
use crate::subagents::SubagentProfile;
use anyhow::{Result, anyhow};
use std::sync::Arc;
use serde_json::Value;

pub struct DelegateTaskTool {
    pub config: Config,
    pub parent_provider: Arc<dyn LLMProvider>,
    pub session_manager: SessionManager,
    pub parent_tools: Vec<Arc<dyn Tool>>,
}

#[async_trait::async_trait]
impl Tool for DelegateTaskTool {
    fn name(&self) -> &str {
        "delegate_task"
    }

    fn description(&self) -> &str {
        "Delegate a specific subtask or research item to a focused subagent. The subagent runs in an isolated workspace, executes tools to accomplish the goal, and returns a summary."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "goal": {
                    "type": "string",
                    "description": "The specific goal/task for the subagent to accomplish. Be clear and detailed."
                },
                "context": {
                    "type": "string",
                    "description": "Additional context, details, files, or background information needed for the task."
                },
                "model": {
                    "type": "string",
                    "description": "Optional model override name (e.g., 'gpt-4o-mini', 'claude-3-5-haiku') for the subagent."
                }
            },
            "required": ["goal"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let goal = arguments.get("goal").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'goal' argument"))?;
        let context = arguments.get("context").and_then(|v| v.as_str()).unwrap_or("");
        let model_override = arguments.get("model").and_then(|v| v.as_str());

        let provider = if let Some(m) = model_override {
            match build_provider_for_model(&self.config, m) {
                Ok(p) => p,
                Err(e) => {
                    crate::tui_println!("{}⚠️ Failed to configure subagent model '{}' ({}). Falling back to parent model.{}", AURA_GOLD, m, e, COLOR_RESET);
                    self.parent_provider.clone()
                }
            }
        } else {
            self.parent_provider.clone()
        };

        let mut child_registry = ToolRegistry::new_with_context(
            self.config.clone(),
            provider.clone(),
            self.session_manager.clone(),
        );
        for tool in &self.parent_tools {
            child_registry.register(tool.clone());
        }
        child_registry.register(std::sync::Arc::new(DelegateTaskTool {
            config: self.config.clone(),
            parent_provider: provider.clone(),
            session_manager: self.session_manager.clone(),
            parent_tools: self.parent_tools.clone(),
        }));

        let child_session_id = format!("subagent:{}", &uuid::Uuid::new_v4().to_string()[..8]);
        let child_agent = AgentLoop::new(
            self.config.clone(),
            provider,
            child_registry,
            self.session_manager.clone(),
        );

        let subagent_prompt = format!(
            "You are a focused subagent. Complete the following task using the tools available.\n\n\
            TASK:\n{}\n\n\
            CONTEXT:\n{}\n\n\
            When finished, provide a clear, concise summary of what you did and found.",
            goal, context
        );

        let branch_id = format!("branch_{}", &uuid::Uuid::new_v4().to_string()[..8]);
        let mut has_memory_mcp = false;
        if let Some(client) = crate::tools::mcp::get_memory_mcp_client() {
            match client.call_tool("create_database_branch", &serde_json::json!({ "branchId": branch_id })).await {
                Ok(_) => {
                    crate::tui_println!("{}  ✓ Isolated simulation space branch '{}' created{}", EMERALD_GREEN, branch_id, COLOR_RESET);
                    has_memory_mcp = true;
                }
                Err(e) => {
                    tracing::warn!("Failed to create database branch: {:?}", e);
                }
            }
        }

        crate::tui_println!("{}◎ Subagent{}", AURA_PURPLE, COLOR_RESET);
        let spinner_msg = format!("{}  Running...{}", AURA_SLATE, COLOR_RESET);
        let run_res = with_spinner(&spinner_msg, child_agent.run(&subagent_prompt, &child_session_id)).await;

        if has_memory_mcp {
            if let Some(client) = crate::tools::mcp::get_memory_mcp_client() {
                if run_res.is_ok() {
                    match client.call_tool("commit_database_branch", &serde_json::json!({})).await {
                        Ok(_) => crate::tui_println!("{}  ✓ Committed simulation space branch '{}'{}", EMERALD_GREEN, branch_id, COLOR_RESET),
                        Err(e) => tracing::warn!("Failed to commit database branch: {:?}", e),
                    }
                } else {
                    match client.call_tool("rollback_database_branch", &serde_json::json!({})).await {
                        Ok(_) => crate::tui_println!("{}  ✓ Rolled back simulation space branch '{}'{}", AURA_GOLD, branch_id, COLOR_RESET),
                        Err(e) => tracing::warn!("Failed to rollback database branch: {:?}", e),
                    }
                }
            }
        }

        let run_res = run_res?;
        crate::tui_println!("{}  ✓ Complete{}", EMERALD_GREEN, COLOR_RESET);

        Ok(serde_json::json!({
            "status": "success",
            "session_id": child_session_id,
            "summary": run_res.content
        }))
    }
}

pub struct DelegateProfileTool {
    pub config: Config,
    pub parent_provider: Arc<dyn LLMProvider>,
    pub session_manager: SessionManager,
    pub profile: SubagentProfile,
    pub parent_tools: Vec<Arc<dyn Tool>>,
}

#[async_trait::async_trait]
impl Tool for DelegateProfileTool {
    fn name(&self) -> &str {
        &self.profile.name
    }

    fn description(&self) -> &str {
        &self.profile.description
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "goal": {
                    "type": "string",
                    "description": "The specific goal or task for this specialized subagent to accomplish."
                },
                "context": {
                    "type": "string",
                    "description": "Additional context or background details required for the task."
                }
            },
            "required": ["goal"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let goal = arguments.get("goal").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'goal' argument"))?;
        let context = arguments.get("context").and_then(|v| v.as_str()).unwrap_or("");

        let mut models_to_try = vec![self.profile.model.clone()];
        for fallback in &self.profile.fallbacks {
            if !fallback.trim().is_empty() {
                models_to_try.push(fallback.trim().to_string());
            }
        }

        let default_model = self.config.agents.defaults.model.clone();
        if !models_to_try.contains(&default_model) {
            models_to_try.push(default_model);
        }

        let child_session_id = format!("subagent:{}:{}", self.profile.name, &uuid::Uuid::new_v4().to_string()[..8]);
        let subagent_prompt = format!(
            "You are a specialized subagent operating under the following profile guidelines:\n\n\
            {}\n\n\
            TASK:\n{}\n\n\
            CONTEXT:\n{}\n\n\
            When finished, provide a clear, concise summary of what you did and found.",
            self.profile.system_prompt, goal, context
        );

        let mut last_error = None;
        for (idx, model_name) in models_to_try.iter().enumerate() {
            if idx > 0 {
                crate::tui_println!("{}▲ Primary model failed. Trying fallback model ({} of {}): {}{}", AURA_GOLD, idx, models_to_try.len() - 1, model_name, COLOR_RESET);
            }

            let provider = match build_provider_for_model(&self.config, model_name) {
                Ok(p) => p,
                Err(e) => {
                    last_error = Some(e);
                    continue;
                }
            };

            let mut child_registry = ToolRegistry::new_with_context(
                self.config.clone(),
                provider.clone(),
                self.session_manager.clone(),
            );
            for tool in &self.parent_tools {
                child_registry.register(tool.clone());
            }
            child_registry.register(std::sync::Arc::new(DelegateTaskTool {
                config: self.config.clone(),
                parent_provider: provider.clone(),
                session_manager: self.session_manager.clone(),
                parent_tools: self.parent_tools.clone(),
            }));

            let child_agent = AgentLoop::new(
                self.config.clone(),
                provider,
                child_registry,
                self.session_manager.clone(),
            );

            let formatted_name = format_subagent_name(&self.profile.name);
            let is_vision = self.profile.name == "vision_agent";
            let is_reviewer = self.profile.name == "reviewer";

            if is_reviewer {
                let spinner_msg = format!("{}◇ Reviewing...{}", AURA_PURPLE, COLOR_RESET);
                let branch_id = format!("branch_{}", &uuid::Uuid::new_v4().to_string()[..8]);
                let mut has_memory_mcp = false;
                if let Some(client) = crate::tools::mcp::get_memory_mcp_client() {
                    match client.call_tool("create_database_branch", &serde_json::json!({ "branchId": branch_id })).await {
                        Ok(_) => {
                            crate::tui_println!("{}  ✓ Isolated simulation space branch '{}' created{}", EMERALD_GREEN, branch_id, COLOR_RESET);
                            has_memory_mcp = true;
                        }
                        Err(e) => {
                            tracing::warn!("Failed to create database branch: {:?}", e);
                        }
                    }
                }

                match with_spinner(&spinner_msg, child_agent.run(&subagent_prompt, &child_session_id)).await {
                    Ok(run_res) => {
                        if has_memory_mcp {
                            if let Some(client) = crate::tools::mcp::get_memory_mcp_client() {
                                match client.call_tool("commit_database_branch", &serde_json::json!({})).await {
                                    Ok(_) => crate::tui_println!("{}  ✓ Committed simulation space branch '{}'{}", EMERALD_GREEN, branch_id, COLOR_RESET),
                                    Err(e) => tracing::warn!("Failed to commit database branch: {:?}", e),
                                }
                            }
                        }
                        crate::tui_println!("{}✓ Complete{}", EMERALD_GREEN, COLOR_RESET);
                        return Ok(serde_json::json!({
                            "status": "success",
                            "session_id": child_session_id,
                            "model_used": model_name,
                            "summary": run_res.content
                        }));
                    }
                    Err(e) => {
                        if has_memory_mcp {
                            if let Some(client) = crate::tools::mcp::get_memory_mcp_client() {
                                match client.call_tool("rollback_database_branch", &serde_json::json!({})).await {
                                    Ok(_) => crate::tui_println!("{}  ✓ Rolled back simulation space branch '{}'{}", AURA_GOLD, branch_id, COLOR_RESET),
                                    Err(e) => tracing::warn!("Failed to rollback database branch: {:?}", e),
                                }
                            }
                        }
                        crate::tui_println!("{}✕ Error: Model '{}' execution failed: {}{}", ERROR_RED, model_name, e, COLOR_RESET);
                        last_error = Some(e);
                    }
                }
            } else {
                if is_vision {
                    crate::tui_println!("{}◎ Vision Agent{}", AURA_PURPLE, COLOR_RESET);
                } else {
                    crate::tui_println!("{}◎ {}{}", AURA_PURPLE, formatted_name, COLOR_RESET);
                }

                let spinner_msg = if is_vision {
                    format!("{}  Processing image...{}", AURA_SLATE, COLOR_RESET)
                } else {
                    format!("{}  Running...{}", AURA_SLATE, COLOR_RESET)
                };

                let branch_id = format!("branch_{}", &uuid::Uuid::new_v4().to_string()[..8]);
                let mut has_memory_mcp = false;
                if let Some(client) = crate::tools::mcp::get_memory_mcp_client() {
                    match client.call_tool("create_database_branch", &serde_json::json!({ "branchId": branch_id })).await {
                        Ok(_) => {
                            crate::tui_println!("{}  ✓ Isolated simulation space branch '{}' created{}", EMERALD_GREEN, branch_id, COLOR_RESET);
                            has_memory_mcp = true;
                        }
                        Err(e) => {
                            tracing::warn!("Failed to create database branch: {:?}", e);
                        }
                    }
                }

                match with_spinner(&spinner_msg, child_agent.run(&subagent_prompt, &child_session_id)).await {
                    Ok(run_res) => {
                        if has_memory_mcp {
                            if let Some(client) = crate::tools::mcp::get_memory_mcp_client() {
                                match client.call_tool("commit_database_branch", &serde_json::json!({})).await {
                                    Ok(_) => crate::tui_println!("{}  ✓ Committed simulation space branch '{}'{}", EMERALD_GREEN, branch_id, COLOR_RESET),
                                    Err(e) => tracing::warn!("Failed to commit database branch: {:?}", e),
                                }
                            }
                        }
                        crate::tui_println!("{}  ✓ Complete{}", EMERALD_GREEN, COLOR_RESET);
                        return Ok(serde_json::json!({
                            "status": "success",
                            "session_id": child_session_id,
                            "model_used": model_name,
                            "summary": run_res.content
                        }));
                    }
                    Err(e) => {
                        if has_memory_mcp {
                            if let Some(client) = crate::tools::mcp::get_memory_mcp_client() {
                                match client.call_tool("rollback_database_branch", &serde_json::json!({})).await {
                                    Ok(_) => crate::tui_println!("{}  ✓ Rolled back simulation space branch '{}'{}", AURA_GOLD, branch_id, COLOR_RESET),
                                    Err(e) => tracing::warn!("Failed to rollback database branch: {:?}", e),
                                }
                            }
                        }
                        crate::tui_println!("{}✕ Error: Model '{}' execution failed: {}{}", ERROR_RED, model_name, e, COLOR_RESET);
                        last_error = Some(e);
                    }
                }
            }
        }

        Err(anyhow!("All configured models/fallbacks failed for subagent '{}'. Last error: {:?}", self.profile.name, last_error))
    }
}

pub fn build_provider_for_model(config: &Config, model: &str) -> Result<Arc<dyn LLMProvider>> {
    let defaults = &config.agents.defaults;
    let mut provider_name = defaults.provider.clone();
    let mut clean_model = model;

    let model_lower = model.to_lowercase();

    // 1. Check for explicit provider prefixes
    if model_lower.starts_with("openrouter/") {
        provider_name = "openrouter".to_string();
        clean_model = &model["openrouter/".len()..];
    } else if model_lower.starts_with("ollama/") {
        provider_name = "ollama".to_string();
        clean_model = &model["ollama/".len()..];
    } else if model_lower.starts_with("anthropic/") {
        provider_name = "anthropic".to_string();
        clean_model = &model["anthropic/".len()..];
    } else if model_lower.starts_with("openai/") {
        provider_name = "openai".to_string();
        clean_model = &model["openai/".len()..];
    } else if model_lower.starts_with("deepseek/") {
        provider_name = "deepseek".to_string();
        clean_model = &model["deepseek/".len()..];
    } else if model_lower.starts_with("groq/") {
        provider_name = "groq".to_string();
        clean_model = &model["groq/".len()..];
    } else if model_lower.starts_with("google_ai_studio/") {
        provider_name = "google_ai_studio".to_string();
        clean_model = &model["google_ai_studio/".len()..];
    } else if model_lower.starts_with("google-ai-studio/") {
        provider_name = "google_ai_studio".to_string();
        clean_model = &model["google-ai-studio/".len()..];
    } else if model_lower.starts_with("opencode_zen/") {
        provider_name = "opencode_zen".to_string();
        clean_model = &model["opencode_zen/".len()..];
    } else if model_lower.starts_with("opencode-zen/") {
        provider_name = "opencode_zen".to_string();
        clean_model = &model["opencode-zen/".len()..];
    } else if model_lower.starts_with("z.ai/") {
        provider_name = "z.ai".to_string();
        clean_model = &model["z.ai/".len()..];
    } else if model_lower.starts_with("z_ai/") {
        provider_name = "z.ai".to_string();
        clean_model = &model["z_ai/".len()..];
    } else if model_lower.starts_with("nvidia/") {
        provider_name = "nvidia".to_string();
        clean_model = &model["nvidia/".len()..];
    } else if model_lower.starts_with("minimax/") {
        provider_name = "minimax".to_string();
        clean_model = &model["minimax/".len()..];
    } else if model_lower.starts_with("mistral/") {
        provider_name = "mistral".to_string();
        clean_model = &model["mistral/".len()..];
    } else if model_lower.starts_with("cerebres/") {
        provider_name = "cerebres".to_string();
        clean_model = &model["cerebres/".len()..];
    } else if model_lower.starts_with("cerebras/") {
        provider_name = "cerebres".to_string();
        clean_model = &model["cerebras/".len()..];
    } else if provider_name == "auto" {
        // 2. Resolve based on model name keywords
        let has_key = |prov: &str| -> bool {
            match prov {
                "anthropic" => config.providers.anthropic.as_ref().and_then(|p| p.api_key.as_ref()).is_some() || std::env::var("ANTHROPIC_API_KEY").is_ok(),
                "openai" => config.providers.openai.as_ref().and_then(|p| p.api_key.as_ref()).is_some() || std::env::var("OPENAI_API_KEY").is_ok(),
                "deepseek" => config.providers.deepseek.as_ref().and_then(|p| p.api_key.as_ref()).is_some() || std::env::var("DEEPSEEK_API_KEY").is_ok(),
                "groq" => config.providers.groq.as_ref().and_then(|p| p.api_key.as_ref()).is_some() || std::env::var("GROQ_API_KEY").is_ok(),
                "openrouter" => config.providers.openrouter.as_ref().and_then(|p| p.api_key.as_ref()).is_some() || std::env::var("OPENROUTER_API_KEY").is_ok(),
                "opencode_zen" => config.providers.opencode_zen.as_ref().and_then(|p| p.api_key.as_ref()).is_some() || std::env::var("OPENCODE_ZEN_API_KEY").is_ok(),
                "google_ai_studio" => config.providers.google_ai_studio.as_ref().and_then(|p| p.api_key.as_ref()).is_some() || std::env::var("GOOGLE_AI_STUDIO_API_KEY").is_ok(),
                "z.ai" => config.providers.z_ai.as_ref().and_then(|p| p.api_key.as_ref()).is_some() || std::env::var("Z_AI_API_KEY").is_ok(),
                "nvidia" => config.providers.nvidia.as_ref().and_then(|p| p.api_key.as_ref()).is_some() || std::env::var("NVIDIA_API_KEY").is_ok(),
                "minimax" => config.providers.minimax.as_ref().and_then(|p| p.api_key.as_ref()).is_some() || std::env::var("MINIMAX_API_KEY").is_ok(),
                "mistral" => config.providers.mistral.as_ref().and_then(|p| p.api_key.as_ref()).is_some() || std::env::var("MISTRAL_API_KEY").is_ok(),
                "cerebres" => config.providers.cerebres.as_ref().and_then(|p| p.api_key.as_ref()).is_some() || std::env::var("CEREBRES_API_KEY").is_ok(),
                _ => false,
            }
        };

        if model_lower.contains("claude") {
            if has_key("anthropic") {
                provider_name = "anthropic".to_string();
            } else if has_key("opencode_zen") {
                provider_name = "opencode_zen".to_string();
            } else if has_key("openrouter") {
                provider_name = "openrouter".to_string();
            } else {
                provider_name = "anthropic".to_string();
            }
        } else if model_lower.contains("gpt") {
            if has_key("openai") {
                provider_name = "openai".to_string();
            } else if has_key("opencode_zen") {
                provider_name = "opencode_zen".to_string();
            } else if has_key("openrouter") {
                provider_name = "openrouter".to_string();
            } else {
                provider_name = "openai".to_string();
            }
        } else if model_lower.contains("deepseek") {
            if has_key("deepseek") {
                provider_name = "deepseek".to_string();
            } else if has_key("opencode_zen") {
                provider_name = "opencode_zen".to_string();
            } else if has_key("openrouter") {
                provider_name = "openrouter".to_string();
            } else {
                provider_name = "deepseek".to_string();
            }
        } else if model_lower.contains("gemini") {
            if has_key("google_ai_studio") {
                provider_name = "google_ai_studio".to_string();
            } else if has_key("opencode_zen") {
                provider_name = "opencode_zen".to_string();
            } else if has_key("openrouter") {
                provider_name = "openrouter".to_string();
            } else {
                provider_name = "google_ai_studio".to_string();
            }
        } else if model_lower.contains("gemma") {
            if has_key("google_ai_studio") {
                provider_name = "google_ai_studio".to_string();
            } else if has_key("openrouter") {
                provider_name = "openrouter".to_string();
            } else if has_key("opencode_zen") {
                provider_name = "opencode_zen".to_string();
            } else {
                provider_name = "google_ai_studio".to_string();
            }
        } else if model_lower.contains("mistral") || model_lower.contains("codestral") {
            if has_key("mistral") {
                provider_name = "mistral".to_string();
            } else if has_key("openrouter") {
                provider_name = "openrouter".to_string();
            } else if has_key("opencode_zen") {
                provider_name = "opencode_zen".to_string();
            } else {
                provider_name = "mistral".to_string();
            }
        } else if model_lower.contains("ollama") {
            provider_name = "ollama".to_string();
        } else {
            if has_key("opencode_zen") {
                provider_name = "opencode_zen".to_string();
            } else if has_key("google_ai_studio") {
                provider_name = "google_ai_studio".to_string();
            } else if has_key("anthropic") {
                provider_name = "anthropic".to_string();
            } else if has_key("openai") {
                provider_name = "openai".to_string();
            } else if has_key("deepseek") {
                provider_name = "deepseek".to_string();
            } else if has_key("openrouter") {
                provider_name = "openrouter".to_string();
            } else if has_key("groq") {
                provider_name = "groq".to_string();
            } else if has_key("mistral") {
                provider_name = "mistral".to_string();
            } else if has_key("nvidia") {
                provider_name = "nvidia".to_string();
            } else if has_key("z.ai") {
                provider_name = "z.ai".to_string();
            } else {
                provider_name = "openai".to_string();
            }
        }
    }

    let (api_key, api_base) = match provider_name.as_str() {
        "anthropic" => {
            let p = config.providers.anthropic.as_ref();
            let key = p.and_then(|x| x.api_key.clone())
                .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok())
                .unwrap_or_default();
            let base = p.and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "https://api.anthropic.com".to_string());
            (key, base)
        }
        "openai" => {
            let p = config.providers.openai.as_ref();
            let key = p.and_then(|x| x.api_key.clone())
                .or_else(|| std::env::var("OPENAI_API_KEY").ok())
                .unwrap_or_default();
            let base = p.and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
            (key, base)
        }
        "openrouter" => {
            let p = config.providers.openrouter.as_ref();
            let key = p.and_then(|x| x.api_key.clone())
                .or_else(|| std::env::var("OPENROUTER_API_KEY").ok())
                .unwrap_or_default();
            let base = p.and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "https://openrouter.ai/api/v1".to_string());
            (key, base)
        }
        "deepseek" => {
            let p = config.providers.deepseek.as_ref();
            let key = p.and_then(|x| x.api_key.clone())
                .or_else(|| std::env::var("DEEPSEEK_API_KEY").ok())
                .unwrap_or_default();
            let base = p.and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "https://api.deepseek.com/v1".to_string());
            (key, base)
        }
        "groq" => {
            let p = config.providers.groq.as_ref();
            let key = p.and_then(|x| x.api_key.clone())
                .or_else(|| std::env::var("GROQ_API_KEY").ok())
                .unwrap_or_default();
            let base = p.and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "https://api.groq.com/openai/v1".to_string());
            (key, base)
        }
        "ollama" => {
            let p = config.providers.ollama.as_ref();
            let base = p.and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "http://localhost:11434/v1".to_string());
            (String::new(), base)
        }
        "minimax" => {
            let p = config.providers.minimax.as_ref();
            let key = p.and_then(|x| x.api_key.clone())
                .or_else(|| std::env::var("MINIMAX_API_KEY").ok())
                .unwrap_or_default();
            let base = p.and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "https://api.minimax.io/v1".to_string());
            (key, base)
        }
        "mistral" => {
            let p = config.providers.mistral.as_ref();
            let key = p.and_then(|x| x.api_key.clone())
                .or_else(|| std::env::var("MISTRAL_API_KEY").ok())
                .unwrap_or_default();
            let base = p.and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "https://api.mistral.ai/v1".to_string());
            (key, base)
        }
        "z.ai" => {
            let p = config.providers.z_ai.as_ref();
            let key = p.and_then(|x| x.api_key.clone())
                .or_else(|| std::env::var("Z_AI_API_KEY").ok())
                .unwrap_or_default();
            let base = p.and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "https://api.z.ai/api/paas/v4/".to_string());
            (key, base)
        }
        "nvidia" => {
            let p = config.providers.nvidia.as_ref();
            let key = p.and_then(|x| x.api_key.clone())
                .or_else(|| std::env::var("NVIDIA_API_KEY").ok())
                .unwrap_or_default();
            let base = p.and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "https://integrate.api.nvidia.com/v1".to_string());
            (key, base)
        }
        "opencode_zen" | "opencode zen" => {
            let p = config.providers.opencode_zen.as_ref();
            let key = p.and_then(|x| x.api_key.clone())
                .or_else(|| std::env::var("OPENCODE_ZEN_API_KEY").ok())
                .unwrap_or_default();
            let base = p.and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "https://opencode.ai/zen/v1".to_string());
            (key, base)
        }
        "cerebres" => {
            let p = config.providers.cerebres.as_ref();
            let key = p.and_then(|x| x.api_key.clone())
                .or_else(|| std::env::var("CEREBRES_API_KEY").ok())
                .unwrap_or_default();
            let base = p.and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "https://api.cerebras.ai/v1".to_string());
            (key, base)
        }
        "google_ai_studio" | "google ai studio" => {
            let p = config.providers.google_ai_studio.as_ref();
            let key = p.and_then(|x| x.api_key.clone())
                .or_else(|| std::env::var("GOOGLE_AI_STUDIO_API_KEY").ok())
                .unwrap_or_default();
            let base = p.and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "https://generativelanguage.googleapis.com/v1beta/openai/".to_string());
            (key, base)
        }
        _ => {
            return Err(anyhow!("Unsupported provider: {}", provider_name));
        }
    };

    if provider_name != "ollama" && api_key.is_empty() {
        return Err(anyhow!("No API key configured for provider: {}", provider_name));
    }

    let provider: Arc<dyn LLMProvider> = if provider_name == "anthropic" {
        Arc::new(AnthropicProvider::new(api_key, api_base, clean_model.to_string()))
    } else {
        Arc::new(OpenAIProvider::new(api_key, api_base, clean_model.to_string()))
    };

    Ok(provider)
}

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
        let default_model = self.config.agents.defaults.model.clone();
        let model = arguments.get("model").and_then(|v| v.as_str()).unwrap_or(&default_model).trim().to_string();

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
        if fallbacks.is_empty() {
            fallbacks = vec!["gpt-4o".to_string(), "claude-3-5-haiku".to_string()];
        }

        // Validate name format: starts with a letter, lowercase alphanumeric and underscore only
        if name.is_empty() || !name.chars().next().unwrap().is_ascii_alphabetic() || name.chars().any(|c| !c.is_ascii_lowercase() && !c.is_ascii_digit() && c != '_') {
            return Err(anyhow!("Subagent name must start with a letter and contain only lowercase alphanumeric characters and underscores."));
        }

        // Do not allow overwriting default subagents
        let defaults = ["planner", "researcher", "architect", "skill_creator", "reviewer", "code_auditor", "debugger", "test_engineer", "devops_agent", "refactor_agent", "memory_manager", "vision_agent", "documentation_agent", "self_improvement", "skill_improvement", "openz_maintainer", "mcps_manager"];
        if defaults.contains(&name.as_str()) {
            return Err(anyhow!("Cannot overwrite default subagent '{}'", name));
        }

        let mut profiles = crate::subagents::load_profiles()?;
        let profile = SubagentProfile {
            name: name.clone(),
            description,
            system_prompt,
            model,
            fallbacks,
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

        let defaults = ["planner", "researcher", "architect", "skill_creator", "reviewer", "code_auditor", "debugger", "test_engineer", "devops_agent", "refactor_agent", "memory_manager", "vision_agent", "documentation_agent", "self_improvement", "skill_improvement", "openz_maintainer", "mcps_manager"];
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

fn format_subagent_name(name: &str) -> String {
    match name {
        "vision_agent" => "Vision Agent".to_string(),
        "documentation_agent" => "Documentation Agent".to_string(),
        "self_improvement" => "Self Improvement".to_string(),
        "skill_improvement" => "Skill Improvement".to_string(),
        "openz_maintainer" => "OpenZ Maintainer".to_string(),
        "mcps_manager" => "MCPs Manager".to_string(),
        "memory_manager" => "Memory Manager".to_string(),
        "code_auditor" => "Code Auditor".to_string(),
        "test_engineer" => "Test Engineer".to_string(),
        "devops_agent" => "Devops Agent".to_string(),
        "refactor_agent" => "Refactor Agent".to_string(),
        "skill_creator" => "Skill Creator".to_string(),
        _ => {
            let mut chars = name.chars();
            match chars.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
            }
        }
    }
}

