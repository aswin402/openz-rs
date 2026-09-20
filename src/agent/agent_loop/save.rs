use super::{AgentLoop, TurnContext, TurnState, get_session_lock};
use crate::memory::with_graph_db;
use crate::agent::style::*;
use anyhow::Result;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

static CURATOR_LAST_SPAWN: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();

fn self_improvement_review_prompt() -> &'static str {
    "You are a specialized Self-Improvement Curator for OpenZ. Your job is to review the conversation between the User and the AI Agent and consolidate five types of learnings:\n\n\
1. USER PREFERENCES (USER.md): Durable user preferences, aesthetic styling choices, favorite design themes, color palettes, fonts, preferred programming languages, frameworks, or persona constraints. (e.g. 'User prefers Cyberpunk Neon design with #00ffcc and #ff007f accents', 'Always use Tailwind CSS v4', 'Never explain code unless asked').\n\
2. DELIVERABLE & PROCEDURAL SKILLS (SKILL.md): Whenever the agent creates, modifies, or inspects deliverables (websites, scripts, scrapers, data pipelines, media animations in openz_vault/ or project folders) or solves complex multi-step technical challenges:\n\
   - Extract a clean, reusable procedural skill.\n\
   - Include standard YAML frontmatter with:\n\
     ---\n\
     name: <name_lowercase_with_underscores>\n\
     description: <what this skill accomplishes>\n\
     triggers: [\"<intent keyword 1>\", \"<intent keyword 2>\"]\n\
     vault_category: <websites|scripts|images|videos|documents|presentations|data|diagrams|templates|exports>\n\
     ---\n\
   - Under the frontmatter, provide Markdown instructions with Design System / Tech Rules, File Locations (e.g. openz_vault/websites/...), and Step-by-Step Reproduction/Extension Steps.\n\
3. GENERAL GUIDELINE SKILLS: Task-specific coding workarounds, bug fixes, or library quirks (e.g. 'cargo_build_fix', 'sqlite_lock_workaround').\n\
4. SOURCES: High-trust reusable URLs or documentation links.\n\
5. WORKFLOWS: Reusable multi-step procedures with triggers, steps, preconditions, verification, risk, and status.\n\n\
CRITICAL: Pay special attention to tool execution outcomes. If a tool call failed with an error, look at how the agent resolved it (or what workaround succeeded). Extract this learning into a reusable skill or workflow.\n\n\
Specific workflow patterns to preserve when observed:\n\
- Large static website or generated source creation: split writes into safe chunks instead of one huge tool payload; verify the file exists; open only after successful verification.\n\
- HTML/video generation longer than roughly 20 seconds: render in shorter segments, concatenate with ffmpeg, verify duration, then open.\n\
- Dev servers: when exec_command returns server_registered=true, record the server id and use manage_servers to stop it automatically when the preview/verification task is finished.\n\
- Feature/tool inventory answers: use openz_inventory/tool_catalog instead of answering exact counts from memory.\n\n\
You MUST return your response as a raw JSON object with the following structure. Do not wrap it in conversational text. Return empty arrays when there is nothing to save.\n\n\
JSON Format:\n\
{\n\
   \"memory_updated\": true/false,\n\
   \"memory_content\": \"<updated memory markdown content. If memory_updated is false, keep it identical to existing memory or empty>\",\n\
   \"user_preferences\": \"<bullet points of durable user preferences, design choices, styling tastes, or conventions to record in USER.md>\",\n\
   \"skills_to_save\": [\n\
     {\n\
       \"name\": \"<name of skill, lowercase with underscores>\",\n\
       \"description\": \"<one-sentence summary of when to use this skill>\",\n\
       \"triggers\": [\"<keyword or user intent phrase>\"],\n\
       \"vault_category\": \"websites|scripts|images|videos|documents|presentations|data|diagrams|templates|exports\",\n\
       \"content\": \"<complete markdown content for the skill including YAML frontmatter with name, description, triggers, vault_category, followed by headers, design rules, paths, and examples.>\"\n\
     }\n\
   ],\n\
   \"sources_to_save\": [\n\
     {\n\
       \"label\": \"<short source label>\",\n\
       \"kind\": \"repo|docs|article|paper|other\",\n\
       \"uri\": \"<url or local path>\",\n\
       \"aliases\": [\"<search alias>\"],\n\
       \"summary\": \"<brief source summary>\",\n\
       \"trust_score\": 0.6,\n\
       \"stale_after_secs\": 604800\n\
     }\n\
   ],\n\
   \"workflows_to_save\": [\n\
     {\n\
       \"name\": \"<workflow name, lowercase_with_underscores>\",\n\
       \"triggers\": [\"<phrases that should match this workflow>\"],\n\
       \"summary\": \"<one sentence summary>\",\n\
       \"steps\": [{\"step\": \"<action>\", \"tool\": \"<optional tool name>\"}],\n\
       \"preconditions\": [\"<required setup>\"],\n\
       \"verification\": [\"<how to verify success>\"],\n\
       \"risk\": \"low|normal|high\",\n\
       \"status\": \"active|draft\"\n\
     }\n\
   ]\n\
}"
}

static LAST_CURATOR_HANDLE: std::sync::OnceLock<tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>> =
    std::sync::OnceLock::new();

pub fn set_last_curator_handle(handle: tokio::task::JoinHandle<()>) {
    let mutex = LAST_CURATOR_HANDLE.get_or_init(|| tokio::sync::Mutex::new(None));
    if let Ok(mut guard) = mutex.try_lock() {
        *guard = Some(handle);
    }
}

pub async fn wait_for_curator(timeout_dur: Duration) -> bool {
    let mutex = LAST_CURATOR_HANDLE.get_or_init(|| tokio::sync::Mutex::new(None));
    let mut guard = mutex.lock().await;
    if let Some(handle) = guard.take() {
        let _ = tokio::time::timeout(timeout_dur, handle).await;
        true
    } else {
        false
    }
}

fn should_spawn_curator(session_key: &str, debounce: Duration) -> bool {
    let now = Instant::now();
    let registry = CURATOR_LAST_SPAWN.get_or_init(|| Mutex::new(HashMap::new()));
    let Ok(mut guard) = registry.lock() else {
        return true;
    };
    if let Some(last) = guard.get(session_key) {
        if now.duration_since(*last) < debounce {
            return false;
        }
    }
    guard.insert(session_key.to_string(), now);
    true
}

#[derive(Deserialize, Default, Clone, Debug)]
pub(crate) struct ReviewSkill {
    #[serde(default)]
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) description: Option<String>,
    #[serde(default)]
    pub(crate) triggers: Vec<String>,
    #[serde(default)]
    pub(crate) vault_category: Option<String>,
    #[serde(default)]
    pub(crate) content: String,
}

#[derive(Deserialize, Clone, Debug)]
pub(crate) struct ReviewSource {
    pub(crate) label: String,
    pub(crate) kind: Option<String>,
    pub(crate) uri: String,
    #[serde(default)]
    pub(crate) aliases: Vec<String>,
    pub(crate) summary: Option<String>,
    pub(crate) trust_score: Option<f64>,
    pub(crate) stale_after_secs: Option<i64>,
}

#[derive(Deserialize, Clone, Debug)]
pub(crate) struct ReviewWorkflow {
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) triggers: Vec<String>,
    pub(crate) summary: String,
    #[serde(default)]
    pub(crate) steps: serde_json::Value,
    #[serde(default)]
    pub(crate) preconditions: Vec<String>,
    #[serde(default)]
    pub(crate) verification: Vec<String>,
    pub(crate) risk: Option<String>,
    pub(crate) status: Option<String>,
}

#[derive(Deserialize, Default, Debug)]
pub(crate) struct ReviewResponse {
    #[serde(default)]
    pub(crate) memory_updated: bool,
    #[serde(default)]
    pub(crate) memory_content: String,
    #[serde(default)]
    pub(crate) user_preferences: String,
    #[serde(default)]
    pub(crate) skills_to_save: Vec<ReviewSkill>,
    #[serde(default)]
    pub(crate) sources_to_save: Vec<ReviewSource>,
    #[serde(default)]
    pub(crate) workflows_to_save: Vec<ReviewWorkflow>,
}

pub(crate) fn fallback_parse_curator(raw: &str) -> Option<ReviewResponse> {
    let mut resp = ReviewResponse::default();
    let mut any_salvaged = false;

    // 1. Extract user_preferences
    if let Some(pos) = raw.find("\"user_preferences\"") {
        let after = &raw[pos + "\"user_preferences\"".len()..];
        if let Some(colon_pos) = after.find(':') {
            let val_part = after[colon_pos + 1..].trim_start();
            if let Some(inner) = val_part.strip_prefix('"') {
                if let Some(end_quote) = inner.find('"') {
                    let prefs = inner[..end_quote].replace("\\n", "\n").replace("\\\"", "\"");
                    if !prefs.trim().is_empty() {
                        resp.user_preferences = prefs;
                        any_salvaged = true;
                    }
                }
            }
        }
    }

    // 2. Extract memory_content
    if let Some(pos) = raw.find("\"memory_content\"") {
        let after = &raw[pos + "\"memory_content\"".len()..];
        if let Some(colon_pos) = after.find(':') {
            let val_part = after[colon_pos + 1..].trim_start();
            if let Some(inner) = val_part.strip_prefix('"') {
                if let Some(end_quote) = inner.find('"') {
                    let mem = inner[..end_quote].replace("\\n", "\n").replace("\\\"", "\"");
                    if !mem.trim().is_empty() {
                        resp.memory_content = mem;
                        resp.memory_updated = true;
                        any_salvaged = true;
                    }
                }
            }
        }
    }

    // 3. Extract skills
    if let Some(skills_pos) = raw.find("\"skills_to_save\"") {
        let after_skills = &raw[skills_pos + "\"skills_to_save\"".len()..];
        if let Some(bracket_start) = after_skills.find('[') {
            let inside_array = &after_skills[bracket_start + 1..];
            let mut depth = 0;
            let mut obj_start = None;
            for (idx, ch) in inside_array.char_indices() {
                match ch {
                    '{' => {
                        if depth == 0 {
                            obj_start = Some(idx);
                        }
                        depth += 1;
                    }
                    '}' => {
                        if depth > 0 {
                            depth -= 1;
                            if depth == 0 {
                                if let Some(start) = obj_start {
                                    let obj_str = &inside_array[start..=idx];
                                    if let Ok(skill) = serde_json::from_str::<ReviewSkill>(obj_str) {
                                        if !skill.name.is_empty() && !skill.content.is_empty() {
                                            resp.skills_to_save.push(skill);
                                            any_salvaged = true;
                                        }
                                    }
                                }
                                obj_start = None;
                            }
                        }
                    }
                    ']' if depth == 0 => break,
                    _ => {}
                }
            }
        }
    }

    // 4. Extract sources
    if let Some(sources_pos) = raw.find("\"sources_to_save\"") {
        let after = &raw[sources_pos + "\"sources_to_save\"".len()..];
        if let Some(bracket_start) = after.find('[') {
            let inside_array = &after[bracket_start + 1..];
            let mut depth = 0;
            let mut obj_start = None;
            for (idx, ch) in inside_array.char_indices() {
                match ch {
                    '{' => {
                        if depth == 0 {
                            obj_start = Some(idx);
                        }
                        depth += 1;
                    }
                    '}' => {
                        if depth > 0 {
                            depth -= 1;
                            if depth == 0 {
                                if let Some(start) = obj_start {
                                    let obj_str = &inside_array[start..=idx];
                                    if let Ok(source) = serde_json::from_str::<ReviewSource>(obj_str) {
                                        if !source.label.trim().is_empty() && !source.uri.trim().is_empty() {
                                            resp.sources_to_save.push(source);
                                            any_salvaged = true;
                                        }
                                    }
                                }
                                obj_start = None;
                            }
                        }
                    }
                    ']' if depth == 0 => break,
                    _ => {}
                }
            }
        }
    }

    // 5. Extract workflows
    if let Some(workflows_pos) = raw.find("\"workflows_to_save\"") {
        let after = &raw[workflows_pos + "\"workflows_to_save\"".len()..];
        if let Some(bracket_start) = after.find('[') {
            let inside_array = &after[bracket_start + 1..];
            let mut depth = 0;
            let mut obj_start = None;
            for (idx, ch) in inside_array.char_indices() {
                match ch {
                    '{' => {
                        if depth == 0 {
                            obj_start = Some(idx);
                        }
                        depth += 1;
                    }
                    '}' => {
                        if depth > 0 {
                            depth -= 1;
                            if depth == 0 {
                                if let Some(start) = obj_start {
                                    let obj_str = &inside_array[start..=idx];
                                    if let Ok(wf) = serde_json::from_str::<ReviewWorkflow>(obj_str) {
                                        if !wf.name.trim().is_empty() && !wf.summary.trim().is_empty() {
                                            resp.workflows_to_save.push(wf);
                                            any_salvaged = true;
                                        }
                                    }
                                }
                                obj_start = None;
                            }
                        }
                    }
                    ']' if depth == 0 => break,
                    _ => {}
                }
            }
        }
    }

    if any_salvaged {
        Some(resp)
    } else {
        None
    }
}

pub async fn handle(loop_ref: &AgentLoop, ctx: &mut TurnContext<'_>) -> Result<TurnState> {
    let config = &ctx.config;
    loop_ref.session_manager.save(&ctx.session).await?;
    if let Err(e) = crate::tools::onpkg::sync_onpkg_manifest() {
        tracing::warn!("Failed to synchronize onpkg manifest: {}", e);
    }
    tracing::info!(session = %ctx.session_key, "Session saved successfully. Turn complete.");

    let traces_dir = crate::config::traces_dir();
    if let Err(e) = std::fs::create_dir_all(&traces_dir) {
        tracing::error!(
            "{}▲ Failed to create traces directory: {}{}",
            AURA_GOLD,
            e,
            COLOR_RESET
        );
    } else {
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
        let trace_file = traces_dir.join(format!(
            "trace_{}_{}.json",
            ctx.session_key.replace(":", "_"),
            timestamp
        ));
        let trace_record = serde_json::json!({
            "session_key": ctx.session_key,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "user_query": ctx.user_content,
            "system_prompt": ctx.system_prompt,
            "model": config.agents.defaults.model,
            "messages": ctx.messages,
            "tools_used": ctx.tools_used.clone(),
            "final_response": ctx.final_content,
        });
        if let Ok(content) = serde_json::to_string_pretty(&trace_record) {
            let _ = std::fs::write(trace_file, content);
        }
    }

    if !ctx.user_content.starts_with('/') {
        if !should_spawn_curator(ctx.session_key, Duration::from_secs(20)) {
            tracing::debug!(
                session = %ctx.session_key,
                "Skipping self-improvement curator spawn during debounce window"
            );
            return Ok(TurnState::Done);
        }
        let session_manager = loop_ref.session_manager.clone();
        let session_key = ctx.session_key.to_string();
        let provider = ctx.active_provider.clone();
        let messages = ctx.messages.clone();
        let tools_used = ctx.tools_used.clone();
        let initial_updated_at = ctx.session.updated_at;
        let initial_msg_count = ctx.session.messages.len();

        let handle = tokio::spawn(async move {
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

            let write_log = |status: &str,
                             memory_updated: bool,
                             skills_saved: Vec<String>,
                             error_message: Option<String>| {
                #[derive(serde::Serialize)]
                struct CuratorStatus {
                    last_run_timestamp: String,
                    status: String,
                    session_key: String,
                    memory_updated: bool,
                    skills_saved: Vec<String>,
                    error_message: Option<String>,
                }
                let log_path = crate::config::runtime_data_dir().join("curator_status.json");
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
            if estimated_tokens >= 4000 && tools_used.len() >= 3 {
                should_run = true;
            } else {
                for tool in &tools_used {
                    let t = tool.to_lowercase();
                    // Trigger curator for state-modifying, memory, skill, or execution tools
                    if t.contains("write_file")
                        || t.contains("patch_file")
                        || t.contains("replace_lines")
                        || t.contains("zenflow_edit")
                        || t.contains("db_write")
                        || t.contains("cargo")
                        || t.contains("exec_command")
                        || t.contains("obscura")
                        || t.contains("gsd_browser")
                        || t.contains("remote_input")
                        || t.contains("mcp")
                        || t.contains("curate_skill")
                        || t.contains("store_memory")
                        || t.contains("knowledge_source")
                        || t.contains("workflow_memory")
                        || t.contains("graph_memory")
                    {
                        should_run = true;
                        break;
                    }
                }
                if !should_run {
                    // Check if the user prompt explicitly expresses learning or memory intent
                    for msg in messages.iter().rev().take(3) {
                        if msg.role == "user" {
                            let text = msg.content.to_lowercase();
                            if text.contains("remember")
                                || text.contains("learn")
                                || text.contains("preference")
                                || text.contains("guideline")
                                || text.contains("save this")
                                || text.contains("skill")
                            {
                                should_run = true;
                                break;
                            }
                        }
                    }
                }
            }

            if !should_run {
                write_log("skipped: throttled (simple turn)", false, vec![], None);
                return;
            }

            tracing::info!(session = %session_key, "Self-improvement curator: started processing.");
            write_log("running", false, vec![], None);

            let _ = crate::agent::skills::archive_stale_skills();
            let _ = crate::tools::shared_memory::consolidate_shared_memory(&provider).await;

            let recent_interactions = crate::tools::shared_memory::get_recent_interactions(15)
                .await
                .unwrap_or_default();

            let existing_memory = if let Ok(s) = session_manager.load_async(&session_key).await {
                s.metadata
                    .get("memory")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string()
            } else {
                String::new()
            };

            let mut existing_skills_desc = String::new();
            if let Ok(skills) =
                crate::agent::skills::load_skills_with_profile(profile_name.as_deref())
            {
                for skill in skills {
                    existing_skills_desc.push_str(&format!(
                        "Skill Name: {}\nContent:\n{}\n\n",
                        skill.name, skill.content
                    ));
                }
            }

            let system_prompt_review = self_improvement_review_prompt();

            let mut prompt_content = String::new();

            if !recent_interactions.is_empty() {
                prompt_content.push_str("Recent user tasks across all sessions:\n");
                for (i, item) in recent_interactions.iter().enumerate() {
                    let query = item["query"].as_str().unwrap_or("");
                    let success = item["success"].as_bool().unwrap_or(true);
                    let errors = item["errors"].as_str().unwrap_or("");
                    prompt_content.push_str(&format!(
                        "{}. Task: \"{}\" | Status: {}\n",
                        i + 1,
                        query,
                        if success { "SUCCESS" } else { "FAILED" }
                    ));
                    if !errors.is_empty() {
                        prompt_content.push_str(&format!("   Errors encountered: {}\n", errors));
                    }
                }
                prompt_content.push('\n');
            }

            let tool_count = messages.iter().filter(|m| m.role == "tool").count();
            if tool_count >= 5 {
                prompt_content.push_str(&format!(
                    "[SYSTEM NOTICE: The recent task was complex and involved {} tool executions. Review the successful trajectory and extract a reusable skill so the agent can perform this category of work efficiently next time.]\n\n",
                    tool_count
                ));
            }

            let created_deliverable = messages.iter().any(|m| {
                if let Some(tool_calls) = m.extra.get("tool_calls").and_then(|v| v.as_array()) {
                    tool_calls.iter().any(|tc| {
                        let name = tc.get("name").and_then(|v| v.as_str()).or_else(|| {
                            tc.get("function").and_then(|f| f.get("name")).and_then(|v| v.as_str())
                        }).unwrap_or("");
                        let args = tc.get("arguments").or_else(|| {
                            tc.get("function").and_then(|f| f.get("arguments"))
                        }).map(|v| v.to_string()).unwrap_or_default();
                        (name == "write_file" || name == "patch_file" || name == "replace_lines" || name == "generate_image")
                            && (args.contains("openz_vault")
                                || args.contains("index.html")
                                || args.contains(".css")
                                || args.contains(".js")
                                || args.contains(".py")
                                || args.contains(".sh")
                                || args.contains(".rs"))
                    })
                } else {
                    false
                }
            });

            if created_deliverable {
                prompt_content.push_str("[DELIVERABLE DETECTED: The agent produced project files or creative deliverables in this turn. You MUST synthesize a reusable procedural skill in `skills_to_save` documenting the architecture, design choices, colors/styling, tech stack, and step-by-step procedure used to build this deliverable so subsequent requests can replicate or build upon it.]\n\n");
            }

            let existing_user_profile = crate::agent::skills::load_user_profile();
            if !existing_user_profile.is_empty() {
                prompt_content.push_str(&format!("Existing User Profile & Preferences (USER.md):\n{}\n\n", existing_user_profile));
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
                        if let Some(reasoning) =
                            msg.extra.get("reasoning_content").and_then(|v| v.as_str())
                        {
                            if !reasoning.is_empty() {
                                prompt_content.push_str(&format!("  Thinking:\n{}\n", reasoning));
                            }
                        }
                        if let Some(tool_calls) =
                            msg.extra.get("tool_calls").and_then(|v| v.as_array())
                        {
                            if !tool_calls.is_empty() {
                                prompt_content.push_str("  Tool Calls:\n");
                                for tc in tool_calls {
                                    let name =
                                        tc.get("name").and_then(|v| v.as_str()).or_else(|| {
                                            tc.get("function")
                                                .and_then(|f| f.get("name"))
                                                .and_then(|v| v.as_str())
                                        });
                                    let args = tc.get("arguments").or_else(|| {
                                        tc.get("function").and_then(|f| f.get("arguments"))
                                    });

                                    if let (Some(name_str), Some(args_val)) = (name, args) {
                                        let args_str = args_val.to_string();
                                        let args_truncated = if args_str.len() > 1000 {
                                            let truncated: String =
                                                args_str.chars().take(1000).collect();
                                            format!(
                                                "{}... [TRUNCATED - {} bytes]",
                                                truncated,
                                                args_str.len() - 1000
                                            )
                                        } else {
                                            args_str
                                        };
                                        prompt_content.push_str(&format!(
                                            "    - Call tool '{}' with arguments: {}\n",
                                            name_str, args_truncated
                                        ));
                                    }
                                }
                            }
                        }
                        if !msg.content.is_empty() {
                            let content_truncated = if msg.content.len() > 2000 {
                                let truncated: String = msg.content.chars().take(2000).collect();
                                format!(
                                    "{}... [TRUNCATED - {} bytes]",
                                    truncated,
                                    msg.content.len() - 2000
                                )
                            } else {
                                msg.content.clone()
                            };
                            prompt_content
                                .push_str(&format!("  Response: {}\n", content_truncated));
                        }
                    }
                    "tool" => {
                        let tool_name = msg
                            .extra
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown");
                        let content_truncated = if msg.content.len() > 2000 {
                            let truncated: String = msg.content.chars().take(2000).collect();
                            format!(
                                "{}... [TRUNCATED {} bytes]",
                                truncated,
                                msg.content.len() - 2000
                            )
                        } else {
                            msg.content.clone()
                        };
                        prompt_content.push_str(&format!(
                            "[tool output for '{}']:\n{}\n",
                            tool_name, content_truncated
                        ));
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

            match provider
                .chat(system_prompt_review, &review_msgs, &[], &settings)
                .await
            {
                Ok(resp) => {
                    if let Some(content) = resp.content {
                        let trimmed = content.trim();
                        let clean_json = if let Some(start_idx) = trimmed.find("```json") {
                            let after_start = &trimmed[start_idx + 7..];
                            if let Some(end_idx) = after_start.rfind("```") {
                                after_start[..end_idx].trim().to_string()
                            } else {
                                after_start.trim().to_string()
                            }
                        } else if let Some(start_idx) = trimmed.find("```") {
                            let after_start = &trimmed[start_idx + 3..];
                            if let Some(end_idx) = after_start.rfind("```") {
                                after_start[..end_idx].trim().to_string()
                            } else {
                                after_start.trim().to_string()
                            }
                        } else {
                            if let (Some(first_brace), Some(last_brace)) =
                                (trimmed.find('{'), trimmed.rfind('}'))
                            {
                                if first_brace < last_brace {
                                    trimmed[first_brace..=last_brace].to_string()
                                } else {
                                    trimmed.to_string()
                                }
                            } else {
                                trimmed.to_string()
                            }
                        };

                        let (review_opt, was_salvaged) = match serde_json::from_str::<ReviewResponse>(&clean_json) {
                            Ok(review) => (Some(review), false),
                            Err(e) => {
                                tracing::warn!(session = %session_key, error = %e, "JSON deserialization failed, attempting fallback parsing");
                                (fallback_parse_curator(&clean_json), true)
                            }
                        };

                        if let Some(review) = review_opt {
                            if was_salvaged {
                                tracing::info!(session = %session_key, "Successfully salvaged curator learnings via fallback parser");
                            }

                            // 1. Update durable user preferences in ~/.openz/USER.md
                            if !review.user_preferences.trim().is_empty() {
                                if let Err(e) = crate::agent::skills::update_user_profile(&review.user_preferences) {
                                    tracing::warn!(session = %session_key, error = %e, "Failed to update USER.md preferences");
                                } else {
                                    tracing::info!(session = %session_key, "Self-improvement curator: updated USER.md preferences.");
                                    crate::channels::cli::send_notification(&format!(
                                        "{}◇ [Self-Improvement] User profile preferences updated (~/.openz/USER.md).{}",
                                        AURA_BLUE, COLOR_RESET
                                    ));
                                }
                            }

                            if review.memory_updated {
                                let lock = get_session_lock(&session_key);
                                let _guard = lock.lock().await;
                                if let Ok(mut latest_session) =
                                    session_manager.load_async(&session_key).await
                                {
                                    if latest_session.updated_at != initial_updated_at
                                        || latest_session.messages.len() != initial_msg_count
                                    {
                                        let msg = "Skipped memory update: session was modified concurrently".to_string();
                                        error_msg = Some(msg.clone());
                                        tracing::debug!(
                                            session = %session_key,
                                            "Self-improvement curator skipped stale write because session changed concurrently"
                                        );
                                    } else {
                                        latest_session.metadata.insert(
                                            "memory".to_string(),
                                            serde_json::Value::String(
                                                review.memory_content.trim().to_string(),
                                            ),
                                        );
                                        if let Err(e) =
                                            session_manager.save(&latest_session).await
                                        {
                                            let msg = format!("Failed to save memory: {}", e);
                                            error_msg = Some(msg.clone());
                                            crate::channels::cli::send_notification(&format!(
                                                "{}▲ [Self-Improvement] Failed to save self-improvement memory: {}{}",
                                                AURA_GOLD, e, COLOR_RESET
                                            ));
                                        } else {
                                            memory_updated = true;
                                            tracing::info!(session = %session_key, "Self-improvement curator: updated session memory.");
                                            crate::channels::cli::send_notification(&format!(
                                                "{}◇ [Self-Improvement] Memory updated based on recent conversation.{}",
                                                AURA_BLUE, COLOR_RESET
                                            ));
                                            crate::channels::websocket::publish_activity_notice(
                                                &session_key,
                                                "self_improvement",
                                                "Self-improvement memory updated",
                                                "Recent conversation learnings were saved",
                                            );

                                            let facts: Vec<String> = review
                                                .memory_content
                                                .lines()
                                                .map(|line| line.trim())
                                                .filter(|line| {
                                                    line.starts_with('-')
                                                        || line.starts_with('*')
                                                })
                                                .map(|line| {
                                                    let fact = line[1..].trim();
                                                    fact.trim_start_matches("**")
                                                        .trim_end_matches("**")
                                                        .trim()
                                                        .to_string()
                                                })
                                                .filter(|fact| !fact.is_empty())
                                                .collect();

                                            let uid = "*";
                                            let sid = &session_key;
                                            let aid = "*";

                                            for fact in facts {
                                                let node_id = format!(
                                                    "fact_{}",
                                                    &uuid::Uuid::new_v4().to_string()[..8]
                                                );
                                                let timestamp = chrono::Utc::now().to_rfc3339();
                                                let _ = with_graph_db(
                                                    |conn| {
                                                        let mut check_stmt = conn.prepare(
                                                        "SELECT 1 FROM semantic_metadata WHERE raw_text = ?1 AND valid_until IS NULL"
                                                    ).map_err(|e| anyhow::anyhow!(e))?;
                                                        let exists = check_stmt
                                                            .exists(rusqlite::params![&fact])
                                                            .map_err(|e| anyhow::anyhow!(e))?;
                                                        if !exists {
                                                            conn.execute(
                                                            "INSERT INTO semantic_metadata (node_id, raw_text, timestamp, importance, user_id, session_id, agent_id)
                                                             VALUES (?1, ?2, ?3, 0.8, ?4, ?5, ?6)",
                                                            rusqlite::params![node_id, fact, timestamp, uid, sid, aid],
                                                        ).map_err(|e| anyhow::anyhow!(e))?;
                                                            let _ = conn.execute(
                                                            "INSERT INTO semantic_fts (node_id, raw_text) VALUES (?1, ?2)",
                                                            rusqlite::params![node_id, fact],
                                                        );
                                                        }
                                                        Ok(())
                                                    },
                                                );
                                            }
                                        }
                                    }
                                }
                            }

                            for skill in review.skills_to_save {
                                if !skill.name.is_empty() && !skill.content.is_empty() {
                                    let final_content = if !skill.content.trim_start().starts_with("---") {
                                        let mut fm = String::from("---\n");
                                        fm.push_str(&format!("name: {}\n", skill.name));
                                        if let Some(ref desc) = skill.description {
                                            fm.push_str(&format!("description: {}\n", desc.replace('\n', " ")));
                                        } else {
                                            fm.push_str(&format!("description: Procedural skill for {}\n", skill.name));
                                        }
                                        if !skill.triggers.is_empty() {
                                            fm.push_str("triggers:\n");
                                            for t in &skill.triggers {
                                                fm.push_str(&format!("  - \"{}\"\n", t.replace('"', "\\\"")));
                                            }
                                        }
                                        if let Some(ref cat) = skill.vault_category {
                                            fm.push_str(&format!("vault_category: {}\n", cat));
                                        }
                                        fm.push_str("---\n\n");
                                        fm.push_str(skill.content.trim());
                                        fm
                                    } else {
                                        skill.content.clone()
                                    };

                                    let res = if let Some(ref prof) = profile_name {
                                        crate::agent::skills::save_subagent_skill(
                                            prof,
                                            &skill.name,
                                            &final_content,
                                        )
                                    } else {
                                        crate::agent::skills::save_skill(
                                            &skill.name,
                                            &final_content,
                                        )
                                    };
                                    if let Err(e) = res {
                                        let msg = format!(
                                            "Failed to save skill '{}': {}",
                                            skill.name, e
                                        );
                                        error_msg = Some(msg);
                                        crate::channels::cli::send_notification(&format!(
                                            "{}▲ [Self-Improvement] Failed to save self-improvement skill '{}': {}{}",
                                            AURA_GOLD, skill.name, e, COLOR_RESET
                                        ));
                                    } else {
                                        skills_saved.push(skill.name.clone());
                                        tracing::info!(session = %session_key, skill = %skill.name, "Self-improvement curator: saved skill.");
                                        crate::channels::cli::send_notification(&format!(
                                            "{}◇ [Self-Improvement] Skill '{}' synthesized & saved to ~/.openz/skills/.{}",
                                            AURA_BLUE, skill.name, COLOR_RESET
                                        ));
                                        crate::channels::websocket::publish_activity_notice(
                                            &session_key,
                                            "self_improvement",
                                            "Skill updated",
                                            skill.name.clone(),
                                        );
                                    }
                                }
                            }

                            for source in review.sources_to_save {
                                if !source.label.trim().is_empty()
                                    && !source.uri.trim().is_empty()
                                {
                                    match crate::tools::shared_memory::add_source_bookmark(
                                        &source.label,
                                        source.kind.as_deref().unwrap_or("other"),
                                        &source.uri,
                                        source.aliases,
                                        source.summary.as_deref().unwrap_or(""),
                                        source.trust_score.unwrap_or(0.6),
                                        source.stale_after_secs.unwrap_or(604800),
                                    )
                                    .await
                                    {
                                        Ok(saved) => {
                                            crate::channels::cli::send_notification(&format!(
                                                "{}◇ [Knowledge] Source saved: {}{}",
                                                AURA_BLUE, saved.label, COLOR_RESET
                                            ));
                                            crate::channels::websocket::publish_activity_notice(
                                                &session_key,
                                                "source",
                                                "Source saved",
                                                saved.label.clone(),
                                            );
                                        }
                                        Err(e) => {
                                            tracing::warn!(session = %session_key, source = %source.label, error = %e, "Self-improvement curator failed to save source bookmark");
                                        }
                                    }
                                }
                            }

                            for workflow in review.workflows_to_save {
                                if !workflow.name.trim().is_empty()
                                    && !workflow.summary.trim().is_empty()
                                {
                                    match crate::tools::shared_memory::add_workflow_card(
                                        &workflow.name,
                                        workflow.triggers,
                                        &workflow.summary,
                                        workflow.steps,
                                        workflow.preconditions,
                                        workflow.verification,
                                        workflow.risk.as_deref().unwrap_or("normal"),
                                        workflow.status.as_deref().unwrap_or("draft"),
                                    )
                                    .await
                                    {
                                        Ok(saved) => {
                                            crate::channels::cli::send_notification(&format!(
                                                "{}◇ [Workflow] Workflow saved: {}{}",
                                                AURA_BLUE, saved.name, COLOR_RESET
                                            ));
                                            crate::channels::websocket::publish_activity_notice(
                                                &session_key,
                                                "workflow",
                                                "Workflow saved",
                                                saved.name.clone(),
                                            );
                                        }
                                        Err(e) => {
                                            tracing::warn!(session = %session_key, workflow = %workflow.name, error = %e, "Self-improvement curator failed to save workflow card");
                                        }
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
                        } else {
                            let msg = "JSON deserialization and fallback parsing both failed".to_string();
                            write_log("failed", false, vec![], Some(msg));
                        }
                    } else {
                        write_log(
                            "failed",
                            false,
                            vec![],
                            Some("Empty content returned from LLM".to_string()),
                        );
                    }
                }
                Err(e) => {
                    let msg = format!("LLM chat query failed: {}", e);
                    write_log("failed", false, vec![], Some(msg));
                }
            }
        });
        set_last_curator_handle(handle);
    }

    Ok(TurnState::Done)
}

#[cfg(test)]
#[path = "save_tests.rs"]
mod tests;
