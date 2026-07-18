use super::{AgentLoop, TurnContext, TurnState};
use anyhow::Result;

pub const MIN_MATCH_SCORE: f64 = 6.0;

pub async fn handle(loop_ref: &AgentLoop, ctx: &mut TurnContext<'_>) -> Result<TurnState> {
    let config = &ctx.config;
    let mut summary_part = String::new();
    if let Some(summary) = ctx.session.metadata.get("summary").and_then(|v| v.as_str()) {
        if !summary.is_empty() {
            summary_part = format!(
                "\n\nHere is a summary of the earlier part of the conversation:\n{}\n",
                summary
            );
        }
    }
    let mut memory_part = String::new();
    if let Some(memory) = ctx.session.metadata.get("memory").and_then(|v| v.as_str()) {
        if !memory.is_empty() {
            memory_part = format!(
                "\n\nHere is the long-term memory of key facts, preferences, and decisions from this session:\n{}\n",
                memory
            );
        }
    }
    let mut profile_name = None;
    let parts: Vec<&str> = ctx.session_key.split(':').collect();
    if parts.len() >= 2 && parts[0] == "subagent" {
        profile_name = Some(parts[1]);
    }

    let mut skills_part = String::new();
    if let Ok(skills) = crate::agent::skills::load_relevant_skills_with_profile(
        ctx.user_content,
        &ctx.session.messages,
        profile_name,
    ) {
        if !skills.is_empty() {
            skills_part =
                "\n\nHere are the active guidelines and procedural skills you should follow:\n"
                    .to_string();
            for skill in skills {
                skills_part.push_str(&format!(
                    "=== Skill: {} ===\n{}\n\n",
                    skill.name, skill.content
                ));
            }
        }
    }
    let mut vision_instruction = "";
    if !crate::providers::model_supports_vision(&config.agents.defaults.model) {
        vision_instruction = " If a message contains a markdown image link (e.g. ![](file://...)) and you need to analyze or describe the image, you MUST delegate the visual analysis task to the specialized 'vision_agent' tool (or the 'delegate_task' tool) to see and report on the image contents.";
    }
    let subagents_list = if let Ok(profiles) = crate::subagents::load_profiles() {
        profiles
            .iter()
            .map(|p| p.name.clone())
            .collect::<Vec<String>>()
            .join(", ")
    } else {
        "planner, researcher, debugger, DevOps, skill_improvement, openz_maintainer, mcps_manager"
            .to_string()
    };
    let system_guidelines = format!(
        "\n\nYou are OpenZ, a high-performance personal AI agent framework built in Rust, vibe-coded by Aswin. Your official GitHub repository and source code resides at https://github.com/aswin402/openz-rs. You are inspired by Zeroclaw, Nanobot, hermes-agent, loops!, and DOX. Your architecture is structured as follows:\n\
         * Creator & Inspiration: Vibe-coded by Aswin. Inspired by Zeroclaw, Nanobot, hermes-agent, loops!, and DOX. Official Repository: https://github.com/aswin402/openz-rs\n\
          * Specifications & Changelog: measured binary size depends on compiled heavy dependencies; RAM ~15-30MB cloud / ~200MB+ local embeddings; core CLI is millisecond-scale while full TUI startup varies by enabled checks. Version history:\n{}\n\
\n\
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
          * Local Tools & MCP: {}\n\
\n\
          * Context Scoping & Compression: You have native tools for context management:\n\
            - 'scope_context' (with target_path): Walks up the tree and compiles relevant AGENTS.md instructions. Use this BEFORE editing files to retrieve rules.\n\
            - 'compress_content' (with raw_text and content_type): Compresses logs/code/JSON and registers a CCR reference token (CCR ID).\n\
            - 'retrieve_original' (with ccr_id): Retrieves the original raw text. Use this to read the full content of any truncated output or file (it accepts both CCR IDs and file:// file paths!).\n\
          * Remote Session Control: If the user asks you (e.g., via Telegram or Discord) to execute a command, answer an approval prompt, or run a query in their TUI/CLI session, invoke the 'send_remote_input' tool to forward the prompt directly to that session (e.g., 'cli:direct').\n\
          * Specialized Subagents: You can spawn concurrent subagents (available subagent tools: {}) to delegate tasks.\n\
          * Stateful SOP Workflow Engine: DAG-based template executions (like 'ship-pr-until-green' closed-loop healing, PR creation, CI verification) with Zenflow checkpointed transactions and auto-rollback.\n\
          * Compiler Auto-Healing: 'CompilerAutoHealTool' compiles code natively, reads compiler output, and prompts you to fix syntax or borrow checker issues in a loop until green.\n\
          * Security Guard & BPF Sandbox: Subprocesses are sandboxed using a Linux seccomp BPF filter to block dangerous commands, with strict/normal/loose levels.\n\
          * Cryptographic Audit Ledger: Uses SHA-256 Merkle chain hashing on all session messages/states, verified on boot, with a '/audit' slash command.\n\
          * Proactive Memory & Knowledge Graph: Three memory layers. (1) Simple KV: 'store_memory'/'recall_memory'/'clear_memory' for quick facts. (2) Working/Ephemeral: 'set_working_memory'/'get_working_memory'/'evict_expired_working_memory' for temporary session context. (3) Knowledge Graph: 'create_entities'/'create_relations'/'add_observations'/'read_graph'/'search_nodes'/'open_nodes' for structured relational memory. Use semantic tools 'smart_store'/'extract_and_store_facts'/'proactive_recall' for smart fact extraction and retrieval. Don't ask for permission — just store it.\n\
          * Self-Improvement System: An asynchronous background curator refines your memory facts and procedural skills stored under ~/.openz/skills/ and SQLite database (~/.openz/memory.db).",
        get_version_history(),
        get_dynamic_tools_guideline(&loop_ref.tools),
        subagents_list
    );

    let mut activity_part = String::new();
    if let Some(activity) = crate::agent::activity::get_activity() {
        if activity.session_id != ctx.session_key {
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

    let caveman_rules = if config.agents.defaults.caveman_mode {
        "\n\nRespond terse like smart caveman. All technical substance stay. Only fluff die.\nRules:\n- Drop: articles (a/an/the), filler (just/really/basically), pleasantries, hedging\n- Fragments OK. Short synonyms. Technical terms exact. Code unchanged.\n- Pattern: [thing] [action] [reason]. [next step].\n- Not: \"Sure! I'd be happy to help you with that.\"\n- Yes: \"Bug in auth middleware. Fix:\""
    } else {
        ""
    };

    let pinned_memory = retrieve_pinned_identity_memories().await;
    let recent_session_part = recent_session_context(&ctx.session.messages, 2000);
    let brief_context = retrieve_research_brief_context(ctx.user_content).await;
    let source_context = retrieve_source_context(ctx.user_content).await;
    let workflow_context = retrieve_workflow_context(ctx.user_content).await;
    let weak_model_rules = weak_model_operating_rules(&config.agents.defaults.model);
    let mut cross_session_memory = retrieve_cross_session_memories(ctx.user_content).await;

    // Calculate total character limit and base length
    let budget_limit = 32000;

    let header = format!(
        "You are {}, a helpful assistant. Current date and time: {}. Keep replies clear, precise, and concise.",
        config.agents.defaults.bot_name,
        chrono::Utc::now().to_rfc3339()
    );

    let base_len = header.chars().count()
        + system_guidelines.chars().count()
        + activity_part.chars().count()
        + summary_part.chars().count()
        + memory_part.chars().count()
        + pinned_memory.chars().count()
        + recent_session_part.chars().count()
        + brief_context.chars().count()
        + source_context.chars().count()
        + workflow_context.chars().count()
        + weak_model_rules.chars().count()
        + vision_instruction.chars().count()
        + caveman_rules.chars().count();

    let total_len = base_len + skills_part.chars().count() + cross_session_memory.chars().count();

    if total_len > budget_limit {
        let budget = budget_limit.saturating_sub(base_len);
        let half_budget = budget / 2;
        let skills_len = skills_part.chars().count();
        let cross_len = cross_session_memory.chars().count();

        let (new_skills_budget, new_cross_budget) = if skills_len <= half_budget {
            (skills_len, budget.saturating_sub(skills_len))
        } else if cross_len <= half_budget {
            (budget.saturating_sub(cross_len), cross_len)
        } else {
            (half_budget, budget.saturating_sub(half_budget))
        };

        fn safe_truncate(s: &str, max_chars: usize) -> String {
            let char_count = s.chars().count();
            if char_count <= max_chars {
                s.to_string()
            } else {
                let suffix = "\n... [truncated]";
                let suffix_len = suffix.chars().count();
                if max_chars > suffix_len {
                    let mut truncated: String = s.chars().take(max_chars - suffix_len).collect();
                    truncated.push_str(suffix);
                    truncated
                } else {
                    s.chars().take(max_chars).collect()
                }
            }
        }

        skills_part = safe_truncate(&skills_part, new_skills_budget);
        cross_session_memory = safe_truncate(&cross_session_memory, new_cross_budget);
    }

    ctx.system_prompt = format!(
        "{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}",
        header,
        system_guidelines,
        activity_part,
        summary_part,
        memory_part,
        pinned_memory,
        recent_session_part,
        brief_context,
        source_context,
        workflow_context,
        weak_model_rules,
        cross_session_memory,
        vision_instruction,
        skills_part,
        caveman_rules
    );
    ctx.messages = ctx.session.messages.clone();
    Ok(TurnState::Run)
}

fn recent_session_context(messages: &[crate::session::Message], max_chars: usize) -> String {
    let mut lines: Vec<String> = messages
        .iter()
        .rev()
        .filter(|msg| msg.role == "user" || msg.role == "assistant")
        .take(6)
        .map(|msg| {
            let content = msg.content.split_whitespace().collect::<Vec<_>>().join(" ");
            let label = if msg.role == "user" {
                "User"
            } else {
                "Assistant"
            };
            format!("- {}: {}", label, content)
        })
        .collect();
    lines.reverse();
    if lines.is_empty() {
        return String::new();
    }
    let mut out = String::from("\n\n[Recent Session Context]\nUse these latest turns before claiming there is no current-session context:\n");
    for line in lines {
        if out.chars().count() + line.chars().count() + 1 > max_chars {
            break;
        }
        out.push_str(&line);
        out.push('\n');
    }
    out
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
        "what are you called",
        "persona",
        "personality",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
}

fn identity_memory_candidate(text: &str) -> bool {
    let normalized = text.to_lowercase();
    [
        "name",
        "called",
        "aswin",
        "mivi",
        "persona",
        "personality",
        "friend",
        "preference",
        "prefers",
        "wants",
        "likes",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
}

fn is_weak_or_risky_model(model: &str) -> bool {
    let m = model.to_lowercase();
    if m == "deepseek-v4-flash-free" {
        return false;
    }
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
    .any(|needle| m.contains(needle))
}

fn weak_model_operating_rules(model: &str) -> String {
    if !is_weak_or_risky_model(model) {
        return String::new();
    }
    "\n\n[Small Model Operating Rules]\n- Use [Recent Session Context] before saying you do not know what this session discussed.\n- Use [Pinned Memory] before answering identity, persona, or preference questions.\n- For tool-heavy tasks, keep steps short and prefer exact tool schemas over guessing.\n- For broad tasks, research first, then make an implementation plan and todo list before editing.\n".to_string()
}

fn is_current_or_latest_query(text: &str) -> bool {
    let lower = text.to_lowercase();
    [
        "latest",
        "current",
        "today",
        "now",
        "new",
        "recent",
        "2026",
        "price",
        "version",
        "news",
        "what's new",
        "whats new",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn format_research_brief_context_items(
    items: &[crate::tools::shared_memory::ResearchBrief],
    user_content: &str,
) -> String {
    if items.is_empty() {
        return String::new();
    }
    let current_sensitive = is_current_or_latest_query(user_content);
    let mut out = String::from("\n\n[Relevant Research Briefs]\nUse these saved briefs first for simple definition/comparison questions. Do not call web/search tools when a fresh brief answers the question. Only refresh if freshness=stale or the user asks for latest/current data:\n");
    if current_sensitive {
        out.push_str("- Current/latest intent detected: verify against saved sources or web before final answer.\n");
    } else {
        out.push_str("- Current/latest intent not detected: prefer answering from fresh briefs/sources without extra fetching.\n");
    }
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
        if out.chars().count() + line.chars().count() > 3600 {
            break;
        }
        out.push_str(&line);
    }
    out
}

async fn retrieve_research_brief_context(user_content: &str) -> String {
    match crate::tools::shared_memory::search_research_briefs(user_content, 3).await {
        Ok(items) => {
            if let Some(best) = items.first().filter(|item| item.score >= MIN_MATCH_SCORE) {
                crate::channels::cli::send_notification(&format!(
                    "◇ Research brief matched: {} ({})",
                    best.topic, best.freshness
                ));
            }
            format_research_brief_context_items(&items, user_content)
        }
        Err(err) => {
            tracing::debug!(error = ?err, "research brief context retrieval skipped");
            String::new()
        }
    }
}

fn format_source_context_items(items: &[crate::tools::shared_memory::SourceBookmark]) -> String {
    if items.is_empty() {
        return String::new();
    }
    let mut out = String::from("

[Relevant Saved Sources]
Use these ranked links, repos, docs, paths, or profiles before broad searching. If freshness=stale or the user asks for latest/current data, refresh the specific saved source first and then answer:
");
    for item in items.iter().take(4) {
        let line = format!(
            "- {} [{}] {} | freshness={} ttl={}s trust={:.2} score={:.2} | aliases: {} | summary: {}
",
            item.label,
            item.kind,
            item.uri,
            item.freshness,
            item.stale_after_secs,
            item.trust_score,
            item.score,
            item.aliases.join(", "),
            item.summary
        );
        if out.chars().count() + line.chars().count() > 3000 {
            break;
        }
        out.push_str(&line);
    }
    out
}

async fn retrieve_source_context(user_content: &str) -> String {
    match crate::tools::shared_memory::search_source_bookmarks(user_content, 4).await {
        Ok(items) => {
            if let Some(best) = items.first().filter(|item| item.score >= MIN_MATCH_SCORE) {
                crate::channels::cli::send_notification(&format!(
                    "◇ Sources matched: {} ({})",
                    best.label, best.freshness
                ));
            }
            format_source_context_items(&items)
        }
        Err(err) => {
            tracing::debug!(error = ?err, "source context retrieval skipped");
            String::new()
        }
    }
}

fn summarize_workflow_steps(steps: &serde_json::Value) -> String {
    let Some(arr) = steps.as_array() else {
        return String::new();
    };
    arr.iter()
        .take(5)
        .enumerate()
        .map(|(idx, step)| {
            if let Some(obj) = step.as_object() {
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
                if note.is_empty() {
                    format!("{}. {}", idx + 1, tool)
                } else {
                    format!("{}. {} ({})", idx + 1, tool, note)
                }
            } else {
                format!("{}. {}", idx + 1, step)
            }
        })
        .collect::<Vec<_>>()
        .join("; ")
}

fn format_workflow_context_items(items: &[crate::tools::shared_memory::WorkflowCard]) -> String {
    if items.is_empty() {
        return String::new();
    }
    let mut out = String::from("\n\n[Relevant Reusable Workflows]\nUse these matched procedures automatically for similar tasks instead of rediscovering steps. Follow preconditions, execute the steps, verify the result, then record success/failure with workflow_memory.record_run. For high-risk workflows, request approval before execution:\n");
    for item in items.iter().take(3) {
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
        if out.chars().count() + line.chars().count() > 3600 {
            break;
        }
        out.push_str(&line);
    }
    out
}

async fn retrieve_workflow_context(user_content: &str) -> String {
    match crate::tools::shared_memory::search_workflow_cards(user_content, 3, true).await {
        Ok(items) => {
            if let Some(best) = items.first().filter(|item| item.score >= 4.0) {
                crate::channels::cli::send_notification(&format!(
                    "◇ Workflow matched: {}",
                    best.name
                ));
            }
            format_workflow_context_items(&items)
        }
        Err(err) => {
            tracing::debug!(error = ?err, "workflow context retrieval skipped");
            String::new()
        }
    }
}

async fn retrieve_pinned_identity_memories() -> String {
    let mut entries: Vec<String> = Vec::new();

    let _lock = crate::tools::shared_memory::get_db_mutex().lock().await;
    let cognitive_rows = crate::tools::shared_memory::with_db(|conn| {
        let mut stmt = conn.prepare(
            "SELECT text FROM cognitive_memory WHERE importance >= 0.85 ORDER BY importance DESC, last_accessed DESC LIMIT 24",
        )?;
        let mapped = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut collected = Vec::new();
        for item in mapped {
            collected.push(item?);
        }
        Ok(collected)
    });
    if let Ok(rows) = cognitive_rows {
        entries.extend(
            rows.into_iter()
                .filter(|text| identity_memory_candidate(text)),
        );
    }
    drop(_lock);

    let semantic_rows = crate::tools::graph_memory::with_db(|conn| {
        let mut stmt = conn.prepare(
            "SELECT raw_text FROM semantic_metadata WHERE valid_until IS NULL AND importance >= 0.85 ORDER BY timestamp DESC LIMIT 48",
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
            .filter(|text| identity_memory_candidate(text)),
    );

    let mut seen = std::collections::HashSet::new();
    let mut out = String::new();
    for entry in entries {
        let normalized = entry.trim().to_lowercase();
        if normalized.is_empty() || !seen.insert(normalized) {
            continue;
        }
        if out.is_empty() {
            out.push_str("\n\n[Pinned Memory]\nUse these stable identity/persona/preference facts before guessing:\n");
        }
        let line = format!("- {}\n", entry.trim());
        if out.chars().count() + line.chars().count() > 1500 {
            break;
        }
        out.push_str(&line);
    }
    out
}

fn memory_query_terms(text: &str) -> std::collections::HashSet<String> {
    const STOP_WORDS: &[&str] = &[
        "about", "after", "again", "all", "and", "any", "are", "but", "can", "did", "does", "for",
        "from", "has", "have", "help", "how", "into", "just", "more", "now", "our", "that", "the",
        "their", "then", "there", "these", "this", "those", "too", "use", "user", "was", "were",
        "what", "when", "where", "which", "with", "you", "your",
    ];
    text.split(|c: char| !c.is_alphanumeric())
        .map(|t| t.to_lowercase())
        .filter(|t| t.len() >= 3 && !STOP_WORDS.contains(&t.as_str()))
        .collect()
}

fn memory_relevance_score(query_terms: &std::collections::HashSet<String>, text: &str) -> f32 {
    if query_terms.is_empty() {
        return 1.0;
    }
    let text_terms = memory_query_terms(text);
    if text_terms.is_empty() {
        return 0.0;
    }
    let overlap = query_terms.intersection(&text_terms).count();
    if overlap == 0 {
        0.0
    } else {
        overlap as f32 / query_terms.len().max(1) as f32
    }
}

async fn retrieve_cross_session_memories(user_content: &str) -> String {
    let query_terms = memory_query_terms(user_content);
    let wants_identity = is_identity_or_persona_query(user_content);
    let mut all_entries: Vec<(f32, String)> = Vec::new();
    let uid = "*";
    let aid = "*";

    // 1. Query cognitive_memory (from store_memory tool) — top by importance × recency
    let _lock = crate::tools::shared_memory::get_db_mutex().lock().await;
    let cognitive_rows = crate::tools::shared_memory::with_db(|conn| {
        let mut stmt = conn.prepare(
            "SELECT text, importance, last_accessed, decay_rate FROM cognitive_memory ORDER BY importance DESC, last_accessed DESC LIMIT 8"
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

    if let Ok(rows) = cognitive_rows {
        for row in rows {
            let (text, importance, last_acc, decay_rate) = row;
            let days_elapsed = chrono::Utc::now()
                .signed_duration_since(
                    chrono::DateTime::parse_from_rfc3339(&last_acc)
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .unwrap_or_else(|_| chrono::Utc::now()),
                )
                .num_seconds() as f32
                / 86400.0;
            let mut relevance = memory_relevance_score(&query_terms, &text);
            if relevance == 0.0 && wants_identity && identity_memory_candidate(&text) {
                relevance = 1.0;
            }
            if relevance > 0.0 {
                let score = importance * (-decay_rate * days_elapsed).exp() * relevance;
                all_entries.push((score, text));
            }
        }
    }
    drop(_lock);

    // 2. Query semantic_metadata (curator facts)
    let semantic_facts = crate::tools::graph_memory::with_db(|conn| {
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

    // Score semantic facts just below cognitive ones, gated by current query relevance.
    for fact in &semantic_facts {
        let mut relevance = memory_relevance_score(&query_terms, fact);
        if relevance == 0.0 && wants_identity && identity_memory_candidate(fact) {
            relevance = 1.0;
        }
        if relevance > 0.0 {
            all_entries.push((0.7 * relevance, fact.clone()));
        }
    }

    // 3. Query graph_nodes (entity observations)
    let graph_nodes = crate::tools::graph_memory::with_db(|conn| {
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

    // 4. Sort by score descending
    all_entries.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    // 5. Dedup by normalized text
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
        if norm.is_empty() || seen.contains(&norm) {
            continue;
        }
        // Check if norm is a substring of any already-seen entry (or vice versa)
        let is_duplicate = seen
            .iter()
            .any(|s: &String| s.contains(&norm) || norm.contains(s));
        if !is_duplicate {
            seen.insert(norm);
            deduped.push(text.clone());
        }
    }

    // 6. Build output — top 30 scored entries, no char truncation
    let mut persistent_mem = String::new();
    if deduped.is_empty() && graph_nodes.is_empty() {
        return persistent_mem;
    }

    persistent_mem.push_str(
        "\n\nHere are persistent key facts, decisions, and context across all past sessions:\n",
    );
    for fact in deduped.iter().take(30) {
        persistent_mem.push_str(&format!("- {}\n", fact));
    }

    // Append graph nodes
    let filtered_nodes: Vec<_> = graph_nodes
        .into_iter()
        .filter(|(name, obs)| {
            if obs.is_empty() {
                return false;
            }
            let joined = format!("{} {}", name, obs.join(" "));
            memory_relevance_score(&query_terms, &joined) > 0.0
        })
        .collect();
    for (name, obs) in filtered_nodes.iter().take(5) {
        persistent_mem.push_str(&format!("- Entity '{}':\n", name));
        for ob in obs.iter().take(10) {
            persistent_mem.push_str(&format!("  * {}\n", ob));
        }
    }

    persistent_mem
}

const CHANGELOG_CONTENT: &str = include_str!("../../../CHANGELOG.md");

fn get_version_history() -> String {
    let mut history = String::new();
    let mut recording = false;
    let mut header_count = 0;

    for line in CHANGELOG_CONTENT.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("### v") {
            header_count += 1;
            if header_count > 5 {
                break;
            }
            recording = true;
        }
        if recording {
            history.push_str(line);
            history.push('\n');
        }
    }
    history.trim().to_string()
}

fn get_dynamic_tools_guideline(registry: &crate::tools::ToolRegistry) -> String {
    let tools = registry.get_static_tools();
    let mut core_tools = Vec::new();
    let mut searchxyz_tools = Vec::new();
    let mut openmedia_tools = Vec::new();
    let mut opendoc_tools = Vec::new();
    let mut github_tools = Vec::new();
    let mut docs_tools = Vec::new();

    for t in tools {
        let name = t.name();
        if name.starts_with("searchxyz_") {
            searchxyz_tools.push(format!("'{}'", name));
        } else if name.starts_with("openmedia_") {
            openmedia_tools.push(format!("'{}'", name));
        } else if name.starts_with("opendoc_") {
            opendoc_tools.push(format!("'{}'", name));
        } else if name.starts_with("github_") {
            github_tools.push(format!("'{}'", name));
        } else if name.starts_with("docs_") {
            docs_tools.push(format!("'{}'", name));
        } else {
            core_tools.push(format!("'{}'", name));
        }
    }

    core_tools.sort();
    searchxyz_tools.sort();
    openmedia_tools.sort();
    opendoc_tools.sort();
    github_tools.sort();
    docs_tools.sort();

    let mut out = String::new();
    out.push_str("You have native tools for files, shell execution, and other utilities. The following tools are registered in your environment:\n");
    if !core_tools.is_empty() {
        out.push_str(&format!(
            "            - Core Tools: {}\n",
            core_tools.join(", ")
        ));
    }
    if !searchxyz_tools.is_empty() {
        out.push_str(&format!(
            "            - SearchXyz Tools: {}\n",
            searchxyz_tools.join(", ")
        ));
    }
    if !openmedia_tools.is_empty() {
        out.push_str(&format!(
            "            - OpenMedia Tools: {}\n",
            openmedia_tools.join(", ")
        ));
    }
    if !opendoc_tools.is_empty() {
        out.push_str(&format!(
            "            - OpenDoc Tools: {}\n",
            opendoc_tools.join(", ")
        ));
    }
    if !github_tools.is_empty() {
        out.push_str(&format!(
            "            - GitHub Integration Tools: {}\n",
            github_tools.join(", ")
        ));
    }
    if !docs_tools.is_empty() {
        out.push_str(&format!(
            "            - Local & Crates Docs Tools: {}\n",
            docs_tools.join(", ")
        ));
    }
    out.push_str("            - MCP server integration managed via 'manage_mcp' tool.");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::graph_memory::test_lock;
    use crate::tools::graph_memory::with_db;
    use crate::tools::memory_extra::working::store_semantic_fact;

    #[test]
    fn recent_session_context_prioritizes_latest_user_assistant_turns() {
        let messages = vec![
            crate::session::Message {
                role: "user".to_string(),
                content: "old topic".to_string(),
                timestamp: None,
                extra: serde_json::Map::new(),
            },
            crate::session::Message {
                role: "assistant".to_string(),
                content: "old answer".to_string(),
                timestamp: None,
                extra: serde_json::Map::new(),
            },
            crate::session::Message {
                role: "user".to_string(),
                content: "we are testing model switching".to_string(),
                timestamp: None,
                extra: serde_json::Map::new(),
            },
            crate::session::Message {
                role: "assistant".to_string(),
                content: "listed weak model failures".to_string(),
                timestamp: None,
                extra: serde_json::Map::new(),
            },
        ];
        let block = recent_session_context(&messages, 500);
        assert!(block.contains("testing model switching"));
        assert!(block.contains("weak model failures"));
    }

    #[test]
    fn identity_query_detection_catches_name_and_persona_questions() {
        assert!(is_identity_or_persona_query("what is my name"));
        assert!(is_identity_or_persona_query("who are you"));
        assert!(is_identity_or_persona_query("what is your persona"));
        assert!(!is_identity_or_persona_query("fix the cargo build"));
    }

    #[test]
    fn weak_model_detection_catches_small_or_free_models() {
        assert!(is_weak_or_risky_model("llama-3.1-8b-instant"));
        assert!(is_weak_or_risky_model("mimo-v2.5-free"));
        assert!(is_weak_or_risky_model("gemini-3.1-flash-lite"));
        assert!(!is_weak_or_risky_model("deepseek-v4-flash-free"));
    }

    #[test]
    fn research_brief_context_discourages_unneeded_fetches() {
        let item = crate::tools::shared_memory::ResearchBrief {
            id: "id".to_string(),
            topic: "mem0".to_string(),
            summary: "Mem0 is a memory layer for AI agents.".to_string(),
            source_ids: vec![],
            confidence: 0.8,
            stale_after_secs: 86400,
            freshness: "fresh".to_string(),
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
            use_count: 0,
            score: 9.0,
        };
        let block = format_research_brief_context_items(&[item], "what is mem0");
        assert!(block.contains("Do not call web/search tools"));
        assert!(block.contains("Current/latest intent not detected"));
    }

    #[test]
    fn workflow_context_includes_reusable_steps() {
        let item = crate::tools::shared_memory::WorkflowCard {
            id: "id".to_string(),
            name: "screenshot_to_telegram".to_string(),
            triggers: vec!["send screenshot to telegram".to_string()],
            summary: "Capture active window and send it through Telegram".to_string(),
            steps: serde_json::json!([{ "tool": "exec_command", "note": "capture active window" }]),
            preconditions: vec!["Telegram configured".to_string()],
            verification: vec!["Telegram API ok=true".to_string()],
            risk: "normal".to_string(),
            status: "active".to_string(),
            success_count: 2,
            failure_count: 0,
            last_used: None,
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
            score: 7.5,
        };
        let block = format_workflow_context_items(&[item]);
        assert!(block.contains("steps:"));
        assert!(block.contains("exec_command"));
        assert!(block.contains("workflow_memory.record_run"));
    }

    #[test]
    fn test_get_version_history() {
        let history = get_version_history();
        assert!(!history.is_empty());
        // The latest release heading is always the first recorded block and is
        // guaranteed to match CARGO_PKG_VERSION by version_sync_tests.
        assert!(history.contains(&format!("v{}", env!("CARGO_PKG_VERSION"))));
    }

    #[test]
    fn test_get_dynamic_tools_guideline() {
        let registry = crate::tools::ToolRegistry::new();
        struct DummyTool;
        #[async_trait::async_trait]
        impl crate::tools::Tool for DummyTool {
            fn name(&self) -> &str {
                "dummy_tool"
            }
            fn description(&self) -> &str {
                "dummy"
            }
            fn parameters(&self) -> serde_json::Value {
                serde_json::json!({})
            }
            async fn call(&self, _args: &serde_json::Value) -> Result<serde_json::Value> {
                Ok(serde_json::json!({}))
            }
        }
        registry.register(std::sync::Arc::new(DummyTool));
        let guideline = get_dynamic_tools_guideline(&registry);
        assert!(guideline.contains("dummy_tool"));
        assert!(guideline.contains("Core Tools"));
    }

    #[tokio::test]
    async fn test_cross_session_memory_excludes_stale_semantic_facts() {
        let _l = test_lock().lock().await;
        let scope = format!(
            "test_prompt_stale_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let stale_fact = format!("stale-marker-{} Rust obsolete rule", scope);
        let active_fact = format!("active-marker-{} Rust current rule", scope);

        store_semantic_fact(
            &format!("{}-stale", scope),
            &stale_fact,
            0.9,
            "*",
            &scope,
            "*",
        )
        .unwrap();
        store_semantic_fact(
            &format!("{}-active", scope),
            &active_fact,
            0.9,
            "*",
            &scope,
            "*",
        )
        .unwrap();
        with_db(|conn| {
            conn.execute(
                "UPDATE semantic_metadata SET valid_until = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE node_id = ?1",
                rusqlite::params![format!("{}-stale", scope)],
            )?;
            Ok(())
        })
        .unwrap();

        let memory = retrieve_cross_session_memories("Rust current obsolete rule").await;
        assert!(memory.contains(&active_fact));
        assert!(!memory.contains(&stale_fact));
    }

    #[tokio::test]
    async fn test_cross_session_memory_is_top_k_budgeted() {
        let _l = test_lock().lock().await;
        let scope = format!(
            "test_prompt_budget_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );

        for i in 0..35 {
            store_semantic_fact(
                &format!("{}-fact-{}", scope, i),
                &format!("budget-marker-{} Rust memory fact number {}", scope, i),
                0.9,
                "*",
                &scope,
                "*",
            )
            .unwrap();
        }

        let memory = retrieve_cross_session_memories("Rust memory budget marker").await;
        let fact_lines = memory
            .lines()
            .filter(|line| line.starts_with("- budget-marker-"))
            .count();
        assert!(
            fact_lines <= 30,
            "prompt memory should stay top-30 budgeted"
        );
    }

    #[tokio::test]
    async fn test_cross_session_memory_is_query_relevant() {
        let _l = test_lock().lock().await;
        let scope = format!(
            "test_prompt_memory_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let rust_fact = format!("rust-query-marker-{} borrow checker lifetime rule", scope);
        let cooking_fact = format!(
            "cooking-query-marker-{} sourdough fermentation schedule",
            scope
        );

        store_semantic_fact(
            &format!("{}-rust", scope),
            &rust_fact,
            0.9,
            "*",
            &scope,
            "*",
        )
        .unwrap();
        store_semantic_fact(
            &format!("{}-cooking", scope),
            &cooking_fact,
            0.9,
            "*",
            &scope,
            "*",
        )
        .unwrap();

        let memory =
            retrieve_cross_session_memories("help with Rust lifetime borrow checker").await;

        assert!(memory.contains(&rust_fact));
        assert!(
            !memory.contains(&cooking_fact),
            "irrelevant memory should not be injected into every prompt"
        );

        with_db(|conn| {
            conn.execute(
                "UPDATE semantic_metadata SET valid_until = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE session_id = ?1",
                rusqlite::params![scope],
            )?;
            Ok(())
        })
        .unwrap();
    }
}
