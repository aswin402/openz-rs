use super::{get_session_lock, AgentLoop, TurnContext, TurnState};
use crate::agent::style::*;
use crate::memory::with_graph_db;
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

static LAST_CURATOR_HANDLE: std::sync::OnceLock<
    tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
> = std::sync::OnceLock::new();

pub fn set_last_curator_handle(handle: tokio::task::JoinHandle<()>) {
    let mutex = LAST_CURATOR_HANDLE.get_or_init(|| tokio::sync::Mutex::new(None));
    let _ = mutex.try_lock().map(|mut guard| {
        *guard = Some(handle);
    });
}

pub async fn wait_for_curator(timeout_dur: Duration) -> bool {
    let mutex = LAST_CURATOR_HANDLE.get_or_init(|| tokio::sync::Mutex::new(None));
    let mut guard = mutex.lock().await;
    match guard.take() {
        Some(handle) => {
            let _ = tokio::time::timeout(timeout_dur, handle).await;
            true
        }
        None => false,
    }
}

fn should_spawn_curator(session_key: &str, debounce: Duration) -> bool {
    let now = Instant::now();
    let registry = CURATOR_LAST_SPAWN.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = match registry.lock() {
        Ok(guard) => guard,
        Err(_) => return true,
    };
    let debounced = guard
        .get(session_key)
        .is_some_and(|last| now.duration_since(*last) < debounce);
    match debounced {
        true => false,
        false => {
            guard.insert(session_key.to_string(), now);
            true
        }
    }
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

fn salvage_string_field(raw: &str, field_name: &str) -> Option<String> {
    let pos = raw.find(field_name)?;
    let after = &raw[pos + field_name.len()..];
    let colon_pos = after.find(':')?;
    let val_part = after[colon_pos + 1..].trim_start();
    let inner = val_part.strip_prefix('"')?;
    let end_quote = inner.find('"')?;
    let unescaped = inner[..end_quote].replace("\\n", "\n").replace("\\\"", "\"");
    (!unescaped.trim().is_empty()).then_some(unescaped)
}

fn salvage_json_array_objects<T, F>(raw: &str, field_name: &str, is_valid: F) -> Vec<T>
where
    T: serde::de::DeserializeOwned,
    F: Fn(&T) -> bool,
{
    let field_pos = match raw.find(field_name) {
        Some(pos) => pos,
        None => return Vec::new(),
    };
    let after = &raw[field_pos + field_name.len()..];
    let bracket_start = match after.find('[') {
        Some(pos) => pos,
        None => return Vec::new(),
    };
    let inside_array = &after[bracket_start + 1..];
    let mut depth = 0usize;
    let mut obj_start: Option<usize> = None;
    let mut items = Vec::new();

    for (idx, ch) in inside_array.char_indices() {
        match (ch, depth) {
            ('{', 0) => {
                obj_start = Some(idx);
                depth = 1;
            }
            ('{', _) => {
                depth += 1;
            }
            ('}', 1) => {
                depth = 0;
                let _ = obj_start
                    .take()
                    .and_then(|start| serde_json::from_str::<T>(&inside_array[start..=idx]).ok())
                    .filter(&is_valid)
                    .map(|item| items.push(item));
            }
            ('}', d) => {
                depth = d.saturating_sub(1);
            }
            (']', 0) => break,
            _ => {}
        }
    }
    items
}

pub(crate) fn fallback_parse_curator(raw: &str) -> Option<ReviewResponse> {
    let mut resp = ReviewResponse::default();
    let mut any_salvaged = false;

    let _ = salvage_string_field(raw, "\"user_preferences\"").map(|prefs| {
        resp.user_preferences = prefs;
        any_salvaged = true;
    });

    let _ = salvage_string_field(raw, "\"memory_content\"").map(|mem| {
        resp.memory_content = mem;
        resp.memory_updated = true;
        any_salvaged = true;
    });

    resp.skills_to_save =
        salvage_json_array_objects::<ReviewSkill, _>(raw, "\"skills_to_save\"", |skill| {
            !skill.name.is_empty() && !skill.content.is_empty()
        });
    any_salvaged = any_salvaged || !resp.skills_to_save.is_empty();

    resp.sources_to_save =
        salvage_json_array_objects::<ReviewSource, _>(raw, "\"sources_to_save\"", |source| {
            !source.label.trim().is_empty() && !source.uri.trim().is_empty()
        });
    any_salvaged = any_salvaged || !resp.sources_to_save.is_empty();

    resp.workflows_to_save =
        salvage_json_array_objects::<ReviewWorkflow, _>(raw, "\"workflows_to_save\"", |wf| {
            !wf.name.trim().is_empty() && !wf.summary.trim().is_empty()
        });
    any_salvaged = any_salvaged || !resp.workflows_to_save.is_empty();

    any_salvaged.then_some(resp)
}

const CURATOR_TRIGGER_TOOLS: &[&str] = &[
    "write_file",
    "patch_file",
    "replace_lines",
    "zenflow_edit",
    "db_write",
    "cargo",
    "exec_command",
    "obscura",
    "gsd_browser",
    "remote_input",
    "mcp",
    "curate_skill",
    "store_memory",
    "knowledge_source",
    "workflow_memory",
    "graph_memory",
];

const CURATOR_USER_INTENT_KEYWORDS: &[&str] = &[
    "remember",
    "learn",
    "preference",
    "guideline",
    "save this",
    "skill",
];

fn should_run_curator_for_turn(
    messages: &[crate::session::Message],
    tools_used: &[String],
) -> bool {
    let total_chars: usize = messages.iter().map(|m| m.content.len()).sum();
    let estimated_tokens = total_chars / 4;
    let complex_turn = estimated_tokens >= 4000 && tools_used.len() >= 3;
    let used_state_tool = tools_used.iter().any(|tool| {
        let t = tool.to_lowercase();
        CURATOR_TRIGGER_TOOLS.iter().any(|needle| t.contains(needle))
    });
    let explicit_user_intent = messages
        .iter()
        .rev()
        .take(3)
        .filter(|msg| msg.role == "user")
        .any(|msg| {
            let text = msg.content.to_lowercase();
            CURATOR_USER_INTENT_KEYWORDS
                .iter()
                .any(|kw| text.contains(kw))
        });

    complex_turn || used_state_tool || explicit_user_intent
}

fn extract_clean_curator_json(content: &str) -> String {
    let trimmed = content.trim();
    trimmed
        .find("```json")
        .map(|start_idx| {
            let after_start = &trimmed[start_idx + 7..];
            after_start
                .rfind("```")
                .map(|end_idx| after_start[..end_idx].trim().to_string())
                .unwrap_or_else(|| after_start.trim().to_string())
        })
        .or_else(|| {
            trimmed.find("```").map(|start_idx| {
                let after_start = &trimmed[start_idx + 3..];
                after_start
                    .rfind("```")
                    .map(|end_idx| after_start[..end_idx].trim().to_string())
                    .unwrap_or_else(|| after_start.trim().to_string())
            })
        })
        .or_else(|| {
            trimmed
                .find('{')
                .zip(trimmed.rfind('}'))
                .filter(|(first, last)| first < last)
                .map(|(first, last)| trimmed[first..=last].to_string())
        })
        .unwrap_or_else(|| trimmed.to_string())
}

fn format_curator_skill_content(skill: &ReviewSkill) -> String {
    match skill.content.trim_start().starts_with("---") {
        true => skill.content.clone(),
        false => {
            let mut fm = String::from("---\n");
            fm.push_str(&format!("name: {}\n", skill.name));
            let desc_line = skill
                .description
                .as_ref()
                .map(|desc| format!("description: {}\n", desc.replace('\n', " ")))
                .unwrap_or_else(|| format!("description: Procedural skill for {}\n", skill.name));
            fm.push_str(&desc_line);
            let _ = (!skill.triggers.is_empty()).then(|| {
                fm.push_str("triggers:\n");
                for t in &skill.triggers {
                    fm.push_str(&format!("  - \"{}\"\n", t.replace('"', "\\\"")));
                }
            });
            let _ = skill.vault_category.as_ref().map(|cat| {
                fm.push_str(&format!("vault_category: {}\n", cat));
            });
            fm.push_str("---\n\n");
            fm.push_str(skill.content.trim());
            fm
        }
    }
}

fn truncate_curator_log_text(text: &str, max_len: usize, with_dash: bool) -> String {
    match text.len() > max_len {
        true => {
            let truncated: String = text.chars().take(max_len).collect();
            let sep = with_dash.then_some(" - ").unwrap_or(" ");
            format!(
                "{}... [TRUNCATED{}{} bytes]",
                truncated,
                sep,
                text.len() - max_len
            )
        }
        false => text.to_string(),
    }
}

pub async fn handle(loop_ref: &AgentLoop, ctx: &mut TurnContext<'_>) -> Result<TurnState> {
    let config = &ctx.config;
    loop_ref.session_manager.save(&ctx.session).await?;
    let _ = crate::tools::onpkg::sync_onpkg_manifest().map_err(|e| {
        tracing::warn!("Failed to synchronize onpkg manifest: {}", e);
    });
    tracing::info!(session = %ctx.session_key, "Session saved successfully. Turn complete.");

    let traces_dir = crate::config::traces_dir();
    match std::fs::create_dir_all(&traces_dir) {
        Err(e) => {
            tracing::error!(
                "{}▲ Failed to create traces directory: {}{}",
                AURA_GOLD,
                e,
                COLOR_RESET
            );
        }
        Ok(()) => {
            let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
            let trace_file = traces_dir.join(format!(
                "trace_{}_{}.json",
                ctx.session_key.replace(':', "_"),
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
            let _ = serde_json::to_string_pretty(&trace_record)
                .ok()
                .map(|content| std::fs::write(trace_file, content));
        }
    }

    match ctx.user_content.starts_with('/') {
        true => Ok(TurnState::Done),
        false => match should_spawn_curator(ctx.session_key, Duration::from_secs(20)) {
            false => {
                tracing::debug!(
                    session = %ctx.session_key,
                    "Skipping self-improvement curator spawn during debounce window"
                );
                Ok(TurnState::Done)
            }
            true => {
                let session_manager = loop_ref.session_manager.clone();
                let session_key = ctx.session_key.to_string();
                let provider = ctx.active_provider.clone();
                let messages = ctx.messages.clone();
                let tools_used = ctx.tools_used.clone();
                let initial_updated_at = ctx.session.updated_at;
                let initial_msg_count = ctx.session.messages.len();

                let handle = tokio::spawn(async move {
                    let is_shutting_down =
                        crate::shutdown::receiver().is_some_and(|rx| *rx.borrow());
                    match is_shutting_down {
                        true => return,
                        false => {}
                    }
                    let parts_key: Vec<&str> = session_key.split(':').collect();
                    let profile_name = match parts_key.as_slice() {
                        ["subagent", prof, ..] => Some((*prof).to_string()),
                        _ => None,
                    };

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
                        let log_path =
                            crate::config::runtime_data_dir().join("curator_status.json");
                        let record = CuratorStatus {
                            last_run_timestamp: chrono::Utc::now().to_rfc3339(),
                            status: status.to_string(),
                            session_key: session_key.clone(),
                            memory_updated,
                            skills_saved,
                            error_message,
                        };
                        let _ = log_path.parent().map(std::fs::create_dir_all);
                        let _ = serde_json::to_string_pretty(&record)
                            .ok()
                            .map(|content| std::fs::write(log_path, content));
                    };

                    match should_run_curator_for_turn(&messages, &tools_used) {
                        false => {
                            write_log("skipped: throttled (simple turn)", false, vec![], None);
                            return;
                        }
                        true => {}
                    }

                    tracing::info!(session = %session_key, "Self-improvement curator: started processing.");
                    write_log("running", false, vec![], None);

                    let _ = crate::agent::skills::archive_stale_skills();
                    let _ = crate::tools::shared_memory::consolidate_shared_memory(&provider).await;

                    let recent_interactions =
                        crate::tools::shared_memory::get_recent_interactions(15)
                            .await
                            .unwrap_or_default();

                    let existing_memory = session_manager
                        .load_async(&session_key)
                        .await
                        .ok()
                        .and_then(|s| {
                            s.metadata
                                .get("memory")
                                .and_then(|v| v.as_str())
                                .map(ToString::to_string)
                        })
                        .unwrap_or_default();

                    let mut existing_skills_desc = String::new();
                    let _ =
                        crate::agent::skills::load_skills_with_profile(profile_name.as_deref())
                            .map(|skills| {
                                for skill in skills {
                                    existing_skills_desc.push_str(&format!(
                                        "Skill Name: {}\nContent:\n{}\n\n",
                                        skill.name, skill.content
                                    ));
                                }
                            });

                    let system_prompt_review = self_improvement_review_prompt();
                    let mut prompt_content = String::new();

                    let _ = (!recent_interactions.is_empty()).then(|| {
                        prompt_content.push_str("Recent user tasks across all sessions:\n");
                        for (i, item) in recent_interactions.iter().enumerate() {
                            let query = item["query"].as_str().unwrap_or("");
                            let success = item["success"].as_bool().unwrap_or(true);
                            let errors = item["errors"].as_str().unwrap_or("");
                            let status_label = success.then_some("SUCCESS").unwrap_or("FAILED");
                            prompt_content.push_str(&format!(
                                "{}. Task: \"{}\" | Status: {}\n",
                                i + 1,
                                query,
                                status_label
                            ));
                            let _ = (!errors.is_empty()).then(|| {
                                prompt_content
                                    .push_str(&format!("   Errors encountered: {}\n", errors));
                            });
                        }
                        prompt_content.push('\n');
                    });

                    let tool_count = messages.iter().filter(|m| m.role == "tool").count();
                    let _ = (tool_count >= 5).then(|| {
                        prompt_content.push_str(&format!(
                            "[SYSTEM NOTICE: The recent task was complex and involved {} tool executions. Review the successful trajectory and extract a reusable skill so the agent can perform this category of work efficiently next time.]\n\n",
                            tool_count
                        ));
                    });

                    let created_deliverable = messages.iter().any(|m| {
                        m.extra
                            .get("tool_calls")
                            .and_then(|v| v.as_array())
                            .is_some_and(|tool_calls| {
                                tool_calls.iter().any(|tc| {
                                    let name = tc
                                        .get("name")
                                        .and_then(|v| v.as_str())
                                        .or_else(|| {
                                            tc.get("function")
                                                .and_then(|f| f.get("name"))
                                                .and_then(|v| v.as_str())
                                        })
                                        .unwrap_or("");
                                    let args = tc
                                        .get("arguments")
                                        .or_else(|| {
                                            tc.get("function").and_then(|f| f.get("arguments"))
                                        })
                                        .map(|v| v.to_string())
                                        .unwrap_or_default();
                                    matches!(
                                        name,
                                        "write_file"
                                            | "patch_file"
                                            | "replace_lines"
                                            | "generate_image"
                                    ) && [
                                        "openz_vault",
                                        "index.html",
                                        ".css",
                                        ".js",
                                        ".py",
                                        ".sh",
                                        ".rs",
                                    ]
                                    .iter()
                                    .any(|ext| args.contains(ext))
                                })
                            })
                    });

                    let _ = created_deliverable.then(|| {
                        prompt_content.push_str("[DELIVERABLE DETECTED: The agent produced project files or creative deliverables in this turn. You MUST synthesize a reusable procedural skill in `skills_to_save` documenting the architecture, design choices, colors/styling, tech stack, and step-by-step procedure used to build this deliverable so subsequent requests can replicate or build upon it.]\n\n");
                    });

                    let existing_user_profile = crate::agent::skills::load_user_profile();
                    let _ = (!existing_user_profile.is_empty()).then(|| {
                        prompt_content.push_str(&format!(
                            "Existing User Profile & Preferences (USER.md):\n{}\n\n",
                            existing_user_profile
                        ));
                    });

                    let _ = (!existing_memory.is_empty()).then(|| {
                        prompt_content
                            .push_str(&format!("Existing Memory:\n{}\n\n", existing_memory));
                    });
                    let _ = (!existing_skills_desc.is_empty()).then(|| {
                        prompt_content
                            .push_str(&format!("Existing Skills:\n{}\n\n", existing_skills_desc));
                    });
                    prompt_content.push_str("Recent conversation history to review:\n");
                    for msg in &messages {
                        match msg.role.as_str() {
                            "user" => {
                                prompt_content.push_str(&format!("[user]: {}\n", msg.content));
                            }
                            "assistant" => {
                                prompt_content.push_str("[assistant]:\n");
                                let _ = msg
                                    .extra
                                    .get("reasoning_content")
                                    .and_then(|v| v.as_str())
                                    .filter(|r| !r.is_empty())
                                    .map(|reasoning| {
                                        prompt_content
                                            .push_str(&format!("  Thinking:\n{}\n", reasoning));
                                    });
                                let _ = msg
                                    .extra
                                    .get("tool_calls")
                                    .and_then(|v| v.as_array())
                                    .filter(|tc| !tc.is_empty())
                                    .map(|tool_calls| {
                                        prompt_content.push_str("  Tool Calls:\n");
                                        for tc in tool_calls {
                                            let name =
                                                tc.get("name").and_then(|v| v.as_str()).or_else(
                                                    || {
                                                        tc.get("function")
                                                            .and_then(|f| f.get("name"))
                                                            .and_then(|v| v.as_str())
                                                    },
                                                );
                                            let args = tc.get("arguments").or_else(|| {
                                                tc.get("function").and_then(|f| f.get("arguments"))
                                            });
                                            let _ = name.zip(args).map(|(name_str, args_val)| {
                                                let args_truncated = truncate_curator_log_text(
                                                    &args_val.to_string(),
                                                    1000,
                                                    true,
                                                );
                                                prompt_content.push_str(&format!(
                                                    "    - Call tool '{}' with arguments: {}\n",
                                                    name_str, args_truncated
                                                ));
                                            });
                                        }
                                    });
                                let _ = (!msg.content.is_empty()).then(|| {
                                    let content_truncated =
                                        truncate_curator_log_text(&msg.content, 2000, true);
                                    prompt_content
                                        .push_str(&format!("  Response: {}\n", content_truncated));
                                });
                            }
                            "tool" => {
                                let tool_name = msg
                                    .extra
                                    .get("name")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("unknown");
                                let content_truncated =
                                    truncate_curator_log_text(&msg.content, 2000, false);
                                prompt_content.push_str(&format!(
                                    "[tool output for '{}']:\n{}\n",
                                    tool_name, content_truncated
                                ));
                            }
                            role => {
                                prompt_content
                                    .push_str(&format!("[{}]: {}\n", role, msg.content));
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
                        Ok(resp) => match resp.content {
                            None => {
                                write_log(
                                    "failed",
                                    false,
                                    vec![],
                                    Some("Empty content returned from LLM".to_string()),
                                );
                            }
                            Some(content) => {
                                let clean_json = extract_clean_curator_json(&content);
                                let (review_opt, was_salvaged) =
                                    match serde_json::from_str::<ReviewResponse>(&clean_json) {
                                        Ok(review) => (Some(review), false),
                                        Err(e) => {
                                            tracing::warn!(session = %session_key, error = %e, "JSON deserialization failed, attempting fallback parsing");
                                            (fallback_parse_curator(&clean_json), true)
                                        }
                                    };

                                match review_opt {
                                    None => {
                                        let msg =
                                            "JSON deserialization and fallback parsing both failed"
                                                .to_string();
                                        write_log("failed", false, vec![], Some(msg));
                                    }
                                    Some(review) => {
                                        let _ = was_salvaged.then(|| {
                                            tracing::info!(session = %session_key, "Successfully salvaged curator learnings via fallback parser");
                                        });

                                        let _ = (!review.user_preferences.trim().is_empty()).then(
                                            || match crate::agent::skills::update_user_profile(
                                                &review.user_preferences,
                                            ) {
                                                Err(e) => {
                                                    tracing::warn!(session = %session_key, error = %e, "Failed to update USER.md preferences");
                                                }
                                                Ok(()) => {
                                                    tracing::info!(session = %session_key, "Self-improvement curator: updated USER.md preferences.");
                                                    crate::channels::cli::send_notification(
                                                        &format!(
                                                            "{}◇ [Self-Improvement] User profile preferences updated (~/.openz/USER.md).{}",
                                                            AURA_BLUE, COLOR_RESET
                                                        ),
                                                    );
                                                }
                                            },
                                        );

                                        match review.memory_updated {
                                            false => {}
                                            true => {
                                                let lock = get_session_lock(&session_key);
                                                let _guard = lock.lock().await;
                                                match session_manager
                                                    .load_async(&session_key)
                                                    .await
                                                {
                                                    Err(_) => {}
                                                    Ok(mut latest_session) => {
                                                        let session_changed =
                                                            latest_session.updated_at
                                                                != initial_updated_at
                                                                || latest_session.messages.len()
                                                                    != initial_msg_count;
                                                        match session_changed {
                                                            true => {
                                                                let msg = "Skipped memory update: session was modified concurrently".to_string();
                                                                error_msg = Some(msg);
                                                                tracing::debug!(
                                                                    session = %session_key,
                                                                    "Self-improvement curator skipped stale write because session changed concurrently"
                                                                );
                                                            }
                                                            false => {
                                                                latest_session.metadata.insert(
                                                                    "memory".to_string(),
                                                                    serde_json::Value::String(
                                                                        review
                                                                            .memory_content
                                                                            .trim()
                                                                            .to_string(),
                                                                    ),
                                                                );
                                                                match session_manager
                                                                    .save(&latest_session)
                                                                    .await
                                                                {
                                                                    Err(e) => {
                                                                        let msg = format!(
                                                                            "Failed to save memory: {}",
                                                                            e
                                                                        );
                                                                        error_msg = Some(msg);
                                                                        crate::channels::cli::send_notification(&format!(
                                                                            "{}▲ [Self-Improvement] Failed to save self-improvement memory: {}{}",
                                                                            AURA_GOLD, e, COLOR_RESET
                                                                        ));
                                                                    }
                                                                    Ok(()) => {
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
                                                                                    let _ = (!exists).then(|| {
                                                                                        let _ = conn.execute(
                                                                                            "INSERT INTO semantic_metadata (node_id, raw_text, timestamp, importance, user_id, session_id, agent_id)
                                                                                             VALUES (?1, ?2, ?3, 0.8, ?4, ?5, ?6)",
                                                                                            rusqlite::params![node_id, fact, timestamp, uid, sid, aid],
                                                                                        );
                                                                                        let _ = conn.execute(
                                                                                            "INSERT INTO semantic_fts (node_id, raw_text) VALUES (?1, ?2)",
                                                                                            rusqlite::params![node_id, fact],
                                                                                        );
                                                                                    });
                                                                                    Ok(())
                                                                                },
                                                                            );
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }

                                        for skill in review.skills_to_save.into_iter().filter(|s| {
                                            !s.name.is_empty() && !s.content.is_empty()
                                        }) {
                                            let final_content =
                                                format_curator_skill_content(&skill);
                                            let res = match profile_name.as_deref() {
                                                Some(prof) => {
                                                    crate::agent::skills::save_subagent_skill(
                                                        prof,
                                                        &skill.name,
                                                        &final_content,
                                                    )
                                                }
                                                None => crate::agent::skills::save_skill(
                                                    &skill.name,
                                                    &final_content,
                                                ),
                                            };
                                            match res {
                                                Err(e) => {
                                                    let msg = format!(
                                                        "Failed to save skill '{}': {}",
                                                        skill.name, e
                                                    );
                                                    error_msg = Some(msg);
                                                    crate::channels::cli::send_notification(
                                                        &format!(
                                                            "{}▲ [Self-Improvement] Failed to save self-improvement skill '{}': {}{}",
                                                            AURA_GOLD, skill.name, e, COLOR_RESET
                                                        ),
                                                    );
                                                }
                                                Ok(()) => {
                                                    skills_saved.push(skill.name.clone());
                                                    tracing::info!(session = %session_key, skill = %skill.name, "Self-improvement curator: saved skill.");
                                                    crate::channels::cli::send_notification(
                                                        &format!(
                                                            "{}◇ [Self-Improvement] Skill '{}' synthesized & saved to ~/.openz/skills/.{}",
                                                            AURA_BLUE, skill.name, COLOR_RESET
                                                        ),
                                                    );
                                                    crate::channels::websocket::publish_activity_notice(
                                                        &session_key,
                                                        "self_improvement",
                                                        "Skill updated",
                                                        skill.name.clone(),
                                                    );
                                                }
                                            }
                                        }

                                        for source in review.sources_to_save.into_iter().filter(
                                            |s| {
                                                !s.label.trim().is_empty()
                                                    && !s.uri.trim().is_empty()
                                            },
                                        ) {
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
                                                    crate::channels::cli::send_notification(
                                                        &format!(
                                                            "{}◇ [Knowledge] Source saved: {}{}",
                                                            AURA_BLUE, saved.label, COLOR_RESET
                                                        ),
                                                    );
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

                                        for workflow in review.workflows_to_save.into_iter().filter(
                                            |w| {
                                                !w.name.trim().is_empty()
                                                    && !w.summary.trim().is_empty()
                                            },
                                        ) {
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
                                                    crate::channels::cli::send_notification(
                                                        &format!(
                                                            "{}◇ [Workflow] Workflow saved: {}{}",
                                                            AURA_BLUE, saved.name, COLOR_RESET
                                                        ),
                                                    );
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

                                        match error_msg.is_none() {
                                            true => {
                                                tracing::info!(session = %session_key, "Self-improvement curator finished successfully.");
                                                write_log(
                                                    "success",
                                                    memory_updated,
                                                    skills_saved,
                                                    None,
                                                );
                                            }
                                            false => {
                                                tracing::warn!(session = %session_key, error = ?error_msg, "Self-improvement curator finished with errors.");
                                                write_log(
                                                    "failed",
                                                    memory_updated,
                                                    skills_saved,
                                                    error_msg,
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        },
                        Err(e) => {
                            let msg = format!("LLM chat query failed: {}", e);
                            write_log("failed", false, vec![], Some(msg));
                        }
                    }
                });
                set_last_curator_handle(handle);
                Ok(TurnState::Done)
            }
        },
    }
}

#[cfg(test)]
#[path = "save_tests.rs"]
mod tests;
