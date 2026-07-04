use anyhow::Result;
use super::{AgentLoop, TurnContext, TurnState};

pub async fn handle(loop_ref: &AgentLoop, ctx: &mut TurnContext<'_>) -> Result<TurnState> {
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
    if !crate::providers::model_supports_vision(&loop_ref.config.agents.defaults.model) {
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
          * Specifications & Changelog: ROM ~10-15MB, RAM ~15-30MB cloud / ~200MB+ local, <5ms startup. Version history: [v0.0.32] Native OpenDoc tool port and searchxyz modularization. [v0.0.31] Offline OpenMedia-RS native integration. [v0.0.30] Integrated SearchXyz tool suite. [v0.0.28] Codebase modularization (split CLI, agent_loop, headroom, memory_extra, shared_memory, sequential_thinking, graph_memory monoliths into package submodules). [v0.0.27] Cross-session memory persistence, 67 MCP-to-native tool port, dual SQLite fix. [v0.0.26] Repo awareness, monologue formatting, color system. [v0.0.25] Live log visualizer, execution tracing. [v0.0.24] gRPC bridge panic fix, retry loop. [v0.0.23] DeepSeek/V4 param fix, streaming toggle. [v0.0.22] MCP health monitoring, native git_provider, SSRF hardening. [v0.0.21] Tool timeouts, graceful shutdown, file locking, streaming. [v0.0.20] Default-deny gateway auth, SSRF/IPv6, SQL injection elim. [v0.0.19] Subagent loopback isolation. [v0.0.18] Discord fix, raw-mode helpers, regex caching. [v0.0.17] MCP cache consolidation, TOCTOU fix, bridge monitoring. [v0.0.16] DNS rebinding defense, port scanner restriction, cmd injection fix, file size limits, blocking I/O fixes. [v0.0.15] SQL injection defense, SSRF, WhatsApp webhook, CORS, browser flags, UTF-8 panics, disk usage. [v0.0.14] Incremental session saving, history rendering. [v0.0.13] Log layer fixes, path resolution, tail default. [v0.0.12] Image/video generation, CDP compat, SVG animator, log streaming. [v0.0.11] Changelog command, curator throttling, cloud embeddings, auto-heal. [v0.0.9] Merkle audit ledger, WhatsApp channel, auto-continuation. [v0.0.8] Email & Discord channels. [v0.0.7] Telegram bot, WebSocket gateway. [v0.0.1-v0.0.6] Core foundation.

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
         * Local Tools & MCP: You have native tools for file reading/writing, codebase text search ('grep_search'), file code structure parsing ('code_outline'), git operations ('git_manager'), database inspection ('db_inspector'), cargo toolchain execution ('cargo_manager'), system clipboard access ('clipboard'), opening files/folders/URLs ('open_path'), background file change watching ('file_watcher'), structural code search ('ast_grep'), real browser automation ('gsd_browser'), web search queries ('web_search'), integrated search/crawling/indexing engine ('searchxyz_search_web', 'searchxyz_read_url', 'searchxyz_search_and_read', 'searchxyz_recall', 'searchxyz_list_sources', 'searchxyz_deep_research', 'searchxyz_index_content', 'searchxyz_site_map', 'searchxyz_index_relationship', 'searchxyz_query_graph', 'searchxyz_read_github_repo', 'searchxyz_export_research', 'searchxyz_import_research', 'searchxyz_delete_source', 'searchxyz_clear_index'), shell command execution, web fetching, remote control forwarding, document reading ('read_doc'), sandboxed WASM execution ('wasm_execute'), project template/package scaffolding ('onpkg'), sequential thinking ('sequentialthinking', 'analyze_graph', 'summarize_reasoning'), knowledge graph memory ('create_entities', 'create_relations', 'add_observations', 'read_graph', 'search_nodes', 'open_nodes'), context scoping/compression ('scope_context', 'compress_content', 'retrieve_original', 'compress_schema', 'compress_file', 'compress_diff', 'compress_directory', 'run_and_compress', 'compress_url', 'summarize_codebase'), working/ephemeral memory ('set_working_memory', 'get_working_memory'), smart semantic memory ('smart_store', 'extract_and_store_facts', 'proactive_recall', 'invalidate_fact', 'query_fact_history'), shared team memory ('store_shared_team_memory', 'retrieve_shared_team_memory'), and offline media tools ('openmedia_ping', 'openmedia_model_download', 'openmedia_rasterize_svg', 'openmedia_diagram_generate_mermaid', 'openmedia_html_to_image', 'openmedia_create_svg', 'openmedia_create_chart', 'openmedia_create_icon', 'openmedia_animate_svg', 'openmedia_animate_create_timeline', 'openmedia_animate_morph_paths', 'openmedia_animate_generate_spinner', 'openmedia_animate_from_lottie', 'openmedia_animate_to_lottie', 'openmedia_image_apply_filter', 'openmedia_image_resize', 'openmedia_image_crop', 'openmedia_image_transform', 'openmedia_image_convert', 'openmedia_image_batch_process', 'openmedia_video_create', 'openmedia_video_preview', 'openmedia_video_create_slideshow', 'openmedia_video_add_transition', 'openmedia_video_add_audio', 'openmedia_video_from_template', 'openmedia_video_extract_frames', 'openmedia_video_trim', 'openmedia_template_create', 'openmedia_template_read', 'openmedia_template_update', 'openmedia_template_delete', 'openmedia_improve_score_image', 'openmedia_improve_refine_prompt', 'openmedia_improve_auto_refine', 'openmedia_improve_feedback', 'openmedia_improve_quality_report'), and native document intelligence tools ('opendoc_open_document', 'opendoc_read_document_text', 'opendoc_search_document', 'opendoc_replace_text', 'opendoc_diff_documents', 'opendoc_diff_documents_visual', 'opendoc_chunk_for_embedding', 'opendoc_fill_template', 'opendoc_validate_document', 'opendoc_validate_pdf_a_compliance', 'opendoc_extract_structured_metadata', 'opendoc_convert', 'opendoc_extract_images', 'opendoc_split_pdf', 'opendoc_create_html', 'opendoc_batch_convert', 'opendoc_create_docx', 'opendoc_docx_add_paragraph', 'opendoc_docx_add_table', 'opendoc_docx_add_image', 'opendoc_create_pptx', 'opendoc_pptx_add_slide', 'opendoc_create_xlsx', 'opendoc_edit_xlsx', 'opendoc_create_pdf', 'opendoc_create_formatted_pdf', 'opendoc_merge_pdfs', 'opendoc_extract_pdf_text', 'opendoc_list_pdf_fields', 'opendoc_fill_pdf_form', 'opendoc_find_tables', 'opendoc_analyze_document_complexity', 'opendoc_ocr_document', 'opendoc_check_ocr_available', 'opendoc_render_document_pages', 'opendoc_extract_archive_digest'), and native GitHub integration tools ('github_create_pull_request', 'github_search_issues', 'github_get_issue_comments'), and native local and crates documentation tools ('docs_list_docsets', 'docs_install_docset', 'docs_search_docs', 'docs_read_doc_page', 'docs_search_rust_crate', 'docs_read_rust_docs'). MCP server integration managed via 'manage_mcp' tool.
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

    let caveman_rules = if loop_ref.config.agents.defaults.caveman_mode {
        "\n\nRespond terse like smart caveman. All technical substance stay. Only fluff die.\nRules:\n- Drop: articles (a/an/the), filler (just/really/basically), pleasantries, hedging\n- Fragments OK. Short synonyms. Technical terms exact. Code unchanged.\n- Pattern: [thing] [action] [reason]. [next step].\n- Not: \"Sure! I'd be happy to help you with that.\"\n- Yes: \"Bug in auth middleware. Fix:\""
    } else {
        ""
    };

    let mut cross_session_memory = retrieve_cross_session_memories(ctx.user_content).await;

    // Calculate total character limit and base length
    let budget_limit = 32000;

    let header = format!(
        "You are {}, a helpful assistant. Current date and time: {}. Keep replies clear, precise, and concise.",
        loop_ref.config.agents.defaults.bot_name,
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
