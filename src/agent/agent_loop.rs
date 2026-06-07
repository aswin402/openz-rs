use crate::config::schema::Config;
use crate::providers::{LLMProvider, GenerationSettings};
use crate::tools::ToolRegistry;
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
                    state = TurnState::Compact;
                }
                TurnState::Compact => {
                    let max_msgs = self.config.agents.defaults.max_messages;
                    if session.messages.len() > max_msgs {
                        let len = session.messages.len();
                        session.messages = session.messages[len - max_msgs..].to_vec();
                    }
                    state = TurnState::Command;
                }
                TurnState::Command => {
                    if user_content.starts_with('/') {
                        let parts: Vec<&str> = user_content.split_whitespace().collect();
                        if let Some(cmd) = parts.first() {
                            match *cmd {
                                "/help" => {
                                    final_content = "OpenZ Rebranded AI Agent Command Menu:\n/help - Show this menu\n/history - Show history\n/clear - Reset session history\n/status - Print active model and configuration info".to_string();
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
                                _ => {}
                            }
                        }
                    }
                    state = TurnState::Build;
                }
                TurnState::Build => {
                    system_prompt = format!(
                        "You are {}, a helpful assistant. Current date and time: {}. Keep replies clear, precise, and concise.",
                        self.config.agents.defaults.bot_name,
                        chrono::Utc::now().to_rfc3339()
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

        Ok(RunResult {
            content: final_content,
            tools_used,
        })
    }
}
