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

enum RatatuiSlashCommand<'a> {
    Exit,
    Clear,
    Help,
    History,
    Model(&'a str),
    NewSession,
    Settings,
    Streaming,
    Servers,
    StopServer(&'a str),
    Device(&'a str),
    Memory,
    Sources(&'a str),
    Workflows(&'a str),
    Skills,
    Mcps,
    Audit,
    Sop(&'a str),
    Logs(&'a str),
    Unhandled,
}

impl<'a> RatatuiSlashCommand<'a> {
    fn parse(trimmed: &'a str) -> Self {
        match trimmed {
            "/exit" | "exit" | "quit" => return Self::Exit,
            "/clear" => return Self::Clear,
            "/help" | "/commands" => return Self::Help,
            "/history" => return Self::History,
            "/new-session" => return Self::NewSession,
            "/settings" => return Self::Settings,
            "/streaming" => return Self::Streaming,
            "/servers" => return Self::Servers,
            "/memory" => return Self::Memory,
            "/skills" | "/skill" => return Self::Skills,
            "/mcps" => return Self::Mcps,
            "/audit" => return Self::Audit,
            _ => {}
        }
        match trimmed.strip_prefix("/model") {
            Some(rest) => return Self::Model(rest.trim()),
            None => {}
        }
        match trimmed.strip_prefix("/stop-server") {
            Some(rest) => return Self::StopServer(rest.trim()),
            None => {}
        }
        match trimmed.strip_prefix("/device") {
            Some(rest) => return Self::Device(rest.trim()),
            None => {}
        }
        match trimmed.strip_prefix("/sources") {
            Some(rest) => return Self::Sources(rest.trim()),
            None => {}
        }
        match trimmed.strip_prefix("/workflows") {
            Some(rest) => return Self::Workflows(rest.trim()),
            None => {}
        }
        match trimmed.strip_prefix("/sop") {
            Some(rest) => return Self::Sop(rest.trim()),
            None => {}
        }
        match trimmed.strip_prefix("/logs") {
            Some(rest) => return Self::Logs(rest.trim()),
            None => {}
        }
        Self::Unhandled
    }
}

fn enabled_title_label(flag: bool) -> &'static str {
    match flag {
        true => "Enabled",
        false => "Disabled",
    }
}

use crate::channels::enabled_lower_label;

fn assistant_msg(text: impl Into<String>) -> SlashResult {
    SlashResult::Message(ChatMessage::simple("assistant", text.into()))
}

pub async fn handle_slash_command(
    trimmed: &str,
    app: &mut RatatuiApp,
    agent_loop: &crate::agent::AgentLoop,
    session_key: &str,
    session_manager: &crate::session::SessionManager,
    config: &crate::config::schema::Config,
) -> SlashResult {
    match RatatuiSlashCommand::parse(trimmed) {
        RatatuiSlashCommand::Exit => {
            SlashResult::OpenModal(ModalState::ExitConfirm { selected_yes: true })
        }
        RatatuiSlashCommand::Clear => SlashResult::ClearTimeline,
        RatatuiSlashCommand::Help => SlashResult::OpenModal(ModalState::new_command_catalog()),
        RatatuiSlashCommand::History => {
            let sessions = crate::cli::load_session_history().unwrap_or_default();
            let items: Vec<(String, String, String)> = sessions
                .into_iter()
                .map(|s| {
                    (
                        s.key,
                        s.display_title,
                        s.updated_at.format("%Y-%m-%d %H:%M").to_string(),
                    )
                })
                .collect();
            SlashResult::OpenModal(ModalState::History {
                sessions: items,
                selected_idx: 0,
            })
        }
        RatatuiSlashCommand::Model("") => {
            let configured = crate::channels::build_configured_providers(config);
            SlashResult::OpenModal(ModalState::ProviderSelect {
                providers: configured,
                selected_idx: 0,
            })
        }
        RatatuiSlashCommand::Model(arg) => {
            let (prov, mdl) = match arg.find('/') {
                Some(idx) => (&arg[..idx], &arg[idx + 1..]),
                None => ("auto", arg),
            };
            let marker_dir = super::session::tui_marker_dir();
            match super::session::apply_session_model_selection(
                session_manager,
                session_key,
                &marker_dir,
                prov,
                mdl,
            )
            .await
            {
                Ok(()) => {
                    app.model = mdl.to_string();
                    app.provider = prov.to_string();
                    assistant_msg(format!(
                        "✓ Active session model switched to `{}` (provider: `{}`).",
                        mdl, prov
                    ))
                }
                Err(_) => assistant_msg(format!(
                    "✕ Failed to resolve model `{}` for provider `{}`.",
                    mdl, prov
                )),
            }
        }
        RatatuiSlashCommand::NewSession => SlashResult::NewSession,
        RatatuiSlashCommand::Settings => {
            let mut effective_cfg = crate::config::loader::load_config()
                .unwrap_or_else(|_| agent_loop.config.clone());
            match session_manager.load(session_key) {
                Ok(session) => {
                    crate::agent::agent_loop::apply_session_overrides(
                        &mut effective_cfg,
                        &session.metadata,
                    );
                }
                Err(_) => {}
            }
            let defaults = &effective_cfg.agents.defaults;

            let mut lines = vec![
                "**Active Session Settings:**".to_string(),
                format!("• **Session Key:** `{}`", session_key),
                format!("• **Model:** `{}`", defaults.model),
                format!("• **Provider:** `{}`", defaults.provider),
                format!("• **Security Mode:** `{}`", defaults.security_mode),
                format!(
                    "• **Streaming:** `{}`",
                    enabled_title_label(defaults.streaming)
                ),
                format!(
                    "• **Sandbox (seccomp BPF):** `{}`",
                    enabled_title_label(defaults.enable_sandbox)
                ),
                format!(
                    "• **Caveman Mode:** `{}`",
                    enabled_title_label(defaults.caveman_mode)
                ),
                format!("• **Max Messages:** `{}`", defaults.max_messages),
                format!(
                    "• **Max Tool Iterations:** `{}`",
                    defaults.max_tool_iterations
                ),
            ];

            match defaults.whitelisted_command_prefixes.is_empty() {
                true => {}
                false => lines.push(format!(
                    "• **Whitelisted Command Prefixes:** {}",
                    defaults
                        .whitelisted_command_prefixes
                        .iter()
                        .map(|p| format!("`{}`", p))
                        .collect::<Vec<_>>()
                        .join(", ")
                )),
            }

            match defaults.whitelisted_paths.is_empty() {
                true => {}
                false => lines.push(format!(
                    "• **Whitelisted Paths:** {}",
                    defaults
                        .whitelisted_paths
                        .iter()
                        .map(|p| format!("`{}`", p))
                        .collect::<Vec<_>>()
                        .join(", ")
                )),
            }

            assistant_msg(lines.join("\n"))
        }
        RatatuiSlashCommand::Streaming => {
            let current_streaming = session_manager
                .load(session_key)
                .ok()
                .and_then(|session| session.metadata.get("streaming").and_then(|v| v.as_bool()))
                .unwrap_or(agent_loop.config.agents.defaults.streaming);
            let next_streaming = !current_streaming;
            let _ = super::session::save_session_streaming_override(
                session_manager,
                session_key,
                next_streaming,
            )
            .await;

            assistant_msg(format!(
                "Response streaming is now **{}** for this session.",
                enabled_lower_label(next_streaming)
            ))
        }
        RatatuiSlashCommand::Servers => {
            let servers = crate::shutdown::list_registered_children();
            match servers.is_empty() {
                true => assistant_msg(
                    "No OpenZ-launched background servers running.\n\nUse `/stop-server <id|all>` to stop active servers.",
                ),
                false => {
                    let mut lines = vec!["**OpenZ Background Servers:**".to_string()];
                    for server in servers {
                        lines.push(format!(
                            "• `#{}` (PID {}) [{}] `{}`",
                            server.id, server.pid, server.kind, server.command
                        ));
                    }
                    lines.push(
                        "\n*Use `/stop-server <id>` or `/stop-server all` to terminate.*"
                            .to_string(),
                    );
                    assistant_msg(lines.join("\n"))
                }
            }
        }
        RatatuiSlashCommand::StopServer("") => assistant_msg("Usage: `/stop-server <id|all>`"),
        RatatuiSlashCommand::StopServer(target) => {
            match crate::shutdown::stop_registered_child(target) {
                Ok(0) => assistant_msg(format!(
                    "No matching background server found for `{}`.",
                    target
                )),
                Ok(count) => {
                    assistant_msg(format!("✓ Stopped {} background server(s).", count))
                }
                Err(e) => assistant_msg(format!("✕ Failed to stop server: {}", e)),
            }
        }
        RatatuiSlashCommand::Device(arg) => {
            let output = handle_device_command_markdown(arg).await;
            assistant_msg(output)
        }
        RatatuiSlashCommand::Memory => match session_manager.load(session_key) {
            Ok(session) => match session.metadata.is_empty() {
                true => assistant_msg("No metadata or facts recorded for this session yet."),
                false => {
                    let mut lines = vec!["**Session Metadata & Memory:**".to_string()];
                    for (k, v) in &session.metadata {
                        lines.push(format!("• **{}:** {}", k, v));
                    }
                    assistant_msg(lines.join("\n"))
                }
            },
            Err(_) => assistant_msg("No active session record found."),
        },
        RatatuiSlashCommand::Sources(query) => {
            match crate::tools::shared_memory::search_source_bookmarks(query, 10).await {
                Ok(items) => match items.is_empty() {
                    true => assistant_msg("No saved source bookmarks matched."),
                    false => {
                        let mut lines = vec!["**Saved Sources:**".to_string()];
                        for item in items {
                            lines.push(format!(
                                "• **{}** `[{}]` {}",
                                item.label, item.kind, item.uri
                            ));
                            match item.summary.trim().is_empty() {
                                true => {}
                                false => lines.push(format!("  _{}_", item.summary.trim())),
                            }
                        }
                        assistant_msg(lines.join("\n"))
                    }
                },
                Err(e) => assistant_msg(format!("✕ Error searching sources: {}", e)),
            }
        }
        RatatuiSlashCommand::Workflows(query) => {
            match crate::tools::shared_memory::search_workflow_cards(query, 10, false).await {
                Ok(items) => match items.is_empty() {
                    true => assistant_msg("No reusable workflows matched."),
                    false => {
                        let mut lines = vec!["**Reusable Workflows:**".to_string()];
                        for item in items {
                            lines.push(format!(
                                "• **{}** `[{}]` (success: {}, failure: {})",
                                item.name, item.status, item.success_count, item.failure_count
                            ));
                            match item.summary.trim().is_empty() {
                                true => {}
                                false => lines.push(format!("  _{}_", item.summary.trim())),
                            }
                        }
                        assistant_msg(lines.join("\n"))
                    }
                },
                Err(e) => assistant_msg(format!("✕ Error searching workflows: {}", e)),
            }
        }
        RatatuiSlashCommand::Skills => match crate::agent::skills::load_skills() {
            Ok(skills) => match skills.is_empty() {
                true => assistant_msg("No active skills found in `~/.openz/skills`."),
                false => {
                    let mut lines = vec!["**Active Skills:**".to_string()];
                    for skill in skills {
                        lines.push(format!("• **{}** — {}", skill.name, skill.description()));
                    }
                    assistant_msg(lines.join("\n"))
                }
            },
            Err(e) => assistant_msg(format!("✕ Error loading skills: {}", e)),
        },
        RatatuiSlashCommand::Mcps => {
            let latest_cfg = crate::config::loader::load_config().ok();
            let servers = latest_cfg
                .as_ref()
                .map(|c| &c.mcp_servers)
                .unwrap_or(&agent_loop.config.mcp_servers);

            match servers.is_empty() {
                true => assistant_msg(
                    "No MCP servers configured in `~/.openz/config.json`.\n\nUse `openz configure` or the `manage_mcp` tool to add servers.",
                ),
                false => {
                    let mut lines = vec!["**Configured MCP Servers:**".to_string()];
                    for (name, mcp_cfg) in servers {
                        let status = enabled_lower_label(mcp_cfg.enabled);
                        lines.push(format!(
                            "• **{}** `[{}]` — `{}`",
                            name, status, mcp_cfg.command
                        ));
                    }
                    assistant_msg(lines.join("\n"))
                }
            }
        }
        RatatuiSlashCommand::Audit => match session_manager.load(session_key) {
            Ok(session) => match session.verify_hash_chain() {
                Ok(()) => assistant_msg(format!(
                    "🛡️ **Audit Ledger Verified:** All {} message hashes and SHA-256 hash chains in session `{}` are cryptographically intact.",
                    session.messages.len(),
                    session_key
                )),
                Err(e) => assistant_msg(format!(
                    "⚠️ **Audit Ledger Discrepancy:** Verification failed: {}",
                    e
                )),
            },
            Err(_) => assistant_msg("No active session record found to audit."),
        },
        RatatuiSlashCommand::Sop("" | "list") => match crate::sop::load_definitions() {
            Ok(defs) => match defs.is_empty() {
                true => assistant_msg(
                    "No Standard Operating Procedure (SOP) definitions found in `~/.openz/sop/`.",
                ),
                false => {
                    let mut lines = vec!["**Available SOP Workflows:**".to_string()];
                    for def in defs {
                        lines.push(format!(
                            "• **{}** (`{}`): {}",
                            def.name, def.id, def.description
                        ));
                        lines.push(format!("  Steps: {} defined", def.steps.len()));
                    }
                    lines.push("\n*Use `openz sop trigger <id>` or `/sop instances`.*".to_string());
                    assistant_msg(lines.join("\n"))
                }
            },
            Err(e) => assistant_msg(format!("✕ Error loading SOP definitions: {}", e)),
        },
        RatatuiSlashCommand::Sop("instances") => match crate::sop::list_instances() {
            Ok(instances) => match instances.is_empty() {
                true => assistant_msg("No SOP execution instances recorded yet."),
                false => {
                    let mut lines = vec!["**SOP Execution Instances:**".to_string()];
                    for inst in instances {
                        lines.push(format!(
                            "• Instance `{}` — SOP `{}` — Status: `{:?}` (Started: {})",
                            inst.id, inst.sop_id, inst.status, inst.started_at
                        ));
                    }
                    assistant_msg(lines.join("\n"))
                }
            },
            Err(e) => assistant_msg(format!("✕ Error listing SOP instances: {}", e)),
        },
        RatatuiSlashCommand::Sop(_) => assistant_msg(
            "Usage:\n• `/sop list` — List available SOP definitions\n• `/sop instances` — List recent execution instances",
        ),
        RatatuiSlashCommand::Logs(raw_count) => {
            let tail_count = raw_count.parse::<usize>().unwrap_or(10).min(50);
            let log_path = dirs::home_dir()
                .map(|h| h.join(".openz").join("openz.log"))
                .unwrap_or_else(|| std::path::PathBuf::from("openz.log"));

            match std::fs::read_to_string(&log_path) {
                Ok(content) => {
                    let lines: Vec<&str> = content.lines().collect();
                    let start = lines.len().saturating_sub(tail_count);
                    let tail_lines = &lines[start..];
                    assistant_msg(format!(
                        "**Last {} log lines from `{}`:**\n```\n{}\n```",
                        tail_lines.len(),
                        log_path.display(),
                        tail_lines.join("\n")
                    ))
                }
                Err(_) => assistant_msg(
                    "No active log file found at `~/.openz/openz.log`.\n\nUse `openz logs` in another terminal for live streaming logs.",
                ),
            }
        }
        RatatuiSlashCommand::Unhandled => SlashResult::Unhandled,
    }
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
            let mut args = serde_json::json!({"action": "list"});
            match parts.next() {
                Some(category) => args["category"] = serde_json::json!(category),
                None => {}
            }
            match tool.call(&args).await {
                Ok(val) => val,
                Err(e) => return format!("✕ Error: {}", e),
            }
        }
        "suggest" => {
            let category = parts.next().unwrap_or("");
            let target = parts.next().unwrap_or("");
            match category.is_empty() || target.is_empty() {
                true => return "Usage: `/device suggest <category> <target|extension>`".to_string(),
                false => {}
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
            match id.is_empty() {
                true => return "Usage: `/device delete <id>`".to_string(),
                false => {}
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
            match category.is_empty() || name.is_empty() || command.is_empty() {
                true => {
                    return "Usage: `/device add <category> <name> <command> [works_for_csv]`"
                        .to_string()
                }
                false => {}
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
            return format!(
                "Unknown `/device` action: `{}`. Type `/device help` for options.",
                other
            );
        }
    };

    format_device_result_markdown(&result)
}

fn format_device_result_markdown(result: &serde_json::Value) -> String {
    match result.get("capabilities").and_then(|v| v.as_array()) {
        Some(caps) => match caps.is_empty() {
            true => return "No device capabilities saved.".to_string(),
            false => {
                let mut lines = vec!["**Device Capabilities:**".to_string()];
                for cap in caps {
                    let id = cap.get("id").and_then(|v| v.as_str()).unwrap_or("?");
                    let name = cap.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                    let category = cap.get("category").and_then(|v| v.as_str()).unwrap_or("?");
                    let cmd = cap.get("command").and_then(|v| v.as_str()).unwrap_or("?");
                    lines.push(format!(
                        "• `#{}` **{}** `[{}]` — `{}`",
                        id, name, category, cmd
                    ));
                }
                return lines.join("\n");
            }
        },
        None => {}
    }

    match result.get("suggestions").and_then(|v| v.as_array()) {
        Some(suggs) => match suggs.is_empty() {
            true => return "No matching device capability suggestions found.".to_string(),
            false => {
                let mut lines = vec!["**Suggestions:**".to_string()];
                for sug in suggs {
                    let name = sug.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                    let cmd = sug.get("command").and_then(|v| v.as_str()).unwrap_or("?");
                    lines.push(format!("• **{}**: `{}`", name, cmd));
                }
                return lines.join("\n");
            }
        },
        None => {}
    }

    match result.get("message").and_then(|v| v.as_str()) {
        Some(msg) => format!("✓ {}", msg),
        None => result.to_string(),
    }
}
