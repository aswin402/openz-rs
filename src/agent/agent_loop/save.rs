use super::{get_session_lock, AgentLoop, TurnContext, TurnState};
use crate::agent::style::*;
use crate::memory::with_graph_db;
use anyhow::Result;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

#[cfg_attr(not(test), allow(dead_code))]
static CURATOR_LAST_SPAWN: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();

#[derive(Clone)]
struct CuratorJobPayload {
    session_manager: crate::session::SessionManager,
    session_key: String,
    user_query: String,
    provider: Arc<dyn crate::providers::LLMProvider>,
    messages: Vec<crate::session::Message>,
    tools_used: Vec<String>,
    turn_succeeded_cleanly: bool,
    turn_recovered: bool,
}

#[derive(Default)]
struct CuratorQueueState {
    in_flight: HashSet<String>,
    pending: HashMap<String, CuratorJobPayload>,
}

static CURATOR_QUEUE: OnceLock<Mutex<CuratorQueueState>> = OnceLock::new();

fn claim_or_coalesce_curator_job(job: CuratorJobPayload) -> Option<CuratorJobPayload> {
    let queue = CURATOR_QUEUE.get_or_init(|| Mutex::new(CuratorQueueState::default()));
    let mut guard = match queue.lock() {
        Ok(g) => g,
        Err(_) => return Some(job),
    };
    match guard.in_flight.contains(&job.session_key) {
        true => {
            guard.pending.insert(job.session_key.clone(), job);
            None
        }
        false => {
            guard.in_flight.insert(job.session_key.clone());
            Some(job)
        }
    }
}

fn finish_or_pop_coalesced_curator_job(session_key: &str) -> Option<CuratorJobPayload> {
    let queue = CURATOR_QUEUE.get_or_init(|| Mutex::new(CuratorQueueState::default()));
    let mut guard = match queue.lock() {
        Ok(g) => g,
        Err(_) => return None,
    };
    match guard.pending.remove(session_key) {
        Some(next_job) => Some(next_job),
        None => {
            guard.in_flight.remove(session_key);
            None
        }
    }
}

fn self_improvement_review_prompt() -> &'static str {
    "You are a specialized Self-Improvement Curator for OpenZ. Your job is to review the conversation between the User and the AI Agent and consolidate five types of learnings:\n\n\
1. USER PREFERENCES (USER.md): Durable user preferences, aesthetic styling choices, favorite design themes, color palettes, fonts, preferred programming languages, frameworks, or persona constraints. Use '- replace: <old> => <new>' when updating an outdated preference or '- remove: <stale>' when a preference is retracted.\n\
2. DELIVERABLE & PROCEDURAL SKILL BUNDLES (<name>/SKILL.md): Whenever the agent creates, modifies, or inspects deliverables (websites, scripts, scrapers, data pipelines, media animations in openz_vault/ or project folders) or solves complex multi-step technical challenges:\n\
   - Extract a clean, reusable procedural skill package.\n\
   - Include standard YAML frontmatter with:\n\
     ---\n\
     name: <name_lowercase_with_underscores>\n\
     description: <what this skill accomplishes>\n\
     triggers: [\"<intent keyword 1>\", \"<intent keyword 2>\"]\n\
     vault_category: <websites|scripts|images|videos|documents|presentations|data|diagrams|templates|exports>\n\
     ---\n\
   - Under the frontmatter, provide Markdown instructions with Design System / Tech Rules, File Locations (e.g. openz_vault/websites/...), and Step-by-Step Reproduction/Extension Steps.\n\
   - Optionally include reusable helper scripts, templates, or deep reference docs in `supporting_files` (`scripts/...`, `templates/...`, `references/...`).\n\
3. GENERAL GUIDELINE SKILLS & ACTIVE SKILL SELF-HEALING: Task-specific coding workarounds, bug fixes, or library quirks (e.g. 'cargo_build_fix', 'sqlite_lock_workaround'). When an existing skill had an outdated flag/step, use operation='patch' to heal it.\n\
4. SOURCES: High-trust reusable URLs or documentation links.\n\
5. WORKFLOWS: Reusable multi-step procedures with triggers, steps, preconditions, verification, risk, and status.\n\n\
CRITICAL: Pay special attention to tool execution outcomes. If a tool call failed with an error, look at how the agent resolved it (or what workaround succeeded). Extract this learning into a reusable skill or workflow.\n\n\
Specific workflow patterns to preserve when observed:\n\
- Large static website or generated source creation: split writes into safe chunks instead of one huge tool payload; verify the file exists; open only after successful verification.\n\
- HTML/video generation longer than roughly 20 seconds: render in shorter segments, concatenate with ffmpeg, verify duration, then open.\n\
- Dev servers: when exec_command returns server_registered=true, record the server id and use manage_servers to stop it automatically when the preview/verification task is finished.\n\
- Feature/tool inventory answers: use openz_inventory/tool_catalog instead of answering exact counts from memory.\n\n\
REASONING BUDGET: Keep internal thinking under 120 words and emit the raw JSON object immediately. When the agent already called curate_skill during this turn to add or patch a skill, leave skills_to_save empty [] so you never overwrite in-turn edits.\n\n\
You MUST return your response as a raw JSON object with the following structure. Do not wrap it in conversational text. Return empty arrays when there is nothing to save.\n\n\
JSON Format:\n\
{\n\
   \"memory_updated\": true/false,\n\
   \"memory_content\": \"<bullet points of durable cross-session environment/project facts for MEMORY.md (supports '- replace: old => new' and '- remove: stale'); empty when memory_updated is false>\",\n\
   \"user_preferences\": \"<bullet points of durable user preferences, design choices, styling tastes, or conventions to record in USER.md>\",\n\
   \"skills_to_save\": [\n\
     {\n\
       \"operation\": \"create|patch\",\n\
       \"name\": \"<name of skill, lowercase with underscores — reuse an existing skill name with operation='patch' whenever refining an existing skill rather than creating duplicates>\",\n\
       \"description\": \"<one-sentence summary of when to use this skill>\",\n\
       \"triggers\": [\"<keyword or user intent phrase>\"],\n\
       \"vault_category\": \"websites|scripts|images|videos|documents|presentations|data|diagrams|templates|exports\",\n\
       \"old_text\": \"<optional exact snippet from existing skill when operation='patch'>\",\n\
       \"new_text\": \"<optional replacement or appended section when operation='patch'>\",\n\
       \"content\": \"<complete markdown content for operation='create' including YAML frontmatter with name, description, triggers, vault_category, followed by headers, rules, and steps.>\",\n\
       \"supporting_files\": [{\"path\": \"scripts/helper.py\", \"content\": \"<optional reusable script, template, or reference file>\"}]\n\
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

fn memory_only_review_prompt() -> &'static str {
    "You are OpenZ's fast Memory & Preference Curator. Review the latest turn and extract ONLY durable cross-session facts or user preferences that are NOT already in Existing Memory or USER.md.\n\n\
Rules:\n\
- MEMORY.md (`memory_content`): Durable project/architecture facts, verified environment paths, or persistent operational truths. Supports '- replace: <old> => <new>' and '- remove: <stale>'. Leave `memory_updated: false` and `memory_content: \"\"` when nothing new and durable was learned.\n\
- USER.md (`user_preferences`): Explicit durable user tastes, coding conventions, persona rules, or workflow preferences. Supports '- replace: <old> => <new>' and '- remove: <stale>'. Leave `\"\"` when no user preference was stated.\n\
- Keep internal thinking under 60 words and output raw JSON immediately.\n\n\
JSON Format:\n\
{\n\
  \"memory_updated\": true/false,\n\
  \"memory_content\": \"- <concise durable fact bullet>\",\n\
  \"user_preferences\": \"- <concise durable preference bullet>\",\n\
  \"skills_to_save\": [],\n\
  \"sources_to_save\": [],\n\
  \"workflows_to_save\": []\n\
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

#[cfg_attr(not(test), allow(dead_code))]
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
pub(crate) struct ReviewSkillFile {
    #[serde(default)]
    pub(crate) path: String,
    #[serde(default)]
    pub(crate) content: String,
}

#[derive(Deserialize, Default, Clone, Debug)]
pub(crate) struct ReviewSkill {
    #[serde(default)]
    pub(crate) operation: Option<String>,
    #[serde(default)]
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) description: Option<String>,
    #[serde(default)]
    pub(crate) triggers: Vec<String>,
    #[serde(default)]
    pub(crate) vault_category: Option<String>,
    #[serde(default)]
    pub(crate) old_text: Option<String>,
    #[serde(default)]
    pub(crate) new_text: Option<String>,
    #[serde(default)]
    pub(crate) content: String,
    #[serde(default)]
    pub(crate) supporting_files: Vec<ReviewSkillFile>,
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

fn decode_json_string_escape(after_open_quote: &str) -> Option<String> {
    let mut decoded = String::new();
    let mut escaped = false;
    for ch in after_open_quote.chars() {
        match (escaped, ch) {
            (true, 'n') => {
                decoded.push('\n');
                escaped = false;
            }
            (true, 'r') => {
                decoded.push('\r');
                escaped = false;
            }
            (true, 't') => {
                decoded.push('\t');
                escaped = false;
            }
            (true, other) => {
                decoded.push(other);
                escaped = false;
            }
            (false, '\\') => {
                escaped = true;
            }
            (false, '"') => return Some(decoded),
            (false, other) => {
                decoded.push(other);
            }
        }
    }
    (!decoded.trim().is_empty()).then_some(decoded)
}

fn salvage_string_field(raw: &str, field_name: &str) -> Option<String> {
    let pos = raw.find(field_name)?;
    let after = &raw[pos + field_name.len()..];
    let colon_pos = after.find(':')?;
    let val_part = after[colon_pos + 1..].trim_start();
    let inner = val_part.strip_prefix('"')?;
    let unescaped = decode_json_string_escape(inner)?;
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
    let mut in_string = false;
    let mut escaped = false;
    let mut obj_start: Option<usize> = None;
    let mut items = Vec::new();

    for (idx, ch) in inside_array.char_indices() {
        match (in_string, escaped, ch, depth) {
            (true, true, _, _) => {
                escaped = false;
            }
            (true, false, '\\', _) => {
                escaped = true;
            }
            (true, false, '"', _) => {
                in_string = false;
            }
            (true, false, _, _) => {}
            (false, _, '"', _) => {
                in_string = true;
            }
            (false, _, '{', 0) => {
                obj_start = Some(idx);
                depth = 1;
            }
            (false, _, '{', _) => {
                depth += 1;
            }
            (false, _, '}', 1) => {
                depth = 0;
                let _ = obj_start
                    .take()
                    .and_then(|start| serde_json::from_str::<T>(&inside_array[start..=idx]).ok())
                    .filter(&is_valid)
                    .map(|item| items.push(item));
            }
            (false, _, '}', d) => {
                depth = d.saturating_sub(1);
            }
            (false, _, ']', 0) => break,
            _ => {}
        }
    }
    items
}

fn review_skill_has_payload(skill: &ReviewSkill) -> bool {
    !skill.name.trim().is_empty()
        && (!skill.content.trim().is_empty()
            || skill
                .new_text
                .as_deref()
                .is_some_and(|s| !s.trim().is_empty()))
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
        salvage_json_array_objects::<ReviewSkill, _>(raw, "\"skills_to_save\"", review_skill_has_payload);
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

const MEMORY_NUDGE_TURN_CADENCE: usize = 8;

const LEARNING_INTENT_STEMS: &[&str] = &[
    "remember",
    "prefer",
    "preference",
    "guideline",
    "skill",
    "convention",
    "playbook",
    "workflow",
];

const SKILL_SYNTHESIS_INTENT_STEMS: &[&str] = &[
    "skill",
    "playbook",
    "workflow",
    "runbook",
    "sop",
];

pub(crate) fn has_explicit_skill_intent(messages: &[crate::session::Message]) -> bool {
    messages
        .iter()
        .rev()
        .take(2)
        .filter(|msg| msg.role == "user")
        .any(|msg| {
            let stems = crate::agent::skills::ranker::tokenize_stemmed(&msg.content, false);
            let has_create_verb = stems.iter().any(|tok| {
                ["create", "save", "synthesize", "extract", "write", "build"]
                    .iter()
                    .any(|target| crate::agent::skills::ranker::stem_token(target) == *tok)
            });
            let has_skill_noun = stems.iter().any(|tok| {
                SKILL_SYNTHESIS_INTENT_STEMS
                    .iter()
                    .any(|target| crate::agent::skills::ranker::stem_token(target) == *tok)
            });
            has_create_verb && has_skill_noun
        })
}

pub(crate) fn has_explicit_learning_intent(messages: &[crate::session::Message]) -> bool {
    messages
        .iter()
        .rev()
        .take(3)
        .filter(|msg| msg.role == "user")
        .any(|msg| {
            let stems = crate::agent::skills::ranker::tokenize_stemmed(&msg.content, false);
            let stem_hit = stems.iter().any(|tok| {
                LEARNING_INTENT_STEMS
                    .iter()
                    .any(|target| crate::agent::skills::ranker::stem_token(target) == *tok)
            });
            let phrase_hit =
                crate::agent::skills::ranker::phrase_matches_tokens(&stems, "save this")
                    || crate::agent::skills::ranker::phrase_matches_tokens(&stems, "learn from");
            stem_hit || phrase_hit
        })
}

fn has_state_or_learning_tool_signal(tools_used: &[String]) -> bool {
    tools_used.iter().any(|tool| {
        let meta = crate::tools::ToolMetadata::infer(tool);
        meta.writes_disk
            || matches!(
                meta.domain,
                "shell" | "self_management" | "memory" | "cron" | "code"
            ) && crate::tools::metadata::is_state_changing_tool(tool)
            || matches!(meta.domain, "self_management" | "memory")
    })
}

pub(crate) fn should_run_curator_for_turn(
    messages: &[crate::session::Message],
    tools_used: &[String],
) -> bool {
    let total_chars: usize = messages.iter().map(|m| m.content.len()).sum();
    let estimated_tokens = total_chars / 4;
    let distinct_tools: HashSet<&str> = tools_used.iter().map(String::as_str).collect();
    let complex_turn =
        (estimated_tokens >= 3_500 && tools_used.len() >= 3) || distinct_tools.len() >= 4;
    let user_turns = messages.iter().filter(|m| m.role == "user").count();
    let periodic_memory_cadence =
        user_turns > 0 && user_turns.is_multiple_of(MEMORY_NUDGE_TURN_CADENCE);

    complex_turn
        || has_state_or_learning_tool_signal(tools_used)
        || has_explicit_learning_intent(messages)
        || trajectory_recovered_after_error(messages)
        || periodic_memory_cadence
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

fn tool_message_indicates_failure(content: &str) -> bool {
    match serde_json::from_str::<serde_json::Value>(content) {
        Ok(val) => {
            val.get("error").is_some()
                || val.get("status_code").is_some_and(|c| c != 0)
                || val.get("exit_code").is_some_and(|c| c != 0)
                || val.get("success").and_then(|v| v.as_bool()) == Some(false)
        }
        Err(_) => false,
    }
}

pub(crate) fn trajectory_recovered_after_error(messages: &[crate::session::Message]) -> bool {
    let last_user_idx = messages.iter().rposition(|m| m.role == "user").unwrap_or(0);
    let mut saw_failure = false;
    let mut recovered_after_failure = false;
    for msg in messages[last_user_idx..].iter().filter(|m| m.role == "tool") {
        let failed = tool_message_indicates_failure(&msg.content);
        match (failed, saw_failure) {
            (true, _) => {
                saw_failure = true;
                recovered_after_failure = false;
            }
            (false, true) => {
                recovered_after_failure = true;
            }
            (false, false) => {}
        }
    }
    saw_failure && recovered_after_failure
}

fn build_budgeted_existing_skills_context(
    user_query: &str,
    messages: &[crate::session::Message],
    profile_name: Option<&str>,
) -> String {
    let all_skills = crate::agent::skills::load_skills_with_profile(profile_name).unwrap_or_default();
    match all_skills.is_empty() {
        true => String::new(),
        false => {
            let mut out = String::from(
                "Existing Skills Catalog (reuse an existing skill name with operation='patch' instead of creating duplicate skills):\n",
            );
            for skill in &all_skills {
                let line = format!("- {}: {}\n", skill.name, skill.description());
                match out.chars().count() + line.chars().count() <= 3_000 {
                    true => out.push_str(&line),
                    false => break,
                }
            }

            let closest = crate::agent::skills::load_ranked_skill_matches_with_profile(
                user_query,
                messages,
                profile_name,
            )
            .unwrap_or_default();
            let _ = (!closest.is_empty()).then(|| {
                out.push_str("\nClosest Existing Skill Bodies (for exact `old_text` when patching):\n");
                for m in closest.iter().take(3) {
                    let snippet = truncate_curator_log_text(&m.skill.content, 1_800, false);
                    let block = format!("=== {} ===\n{}\n\n", m.skill.name, snippet);
                    match out.chars().count() + block.chars().count() <= 8_500 {
                        true => out.push_str(&block),
                        false => break,
                    }
                }
            });
            out
        }
    }
}

pub async fn handle(loop_ref: &AgentLoop, ctx: &mut TurnContext<'_>) -> Result<TurnState> {
    let config = &ctx.config;
    loop_ref.session_manager.save(&ctx.session).await?;
    let turn_recovered = trajectory_recovered_after_error(&ctx.messages);
    let turn_succeeded_cleanly = (!ctx.final_content.trim().is_empty())
        && (!ctx.final_content.starts_with("⚠️ Halted execution:"))
        && (ctx.turn_errors.is_empty() || turn_recovered);
    let active_prof = super::build::subagent_profile_name(ctx.session_key);
    for used_skill in &ctx.skill_notice_names {
        crate::agent::skills::record_skill_outcome(
            used_skill,
            active_prof,
            turn_succeeded_cleanly,
        );
    }
    let _ = (config.agents.defaults.caveman_mode && !ctx.final_content.trim().is_empty()).then(
        || {
            let level =
                opencaveman::CavemanLevel::from_config_str(&config.agents.defaults.caveman_level);
            let _ = opencaveman::record_turn_savings(
                &crate::config::runtime_data_dir(),
                level,
                &ctx.final_content,
            );
        },
    );
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
        false => {
            let initial_job = CuratorJobPayload {
                session_manager: loop_ref.session_manager.clone(),
                session_key: ctx.session_key.to_string(),
                user_query: ctx.user_content.to_string(),
                provider: ctx.active_provider.clone(),
                messages: ctx.messages.clone(),
                tools_used: ctx.tools_used.clone(),
                turn_succeeded_cleanly,
                turn_recovered,
            };
            match claim_or_coalesce_curator_job(initial_job) {
                None => {
                    tracing::debug!(
                        session = %ctx.session_key,
                        "Coalesced self-improvement curator snapshot behind active in-flight curator"
                    );
                    Ok(TurnState::Done)
                }
                Some(mut current_job) => {
                    let handle = tokio::spawn(async move {
                        loop {
                            run_single_curator_job(current_job.clone()).await;
                            match finish_or_pop_coalesced_curator_job(&current_job.session_key) {
                                Some(next_job) => {
                                    current_job = next_job;
                                }
                                None => break,
                            }
                        }
                    });
                    set_last_curator_handle(handle);
                    Ok(TurnState::Done)
                }
            }
        }
    }
}

fn archive_facts_to_semantic_graph(session_key: &str, facts: &[String]) {
    let uid = "*";
    let sid = session_key;
    let aid = "*";
    for fact in facts.iter().map(|s| s.trim()).filter(|s| !s.is_empty()) {
        let node_id = format!("fact_{}", crate::core::short_uuid());
        let timestamp = chrono::Utc::now().to_rfc3339();
        let _ = with_graph_db(|conn| {
            let mut check_stmt = conn
                .prepare("SELECT 1 FROM semantic_metadata WHERE raw_text = ?1 AND valid_until IS NULL")
                .map_err(|e| anyhow::anyhow!(e))?;
            let exists = check_stmt
                .exists(rusqlite::params![fact])
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
        });
    }
}

async fn run_single_curator_job(job: CuratorJobPayload) {
                    let CuratorJobPayload {
                        session_manager,
                        session_key,
                        user_query,
                        provider,
                        messages,
                        tools_used,
                        turn_succeeded_cleanly,
                        turn_recovered,
                    } = job;
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

                    let existing_memory = crate::agent::core_memory::load_global_memory();
                    let tool_count = messages.iter().filter(|m| m.role == "tool").count();
                    let explicit_skill_intent = has_explicit_skill_intent(&messages);
                    let agent_curated_skill_in_turn = tools_used.iter().any(|t| {
                        matches!(
                            t.as_str(),
                            "curate_skill" | "skill_manage" | "manage_skills"
                        )
                    });

                    let created_vault_deliverable = turn_succeeded_cleanly
                        && messages.iter().any(|m| {
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
                                                tc.get("function")
                                                    .and_then(|f| f.get("arguments"))
                                            })
                                            .map(|v| v.to_string())
                                            .unwrap_or_default();
                                        matches!(
                                            name,
                                            "write_file"
                                                | "patch_file"
                                                | "replace_lines"
                                                | "generate_image"
                                        ) && args.contains("openz_vault")
                                    })
                                })
                        });

                    let allow_skill_synthesis = explicit_skill_intent
                        || (!agent_curated_skill_in_turn
                            && turn_succeeded_cleanly
                            && (turn_recovered
                                || created_vault_deliverable
                                || tool_count >= 5));

                    let system_prompt_review = match allow_skill_synthesis {
                        true => self_improvement_review_prompt(),
                        false => memory_only_review_prompt(),
                    };
                    let mut prompt_content = String::new();

                    let recent_interactions = match allow_skill_synthesis {
                        true => crate::tools::shared_memory::get_recent_interactions(10)
                            .await
                            .unwrap_or_default(),
                        false => Vec::new(),
                    };

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

                    let _ = (allow_skill_synthesis && turn_succeeded_cleanly && turn_recovered).then(|| {
                        prompt_content.push_str(
                            "[VERIFIED ERROR RECOVERY DETECTED: An earlier tool call failed and the agent resolved it cleanly. Extract or patch a concise troubleshooting skill ONLY when the root cause and fix generalize beyond this single file.]\n\n",
                        );
                    });

                    let _ = (allow_skill_synthesis && turn_succeeded_cleanly && tool_count >= 5).then(|| {
                        prompt_content.push_str(&format!(
                            "[MULTI-STEP WORKFLOW COMPLETED: Turn succeeded across {} tool executions. Extract or patch a reusable procedural skill ONLY when the workflow is generalizable across future tasks; return empty `skills_to_save` for routine single-repo edits.]\n\n",
                            tool_count
                        ));
                    });

                    let _ = (allow_skill_synthesis && created_vault_deliverable).then(|| {
                        prompt_content.push_str(
                            "[VAULT DELIVERABLE COMPLETED: Synthesize or patch a reusable deliverable skill documenting the architecture, design tokens, and reproduction steps.]\n\n",
                        );
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

                    let existing_skills_desc = match allow_skill_synthesis {
                        true => build_budgeted_existing_skills_context(
                            &user_query,
                            &messages,
                            profile_name.as_deref(),
                        ),
                        false => String::new(),
                    };
                    let _ = (!existing_skills_desc.is_empty()).then(|| {
                        prompt_content
                            .push_str(&format!("Existing Skills:\n{}\n\n", existing_skills_desc));
                    });
                    prompt_content.push_str("Recent conversation history to review:\n");
                    let window_start = messages.len().saturating_sub(12);
                    for msg in &messages[window_start..] {
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
                                        let short_reasoning =
                                            truncate_curator_log_text(reasoning, 250, true);
                                        prompt_content
                                            .push_str(&format!("  Thinking:\n{}\n", short_reasoning));
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
                                                    600,
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
                                        truncate_curator_log_text(&msg.content, 1200, true);
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
                                    truncate_curator_log_text(&msg.content, 800, false);
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

                    let curator_max_tokens = match allow_skill_synthesis {
                        true => 4096,
                        false => 2048,
                    };
                    let settings = crate::providers::GenerationSettings {
                        temperature: 0.1,
                        max_tokens: curator_max_tokens,
                        reasoning_effort: Some("low".to_string()),
                    };

                    let mut skills_saved = Vec::new();
                    let mut memory_updated = false;
                    let mut error_msg = None;

                    match provider
                        .chat(system_prompt_review, &review_msgs, &[], &settings)
                        .await
                    {
                        Ok(resp) => {
                            let resolved_content = resp.content.or_else(|| {
                                resp.reasoning_content
                                    .filter(|r| r.contains('{') && r.contains('}'))
                            });
                            match resolved_content {
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
                                            || match crate::agent::core_memory::apply_user_profile_mutations(
                                                &review.user_preferences,
                                                &[],
                                            ) {
                                                Err(e) => {
                                                    tracing::warn!(session = %session_key, error = %e, "Failed to update USER.md preferences");
                                                }
                                                Ok(evicted) => {
                                                    archive_facts_to_semantic_graph(
                                                        &session_key,
                                                        &evicted,
                                                    );
                                                    tracing::info!(session = %session_key, "Self-improvement curator: updated USER.md preferences.");
                                                    crate::channels::send_session_notice(
                                                        &session_key,
                                                        &format!(
                                                            "{}◇ [Self-Improvement] User profile preferences updated (~/.openz/USER.md).{}",
                                                            AURA_BLUE, COLOR_RESET
                                                        ),
                                                    );
                                                }
                                            },
                                        );

                                        match review.memory_updated
                                            && !review.memory_content.trim().is_empty()
                                        {
                                            false => {}
                                            true => {
                                                match crate::agent::core_memory::update_global_memory(
                                                    &review.memory_content,
                                                ) {
                                                    Err(e) => {
                                                        let msg = format!(
                                                            "Failed to update MEMORY.md: {}",
                                                            e
                                                        );
                                                        error_msg = Some(msg);
                                                    }
                                                    Ok(evicted) => {
                                                        memory_updated = true;
                                                        archive_facts_to_semantic_graph(
                                                            &session_key,
                                                            &evicted,
                                                        );
                                                        let facts: Vec<String> = review
                                                            .memory_content
                                                            .lines()
                                                            .filter_map(
                                                                crate::agent::core_memory::clean_bullet_line,
                                                            )
                                                            .map(ToString::to_string)
                                                            .filter(|f| !f.is_empty())
                                                            .collect();
                                                        archive_facts_to_semantic_graph(
                                                            &session_key,
                                                            &facts,
                                                        );
                                                        tracing::info!(session = %session_key, "Self-improvement curator: updated global MEMORY.md.");
                                                        crate::channels::send_session_notice(
                                                            &session_key,
                                                            &format!(
                                                                "{}◇ [Self-Improvement] Global memory updated (~/.openz/MEMORY.md).{}",
                                                                AURA_BLUE, COLOR_RESET
                                                            ),
                                                        );
                                                        crate::channels::websocket::publish_activity_notice(
                                                            &session_key,
                                                            "self_improvement",
                                                            "Self-improvement memory updated",
                                                            "Recent conversation learnings were consolidated into MEMORY.md",
                                                        );
                                                    }
                                                }

                                                let lock = get_session_lock(&session_key);
                                                let _guard = lock.lock().await;
                                                match session_manager
                                                    .load_async(&session_key)
                                                    .await
                                                {
                                                    Err(_) => {}
                                                    Ok(mut latest_session) => {
                                                        latest_session.metadata.insert(
                                                            "memory".to_string(),
                                                            serde_json::Value::String(
                                                                review
                                                                    .memory_content
                                                                    .trim()
                                                                    .to_string(),
                                                            ),
                                                        );
                                                        let _ = session_manager
                                                            .save(&latest_session)
                                                            .await;
                                                    }
                                                }
                                            }
                                        }

                                        for skill in review
                                            .skills_to_save
                                            .into_iter()
                                            .filter(|_| allow_skill_synthesis)
                                            .filter(review_skill_has_payload)
                                        {
                                            let is_patch = skill
                                                .operation
                                                .as_deref()
                                                .is_some_and(|op| op.eq_ignore_ascii_case("patch"));
                                            let patch_payload = skill
                                                .new_text
                                                .as_deref()
                                                .filter(|s| !s.trim().is_empty())
                                                .unwrap_or(skill.content.as_str());
                                            let res = match is_patch && !patch_payload.trim().is_empty() {
                                                true => crate::agent::skills::patch_skill(
                                                    &skill.name,
                                                    skill.old_text.as_deref(),
                                                    patch_payload,
                                                    profile_name.as_deref(),
                                                )
                                                .map(|_| ())
                                                .or_else(|_| {
                                                    match !skill.content.trim().is_empty() {
                                                        false => Err(anyhow::anyhow!(
                                                            "Patch target missing and no fallback content supplied"
                                                        )),
                                                        true => {
                                                            let final_content =
                                                                format_curator_skill_content(&skill);
                                                            match profile_name.as_deref() {
                                                                Some(prof) => crate::agent::skills::save_subagent_skill(
                                                                    prof,
                                                                    &skill.name,
                                                                    &final_content,
                                                                ),
                                                                None => crate::agent::skills::save_skill(
                                                                    &skill.name,
                                                                    &final_content,
                                                                ),
                                                            }
                                                        }
                                                    }
                                                }),
                                                false => {
                                                    let final_content =
                                                        format_curator_skill_content(&skill);
                                                    match profile_name.as_deref() {
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
                                                    }
                                                }
                                            };
                                            match res {
                                                Err(e) => {
                                                    let msg = format!(
                                                        "Failed to save skill '{}': {}",
                                                        skill.name, e
                                                    );
                                                    error_msg = Some(msg);
                                                    crate::channels::send_session_notice(
                                                        &session_key,
                                                        &format!(
                                                            "{}▲ [Self-Improvement] Failed to save self-improvement skill '{}': {}{}",
                                                            AURA_GOLD, skill.name, e, COLOR_RESET
                                                        ),
                                                    );
                                                }
                                                Ok(()) => {
                                                    for sf in skill.supporting_files.iter().filter(|f| {
                                                        !f.path.trim().is_empty()
                                                            && !f.content.trim().is_empty()
                                                    }) {
                                                        let _ = crate::agent::skills::write_skill_package_file(
                                                            &skill.name,
                                                            &sf.path,
                                                            &sf.content,
                                                            profile_name.as_deref(),
                                                        );
                                                    }
                                                    skills_saved.push(skill.name.clone());
                                                    tracing::info!(session = %session_key, skill = %skill.name, "Self-improvement curator: saved skill.");
                                                    crate::channels::send_session_notice(
                                                        &session_key,
                                                        &format!(
                                                            "{}◇ [Self-Improvement] Skill '{}' synthesized & saved to ~/.openz/skills/{}/SKILL.md.{}",
                                                            AURA_BLUE, skill.name, skill.name, COLOR_RESET
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
                                                    crate::channels::send_session_notice(
                                                        &session_key,
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
                                                    crate::channels::send_session_notice(
                                                        &session_key,
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
                            }
                        }
                        Err(e) => {
                            let msg = format!("LLM chat query failed: {}", e);
                            write_log("failed", false, vec![], Some(msg));
                        }
                    }

                    let shared_consolidation_stamp = crate::config::runtime_data_dir()
                        .join("last_shared_memory_consolidation.json");
                    let consolidated_recently = shared_consolidation_stamp
                        .is_file()
                        .then(|| std::fs::read_to_string(&shared_consolidation_stamp).ok())
                        .flatten()
                        .and_then(|s| serde_json::from_str::<String>(&s).ok())
                        .and_then(|ts| chrono::DateTime::parse_from_rfc3339(&ts).ok())
                        .is_some_and(|dt| {
                            chrono::Utc::now()
                                .signed_duration_since(dt.with_timezone(&chrono::Utc))
                                < chrono::Duration::hours(24)
                        });
                    match consolidated_recently {
                        true => {}
                        false => {
                            let _ = serde_json::to_string(&chrono::Utc::now().to_rfc3339())
                                .ok()
                                .map(|ts_json| std::fs::write(&shared_consolidation_stamp, ts_json));
                            let _ =
                                crate::tools::shared_memory::consolidate_shared_memory(&provider)
                                    .await;
                        }
                    }
}

#[cfg(test)]
#[path = "save_tests.rs"]
mod tests;
