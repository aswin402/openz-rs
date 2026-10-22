use super::{AgentLoop, TurnContext, TurnState};
use crate::memory::{with_graph_db, with_shared_db};
use anyhow::Result;
use std::collections::HashSet;

pub const MIN_MATCH_SCORE: f64 = 6.0;
pub const MAX_SOURCE_CONTEXT_CHARS: usize = 3000;

fn extract_metadata_section(
    metadata: &serde_json::Map<String, serde_json::Value>,
    key: &str,
    heading: &str,
) -> String {
    metadata
        .get(key)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| format!("\n\n{}\n{}\n", heading, s))
        .unwrap_or_default()
}

pub(super) fn subagent_profile_name(session_key: &str) -> Option<&str> {
    let parts: Vec<&str> = session_key.split(':').collect();
    match parts.as_slice() {
        ["subagent", profile, ..] => Some(*profile),
        _ => None,
    }
}

fn format_skills_part(
    user_content: &str,
    messages: &[crate::session::Message],
    profile_name: Option<&str>,
) -> String {
    let relevant_skills = crate::agent::skills::load_relevant_skills_with_profile(
        user_content,
        messages,
        profile_name,
    )
    .unwrap_or_default();

    match relevant_skills.is_empty() {
        false => {
            let mut skills_part =
                "\n\nHere are the active guidelines and procedural skills you should follow:\n"
                    .to_string();
            let top_count = 2.min(relevant_skills.len());
            for skill in relevant_skills.iter().take(top_count) {
                skills_part.push_str(&format!(
                    "=== Active Skill: {} ===\n{}\n\n",
                    skill.name, skill.content
                ));
            }
            let _ = (relevant_skills.len() > top_count).then(|| {
                skills_part.push_str("[Indexed Skills Catalog]\nThe following additional skills are active in ~/.openz/skills/:\n");
                for skill in relevant_skills.iter().skip(top_count) {
                    let desc = skill.description();
                    let triggers = skill.triggers();
                    let triggers_str = (!triggers.is_empty())
                        .then(|| format!(" (triggers: {})", triggers.join(", ")))
                        .unwrap_or_default();
                    skills_part.push_str(&format!("- {}: {}{}\n", skill.name, desc, triggers_str));
                }
                skills_part.push('\n');
            });
            skills_part
        }
        true => crate::agent::skills::load_skills_with_profile(profile_name)
            .ok()
            .filter(|all_skills| !all_skills.is_empty())
            .map(|all_skills| {
                let mut skills_part = "\n\n[Available Skills Catalog]\nThe following procedural skills are indexed in ~/.openz/skills/:\n".to_string();
                for skill in all_skills.iter().take(5) {
                    skills_part.push_str(&format!("- {}: {}\n", skill.name, skill.description()));
                }
                skills_part.push('\n');
                skills_part
            })
            .unwrap_or_default(),
    }
}

pub async fn handle(loop_ref: &AgentLoop, ctx: &mut TurnContext<'_>) -> Result<TurnState> {
    let config = &ctx.config;
    let summary_part = extract_metadata_section(
        &ctx.session.metadata,
        "summary",
        "Here is a summary of the earlier part of the conversation:",
    );
    let memory_part = extract_metadata_section(
        &ctx.session.metadata,
        "memory",
        "Here is the long-term memory of key facts, preferences, and decisions from this session:",
    );
    let profile_name = subagent_profile_name(ctx.session_key);
    let skills_part = format_skills_part(ctx.user_content, &ctx.session.messages, profile_name);
    let vision_instruction = (!crate::providers::model_supports_vision(&config.agents.defaults.model))
        .then_some(" If a message contains a markdown image link (e.g. ![](file://...)) and you need to analyze or describe the image, you MUST call the direct 'vision_agent' tool to see and report on the image contents. Do not route image analysis through generic 'delegate_task' when the specialized 'vision_agent' tool is available; generic delegation may select a non-vision model.")
        .unwrap_or("");
    let subagents_list = crate::subagents::load_profiles()
        .map(|profiles| {
            profiles
                .iter()
                .map(|p| p.name.clone())
                .collect::<Vec<String>>()
                .join(", ")
        })
        .unwrap_or_else(|_| {
            "planner, researcher, debugger, DevOps, skill_improvement, openz_maintainer, mcps_manager"
                .to_string()
        });
    let vault_path = crate::core::vault::resolve_vault_path(config);
    let vault_display = vault_path.display().to_string();
    let vault_name = &config.vault.name;
    let vault_guideline = format!(
        "The user's creative deliverables vault is active at '{vault_display}' (name: '{vault_name}'). When the user asks you to create, generate, build, or scrape content (websites, apps, scripts, images, videos, reports, presentations, datasets, diagrams, templates, or exports), ALWAYS organize and save deliverables in the user's OpenZ Vault under the appropriate category:\n\
         - Websites & Web Apps: {vault_display}/websites/<project_name>/ (e.g. index.html, styles.css, app.js)\n\
         - Scripts & Automations: {vault_display}/scripts/<script_name> (e.g. scraper.py, backup.sh)\n\
         - Images & Renders: {vault_display}/images/<image_name> (e.g. logo.png, mockup.png)\n\
         - Programmatic Videos: {vault_display}/videos/<video_name> (e.g. explainer.mp4)\n\
         - Documents & Reports: {vault_display}/documents/<doc_name> (e.g. report.pdf, spec.docx)\n\
         - Presentations & Slides: {vault_display}/presentations/<deck_name> (e.g. pitch.pptx)\n\
         - Extracted Data & Crawls: {vault_display}/data/<data_name> (e.g. jobs.csv, dataset.json)\n\
         - Architecture & Diagrams: {vault_display}/diagrams/<diagram_name> (e.g. architecture.svg, flow.mermaid)\n\
         - Templates & Starter Kits: {vault_display}/templates/<template_name>/\n\
         - Chat & Knowledge Exports: {vault_display}/exports/<export_name> (e.g. chat_summary.md)\n\
         Ensure you create and write to this vault path so the user can easily find, preview, and use their deliverables. Never store user project outputs inside internal configuration ~/.openz/ (which is reserved exclusively for system configs, sessions, and SQLite databases)."
    );
    let system_guidelines = format!(
        "\n\nYou are OpenZ, a high-performance personal AI agent framework built in Rust, vibe-coded by Aswin. Your official GitHub repository and source code resides at https://github.com/aswin402/openz-rs. You are inspired by Zeroclaw, Nanobot, hermes-agent, loops!, and DOX. Your architecture is structured as follows:\n\
         * Creator & Inspiration: Vibe-coded by Aswin. Inspired by Zeroclaw, Nanobot, hermes-agent, loops!, and DOX. Official Repository: https://github.com/aswin402/openz-rs\n\
          * Specifications & Changelog: OpenZ is MIT-licensed open-source software; measured binary size depends on compiled heavy dependencies; RAM ~15-30MB cloud / ~200MB+ local embeddings; core CLI is millisecond-scale while full TUI startup varies by enabled checks. Version history:\n{}\n\
    \n\
          * CLI Subcommands & Flags: The executable is launched via:\n\
            - 'openz onboard': Runs the setup wizard for LLM provider API keys.\n\
            - 'openz configure': Configures providers, gateways, channels, and preferences.\n\
            - 'openz': Starts the interactive TUI terminal chat loop (auto-starts background channels & cron job scheduler).\n\
            - 'openz gateway': Starts the WebSocket + WebUI server (default port 8765).\n\
            - 'openz telegram': Starts the Telegram bot polling listener.\n\
            - 'openz discord': Starts the Discord bot gateway listener.\n\
            - 'openz whatsapp': Starts the WhatsApp Axum webhook receiver (default port 8090).\n\
            - 'openz subagent': Starts the TUI subagent profile manager.\n\
            - 'openz logs [--path <file>] [--tail <lines>] [--session <prefix>]': View real-time color-coded structured logs (live follow mode with rotation detection).\n\
            - 'openz mcp-bridge --port <port> -- <command> [args...]': Runs a gRPC MCP bridge wrapper.\n\
            - 'openz sop <list | instances | trigger <id> | resume <id> | simulate <id>>': Controls the stateful SOP workflow engine.\n\
          * Pluggable Gateway Channels: You can receive messages and reply over CLI terminal, WebSocket gateway (serving the WebUI workbench), Telegram bot polling, Discord bot polling, WhatsApp Business API, and pure Rust IMAP/SMTP Email client.\n\
          * Local Tools & MCP: {}\n\
          * Runtime Tool Discipline:\n\
    {}\n\
          * OpenZ Vault (User Deliverables & Projects Workspace): {}\n\
          * Context Scoping & Compression: You have native tools for context management:\n\
            - 'scope_context' (with target_path): Walks up the tree and compiles relevant AGENTS.md instructions. Use this BEFORE editing files to retrieve rules.\n\
            - 'web_fetch' cache discipline: for direct URL checks or when the user asks to verify/check again/refresh/browse, pass cache_mode=\"revalidate\"; use cache_mode=\"bypass\" only when the user explicitly requests a fresh uncached fetch.\n\
            - Web acquisition/open workflow discipline: if a search or fetch backend fails while the user asks to find, download, open, play, or show a file/URL, do not declare the task blocked while another relevant path remains. Prefer brokered/headless browser-backed paths (`searchxyz_browser_search`, `obscura_browser`, then `firefox_browser`; use `gsd_browser` only as last fallback), inspect DOM/source/image links, download only after finding a concrete URL, then use `open_path`/device inventory to show it. Repeated browser snapshots/page-source reads are normal observation steps. For local GUI display requests, prefer `open_path` first; only try explicit desktop apps or shell launchers after `open_path` fails or the user says the app did not appear. For one direct URL, choose exactly one reader (`web_fetch` or `searchxyz_read_url`) and do not call both; stay on that page unless the user explicitly asks for deeper, broader, comparative, multi-source, scrape, download, source-code, or local-copy work. Extraction tasks may follow embed and asset URLs; use `searchxyz_search_and_read` only when broader discovery is actually requested.\n\
            - 'compress_content' (with raw_text and content_type): Compresses logs/code/JSON and registers a CCR reference token (CCR ID).\n\
            - 'retrieve_original' (with ccr_id): Retrieves the original raw text. Use this to read the full content of any truncated output or file (it accepts both CCR IDs and file:// file paths!).\n\
          * Remote Session Control: If the user asks you (e.g., via Telegram or Discord) to execute a command, answer an approval prompt, or run a query in their TUI/CLI session, invoke the 'send_remote_input' tool to forward the prompt directly to that session (e.g., 'cli:direct').\n\
          * Specialized Subagents: You can spawn concurrent subagents (available subagent tools: {}) to delegate tasks.\n\
          * Stateful SOP Workflow Engine: DAG-based template executions (like 'ship-pr-until-green' closed-loop healing, PR creation, CI verification) with Zenflow checkpointed transactions and auto-rollback.\n\
          * Compiler Auto-Healing: 'CompilerAutoHealTool' compiles code natively, reads compiler output, and prompts you to fix syntax or borrow checker issues in a loop until green.\n\
          * Security Guard & BPF Sandbox: Subprocesses are sandboxed using a Linux seccomp BPF filter to block dangerous commands, with strict/normal/loose levels. Pre-flight discipline: the guard runs BEFORE execution but AFTER you emit the call — exec_command payloads containing sudo/su/chmod/chown/eval/source, power commands (shutdown/reboot/poweroff/halt), destructive deletes (rm/rmdir/unlink/dd/mkfs/fdisk, 'cargo clean', 'npm run clean'), and write_file/patch targets outside the workspace or ~/.openz will always require interactive user approval or be denied. Do not emit such calls unless the user explicitly asked for that exact action; prefer non-destructive alternatives (patch_file/replace_lines instead of deleting and rewriting, workspace-relative paths, no privilege escalation) so turns are not wasted generating calls that will only be denied.\n\
          * Cryptographic Audit Ledger: Uses SHA-256 Merkle chain hashing on all session messages/states, verified on boot, with a '/audit' slash command.\n\
          * Proactive Memory & Knowledge Graph: Three memory layers. (1) Simple KV: 'store_memory'/'recall_memory'/'clear_memory' for quick facts. (2) Working/Ephemeral: 'set_working_memory'/'get_working_memory'/'evict_expired_working_memory' for temporary session context. (3) Knowledge Graph: 'create_entities'/'create_relations'/'add_observations'/'read_graph'/'search_nodes'/'open_nodes' for structured relational memory. Use semantic tools 'smart_store'/'extract_and_store_facts'/'proactive_recall' for smart fact extraction and retrieval. Don't ask for permission — just store it.\n\
          * User Preference Discipline: Stable preferences in [Pinned Memory] are hard constraints. If pinned memory says no emojis, do not use emojis in final answers, headings, or tables.\n\
          * Self-Improvement System: An asynchronous background curator refines your memory facts and procedural skills stored under ~/.openz/skills/ and SQLite database (~/.openz/memory.db).",
        get_version_history(),
        get_dynamic_tools_guideline(&loop_ref.tools),
        runtime_tool_discipline_rules(),
        vault_guideline,
        subagents_list
    );

    let workspace_root_display = crate::tools::subagent::workspace::current_workspace_root()
        .to_string_lossy()
        .to_string();
    let mut activity_part = format!(
        "\n\n[Active Workspace]\nCurrent working directory: {} (resolve relative paths like '.' or filenames directly inside this workspace; do not assume another repository path from past sessions).",
        workspace_root_display
    );
    let _ = crate::agent::activity::get_activity()
        .filter(|activity| activity.session_id != ctx.session_key)
        .map(|activity| {
            activity_part.push_str(&format!(
                "\n\n[SYSTEM NOTICE] Status of the other active/last-run session on this computer:\n\
                 * Session ID: {}\n\
                 * Status: {}\n\
                 * Last/Current Tool: {}\n\
                 * Timestamp: {}\n",
                activity.session_id,
                activity.status,
                activity.current_tool.as_deref().unwrap_or("None"),
                activity.timestamp
            ));
        });

    let caveman_rules = config
        .agents
        .defaults
        .caveman_mode
        .then_some("\n\nRespond terse like smart caveman. All technical substance stay. Only fluff die.\nRules:\n- Drop: articles (a/an/the), filler (just/really/basically), pleasantries, hedging\n- Fragments OK. Short synonyms. Technical terms exact. Code unchanged.\n- Pattern: [thing] [action] [reason]. [next step].\n- Not: \"Sure! I'd be happy to help you with that.\"\n- Yes: \"Bug in auth middleware. Fix:\"")
        .unwrap_or("");

    let user_profile_raw = crate::agent::skills::load_user_profile();
    let user_profile_part = (!user_profile_raw.trim().is_empty())
        .then(|| {
            format!(
                "\n\n[User Profile & Design Preferences]\nFollow these persistent user preferences and styling guidelines unconditionally across all deliverables:\n{}\n",
                user_profile_raw.trim()
            )
        })
        .unwrap_or_default();

    let pinned_memory =
        retrieve_pinned_identity_memories(&config.agents.defaults.bot_name).await;
    let persona_priority_part = identity_answer_priority_context(ctx.user_content, &pinned_memory);
    let recent_session_part = recent_session_context(&ctx.session.messages, 2000);
    let brief_context = retrieve_research_brief_context(ctx.session_key, ctx.user_content).await;
    let source_context =
        retrieve_source_context(ctx.session_key, ctx.user_content, !brief_context.is_empty()).await;
    let workflow_context = retrieve_workflow_context(
        ctx.session_key,
        ctx.user_content,
        &mut ctx.workflow_notice_names,
    )
    .await;
    let weak_model_rules = weak_model_operating_rules(&config.agents.defaults.model);
    let grounding_rules = crate::grounding::main_agent_grounding_rules();
    let cross_session_memory =
        retrieve_cross_session_memories(ctx.user_content, &config.agents.defaults.bot_name).await;

    let budget_limit = crate::providers::DynamicContextRegistry::resolve_prompt_budget_chars(
        &config.agents.defaults.model,
        config,
    );
    let now_rfc3339 = chrono::Utc::now().to_rfc3339();
    let static_prefix = StaticPromptPrefix {
        bot_name: &config.agents.defaults.bot_name,
        user_profile_part: &user_profile_part,
        system_guidelines: &system_guidelines,
        grounding_rules: &grounding_rules,
        weak_model_rules: &weak_model_rules,
        vision_instruction,
        caveman_rules,
        pinned_memory: &pinned_memory,
    };
    let dynamic_suffix = DynamicPromptSuffix {
        persona_priority_part: &persona_priority_part,
        summary_part: &summary_part,
        memory_part: &memory_part,
        cross_session_memory: &cross_session_memory,
        skills_part: &skills_part,
        workflow_context: &workflow_context,
        brief_context: &brief_context,
        source_context: &source_context,
        recent_session_part: &recent_session_part,
        activity_part: &activity_part,
        utc_timestamp_rfc3339: &now_rfc3339,
    };

    ctx.system_prompt =
        assemble_kv_cached_system_prompt(&static_prefix, &dynamic_suffix, budget_limit);
    ctx.messages = ctx.session.messages.clone();
    Ok(TurnState::Run)
}

#[derive(Debug, Clone)]
pub(crate) struct StaticPromptPrefix<'a> {
    pub bot_name: &'a str,
    pub user_profile_part: &'a str,
    pub system_guidelines: &'a str,
    pub grounding_rules: &'a str,
    pub weak_model_rules: &'a str,
    pub vision_instruction: &'a str,
    pub caveman_rules: &'a str,
    pub pinned_memory: &'a str,
}

impl StaticPromptPrefix<'_> {
    pub(crate) fn render(&self) -> String {
        format!(
            "You are {}, a helpful assistant. Keep replies clear, precise, and concise.{}{}{}{}{}{}{}",
            self.bot_name,
            self.user_profile_part,
            self.system_guidelines,
            self.grounding_rules,
            self.weak_model_rules,
            self.vision_instruction,
            self.caveman_rules,
            self.pinned_memory,
        )
    }
}

#[derive(Debug, Clone)]
pub(crate) struct DynamicPromptSuffix<'a> {
    pub persona_priority_part: &'a str,
    pub summary_part: &'a str,
    pub memory_part: &'a str,
    pub cross_session_memory: &'a str,
    pub skills_part: &'a str,
    pub workflow_context: &'a str,
    pub brief_context: &'a str,
    pub source_context: &'a str,
    pub recent_session_part: &'a str,
    pub activity_part: &'a str,
    pub utc_timestamp_rfc3339: &'a str,
}

pub(crate) fn safe_truncate(s: &str, max_chars: usize) -> String {
    const SUFFIX: &str = "\n... [truncated]";
    let char_count = s.chars().count();
    let suffix_len = SUFFIX.chars().count();
    match (char_count <= max_chars, max_chars > suffix_len) {
        (true, _) => s.to_string(),
        (false, true) => {
            let mut truncated: String = s
                .chars()
                .take(max_chars.saturating_sub(suffix_len))
                .collect();
            truncated.push_str(SUFFIX);
            truncated
        }
        (false, false) => s.chars().take(max_chars).collect(),
    }
}

pub(crate) fn allocate_dynamic_budgets(
    budget: usize,
    skills_len: usize,
    cross_len: usize,
) -> (usize, usize) {
    let half_budget = budget / 2;
    match (skills_len <= half_budget, cross_len <= half_budget) {
        (true, _) => (skills_len, budget.saturating_sub(skills_len)),
        (false, true) => (budget.saturating_sub(cross_len), cross_len),
        (false, false) => (half_budget, budget.saturating_sub(half_budget)),
    }
}

pub(crate) fn assemble_kv_cached_system_prompt(
    prefix: &StaticPromptPrefix<'_>,
    suffix: &DynamicPromptSuffix<'_>,
    budget_limit: usize,
) -> String {
    let static_prefix = prefix.render();
    let temporal_tail = format!(
        "\n\n[Current Turn Timestamp]\nCurrent date and time: {}",
        suffix.utc_timestamp_rfc3339
    );
    let fixed_dynamic_len = suffix.persona_priority_part.chars().count()
        + suffix.summary_part.chars().count()
        + suffix.memory_part.chars().count()
        + suffix.workflow_context.chars().count()
        + suffix.brief_context.chars().count()
        + suffix.source_context.chars().count()
        + suffix.recent_session_part.chars().count()
        + suffix.activity_part.chars().count()
        + temporal_tail.chars().count();
    let base_len = static_prefix.chars().count() + fixed_dynamic_len;
    let skills_len = suffix.skills_part.chars().count();
    let cross_len = suffix.cross_session_memory.chars().count();
    let total_len = base_len + skills_len + cross_len;

    let (fitted_skills, fitted_cross) = match total_len > budget_limit {
        false => (
            suffix.skills_part.to_string(),
            suffix.cross_session_memory.to_string(),
        ),
        true => {
            let remaining_budget = budget_limit.saturating_sub(base_len);
            let (skills_budget, cross_budget) =
                allocate_dynamic_budgets(remaining_budget, skills_len, cross_len);
            (
                safe_truncate(suffix.skills_part, skills_budget),
                safe_truncate(suffix.cross_session_memory, cross_budget),
            )
        }
    };

    format!(
        "{}{}{}{}{}{}{}{}{}{}{}{}",
        static_prefix,
        suffix.persona_priority_part,
        suffix.summary_part,
        suffix.memory_part,
        fitted_cross,
        fitted_skills,
        suffix.workflow_context,
        suffix.brief_context,
        suffix.source_context,
        suffix.recent_session_part,
        suffix.activity_part,
        temporal_tail,
    )
}

fn recent_session_context(messages: &[crate::session::Message], max_chars: usize) -> String {
    let mut lines: Vec<String> = messages
        .iter()
        .rev()
        .filter(|msg| msg.role == "user" || msg.role == "assistant")
        .take(6)
        .map(|msg| {
            let content = msg.content.split_whitespace().collect::<Vec<_>>().join(" ");
            let label = match msg.role.as_str() {
                "user" => "User",
                _ => "Assistant",
            };
            format!("- {}: {}", label, content)
        })
        .collect();
    lines.reverse();
    match lines.is_empty() {
        true => String::new(),
        false => {
            let mut out = String::from(
                "\n\n[Recent Session Context]\nUse these latest turns before claiming there is no current-session context:\n",
            );
            for line in lines {
                match out.chars().count() + line.chars().count() + 1 > max_chars {
                    true => break,
                    false => {
                        out.push_str(&line);
                        out.push('\n');
                    }
                }
            }
            out
        }
    }
}

fn is_identity_or_persona_query(text: &str) -> bool {
    let normalized = text.to_lowercase();
    [
        "my name",
        "who am i",
        "what am i called",
        "what do you know about me",
        "remember about me",
        "your name",
        "who are you",
        "who are u",
        "who r u",
        "what are you called",
        "persona",
        "personality",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
}

pub fn resolve_prompt_budget(configured_budget: Option<usize>, context_window: usize) -> usize {
    configured_budget.unwrap_or_else(|| ((context_window * 3) / 8).max(8000))
}

fn identity_answer_priority_context(user_content: &str, pinned_memory: &str) -> &'static str {
    (is_identity_or_persona_query(user_content) && !pinned_memory.trim().is_empty())
        .then_some(
            "\n\n[Identity Answer Priority]\nThe active persona and user preference facts in [Pinned Memory] outrank the generic system product identity for identity/persona questions. If asked who you are, answer as the active persona first, then mention OpenZ only as underlying system/context if useful. Do not ignore custom persona memory because the generic header says OpenZ.\n",
        )
        .unwrap_or("")
}

fn identity_memory_candidate(text: &str, bot_name: &str) -> bool {
    let normalized = text.to_lowercase();
    let bot_norm = bot_name.trim().to_lowercase();
    (!bot_norm.is_empty() && normalized.contains(&bot_norm))
        || [
            "name",
            "call",
            "called",
            "persona",
            "personality",
            "friend",
            "preference",
            "prefers",
            "wants",
            "likes",
            "creator",
            "developer",
        ]
        .iter()
        .any(|needle| normalized.contains(needle))
}

fn runtime_tool_discipline_rules() -> &'static str {
    "- When asked what OpenZ can do, what features/tools exist, or how OpenZ compares, call 'openz_inventory' (and 'tool_catalog' only for deeper schema/routing details) before giving exact counts. Do not guess feature/tool counts from memory.\n\
            - When asked model/provider identity questions such as 'what model are you', 'are you DeepSeek', 'which provider are you using', or model capability questions such as 'which programming language are you best at', call 'openz_inventory' first and answer from runtime_identity. Do not guess hidden model architecture, training data, parameter count, or benchmark ranking. State uncertainty when the live config only exposes a model label/provider.\n\
            - Layered tool use: L0 classify the turn as direct answer, local repo, local execution, web research, memory, cron, media/document, or orchestration. Complete the request directly when the answer is stable, trivial, or already available in local context. Do not delegate or research for trivial/general-knowledge tasks. Use web/search/browser tools only when the user asks for research, provides an external URL, asks for latest/current/recent info, or when accuracy depends on live external facts. Use local repo tools for questions about this repository, current directory, implementation locations, diffs, tests, or files. If a required tool is not visible, call 'request_tool_scope' with the exact missing tool or domain instead of guessing.\n\
            - Task lifecycle discipline: OpenZ tracks OpenZ-owned browsers, servers, agents, subagents, MCP bridges, watchers, and background jobs automatically. Use 'manage_tasks' internally to clean/list/stop OpenZ-owned resources when their purpose ends, when the user says done/stop/cleanup, or before finalizing verification-only resources. Never ask the user to run manual task-management slash commands. Never stop external/user-owned resources unless the user explicitly asks and approval flow allows it. When 'exec_command' launches a dev server/background server and returns server_registered=true, treat the launch as complete. Do not retry with pkill/ps guesses. If the user is actively previewing a server, report the server id and leave it running until no longer needed.\n\
            - When creating large websites, generated source files, or video timelines, avoid one huge 'write_file' payload. Prefer chunked file creation/append steps or smaller artifacts, then verify file exists. For 20s+ HTML videos, render in shorter segments and concatenate. After a repeated successful workaround, save it with 'workflow_memory' or 'curate_skill'.\n\
            - When answering from recent memory or saved research, do not scold the user for repeating a question. Answer normally and mention the saved context only if useful.\n\
            - Credential & Configuration Management: When the user provides an API key, personal access token (e.g. GitHub/GitLab token), or asks to store credentials, invoke 'manage_config' (with action 'set_credential' or target/token fields) to persist it. OpenZ will automatically prompt the user for confirmation via SecurityGuard with secret redaction before writing to ~/.openz/config.json. Never claim that OpenZ cannot store credentials, never claim that config only stores LLM providers, and never tell the user to manually export env vars when 'manage_config' is available.\n\
            - Deliverables & Artifact Storage (OpenZ Vault): When the user requests the creation of a website, web application, script, document, image, video, presentation, or dataset, write the generated files into the OpenZ Vault under the appropriate category folder (websites, scripts, images, videos, documents, presentations, data, diagrams, templates, exports). If the user asks where their creations are or how to customize the vault, explain that the vault is located at the configured path, customizable in config.json under 'vault.path' or via the OPENZ_VAULT_DIR environment variable."
}

use crate::providers::risk::model_name_suggests_strong;

fn is_weak_or_risky_model(model: &str) -> bool {
    (!model_name_suggests_strong(model)) && {
        let m = model.to_lowercase();
        let tokens: Vec<&str> = m
            .split(|c: char| !c.is_ascii_alphanumeric())
            .filter(|s| !s.is_empty())
            .collect();
        [
            "1b",
            "2b",
            "3b",
            "4b",
            "6b",
            "7b",
            "8b",
            "9b",
            "small",
            "mini",
            "lite",
            "free",
            "preview",
            "experimental",
            "beta",
            "pickle",
            "mimo",
            "hy3",
        ]
        .iter()
        .any(|needle| tokens.contains(needle))
    }
}

fn weak_model_operating_rules(model: &str) -> String {
    is_weak_or_risky_model(model)
        .then(|| {
            "\n\n[Small Model Operating Rules]\n- Use [Recent Session Context] before saying you do not know what this session discussed.\n- Use [Pinned Memory] before answering identity, persona, or preference questions.\n- For tool-heavy tasks, keep steps short and prefer exact tool schemas over guessing.\n- For broad tasks, research first, then make an implementation plan and todo list before editing.\n"
                .to_string()
        })
        .unwrap_or_default()
}

fn concrete_research_topic_terms(text: &str) -> Vec<String> {
    let canonical = crate::tools::shared_memory::auto_capture::canonical_research_topic(text);
    const GENERIC_TERMS: &[&str] = &[
        "new", "news", "latest", "current", "recent", "update", "updates", "today", "now", "what",
        "whats", "what's", "happened", "anything", "any", "fresh", "version", "in", "on", "for",
        "about",
    ];
    canonical
        .split(|c: char| !c.is_alphanumeric() && c != '/' && c != '.' && c != '-')
        .map(str::trim)
        .filter(|term| term.len() > 1 && !GENERIC_TERMS.contains(term))
        .map(ToString::to_string)
        .collect()
}

fn is_local_operational_or_ack_query(text: &str) -> bool {
    let lower = text.to_lowercase();
    let normalized = lower.split_whitespace().collect::<Vec<_>>().join(" ");

    let action_start = [
        "open ", "play ", "show ", "launch ", "run ", "stop ", "close ", "start ",
    ]
    .iter()
    .any(|prefix| normalized.starts_with(prefix));
    let local_target = [
        "website",
        "web site",
        "video",
        "browser",
        "firefox",
        "chrome",
        "vlc",
        "media player",
        "server",
        "servers",
        "file",
        "folder",
        "directory",
        "html",
        "mp4",
    ]
    .iter()
    .any(|needle| normalized.contains(needle));

    let ack = [
        "good",
        "good one",
        "thats good",
        "that's good",
        "nice",
        "ok",
        "okay",
        "thanks",
        "thank you",
        "done",
        "works",
        "worked",
    ];
    (action_start && local_target)
        || ack
            .iter()
            .any(|phrase| normalized == *phrase || normalized.starts_with(&format!("{} ", phrase)))
}

fn is_openz_self_inventory_query(text: &str) -> bool {
    let lower = text.to_lowercase();
    let asks_inventory = [
        "tool",
        "tools",
        "feature",
        "features",
        "capability",
        "capabilities",
        "what can you do",
        "what do you have",
        "what are u",
        "what are you",
        "inventory",
    ]
    .iter()
    .any(|needle| lower.contains(needle));

    let asks_external_comparison = [
        "compare",
        " compared to",
        " compared with",
        " vs ",
        " versus ",
        "different from",
        "difference between",
        "better than",
        "worse than",
    ]
    .iter()
    .any(|needle| lower.contains(needle));

    asks_inventory && !asks_external_comparison
}

fn should_skip_research_memory_for_generic_query(text: &str) -> bool {
    (!super::research_policy::text_has_http_url(text))
        && (is_openz_self_inventory_query(text)
            || is_local_operational_or_ack_query(text)
            || concrete_research_topic_terms(text).is_empty())
}

fn should_show_saved_context_notification(text: &str) -> bool {
    !super::research_policy::has_live_research_intent(text)
}

fn format_research_brief_context_items(
    items: &[crate::tools::shared_memory::ResearchBrief],
    user_content: &str,
) -> String {
    match items.is_empty() {
        true => String::new(),
        false => {
            let current_sensitive = super::research_policy::has_live_research_intent(user_content);
            let mut out = String::from(
                "\n\n[Relevant Research Briefs]\nUse these saved briefs first for simple definition/comparison questions. Do not call web/search tools when a fresh brief answers a stable non-live question. If the user provides a URL, asks to verify/check again, or asks for latest/current data, refresh the exact source or web before final answer. Only state facts present in the briefs or saved sources; say unknown if a requested detail is missing. Do not guess licenses, channels, release numbers, integrations, or comparisons from memory alone:\n",
            );
            let intent_line = match current_sensitive {
                true => "- Live/refresh intent detected: verify exact saved sources or web before final answer.\n",
                false => "- Stable intent detected: answer directly from fresh briefs/sources when they cover the question; do not call web/search only to improve wording or re-check stable definitions.\n",
            };
            out.push_str(intent_line);
            for item in items.iter().take(3) {
                let line = format!(
                    "- {} | freshness={} ttl={}s confidence={:.2} score={:.2} | summary: {}\n",
                    item.topic,
                    item.freshness,
                    item.stale_after_secs,
                    item.confidence,
                    item.score,
                    item.summary
                );
                match out.chars().count() + line.chars().count() > 3600 {
                    true => break,
                    false => out.push_str(&line),
                }
            }
            out
        }
    }
}

async fn retrieve_research_brief_context(session_key: &str, user_content: &str) -> String {
    match should_skip_research_memory_for_generic_query(user_content) {
        true => String::new(),
        false => match crate::tools::shared_memory::search_research_briefs(user_content, 3).await {
            Ok(items) => {
                let _ = should_show_saved_context_notification(user_content)
                    .then(|| items.first().filter(|item| item.score >= MIN_MATCH_SCORE))
                    .flatten()
                    .map(|best| {
                        let notice = format!(
                            "◇ Research brief matched: {} ({})",
                            best.topic, best.freshness
                        );
                        crate::channels::send_notification(&notice);
                        crate::channels::websocket::publish_activity_notice(
                            session_key,
                            "research",
                            "Research brief matched",
                            format!("{} ({})", best.topic, best.freshness),
                        );
                    });
                format_research_brief_context_items(&items, user_content)
            }
            Err(err) => {
                tracing::debug!(error = ?err, "research brief context retrieval skipped");
                String::new()
            }
        },
    }
}

fn format_source_context_items(items: &[crate::tools::shared_memory::SourceBookmark]) -> String {
    match items.is_empty() {
        true => String::new(),
        false => {
            let mut out = String::from(
                "\n\n[Relevant Saved Sources]\nUse these ranked links, repos, docs, paths, or profiles before broad searching. If freshness=stale or the user asks for latest/current data, refresh the specific saved source first and then answer:\n",
            );
            for item in items.iter().take(4) {
                let line = format!(
                    "- {} [{}] {} | freshness={} ttl={}s trust={:.2} score={:.2} | aliases: {} | summary: {}\n",
                    crate::tools::shared_memory::display_source_label(&item.label, &item.uri),
                    item.kind,
                    item.uri,
                    item.freshness,
                    item.stale_after_secs,
                    item.trust_score,
                    item.score,
                    item.aliases.join(", "),
                    item.summary
                );
                match out.chars().count() + line.chars().count() > MAX_SOURCE_CONTEXT_CHARS {
                    true => break,
                    false => out.push_str(&line),
                }
            }
            out
        }
    }
}

fn should_notify_source_context(user_content: &str, brief_context_available: bool) -> bool {
    should_show_saved_context_notification(user_content) && !brief_context_available
}

async fn retrieve_source_context(
    session_key: &str,
    user_content: &str,
    brief_context_available: bool,
) -> String {
    match should_skip_research_memory_for_generic_query(user_content) {
        true => String::new(),
        false => match crate::tools::shared_memory::search_source_bookmarks(user_content, 4).await {
            Ok(items) => {
                let _ = should_notify_source_context(user_content, brief_context_available)
                    .then(|| items.first().filter(|item| item.score >= MIN_MATCH_SCORE))
                    .flatten()
                    .map(|best| {
                        let label = crate::tools::shared_memory::display_source_label(
                            &best.label,
                            &best.uri,
                        );
                        let notice = format!("◇ Sources matched: {} ({})", label, best.freshness);
                        crate::channels::send_notification(&notice);
                        crate::channels::websocket::publish_activity_notice(
                            session_key,
                            "research",
                            "Sources matched",
                            format!("{} ({})", label, best.freshness),
                        );
                    });
                format_source_context_items(&items)
            }
            Err(err) => {
                tracing::debug!(error = ?err, "source context retrieval skipped");
                String::new()
            }
        },
    }
}

fn summarize_workflow_steps(steps: &serde_json::Value) -> String {
    let arr = match steps.as_array() {
        Some(arr) => arr,
        None => return String::new(),
    };
    arr.iter()
        .take(5)
        .enumerate()
        .map(|(idx, step)| match step.as_object() {
            Some(obj) => {
                let tool = obj
                    .get("tool")
                    .or_else(|| obj.get("name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("step");
                let note = obj
                    .get("note")
                    .or_else(|| obj.get("description"))
                    .or_else(|| obj.get("action"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                match note.is_empty() {
                    true => format!("{}. {}", idx + 1, tool),
                    false => format!("{}. {} ({})", idx + 1, tool, note),
                }
            }
            None => format!("{}. {}", idx + 1, step),
        })
        .collect::<Vec<_>>()
        .join("; ")
}

fn format_workflow_context_items(items: &[crate::tools::shared_memory::WorkflowCard]) -> String {
    let relevant: Vec<_> = items
        .iter()
        .filter(|item| item.score >= 4.0)
        .take(3)
        .collect();
    match relevant.is_empty() {
        true => String::new(),
        false => {
            let mut out = String::from(
                "\n\n[Relevant Reusable Workflows]\nUse these matched procedures automatically for similar tasks instead of rediscovering steps. Follow preconditions, execute the steps, verify the result, then record success/failure with workflow_memory.record_run. For high-risk workflows, request approval before execution:\n",
            );
            for item in relevant {
                let steps = summarize_workflow_steps(&item.steps);
                let line = format!(
                    "- {} [{}] risk={} score={:.2} success={} failure={} | triggers: {} | preconditions: {} | summary: {} | steps: {} | verify: {}\n",
                    item.name,
                    item.status,
                    item.risk,
                    item.score,
                    item.success_count,
                    item.failure_count,
                    item.triggers.join(", "),
                    item.preconditions.join(", "),
                    item.summary,
                    steps,
                    item.verification.join(", ")
                );
                match out.chars().count() + line.chars().count() > 3600 {
                    true => break,
                    false => out.push_str(&line),
                }
            }
            out
        }
    }
}

fn should_emit_workflow_notice(seen: &mut HashSet<String>, workflow_name: &str) -> bool {
    seen.insert(workflow_name.to_string())
}

async fn retrieve_workflow_context(
    session_key: &str,
    user_content: &str,
    seen_workflow_notices: &mut HashSet<String>,
) -> String {
    match crate::tools::shared_memory::search_workflow_cards(user_content, 3, true).await {
        Ok(items) => {
            let _ = items
                .first()
                .filter(|item| item.score >= 4.0)
                .filter(|best| should_emit_workflow_notice(seen_workflow_notices, &best.name))
                .map(|best| {
                    let notice = format!("◇ Workflow matched: {}", best.name);
                    crate::channels::send_notification(&notice);
                    crate::channels::websocket::publish_activity_notice(
                        session_key,
                        "workflow",
                        "Workflow matched",
                        best.name.clone(),
                    );
                });
            format_workflow_context_items(&items)
        }
        Err(err) => {
            tracing::debug!(error = ?err, "workflow context retrieval skipped");
            String::new()
        }
    }
}

async fn retrieve_pinned_identity_memories(bot_name: &str) -> String {
    let mut entries: Vec<String> = Vec::new();

    let _lock = crate::tools::shared_memory::get_db_mutex().lock().await;
    let cognitive_rows = with_shared_db(|conn| {
        let mut stmt = conn.prepare(
            "SELECT text FROM cognitive_memory WHERE importance >= 0.75 ORDER BY importance DESC, last_accessed DESC LIMIT 24",
        )?;
        let mapped = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut collected = Vec::new();
        for item in mapped {
            collected.push(item?);
        }
        Ok(collected)
    });
    let _ = cognitive_rows.map(|rows| {
        entries.extend(
            rows.into_iter()
                .filter(|text| identity_memory_candidate(text, bot_name)),
        );
    });
    drop(_lock);

    let semantic_rows = with_graph_db(|conn| {
        let mut stmt = conn.prepare(
            "SELECT raw_text FROM semantic_metadata WHERE valid_until IS NULL AND importance >= 0.75 ORDER BY timestamp DESC LIMIT 48",
        )?;
        let mapped = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut collected = Vec::new();
        for item in mapped {
            collected.push(item?);
        }
        Ok(collected)
    })
    .unwrap_or_default();
    entries.extend(
        semantic_rows
            .into_iter()
            .filter(|text| identity_memory_candidate(text, bot_name)),
    );

    let mut seen = std::collections::HashSet::new();
    let mut out = String::new();
    for entry in entries {
        let normalized = entry.trim().to_lowercase();
        let is_new = !normalized.is_empty() && seen.insert(normalized);
        match is_new {
            false => continue,
            true => {
                let _ = out.is_empty().then(|| {
                    out.push_str("\n\n[Pinned Memory]\nUse these stable identity/persona/preference facts before guessing:\n");
                });
                let line = format!("- {}\n", entry.trim());
                match out.chars().count() + line.chars().count() > 1500 {
                    true => break,
                    false => out.push_str(&line),
                }
            }
        }
    }
    out
}

pub(crate) fn memory_query_terms(text: &str) -> std::collections::HashSet<String> {
    const STOP_WORDS: &[&str] = &[
        "about", "after", "again", "all", "and", "any", "are", "but", "call", "calls", "can",
        "check", "code", "create", "did", "dir", "does", "edit", "execute", "file", "files",
        "find", "for", "from", "function", "get", "has", "have", "help", "how", "into", "just",
        "line", "lines", "list", "make", "method", "modify", "more", "new", "now", "only", "our",
        "output", "patch", "path", "please", "read", "replace", "return", "run", "search", "set",
        "show", "some", "step", "task", "test", "that", "the", "their", "then", "there", "these",
        "this", "those", "tool", "tools", "too", "turn", "update", "use", "used", "user", "using",
        "value", "verify", "was", "were", "what", "when", "where", "which", "with", "work",
        "write", "you", "your",
    ];
    text.split(|c: char| !c.is_alphanumeric())
        .map(|t| t.to_lowercase())
        .filter(|t| t.len() >= 3 && !STOP_WORDS.contains(&t.as_str()))
        .collect()
}

pub(crate) fn memory_relevance_score(
    query_terms: &std::collections::HashSet<String>,
    text: &str,
) -> f32 {
    let text_terms = memory_query_terms(text);
    let overlap = query_terms.intersection(&text_terms).count();
    match (query_terms.is_empty(), text_terms.is_empty(), overlap == 0) {
        (true, _, _) => 0.0,
        (false, true, _) | (false, false, true) => 0.0,
        (false, false, false) => overlap as f32 / query_terms.len().max(1) as f32,
    }
}

pub(crate) fn has_foreign_workspace_path(
    text: &str,
    active_workspace: &str,
    user_content: &str,
) -> bool {
    let clean_ws = active_workspace.trim().trim_end_matches('/');
    (!clean_ws.is_empty()).then(|| {
        text.split(|c: char| {
            matches!(
                c,
                ' ' | '\t'
                    | '\n'
                    | '\r'
                    | '"'
                    | '\''
                    | '`'
                    | '('
                    | ')'
                    | '['
                    | ']'
                    | '{'
                    | '}'
                    | ','
                    | ';'
                    | ':'
            )
        })
        .map(|tok| tok.trim_end_matches(['.', '/', ')']))
        .filter(|tok| {
            (tok.starts_with("/home/")
                || tok.starts_with("/Users/")
                || tok.starts_with("/tmp/")
                || tok.starts_with("/var/folders/"))
                && tok.matches('/').count() >= 3
        })
        .any(|path_tok| {
            let is_active_ws =
                path_tok.starts_with(clean_ws) || clean_ws.starts_with(path_tok);
            let is_global_shared =
                path_tok.contains("/.openz") || path_tok.contains("/OpenZ_Vault");
            let explicitly_requested = user_content.contains(path_tok);
            !is_active_ws && !is_global_shared && !explicitly_requested
        })
    }) == Some(true)
}

async fn retrieve_cross_session_memories(user_content: &str, bot_name: &str) -> String {
    let query_terms = memory_query_terms(user_content);
    let wants_identity = is_identity_or_persona_query(user_content);
    let active_workspace = crate::tools::subagent::workspace::current_workspace_root()
        .to_string_lossy()
        .to_string();
    let mut all_entries: Vec<(f32, String)> = Vec::new();
    let uid = "*";
    let aid = "*";

    let _lock = crate::tools::shared_memory::get_db_mutex().lock().await;
    let cognitive_rows = with_shared_db(|conn| {
        let mut stmt = conn.prepare(
            "SELECT text, importance, last_accessed, decay_rate FROM cognitive_memory ORDER BY importance DESC, last_accessed DESC LIMIT 8",
        )?;
        let mapped = stmt.query_map([], |row| {
            let text: String = row.get(0)?;
            let importance: f32 = row.get(1)?;
            let last_acc: String = row.get(2)?;
            let decay_rate: f32 = row.get(3)?;
            Ok((text, importance, last_acc, decay_rate))
        })?;
        let mut collected = Vec::new();
        for item in mapped {
            collected.push(item?);
        }
        Ok(collected)
    });

    for (text, importance, last_acc, decay_rate) in cognitive_rows.unwrap_or_default() {
        let is_foreign = has_foreign_workspace_path(&text, &active_workspace, user_content);
        let days_elapsed = chrono::Utc::now()
            .signed_duration_since(
                chrono::DateTime::parse_from_rfc3339(&last_acc)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now()),
            )
            .num_seconds() as f32
            / 86400.0;
        let base_relevance = memory_relevance_score(&query_terms, &text);
        let identity_boost =
            (base_relevance == 0.0 && wants_identity && identity_memory_candidate(&text, bot_name))
                .then_some(1.0f32);
        let relevance = identity_boost.unwrap_or(base_relevance);
        let _ = (!is_foreign && relevance > 0.0).then(|| {
            let score = importance * (-decay_rate * days_elapsed).exp() * relevance;
            all_entries.push((score, text));
        });
    }
    drop(_lock);

    let semantic_facts = with_graph_db(|conn| {
        let mut stmt = conn
            .prepare(
                "SELECT raw_text FROM semantic_metadata
             WHERE valid_until IS NULL
               AND (user_id = ?1 OR user_id = '*')
               AND (agent_id = ?2 OR agent_id = '*')
             ORDER BY timestamp DESC LIMIT 100",
            )
            .map_err(|e| anyhow::anyhow!(e))?;
        let mut rows = stmt
            .query(rusqlite::params![uid, aid])
            .map_err(|e| anyhow::anyhow!(e))?;
        let mut facts = Vec::new();
        while let Some(row) = rows.next().map_err(|e| anyhow::anyhow!(e))? {
            let text: String = row.get(0).map_err(|e| anyhow::anyhow!(e))?;
            facts.push(text);
        }
        Ok(facts)
    })
    .unwrap_or_default();

    for fact in &semantic_facts {
        let is_foreign = has_foreign_workspace_path(fact, &active_workspace, user_content);
        let base_relevance = memory_relevance_score(&query_terms, fact);
        let identity_boost =
            (base_relevance == 0.0 && wants_identity && identity_memory_candidate(fact, bot_name))
                .then_some(1.0f32);
        let relevance = identity_boost.unwrap_or(base_relevance);
        let _ = (!is_foreign && relevance > 0.0).then(|| {
            all_entries.push((0.7 * relevance, fact.clone()));
        });
    }

    let graph_nodes = with_graph_db(|conn| {
        let mut stmt = conn
            .prepare(
                "SELECT name, observations FROM graph_nodes
             WHERE (user_id = ?1 OR user_id = '*')
               AND (agent_id = ?2 OR agent_id = '*')
             LIMIT 100",
            )
            .map_err(|e| anyhow::anyhow!(e))?;
        let mut rows = stmt
            .query(rusqlite::params![uid, aid])
            .map_err(|e| anyhow::anyhow!(e))?;
        let mut nodes = Vec::new();
        while let Some(row) = rows.next().map_err(|e| anyhow::anyhow!(e))? {
            let name: String = row.get(0).map_err(|e| anyhow::anyhow!(e))?;
            let obs_json: String = row.get(1).map_err(|e| anyhow::anyhow!(e))?;
            let obs: Vec<String> = serde_json::from_str(&obs_json).unwrap_or_default();
            nodes.push((name, obs));
        }
        Ok(nodes)
    })
    .unwrap_or_default();

    all_entries.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    fn normalize(s: &str) -> String {
        s.trim()
            .trim_start_matches(['-', '*', ' '])
            .trim_start_matches("**")
            .trim_end_matches("**")
            .trim()
            .to_lowercase()
    }

    let mut seen = std::collections::HashSet::new();
    let mut deduped: Vec<String> = Vec::new();
    for (_, text) in &all_entries {
        let norm = normalize(text);
        let is_duplicate = norm.is_empty()
            || seen.contains(&norm)
            || seen
                .iter()
                .any(|s: &String| s.contains(&norm) || norm.contains(s));
        let _ = (!is_duplicate).then(|| {
            seen.insert(norm);
            deduped.push(text.clone());
        });
    }

    let filtered_nodes: Vec<_> = graph_nodes
        .into_iter()
        .filter(|(name, obs)| {
            let joined = format!("{} {}", name, obs.join(" "));
            !obs.is_empty()
                && !has_foreign_workspace_path(&joined, &active_workspace, user_content)
                && memory_relevance_score(&query_terms, &joined) > 0.0
        })
        .collect();

    match deduped.is_empty() && filtered_nodes.is_empty() {
        true => String::new(),
        false => {
            let mut persistent_mem = String::from(
                "\n\nHere are persistent key facts, decisions, and context across all past sessions:\n",
            );
            for fact in deduped.iter().take(30) {
                persistent_mem.push_str(&format!("- {}\n", fact));
            }
            for (name, obs) in filtered_nodes.iter().take(5) {
                persistent_mem.push_str(&format!("- Entity '{}':\n", name));
                for ob in obs.iter().take(10) {
                    persistent_mem.push_str(&format!("  * {}\n", ob));
                }
            }
            persistent_mem
        }
    }
}

const CHANGELOG_CONTENT: &str = include_str!("../../../CHANGELOG.md");

fn get_version_history() -> String {
    let mut history = String::new();
    let mut recording = false;
    let mut header_count = 0;

    for line in CHANGELOG_CONTENT.lines() {
        let trimmed = line.trim();
        let is_version_header = trimmed.starts_with("### v");
        let _ = is_version_header.then(|| {
            header_count += 1;
            recording = true;
        });
        match is_version_header && header_count > 5 {
            true => break,
            false => {
                let _ = recording.then(|| {
                    history.push_str(line);
                    history.push('\n');
                });
            }
        }
    }
    history.trim().to_string()
}

const TOOL_NAMESPACE_CATEGORIES: &[(&str, &str)] = &[
    ("searchxyz_", "SearchXyz Tools"),
    ("openmedia_", "OpenMedia Tools"),
    ("opendoc_", "OpenDoc Tools"),
    ("github_", "GitHub Integration Tools"),
    ("docs_", "Local & Crates Docs Tools"),
];

fn get_dynamic_tools_guideline(registry: &crate::tools::ToolRegistry) -> String {
    let tools = registry.get_static_tools();
    let mut core_tools = Vec::new();
    let mut grouped: Vec<Vec<String>> = vec![Vec::new(); TOOL_NAMESPACE_CATEGORIES.len()];

    for t in tools {
        let name = t.name();
        let formatted = format!("'{}'", name);
        let matched_idx = TOOL_NAMESPACE_CATEGORIES
            .iter()
            .position(|(prefix, _)| name.starts_with(prefix));
        match matched_idx {
            Some(idx) => grouped[idx].push(formatted),
            None => core_tools.push(formatted),
        }
    }

    core_tools.sort();
    for list in &mut grouped {
        list.sort();
    }

    let mut out = String::new();
    out.push_str("You have native tools for files, shell execution, and other utilities. The following tools are registered in your environment:\n");
    let _ = (!core_tools.is_empty()).then(|| {
        out.push_str(&format!(
            "            - Core Tools: {}\n",
            core_tools.join(", ")
        ));
    });
    for ((_, label), list) in TOOL_NAMESPACE_CATEGORIES.iter().zip(grouped.iter()) {
        let _ = (!list.is_empty()).then(|| {
            out.push_str(&format!("            - {}: {}\n", label, list.join(", ")));
        });
    }
    out.push_str("            - MCP server integration managed via 'manage_mcp' tool.");
    out
}

#[cfg(test)]
#[path = "build_tests.rs"]
mod tests;
