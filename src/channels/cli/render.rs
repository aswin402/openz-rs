use crate::agent::style::*;
use crate::println;
use std::io::{stdout, Write};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

pub use crate::agent::style::markdown::{
    char_display_width, clean_cell_text as shared_clean_cell_text, compute_table_column_widths,
    horizontal_divider, is_divider_row, is_table_row, provider_display_name, split_row,
    str_display_width, terminal_width_or_default, text_display_width, tokenize_inline_markdown,
    wrap_display_text as wrap_text, InlineSegment, OPENZ_BANNER_PARTS,
};

pub static CUSTOM_CONTEXT_LIMIT: Mutex<Option<usize>> = Mutex::new(None);
static MCP_SPIN: AtomicU32 = AtomicU32::new(0);

pub const SLASH_COMMANDS: &[(&str, &str)] = &[
    ("/clear", "Clear screen"),
    ("/device", "Manage local app/device inventory"),
    ("/exit", "Exit OpenZ"),
    ("/help", "List slash commands"),
    ("/history", "Restore/switch sessions using selection menu"),
    ("/logs", "Show recent logs"),
    ("/mcps", "List configured MCP servers"),
    ("/memory", "View metadata memory"),
    ("/model", "Show or change active default model"),
    ("/new-session", "Start a new session"),
    ("/servers", "List OpenZ-launched background servers"),
    ("/settings", "Show active settings"),
    ("/skill", "List active skills"),
    ("/sources", "Search saved source bookmarks"),
    ("/stop-server", "Stop a background server by id, or all"),
    ("/streaming", "Manage response streaming mode"),
    ("/tui", "Manage TUI display settings"),
    ("/workflows", "Search reusable workflows"),
];

fn colorize_status_glyphs(text: &str) -> String {
    text.replace('✔', &format!("{}✔{}", EMERALD_GREEN, COLOR_RESET))
        .replace('✅', &format!("{}✅{}", EMERALD_GREEN, COLOR_RESET))
        .replace('✓', &format!("{}✓{}", EMERALD_GREEN, COLOR_RESET))
        .replace('✖', &format!("{}✖{}", ERROR_RED, COLOR_RESET))
        .replace('❌', &format!("{}❌{}", ERROR_RED, COLOR_RESET))
        .replace('✗', &format!("{}✗{}", ERROR_RED, COLOR_RESET))
}

fn format_cell_text(text: &str) -> String {
    tokenize_inline_markdown(&colorize_status_glyphs(text))
        .into_iter()
        .map(|seg| match seg {
            InlineSegment::Plain(s) => s,
            InlineSegment::Bold(s) => format!("{}{}{}{}", RED_ORANGE, COLOR_BOLD, s, COLOR_RESET),
            InlineSegment::Code(s) | InlineSegment::Italic(s) => {
                format!("{}{}{}", LIGHT_BLUE, s, COLOR_RESET)
            }
        })
        .collect::<Vec<String>>()
        .join("")
}

pub fn format_markdown_line(line: &str) -> String {
    let trimmed = line.trim();
    let is_hr = trimmed.len() >= 3 && trimmed.chars().all(|c| c == '-');
    let is_heading = line.trim_start().starts_with('#');
    match (is_hr, is_heading) {
        (true, _) => format!("{}──────{}", LIGHT_WHITE, COLOR_RESET),
        (false, true) => format!("{}{}{}", HEADING_BLUE, line, COLOR_RESET),
        (false, false) => format_cell_text(line),
    }
}

pub fn print_normal_line(line: &str) {
    let formatted = format_markdown_line(line);
    std::print!("{}\r\n", formatted);
    let _ = std::io::stdout().flush();
}

pub fn render_table_lines(table_lines: &[&str]) -> Vec<String> {
    let headers: Vec<String> = table_lines
        .first()
        .map(|first| {
            split_row(first)
                .into_iter()
                .map(|h| shared_clean_cell_text(&h, true))
                .collect()
        })
        .unwrap_or_default();
    let num_cols = headers.len();

    match (table_lines.len() >= 2, num_cols > 0) {
        (false, _) | (_, false) => table_lines.iter().map(|l| format_markdown_line(l)).collect(),
        (true, true) => {
            let data_rows: Vec<Vec<String>> = table_lines[2..]
                .iter()
                .map(|&line| {
                    let mut cells = split_row(line);
                    cells.resize(num_cols, String::new());
                    cells
                })
                .collect();

            let term_width = terminal_width_or_default(80);
            let separator_overhead = 3 * (num_cols - 1);
            let available_width = term_width
                .saturating_sub(separator_overhead)
                .saturating_sub(2);

            let max_content_widths: Vec<usize> = (0..num_cols)
                .map(|col| {
                    data_rows
                        .iter()
                        .map(|row| text_display_width(&row[col]))
                        .fold(text_display_width(&headers[col]), usize::max)
                        .max(3)
                })
                .collect();

            let col_widths = compute_table_column_widths(&max_content_widths, available_width, 10);

            let divider_segments: Vec<String> = (0..num_cols)
                .map(|col| {
                    let extra_pad = match col == 0 || col == num_cols - 1 {
                        true => 1,
                        false => 2,
                    };
                    "─".repeat(col_widths[col] + extra_pad)
                })
                .collect();
            let divider = divider_segments.join("┼");

            let separator = format!(" {}│{} ", LIGHT_WHITE, COLOR_RESET);
            let divider_colored = format!("{}{}{}", LIGHT_WHITE, divider, COLOR_RESET);

            let header_cell_lines: Vec<Vec<String>> = (0..num_cols)
                .map(|col| wrap_text(&headers[col], col_widths[col]))
                .collect();
            let max_header_lines = header_cell_lines
                .iter()
                .map(Vec::len)
                .max()
                .unwrap_or(1)
                .max(1);

            let mut result_lines = Vec::new();
            for line_idx in 0..max_header_lines {
                let header_line_parts: Vec<String> = (0..num_cols)
                    .map(|col| {
                        let text = header_cell_lines[col]
                            .get(line_idx)
                            .cloned()
                            .unwrap_or_default();
                        let visible_w = text_display_width(&text);
                        let padding_len = col_widths[col].saturating_sub(visible_w);
                        let formatted = format_cell_text(&text);
                        format!(
                            "{}{}{}{}{}",
                            HEADING_BLUE,
                            COLOR_BOLD,
                            formatted,
                            COLOR_RESET,
                            " ".repeat(padding_len)
                        )
                    })
                    .collect();
                result_lines.push(header_line_parts.join(&separator));
            }

            result_lines.push(divider_colored);

            for row in data_rows {
                let cell_lines: Vec<Vec<String>> = (0..num_cols)
                    .map(|col| wrap_text(&row[col], col_widths[col]))
                    .collect();
                let max_lines = cell_lines.iter().map(Vec::len).max().unwrap_or(1).max(1);

                for line_idx in 0..max_lines {
                    let row_line_parts: Vec<String> = (0..num_cols)
                        .map(|col| {
                            let text = cell_lines[col].get(line_idx).cloned().unwrap_or_default();
                            let visible_w = text_display_width(&text);
                            let padding_len = col_widths[col].saturating_sub(visible_w);
                            let formatted = format_cell_text(&text);
                            format!("{}{}", formatted, " ".repeat(padding_len))
                        })
                        .collect();
                    result_lines.push(row_line_parts.join(&separator));
                }
            }

            result_lines
        }
    }
}

pub fn render_table(table_lines: &[&str]) {
    for line in render_table_lines(table_lines) {
        std::print!("{}\r\n", line);
    }
    let _ = std::io::stdout().flush();
}

pub fn print_session_history(session: &crate::session::Session) {
    (!session.messages.is_empty()).then(|| {
        println!("{}=== Session History ==={}", COLOR_BOLD, COLOR_RESET);
        let divider = horizontal_divider(terminal_width_or_default(60).min(60));

        let mut tool_calls_map = std::collections::HashMap::new();
        for msg in &session.messages {
            msg.extra
                .get("tool_calls")
                .and_then(|v| v.as_array())
                .map(|tool_calls| {
                    for tc in tool_calls {
                        let id_opt = tc.get("id").and_then(|v| v.as_str());
                        let name_opt = tc
                            .get("function")
                            .and_then(|f| f.get("name"))
                            .and_then(|v| v.as_str());
                        id_opt.zip(name_opt).map(|(id, name)| {
                            let args = tc
                                .get("function")
                                .and_then(|f| f.get("arguments"))
                                .cloned()
                                .unwrap_or(serde_json::Value::Null);
                            tool_calls_map.insert(id.to_string(), (name.to_string(), args));
                        });
                    }
                });
        }

        let mut first_user = true;
        for msg in &session.messages {
            let content = msg.content.trim();
            match msg.role.as_str() {
                "user" => {
                    (!content.is_empty()).then(|| {
                        (!first_user).then(|| {
                            println!("{}{}{}", LIGHT_WHITE, divider, COLOR_RESET);
                        });
                        first_user = false;
                        for (idx, line) in content.lines().enumerate() {
                            let prefix = match idx == 0 {
                                true => "> ",
                                false => "- ",
                            };
                            println!("{}{}{}{}", COLOR_BOLD, AURA_SLATE, prefix, line);
                        }
                    });
                }
                "assistant" => {
                    msg.extra
                        .get("reasoning_content")
                        .and_then(|v| v.as_str())
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(|trimmed_reasoning| {
                            println!("{}• {}Thought{}", RED_ORANGE, COLOR_BOLD, COLOR_RESET);
                            crate::agent::style::print_tree_monologue("  L ", trimmed_reasoning);
                            println!();
                        });

                    (!content.is_empty()).then(|| {
                        print_colored_markdown(content);
                        println!();
                    });
                }
                "tool" => {
                    let tool_call_id = msg
                        .extra
                        .get("tool_call_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let leaf_prefix = "  L ";

                    match tool_calls_map.get(tool_call_id) {
                        Some((name, args_val)) => {
                            let skip = crate::agent::style::is_profile_subagent(name)
                                || name == "parallel_research";
                            (!skip).then(|| {
                                let parsed_args = args_val
                                    .as_str()
                                    .and_then(|s| serde_json::from_str(s).ok())
                                    .unwrap_or_else(|| args_val.clone());

                                let formatted_args =
                                    crate::agent::agent_loop::tool_execution::format_tool_args(
                                        name,
                                        &parsed_args,
                                    );
                                let title = crate::agent::style::get_tool_clean_name(name);
                                let details =
                                    crate::agent::style::clean_tool_args_msg(name, &formatted_args);

                                match details.is_empty() {
                                    true => println!(
                                        "{}• {}{}{}{}{}",
                                        RED_ORANGE,
                                        COLOR_RESET,
                                        COLOR_BOLD,
                                        LIGHT_WHITE,
                                        title,
                                        COLOR_RESET
                                    ),
                                    false => println!(
                                        "{}• {}{}{}{} {}{}{}{}",
                                        RED_ORANGE,
                                        COLOR_RESET,
                                        COLOR_BOLD,
                                        LIGHT_WHITE,
                                        title,
                                        COLOR_RESET,
                                        AURA_SLATE,
                                        details,
                                        COLOR_RESET
                                    ),
                                }

                                crate::agent::security::SecurityGuard::is_sensitive(
                                    name,
                                    &parsed_args,
                                )
                                .then(|| {
                                    println!(
                                        "{}> Authorize execution?: {}{}Approve (Allow once){}",
                                        COLOR_BOLD, COLOR_RESET, RED_ORANGE, COLOR_RESET
                                    );
                                });

                                let outcome_val: serde_json::Value = serde_json::from_str(content)
                                    .unwrap_or_else(|_| {
                                        serde_json::json!({
                                            "status": "success",
                                            "output": content
                                        })
                                    });

                                let summary = crate::agent::style::format_tool_outcome_summary(
                                    name,
                                    &parsed_args,
                                    &outcome_val,
                                );

                                let has_symbol = summary.contains('\u{2713}')
                                    || summary.contains('\u{2715}')
                                    || matches!(
                                        name.as_str(),
                                        "write_file" | "patch_file" | "replace_lines"
                                    );
                                match has_symbol {
                                    true => {
                                        println!(
                                            "{}{}{}{}",
                                            AURA_SLATE, leaf_prefix, COLOR_RESET, summary
                                        );
                                    }
                                    false => {
                                        println!(
                                            "{}{}{}✓ {}{}",
                                            AURA_SLATE,
                                            leaf_prefix,
                                            AURA_GREEN,
                                            summary,
                                            COLOR_RESET
                                        );
                                    }
                                }
                            });
                        }
                        None => {
                            let name = msg
                                .extra
                                .get("name")
                                .and_then(|v| v.as_str())
                                .unwrap_or("Tool");
                            let summary = match content.len() > 100 {
                                true => format!("{}...", &content[..100]),
                                false => content.to_string(),
                            };
                            println!(
                                "{}{}{}✓ {}: {}{}",
                                AURA_SLATE, leaf_prefix, AURA_GREEN, name, summary, COLOR_RESET
                            );
                        }
                    }
                }
                other => {
                    (!content.is_empty()).then(|| {
                        println!(
                            "{}{}:{}{} {}",
                            AURA_SLATE, other, COLOR_BOLD, COLOR_RESET, content
                        );
                    });
                }
            }
        }

        println!("{}{}{}", LIGHT_WHITE, divider, COLOR_RESET);
    });
}

#[derive(Debug, Clone)]
enum StreamBlockState {
    Normal,
    CodeBlock,
    PendingTable(String),
    InTable(Vec<String>),
}

#[derive(Debug, Clone)]
pub struct StreamingMarkdownRenderer {
    current_line: String,
    state: StreamBlockState,
    silent: bool,
    started: bool,
    captured_lines: Option<Vec<String>>,
}

impl StreamingMarkdownRenderer {
    pub fn new(silent: bool) -> Self {
        Self {
            current_line: String::new(),
            state: StreamBlockState::Normal,
            silent,
            started: false,
            captured_lines: None,
        }
    }

    pub fn new_with_capture() -> Self {
        Self {
            current_line: String::new(),
            state: StreamBlockState::Normal,
            silent: true,
            started: false,
            captured_lines: Some(Vec::new()),
        }
    }

    pub fn has_started(&self) -> bool {
        self.started
    }

    pub fn current_buffer(&self) -> &str {
        &self.current_line
    }

    pub fn captured(&self) -> &[String] {
        self.captured_lines.as_deref().unwrap_or(&[])
    }

    pub fn push_chunk(&mut self, text: &str) {
        let should_process = !(self.silent && self.captured_lines.is_none()) && !text.is_empty();
        should_process.then(|| {
            self.started = true;
            for c in text.chars() {
                match c {
                    '\r' => {}
                    '\n' => {
                        let line = std::mem::take(&mut self.current_line);
                        self.process_completed_line(line);
                    }
                    other => self.current_line.push(other),
                }
            }
        });
    }

    fn process_completed_line(&mut self, line: String) {
        let is_fence = line.trim_start().starts_with("```");
        let is_div = is_divider_row(&line);
        let is_row = is_table_row(&line);
        let prev_state = std::mem::replace(&mut self.state, StreamBlockState::Normal);

        match (prev_state, is_fence, is_div, is_row) {
            (StreamBlockState::CodeBlock, true, _, _) => {
                self.output_raw_line(&line);
                self.state = StreamBlockState::Normal;
            }
            (StreamBlockState::CodeBlock, false, _, _) => {
                self.output_raw_line(&line);
                self.state = StreamBlockState::CodeBlock;
            }
            (StreamBlockState::Normal, true, _, _) => {
                self.output_raw_line(&line);
                self.state = StreamBlockState::CodeBlock;
            }
            (StreamBlockState::PendingTable(header), true, _, _) => {
                self.output_line(&header);
                self.output_raw_line(&line);
                self.state = StreamBlockState::CodeBlock;
            }
            (StreamBlockState::InTable(rows), true, _, _) => {
                self.emit_table_rows(&rows);
                self.output_raw_line(&line);
                self.state = StreamBlockState::CodeBlock;
            }
            (StreamBlockState::InTable(mut rows), false, _, true) => {
                rows.push(line);
                self.state = StreamBlockState::InTable(rows);
            }
            (StreamBlockState::InTable(rows), false, _, false) => {
                self.emit_table_rows(&rows);
                self.state = StreamBlockState::Normal;
                self.process_completed_line(line);
            }
            (StreamBlockState::PendingTable(header), false, true, _) => {
                self.state = StreamBlockState::InTable(vec![header, line]);
            }
            (StreamBlockState::PendingTable(header), false, false, true) => {
                self.output_line(&header);
                self.state = StreamBlockState::PendingTable(line);
            }
            (StreamBlockState::PendingTable(header), false, false, false) => {
                self.output_line(&header);
                self.output_line(&line);
                self.state = StreamBlockState::Normal;
            }
            (StreamBlockState::Normal, false, _, true) => {
                self.state = StreamBlockState::PendingTable(line);
            }
            (StreamBlockState::Normal, false, _, false) => {
                self.output_line(&line);
                self.state = StreamBlockState::Normal;
            }
        }
    }

    fn emit_table_rows(&mut self, rows: &[String]) {
        (!rows.is_empty()).then(|| {
            let refs: Vec<&str> = rows.iter().map(String::as_str).collect();
            for rendered in render_table_lines(&refs) {
                self.captured_lines
                    .as_mut()
                    .map(|cap| cap.push(rendered.clone()));
                (!self.silent).then(|| {
                    std::print!("{}\r\n", rendered);
                });
            }
            (!self.silent).then(|| {
                let _ = std::io::stdout().flush();
            });
        });
    }

    fn output_line(&mut self, line: &str) {
        let formatted = format_markdown_line(line);
        self.captured_lines
            .as_mut()
            .map(|cap| cap.push(formatted.clone()));
        (!self.silent).then(|| {
            std::print!("{}\r\n", formatted);
            let _ = std::io::stdout().flush();
        });
    }

    fn output_raw_line(&mut self, line: &str) {
        self.captured_lines
            .as_mut()
            .map(|cap| cap.push(line.to_string()));
        (!self.silent).then(|| {
            std::print!("{}\r\n", line);
            let _ = std::io::stdout().flush();
        });
    }

    pub fn finish(&mut self) {
        (!self.current_line.is_empty()).then(|| {
            let line = std::mem::take(&mut self.current_line);
            self.process_completed_line(line);
        });
        match std::mem::replace(&mut self.state, StreamBlockState::Normal) {
            StreamBlockState::InTable(rows) => self.emit_table_rows(&rows),
            StreamBlockState::PendingTable(header) => self.output_line(&header),
            StreamBlockState::Normal | StreamBlockState::CodeBlock => {}
        }
    }
}

pub fn print_colored_markdown(content: &str) {
    let mut streamer = StreamingMarkdownRenderer::new(false);
    streamer.push_chunk(content);
    streamer.finish();
}

pub static LAST_CURSOR_LINE: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

pub fn get_last_cursor_line() -> usize {
    LAST_CURSOR_LINE.load(Ordering::Relaxed)
}

pub fn clear_input_rendering(lines_printed: usize) {
    let prev_cursor_line = get_last_cursor_line();
    (prev_cursor_line > 0).then(|| {
        std::print!("\x1b[{}A\r", prev_cursor_line);
    });
    (lines_printed > 1).then(|| {
        for _ in 0..(lines_printed - 1) {
            std::print!("\r\n\x1b[2K");
        }
        std::print!("\x1b[{}A\r", lines_printed - 1);
    });
    std::print!("\r\x1b[2K");
}

fn format_mcp_status_pill() -> (String, String) {
    const SPIN_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let mcp_done = super::mcp::is_mcp_done();
    let (mcp_loaded, mcp_failed, mcp_total) = super::mcp::get_mcp_stats();

    match (mcp_done, mcp_failed > 0, mcp_loaded > 0, mcp_total > 0) {
        (false, _, _, true) => {
            let frame_idx = MCP_SPIN.fetch_add(1, Ordering::Relaxed) as usize % SPIN_FRAMES.len();
            let frame = SPIN_FRAMES[frame_idx];
            let processed = mcp_loaded + mcp_failed;
            (
                format!(" mcp {}/{} {}  │ ", processed, mcp_total, frame),
                format!(
                    " {}mcp {}{}/{} {}{}  {}│{} ",
                    AURA_SLATE,
                    AURA_GOLD,
                    processed,
                    mcp_total,
                    frame,
                    COLOR_RESET,
                    AURA_SLATE,
                    COLOR_RESET
                ),
            )
        }
        (false, _, _, false) => {
            let frame_idx = MCP_SPIN.fetch_add(1, Ordering::Relaxed) as usize % SPIN_FRAMES.len();
            let frame = SPIN_FRAMES[frame_idx];
            (
                format!(" mcp {}  │ ", frame),
                format!(
                    " {}mcp {}{}  {}│{} ",
                    AURA_SLATE, frame, COLOR_RESET, AURA_SLATE, COLOR_RESET
                ),
            )
        }
        (true, false, false, _) => (
            " mcp 0  │ ".to_string(),
            format!(
                " {}mcp 0{}  {}│{} ",
                AURA_SLATE, COLOR_RESET, AURA_SLATE, COLOR_RESET
            ),
        ),
        (true, false, true, _) => (
            format!(" mcp {}  │ ", mcp_loaded),
            format!(
                " {}mcp {}{}{}  {}│{} ",
                AURA_SLATE, AURA_GREEN, mcp_loaded, COLOR_RESET, AURA_SLATE, COLOR_RESET
            ),
        ),
        (true, true, _, _) => (
            format!(" mcp {} ({}✗)  │ ", mcp_loaded, mcp_failed),
            format!(
                " {}mcp {}{}{} ({}{}{}✗)  {}│{} ",
                AURA_SLATE,
                AURA_GREEN,
                mcp_loaded,
                COLOR_RESET,
                AURA_ROSE,
                mcp_failed,
                COLOR_RESET,
                AURA_SLATE,
                COLOR_RESET
            ),
        ),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn render_box(
    model: &str,
    provider: &str,
    session_manager: &crate::session::SessionManager,
    session_key: &str,
    typed_input: &str,
    cursor_idx: usize,
    _viewport_start: &mut usize,
    selected_index: Option<usize>,
    autocomplete_visible: bool,
    width: usize,
    lines_printed: &mut usize,
) -> anyhow::Result<()> {
    clear_input_rendering(*lines_printed);

    let session = session_manager.get_or_create(session_key);
    let total_chars: usize = session.messages.iter().map(|m| m.content.len()).sum();
    let approx_tokens = total_chars / 4;

    let custom_limit = CUSTOM_CONTEXT_LIMIT.lock().ok().and_then(|guard| *guard);
    let limit_tokens = custom_limit.unwrap_or_else(|| {
        crate::providers::DynamicContextRegistry::resolve_context_window(
            model,
            &crate::config::schema::Config::default(),
        )
    });

    let limit_str = match limit_tokens >= 1_000_000 {
        true => format!("{}M", limit_tokens / 1_000_000),
        false => format!("{}K", limit_tokens / 1000),
    };

    let approx_tokens_str = match approx_tokens >= 1000 {
        true => format!("{:.1}K", approx_tokens as f64 / 1000.0),
        false => format!("{}", approx_tokens),
    };

    let display_provider = provider_display_name(provider);

    let max_model_len = match (width < 60, width < 80) {
        (true, _) => 10,
        (false, true) => 16,
        (false, false) => 30,
    };

    let display_model = match model.chars().count() > max_model_len {
        true => {
            let truncated: String = model.chars().take(max_model_len - 1).collect();
            format!("{}…", truncated)
        }
        false => model.to_string(),
    };

    let (mcp_pill_plain, mcp_pill_colored) = format_mcp_status_pill();
    let pill_plain_len = mcp_pill_plain.chars().count();

    let visible_status_len = pill_plain_len
        + display_provider.chars().count()
        + display_model.chars().count()
        + approx_tokens_str.chars().count()
        + limit_str.chars().count()
        + 11;

    let fill_chars = width.saturating_sub(visible_status_len);
    let line_fill: String = std::iter::repeat_n('─', fill_chars).collect();

    let status_content = format!(
        "{} {}{}{} | {}{}{} | {}{}{}/{}{}",
        mcp_pill_colored,
        RED_ORANGE,
        display_provider,
        LIGHT_WHITE,
        RED_ORANGE,
        display_model,
        LIGHT_WHITE,
        RED_ORANGE,
        approx_tokens_str,
        LIGHT_WHITE,
        RED_ORANGE,
        limit_str
    );

    let matches: Vec<(&str, &str)> = (typed_input.starts_with('/') && !typed_input.contains(' '))
        .then(|| {
            SLASH_COMMANDS
                .iter()
                .copied()
                .filter(|&(cmd, _)| cmd.starts_with(typed_input))
                .collect()
        })
        .unwrap_or_default();

    let status_line = match autocomplete_visible && !matches.is_empty() {
        true => {
            let full_fill: String = std::iter::repeat_n('─', width).collect();
            format!("{}{}{}", LIGHT_WHITE, full_fill, COLOR_RESET)
        }
        false => format!(
            "{}{}{}[{}]{}{}",
            LIGHT_WHITE, line_fill, RED_ORANGE, status_content, LIGHT_WHITE, COLOR_RESET
        ),
    };

    let display_text = selected_index
        .and_then(|idx| matches.get(idx).map(|&(cmd, _)| cmd))
        .unwrap_or(typed_input);

    let display_chars: Vec<char> = display_text.chars().collect();
    let char_count = display_chars.len();

    let active_cursor_idx = match selected_index.is_some() {
        true => char_count,
        false => cursor_idx.min(char_count),
    };

    let prompt_prefix_width = 2;
    let max_line_content_width = width.saturating_sub(prompt_prefix_width).max(1);

    let mut input_lines: Vec<String> = Vec::new();
    let mut current_chars = String::new();
    let mut current_width = 0;
    let mut cursor_line_idx = 0;
    let mut cursor_col_offset = 0;

    for (idx, &c) in display_chars.iter().enumerate() {
        (idx == active_cursor_idx).then(|| {
            cursor_line_idx = input_lines.len();
            cursor_col_offset = current_width;
        });

        let cw = char_display_width(c);
        let should_wrap =
            c == '\n' || (current_width + cw > max_line_content_width && !current_chars.is_empty());
        should_wrap.then(|| {
            input_lines.push(std::mem::take(&mut current_chars));
            current_width = 0;
        });

        (c != '\n').then(|| {
            current_chars.push(c);
            current_width += cw;
        });
    }

    (active_cursor_idx == display_chars.len()).then(|| {
        cursor_line_idx = input_lines.len();
        cursor_col_offset = current_width;
    });

    input_lines.push(current_chars);
    LAST_CURSOR_LINE.store(cursor_line_idx, Ordering::Relaxed);

    let mut new_lines_printed: usize = 0;
    for (line_idx, line_content) in input_lines.iter().enumerate() {
        let (lead, prefix_str) = match line_idx == 0 {
            true => ("", "> "),
            false => ("\r\n\x1b[2K", "- "),
        };
        std::print!(
            "{}{}{}{}{}",
            lead,
            LIGHT_WHITE,
            prefix_str,
            COLOR_RESET,
            line_content
        );
        new_lines_printed += 1;
    }

    std::print!("\r\n\x1b[2K{}", status_line);
    new_lines_printed += 1;

    (autocomplete_visible && !matches.is_empty()).then(|| {
        let max_display = 5usize;
        let start_idx = selected_index
            .filter(|&idx| idx >= max_display)
            .map(|idx| idx - max_display + 1)
            .unwrap_or(0);
        let end_idx = (start_idx + max_display).min(matches.len());

        for (i, &(cmd, desc)) in matches.iter().enumerate().take(end_idx).skip(start_idx) {
            match selected_index == Some(i) {
                true => std::print!(
                    "\r\n\x1b[2K> {}{:<30}{}{}{}",
                    RED_ORANGE,
                    cmd,
                    AURA_SLATE,
                    desc,
                    COLOR_RESET
                ),
                false => std::print!(
                    "\r\n\x1b[2K  {:<30}{}{}{}",
                    cmd,
                    AURA_SLATE,
                    desc,
                    COLOR_RESET
                ),
            }
            new_lines_printed += 1;
        }

        let rem_below = matches.len() - end_idx;
        let rem_above = start_idx;
        let more_parts: Vec<String> = [
            (rem_above > 0).then(|| format!("↑ {} more", rem_above)),
            (rem_below > 0).then(|| format!("↓ {} more", rem_below)),
        ]
        .into_iter()
        .flatten()
        .collect();

        (!more_parts.is_empty()).then(|| {
            std::print!(
                "\r\n\x1b[2K  {}{}{}",
                AURA_SLATE,
                more_parts.join(" / "),
                COLOR_RESET
            );
            new_lines_printed += 1;
        });

        std::print!("\r\n\x1b[2K");
        new_lines_printed += 1;

        std::print!(
            "\r\n\x1b[2K  {}↑/↓ Navigate · enter Select · tab Complete{}",
            AURA_SLATE,
            COLOR_RESET
        );

        let cancel_text = format!("  {}esc to cancel{}", AURA_SLATE, COLOR_RESET);
        let cancel_width = 15;
        let model_display = format!("{}{}{}", AURA_SLATE, model, COLOR_RESET);
        let model_width = model.chars().count();
        let spacing = width.saturating_sub(cancel_width + model_width);
        let spaces: String = std::iter::repeat_n(' ', spacing).collect();

        std::print!("\r\n\x1b[2K{}{}{}", cancel_text, spaces, model_display);
        new_lines_printed += 2;
    });

    let up_lines = new_lines_printed.saturating_sub(1 + cursor_line_idx);
    let cursor_col = 3 + cursor_col_offset;

    match up_lines > 0 {
        true => std::print!("\x1b[{}A\x1b[{}G", up_lines, cursor_col),
        false => std::print!("\x1b[{}G", cursor_col),
    }

    let _ = crossterm::execute!(stdout(), crossterm::cursor::Show);

    *lines_printed = new_lines_printed;
    stdout().flush()?;
    Ok(())
}

#[cfg(test)]
#[path = "render_tests.rs"]
mod tests;
