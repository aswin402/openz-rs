use super::app::{ChatMessage, ModalState, RatatuiApp};

pub enum SlashResult {
    /// Post assistant message into the chat timeline
    Message(ChatMessage),
    /// Open a modal overlay
    OpenModal(ModalState),
    /// Clear the chat timeline
    ClearTimeline,
    /// Reset active session and start clean
    NewSession,
    /// Exit the application
    Exit,
    /// Not a slash command or unhandled
    Unhandled,
}

pub async fn handle_slash_command(
    trimmed: &str,
    app: &mut RatatuiApp,
    agent_loop: &tokio::sync::Mutex<crate::agent::AgentLoop>,
    session_key: &str,
    session_manager: &crate::session::SessionManager,
    config: &crate::config::schema::Config,
) -> SlashResult {
    if trimmed == "/exit" || trimmed == "exit" || trimmed == "quit" {
        return SlashResult::OpenModal(ModalState::ExitConfirm { selected_yes: true });
    }

    if trimmed == "/clear" {
        return SlashResult::ClearTimeline;
    }

    if trimmed == "/help" || trimmed == "/commands" {
        return SlashResult::OpenModal(ModalState::new_command_catalog());
    }

    if trimmed == "/history" {
        let sessions = crate::cli::load_session_history().unwrap_or_default();
        let items: Vec<(String, String, String)> = sessions
            .into_iter()
            .map(|s| (s.key, s.display_title, s.updated_at.format("%Y-%m-%d %H:%M").to_string()))
            .collect();
        return SlashResult::OpenModal(ModalState::History {
            sessions: items,
            selected_idx: 0,
        });
    }

    if trimmed == "/model" {
        let configured = crate::channels::build_configured_providers(config);
        return SlashResult::OpenModal(ModalState::ProviderSelect {
            providers: configured,
            selected_idx: 0,
        });
    }

    if let Some(model_arg) = trimmed.strip_prefix("/model") {
        let arg = model_arg.trim();
        if !arg.is_empty() {
            let (prov, mdl) = if let Some(idx) = arg.find('/') {
                (&arg[..idx], &arg[idx + 1..])
            } else {
                ("auto", arg)
            };

            match crate::config::loader::load_config() {
                Ok(mut new_config) => {
                    new_config.agents.defaults.provider = prov.to_string();
                    new_config.agents.defaults.model = mdl.to_string();
                    let _ = crate::config::loader::save_config(&new_config);

                    if let Ok(resolved) =
                        crate::providers::resolver::resolve_provider_full(&new_config, mdl)
                    {
                        if let Ok(mut loop_lock) = agent_loop.try_lock() {
                            loop_lock.update_model_and_provider(new_config.clone(), resolved.instance);
                        }
                        app.model = mdl.to_string();
                        app.provider = prov.to_string();
                        return SlashResult::Message(ChatMessage::simple(
                            "assistant",
                            format!("✓ Active model switched to `{}` (provider: `{}`).", mdl, prov),
                        ));
                    } else {
                        return SlashResult::Message(ChatMessage::simple(
                            "assistant",
                            format!("✕ Failed to resolve model `{}` for provider `{}`.", mdl, prov),
                        ));
                    }
                }
                Err(e) => {
                    return SlashResult::Message(ChatMessage::simple(
                        "assistant",
                        format!("✕ Failed to load configuration: {}", e),
                    ));
                }
            }
        }
    }

    if trimmed == "/new-session" {
        return SlashResult::NewSession;
    }

    if trimmed == "/settings" {
        let loop_guard = agent_loop.try_lock();
        let current_cfg = loop_guard.as_ref().map(|l| &l.config).unwrap_or(config);
        let defaults = &current_cfg.agents.defaults;

        let mut lines = Vec::new();
        lines.push("**Active Settings:**".to_string());
        lines.push(format!("• **Model:** `{}`", defaults.model));
        lines.push(format!("• **Provider:** `{}`", defaults.provider));
        lines.push(format!("• **Security Mode:** `{}`", defaults.security_mode));
        lines.push(format!(
            "• **Streaming:** `{}`",
            if defaults.streaming { "Enabled" } else { "Disabled" }
        ));
        lines.push(format!(
            "• **Sandbox (seccomp BPF):** `{}`",
            if defaults.enable_sandbox { "Enabled" } else { "Disabled" }
        ));
        lines.push(format!("• **Caveman Mode:** `{}`", if defaults.caveman_mode { "Enabled" } else { "Disabled" }));
        lines.push(format!("• **Max Messages:** `{}`", defaults.max_messages));
        lines.push(format!("• **Max Tool Iterations:** `{}`", defaults.max_tool_iterations));

        if !defaults.whitelisted_command_prefixes.is_empty() {
            lines.push(format!(
                "• **Whitelisted Command Prefixes:** {}",
                defaults
                    .whitelisted_command_prefixes
                    .iter()
                    .map(|p| format!("`{}`", p))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }

        if !defaults.whitelisted_paths.is_empty() {
            lines.push(format!(
                "• **Whitelisted Paths:** {}",
                defaults
                    .whitelisted_paths
                    .iter()
                    .map(|p| format!("`{}`", p))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }

        return SlashResult::Message(ChatMessage::simple("assistant", lines.join("\n")));
    }

    if trimmed == "/streaming" {
        let current_streaming = session_manager
            .load(session_key)
            .ok()
            .and_then(|session| {
                session
                    .metadata
                    .get("streaming")
                    .and_then(|v| v.as_bool())
            })
            .unwrap_or_else(|| {
                agent_loop
                    .try_lock()
                    .map(|l| l.config.agents.defaults.streaming)
                    .unwrap_or(config.agents.defaults.streaming)
            });
        let next_streaming = !current_streaming;
        let _ = super::session::save_session_streaming_override(
            session_manager,
            session_key,
            next_streaming,
        )
        .await;

        return SlashResult::Message(ChatMessage::simple(
            "assistant",
            format!(
                "Response streaming is now **{}** for this session.",
                if next_streaming { "enabled" } else { "disabled" }
            ),
        ));
    }

    if trimmed == "/servers" {
        let servers = crate::shutdown::list_registered_children();
        if servers.is_empty() {
            return SlashResult::Message(ChatMessage::simple(
                "assistant",
                "No OpenZ-launched background servers running.\n\nUse `/stop-server <id|all>` to stop servers if any are running.".to_string(),
            ));
        }

        let mut lines = Vec::new();
        lines.push("**OpenZ Background Servers:**".to_string());
        for server in servers {
            lines.push(format!(
                "• `#{}` (PID {}) [{}] `{}`",
                server.id, server.pid, server.kind, server.command
            ));
        }
        lines.push("\n*Use `/stop-server <id>` or `/stop-server all` to terminate.*".to_string());
        return SlashResult::Message(ChatMessage::simple("assistant", lines.join("\n")));
    }

    if let Some(stripped) = trimmed.strip_prefix("/stop-server") {
        let target = stripped.trim();
        if target.is_empty() {
            return SlashResult::Message(ChatMessage::simple(
                "assistant",
                "Usage: `/stop-server <id|all>`".to_string(),
            ));
        }

        match crate::shutdown::stop_registered_child(target) {
            Ok(0) => {
                return SlashResult::Message(ChatMessage::simple(
                    "assistant",
                    format!("No matching background server found for `{}`.", target),
                ));
            }
            Ok(count) => {
                return SlashResult::Message(ChatMessage::simple(
                    "assistant",
                    format!("✓ Stopped {} background server(s).", count),
                ));
            }
            Err(e) => {
                return SlashResult::Message(ChatMessage::simple(
                    "assistant",
                    format!("✕ Failed to stop server: {}", e),
                ));
            }
        }
    }

    if let Some(stripped) = trimmed.strip_prefix("/device") {
        let output = handle_device_command_markdown(stripped.trim()).await;
        return SlashResult::Message(ChatMessage::simple("assistant", output));
    }

    if trimmed == "/memory" {
        if let Ok(session) = session_manager.load(session_key) {
            if session.metadata.is_empty() {
                return SlashResult::Message(ChatMessage::simple(
                    "assistant",
                    "No metadata or facts recorded for this session yet.".to_string(),
                ));
            }
            let mut lines = Vec::new();
            lines.push("**Session Metadata & Memory:**".to_string());
            for (k, v) in &session.metadata {
                lines.push(format!("• **{}:** {}", k, v));
            }
            return SlashResult::Message(ChatMessage::simple("assistant", lines.join("\n")));
        } else {
            return SlashResult::Message(ChatMessage::simple(
                "assistant",
                "No active session record found.".to_string(),
            ));
        }
    }

    if trimmed.starts_with("/sources") {
        let query = trimmed.strip_prefix("/sources").unwrap_or("").trim();
        match crate::tools::shared_memory::search_source_bookmarks(query, 10).await {
            Ok(items) if items.is_empty() => {
                return SlashResult::Message(ChatMessage::simple(
                    "assistant",
                    "No saved source bookmarks matched.".to_string(),
                ));
            }
            Ok(items) => {
                let mut lines = Vec::new();
                lines.push("**Saved Sources:**".to_string());
                for item in items {
                    lines.push(format!("• **{}** `[{}]` {}", item.label, item.kind, item.uri));
                    if !item.summary.trim().is_empty() {
                        lines.push(format!("  _{}_", item.summary.trim()));
                    }
                }
                return SlashResult::Message(ChatMessage::simple("assistant", lines.join("\n")));
            }
            Err(e) => {
                return SlashResult::Message(ChatMessage::simple(
                    "assistant",
                    format!("✕ Error searching sources: {}", e),
                ));
            }
        }
    }

    if trimmed.starts_with("/workflows") {
        let query = trimmed.strip_prefix("/workflows").unwrap_or("").trim();
        match crate::tools::shared_memory::search_workflow_cards(query, 10, false).await {
            Ok(items) if items.is_empty() => {
                return SlashResult::Message(ChatMessage::simple(
                    "assistant",
                    "No reusable workflows matched.".to_string(),
                ));
            }
            Ok(items) => {
                let mut lines = Vec::new();
                lines.push("**Reusable Workflows:**".to_string());
                for item in items {
                    lines.push(format!(
                        "• **{}** `[{}]` (success: {}, failure: {})",
                        item.name, item.status, item.success_count, item.failure_count
                    ));
                    if !item.summary.trim().is_empty() {
                        lines.push(format!("  _{}_", item.summary.trim()));
                    }
                }
                return SlashResult::Message(ChatMessage::simple("assistant", lines.join("\n")));
            }
            Err(e) => {
                return SlashResult::Message(ChatMessage::simple(
                    "assistant",
                    format!("✕ Error searching workflows: {}", e),
                ));
            }
        }
    }

    if trimmed == "/skills" || trimmed == "/skill" {
        match crate::agent::skills::load_skills() {
            Ok(skills) if skills.is_empty() => {
                return SlashResult::Message(ChatMessage::simple(
                    "assistant",
                    "No active skills found in `~/.openz/skills`.".to_string(),
                ));
            }
            Ok(skills) => {
                let mut lines = Vec::new();
                lines.push("**Active Skills:**".to_string());
                for skill in skills {
                    lines.push(format!("• **{}** — {}", skill.name, skill.description()));
                }
                return SlashResult::Message(ChatMessage::simple("assistant", lines.join("\n")));
            }
            Err(e) => {
                return SlashResult::Message(ChatMessage::simple(
                    "assistant",
                    format!("✕ Error loading skills: {}", e),
                ));
            }
        }
    }

    if trimmed == "/mcps" {
        let loop_guard = agent_loop.try_lock();
        let servers = loop_guard
            .as_ref()
            .map(|l| &l.config.mcp_servers)
            .unwrap_or(&config.mcp_servers);

        if servers.is_empty() {
            return SlashResult::Message(ChatMessage::simple(
                "assistant",
                "No MCP servers configured in `~/.openz/config.json`.\n\nUse `openz configure` or the `manage_mcp` tool to add servers.".to_string(),
            ));
        }

        let mut lines = Vec::new();
        lines.push("**Configured MCP Servers:**".to_string());
        for (name, mcp_cfg) in servers {
            let status = if mcp_cfg.enabled { "enabled" } else { "disabled" };
            lines.push(format!("• **{}** `[{}]` — `{}`", name, status, mcp_cfg.command));
        }
        return SlashResult::Message(ChatMessage::simple("assistant", lines.join("\n")));
    }

    if trimmed == "/audit" {
        if let Ok(session) = session_manager.load(session_key) {
            match session.verify_hash_chain() {
                Ok(()) => {
                    return SlashResult::Message(ChatMessage::simple(
                        "assistant",
                        format!(
                            "🛡️ **Audit Ledger Verified:** All {} message hashes and SHA-256 hash chains in session `{}` are cryptographically intact.",
                            session.messages.len(),
                            session_key
                        ),
                    ));
                }
                Err(e) => {
                    return SlashResult::Message(ChatMessage::simple(
                        "assistant",
                        format!("⚠️ **Audit Ledger Discrepancy:** Verification failed: {}", e),
                    ));
                }
            }
        } else {
            return SlashResult::Message(ChatMessage::simple(
                "assistant",
                "No active session record found to audit.".to_string(),
            ));
        }
    }

    if trimmed.starts_with("/sop") {
        let sub = trimmed.strip_prefix("/sop").unwrap_or("").trim();
        if sub.is_empty() || sub == "list" {
            match crate::sop::load_definitions() {
                Ok(defs) if defs.is_empty() => {
                    return SlashResult::Message(ChatMessage::simple(
                        "assistant",
                        "No Standard Operating Procedure (SOP) definitions found in `~/.openz/sop/`.".to_string(),
                    ));
                }
                Ok(defs) => {
                    let mut lines = Vec::new();
                    lines.push("**Available SOP Workflows:**".to_string());
                    for def in defs {
                        lines.push(format!("• **{}** (`{}`): {}", def.name, def.id, def.description));
                        lines.push(format!("  Steps: {} defined", def.steps.len()));
                    }
                    lines.push("\n*Use `openz sop trigger <id>` or `/sop instances`.*".to_string());
                    return SlashResult::Message(ChatMessage::simple("assistant", lines.join("\n")));
                }
                Err(e) => {
                    return SlashResult::Message(ChatMessage::simple(
                        "assistant",
                        format!("✕ Error loading SOP definitions: {}", e),
                    ));
                }
            }
        } else if sub == "instances" {
            match crate::sop::list_instances() {
                Ok(instances) if instances.is_empty() => {
                    return SlashResult::Message(ChatMessage::simple(
                        "assistant",
                        "No SOP execution instances recorded yet.".to_string(),
                    ));
                }
                Ok(instances) => {
                    let mut lines = Vec::new();
                    lines.push("**SOP Execution Instances:**".to_string());
                    for inst in instances {
                        lines.push(format!(
                            "• Instance `{}` — SOP `{}` — Status: `{:?}` (Started: {})",
                            inst.id, inst.sop_id, inst.status, inst.started_at
                        ));
                    }
                    return SlashResult::Message(ChatMessage::simple("assistant", lines.join("\n")));
                }
                Err(e) => {
                    return SlashResult::Message(ChatMessage::simple(
                        "assistant",
                        format!("✕ Error listing SOP instances: {}", e),
                    ));
                }
            }
        } else {
            return SlashResult::Message(ChatMessage::simple(
                "assistant",
                "Usage:\n• `/sop list` — List available SOP definitions\n• `/sop instances` — List recent execution instances".to_string(),
            ));
        }
    }

    if trimmed.starts_with("/logs") {
        let tail_count = trimmed
            .strip_prefix("/logs")
            .unwrap_or("")
            .trim()
            .parse::<usize>()
            .unwrap_or(10)
            .min(50);

        let log_path = dirs::home_dir()
            .map(|h| h.join(".openz").join("openz.log"))
            .unwrap_or_else(|| std::path::PathBuf::from("openz.log"));

        if log_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&log_path) {
                let lines: Vec<&str> = content.lines().collect();
                let start = lines.len().saturating_sub(tail_count);
                let tail_lines = &lines[start..];
                let formatted = format!(
                    "**Last {} log lines from `{}`:**\n```\n{}\n```",
                    tail_lines.len(),
                    log_path.display(),
                    tail_lines.join("\n")
                );
                return SlashResult::Message(ChatMessage::simple("assistant", formatted));
            }
        }
        return SlashResult::Message(ChatMessage::simple(
            "assistant",
            "No active log file found at `~/.openz/openz.log`.\n\nUse `openz logs` in another terminal for live streaming logs.".to_string(),
        ));
    }

    SlashResult::Unhandled
}

pub async fn handle_device_command_markdown(input: &str) -> String {
    use crate::tools::Tool;
    let tool = crate::tools::device_inventory::DeviceInventoryTool::new();
    let mut parts = input.split_whitespace();
    let action = parts.next().unwrap_or("help");

    let result = match action {
        "" | "help" => {
            return "**Device Inventory Commands:**\n\
                    • `/device list [category]` — List device capabilities\n\
                    • `/device suggest <category> <target>` — Suggest command for target\n\
                    • `/device add <category> <name> <command> [works_for_csv]` — Add device capability\n\
                    • `/device delete <id>` — Remove capability by ID\n\
                    • `/device success <id>` — Record successful run\n\
                    • `/device failure <id>` — Record failed run"
                .to_string();
        }
        "list" => {
            let category = parts.next();
            let mut args = serde_json::json!({"action": "list"});
            if let Some(category) = category {
                args["category"] = serde_json::json!(category);
            }
            match tool.call(&args).await {
                Ok(val) => val,
                Err(e) => return format!("✕ Error: {}", e),
            }
        }
        "suggest" => {
            let category = parts.next().unwrap_or("");
            let target = parts.next().unwrap_or("");
            if category.is_empty() || target.is_empty() {
                return "Usage: `/device suggest <category> <target|extension>`".to_string();
            }
            match tool
                .call(&serde_json::json!({
                    "action": "suggest",
                    "category": category,
                    "target": target,
                }))
                .await
            {
                Ok(val) => val,
                Err(e) => return format!("✕ Error: {}", e),
            }
        }
        "delete" | "remove" => {
            let id = parts.next().unwrap_or("");
            if id.is_empty() {
                return "Usage: `/device delete <id>`".to_string();
            }
            match tool.call(&serde_json::json!({"action": "delete", "id": id})).await {
                Ok(val) => val,
                Err(e) => return format!("✕ Error: {}", e),
            }
        }
        "add" => {
            let category = parts.next().unwrap_or("");
            let name = parts.next().unwrap_or("");
            let command = parts.next().unwrap_or("");
            let works_for = parts.next().unwrap_or("");
            if category.is_empty() || name.is_empty() || command.is_empty() {
                return "Usage: `/device add <category> <name> <command> [works_for_csv]`".to_string();
            }
            let wf: Vec<String> = works_for
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(ToOwned::to_owned)
                .collect();
            match tool
                .call(&serde_json::json!({
                    "action": "add",
                    "category": category,
                    "name": name,
                    "command": command,
                    "args": ["{target}"],
                    "works_for": wf,
                    "source": "user",
                }))
                .await
            {
                Ok(val) => val,
                Err(e) => return format!("✕ Error: {}", e),
            }
        }
        other => {
            return format!("Unknown `/device` action: `{}`. Type `/device help` for options.", other);
        }
    };

    format_device_result_markdown(&result)
}

fn format_device_result_markdown(result: &serde_json::Value) -> String {
    if let Some(caps) = result.get("capabilities").and_then(|v| v.as_array()) {
        if caps.is_empty() {
            return "No device capabilities saved.".to_string();
        }
        let mut lines = Vec::new();
        lines.push("**Device Capabilities:**".to_string());
        for cap in caps {
            let id = cap.get("id").and_then(|v| v.as_str()).unwrap_or("?");
            let name = cap.get("name").and_then(|v| v.as_str()).unwrap_or("?");
            let category = cap.get("category").and_then(|v| v.as_str()).unwrap_or("?");
            let cmd = cap.get("command").and_then(|v| v.as_str()).unwrap_or("?");
            lines.push(format!("• `#{}` **{}** `[{}]` — `{}`", id, name, category, cmd));
        }
        return lines.join("\n");
    }

    if let Some(suggs) = result.get("suggestions").and_then(|v| v.as_array()) {
        if suggs.is_empty() {
            return "No matching device capability suggestions found.".to_string();
        }
        let mut lines = Vec::new();
        lines.push("**Suggestions:**".to_string());
        for sug in suggs {
            let name = sug.get("name").and_then(|v| v.as_str()).unwrap_or("?");
            let cmd = sug.get("command").and_then(|v| v.as_str()).unwrap_or("?");
            lines.push(format!("• **{}**: `{}`", name, cmd));
        }
        return lines.join("\n");
    }

    if let Some(msg) = result.get("message").and_then(|v| v.as_str()) {
        return format!("✓ {}", msg);
    }

    result.to_string()
}
