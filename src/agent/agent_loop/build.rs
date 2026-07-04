use anyhow::Result;
use super::{AgentLoop, TurnContext, TurnState};

pub async fn handle(loop_ref: &AgentLoop, ctx: &mut TurnContext<'_>) -> Result<TurnState> {
    let config = &ctx.config;
    let mut summary_part = String::new();
    if let Some(summary) = ctx.session.metadata.get("summary").and_then(|v| v.as_str()) {
        if !summary.is_empty() {
            summary_part = format!("\n\nHere is a summary of the earlier part of the conversation:\n{}\n", summary);
        }
    }
    let mut memory_part = String::new();
    if let Some(memory) = ctx.session.metadata.get("memory").and_then(|v| v.as_str()) {
        if !memory.is_empty() {
            memory_part = format!("\n\nHere is the long-term memory of key facts, preferences, and decisions from this session:\n{}\n", memory);
        }
    }
    let mut profile_name = None;
    let parts: Vec<&str> = ctx.session_key.split(':').collect();
    if parts.len() >= 2 && parts[0] == "subagent" {
        profile_name = Some(parts[1]);
    }

    let mut skills_part = String::new();
    if let Ok(skills) = crate::agent::skills::load_relevant_skills_with_profile(ctx.user_content, &ctx.session.messages, profile_name) {
        if !skills.is_empty() {
            skills_part = "\n\nHere are the active guidelines and procedural skills you should follow:\n".to_string();
            for skill in skills {
                skills_part.push_str(&format!("=== Skill: {} ===\n{}\n\n", skill.name, skill.content));
            }
        }
    }
    let mut vision_instruction = "";
    if !crate::providers::model_supports_vision(&config.agents.defaults.model) {
        vision_instruction = " If a message contains a markdown image link (e.g. ![](file://...)) and you need to analyze or describe the image, you MUST delegate the visual analysis task to the specialized 'vision_agent' tool (or the 'delegate_task' tool) to see and report on the image contents.";
    }
    let subagents_list = if let Ok(profiles) = crate::subagents::load_profiles() {
        profiles.iter().map(|p| p.name.clone()).collect::<Vec<String>>().join(", ")
    } else {
        "planner, researcher, debugger, DevOps, skill_improvement, openz_maintainer, mcps_manager".to_string()
    };
    let system_guidelines = format!(
        "\n\nYou are OpenZ, a high-performance personal AI agent framework built in Rust, vibe-coded by Aswin. Your official GitHub repository and source code resides at https://github.com/aswin402/openz-rs. You are inspired by Zeroclaw, Nanobot, hermes-agent, loops!, and DOX. Your architecture is structured as follows:\n\
         * Creator & Inspiration: Vibe-coded by Aswin. Inspired by Zeroclaw, Nanobot, hermes-agent, loops!, and DOX. Official Repository: https://github.com/aswin402/openz-rs\n\
          * Specifications & Changelog: ROM ~10-15MB, RAM ~15-30MB cloud / ~200MB+ local, <5ms startup. Version history:\n{}\n\
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
        + vision_instruction.chars().count()
        + caveman_rules.chars().count();

    let total_len = base_len
        + skills_part.chars().count()
        + cross_session_memory.chars().count();

    if total_len > budget_limit {
        let budget = if budget_limit > base_len { budget_limit - base_len } else { 0 };
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
        "{}{}{}{}{}{}{}{}{}",
        header,
        system_guidelines,
        activity_part,
        summary_part,
        memory_part,
        cross_session_memory,
        vision_instruction,
        skills_part,
        caveman_rules
    );
    ctx.messages = ctx.session.messages.clone();
    Ok(TurnState::Run)
}

async fn retrieve_cross_session_memories(_user_content: &str) -> String {
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
                        .unwrap_or_else(|_| chrono::Utc::now())
                )
                .num_seconds() as f32 / 86400.0;
            let score = importance * (-decay_rate * days_elapsed).exp();
            all_entries.push((score, text));
        }
    }
    drop(_lock);

    // 2. Query semantic_metadata (curator facts)
    let semantic_facts = crate::tools::graph_memory::with_db(|conn| {
        let mut stmt = conn.prepare(
            "SELECT raw_text FROM semantic_metadata 
             WHERE valid_until IS NULL 
               AND (user_id = ?1 OR user_id = '*')
               AND (agent_id = ?2 OR agent_id = '*')
             ORDER BY timestamp DESC LIMIT 100"
        ).map_err(|e| anyhow::anyhow!(e))?;
        let mut rows = stmt.query(rusqlite::params![uid, aid]).map_err(|e| anyhow::anyhow!(e))?;
        let mut facts = Vec::new();
        while let Some(row) = rows.next().map_err(|e| anyhow::anyhow!(e))? {
            let text: String = row.get(0).map_err(|e| anyhow::anyhow!(e))?;
            facts.push(text);
        }
        Ok(facts)
    }).unwrap_or_default();

    // Score semantic facts just below cognitive ones (base score 0.7, aged)
    for fact in &semantic_facts {
        all_entries.push((0.7, fact.clone()));
    }

    // 3. Query graph_nodes (entity observations)
    let graph_nodes = crate::tools::graph_memory::with_db(|conn| {
        let mut stmt = conn.prepare(
            "SELECT name, observations FROM graph_nodes 
             WHERE (user_id = ?1 OR user_id = '*')
               AND (agent_id = ?2 OR agent_id = '*')
             LIMIT 100"
        ).map_err(|e| anyhow::anyhow!(e))?;
        let mut rows = stmt.query(rusqlite::params![uid, aid]).map_err(|e| anyhow::anyhow!(e))?;
        let mut nodes = Vec::new();
        while let Some(row) = rows.next().map_err(|e| anyhow::anyhow!(e))? {
            let name: String = row.get(0).map_err(|e| anyhow::anyhow!(e))?;
            let obs_json: String = row.get(1).map_err(|e| anyhow::anyhow!(e))?;
            let obs: Vec<String> = serde_json::from_str(&obs_json).unwrap_or_default();
            nodes.push((name, obs));
        }
        Ok(nodes)
    }).unwrap_or_default();

    // 4. Sort by score descending
    all_entries.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    // 5. Dedup by normalized text
    fn normalize(s: &str) -> String {
        s.trim()
            .trim_start_matches(|c: char| c == '-' || c == '*' || c == ' ')
            .trim_start_matches("**").trim_end_matches("**")
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
        let is_duplicate = seen.iter().any(|s: &String| {
            s.contains(&norm) || norm.contains(s)
        });
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

    persistent_mem.push_str("\n\nHere are persistent key facts, decisions, and context across all past sessions:\n");
    for fact in deduped.iter().take(30) {
        persistent_mem.push_str(&format!("- {}\n", fact));
    }

    // Append graph nodes
    let filtered_nodes: Vec<_> = graph_nodes.into_iter().filter(|(_, obs)| !obs.is_empty()).collect();
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
        out.push_str(&format!("            - Core Tools: {}\n", core_tools.join(", ")));
    }
    if !searchxyz_tools.is_empty() {
        out.push_str(&format!("            - SearchXyz Tools: {}\n", searchxyz_tools.join(", ")));
    }
    if !openmedia_tools.is_empty() {
        out.push_str(&format!("            - OpenMedia Tools: {}\n", openmedia_tools.join(", ")));
    }
    if !opendoc_tools.is_empty() {
        out.push_str(&format!("            - OpenDoc Tools: {}\n", opendoc_tools.join(", ")));
    }
    if !github_tools.is_empty() {
        out.push_str(&format!("            - GitHub Integration Tools: {}\n", github_tools.join(", ")));
    }
    if !docs_tools.is_empty() {
        out.push_str(&format!("            - Local & Crates Docs Tools: {}\n", docs_tools.join(", ")));
    }
    out.push_str("            - MCP server integration managed via 'manage_mcp' tool.");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_version_history() {
        let history = get_version_history();
        assert!(!history.is_empty());
        assert!(history.contains("v0.0.36"));
    }

    #[test]
    fn test_get_dynamic_tools_guideline() {
        let registry = crate::tools::ToolRegistry::new();
        struct DummyTool;
        #[async_trait::async_trait]
        impl crate::tools::Tool for DummyTool {
            fn name(&self) -> &str { "dummy_tool" }
            fn description(&self) -> &str { "dummy" }
            fn parameters(&self) -> serde_json::Value { serde_json::json!({}) }
            async fn call(&self, _args: &serde_json::Value) -> Result<serde_json::Value> { Ok(serde_json::json!({})) }
        }
        registry.register(std::sync::Arc::new(DummyTool));
        let guideline = get_dynamic_tools_guideline(&registry);
        assert!(guideline.contains("dummy_tool"));
        assert!(guideline.contains("Core Tools"));
    }
}


