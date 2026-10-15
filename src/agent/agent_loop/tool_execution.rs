use crate::agent::style::*;
use crate::channels::notifications::{
    discord_message_url, send_external_notification, telegram_api_url, whatsapp_message_url,
    NotificationAuth, NotificationRequest, NotificationTarget,
};
use crate::providers::ToolCallRequest;
use crate::tools::arguments::{COMMAND_KEYS, OUTPUT_KEYS, PATH_KEYS, QUERY_KEYS, URL_KEYS};

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct ToolExecutionOutcome {
    pub id: String,
    pub name: String,
    pub result: serde_json::Value,
    pub assistant_tool_call: serde_json::Value,
    pub should_halt: bool,
}

fn string_arg<'a>(
    map: &'a serde_json::Map<String, serde_json::Value>,
    keys: &[&str],
) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| map.get(*key).and_then(|value| value.as_str()))
}

fn filename_arg(map: &serde_json::Map<String, serde_json::Value>, keys: &[&str]) -> Option<String> {
    string_arg(map, keys).map(file_name_of)
}

/// Truncate to at most `max` characters with a trailing ellipsis.
fn clip(s: &str, max: usize) -> String {
    match s.chars().count() > max {
        true => format!(
            "{}...",
            s.chars().take(max.saturating_sub(3)).collect::<String>()
        ),
        false => s.to_string(),
    }
}

fn file_name_of(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

fn format_browser_action(
    map: &serde_json::Map<String, serde_json::Value>,
    default_action: &str,
) -> String {
    let action = map
        .get("action")
        .and_then(|v| v.as_str())
        .unwrap_or(default_action);
    let url = string_arg(map, URL_KEYS).unwrap_or("");
    match url.is_empty() {
        true => format!("action: {}", action),
        false => format!("action: {}, url: \"{}\"", action, clip(url, 30)),
    }
}

fn format_action_with_optional_field(
    map: &serde_json::Map<String, serde_json::Value>,
    action_keys: &[&str],
    default_action: &str,
    field_keys: &[&str],
    field_label: &str,
    clip_len: Option<usize>,
) -> String {
    let action = string_arg(map, action_keys).unwrap_or(default_action);
    let field_val = string_arg(map, field_keys).unwrap_or("");
    match field_val.is_empty() {
        true => format!("action: {}", action),
        false => {
            let rendered = match clip_len {
                Some(max) => clip(field_val, max),
                None => field_val.to_string(),
            };
            format!("action: {}, {}: \"{}\"", action, field_label, rendered)
        }
    }
}

pub(crate) fn format_tool_args(name: &str, raw_args: &serde_json::Value) -> String {
    let args = raw_args.clone();
    let friendly_name = crate::tools::compact_presentation_name(name);

    let details = match &args {
        serde_json::Value::Object(map) => match name {
            // Query-style tools
            "grep_search" | "web_search" | "semantic_search" => string_arg(map, QUERY_KEYS)
                .map(|q| format!("query: \"{}\"", clip(q, 35)))
                .unwrap_or_default(),
            // Filesystem-style tools — show just the target file name
            "read_file"
            | "view_file"
            | "write_file"
            | "write_to_file"
            | "replace_file_content"
            | "multi_replace_file_content"
            | "patch_file"
            | "replace_lines"
            | "list_dir" => filename_arg(map, PATH_KEYS).unwrap_or_default(),
            // Shell tools — show the first command line
            "run_command" | "exec_command" => string_arg(map, COMMAND_KEYS)
                .map(|cmd| clip(cmd.lines().next().unwrap_or("").trim(), 40))
                .unwrap_or_default(),
            "git_manager" => map
                .get("action")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            "cargo_manager" => map
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            "web_fetch" | "read_url_content" | "read_url" => string_arg(map, URL_KEYS)
                .map(|url| format!("\"{}\"", clip(url, 35)))
                .unwrap_or_default(),
            "ast_grep" => map
                .get("pattern")
                .and_then(|v| v.as_str())
                .map(|p| format!("\"{}\"", clip(p, 35)))
                .unwrap_or_default(),
            "crawl" | "crawl_website" | "crawl_site" => map
                .get("url")
                .and_then(|v| v.as_str())
                .map(|url| format!("url: \"{}\"", clip(url, 30)))
                .unwrap_or_default(),
            "obscura_browser" | "obscura" => format_browser_action(map, "render"),
            "gsd_browser" | "gsd" | "firefox_browser" | "firefox" => {
                format_browser_action(map, "navigate")
            }
            "inspect_browsers" => "check health".to_string(),
            "generate_image" => {
                let path = string_arg(map, OUTPUT_KEYS).unwrap_or("output.png");
                let filename = file_name_of(path);
                let shapes_count = map
                    .get("shapes")
                    .and_then(|v| v.as_array())
                    .map(|a| a.len())
                    .unwrap_or(0);
                match shapes_count > 0 {
                    true => format!("output: \"{}\", shapes: {}", filename, shapes_count),
                    false => format!("output: \"{}\"", filename),
                }
            }
            "generate_video" => {
                let path = string_arg(map, OUTPUT_KEYS).unwrap_or("output.mp4");
                format!("output: \"{}\"", file_name_of(path))
            }
            "html_to_video" => {
                let html_path = string_arg(map, &["html_path", "htmlPath"]).unwrap_or("");
                let is_remote =
                    html_path.starts_with("http://") || html_path.starts_with("https://");
                let html_filename = match is_remote {
                    true => clip(html_path, 30),
                    false => file_name_of(html_path),
                };
                let out_path = string_arg(map, OUTPUT_KEYS).unwrap_or("output.mp4");
                let duration = map
                    .get("duration_seconds")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(5.0);
                let fps = map.get("fps").and_then(|v| v.as_i64()).unwrap_or(30);
                let frames = (duration * fps as f64).round() as usize;
                let duration_display = match duration.fract() == 0.0 {
                    true => format!("{:.0}", duration),
                    false => format!("{:.1}", duration),
                };
                format!(
                    "html: \"{}\", output: \"{}\", duration: {}s, fps: {}, frames: {}",
                    html_filename,
                    file_name_of(out_path),
                    duration_display,
                    fps,
                    frames
                )
            }
            "create_animated_svg" => {
                let path = string_arg(map, OUTPUT_KEYS).unwrap_or("output.svg");
                let filename = file_name_of(path);
                let elem_count = map
                    .get("elements")
                    .and_then(|v| v.as_array())
                    .map(|a| a.len())
                    .unwrap_or(0);
                let anim_count: usize = map
                    .get("elements")
                    .and_then(|v| v.as_array())
                    .map(|elems| {
                        elems
                            .iter()
                            .map(|e| {
                                e.get("animations")
                                    .and_then(|a| a.as_array())
                                    .map(|a| a.len())
                                    .unwrap_or(0)
                            })
                            .sum()
                    })
                    .unwrap_or(0);
                match elem_count > 0 {
                    true => format!(
                        "output: \"{}\", elements: {}, animations: {}",
                        filename, elem_count, anim_count
                    ),
                    false => format!("output: \"{}\"", filename),
                }
            }
            "doc_reader" | "read_doc" => string_arg(map, PATH_KEYS)
                .map(|path| format!("file: \"{}\"", file_name_of(path)))
                .unwrap_or_default(),
            "opendoc_open_document"
            | "opendoc_read_document_text"
            | "opendoc_search_document"
            | "opendoc_find_tables"
            | "opendoc_convert"
            | "opendoc_create_pdf"
            | "opendoc_create_formatted_pdf"
            | "opendoc_create_docx"
            | "opendoc_create_pptx"
            | "opendoc_create_xlsx"
            | "opendoc_create_html"
            | "opendoc_extract_images"
            | "opendoc_analyze_document_complexity"
            | "opendoc_ocr_document"
            | "opendoc_split_pdf"
            | "opendoc_merge_pdfs"
            | "opendoc_extract_archive_digest" => string_arg(
                map,
                &[
                    "file_path",
                    "source",
                    "output_path",
                    "archive_path",
                    "path",
                    "file",
                ],
            )
            .map(|path| format!("file: \"{}\"", file_name_of(path)))
            .unwrap_or_default(),
            "wasm_sandbox" => string_arg(map, &["wasm_path", "wasmPath"])
                .map(|path| format!("wasm: \"{}\"", file_name_of(path)))
                .unwrap_or_default(),
            "cron" | "watcher" => map
                .get("action")
                .and_then(|v| v.as_str())
                .map(|action| format!("action: \"{}\"", action))
                .unwrap_or_default(),
            "schedule_job" => {
                let id = string_arg(map, &["id", "job_id", "jobId", "name"]).unwrap_or("");
                let schedule = string_arg(map, &["schedule", "cron", "interval"]).unwrap_or("");
                format!("id: \"{}\", schedule: \"{}\"", id, schedule)
            }
            "get_job" | "remove_job" | "pause_job" | "resume_job" | "run_job_now"
            | "get_job_logs" => {
                let id = string_arg(map, &["id", "job_id", "jobId", "name"]).unwrap_or("");
                format!("id: \"{}\"", id)
            }
            "db_inspector" | "db_write" => {
                let db_path = string_arg(
                    map,
                    &["db_path", "dbPath", "path", "database", "db", "file"],
                )
                .unwrap_or("memory.db");
                let db_filename = file_name_of(db_path);
                let sql =
                    string_arg(map, &["sql", "query", "statement", "select", "mutation"])
                        .unwrap_or("");
                match sql.is_empty() {
                    false => format!("db: \"{}\", sql: \"{}\"", db_filename, clip(sql, 35)),
                    true => {
                        let action =
                            string_arg(map, &["action", "command", "cmd"]).unwrap_or("schema");
                        format!("db: \"{}\", action: \"{}\"", db_filename, action)
                    }
                }
            }
            "knowledge_source" => format_action_with_optional_field(
                map,
                &["action", "act", "op"],
                "search",
                &["uri", "url", "link", "query", "q", "id"],
                "target",
                Some(35),
            ),
            "store_memory" => {
                let text = string_arg(map, &["text", "content", "memory", "fact"]).unwrap_or("");
                format!("fact: \"{}\"", clip(text, 40))
            }
            "recall_memory" => {
                let query = string_arg(map, &["query", "q", "search", "term"]).unwrap_or("");
                format!("query: \"{}\"", clip(query, 40))
            }
            "curate_skill" => format_action_with_optional_field(
                map,
                &["action", "act", "op"],
                "list",
                &["skill_name", "name", "skill"],
                "skill",
                None,
            ),
            "openz_inventory" => "runtime identity".to_string(),
            "tool_catalog" => {
                let domain = string_arg(map, &["domain", "category", "cat"]).unwrap_or("all");
                format!("domain: \"{}\"", domain)
            }
            _ => {
                let parts: Vec<String> = map
                    .iter()
                    .filter(|(k, _)| k.as_str() != "session_key" && k.as_str() != "session_id")
                    .map(|(k, v)| {
                        let val_str = match v {
                            serde_json::Value::String(s) => format!("\"{}\"", clip(s, 20)),
                            other => clip(&other.to_string(), 20),
                        };
                        format!("{}: {}", k, val_str)
                    })
                    .collect();
                clip(&parts.join(", "), 50)
            }
        },
        _ => clip(&args.to_string(), 50),
    };

    match details.is_empty() {
        true => format!("{}{}{}", COLOR_BOLD, friendly_name, COLOR_RESET),
        false => format!(
            "{}{}{}({})",
            COLOR_BOLD, friendly_name, COLOR_RESET, details
        ),
    }
}

pub(crate) async fn send_progress_update(session_key: &str, text: &str) {
    let actual_session = crate::agent::style::spinner::get_current_session_key()
        .unwrap_or_else(|| session_key.to_string());
    match crate::channels::websocket::ws_chat_id(&actual_session)
        .or_else(|| crate::channels::websocket::ws_chat_id(session_key))
    {
        Some(chat_id) => {
            crate::channels::websocket::publish_ws_event(
                crate::channels::websocket::protocol::tool_progress(
                    chat_id,
                    crate::agent::agent_loop::current_turn_id(),
                    "",
                    "",
                    text,
                ),
            );
            crate::channels::websocket::publish_activity_notice(
                &actual_session,
                "progress",
                "Progress",
                text,
            );
        }
        None => {}
    }

    match (
        actual_session.strip_prefix("telegram:"),
        actual_session.strip_prefix("discord:"),
        actual_session.strip_prefix("whatsapp:"),
    ) {
        (Some(chat_id_str), _, _) => {
            match (
                chat_id_str.parse::<i64>(),
                crate::channels::telegram::get_telegram_bot_info(),
            ) {
                (Ok(chat_id), Some((bot_token, client))) => {
                    let request = NotificationRequest {
                        target: NotificationTarget::Telegram {
                            chat_id: chat_id.to_string(),
                        },
                        url: telegram_api_url(&bot_token, "sendMessage"),
                        payload: serde_json::json!({
                            "chat_id": chat_id,
                            "text": text,
                            "parse_mode": "Markdown"
                        }),
                        auth: NotificationAuth::None,
                    };
                    send_external_notification(&client, &request).await;
                }
                _ => {}
            }
        }
        (None, Some(channel_id), _) => match crate::channels::discord::get_discord_bot_info() {
            Some((bot_token, client)) => {
                let request = NotificationRequest {
                    target: NotificationTarget::Discord {
                        channel_id: channel_id.to_string(),
                    },
                    url: discord_message_url(channel_id),
                    payload: serde_json::json!({ "content": text }),
                    auth: NotificationAuth::Header {
                        name: "Authorization",
                        value: format!("Bot {bot_token}"),
                    },
                };
                send_external_notification(&client, &request).await;
            }
            None => {}
        },
        (None, None, Some(phone_number)) => {
            match crate::channels::whatsapp::get_whatsapp_bot_info() {
                Some((api_key, phone_number_id, client)) => {
                    let request = NotificationRequest {
                        target: NotificationTarget::WhatsApp {
                            recipient: phone_number.to_string(),
                        },
                        url: whatsapp_message_url(&phone_number_id),
                        payload: serde_json::json!({
                            "messaging_product": "whatsapp",
                            "recipient_type": "individual",
                            "to": phone_number,
                            "type": "text",
                            "text": { "body": text }
                        }),
                        auth: NotificationAuth::Bearer(api_key),
                    };
                    send_external_notification(&client, &request).await;
                }
                None => {}
            }
        }
        (None, None, None) => {}
    }
}

pub(crate) async fn render_tool_success(
    call: &ToolCallRequest,
    formatted_args: &str,
    session_key: &str,
    silent: bool,
    result: serde_json::Value,
) -> serde_json::Value {
    let success_msg = format!("✓ *{}*", formatted_args);
    send_progress_update(session_key, &success_msg).await;
    let should_print = !silent
        && !crate::agent::style::is_profile_subagent(&call.name)
        && call.name != "parallel_research";
    match should_print {
        true => {
            let leaf_prefix = crate::agent::style::get_tree_prefix(true);
            let summary = crate::agent::style::format_tool_outcome_summary(
                &call.name,
                &call.arguments,
                &result,
            );
            let has_own_badge = call.name == "write_file"
                || call.name == "patch_file"
                || call.name == "replace_lines"
                || summary.contains('\u{2713}')
                || summary.contains('\u{2715}');
            match has_own_badge {
                true => {
                    crate::tui_println!("{}{}{}{}", AURA_SLATE, leaf_prefix, COLOR_RESET, summary);
                }
                false => {
                    crate::tui_println!(
                        "{}{}{}✓ {}{}",
                        AURA_SLATE,
                        leaf_prefix,
                        AURA_GREEN,
                        summary,
                        COLOR_RESET
                    );
                }
            }
        }
        false => {}
    }
    tracing::info!(
        session = %session_key,
        tool = %call.name,
        status = "success",
        "Tool call completed"
    );
    tracing::debug!(
        session = %session_key,
        tool = %call.name,
        result = %result,
        "Tool output result"
    );
    result
}

pub(crate) async fn render_tool_failure(
    call: &ToolCallRequest,
    formatted_args: &str,
    session_key: &str,
    silent: bool,
    error_str: &str,
) -> serde_json::Value {
    let fail_msg = format!("✕ *{}* - Failed: {}", formatted_args, error_str);
    send_progress_update(session_key, &fail_msg).await;
    let should_print = !silent
        && !crate::agent::style::is_profile_subagent(&call.name)
        && call.name != "parallel_research";
    match should_print {
        true => {
            let leaf_prefix = crate::agent::style::get_tree_prefix(true);
            crate::tui_println!(
                "{}{}{}✕ {}{}",
                AURA_SLATE,
                leaf_prefix,
                AURA_ROSE,
                error_str,
                COLOR_RESET
            );
        }
        false => {}
    }
    tracing::error!(
        session = %session_key,
        tool = %call.name,
        error = %error_str,
        "Tool call failed"
    );
    error_value_with_hint(&call.name, error_str)
}

pub(crate) async fn render_tool_not_found(
    call: &ToolCallRequest,
    formatted_args: &str,
    session_key: &str,
    silent: bool,
) -> serde_json::Value {
    let error_str = format!("Tool '{}' not found", call.name);
    let fail_msg = format!("✕ *{}* - Failed: {}", formatted_args, error_str);
    send_progress_update(session_key, &fail_msg).await;
    match silent {
        false => {
            let leaf_prefix = crate::agent::style::get_tree_prefix(true);
            crate::tui_println!(
                "{}{}{}✗{} {} - Failed: {}{}",
                AURA_SLATE,
                leaf_prefix,
                COLOR_RESET,
                AURA_ROSE,
                formatted_args,
                error_str,
                COLOR_RESET
            );
        }
        true => {}
    }
    error_value_with_hint(&call.name, &error_str)
}

fn error_value_with_hint(tool_name: &str, error_str: &str) -> serde_json::Value {
    let hint = super::loop_control::generate_self_healing_hint(tool_name, error_str);
    match serde_json::from_str::<serde_json::Value>(error_str) {
        Ok(serde_json::Value::Object(mut map)) => {
            match !map.contains_key("self_healing_suggestion") && !map.contains_key("suggestion") {
                true => {
                    map.insert(
                        "self_healing_suggestion".to_string(),
                        serde_json::Value::String(hint),
                    );
                }
                false => {}
            }
            serde_json::Value::Object(map)
        }
        _ => serde_json::json!({
            "error": error_str,
            "self_healing_suggestion": hint
        }),
    }
}

#[cfg(test)]
#[path = "tool_execution_tests.rs"]
mod tests;
