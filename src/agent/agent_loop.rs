use crate::config::schema::Config;
use crate::providers::{LLMProvider, GenerationSettings};
use crate::tools::ToolRegistry;
use crate::tools::subagent::{DelegateTaskTool, CancellationToken};
use crate::session::{Session, SessionManager, Message};
use crate::agent::style::*;
use anyhow::Result;
use std::sync::Arc;
use serde::Deserialize;
use std::io::Write;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnState {
    Restore,
    Compact,
    Command,
    Build,
    Run,
    Save,
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
    pub streamed: bool,
}

struct ActivityGuard<'a> {
    session_key: &'a str,
}

impl<'a> Drop for ActivityGuard<'a> {
    fn drop(&mut self) {
        crate::agent::activity::update_activity(self.session_key, "Idle", None);
    }
}

static SESSION_LOCKS: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, std::sync::Arc<tokio::sync::Mutex<()>>>>> = std::sync::OnceLock::new();

fn get_session_lock(session_key: &str) -> std::sync::Arc<tokio::sync::Mutex<()>> {
    let map = SESSION_LOCKS.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    let mut guard = map.lock().unwrap_or_else(|e| e.into_inner());
    guard.entry(session_key.to_string())
        .or_insert_with(|| std::sync::Arc::new(tokio::sync::Mutex::new(())))
        .clone()
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

    pub fn update_model_and_provider(&mut self, config: Config, provider: Arc<dyn LLMProvider>) {
        self.config = config.clone();
        self.provider = provider.clone();
        if let Some(ref mut ctx) = self.tools.context {
            ctx.0 = config;
            ctx.1 = provider;
        }
    }

    /// Clean up old trace files and tool output files to prevent unbounded disk usage.
    /// Deletes files older than 7 days in ~/.openz/traces/ and ~/.openz/tool_outputs/.
    fn cleanup_old_files() {
        use std::time::{SystemTime, Duration};

        let max_age = Duration::from_secs(7 * 24 * 60 * 60); // 7 days
        let now = SystemTime::now();

        for dir_name in &["traces", "tool_outputs"] {
            let dir = crate::config::resolve_path(&format!("~/.openz/{}", dir_name));
            if !dir.exists() {
                continue;
            }
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    if let Ok(metadata) = entry.metadata() {
                        if let Ok(modified) = metadata.modified() {
                            if let Ok(age) = now.duration_since(modified) {
                                if age > max_age {
                                    let _ = std::fs::remove_file(entry.path());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    async fn chat_with_fallback(
        &self,
        active_provider: &mut Arc<dyn LLMProvider>,
        system_prompt: &str,
        messages: &[Message],
        tools: &[serde_json::Value],
        settings: &GenerationSettings,
        activity_msg: &str,
    ) -> Result<crate::providers::LLMResponse> {
        let chat_fut = active_provider.chat(system_prompt, messages, tools, settings);
        let mut chat_result = with_spinner(activity_msg, chat_fut).await;

        if chat_result.is_err() {
            let mut fallbacks = Vec::new();
            for fallback in &self.config.agents.defaults.fallback_models {
                if let Some(s) = fallback.as_str() {
                    if !s.trim().is_empty() {
                        fallbacks.push(s.trim().to_string());
                    }
                }
            }

            let mut resolved_fallback = false;
            for fallback_model in fallbacks {
                let silent = crate::agent::style::spinner::is_silent();
                if !silent {
                    crate::tui_println!(
                        "{}▲ Primary provider failed. Attempting fallback model: {}{}",
                        AURA_GOLD, fallback_model, COLOR_RESET
                    );
                }
                if let Ok(fallback_provider) = crate::tools::subagent::build_provider_for_model(&self.config, &fallback_model) {
                    let chat_fut = fallback_provider.chat(system_prompt, messages, tools, settings);
                    chat_result = with_spinner(activity_msg, chat_fut).await;
                    if chat_result.is_ok() {
                        resolved_fallback = true;
                        *active_provider = fallback_provider;
                        break;
                    }
                }
            }

            if !resolved_fallback {
                let chat_fut = active_provider.chat(system_prompt, messages, tools, settings);
                chat_result = with_spinner(activity_msg, chat_fut).await;
            }
        }

        chat_result
    }

    async fn chat_stream_with_fallback(
        &self,
        active_provider: &mut Arc<dyn LLMProvider>,
        system_prompt: &str,
        messages: &[Message],
        tools: &[serde_json::Value],
        settings: &GenerationSettings,
        activity_msg: &str,
    ) -> Result<std::pin::Pin<Box<dyn futures_util::Stream<Item = Result<crate::providers::ChatStreamChunk>> + Send>>> {
        let chat_fut = active_provider.chat_stream(system_prompt, messages, tools, settings);
        let mut chat_result = with_spinner(activity_msg, chat_fut).await;

        if chat_result.is_err() {
            let mut fallbacks = Vec::new();
            for fallback in &self.config.agents.defaults.fallback_models {
                if let Some(s) = fallback.as_str() {
                    if !s.trim().is_empty() {
                        fallbacks.push(s.trim().to_string());
                    }
                }
            }

            let mut resolved_fallback = false;
            for fallback_model in fallbacks {
                let silent = crate::agent::style::spinner::is_silent();
                if !silent {
                    crate::tui_println!(
                        "{}▲ Primary provider failed. Attempting fallback model: {}{}",
                        AURA_GOLD, fallback_model, COLOR_RESET
                    );
                }
                if let Ok(fallback_provider) = crate::tools::subagent::build_provider_for_model(&self.config, &fallback_model) {
                    let chat_fut = fallback_provider.chat_stream(system_prompt, messages, tools, settings);
                    chat_result = with_spinner(activity_msg, chat_fut).await;
                    if chat_result.is_ok() {
                        resolved_fallback = true;
                        *active_provider = fallback_provider;
                        break;
                    }
                }
            }

            if !resolved_fallback {
                let chat_fut = active_provider.chat_stream(system_prompt, messages, tools, settings);
                chat_result = with_spinner(activity_msg, chat_fut).await;
            }
        }

        chat_result
    }

    pub async fn run(&self, user_content: &str, session_key: &str) -> Result<RunResult> {
        let lock = get_session_lock(session_key);
        let _guard = lock.lock().await;

        let parent_key = crate::agent::style::spinner::get_current_session_key();
        let target_key = match parent_key {
            Some(ref pk) if !pk.starts_with("subagent:") => pk.clone(),
            _ => session_key.to_string(),
        };

        let is_cli = target_key == "cli:direct" && !session_key.starts_with("subagent:");
        let silent = !is_cli;

        crate::agent::style::spinner::IS_SILENT.scope(silent, async move {
            crate::agent::style::spinner::CURRENT_SESSION_KEY.scope(target_key, async move {
                // Wrap the entire turn in a tracing span tagged with the session key.
                // Every log event emitted inside run_inner (tools, MCP, LLM calls) will
                // carry `session=<key>` in their output, enabling `openz logs --session`
                // filtering in the live log viewer.
                let span = tracing::info_span!("turn", session = %session_key);
                let _enter = span.enter();
                self.run_inner(user_content, session_key).await
            }).await
        }).await
    }

    async fn run_inner(&self, user_content: &str, session_key: &str) -> Result<RunResult> {
        // Clean up old traces and tool outputs on first run of session
        static CLEANUP_ONCE: std::sync::Once = std::sync::Once::new();
        CLEANUP_ONCE.call_once(|| {
            Self::cleanup_old_files();
        });

        crate::agent::activity::update_activity(session_key, "Processing user prompt", None);
        let _guard = ActivityGuard { session_key };

        let mut active_provider = self.provider.clone();
        let mut state = TurnState::Restore;
        let mut session = Session::new(session_key);
        let mut messages = Vec::new();
        let mut system_prompt = String::new();
        let mut final_content = String::new();
        let mut tools_used = Vec::new();
        let mut interaction_id = None;
        let mut turn_errors = Vec::new();
        let mut _session_file_lock = None;
        let mut streamed = false;

        while state != TurnState::Done {
            match state {
                TurnState::Restore => {
                    _session_file_lock = Some(self.session_manager.acquire_lock(session_key).map_err(|e| {
                        anyhow::anyhow!("Cannot start session: {}", e)
                    })?);
                    session = self.session_manager.get_or_create(session_key);
                    if !user_content.starts_with('/') {
                        interaction_id = crate::tools::shared_memory::log_interaction(session_key, user_content).await.ok();
                    }
                    session.add_message("user", user_content);
                    tracing::info!(session = %session_key, "Restored session history ({} messages). User prompt: {:?}", session.messages.len(), user_content);

                    let parts = crate::providers::parse_multimodal_content(user_content).await;
                    let has_images = parts.iter().any(|p| matches!(p, crate::providers::ContentPart::Image { .. }));
                    let supports_vision = crate::providers::model_supports_vision(&self.config.agents.defaults.model);
                    let silent = crate::agent::style::spinner::is_silent();
                    if has_images && !supports_vision
                        && !silent {
                            eprintln!("{}▲ Image unsupported: The active model '{}' does not support images. Images will be ignored.{}", AURA_GOLD, self.config.agents.defaults.model, COLOR_RESET);
                        }

                    if let Err(e) = self.session_manager.save(&session) {
                        tracing::warn!("Failed to save session incrementally in Restore: {}", e);
                    }

                    state = TurnState::Compact;
                }
                TurnState::Compact => {
                    let max_msgs = self.config.agents.defaults.max_messages;
                    let len = session.messages.len();
                    if len > max_msgs {
                        let keep_msgs = max_msgs.saturating_sub(10).max(5);
                        let mut k = len.saturating_sub(keep_msgs);
                        
                        // Find the nearest "user" message by scanning backwards.
                        // This ensures the kept history slice always starts with a "user" message,
                        // and prevents orphaned "tool" messages from causing API errors.
                        while k > 0 && session.messages[k].role != "user" {
                            k -= 1;
                        }
                        
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
                            
                            let spinner_msg = format!(
                                "{}▸ Consolidating conversation context...{}",
                                RED_ORANGE,
                                COLOR_RESET
                            );
                            tracing::info!(session = %session_key, "Compacting history ({} messages > {} limit)...", len, max_msgs);
                            match self.chat_with_fallback(&mut active_provider, system_prompt_sum, &summary_msgs, &[], &settings, &spinner_msg).await {
                                Ok(resp) => {
                                    if let Some(new_summary) = resp.content {
                                        tracing::info!(session = %session_key, "Compacted summary length: {} chars", new_summary.len());
                                        session.metadata.insert("summary".to_string(), serde_json::Value::String(new_summary));
                                    }
                                }
                                Err(e) => {
                                    tracing::error!(session = %session_key, "Failed to compact conversation history: {}", e);
                                    eprintln!("{}▲ Failed to summarize conversation history: {}{}", AURA_GOLD, e, COLOR_RESET);
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

                            let spinner_msg = format!(
                                "{}▸ Consolidating long-term memory...{}",
                                RED_ORANGE,
                                COLOR_RESET
                            );
                            match self.chat_with_fallback(&mut active_provider, system_prompt_mem, &mem_msgs, &[], &settings, &spinner_msg).await {
                                Ok(resp) => {
                                    if let Some(new_memory) = resp.content {
                                        tracing::info!(session = %session_key, "Consolidated long-term memory. Memory size: {} chars", new_memory.len());
                                        session.metadata.insert("memory".to_string(), serde_json::Value::String(new_memory));
                                    }
                                }
                                Err(e) => {
                                    let silent = crate::agent::style::spinner::is_silent();
                                    tracing::error!(session = %session_key, "Failed to consolidate long-term memory: {}", e);
                                    if !silent {
                                        eprintln!("{}▲ Failed to update long-term memory: {}{}", AURA_GOLD, e, COLOR_RESET);
                                    }
                                }
                            }

                            session.messages = session.messages[k..].to_vec();
                        } else {
                            let mut k = len - max_msgs;
                            while k > 0 && session.messages[k].role != "user" {
                                k -= 1;
                            }
                            session.messages = session.messages[k..].to_vec();
                        }
                    }
                    state = TurnState::Command;
                }
                TurnState::Command => {
                    let mut profile_name = None;
                    let parts_key: Vec<&str> = session_key.split(':').collect();
                    if parts_key.len() >= 2 && parts_key[0] == "subagent" {
                        profile_name = Some(parts_key[1]);
                    }
                    if user_content.starts_with('/') {
                        let parts: Vec<&str> = user_content.split_whitespace().collect();
                        if let Some(cmd) = parts.first() {
                            match *cmd {
                                "/help" => {
                                    final_content = "OpenZ Rebranded AI Agent Command Menu:\n/help - Show this menu\n/history - Show history\n/clear - Reset session history\n/status - Print active model and configuration info\n/memory - Show or manage agent memory (/memory, /memory clear, /memory add <fact>)\n/skills - List active skills (/skills, /skills clear)\n/skill - Manage skills (/skill view <name>, /skill add <name> <content>, /skill delete <name>)\n/audit - Cryptographically verify session message chain integrity\n/delegate <goal> - Directly delegate a task to a focused subagent".to_string();
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
                                "/audit" => {
                                    match session.verify_hash_chain() {
                                        Ok(_) => {
                                            let mut output = "✅ MERKLE AUDIT PASS: Chain integrity verified successfully.\n\n".to_string();
                                            output.push_str("Index | Role | Timestamp | Merkle Block Hash\n");
                                            output.push_str("------|------|-----------|-------------------\n");
                                            for (i, msg) in session.messages.iter().enumerate() {
                                                let hash = msg.get_hash().unwrap_or("None");
                                                let ts = msg.timestamp.as_deref().unwrap_or("N/A");
                                                output.push_str(&format!(
                                                    "{:5} | {:4} | {} | {}\n",
                                                    i, msg.role, ts, hash
                                                ));
                                            }
                                            final_content = output;
                                        }
                                        Err(e) => {
                                            final_content = format!("❌ MERKLE AUDIT FAIL: Chain integrity compromised!\nError: {}", e);
                                        }
                                    }
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
                                                        existing.push('\n');
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
                                "/skills" => {
                                    if parts.len() > 1 && parts[1] == "clear" {
                                        if let Err(e) = crate::agent::skills::clear_skills() {
                                            final_content = format!("Error clearing skills: {}", e);
                                        } else {
                                            final_content = "All agent skills have been cleared.".to_string();
                                        }
                                    } else {
                                        match crate::agent::skills::load_skills_with_profile(profile_name) {
                                            Ok(skills) => {
                                                if skills.is_empty() {
                                                    final_content = "No active skills recorded yet.".to_string();
                                                } else {
                                                    let list: Vec<String> = skills.iter().map(|s| format!("* {}", s.name)).collect();
                                                    final_content = format!("=== Agent Skills ===\n{}", list.join("\n"));
                                                }
                                            }
                                            Err(e) => {
                                                final_content = format!("Error loading skills: {}", e);
                                            }
                                        }
                                    }
                                    state = TurnState::Done;
                                    continue;
                                }
                                "/skill" => {
                                    if parts.len() < 2 {
                                        final_content = "Usage: /skill view <name>, /skill add <name> <content>, /skill delete <name>".to_string();
                                    } else {
                                        match parts[1] {
                                            "view" => {
                                                if parts.len() < 3 {
                                                    final_content = "Usage: /skill view <name>".to_string();
                                                } else {
                                                    let name = parts[2];
                                                    match crate::agent::skills::load_skills_with_profile(profile_name) {
                                                        Ok(skills) => {
                                                            if let Some(skill) = skills.iter().find(|s| s.name == name) {
                                                                final_content = format!("=== Skill: {} ===\n{}", skill.name, skill.content);
                                                            } else {
                                                                final_content = format!("Skill '{}' not found.", name);
                                                            }
                                                        }
                                                        Err(e) => {
                                                            final_content = format!("Error: {}", e);
                                                        }
                                                    }
                                                }
                                            }
                                            "add" | "set" => {
                                                if parts.len() < 4 {
                                                    final_content = "Usage: /skill add <name> <content>".to_string();
                                                } else {
                                                    let name = parts[2];
                                                    let content = parts[3..].join(" ");
                                                    let res = if let Some(prof) = profile_name {
                                                        crate::agent::skills::save_subagent_skill(prof, name, &content)
                                                    } else {
                                                        crate::agent::skills::save_skill(name, &content)
                                                    };
                                                    if let Err(e) = res {
                                                        final_content = format!("Error saving skill: {}", e);
                                                    } else {
                                                        final_content = format!("Skill '{}' added/updated successfully.", name);
                                                    }
                                                }
                                            }
                                            "delete" | "remove" => {
                                                if parts.len() < 3 {
                                                    final_content = "Usage: /skill delete <name>".to_string();
                                                } else {
                                                    let name = parts[2];
                                                    if let Err(e) = crate::agent::skills::delete_skill_with_profile(name, profile_name) {
                                                        final_content = format!("Error deleting skill: {}", e);
                                                    } else {
                                                        final_content = format!("Skill '{}' deleted successfully.", name);
                                                    }
                                                }
                                            }
                                            _ => {
                                                final_content = "Unknown skill command. Options: /skill view <name>, /skill add <name> <content>, /skill delete <name>".to_string();
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
                                        let parent_tools = self.tools.get_static_tools()
                                            .into_iter()
                                            .filter(|t| t.name() != "delegate_task" && t.name() != "parallel_research")
                                            .collect();
                                        let delegate_tool: std::sync::Arc<dyn crate::tools::Tool> = std::sync::Arc::new(DelegateTaskTool {
                                            config: self.config.clone(),
                                            parent_provider: active_provider.clone(),
                                            session_manager: self.session_manager.clone(),
                                            parent_tools,
                                            cancellation_token: CancellationToken::new(),
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
                    let mut profile_name = None;
                    let parts: Vec<&str> = session_key.split(':').collect();
                    if parts.len() >= 2 && parts[0] == "subagent" {
                        profile_name = Some(parts[1]);
                    }

                    let mut skills_part = String::new();
                    if let Ok(skills) = crate::agent::skills::load_relevant_skills_with_profile(user_content, &session.messages, profile_name) {
                        if !skills.is_empty() {
                            skills_part = "\n\nHere are the active guidelines and procedural skills you should follow:\n".to_string();
                            for skill in skills {
                                skills_part.push_str(&format!("=== Skill: {} ===\n{}\n\n", skill.name, skill.content));
                            }
                        }
                    }
                    let mut vision_instruction = "";
                    if !crate::providers::model_supports_vision(&self.config.agents.defaults.model) {
                        vision_instruction = " If a message contains a markdown image link (e.g. ![](file://...)) and you need to analyze or describe the image, you MUST delegate the visual analysis task to the specialized 'vision_agent' tool (or the 'delegate_task' tool) to see and report on the image contents.";
                    }
                    let subagents_list = if let Ok(profiles) = crate::subagents::load_profiles() {
                        profiles.iter().map(|p| p.name.clone()).collect::<Vec<String>>().join(", ")
                    } else {
                        "planner, researcher, debugger, DevOps, skill_improvement, openz_maintainer, mcps_manager".to_string()
                    };
                    let system_guidelines = format!(
                        "\n\nYou are OpenZ, a high-performance personal AI agent framework built in Rust, vibe-coded by Aswin. You are inspired by Zeroclaw, Nanobot, hermes-agent, loops!, and DOX. Your architecture is structured as follows:\n\
                         * Creator & Inspiration: Vibe-coded by Aswin. Inspired by Zeroclaw, Nanobot, hermes-agent, loops!, and DOX.\n\
                         * Specifications & Changelog: The root of your workspace contains 'CHANGELOG.md' and you have a native command 'openz changelog' displaying system specs (ROM ~10-15MB, RAM ~15-30MB cloud / ~200MB+ local, <5ms startup), design inspirations, key capabilities, and version release history.\n\
                         * CLI Subcommands & Flags: The executable is launched via:\n\
                           - 'openz onboard': Runs the setup wizard for LLM provider API keys.\n\
                           - 'openz configure': Configures providers, gateways, channels, and preferences.\n\
                           - 'openz agent': Starts the TUI terminal chat loop (auto-starts background channels & cron job scheduler).\n\
                           - 'openz gateway': Starts the WebSocket + WebUI server (default port 8765).\n\
                           - 'openz telegram': Starts the Telegram bot polling listener.\n\
                           - 'openz discord': Starts the Discord bot gateway listener.\n\
                           - 'openz whatsapp': Starts the WhatsApp Axum webhook receiver (default port 8090).\n\
                           - 'openz subagent': Starts the TUI subagent profile manager.\n\
                           - 'openz logs [--path <file>] [--tail <lines>] [--session <prefix>]': View real-time color-coded structured logs (live follow mode with rotation detection).\n\
                           - 'openz mcp-bridge --port <port> -- <command> [args...]': Runs a gRPC MCP bridge wrapper.\n\
                           - 'openz sop <list | instances | trigger <id> | resume <id> | simulate <id>>': Controls the stateful SOP workflow engine.\n\
                         * Pluggable Gateway Channels: You can receive messages and reply over CLI terminal, WebSocket gateway (serving the WebUI workbench), Telegram bot polling, Discord bot polling, WhatsApp Business API, and pure Rust IMAP/SMTP Email client.\n\
                         * Local Tools & MCP: You have native tools for file reading/writing, codebase text search ('grep_search'), file code structure parsing ('code_outline'), git operations ('git_manager'), database inspection ('db_inspector'), cargo toolchain execution ('cargo_manager'), system clipboard access ('clipboard'), opening files/folders/URLs ('open_path'), background file change watching ('file_watcher'), structural code search ('ast_grep'), real browser automation ('gsd_browser'), web search queries ('web_search'), shell command execution, web fetching, remote control forwarding, document reading ('read_doc'), sandboxed WASM execution ('wasm_execute'), and project template/package scaffolding ('onpkg'). You support the Model Context Protocol (MCP) powered by high-performance Rust binaries for sequential thinking, memory graph storage, and the 'headroom' context compression server. Managed via the native 'manage_mcp' tool.\n\
                         * Context Scoping & Compression: Via the 'headroom' MCP server, you can call:\n\
                           - 'scope_context' (with target_path): Walks up the tree and compiles relevant AGENTS.md instructions. Use this BEFORE editing files to retrieve rules.\n\
                           - 'compress_content' (with raw_text and content_type): Compresses logs/code/JSON and registers a CCR reference token (CCR ID).\n\
                           - 'retrieve_original' (with ccr_id): Retrieves the original raw text. Use this to read the full content of any truncated output or file (it accepts both CCR IDs and file:// file paths!).\n\
                         * Remote Session Control: If the user asks you (e.g., via Telegram or Discord) to execute a command, answer an approval prompt, or run a query in their TUI/CLI session, invoke the 'send_remote_input' tool to forward the prompt directly to that session (e.g., 'cli:direct').\n\
                         * Specialized Subagents: You can spawn concurrent subagents (available subagent tools: {}) to delegate tasks.\n\
                         * Stateful SOP Workflow Engine: DAG-based template executions (like 'ship-pr-until-green' closed-loop healing, PR creation, CI verification) with Zenflow checkpointed transactions and auto-rollback.\n\
                         * Compiler Auto-Healing: 'CompilerAutoHealTool' compiles code natively, reads compiler output, and prompts you to fix syntax or borrow checker issues in a loop until green.\n\
                         * Security Guard & BPF Sandbox: Subprocesses are sandboxed using a Linux seccomp BPF filter to block dangerous commands, with strict/normal/loose levels.\n\
                         * Cryptographic Audit Ledger: Uses SHA-256 Merkle chain hashing on all session messages/states, verified on boot, with a '/audit' slash command.\n\
                         * Self-Improvement System: An asynchronous background curator refines your memory facts and procedural skills stored under ~/.openz/skills/ and SQLite database (~/.openz/memory.db).",
                        subagents_list
                    );

                    let mut activity_part = String::new();
                    if let Some(activity) = crate::agent::activity::get_activity() {
                        if activity.session_id != session_key {
                            activity_part = format!(
                                "\n\n[SYSTEM NOTICE] Status of the other active/last-run session on this computer:\n\
                                 * Session ID: {}\n\
                                 * Status: {}\n\
                                 * Last/Current Tool: {}\n\
                                 * Timestamp: {}\n",
                                activity.session_id,
                                activity.status,
                                activity.current_tool.as_deref().unwrap_or("None"),
                                activity.timestamp
                            );
                        }
                    }

                    let caveman_rules = if self.config.agents.defaults.caveman_mode {
                        "\n\nRespond terse like smart caveman. All technical substance stay. Only fluff die.\nRules:\n- Drop: articles (a/an/the), filler (just/really/basically), pleasantries, hedging\n- Fragments OK. Short synonyms. Technical terms exact. Code unchanged.\n- Pattern: [thing] [action] [reason]. [next step].\n- Not: \"Sure! I'd be happy to help you with that.\"\n- Yes: \"Bug in auth middleware. Fix:\""
                    } else {
                        ""
                    };

                    system_prompt = format!(
                        "You are {}, a helpful assistant. Current date and time: {}. Keep replies clear, precise, and concise.{}{}{}{}{}{}{}",
                        self.config.agents.defaults.bot_name,
                        chrono::Utc::now().to_rfc3339(),
                        system_guidelines,
                        activity_part,
                        summary_part,
                        memory_part,
                        vision_instruction,
                        skills_part,
                        caveman_rules
                    );
                    messages = session.messages.clone();
                    state = TurnState::Run;
                }
                TurnState::Run => {
                    let mut iterations = 0;
                    let mut loop_blocked_count = 0;
                    let max_iterations = self.config.agents.defaults.max_tool_iterations;
                    let settings = GenerationSettings {
                        temperature: self.config.agents.defaults.temperature,
                        max_tokens: self.config.agents.defaults.max_tokens,
                        reasoning_effort: None,
                    };

                    loop {
                        tracing::info!(session = %session_key, iteration = iterations, "Sending completion request to LLM (model: {})", self.config.agents.defaults.model);
                        if iterations >= max_iterations {
                            let msg = format!(
                                "⚠️ Reached tool iteration limit ({}). Summarizing work so far.",
                                max_iterations
                            );
                            final_content = msg.clone();
                            send_progress_update(session_key, &msg).await;
                            if !crate::agent::style::spinner::is_silent() {
                                print!("{}⚠️ {}{}\r\n", AURA_GOLD, msg, COLOR_RESET);
                                let _ = std::io::stdout().flush();
                            }
                            messages.push(Message {
                                role: "assistant".to_string(),
                                content: msg,
                                timestamp: Some(chrono::Utc::now().to_rfc3339()),
                                extra: serde_json::Map::new(),
                            });
                            break;
                        }
                        
                        let tools_openai = self.tools.to_openai_format();
                        
                        let activity_msg = format!("{}▶ Thinking{}", RED_ORANGE, COLOR_RESET);
                        let start_time = std::time::Instant::now();
                        // Track if content was already streamed to terminal (to avoid duplicate display)
                        let mut content_streaming_started = false;
                        let mut reasoning_printed = false;
                        let mut current_line_buffer = String::new();
                        let mut resp = if self.config.agents.defaults.streaming {
                            let mut stream = self.chat_stream_with_fallback(&mut active_provider, &system_prompt, &messages, &tools_openai, &settings, &activity_msg).await?;
                            let silent = crate::agent::style::spinner::is_silent();
                            
                            let mut full_content = String::new();
                            let mut full_reasoning = String::new();
                            let mut finish_reason = "stop".to_string();
                            // Track whether we're currently in reasoning phase (for live spinner)
                            let mut in_reasoning_phase = false;

                            let print_reasoning = |full_reasoning: &str, in_reasoning_phase: &mut bool, reasoning_printed: &mut bool, start_time: std::time::Instant| {
                                if !*reasoning_printed && !full_reasoning.is_empty() {
                                    if !silent {
                                        let elapsed = start_time.elapsed().as_secs_f32();
                                        print!("\r\x1b[2K");
                                        print!("{}{}▶ Thought for {:.1}s{}\r\n", COLOR_BOLD, RED_ORANGE, elapsed, COLOR_RESET);
                                        for line in full_reasoning.trim().lines() {
                                            print!("{}{}{}\r\n", AURA_SLATE, line.trim(), COLOR_RESET);
                                        }
                                        print!("\r\n");
                                        let _ = std::io::stdout().flush();
                                    }
                                    *reasoning_printed = true;
                                    *in_reasoning_phase = false;
                                }
                            };
                            
                            struct PartialToolCall {
                                id: String,
                                name: String,
                                arguments: String,
                            }
                            let mut partial_tool_calls = std::collections::HashMap::<usize, PartialToolCall>::new();

                            use futures_util::StreamExt;
                            while let Some(chunk) = stream.next().await {
                                match chunk? {
                                    crate::providers::ChatStreamChunk::Content(text) => {
                                        // If we have reasoning content that hasn't been printed yet, print it now
                                        print_reasoning(&full_reasoning, &mut in_reasoning_phase, &mut reasoning_printed, start_time);
                                        
                                        // If we were in reasoning phase but reasoning was empty/already printed, clear the spinner
                                        if in_reasoning_phase && !silent {
                                            print!("\r\x1b[2K");
                                            let _ = std::io::stdout().flush();
                                            in_reasoning_phase = false;
                                        }
                                        full_content.push_str(&text);
                                        for c in text.chars() {
                                            if c == '\r' {
                                                continue;
                                            }
                                            if c == '\n' {
                                                if !silent {
                                                    content_streaming_started = true;
                                                    print!("\r\x1b[2K");
                                                    print!("{}", format_markdown_line(&current_line_buffer));
                                                    print!("\r\n");
                                                    let _ = std::io::stdout().flush();
                                                }
                                                current_line_buffer.clear();
                                            } else {
                                                current_line_buffer.push(c);
                                                if !silent {
                                                    content_streaming_started = true;
                                                    print!("{}", c);
                                                    let _ = std::io::stdout().flush();
                                                }
                                            }
                                        }
                                        send_progress_update(session_key, &text).await;
                                    }
                                    crate::providers::ChatStreamChunk::Reasoning(text) => {
                                        full_reasoning.push_str(&text);
                                        in_reasoning_phase = true;
                                        // Show a live thinking spinner instead of raw reasoning text
                                        if !silent {
                                            let elapsed = start_time.elapsed().as_secs_f32();
                                            print!("\r\x1b[2K{}{}▶ Thinking... {:.1}s{}", COLOR_BOLD, RED_ORANGE, elapsed, COLOR_RESET);
                                            let _ = std::io::stdout().flush();
                                        }
                                    }
                                    crate::providers::ChatStreamChunk::ToolCall { index, id, name, arguments } => {
                                        // If we have reasoning content that hasn't been printed yet, print it now
                                        print_reasoning(&full_reasoning, &mut in_reasoning_phase, &mut reasoning_printed, start_time);
                                        
                                        // Also clear thinking spinner if active
                                        if in_reasoning_phase && !silent {
                                            print!("\r\x1b[2K");
                                            let _ = std::io::stdout().flush();
                                            in_reasoning_phase = false;
                                        }
                                        
                                        let entry = partial_tool_calls.entry(index).or_insert_with(|| PartialToolCall {
                                            id: String::new(),
                                            name: String::new(),
                                            arguments: String::new(),
                                        });
                                        if let Some(val) = id {
                                            entry.id = val;
                                        }
                                        if let Some(val) = name {
                                            entry.name = val;
                                        }
                                        if let Some(val) = arguments {
                                            entry.arguments.push_str(&val);
                                        }
                                    }
                                    crate::providers::ChatStreamChunk::Done { finish_reason: reason } => {
                                        if let Some(r) = reason {
                                            finish_reason = r;
                                        }
                                    }
                                }
                            }

                            if in_reasoning_phase && !silent {
                                print!("\r\x1b[2K");
                                let _ = std::io::stdout().flush();
                                in_reasoning_phase = false;
                            }
                            
                            // Print any reasoning that was not printed yet
                            print_reasoning(&full_reasoning, &mut in_reasoning_phase, &mut reasoning_printed, start_time);

                            // Print the final line in the buffer if any
                            if !current_line_buffer.is_empty() && !silent {
                                print!("\r\x1b[2K");
                                print!("{}", format_markdown_line(&current_line_buffer));
                                let _ = std::io::stdout().flush();
                            }

                            // Collect and parse tool calls
                            let mut tool_calls = Vec::new();
                            let mut sorted_keys: Vec<_> = partial_tool_calls.keys().collect();
                            sorted_keys.sort();
                            for k in sorted_keys {
                                if let Some(ptc) = partial_tool_calls.get(k) {
                                    let args_parsed = match serde_json::from_str(&ptc.arguments) {
                                        Ok(parsed) => parsed,
                                        Err(e) => {
                                            let repaired = ptc.arguments.replace("\n", "\\n").replace("\r", "\\r");
                                            serde_json::from_str(&repaired).unwrap_or_else(|_| {
                                                let mut map = serde_json::Map::new();
                                                map.insert("parse_error".to_string(), serde_json::Value::String(e.to_string()));
                                                serde_json::Value::Object(map)
                                            })
                                        }
                                    };
                                    tool_calls.push(crate::providers::ToolCallRequest {
                                        id: ptc.id.clone(),
                                        name: ptc.name.clone(),
                                        arguments: args_parsed,
                                    });
                                }
                            }

                            streamed = true;

                            crate::providers::LLMResponse {
                                content: if full_content.is_empty() { None } else { Some(full_content) },
                                tool_calls,
                                finish_reason,
                                reasoning_content: if full_reasoning.is_empty() { None } else { Some(full_reasoning) },
                            }
                        } else {
                            self.chat_with_fallback(&mut active_provider, &system_prompt, &messages, &tools_openai, &settings, &activity_msg).await?
                        };
                        
                        // Handle potential response truncation (finish_reason = "length") by auto-continuing
                        if resp.finish_reason == "length" {
                            let mut accumulated_content = resp.content.clone();
                            let mut finish_reason = resp.finish_reason.clone();
                            let mut continue_attempts = 0;
                            
                            while finish_reason == "length" && continue_attempts < 3 {
                                continue_attempts += 1;
                                
                                let mut temp_messages = messages.clone();
                                if let Some(ref current_acc) = accumulated_content {
                                    temp_messages.push(Message {
                                        role: "assistant".to_string(),
                                        content: current_acc.clone(),
                                        timestamp: Some(chrono::Utc::now().to_rfc3339()),
                                        extra: serde_json::Map::new(),
                                    });
                                }
                                
                                temp_messages.push(Message {
                                    role: "user".to_string(),
                                    content: "Continue generating the rest of your previous message exactly from where you left off. Do not repeat the beginning.".to_string(),
                                    timestamp: Some(chrono::Utc::now().to_rfc3339()),
                                    extra: serde_json::Map::new(),
                                });
                                
                                let cont_activity_msg = format!("{}▶ Continuing response... (attempt {}){}", RED_ORANGE, continue_attempts, COLOR_RESET);
                                // Pass &[] instead of tools_openai so the model does not get confused and attempt to generate tool calls during text continuation
                                if let Ok(cont_resp) = self.chat_with_fallback(&mut active_provider, &system_prompt, &temp_messages, &[], &settings, &cont_activity_msg).await {
                                    finish_reason = cont_resp.finish_reason.clone();
                                    if let Some(ref cont_content) = cont_resp.content {
                                        if let Some(ref mut acc) = accumulated_content {
                                            acc.push_str(cont_content);
                                        } else {
                                            accumulated_content = Some(cont_content.clone());
                                        }
                                    }
                                    if !cont_resp.tool_calls.is_empty() {
                                        resp.tool_calls.extend(cont_resp.tool_calls);
                                    }
                                } else {
                                    break;
                                }
                            }
                            
                            resp.content = accumulated_content;
                            resp.finish_reason = finish_reason;

                            // If tool_calls is empty, try to parse tool calls from the fully completed accumulated content
                            if resp.tool_calls.is_empty() {
                                if let Some(ref text) = resp.content {
                                    let parsed = crate::providers::openai::parse_fallback_tool_calls(text);
                                    if !parsed.is_empty() {
                                        resp.tool_calls = parsed;
                                        resp.content = None;
                                    }
                                }
                            }
                        }
                        
                        // Handle models that send everything as reasoning_content with no content.
                        // When there's reasoning but no content and no tool calls, the reasoning IS the response.
                        // Common with DeepSeek-V4 and similar reasoning models.
                        if resp.content.is_none() && resp.reasoning_content.is_some() && resp.tool_calls.is_empty() {
                            resp.content = resp.reasoning_content.take();
                            // If we already printed it as reasoning on the terminal, set streamed = true
                            // to avoid printing it again in cli.rs
                            if reasoning_printed {
                                streamed = true;
                            } else {
                                streamed = false;
                            }
                            // Clear any thinking spinner that was active on the terminal
                            if !crate::agent::style::spinner::is_silent() {
                                print!("\r\x1b[2K");
                                let _ = std::io::stdout().flush();
                            }
                        }

                        let duration = start_time.elapsed();
                        tracing::info!(
                            session = %session_key,
                            duration_ms = duration.as_millis(),
                            has_content = resp.content.is_some(),
                            has_reasoning = resp.reasoning_content.is_some(),
                            tool_calls = resp.tool_calls.len(),
                            "Received LLM response (finish_reason: {})",
                            resp.finish_reason
                        );
                        if let Some(ref reasoning) = resp.reasoning_content {
                            tracing::debug!(session = %session_key, "LLM reasoning content: {:?}", reasoning);
                        }
                        if let Some(ref content) = resp.content {
                            tracing::debug!(session = %session_key, "LLM text content: {:?}", content);
                        }
                        
                        let duration_secs = duration.as_secs_f32();
                        let has_reasoning = resp.reasoning_content.as_ref().map(|s| !s.trim().is_empty()).unwrap_or(false);
                        let has_content = resp.content.as_ref().map(|s| !s.trim().is_empty()).unwrap_or(false);
                        let has_tool_calls = !resp.tool_calls.is_empty();
                        
                        if has_reasoning || (has_content && has_tool_calls) {
                            // Send reasoning/thought summary to non-CLI channels (Telegram, WS, etc.)
                            if has_reasoning {
                                if let Some(ref reasoning) = resp.reasoning_content {
                                    let reasoning_msg = format!("▶ *Thought*\n\n> {}", reasoning.trim().replace("\n", "\n> "));
                                    send_progress_update(session_key, &reasoning_msg).await;
                                }
                            } else if has_content && has_tool_calls {
                                if let Some(ref content) = resp.content {
                                    let thought_msg = format!("▶ *Thought*\n\n> {}", content.trim().replace("\n", "\n> "));
                                    send_progress_update(session_key, &thought_msg).await;
                                }
                            }
                            
                            let silent = crate::agent::style::spinner::is_silent();
                            if !silent {
                                if streamed {
                                    // During streaming, the reasoning spinner was already shown and
                                    // the "Thought for Xs" badge was already printed when content
                                    // started arriving or when the stream finished. If no content
                                    // arrived and no reasoning was printed (e.g. pure tool-call-only response),
                                    // finalize the spinner and print the badge now.
                                    if !content_streaming_started && !reasoning_printed {
                                        print!("\r\x1b[2K");
                                        print!("{}{}▶ Thought for {:.1}s{}\r\n", COLOR_BOLD, RED_ORANGE, duration_secs, COLOR_RESET);
                                        let _ = std::io::stdout().flush();
                                    }
                                } else {
                                    // Non-streaming path: print the badge and thinking summary
                                    print!("{}{}▶ Thought for {:.1}s{}\r\n", COLOR_BOLD, RED_ORANGE, duration_secs, COLOR_RESET);
                                    if has_reasoning {
                                        if let Some(ref reasoning) = resp.reasoning_content {
                                            for line in reasoning.trim().lines() {
                                                print!("{}{}{}\r\n", AURA_SLATE, line.trim(), COLOR_RESET);
                                            }
                                        }
                                    } else if has_content && has_tool_calls {
                                        if let Some(ref content) = resp.content {
                                            for line in content.trim().lines() {
                                                print!("{}{}{}\r\n", AURA_SLATE, line.trim(), COLOR_RESET);
                                            }
                                        }
                                    }
                                    let _ = std::io::stdout().flush();
                                }
                                print!("\r\n");
                                let _ = std::io::stdout().flush();
                            }
                        }
                        
                        if let Some(content) = resp.content {
                            let text_repeat = count_previous_text_responses(&messages, &content);
                            if text_repeat >= 2 {
                                let loop_msg = "⚠️ Halted execution: Detected repetitive text responses.";
                                final_content = loop_msg.to_string();
                                send_progress_update(session_key, loop_msg).await;
                                if !crate::agent::style::spinner::is_silent() {
                                    print!("{}⚠️ {}{}\r\n", AURA_GOLD, loop_msg, COLOR_RESET);
                                    let _ = std::io::stdout().flush();
                                }
                                messages.push(Message {
                                    role: "assistant".to_string(),
                                    content: loop_msg.to_string(),
                                    timestamp: Some(chrono::Utc::now().to_rfc3339()),
                                    extra: serde_json::Map::new(),
                                });
                                break;
                            }

                            final_content = content.clone();
                            let mut extra = serde_json::Map::new();
                            if let Some(ref reasoning) = resp.reasoning_content {
                                extra.insert("reasoning_content".to_string(), serde_json::Value::String(reasoning.clone()));
                            }
                            messages.push(Message {
                                role: "assistant".to_string(),
                                content,
                                timestamp: Some(chrono::Utc::now().to_rfc3339()),
                                extra,
                            });
                        }

                        if resp.tool_calls.is_empty() {
                            break;
                        }

                        let mut should_halt = false;
                        let mut tool_results = Vec::new();
                        let mut assistant_tool_calls_json = Vec::new();
                        
                        for call in resp.tool_calls {
                            tools_used.push(call.name.clone());
                            
                            crate::agent::activity::update_activity(session_key, "Executing tool", Some(&call.name));
                            let formatted_args = format_tool_args(&call.name, &call.arguments);
                            let tool_spinner_msg = format!("{}▸{} Running {}...", AURA_GOLD, COLOR_RESET, formatted_args);
                            
                            let tool_msg = format!("▸ Running *{}*...", formatted_args);
                            send_progress_update(session_key, &tool_msg).await;
                            
                            tracing::info!(
                                session = %session_key,
                                tool = %call.name,
                                arguments = %call.arguments,
                                "Executing tool call"
                            );
                            
                            let silent = crate::agent::style::spinner::is_silent();
                            let mut approved = true;
                            let mut forbidden = false;
                            let security_mode = &self.config.agents.defaults.security_mode;

                            let parse_error = call.arguments.get("parse_error").and_then(|v| v.as_str());

                            let repeat_count = count_previous_tool_calls(&messages, &call.name, &call.arguments);
                            let is_loop = repeat_count >= 2;
                            if is_loop && parse_error.is_none() {
                                loop_blocked_count += 1;
                                if loop_blocked_count >= 3 {
                                    should_halt = true;
                                }
                            }

                            if parse_error.is_none() && crate::agent::security::SecurityGuard::is_forbidden(&call.name, &call.arguments) {
                                forbidden = true;
                            } else if parse_error.is_none() && !is_loop && crate::agent::security::SecurityGuard::is_sensitive_with_mode(&call.name, &call.arguments, security_mode) {
                                // Clear the running tool spinner first so the prompt is clean
                                if !silent {
                                    print!("\r\x1b[2K");
                                    let _ = std::io::stdout().flush();
                                }
                                
                                match crate::agent::security::ask_approval(session_key, &call.name, &call.arguments).await {
                                    Ok(app) => approved = app,
                                    Err(_) => approved = false,
                                }
                            }

                            let result_val = if let Some(err_msg) = parse_error {
                                let fail_msg = format!("✕ *{}* - Failed: {}", formatted_args, err_msg);
                                send_progress_update(session_key, &fail_msg).await;
                                if !silent {
                                    crate::tui_println!("{}✕ {} - Failed: {}{}", AURA_PURPLE, formatted_args, err_msg, COLOR_RESET);
                                }
                                turn_errors.push(format!("Tool {} arguments parse error: {}", call.name, err_msg));
                                serde_json::json!({ "error": err_msg })
                            } else if is_loop {
                                let warning_str = format!(
                                    "Loop detected: You have already executed the tool '{}' with these exact arguments {} times in this turn. To prevent infinite loops, execution was blocked. Do NOT call this tool again. Analyze previous tool outputs and use a different strategy, or finish your response.",
                                    call.name, repeat_count
                                );
                                if !silent {
                                    crate::tui_println!("{}⚠️ Loop detected for tool '{}'! Blocking execution. (Count: {}){}", AURA_GOLD, call.name, loop_blocked_count, COLOR_RESET);
                                }
                                tracing::warn!(
                                    session = %session_key,
                                    tool = %call.name,
                                    "Tool execution blocked (repetition/loop detected)"
                                );
                                serde_json::json!({ "error": warning_str })
                            } else if forbidden {
                                let reject_msg = format!("✕ *{}* - Rejected: Dangerous command is forbidden", formatted_args);
                                send_progress_update(session_key, &reject_msg).await;
                                if !silent {
                                    print!("{}✕{} {} - Rejected: Dangerous command is forbidden\r\n", ERROR_RED, COLOR_RESET, formatted_args);
                                    let _ = std::io::stdout().flush();
                                }
                                tracing::warn!(
                                    session = %session_key,
                                    tool = %call.name,
                                    "Tool execution forbidden by security guard"
                                );
                                serde_json::json!({ "error": "Execution denied by host: This command is forbidden by security rules." })
                            } else if !approved {
                                let deny_msg = format!("✕ *{}* - Denied by user", formatted_args);
                                send_progress_update(session_key, &deny_msg).await;
                                if !silent {
                                    print!("{}✕{} {} - Denied by user\r\n", ERROR_RED, COLOR_RESET, formatted_args);
                                    let _ = std::io::stdout().flush();
                                }
                                tracing::warn!(
                                    session = %session_key,
                                    tool = %call.name,
                                    "Tool execution denied by user approval request"
                                );
                                serde_json::json!({ "error": "Execution denied by user." })
                            } else {
                                match self.tools.get(&call.name) {
                                    Some(t) => {
                                        let tool_timeout = std::time::Duration::from_secs(self.config.agents.defaults.tool_timeout_secs);
                                        let fut = t.call(&call.arguments);
                                        let timed_fut = tokio::time::timeout(tool_timeout, fut);
                                        match with_spinner(&tool_spinner_msg, timed_fut).await {
                                            Ok(Ok(res)) => {
                                                let success_msg = format!("✓ *{}*", formatted_args);
                                                send_progress_update(session_key, &success_msg).await;
                                                if !silent {
                                                    print!("{}✓{} {}\r\n", EMERALD_GREEN, COLOR_RESET, formatted_args);
                                                    let _ = std::io::stdout().flush();
                                                }
                                                tracing::info!(
                                                    session = %session_key,
                                                    tool = %call.name,
                                                    status = "success",
                                                    "Tool call completed"
                                                );
                                                tracing::debug!(
                                                    session = %session_key,
                                                    tool = %call.name,
                                                    result = %res,
                                                    "Tool output result"
                                                );
                                                res
                                            }
                                            Ok(Err(e)) => {
                                                let error_str = e.to_string();
                                                turn_errors.push(format!("Tool {} failed: {}", call.name, error_str));
                                                let fail_msg = format!("✕ *{}* - Failed: {}", formatted_args, error_str);
                                                send_progress_update(session_key, &fail_msg).await;
                                                if !silent {
                                                    crate::tui_println!("{}✕ {} - Failed: {}{}", AURA_PURPLE, formatted_args, error_str, COLOR_RESET);
                                                }
                                                tracing::error!(
                                                    session = %session_key,
                                                    tool = %call.name,
                                                    error = %error_str,
                                                    "Tool call failed"
                                                );
                                                let hint = generate_self_healing_hint(&call.name, &error_str);
                                                serde_json::json!({
                                                    "error": error_str,
                                                    "self_healing_suggestion": hint
                                                })
                                            }
                                            Err(_) => {
                                                let timeout_msg = format!("Tool '{}' timed out after {}s", call.name, self.config.agents.defaults.tool_timeout_secs);
                                                turn_errors.push(timeout_msg.clone());
                                                let fail_msg = format!("⏱️ *{}* - Timed out", formatted_args);
                                                send_progress_update(session_key, &fail_msg).await;
                                                if !silent {
                                                    crate::tui_println!("{}⏱️ {} - Timed out after {}s{}", AURA_GOLD, formatted_args, self.config.agents.defaults.tool_timeout_secs, COLOR_RESET);
                                                }
                                                tracing::error!(
                                                    session = %session_key,
                                                    tool = %call.name,
                                                    "Tool call timed out"
                                                );
                                                serde_json::json!({
                                                    "error": timeout_msg,
                                                    "hint": "The tool exceeded the time limit. Try a more specific query, a smaller scope, or break the task into steps."
                                                })
                                            }
                                        }
                                    }
                                    None => {
                                        let error_str = format!("Tool '{}' not found", call.name);
                                        turn_errors.push(format!("Tool {} not found", call.name));
                                        let fail_msg = format!("✕ *{}* - Failed: {}", formatted_args, error_str);
                                        send_progress_update(session_key, &fail_msg).await;
                                        if !silent {
                                            crate::tui_println!("{}✕ {} - Failed: {}{}", AURA_PURPLE, formatted_args, error_str, COLOR_RESET);
                                        }
                                        let hint = generate_self_healing_hint(&call.name, &error_str);
                                        serde_json::json!({
                                            "error": error_str,
                                            "self_healing_suggestion": hint
                                        })
                                    }
                                }
                            };
                            if let Some(err_val) = result_val.get("error").and_then(|v| v.as_str()) {
                                turn_errors.push(format!("Tool {} returned error: {}", call.name, err_val));
                            }
                            crate::agent::activity::update_activity(session_key, "Processing user prompt", None);
                            
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
                                if let Some(ref reasoning) = resp.reasoning_content {
                                    last_msg.extra.insert("reasoning_content".to_string(), serde_json::Value::String(reasoning.clone()));
                                }
                            } else {
                                let mut extra = serde_json::Map::new();
                                extra.insert("tool_calls".to_string(), serde_json::Value::Array(assistant_tool_calls_json));
                                if let Some(ref reasoning) = resp.reasoning_content {
                                    extra.insert("reasoning_content".to_string(), serde_json::Value::String(reasoning.clone()));
                                }
                                messages.push(Message {
                                    role: "assistant".to_string(),
                                    content: String::new(),
                                    timestamp: Some(chrono::Utc::now().to_rfc3339()),
                                    extra,
                                });
                            }
                        } else {
                            let mut extra = serde_json::Map::new();
                            extra.insert("tool_calls".to_string(), serde_json::Value::Array(assistant_tool_calls_json));
                            if let Some(ref reasoning) = resp.reasoning_content {
                                extra.insert("reasoning_content".to_string(), serde_json::Value::String(reasoning.clone()));
                            }
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
                            extra.insert("name".to_string(), serde_json::Value::String(name.clone()));
                            
                            let content_str = result.to_string();
                            let limit = self.config.agents.defaults.tool_output_limit.unwrap_or(4000);
                            let is_retrieve = name == "retrieve_original" || name == "headroom/retrieve_original";
                            let content = if content_str.len() > limit && !is_retrieve {
                                let outputs_dir = crate::config::resolve_path("~/.openz/tool_outputs");
                                let _ = std::fs::create_dir_all(&outputs_dir);
                                let file_name = format!("output_{}_{}.json", name, uuid::Uuid::new_v4());
                                let file_path = outputs_dir.join(file_name);
                                let _ = std::fs::write(&file_path, &content_str);
                                
                                let compressed = crate::agent::context_compactor::compress_tool_output(&name, &content_str);
                                format!(
                                    "{}\n\n... [TRUNCATED - Full output saved for reference at file://{}] ...",
                                    compressed,
                                    file_path.to_string_lossy()
                                )
                            } else {
                                content_str
                            };

                            messages.push(Message {
                                role: "tool".to_string(),
                                content,
                                timestamp: Some(chrono::Utc::now().to_rfc3339()),
                                extra,
                            });
                        }

                        session.messages = messages.clone();
                        if let Err(e) = self.session_manager.save(&session) {
                            tracing::warn!("Failed to save session incrementally in Run loop: {}", e);
                        }

                        iterations += 1;
                        if should_halt {
                            let halt_msg = "⚠️ Halted execution: Too many repeating tool calls blocked by loop detection. Halting to save RAM and tokens.";
                            final_content = halt_msg.to_string();
                            send_progress_update(session_key, halt_msg).await;
                            if !crate::agent::style::spinner::is_silent() {
                                print!("{}⚠️ {}{}\r\n", AURA_GOLD, halt_msg, COLOR_RESET);
                                let _ = std::io::stdout().flush();
                            }
                            messages.push(Message {
                                role: "assistant".to_string(),
                                content: halt_msg.to_string(),
                                timestamp: Some(chrono::Utc::now().to_rfc3339()),
                                extra: serde_json::Map::new(),
                            });
                            break;
                        }
                    }
                    
                    session.messages = messages.clone();
                    if let Some(ref inter_id) = interaction_id {
                        if !turn_errors.is_empty() {
                            let errors_str = turn_errors.join("\n");
                            let _ = crate::tools::shared_memory::update_interaction_errors(inter_id, &errors_str).await;
                        }
                    }
                    state = TurnState::Save;
                }
                TurnState::Save => {
                    self.session_manager.save(&session)?;
                    if let Err(e) = crate::tools::onpkg::sync_onpkg_manifest() {
                        tracing::warn!("Failed to synchronize onpkg manifest: {}", e);
                    }
                    tracing::info!(session = %session_key, "Session saved successfully. Turn complete.");
                    state = TurnState::Done;
                }
                TurnState::Done => {}
            }
        }

        let traces_dir = crate::config::resolve_path("~/.openz/traces");
        if let Err(e) = std::fs::create_dir_all(&traces_dir) {
            eprintln!("{}▲ Failed to create traces directory: {}{}", AURA_GOLD, e, COLOR_RESET);
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
                "tools_used": tools_used.clone(),
                "final_response": final_content,
            });
            if let Ok(content) = serde_json::to_string_pretty(&trace_record) {
                let _ = std::fs::write(trace_file, content);
            }
        }

        // Spawn background self-improvement curator
        if !user_content.starts_with('/') {
            let session_manager = self.session_manager.clone();
            let session_key = session_key.to_string();
            let provider = active_provider.clone();
            let messages = messages.clone();
            let tools_used = tools_used.clone();

            tokio::spawn(async move {
                if let Some(rx) = crate::shutdown::receiver() {
                    if *rx.borrow() {
                        return;
                    }
                }
                let mut profile_name = None;
                let parts_key: Vec<&str> = session_key.split(':').collect();
                if parts_key.len() >= 2 && parts_key[0] == "subagent" {
                    profile_name = Some(parts_key[1].to_string());
                }

                let write_log = |status: &str, memory_updated: bool, skills_saved: Vec<String>, error_message: Option<String>| {
                    #[derive(serde::Serialize)]
                    struct CuratorStatus {
                        last_run_timestamp: String,
                        status: String,
                        session_key: String,
                        memory_updated: bool,
                        skills_saved: Vec<String>,
                        error_message: Option<String>,
                    }
                    let log_path = crate::config::resolve_path("~/.openz/curator_status.json");
                    let record = CuratorStatus {
                        last_run_timestamp: chrono::Utc::now().to_rfc3339(),
                        status: status.to_string(),
                        session_key: session_key.clone(),
                        memory_updated,
                        skills_saved,
                        error_message,
                    };
                    if let Some(parent) = log_path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    if let Ok(content) = serde_json::to_string_pretty(&record) {
                        let _ = std::fs::write(log_path, content);
                    }
                };

                let mut should_run = false;
                let total_chars: usize = messages.iter().map(|m| m.content.len()).sum();
                let estimated_tokens = total_chars / 4;
                if estimated_tokens >= 4000 {
                    should_run = true;
                } else {
                    for tool in &tools_used {
                        let t = tool.to_lowercase();
                        if t.contains("write_file") ||
                           t.contains("patch_file") ||
                           t.contains("replace_lines") ||
                           t.contains("zenflow_edit") ||
                           t.contains("db_write") ||
                           t.contains("cargo") ||
                           t.contains("exec_command") ||
                           t.contains("web_fetch") ||
                           t.contains("web_search") ||
                           t.contains("crawl") ||
                           t.contains("obscura") ||
                           t.contains("gsd_browser") ||
                           t.contains("remote_input") ||
                           t.contains("mcp") {
                            should_run = true;
                            break;
                        }
                    }
                }

                if !should_run {
                    write_log("skipped: throttled (simple turn)", false, vec![], None);
                    return;
                }

                tracing::info!(session = %session_key, "Self-improvement curator: started processing.");
                write_log("running", false, vec![], None);

                // Run background skill archiving check
                let _ = crate::agent::skills::archive_stale_skills();

                // Run shared memory consolidation
                let _ = crate::tools::shared_memory::consolidate_shared_memory(&provider).await;

                #[derive(Deserialize)]
                struct ReviewSkill {
                    name: String,
                    content: String,
                }

                #[derive(Deserialize)]
                struct ReviewResponse {
                    memory_updated: bool,
                    memory_content: String,
                    skills_to_save: Vec<ReviewSkill>,
                }

                // 1. Get recent interactions
                let recent_interactions = crate::tools::shared_memory::get_recent_interactions(15).await.unwrap_or_default();

                // 2. Get existing memory from current file
                let existing_memory = if let Ok(s) = session_manager.load(&session_key) {
                    s.metadata.get("memory")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string()
                } else {
                    String::new()
                };

                // 3. Get existing skills list and contents
                let mut existing_skills_desc = String::new();
                if let Ok(skills) = crate::agent::skills::load_skills_with_profile(profile_name.as_deref()) {
                    for skill in skills {
                        existing_skills_desc.push_str(&format!("Skill Name: {}\nContent:\n{}\n\n", skill.name, skill.content));
                    }
                }

                // 4. Setup prompts for self-improvement review
                let system_prompt_review = "You are a specialized Self-Improvement Curator. Your job is to review the conversation between the User and the AI Agent and consolidate two types of learnings:\n\n\
                    1. MEMORY: Facts about the user (e.g. persona, desires, expectations) or the project (e.g. settings, environment details).\n\
                    2. SKILLS: Task-specific procedural guidelines, coding styles, workarounds, or workflows (e.g. 'do not explain code', 'always use async-trait', 'cargo build guidelines').\n\n\
                    CRITICAL: Pay special attention to tool execution outcomes. If a tool call (such as a compiler build, script execution, or API request) failed with an error, look at how the agent resolved it (or what workaround succeeded). Extract this learning and write it into a reusable 'skill' file so the agent will avoid making the same mistake again.\n\n\
                    REPETITIVE TASKS: Look at the list of recent user tasks provided. If the user is repeatedly asking to do a similar thing, action, or custom automation, extract a skill/workflow to automate that task so future requests can be handled instantly without re-discovering the solution.\n\n\
                    Guidelines for Skills:\n\
                    - Structure each skill as a clean, professional Markdown document containing: a title (# Skill: ...), a description of when to use it, the specific rules/guidelines, and examples of problems and their corresponding workarounds/solutions.\n\
                    - If a skill already exists in the 'Existing Skills' list, you MUST merge the new rules/workarounds into the existing skill content rather than replacing it entirely. Do not lose existing guidelines.\n\
                    - Keep skill names lowercase with underscores (e.g., 'cargo_build_fix', 'react_routing_pattern').\n\n\
                    You MUST return your response as a raw JSON object with the following structure. Do not output anything else besides the raw JSON (do not wrap it in explanation text).\n\n\
                    JSON Format:\n\
                    {\n\
                       \"memory_updated\": true/false,\n\
                       \"memory_content\": \"<updated memory markdown content. If memory_updated is false, keep it identical to existing memory or empty>\",\n\
                       \"skills_to_save\": [\n\
                         {\n\
                           \"name\": \"<name of skill, lowercase with underscores>\",\n\
                           \"content\": \"<complete updated or new markdown content for the skill. Include headers, rules, and examples. Keep existing rules and merge any new ones.>\"\n\
                         }\n\
                       ]\n\
                    }";

                let mut prompt_content = String::new();

                if !recent_interactions.is_empty() {
                    prompt_content.push_str("Recent user tasks across all sessions:\n");
                    for (i, item) in recent_interactions.iter().enumerate() {
                        let query = item["query"].as_str().unwrap_or("");
                        let success = item["success"].as_bool().unwrap_or(true);
                        let errors = item["errors"].as_str().unwrap_or("");
                        prompt_content.push_str(&format!("{}. Task: \"{}\" | Status: {}\n", i + 1, query, if success { "SUCCESS" } else { "FAILED" }));
                        if !errors.is_empty() {
                            prompt_content.push_str(&format!("   Errors encountered: {}\n", errors));
                        }
                    }
                    prompt_content.push('\n');
                }

                // Autonomous Skill Creation Notice if task was complex (>= 5 tool calls)
                let tool_count = messages.iter().filter(|m| m.role == "tool").count();
                if tool_count >= 5 {
                    prompt_content.push_str(&format!(
                        "[SYSTEM NOTICE: The recent task was complex and involved {} tool executions. Review the successful trajectory and extract a reusable skill so the agent can perform this category of work efficiently next time.]\n\n",
                        tool_count
                    ));
                }

                if !existing_memory.is_empty() {
                    prompt_content.push_str(&format!("Existing Memory:\n{}\n\n", existing_memory));
                }
                if !existing_skills_desc.is_empty() {
                    prompt_content.push_str(&format!("Existing Skills:\n{}\n\n", existing_skills_desc));
                }
                prompt_content.push_str("Recent conversation history to review:\n");
                for msg in &messages {
                    match msg.role.as_str() {
                        "user" => {
                            prompt_content.push_str(&format!("[user]: {}\n", msg.content));
                        }
                        "assistant" => {
                            prompt_content.push_str("[assistant]:\n");
                            if let Some(reasoning) = msg.extra.get("reasoning_content").and_then(|v| v.as_str()) {
                                if !reasoning.is_empty() {
                                    prompt_content.push_str(&format!("  Thinking:\n{}\n", reasoning));
                                }
                            }
                            if let Some(tool_calls) = msg.extra.get("tool_calls").and_then(|v| v.as_array()) {
                                if !tool_calls.is_empty() {
                                    prompt_content.push_str("  Tool Calls:\n");
                                    for tc in tool_calls {
                                        let name = tc.get("name").and_then(|v| v.as_str())
                                            .or_else(|| tc.get("function").and_then(|f| f.get("name")).and_then(|v| v.as_str()));
                                        let args = tc.get("arguments")
                                            .or_else(|| tc.get("function").and_then(|f| f.get("arguments")));
                                        
                                        if let (Some(name_str), Some(args_val)) = (name, args) {
                                            let args_str = args_val.to_string();
                                            let args_truncated = if args_str.len() > 1000 {
                                                let truncated: String = args_str.chars().take(1000).collect();
                                                format!("{}... [TRUNCATED - {} bytes]", truncated, args_str.len() - 1000)
                                            } else {
                                                args_str
                                            };
                                            prompt_content.push_str(&format!("    - Call tool '{}' with arguments: {}\n", name_str, args_truncated));
                                        }
                                    }
                                }
                            }
                            if !msg.content.is_empty() {
                                let content_truncated = if msg.content.len() > 2000 {
                                    let truncated: String = msg.content.chars().take(2000).collect();
                                    format!("{}... [TRUNCATED - {} bytes]", truncated, msg.content.len() - 2000)
                                } else {
                                    msg.content.clone()
                                };
                                prompt_content.push_str(&format!("  Response: {}\n", content_truncated));
                            }
                        }
                        "tool" => {
                            let tool_name = msg.extra.get("name")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown");
                            let content_truncated = if msg.content.len() > 2000 {
                                let truncated: String = msg.content.chars().take(2000).collect();
                                format!("{}... [TRUNCATED {} bytes]", truncated, msg.content.len() - 2000)
                            } else {
                                msg.content.clone()
                            };
                            prompt_content.push_str(&format!("[tool output for '{}']:\n{}\n", tool_name, content_truncated));
                        }
                        role => {
                            prompt_content.push_str(&format!("[{}]: {}\n", role, msg.content));
                        }
                    }
                }

                let review_msgs = vec![crate::session::Message {
                    role: "user".to_string(),
                    content: prompt_content,
                    timestamp: Some(chrono::Utc::now().to_rfc3339()),
                    extra: serde_json::Map::new(),
                }];

                let settings = crate::providers::GenerationSettings {
                    temperature: 0.1,
                    max_tokens: 4096,
                    reasoning_effort: None,
                };

                let mut skills_saved = Vec::new();
                let mut memory_updated = false;
                let mut error_msg = None;

                // 4. Query the LLM
                match provider.chat(system_prompt_review, &review_msgs, &[], &settings).await {
                    Ok(resp) => {
                        if let Some(content) = resp.content {
                            let trimmed = content.trim();
                            // Strip markdown code block markers if any (e.g. ```json ... ```)
                            // Strip markdown code block markers robustly (e.g. ```json ... ```)
                            let clean_json = if let Some(start_idx) = trimmed.find("```json") {
                                let after_start = &trimmed[start_idx + 7..];
                                if let Some(end_idx) = after_start.find("```") {
                                    after_start[..end_idx].trim().to_string()
                                } else {
                                    after_start.trim().to_string()
                                }
                            } else if let Some(start_idx) = trimmed.find("```") {
                                let after_start = &trimmed[start_idx + 3..];
                                if let Some(end_idx) = after_start.find("```") {
                                    after_start[..end_idx].trim().to_string()
                                } else {
                                    after_start.trim().to_string()
                                }
                            } else {
                                // Fallback: find the first '{' and last '}'
                                if let (Some(first_brace), Some(last_brace)) = (trimmed.find('{'), trimmed.rfind('}')) {
                                    if first_brace < last_brace {
                                        trimmed[first_brace..=last_brace].to_string()
                                    } else {
                                        trimmed.to_string()
                                    }
                                } else {
                                    trimmed.to_string()
                                }
                            };

                            match serde_json::from_str::<ReviewResponse>(&clean_json) {
                                Ok(review) => {
                                    // Update memory
                                    if review.memory_updated {
                                        let lock = get_session_lock(&session_key);
                                        let _guard = lock.lock().await;
                                        if let Ok(mut latest_session) = session_manager.load(&session_key) {
                                            latest_session.metadata.insert("memory".to_string(), serde_json::Value::String(review.memory_content.trim().to_string()));
                                            if let Err(e) = session_manager.save(&latest_session) {
                                                let msg = format!("Failed to save memory: {}", e);
                                                error_msg = Some(msg.clone());
                                                crate::channels::cli::send_notification(&format!("{}▲ [Self-Improvement] Failed to save self-improvement memory: {}{}", AURA_GOLD, e, COLOR_RESET));
                                            } else {
                                                memory_updated = true;
                                                tracing::info!(session = %session_key, "Self-improvement curator: updated session memory.");
                                                crate::channels::cli::send_notification(&format!("{}◇ [Self-Improvement] Memory updated based on recent conversation.{}", AURA_BLUE, COLOR_RESET));
                                            }
                                        }
                                    }
                                    
                                    // Save skills
                                    for skill in review.skills_to_save {
                                        if !skill.name.is_empty() && !skill.content.is_empty() {
                                            let res = if let Some(ref prof) = profile_name {
                                                crate::agent::skills::save_subagent_skill(prof, &skill.name, &skill.content)
                                            } else {
                                                crate::agent::skills::save_skill(&skill.name, &skill.content)
                                            };
                                            if let Err(e) = res {
                                                let msg = format!("Failed to save skill '{}': {}", skill.name, e);
                                                error_msg = Some(msg);
                                                crate::channels::cli::send_notification(&format!("{}▲ [Self-Improvement] Failed to save self-improvement skill '{}': {}{}", AURA_GOLD, skill.name, e, COLOR_RESET));
                                            } else {
                                                skills_saved.push(skill.name.clone());
                                                tracing::info!(session = %session_key, skill = %skill.name, "Self-improvement curator: saved skill.");
                                                crate::channels::cli::send_notification(&format!("{}◇ [Self-Improvement] Skill '{}' updated/created based on recent conversation.{}", AURA_BLUE, skill.name, COLOR_RESET));
                                            }
                                        }
                                    }

                                    if error_msg.is_none() {
                                        tracing::info!(session = %session_key, "Self-improvement curator finished successfully.");
                                        write_log("success", memory_updated, skills_saved, None);
                                    } else {
                                        tracing::warn!(session = %session_key, error = ?error_msg, "Self-improvement curator finished with errors.");
                                        write_log("failed", memory_updated, skills_saved, error_msg);
                                    }
                                }
                                Err(e) => {
                                    let msg = format!("JSON deserialization failed: {}", e);
                                    write_log("failed", false, vec![], Some(msg));
                                }
                            }
                        } else {
                            write_log("failed", false, vec![], Some("Empty content returned from LLM".to_string()));
                        }
                    }
                    Err(e) => {
                        let msg = format!("LLM chat query failed: {}", e);
                        write_log("failed", false, vec![], Some(msg));
                    }
                }
            });
        }

        Ok(RunResult {
            content: final_content,
            tools_used,
            streamed,
        })
    }
}

fn format_tool_args(name: &str, args: &serde_json::Value) -> String {
    let friendly_name = match name {
        "grep_search" => "Search",
        "read_file" | "view_file" => "Read",
        "write_file" | "write_to_file" | "replace_file_content" | "multi_replace_file_content" => "Edit",
        "run_command" | "exec_command" => "Bash",
        "list_dir" => "ListDir",
        "code_outline" => "Outline",
        "ast_grep" => "AstGrep",
        "git_manager" => "Git",
        "cargo_manager" => "Cargo",
        "web_search" => "WebSearch",
        "gsd_browser" => "Browser",
        "clipboard" => "Clipboard",
        "open_path" | "open" => "Open",
        "web_fetch" | "read_url_content" | "read_url" => "Fetch",
        "generate_image" => "Image",
        "generate_video" => "Video",
        "html_to_video" => "HtmlVideo",
        "create_animated_svg" | "svg_animator" => "SvgAnim",
        "obscura_browser" => "Obscura",
        "db_inspector" => "DbInspect",
        "db_write" => "DbWrite",
        "doc_reader" => "DocRead",
        "crawl" => "Crawl",
        "semantic_search" => "SemanticSearch",
        "wasm_sandbox" => "Wasm",
        "cron" => "Cron",
        "watcher" => "Watcher",
        other => other,
    };

    let details = if let serde_json::Value::Object(map) = args {
        if name == "grep_search" {
            if let Some(q) = map.get("Query").and_then(|v| v.as_str()) {
                if q.len() > 35 {
                    format!("\"{}...\"", q.chars().take(32).collect::<String>())
                } else {
                    format!("\"{}\"", q)
                }
            } else {
                String::new()
            }
        } else if name == "read_file" || name == "view_file" {
            if let Some(path) = map.get("Path").or(map.get("AbsolutePath")).and_then(|v| v.as_str()) {
                if let Some(filename) = std::path::Path::new(path).file_name() {
                    filename.to_string_lossy().to_string()
                } else {
                    path.to_string()
                }
            } else {
                String::new()
            }
        } else if name == "write_file" || name == "write_to_file" || name == "replace_file_content" || name == "multi_replace_file_content" {
            if let Some(path) = map.get("TargetFile").or(map.get("Path")).and_then(|v| v.as_str()) {
                if let Some(filename) = std::path::Path::new(path).file_name() {
                    filename.to_string_lossy().to_string()
                } else {
                    path.to_string()
                }
            } else {
                String::new()
            }
        } else if name == "run_command" || name == "exec_command" {
            if let Some(cmd) = map.get("CommandLine")
                .or(map.get("Command"))
                .or(map.get("command"))
                .or(map.get("command_line"))
                .and_then(|v| v.as_str())
            {
                let first_line = cmd.lines().next().unwrap_or("").trim();
                if first_line.len() > 40 {
                    format!("{}...", first_line.chars().take(37).collect::<String>())
                } else {
                    first_line.to_string()
                }
            } else {
                String::new()
            }
        } else if name == "list_dir" {
            if let Some(path) = map.get("DirectoryPath").or(map.get("Path")).and_then(|v| v.as_str()) {
                if let Some(filename) = std::path::Path::new(path).file_name() {
                    filename.to_string_lossy().to_string()
                } else {
                    path.to_string()
                }
            } else {
                String::new()
            }
        } else if name == "git_manager" {
            if let Some(action) = map.get("Action").and_then(|v| v.as_str()) {
                action.to_string()
            } else {
                String::new()
            }
        } else if name == "cargo_manager" {
            if let Some(command) = map.get("Command").and_then(|v| v.as_str()) {
                command.to_string()
            } else {
                String::new()
            }
        } else if name == "web_search" {
            if let Some(q) = map.get("Query").and_then(|v| v.as_str()) {
                if q.len() > 35 {
                    format!("\"{}...\"", q.chars().take(32).collect::<String>())
                } else {
                    format!("\"{}\"", q)
                }
            } else {
                String::new()
            }
        } else if name == "web_fetch" || name == "read_url_content" || name == "read_url" {
            if let Some(url) = map.get("Url").or(map.get("url")).and_then(|v| v.as_str()) {
                if url.len() > 35 {
                    format!("\"{}...\"", url.chars().take(32).collect::<String>())
                } else {
                    format!("\"{}\"", url)
                }
            } else {
                String::new()
            }
        } else if name == "generate_image" {
            let path = map.get("output_path").or(map.get("ImageName")).and_then(|v| v.as_str()).unwrap_or("output.png");
            let filename = std::path::Path::new(path).file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string());
            let shapes_count = map.get("shapes").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0);
            if shapes_count > 0 {
                format!("output: \"{}\", shapes: {}", filename, shapes_count)
            } else {
                format!("output: \"{}\"", filename)
            }
        } else if name == "generate_video" {
            let path = map.get("output_path").and_then(|v| v.as_str()).unwrap_or("output.mp4");
            let filename = std::path::Path::new(path).file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string());
            format!("output: \"{}\"", filename)
        } else if name == "html_to_video" {
            let html_path = map.get("html_path").and_then(|v| v.as_str()).unwrap_or("");
            let html_filename = if html_path.starts_with("http://") || html_path.starts_with("https://") {
                if html_path.len() > 30 {
                    format!("{}...", html_path.chars().take(27).collect::<String>())
                } else {
                    html_path.to_string()
                }
            } else {
                std::path::Path::new(html_path).file_name()
                    .map(|f| f.to_string_lossy().to_string())
                    .unwrap_or_else(|| html_path.to_string())
            };
            let out_path = map.get("output_path").and_then(|v| v.as_str()).unwrap_or("output.mp4");
            let out_filename = std::path::Path::new(out_path).file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| out_path.to_string());
            format!("html: \"{}\", output: \"{}\"", html_filename, out_filename)
        } else if name == "create_animated_svg" {
            let path = map.get("output_path").and_then(|v| v.as_str()).unwrap_or("output.svg");
            let filename = std::path::Path::new(path).file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string());
            let elem_count = map.get("elements").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0);
            let anim_count: usize = map.get("elements").and_then(|v| v.as_array()).map(|elems| {
                elems.iter().map(|e| e.get("animations").and_then(|a| a.as_array()).map(|a| a.len()).unwrap_or(0)).sum()
            }).unwrap_or(0);
            if elem_count > 0 {
                format!("output: \"{}\", elements: {}, animations: {}", filename, elem_count, anim_count)
            } else {
                format!("output: \"{}\"", filename)
            }
        } else if name == "obscura_browser" {
            let url = map.get("url").and_then(|v| v.as_str()).unwrap_or("");
            let action = map.get("action").and_then(|v| v.as_str()).unwrap_or("render");
            let truncated_url = if url.len() > 30 {
                format!("{}...", url.chars().take(27).collect::<String>())
            } else {
                url.to_string()
            };
            format!("action: \"{}\", url: \"{}\"", action, truncated_url)
        } else if name == "gsd_browser" {
            let action = map.get("action").and_then(|v| v.as_str()).unwrap_or("");
            let url = map.get("url").and_then(|v| v.as_str()).unwrap_or("");
            let ref_id = map.get("ref_id").and_then(|v| v.as_str()).unwrap_or("");
            if !url.is_empty() {
                let truncated_url = if url.len() > 30 {
                    format!("{}...", url.chars().take(27).collect::<String>())
                } else {
                    url.to_string()
                };
                format!("action: \"{}\", url: \"{}\"", action, truncated_url)
            } else if !ref_id.is_empty() {
                format!("action: \"{}\", ref_id: \"{}\"", action, ref_id)
            } else {
                format!("action: \"{}\"", action)
            }
        } else if name == "ast_grep" {
            if let Some(pattern) = map.get("pattern").and_then(|v| v.as_str()) {
                if pattern.len() > 35 {
                    format!("\"{}...\"", pattern.chars().take(32).collect::<String>())
                } else {
                    format!("\"{}\"", pattern)
                }
            } else {
                String::new()
            }
        } else if name == "crawl" {
            let url = map.get("url").and_then(|v| v.as_str()).unwrap_or("");
            if url.len() > 30 {
                format!("url: \"{}...\"", url.chars().take(27).collect::<String>())
            } else {
                format!("url: \"{}\"", url)
            }
        } else if name == "semantic_search" {
            let query = map.get("query").and_then(|v| v.as_str()).unwrap_or("");
            if query.len() > 35 {
                format!("query: \"{}...\"", query.chars().take(32).collect::<String>())
            } else {
                format!("query: \"{}\"", query)
            }
        } else if name == "doc_reader" {
            let path = map.get("file_path").or(map.get("Path")).and_then(|v| v.as_str()).unwrap_or("");
            let filename = std::path::Path::new(path).file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string());
            format!("file: \"{}\"", filename)
        } else if name == "wasm_sandbox" {
            let path = map.get("wasm_path").and_then(|v| v.as_str()).unwrap_or("");
            let filename = std::path::Path::new(path).file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string());
            format!("wasm: \"{}\"", filename)
        } else if name == "cron" {
            let action = map.get("action").and_then(|v| v.as_str()).unwrap_or("");
            format!("action: \"{}\"", action)
        } else if name == "watcher" {
            let action = map.get("action").and_then(|v| v.as_str()).unwrap_or("");
            format!("action: \"{}\"", action)
        } else if name == "db_inspector" || name == "db_write" {
            let db_path = map.get("db_path").and_then(|v| v.as_str()).unwrap_or("");
            let db_filename = std::path::Path::new(db_path).file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| db_path.to_string());
            let sql = map.get("sql").and_then(|v| v.as_str()).unwrap_or("");
            if !sql.is_empty() {
                let truncated_sql = if sql.len() > 35 {
                    format!("{}...", sql.chars().take(32).collect::<String>())
                } else {
                    sql.to_string()
                };
                format!("db: \"{}\", sql: \"{}\"", db_filename, truncated_sql)
            } else {
                let action = map.get("action").and_then(|v| v.as_str()).unwrap_or("");
                format!("db: \"{}\", action: \"{}\"", db_filename, action)
            }
        } else {
            let mut parts = Vec::new();
            for (k, v) in map {
                if k == "session_key" || k == "session_id" {
                    continue;
                }
                let val_str = match v {
                    serde_json::Value::String(s) => {
                        if s.len() > 20 {
                            format!("\"{}...\"", s.chars().take(17).collect::<String>())
                        } else {
                            format!("\"{}\"", s)
                        }
                    }
                    other => {
                        let os = other.to_string();
                        if os.len() > 20 {
                            format!("{}...", os.chars().take(17).collect::<String>())
                        } else {
                            os
                        }
                    }
                };
                parts.push(format!("{}: {}", k, val_str));
            }
            let joined = parts.join(", ");
            if joined.len() > 50 {
                format!("{}...", joined.chars().take(47).collect::<String>())
            } else {
                joined
            }
        }
    } else {
        let as_str = args.to_string();
        if as_str.len() > 50 {
            format!("{}...", as_str.chars().take(47).collect::<String>())
        } else {
            as_str
        }
    };

    if details.is_empty() {
        format!("{}{}{}", COLOR_BOLD, friendly_name, COLOR_RESET)
    } else {
        format!("{}{}{}({})", COLOR_BOLD, friendly_name, COLOR_RESET, details)
    }
}

async fn send_progress_update(session_key: &str, text: &str) {
    let actual_session = crate::agent::style::spinner::get_current_session_key().unwrap_or_else(|| session_key.to_string());
    if actual_session.starts_with("telegram:") {
        if let Some(chat_id_str) = actual_session.strip_prefix("telegram:") {
            if let Ok(chat_id) = chat_id_str.parse::<i64>() {
                if let Some((bot_token, client)) = crate::channels::telegram::get_telegram_bot_info() {
                    let send_url = format!("https://api.telegram.org/bot{}/sendMessage", bot_token);
                    let payload = serde_json::json!({
                        "chat_id": chat_id,
                        "text": text,
                        "parse_mode": "Markdown"
                    });
                    let _ = client.post(&send_url).json(&payload).send().await;
                }
            }
        }
    } else if actual_session.starts_with("discord:") {
        if let Some(channel_id) = actual_session.strip_prefix("discord:") {
            if let Some((bot_token, client)) = crate::channels::discord::get_discord_bot_info() {
                let send_url = format!("https://discord.com/api/v10/channels/{}/messages", channel_id);
                let payload = serde_json::json!({
                    "content": text
                });
                let _ = client.post(&send_url)
                    .header("Authorization", format!("Bot {}", bot_token))
                    .json(&payload)
                    .send()
                    .await;
            }
        }
    } else if actual_session.starts_with("whatsapp:") {
        if let Some(phone_number) = actual_session.strip_prefix("whatsapp:") {
            if let Some((api_key, phone_number_id, client)) = crate::channels::whatsapp::get_whatsapp_bot_info() {
                let send_url = format!("https://graph.facebook.com/v18.0/{}/messages", phone_number_id);
                let payload = serde_json::json!({
                    "messaging_product": "whatsapp",
                    "recipient_type": "individual",
                    "to": phone_number,
                    "type": "text",
                    "text": {
                        "body": text
                    }
                });
                let _ = client.post(&send_url)
                    .bearer_auth(&api_key)
                    .json(&payload)
                    .send()
                    .await;
            }
        }
    }
}

fn generate_self_healing_hint(tool_name: &str, error_str: &str) -> String {
    let err_lower = error_str.to_lowercase();
    let tool_lower = tool_name.to_lowercase();

    if tool_lower.contains("read") || tool_lower.contains("write") || tool_lower.contains("patch") || tool_lower.contains("replace") || tool_lower.contains("file") {
        if err_lower.contains("notfound") || err_lower.contains("no such file") {
            return "The target path does not exist. Ensure the file path is correct and absolute. You can use 'list_dir' or 'find_files' to check the folder contents.".to_string();
        }
        if err_lower.contains("permission") || err_lower.contains("denied") {
            return "Permission denied. The agent process does not have access to read/write this path. Ensure you are targeting files within the permitted workspace folder.".to_string();
        }
    }

    if tool_lower.contains("exec") || tool_lower.contains("shell") || tool_lower.contains("command") {
        if err_lower.contains("permission") || err_lower.contains("denied") {
            return "Execution permission denied. You may need to make the file executable via 'chmod +x <path>' or run the script using an explicit interpreter (e.g. 'bash <script_path>').".to_string();
        }
        if err_lower.contains("not found") || err_lower.contains("127") || err_lower.contains("no such file") {
            return "Command or script executable not found. Verify the path or binary name is correct and check if the required tool is installed on the system.".to_string();
        }
        if err_lower.contains("seccomp") || err_lower.contains("sandbox") || err_lower.contains("operation not permitted") {
            return "Operation blocked by the seccomp BPF sandbox. Note that networking syscalls (e.g. curl, wget, git push), mount/umount, and other privileged actions are forbidden in the sandboxed environment. Please perform the action without network or sandbox-restricted system calls, or run locally via a different approved script if possible.".to_string();
        }
    }

    if err_lower.contains("mcp") || err_lower.contains("connection") || err_lower.contains("broken pipe") || err_lower.contains("bridge") {
        return "MCP server connection error. The MCP server process might be offline or failed to initialize. Try using the 'manage_mcp' tool to list, configure, or restart the active MCP servers.".to_string();
    }

    if tool_lower.contains("delegate") || tool_lower.contains("research") || tool_lower.contains("optimizer") || tool_lower.contains("loop") {
        return "Subagent execution encountered an error. You can use the 'optimize_subagent' tool to refine the subagent system prompt/instructions to handle the issue better, or try breaking the goal down into smaller, simpler tasks for subagent delegation.".to_string();
    }

    // Default suggestion
    "Please double-check the arguments format, verify the target file paths or command options exist, and try a different tool or approach if this error persists.".to_string()
}

fn count_previous_tool_calls(messages: &[Message], tool_name: &str, tool_args: &serde_json::Value) -> usize {
    let last_user_idx = messages.iter().rposition(|m| m.role == "user").unwrap_or(0);
    let mut count = 0;
    for msg in &messages[last_user_idx..] {
        if msg.role == "assistant" {
            if let Some(tool_calls) = msg.extra.get("tool_calls").and_then(|v| v.as_array()) {
                for tc in tool_calls {
                    let name = tc.get("name").and_then(|v| v.as_str())
                        .or_else(|| tc.get("function").and_then(|f| f.get("name")).and_then(|v| v.as_str()));
                    let args = tc.get("arguments")
                        .or_else(|| tc.get("function").and_then(|f| f.get("arguments")));
                    
                    if let (Some(name_str), Some(args_val)) = (name, args) {
                        if name_str == tool_name {
                            let match_args = if args_val.is_string() {
                                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(args_val.as_str().unwrap()) {
                                    parsed == *tool_args
                                } else {
                                    false
                                }
                            } else {
                                args_val == tool_args
                            };
                            if match_args {
                                count += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    count
}

fn count_previous_text_responses(messages: &[Message], next_content: &str) -> usize {
    if next_content.trim().is_empty() {
        return 0;
    }
    let last_user_idx = messages.iter().rposition(|m| m.role == "user").unwrap_or(0);
    let mut count = 0;
    let next_trimmed = next_content.trim();
    for msg in &messages[last_user_idx..] {
        if msg.role == "assistant" && !msg.content.trim().is_empty()
            && msg.content.trim() == next_trimmed {
                count += 1;
            }
    }
    count
}

fn format_markdown_line(line: &str) -> String {
    static RE_BOLD: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    static RE_CODE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    static RE_ITALIC: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();

    let re_bold = RE_BOLD.get_or_init(|| regex::Regex::new(r"\*\*(.*?)\*\*").unwrap());
    let re_code = RE_CODE.get_or_init(|| regex::Regex::new(r"`(.*?)`").unwrap());
    let re_italic = RE_ITALIC.get_or_init(|| regex::Regex::new(r"\*(.*?)\*").unwrap());

    let light_blue = "\x1b[38;2;135;206;250m";
    
    let trimmed = line.trim();
    if trimmed.chars().all(|c| c == '-') && trimmed.len() >= 3 && !trimmed.is_empty() {
        return format!("{}──────{}", LIGHT_WHITE, COLOR_RESET);
    }

    if line.trim_start().starts_with("#") {
        return format!("{}{}{}", HEADING_BLUE, line, COLOR_RESET);
    }

    let mut formatted = line.to_string();
    formatted = formatted
        .replace("✔", &format!("{}{}{}", EMERALD_GREEN, "✔", COLOR_RESET))
        .replace("✅", &format!("{}{}{}", EMERALD_GREEN, "✅", COLOR_RESET))
        .replace("✓", &format!("{}{}{}", EMERALD_GREEN, "✓", COLOR_RESET))
        .replace("✖", &format!("{}{}{}", ERROR_RED, "✖", COLOR_RESET))
        .replace("❌", &format!("{}{}{}", ERROR_RED, "❌", COLOR_RESET))
        .replace("✗", &format!("{}{}{}", ERROR_RED, "✗", COLOR_RESET));

    formatted = re_bold.replace_all(&formatted, &format!("{}{}$1{}", RED_ORANGE, COLOR_BOLD, COLOR_RESET)).to_string();
    formatted = re_code.replace_all(&formatted, &format!("{}$1{}", light_blue, COLOR_RESET)).to_string();
    formatted = re_italic.replace_all(&formatted, &format!("{}$1{}", light_blue, COLOR_RESET)).to_string();

    formatted
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_format_tool_args() {
        // html_to_video
        let args = json!({
            "html_path": "src/index.html",
            "output_path": "video.mp4"
        });
        let formatted = format_tool_args("html_to_video", &args);
        assert!(formatted.contains("HtmlVideo"));
        assert!(formatted.contains("html: \"index.html\""));
        assert!(formatted.contains("output: \"video.mp4\""));

        // generate_video
        let args = json!({
            "composition_json": "{}",
            "output_path": "path/to/my_output_file.mp4"
        });
        let formatted = format_tool_args("generate_video", &args);
        assert!(formatted.contains("Video"));
        assert!(formatted.contains("output: \"my_output_file.mp4\""));

        // obscura_browser
        let args = json!({
            "url": "https://a-very-long-url-that-exceeds-thirty-characters.com/index.html",
            "action": "render"
        });
        let formatted = format_tool_args("obscura_browser", &args);
        assert!(formatted.contains("Obscura"));
        assert!(formatted.contains("url: \"https://a-very-long-url-tha...\""));

        // fallback with clamping
        let args = json!({
            "some_huge_mcp_param": "this is a very long string that should get truncated during formatting to keep things clean"
        });
        let formatted = format_tool_args("mcp_custom_tool", &args);
        assert!(formatted.contains("mcp_custom_tool"));
        assert!(formatted.contains("some_huge_mcp_param: \"this is a very lo...\""));
    }
}
