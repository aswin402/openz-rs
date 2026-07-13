use crate::agent::style::*;
use crate::providers::ToolCallRequest;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct ToolExecutionOutcome {
    pub id: String,
    pub name: String,
    pub result: serde_json::Value,
    pub assistant_tool_call: serde_json::Value,
    pub should_halt: bool,
}

pub(crate) fn format_tool_args(name: &str, args: &serde_json::Value) -> String {
    let friendly_name = match name {
        "grep_search" => "Search",
        "read_file" | "view_file" => "Read",
        "write_file" | "write_to_file" | "replace_file_content" | "multi_replace_file_content" => {
            "Edit"
        }
        "run_command" | "exec_command" => "Bash",
        "list_dir" => "ListDir",
        "code_outline" => "Outline",
        "ast_grep" => "AstGrep",
        "git_manager" => "Git",
        "cargo_manager" => "Cargo",
        "web_search" => "WebSearch",
        "gsd_browser" => "Browser",
        "clipboard" => "Clipboard",
        "open_path" | "open" => "Open",
        "web_fetch" | "read_url_content" | "read_url" => "Fetch",
        "generate_image" => "Image",
        "generate_video" => "Video",
        "html_to_video" => "HtmlVideo",
        "create_animated_svg" | "svg_animator" => "SvgAnim",
        "obscura_browser" => "Obscura",
        "db_inspector" => "DbInspect",
        "db_write" => "DbWrite",
        "read_doc" => "DocRead",
        "crawl" => "Crawl",
        "semantic_search" => "SemanticSearch",
        "wasm_sandbox" => "Wasm",
        "cron" => "Cron",
        "watcher" => "Watcher",
        other => other,
    };

    let details = if let serde_json::Value::Object(map) = args {
        if name == "grep_search" {
            if let Some(q) = map
                .get("query")
                .or_else(|| map.get("Query"))
                .and_then(|v| v.as_str())
            {
                if q.len() > 35 {
                    format!("query: \"{}...\"", q.chars().take(32).collect::<String>())
                } else {
                    format!("query: \"{}\"", q)
                }
            } else {
                String::new()
            }
        } else if name == "read_file" || name == "view_file" {
            if let Some(path) = map
                .get("path")
                .or_else(|| map.get("Path"))
                .or_else(|| map.get("AbsolutePath"))
                .and_then(|v| v.as_str())
            {
                if let Some(filename) = std::path::Path::new(path).file_name() {
                    filename.to_string_lossy().to_string()
                } else {
                    path.to_string()
                }
            } else {
                String::new()
            }
        } else if name == "write_file"
            || name == "write_to_file"
            || name == "replace_file_content"
            || name == "multi_replace_file_content"
            || name == "patch_file"
            || name == "replace_lines"
        {
            if let Some(path) = map
                .get("path")
                .or_else(|| map.get("TargetFile"))
                .or_else(|| map.get("Path"))
                .and_then(|v| v.as_str())
            {
                if let Some(filename) = std::path::Path::new(path).file_name() {
                    filename.to_string_lossy().to_string()
                } else {
                    path.to_string()
                }
            } else {
                String::new()
            }
        } else if name == "run_command" || name == "exec_command" {
            if let Some(cmd) = map
                .get("CommandLine")
                .or_else(|| map.get("Command"))
                .or_else(|| map.get("command"))
                .or_else(|| map.get("command_line"))
                .and_then(|v| v.as_str())
            {
                let first_line = cmd.lines().next().unwrap_or("").trim();
                if first_line.len() > 40 {
                    format!("{}...", first_line.chars().take(37).collect::<String>())
                } else {
                    first_line.to_string()
                }
            } else {
                String::new()
            }
        } else if name == "list_dir" {
            if let Some(path) = map
                .get("path")
                .or_else(|| map.get("DirectoryPath"))
                .or_else(|| map.get("Path"))
                .and_then(|v| v.as_str())
            {
                if let Some(filename) = std::path::Path::new(path).file_name() {
                    filename.to_string_lossy().to_string()
                } else {
                    path.to_string()
                }
            } else {
                String::new()
            }
        } else if name == "git_manager" {
            if let Some(action) = map.get("Action").and_then(|v| v.as_str()) {
                action.to_string()
            } else {
                String::new()
            }
        } else if name == "cargo_manager" {
            if let Some(command) = map.get("Command").and_then(|v| v.as_str()) {
                command.to_string()
            } else {
                String::new()
            }
        } else if name == "web_search" {
            if let Some(q) = map
                .get("query")
                .or_else(|| map.get("Query"))
                .and_then(|v| v.as_str())
            {
                if q.len() > 35 {
                    format!("query: \"{}...\"", q.chars().take(32).collect::<String>())
                } else {
                    format!("query: \"{}\"", q)
                }
            } else {
                String::new()
            }
        } else if name == "web_fetch" || name == "read_url_content" || name == "read_url" {
            if let Some(url) = map
                .get("Url")
                .or_else(|| map.get("url"))
                .and_then(|v| v.as_str())
            {
                if url.len() > 35 {
                    format!("\"{}...\"", url.chars().take(32).collect::<String>())
                } else {
                    format!("\"{}\"", url)
                }
            } else {
                String::new()
            }
        } else if name == "generate_image" {
            let path = map
                .get("output_path")
                .or_else(|| map.get("ImageName"))
                .and_then(|v| v.as_str())
                .unwrap_or("output.png");
            let filename = std::path::Path::new(path)
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string());
            let shapes_count = map
                .get("shapes")
                .and_then(|v| v.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            if shapes_count > 0 {
                format!("output: \"{}\", shapes: {}", filename, shapes_count)
            } else {
                format!("output: \"{}\"", filename)
            }
        } else if name == "generate_video" {
            let path = map
                .get("output_path")
                .and_then(|v| v.as_str())
                .unwrap_or("output.mp4");
            let filename = std::path::Path::new(path)
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string());
            format!("output: \"{}\"", filename)
        } else if name == "html_to_video" {
            let html_path = map.get("html_path").and_then(|v| v.as_str()).unwrap_or("");
            let html_filename =
                if html_path.starts_with("http://") || html_path.starts_with("https://") {
                    if html_path.len() > 30 {
                        format!("{}...", html_path.chars().take(27).collect::<String>())
                    } else {
                        html_path.to_string()
                    }
                } else {
                    std::path::Path::new(html_path)
                        .file_name()
                        .map(|f| f.to_string_lossy().to_string())
                        .unwrap_or_else(|| html_path.to_string())
                };
            let out_path = map
                .get("output_path")
                .and_then(|v| v.as_str())
                .unwrap_or("output.mp4");
            let out_filename = std::path::Path::new(out_path)
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| out_path.to_string());
            let duration = map
                .get("duration_seconds")
                .and_then(|v| v.as_f64())
                .unwrap_or(5.0);
            let fps = map.get("fps").and_then(|v| v.as_i64()).unwrap_or(30);
            let frames = (duration * fps as f64).round() as usize;
            let duration_display = if duration.fract() == 0.0 {
                format!("{:.0}", duration)
            } else {
                format!("{:.1}", duration)
            };
            format!(
                "html: \"{}\", output: \"{}\", duration: {}s, fps: {}, frames: {}",
                html_filename, out_filename, duration_display, fps, frames
            )
        } else if name == "create_animated_svg" {
            let path = map
                .get("output_path")
                .and_then(|v| v.as_str())
                .unwrap_or("output.svg");
            let filename = std::path::Path::new(path)
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string());
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
            if elem_count > 0 {
                format!(
                    "output: \"{}\", elements: {}, animations: {}",
                    filename, elem_count, anim_count
                )
            } else {
                format!("output: \"{}\"", filename)
            }
        } else if name == "obscura_browser" {
            let url = map.get("url").and_then(|v| v.as_str()).unwrap_or("");
            let action = map
                .get("action")
                .and_then(|v| v.as_str())
                .unwrap_or("render");
            let truncated_url = if url.len() > 30 {
                format!("{}...", url.chars().take(27).collect::<String>())
            } else {
                url.to_string()
            };
            format!("action: \"{}\", url: \"{}\"", action, truncated_url)
        } else if name == "gsd_browser" {
            let action = map.get("action").and_then(|v| v.as_str()).unwrap_or("");
            let url = map.get("url").and_then(|v| v.as_str()).unwrap_or("");
            let ref_id = map.get("ref_id").and_then(|v| v.as_str()).unwrap_or("");
            if !url.is_empty() {
                let truncated_url = if url.len() > 30 {
                    format!("{}...", url.chars().take(27).collect::<String>())
                } else {
                    url.to_string()
                };
                format!("action: \"{}\", url: \"{}\"", action, truncated_url)
            } else if !ref_id.is_empty() {
                format!("action: \"{}\", ref_id: \"{}\"", action, ref_id)
            } else {
                format!("action: \"{}\"", action)
            }
        } else if name == "ast_grep" {
            if let Some(pattern) = map.get("pattern").and_then(|v| v.as_str()) {
                if pattern.len() > 35 {
                    format!("\"{}...\"", pattern.chars().take(32).collect::<String>())
                } else {
                    format!("\"{}\"", pattern)
                }
            } else {
                String::new()
            }
        } else if name == "crawl" {
            let url = map.get("url").and_then(|v| v.as_str()).unwrap_or("");
            if url.len() > 30 {
                format!("url: \"{}...\"", url.chars().take(27).collect::<String>())
            } else {
                format!("url: \"{}\"", url)
            }
        } else if name == "semantic_search" {
            let query = map.get("query").and_then(|v| v.as_str()).unwrap_or("");
            if query.len() > 35 {
                format!(
                    "query: \"{}...\"",
                    query.chars().take(32).collect::<String>()
                )
            } else {
                format!("query: \"{}\"", query)
            }
        } else if name == "doc_reader" {
            let path = map
                .get("path")
                .or_else(|| map.get("file_path"))
                .or_else(|| map.get("Path"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let filename = std::path::Path::new(path)
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string());
            format!("file: \"{}\"", filename)
        } else if name == "wasm_sandbox" {
            let path = map.get("wasm_path").and_then(|v| v.as_str()).unwrap_or("");
            let filename = std::path::Path::new(path)
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string());
            format!("wasm: \"{}\"", filename)
        } else if name == "cron" {
            let action = map.get("action").and_then(|v| v.as_str()).unwrap_or("");
            format!("action: \"{}\"", action)
        } else if name == "watcher" {
            let action = map.get("action").and_then(|v| v.as_str()).unwrap_or("");
            format!("action: \"{}\"", action)
        } else if name == "db_inspector" || name == "db_write" {
            let db_path = map.get("db_path").and_then(|v| v.as_str()).unwrap_or("");
            let db_filename = std::path::Path::new(db_path)
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| db_path.to_string());
            let sql = map.get("sql").and_then(|v| v.as_str()).unwrap_or("");
            if !sql.is_empty() {
                let truncated_sql = if sql.len() > 35 {
                    format!("{}...", sql.chars().take(32).collect::<String>())
                } else {
                    sql.to_string()
                };
                format!("db: \"{}\", sql: \"{}\"", db_filename, truncated_sql)
            } else {
                let action = map.get("action").and_then(|v| v.as_str()).unwrap_or("");
                format!("db: \"{}\", action: \"{}\"", db_filename, action)
            }
        } else {
            let mut parts = Vec::new();
            for (k, v) in map {
                if k == "session_key" || k == "session_id" {
                    continue;
                }
                let val_str = match v {
                    serde_json::Value::String(s) => {
                        if s.len() > 20 {
                            format!("\"{}...\"", s.chars().take(17).collect::<String>())
                        } else {
                            format!("\"{}\"", s)
                        }
                    }
                    other => {
                        let os = other.to_string();
                        if os.len() > 20 {
                            format!("{}...", os.chars().take(17).collect::<String>())
                        } else {
                            os
                        }
                    }
                };
                parts.push(format!("{}: {}", k, val_str));
            }
            let joined = parts.join(", ");
            if joined.len() > 50 {
                format!("{}...", joined.chars().take(47).collect::<String>())
            } else {
                joined
            }
        }
    } else {
        let as_str = args.to_string();
        if as_str.len() > 50 {
            format!("{}...", as_str.chars().take(47).collect::<String>())
        } else {
            as_str
        }
    };

    if details.is_empty() {
        format!("{}{}{}", COLOR_BOLD, friendly_name, COLOR_RESET)
    } else {
        format!(
            "{}{}{}({})",
            COLOR_BOLD, friendly_name, COLOR_RESET, details
        )
    }
}

pub(crate) async fn send_progress_update(session_key: &str, text: &str) {
    let actual_session = crate::agent::style::spinner::get_current_session_key()
        .unwrap_or_else(|| session_key.to_string());
    if actual_session.starts_with("telegram:") {
        if let Some(chat_id_str) = actual_session.strip_prefix("telegram:") {
            if let Ok(chat_id) = chat_id_str.parse::<i64>() {
                if let Some((bot_token, client)) =
                    crate::channels::telegram::get_telegram_bot_info()
                {
                    let send_url = format!("https://api.telegram.org/bot{}/sendMessage", bot_token);
                    let payload = serde_json::json!({
                        "chat_id": chat_id,
                        "text": text,
                        "parse_mode": "Markdown"
                    });
                    let _ = client.post(&send_url).json(&payload).send().await;
                }
            }
        }
    } else if actual_session.starts_with("discord:") {
        if let Some(channel_id) = actual_session.strip_prefix("discord:") {
            if let Some((bot_token, client)) = crate::channels::discord::get_discord_bot_info() {
                let send_url = format!(
                    "https://discord.com/api/v10/channels/{}/messages",
                    channel_id
                );
                let payload = serde_json::json!({
                    "content": text
                });
                let _ = client
                    .post(&send_url)
                    .header("Authorization", format!("Bot {}", bot_token))
                    .json(&payload)
                    .send()
                    .await;
            }
        }
    } else if actual_session.starts_with("whatsapp:") {
        if let Some(phone_number) = actual_session.strip_prefix("whatsapp:") {
            if let Some((api_key, phone_number_id, client)) =
                crate::channels::whatsapp::get_whatsapp_bot_info()
            {
                let send_url = format!(
                    "https://graph.facebook.com/v18.0/{}/messages",
                    phone_number_id
                );
                let payload = serde_json::json!({
                    "messaging_product": "whatsapp",
                    "recipient_type": "individual",
                    "to": phone_number,
                    "type": "text",
                    "text": {
                        "body": text
                    }
                });
                let _ = client
                    .post(&send_url)
                    .bearer_auth(api_key)
                    .json(&payload)
                    .send()
                    .await;
            }
        }
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
    if !silent
        && !crate::agent::style::is_profile_subagent(&call.name)
        && call.name != "parallel_research"
    {
        let leaf_prefix = crate::agent::style::get_tree_prefix(true);
        let summary =
            crate::agent::style::format_tool_outcome_summary(&call.name, &call.arguments, &result);
        if call.name == "write_file" || call.name == "patch_file" || call.name == "replace_lines" {
            crate::tui_println!("{}{}{}{}", AURA_SLATE, leaf_prefix, COLOR_RESET, summary);
        } else if summary.contains('\u{2713}') || summary.contains('\u{2715}') {
            crate::tui_println!("{}{}{}{}", AURA_SLATE, leaf_prefix, COLOR_RESET, summary);
        } else {
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
    if !silent
        && !crate::agent::style::is_profile_subagent(&call.name)
        && call.name != "parallel_research"
    {
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
    if !silent {
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
    error_value_with_hint(&call.name, &error_str)
}

fn error_value_with_hint(tool_name: &str, error_str: &str) -> serde_json::Value {
    let hint = super::loop_control::generate_self_healing_hint(tool_name, error_str);
    if let Ok(mut json_err) = serde_json::from_str::<serde_json::Value>(error_str) {
        if let serde_json::Value::Object(ref mut map) = json_err {
            if !map.contains_key("self_healing_suggestion") && !map.contains_key("suggestion") {
                map.insert(
                    "self_healing_suggestion".to_string(),
                    serde_json::Value::String(hint),
                );
            }
            json_err
        } else {
            serde_json::json!({
                "error": error_str,
                "self_healing_suggestion": hint
            })
        }
    } else {
        serde_json::json!({
            "error": error_str,
            "self_healing_suggestion": hint
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn html_to_video_formatter_shows_timeline_cost() {
        let formatted = format_tool_args(
            "html_to_video",
            &json!({
                "html_path": "/tmp/intro.html",
                "output_path": "/tmp/intro.mp4",
                "duration_seconds": 30,
                "fps": 30
            }),
        );
        assert!(formatted.contains("duration: 30s"));
        assert!(formatted.contains("fps: 30"));
        assert!(formatted.contains("frames: 900"));
    }
}
