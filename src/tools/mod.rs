use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use crate::config::schema::Config;
use crate::providers::LLMProvider;
use crate::session::SessionManager;
use crate::tools::subagent::CancellationToken;

#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> serde_json::Value;
    async fn call(&self, arguments: &serde_json::Value) -> Result<serde_json::Value>;
}

#[derive(Clone)]
pub struct ToolRegistry {
    static_tools: HashMap<String, Arc<dyn Tool>>,
    pub context: Option<(Config, Arc<dyn LLMProvider>, SessionManager)>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {
    pub fn new() -> Self {
        ToolRegistry {
            static_tools: HashMap::new(),
            context: None,
        }
    }

    pub fn new_with_context(config: Config, provider: Arc<dyn LLMProvider>, session_manager: SessionManager) -> Self {
        ToolRegistry {
            static_tools: HashMap::new(),
            context: Some((config, provider, session_manager)),
        }
    }

    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        self.static_tools.insert(tool.name().to_string(), tool);
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        // 1. If name is "delegate_task", override and inject parent tools dynamically
        if name == "delegate_task" {
            let (config, provider, session_manager) = self.context.as_ref()?;
            let mut parent_tools = Vec::new();
            for tool in self.static_tools.values() {
                if tool.name() != "delegate_task" && tool.name() != "parallel_research" {
                    parent_tools.push(tool.clone());
                }
            }
            return Some(Arc::new(crate::tools::subagent::DelegateTaskTool {
                config: config.clone(),
                parent_provider: provider.clone(),
                session_manager: session_manager.clone(),
                parent_tools,
                cancellation_token: CancellationToken::new(),
            }));
        }

        // 1b. If name is "parallel_research", override and inject parent tools dynamically
        if name == "parallel_research" {
            let (config, provider, session_manager) = self.context.as_ref()?;
            let mut parent_tools = Vec::new();
            for tool in self.static_tools.values() {
                if tool.name() != "delegate_task" && tool.name() != "parallel_research" {
                    parent_tools.push(tool.clone());
                }
            }
            return Some(Arc::new(crate::tools::subagent::ParallelResearchTool {
                config: config.clone(),
                parent_provider: provider.clone(),
                session_manager: session_manager.clone(),
                parent_tools,
                cancellation_token: CancellationToken::new(),
            }));
        }

        // 1c. If name is "evaluator_optimizer_loop", override and inject parent tools dynamically
        if name == "evaluator_optimizer_loop" {
            let (config, provider, session_manager) = self.context.as_ref()?;
            let mut parent_tools = Vec::new();
            for tool in self.static_tools.values() {
                if tool.name() != "delegate_task" && tool.name() != "parallel_research" && tool.name() != "evaluator_optimizer_loop" {
                    parent_tools.push(tool.clone());
                }
            }
            return Some(Arc::new(crate::tools::subagent::EvaluatorOptimizerLoopTool {
                config: config.clone(),
                parent_provider: provider.clone(),
                session_manager: session_manager.clone(),
                parent_tools,
                cancellation_token: CancellationToken::new(),
            }));
        }

        // 2. Check static tools
        if let Some(tool) = self.static_tools.get(name) {
            return Some(tool.clone());
        }

        // 3. If not found, check if it matches a custom subagent profile dynamically
        let (config, provider, session_manager) = self.context.as_ref()?;
        let profiles = crate::subagents::load_profiles().ok()?;
        let profile = profiles.into_iter().find(|p| p.name == name)?;

        let mut parent_tools = Vec::new();
        for tool in self.static_tools.values() {
            if tool.name() != "delegate_task" && tool.name() != "parallel_research" {
                parent_tools.push(tool.clone());
            }
        }

        Some(Arc::new(crate::tools::subagent::DelegateProfileTool {
            config: config.clone(),
            parent_provider: provider.clone(),
            session_manager: session_manager.clone(),
            profile,
            parent_tools,
            cancellation_token: CancellationToken::new(),
        }))
    }

    pub fn get_static_tools(&self) -> Vec<Arc<dyn Tool>> {
        self.static_tools.values().cloned().collect()
    }

    pub fn to_openai_format(&self) -> Vec<serde_json::Value> {
        let mut tools_list: Vec<serde_json::Value> = self.static_tools.values().map(|t| {
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": t.name(),
                    "description": t.description(),
                    "parameters": t.parameters(),
                }
            })
        }).collect();

        // Add custom subagents from subagents.json dynamically
        if let Some((_, _, _)) = &self.context {
            if let Ok(profiles) = crate::subagents::load_profiles() {
                for profile in profiles {
                    if !self.static_tools.contains_key(&profile.name) {
                        tools_list.push(serde_json::json!({
                            "type": "function",
                            "function": {
                                "name": profile.name,
                                "description": profile.description,
                                "parameters": serde_json::json!({
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
                        }));
                    }
                }
            }
        }

        if tools_list.len() > 128 {
            tracing::warn!("Too many tools registered ({}); truncating to 128 to satisfy API limits.", tools_list.len());
            tools_list.truncate(128);
        }

        tools_list
    }
}

pub mod filesystem;
pub mod shell;
pub mod web;
pub mod mcp;
pub mod subagent;
pub mod cron;
pub mod remote;
pub mod mcp_manager;
pub mod grep;
pub mod git_manager;
pub mod outline;
pub mod db_inspector;
pub mod cargo_manager;
pub mod clipboard;
pub mod open;
pub mod watcher;
pub mod ast_grep;
pub mod gsd_browser;
pub mod web_search;
pub mod onpkg;
pub mod doc_reader;
pub mod wasm_sandbox;
pub mod js_format;
pub mod semantic_search;
pub mod rust_docs;
pub mod image_generator;
pub mod system_info;
pub mod network;
pub mod browser_common;
pub mod crawl;
pub mod obscura;
pub mod shared_memory;
pub mod firefox;
pub mod notes;
pub mod social_search;
pub mod template_compiler;
pub mod mermaid;
pub mod video;
pub mod sop;
pub mod svg_animator;
pub mod compiler_auto_heal;
pub mod html_video;
