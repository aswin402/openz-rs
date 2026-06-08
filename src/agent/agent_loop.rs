use crate::config::schema::Config;
use crate::providers::{LLMProvider, GenerationSettings};
use crate::tools::ToolRegistry;
use crate::tools::subagent::DelegateTaskTool;
use crate::session::{Session, SessionManager, Message};
use anyhow::{Result, anyhow};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnState {
    Restore,
    Compact,
    Command,
    Build,
    Run,
    Save,
    Respond,
    Done,
}

pub struct AgentLoop {
    pub config: Config,
    pub provider: Arc<dyn LLMProvider>,
    pub tools: ToolRegistry,
    pub session_manager: SessionManager,
}

pub struct RunResult {
    pub content: String,
    pub tools_used: Vec<String>,
}

impl AgentLoop {
    pub fn new(
        config: Config,
        provider: Arc<dyn LLMProvider>,
        tools: ToolRegistry,
        session_manager: SessionManager,
    ) -> Self {
        AgentLoop {
            config,
            provider,
            tools,
            session_manager,
        }
    }

    pub async fn run(&self, user_content: &str, session_key: &str) -> Result<RunResult> {
        let mut state = TurnState::Restore;
        let mut session = Session::new(session_key);
        let mut messages = Vec::new();
        let mut system_prompt = String::new();
        let mut final_content = String::new();
        let mut tools_used = Vec::new();

        while state != TurnState::Done {
            match state {
                TurnState::Restore => {
                    session = self.session_manager.get_or_create(session_key);
                    session.add_message("user", user_content);

                    let parts = crate::providers::parse_multimodal_content(user_content);
                    let has_images = parts.iter().any(|p| matches!(p, crate::providers::ContentPart::Image { .. }));
                    let supports_vision = crate::providers::model_supports_vision(&self.config.agents.defaults.model);
                    if has_images && !supports_vision {
                        eprintln!("⚠️ Warning: The active model '{}' does not support images. Images will be ignored.", self.config.agents.defaults.model);
                    }

                    state = TurnState::Compact;
                }
                TurnState::Compact => {
                    let max_msgs = self.config.agents.defaults.max_messages;
                    let len = session.messages.len();
                    if len > max_msgs {
                        let keep_msgs = max_msgs.saturating_sub(10).max(5);
                        let k = len.saturating_sub(keep_msgs);
                        if k > 0 && k < len {
                            let messages_to_summarize = session.messages[0..k].to_vec();
                            let existing_summary = session.metadata.get("summary")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                                
                            let system_prompt_sum = "You are a helpful assistant. Generate a consolidated summary of the conversation history. Keep it concise, clear, and focused on key facts, decisions, and files created/modified.";
                            let mut prompt_content = String::new();
                            if !existing_summary.is_empty() {
                                prompt_content.push_str(&format!("Previous summary:\n{}\n\n", existing_summary));
                            }
                            prompt_content.push_str("New conversation history to summarize:\n");
                            for msg in &messages_to_summarize {
                                prompt_content.push_str(&format!("[{}]: {}\n", msg.role, msg.content));
                            }
                            
                            let settings = GenerationSettings {
                                temperature: 0.1,
                                max_tokens: 1024,
                                reasoning_effort: None,
                            };
                            
                            let summary_msgs = vec![Message {
                                role: "user".to_string(),
                                content: prompt_content,
                                timestamp: Some(chrono::Utc::now().to_rfc3339()),
                                extra: serde_json::Map::new(),
                            }];
                            
                            println!("📝 Consolidating conversation context (summarizing older history)...");
                            match self.provider.chat(&system_prompt_sum, &summary_msgs, &[], &settings).await {
                                Ok(resp) => {
                                    if let Some(new_summary) = resp.content {
                                        session.metadata.insert("summary".to_string(), serde_json::Value::String(new_summary));
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Warning: Failed to summarize conversation history: {}", e);
                                }
                            }

                            // Consolidate long-term memory
                            let existing_memory = session.metadata.get("memory")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");

                            let system_prompt_mem = "You are a specialized Memory Manager. Your task is to update the long-term memory of key facts, user preferences, decisions, and guidelines based on new conversation history.\n\nIncorporate new facts into the existing memory, remove deprecated/contradicted information, and return a clean, consolidated Markdown list of memory facts. Keep it concise, organized, and focused on durable project context.";
                            let mut mem_prompt_content = String::new();
                            if !existing_memory.is_empty() {
                                mem_prompt_content.push_str(&format!("Existing memory:\n{}\n\n", existing_memory));
                            }
                            mem_prompt_content.push_str("New conversation history to extract facts/decisions from:\n");
                            for msg in &messages_to_summarize {
                                mem_prompt_content.push_str(&format!("[{}]: {}\n", msg.role, msg.content));
                            }

                            let mem_msgs = vec![Message {
                                role: "user".to_string(),
                                content: mem_prompt_content,
                                timestamp: Some(chrono::Utc::now().to_rfc3339()),
                                extra: serde_json::Map::new(),
                            }];

                            println!("🧠 Consolidating long-term memory...");
                            match self.provider.chat(&system_prompt_mem, &mem_msgs, &[], &settings).await {
                                Ok(resp) => {
                                    if let Some(new_memory) = resp.content {
                                        session.metadata.insert("memory".to_string(), serde_json::Value::String(new_memory));
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Warning: Failed to update long-term memory: {}", e);
                                }
                            }

                            session.messages = session.messages[k..].to_vec();
                        } else {
                            session.messages = session.messages[len - max_msgs..].to_vec();
                        }
                    }
                    state = TurnState::Command;
                }
                TurnState::Command => {
                    if user_content.starts_with('/') {
                        let parts: Vec<&str> = user_content.split_whitespace().collect();
                        if let Some(cmd) = parts.first() {
                            match *cmd {
                                "/help" => {
                                    final_content = "OpenZ Rebranded AI Agent Command Menu:\n/help - Show this menu\n/history - Show history\n/clear - Reset session history\n/status - Print active model and configuration info\n/memory - Show or manage agent memory (/memory, /memory clear, /memory add <fact>)\n/delegate <goal> - Directly delegate a task to a focused subagent".to_string();
                                    state = TurnState::Done;
                                    continue;
                                }
                                "/history" => {
                                    let mut hist = String::new();
                                    for msg in &session.messages {
                                        hist.push_str(&format!("[{}]: {}\n", msg.role, msg.content));
                                    }
                                    final_content = hist;
                                    state = TurnState::Done;
                                    continue;
                                }
                                "/clear" | "/restart" => {
                                    session.messages.clear();
                                    self.session_manager.save(&session)?;
                                    final_content = "Conversation history has been cleared.".to_string();
                                    state = TurnState::Done;
                                    continue;
                                }
                                "/status" => {
                                    final_content = format!(
                                        "OpenZ Agent Status:\nModel: {}\nProvider: {}\nWorkspace: {}\nTotal Messages: {}",
                                        self.config.agents.defaults.model,
                                        self.config.agents.defaults.provider,
                                        self.config.agents.defaults.workspace,
                                        session.messages.len()
                                    );
                                    state = TurnState::Done;
                                    continue;
                                }
                                "/memory" => {
                                    if parts.len() < 2 {
                                        let memory = session.metadata.get("memory")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("No memory recorded yet.");
                                        final_content = format!("=== Agent Long-Term Memory ===\n{}", memory);
                                    } else {
                                        match parts[1] {
                                            "clear" => {
                                                session.metadata.remove("memory");
                                                self.session_manager.save(&session)?;
                                                final_content = "Agent memory has been cleared.".to_string();
                                            }
                                            "add" | "set" => {
                                                if parts.len() < 3 {
                                                    final_content = "Usage: /memory add <fact>".to_string();
                                                } else {
                                                    let fact = parts[2..].join(" ");
                                                    let mut existing = session.metadata.get("memory")
                                                        .and_then(|v| v.as_str())
                                                        .unwrap_or("")
                                                        .to_string();
                                                    if !existing.is_empty() {
                                                        existing.push_str("\n");
                                                    }
                                                    existing.push_str(&format!("* {}", fact));
                                                    session.metadata.insert("memory".to_string(), serde_json::Value::String(existing));
                                                    self.session_manager.save(&session)?;
                                                    final_content = format!("Added to memory: {}", fact);
                                                }
                                            }
                                            _ => {
                                                final_content = "Unknown memory command. Options: /memory, /memory clear, /memory add <fact>".to_string();
                                            }
                                        }
                                    }
                                    state = TurnState::Done;
                                    continue;
                                }
                                "/delegate" | "/subagent" => {
                                    if parts.len() < 2 {
                                        final_content = "Usage: /delegate <goal>".to_string();
                                    } else {
                                        let goal = parts[1..].join(" ");
                                        let delegate_tool: std::sync::Arc<dyn crate::tools::Tool> = std::sync::Arc::new(DelegateTaskTool {
                                            config: self.config.clone(),
                                            parent_provider: self.provider.clone(),
                                            session_manager: self.session_manager.clone(),
                                        });

                                        let args = serde_json::json!({
                                            "goal": goal,
                                        });

                                        match delegate_tool.call(&args).await {
                                            Ok(res_val) => {
                                                if let Some(summary) = res_val.get("summary").and_then(|v| v.as_str()) {
                                                    final_content = format!("=== Subagent Summary ===\n{}", summary);
                                                } else {
                                                    final_content = format!("Subagent completed: {}", res_val);
                                                }
                                            }
                                            Err(e) => {
                                                final_content = format!("Error running subagent: {}", e);
                                            }
                                        }
                                    }
                                    state = TurnState::Done;
                                    continue;
                                }
                                _ => {}
                            }
                        }
                    }
                    state = TurnState::Build;
                }
                TurnState::Build => {
                    let mut summary_part = String::new();
                    if let Some(summary) = session.metadata.get("summary").and_then(|v| v.as_str()) {
                        if !summary.is_empty() {
                            summary_part = format!("\n\nHere is a summary of the earlier part of the conversation:\n{}\n", summary);
                        }
                    }
                    let mut memory_part = String::new();
                    if let Some(memory) = session.metadata.get("memory").and_then(|v| v.as_str()) {
                        if !memory.is_empty() {
                            memory_part = format!("\n\nHere is the long-term memory of key facts, preferences, and decisions from this session:\n{}\n", memory);
                        }
                    }
                    let mut vision_instruction = "";
                    if !crate::providers::model_supports_vision(&self.config.agents.defaults.model) {
                        vision_instruction = " If a message contains a markdown image link (e.g. ![](file://...)) and you need to analyze or describe the image, you MUST delegate the visual analysis task to the specialized 'vision_agent' tool (or the 'delegate_task' tool) to see and report on the image contents.";
                    }
                    system_prompt = format!(
                        "You are {}, a helpful assistant. Current date and time: {}. Keep replies clear, precise, and concise.{}{}{}{}",
                        self.config.agents.defaults.bot_name,
                        chrono::Utc::now().to_rfc3339(),
                        summary_part,
                        memory_part,
                        vision_instruction,
                        ""
                    );
                    messages = session.messages.clone();
                    state = TurnState::Run;
                }
                TurnState::Run => {
                    let mut iterations = 0;
                    let max_iterations = self.config.agents.defaults.max_tool_iterations;
                    let settings = GenerationSettings {
                        temperature: self.config.agents.defaults.temperature,
                        max_tokens: self.config.agents.defaults.max_tokens,
                        reasoning_effort: None,
                    };

                    loop {
                        if iterations >= max_iterations {
                            return Err(anyhow!("Reached maximum tool loop iterations ({})", max_iterations));
                        }
                        
                        let tools_openai = self.tools.to_openai_format();
                        
                        println!("{} {} is thinking...", self.config.agents.defaults.bot_icon, self.config.agents.defaults.bot_name);
                        
                        let resp = self.provider.chat(&system_prompt, &messages, &tools_openai, &settings).await?;
                        
                        if let Some(content) = resp.content {
                            final_content = content.clone();
                            messages.push(Message {
                                role: "assistant".to_string(),
                                content,
                                timestamp: Some(chrono::Utc::now().to_rfc3339()),
                                extra: serde_json::Map::new(),
                            });
                        }

                        if resp.tool_calls.is_empty() {
                            break;
                        }

                        let mut tool_results = Vec::new();
                        let mut assistant_tool_calls_json = Vec::new();
                        
                        for call in resp.tool_calls {
                            println!("🔧 Calling tool: {}({})", call.name, call.arguments);
                            tools_used.push(call.name.clone());
                            
                            let result_val = match self.tools.get(&call.name) {
                                Some(t) => match t.call(&call.arguments).await {
                                    Ok(res) => res,
                                    Err(e) => serde_json::json!({ "error": e.to_string() }),
                                },
                                None => serde_json::json!({ "error": format!("Tool '{}' not found", call.name) }),
                            };
                            
                            tool_results.push((call.id.clone(), call.name.clone(), result_val));
                            
                            assistant_tool_calls_json.push(serde_json::json!({
                                "id": call.id,
                                "type": "function",
                                "function": {
                                    "name": call.name,
                                    "arguments": call.arguments.to_string()
                                }
                            }));
                        }

                        if let Some(last_msg) = messages.last_mut() {
                            if last_msg.role == "assistant" {
                                last_msg.extra.insert("tool_calls".to_string(), serde_json::Value::Array(assistant_tool_calls_json));
                            }
                        } else {
                            let mut extra = serde_json::Map::new();
                            extra.insert("tool_calls".to_string(), serde_json::Value::Array(assistant_tool_calls_json));
                            messages.push(Message {
                                role: "assistant".to_string(),
                                content: String::new(),
                                timestamp: Some(chrono::Utc::now().to_rfc3339()),
                                extra,
                            });
                        }

                        for (id, name, result) in tool_results {
                            let mut extra = serde_json::Map::new();
                            extra.insert("tool_call_id".to_string(), serde_json::Value::String(id));
                            extra.insert("name".to_string(), serde_json::Value::String(name));
                            messages.push(Message {
                                role: "tool".to_string(),
                                content: result.to_string(),
                                timestamp: Some(chrono::Utc::now().to_rfc3339()),
                                extra,
                            });
                        }

                        iterations += 1;
                    }
                    
                    session.messages = messages.clone();
                    state = TurnState::Save;
                }
                TurnState::Save => {
                    self.session_manager.save(&session)?;
                    state = TurnState::Respond;
                }
                TurnState::Respond => {
                    state = TurnState::Done;
                }
                TurnState::Done => {}
            }
        }

        let traces_dir = crate::config::resolve_path("~/.openz/traces");
        if let Err(e) = std::fs::create_dir_all(&traces_dir) {
            eprintln!("Warning: Failed to create traces directory: {}", e);
        } else {
            let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
            let trace_file = traces_dir.join(format!("trace_{}_{}.json", session_key.replace(":", "_"), timestamp));
            let trace_record = serde_json::json!({
                "session_key": session_key,
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "user_query": user_content,
                "system_prompt": system_prompt,
                "model": self.config.agents.defaults.model,
                "messages": messages,
                "tools_used": tools_used,
                "final_response": final_content,
            });
            if let Ok(content) = serde_json::to_string_pretty(&trace_record) {
                let _ = std::fs::write(trace_file, content);
            }
        }

        Ok(RunResult {
            content: final_content,
            tools_used,
        })
    }
}
