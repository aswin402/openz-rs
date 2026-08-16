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

fn string_arg<'a>(
    map: &'a serde_json::Map<String, serde_json::Value>,
    keys: &[&str],
) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| map.get(*key).and_then(|value| value.as_str()))
}

fn filename_arg(map: &serde_json::Map<String, serde_json::Value>, keys: &[&str]) -> Option<String> {
    string_arg(map, keys).map(|path| {
        std::path::Path::new(path)
            .file_name()
            .map(|filename| filename.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string())
    })
}

// Canonical argument alias sets for display formatting. New tools should use
// the first entry (snake_case) as their primary argument name; the remaining
// entries are legacy aliases kept for compatibility with existing prompts.
const PATH_KEYS: &[&str] = &["path", "file_path", "filePath", "TargetFile", "filepath", "file"];
const COMMAND_KEYS: &[&str] = &["command", "Command", "CommandLine", "command_line"];
const OUTPUT_KEYS: &[&str] = &["output_path", "outputPath", "OutputPath"];
const QUERY_KEYS: &[&str] = &["query", "Query"];
const URL_KEYS: &[&str] = &["url", "Url", "UrlContent"];

/// Truncate to at most `max` characters with a trailing ellipsis.
fn clip(s: &str, max: usize) -> String {
    if s.chars().count() > max {
        format!(
            "{}...",
            s.chars().take(max.saturating_sub(3)).collect::<String>()
        )
    } else {
        s.to_string()
    }
}

fn file_name_of(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

pub(crate) fn format_tool_args(name: &str, raw_args: &serde_json::Value) -> String {
    let args = raw_args.clone();
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

    let details = if let serde_json::Value::Object(map) = &args {
        match name {
            // Query-style tools
            "grep_search" | "web_search" | "semantic_search" => string_arg(map, QUERY_KEYS)
                .map(|q| format!("query: \"{}\"", clip(q, 35)))
                .unwrap_or_default(),
            // Filesystem-style tools — show just the target file name
            "read_file" | "view_file" | "write_file" | "write_to_file" | "replace_file_content"
            | "multi_replace_file_content" | "patch_file" | "replace_lines" | "list_dir" => {
                filename_arg(map, PATH_KEYS).unwrap_or_default()
            }
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
            "crawl" => map
                .get("url")
                .and_then(|v| v.as_str())
                .map(|url| format!("url: \"{}\"", clip(url, 30)))
                .unwrap_or_default(),
            "generate_image" => {
                let path = string_arg(map, OUTPUT_KEYS).unwrap_or("output.png");
                let filename = file_name_of(path);
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
            }
            "generate_video" => {
                let path = string_arg(map, OUTPUT_KEYS).unwrap_or("output.mp4");
                format!("output: \"{}\"", file_name_of(path))
            }
            "html_to_video" => {
                let html_path = string_arg(map, &["html_path", "htmlPath"]).unwrap_or("");
                let html_filename =
                    if html_path.starts_with("http://") || html_path.starts_with("https://") {
                        clip(html_path, 30)
                    } else {
                        file_name_of(html_path)
                    };
                let out_path = string_arg(map, OUTPUT_KEYS).unwrap_or("output.mp4");
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
                if elem_count > 0 {
                    format!(
                        "output: \"{}\", elements: {}, animations: {}",
                        filename, elem_count, anim_count
                    )
                } else {
                    format!("output: \"{}\"", filename)
                }
            }
            "obscura_browser" => {
                let url = map.get("url").and_then(|v| v.as_str()).unwrap_or("");
                let action = map
                    .get("action")
                    .and_then(|v| v.as_str())
                    .unwrap_or("render");
                format!("action: \"{}\", url: \"{}\"", action, clip(url, 30))
            }
            "gsd_browser" => {
                let action = map.get("action").and_then(|v| v.as_str()).unwrap_or("");
                let url = map.get("url").and_then(|v| v.as_str()).unwrap_or("");
                let ref_id = map.get("ref_id").and_then(|v| v.as_str()).unwrap_or("");
                if !url.is_empty() {
                    format!("action: \"{}\", url: \"{}\"", action, clip(url, 30))
                } else if !ref_id.is_empty() {
                    format!("action: \"{}\", ref_id: \"{}\"", action, ref_id)
                } else {
                    format!("action: \"{}\"", action)
                }
            }
            "doc_reader" => string_arg(map, PATH_KEYS)
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
            "db_inspector" | "db_write" => {
                let db_path = string_arg(map, &["db_path", "dbPath"]).unwrap_or("");
                let db_filename = file_name_of(db_path);
                let sql = map.get("sql").and_then(|v| v.as_str()).unwrap_or("");
                if !sql.is_empty() {
                    format!("db: \"{}\", sql: \"{}\"", db_filename, clip(sql, 35))
                } else {
                    let action = map.get("action").and_then(|v| v.as_str()).unwrap_or("");
                    format!("db: \"{}\", action: \"{}\"", db_filename, action)
                }
            }
            // Generic fallback — renders any tool's arguments without special casing.
            // New tools only need an arm above if they want custom truncation.
            _ => {
                let mut parts = Vec::new();
                for (k, v) in map {
                    if k == "session_key" || k == "session_id" {
                        continue;
                    }
                    let val_str = match v {
                        serde_json::Value::String(s) => format!("\"{}\"", clip(s, 20)),
                        other => clip(&other.to_string(), 20),
                    };
                    parts.push(format!("{}: {}", k, val_str));
                }
                clip(&parts.join(", "), 50)
            }
        }
    } else {
        clip(&args.to_string(), 50)
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
    fn format_tool_args_does_not_use_global_arg_normalizer() {
        let source = std::fs::read_to_string("src/agent/agent_loop/tool_execution.rs").unwrap();
        let production_source = source.split("#[cfg(test)]").next().unwrap_or(&source);
        assert!(
            !production_source.contains("normalize_tool_args(raw_args)"),
            "format_tool_args should not rely on global argument rewriting for display-only formatting"
        );
    }

    #[test]
    fn filesystem_formatter_reads_native_aliases_directly() {
        let formatted = format_tool_args(
            "replace_lines",
            &json!({
                "filePath": "/tmp/example.rs",
                "startLine": 1,
                "endLine": 1,
                "content": "updated"
            }),
        );
        assert!(formatted.contains("example.rs"));
    }

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
