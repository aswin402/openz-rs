use super::{get_session_lock, AgentLoop, TurnContext, TurnState};
use crate::agent::style::*;
use anyhow::Result;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

static CURATOR_LAST_SPAWN: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();

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

pub async fn handle(loop_ref: &AgentLoop, ctx: &mut TurnContext<'_>) -> Result<TurnState> {
    let config = &ctx.config;
    loop_ref.session_manager.save(&ctx.session).await?;
    if let Err(e) = crate::tools::onpkg::sync_onpkg_manifest() {
        tracing::warn!("Failed to synchronize onpkg manifest: {}", e);
    }
    tracing::info!(session = %ctx.session_key, "Session saved successfully. Turn complete.");

    let traces_dir = crate::config::resolve_path("~/.openz/traces");
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
                    if t.contains("write_file")
                        || t.contains("patch_file")
                        || t.contains("replace_lines")
                        || t.contains("zenflow_edit")
                        || t.contains("db_write")
                        || t.contains("cargo")
                        || t.contains("exec_command")
                        || t.contains("web_fetch")
                        || t.contains("web_search")
                        || t.contains("crawl")
                        || t.contains("obscura")
                        || t.contains("gsd_browser")
                        || t.contains("remote_input")
                        || t.contains("mcp")
                    {
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

            let _ = crate::agent::skills::archive_stale_skills();
            let _ = crate::tools::shared_memory::consolidate_shared_memory(&provider).await;

            #[derive(Deserialize)]
            struct ReviewSkill {
                name: String,
                content: String,
            }

            #[derive(Deserialize)]
            struct ReviewSource {
                label: String,
                kind: Option<String>,
                uri: String,
                #[serde(default)]
                aliases: Vec<String>,
                summary: Option<String>,
                trust_score: Option<f64>,
                stale_after_secs: Option<i64>,
            }

            #[derive(Deserialize)]
            struct ReviewWorkflow {
                name: String,
                #[serde(default)]
                triggers: Vec<String>,
                summary: String,
                #[serde(default)]
                steps: serde_json::Value,
                #[serde(default)]
                preconditions: Vec<String>,
                #[serde(default)]
                verification: Vec<String>,
                risk: Option<String>,
                status: Option<String>,
            }

            #[derive(Deserialize)]
            struct ReviewResponse {
                memory_updated: bool,
                memory_content: String,
                #[serde(default)]
                skills_to_save: Vec<ReviewSkill>,
                #[serde(default)]
                sources_to_save: Vec<ReviewSource>,
                #[serde(default)]
                workflows_to_save: Vec<ReviewWorkflow>,
            }

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

                        match serde_json::from_str::<ReviewResponse>(&clean_json) {
                            Ok(review) => {
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
                                                    let _ = crate::tools::graph_memory::with_db(
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
                                        let res = if let Some(ref prof) = profile_name {
                                            crate::agent::skills::save_subagent_skill(
                                                prof,
                                                &skill.name,
                                                &skill.content,
                                            )
                                        } else {
                                            crate::agent::skills::save_skill(
                                                &skill.name,
                                                &skill.content,
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
                                                "{}◇ [Self-Improvement] Skill '{}' updated/created based on recent conversation.{}",
                                                AURA_BLUE, skill.name, COLOR_RESET
                                            ));
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
                            }
                            Err(e) => {
                                let msg = format!("JSON deserialization failed: {}", e);
                                write_log("failed", false, vec![], Some(msg));
                            }
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
    }

    Ok(TurnState::Done)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curator_spawn_debounces_fast_repeated_session() {
        let key = format!(
            "test-curator-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        assert!(should_spawn_curator(&key, Duration::from_secs(20)));
        assert!(!should_spawn_curator(&key, Duration::from_secs(20)));
    }
}
