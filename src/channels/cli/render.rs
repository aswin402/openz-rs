use crate::agent::style::*;
use crate::println;
use std::io::{stdout, Write};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};

pub static CUSTOM_CONTEXT_LIMIT: Mutex<Option<usize>> = Mutex::new(None);
static MCP_SPIN: AtomicU32 = AtomicU32::new(0);

pub const SLASH_COMMANDS: &[(&str, &str)] = &[
    ("/clear", "Clear screen"),
    ("/exit", "Exit OpenZ"),
    ("/help", "List slash commands"),
    ("/history", "Restore/switch sessions using selection menu"),
    ("/mcps", "List configured MCP servers"),
    ("/memory", "View metadata memory"),
    ("/model", "Show or change active default model"),
    ("/new-session", "Start a new session"),
    ("/skill", "List active skills"),
    ("/sources", "Search saved source bookmarks"),
    ("/workflows", "Search reusable workflows"),
];

fn char_display_width(c: char) -> usize {
    let cp = c as u32;
    if cp == 0xFE0F {
        0
    } else if (0x1F000..=0x1FBF9).contains(&cp)
        || c == '⬢'
        || c == '🗑'
        || c == '📊'
        || c == '✅'
        || c == '❌'
        || c == '⚠'
        || c == '⚡'
        || c == 'ℹ'
    {
        2
    } else {
        1
    }
}

fn str_display_width(s: &str) -> usize {
    s.chars().map(char_display_width).sum()
}

fn cli_re_ansi() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"\x1B\[[0-9;]*[a-zA-Z]").unwrap())
}

fn cli_re_bold() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"\*\*(.*?)\*\*").unwrap())
}

fn cli_re_code() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"`(.*?)`").unwrap())
}

fn cli_re_italic() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"\*(.*?)\*").unwrap())
}

fn text_display_width(text: &str) -> usize {
    let mut cleaned = text.to_string();
    cleaned = cli_re_ansi().replace_all(&cleaned, "").to_string();
    cleaned = cli_re_bold().replace_all(&cleaned, "$1").to_string();
    cleaned = cli_re_code().replace_all(&cleaned, "$1").to_string();
    cleaned = cli_re_italic().replace_all(&cleaned, "$1").to_string();
    str_display_width(&cleaned)
}

fn is_table_row(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.contains('|') && !is_divider_row(line)
}

fn is_divider_row(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() || !trimmed.contains('|') {
        return false;
    }
    trimmed
        .chars()
        .all(|c| c == '|' || c == '-' || c == ':' || c.is_whitespace())
}

fn split_row(line: &str) -> Vec<String> {
    let mut trimmed = line.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    if trimmed.starts_with('|') {
        trimmed = trimmed[1..].trim();
    }
    if trimmed.ends_with('|') {
        trimmed = trimmed[..trimmed.len() - 1].trim();
    }
    let mut cells = Vec::new();
    let mut current = String::new();
    let mut chars = trimmed.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(&'|') = chars.peek() {
                current.push('|');
                chars.next();
            } else {
                current.push('\\');
            }
        } else if c == '|' {
            cells.push(current.trim().to_string());
            current.clear();
        } else {
            current.push(c);
        }
    }
    cells.push(current.trim().to_string());
    cells
}

fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return vec![text.to_string()];
    }
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let mut current_line = String::new();
        let mut current_width = 0;

        for word in paragraph.split_whitespace() {
            let word_width = text_display_width(word);

            if current_line.is_empty() {
                if word_width <= max_width {
                    current_line.push_str(word);
                    current_width = word_width;
                } else {
                    let mut w_chars = word.chars().peekable();
                    while w_chars.peek().is_some() {
                        let mut chunk = String::new();
                        let mut chunk_w = 0;
                        while let Some(&c) = w_chars.peek() {
                            let cw = char_display_width(c);
                            if chunk_w + cw > max_width && chunk_w > 0 {
                                break;
                            }
                            chunk.push(c);
                            chunk_w += cw;
                            w_chars.next();
                        }
                        lines.push(chunk);
                    }
                }
            } else {
                let space_width = 1;
                if current_width + space_width + word_width <= max_width {
                    current_line.push(' ');
                    current_line.push_str(word);
                    current_width += space_width + word_width;
                } else {
                    lines.push(current_line);
                    current_line = String::new();
                    current_width = 0;

                    if word_width <= max_width {
                        current_line.push_str(word);
                        current_width = word_width;
                    } else {
                        let mut w_chars = word.chars().peekable();
                        while w_chars.peek().is_some() {
                            let mut chunk = String::new();
                            let mut chunk_w = 0;
                            while let Some(&c) = w_chars.peek() {
                                let cw = char_display_width(c);
                                if chunk_w + cw > max_width && chunk_w > 0 {
                                    break;
                                }
                                chunk.push(c);
                                chunk_w += cw;
                                w_chars.next();
                            }
                            if w_chars.peek().is_some() {
                                lines.push(chunk);
                            } else {
                                current_line = chunk;
                                current_width = chunk_w;
                            }
                        }
                    }
                }
            }
        }
        if !current_line.is_empty() {
            lines.push(current_line);
        }
        if lines.is_empty() && paragraph.is_empty() {
            lines.push(String::new());
        }
    }

    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

fn format_cell_text(text: &str) -> String {
    let light_blue = "\x1b[38;2;135;206;250m";
    let mut formatted = text.to_string();

    formatted = formatted
        .replace("✔", &format!("{}{}{}", EMERALD_GREEN, "✔", COLOR_RESET))
        .replace("✅", &format!("{}{}{}", EMERALD_GREEN, "✅", COLOR_RESET))
        .replace("✓", &format!("{}{}{}", EMERALD_GREEN, "✓", COLOR_RESET))
        .replace("✖", &format!("{}{}{}", ERROR_RED, "✖", COLOR_RESET))
        .replace("❌", &format!("{}{}{}", ERROR_RED, "❌", COLOR_RESET))
        .replace("✗", &format!("{}{}{}", ERROR_RED, "✗", COLOR_RESET));

    formatted = cli_re_bold()
        .replace_all(
            &formatted,
            &format!("{}{}$1{}", RED_ORANGE, COLOR_BOLD, COLOR_RESET),
        )
        .to_string();
    formatted = cli_re_code()
        .replace_all(&formatted, &format!("{}$1{}", light_blue, COLOR_RESET))
        .to_string();
    formatted = cli_re_italic()
        .replace_all(&formatted, &format!("{}$1{}", light_blue, COLOR_RESET))
        .to_string();

    formatted
}

fn print_normal_line(line: &str) {
    let trimmed = line.trim();
    if trimmed.chars().all(|c| c == '-') && trimmed.len() >= 3 && !trimmed.is_empty() {
        println!("{}──────{}", LIGHT_WHITE, COLOR_RESET);
        return;
    }

    let light_blue = "\x1b[38;2;135;206;250m";
    if line.trim_start().starts_with("#") {
        println!("{}{}{}", HEADING_BLUE, line, COLOR_RESET);
    } else {
        let mut formatted = line.to_string();

        formatted = formatted
            .replace("✔", &format!("{}{}{}", EMERALD_GREEN, "✔", COLOR_RESET))
            .replace("✅", &format!("{}{}{}", EMERALD_GREEN, "✅", COLOR_RESET))
            .replace("✓", &format!("{}{}{}", EMERALD_GREEN, "✓", COLOR_RESET))
            .replace("✖", &format!("{}{}{}", ERROR_RED, "✖", COLOR_RESET))
            .replace("❌", &format!("{}{}{}", ERROR_RED, "❌", COLOR_RESET))
            .replace("✗", &format!("{}{}{}", ERROR_RED, "✗", COLOR_RESET));

        formatted = cli_re_bold()
            .replace_all(
                &formatted,
                &format!("{}{}$1{}", RED_ORANGE, COLOR_BOLD, COLOR_RESET),
            )
            .to_string();
        formatted = cli_re_code()
            .replace_all(&formatted, &format!("{}$1{}", light_blue, COLOR_RESET))
            .to_string();
        formatted = cli_re_italic()
            .replace_all(&formatted, &format!("{}$1{}", light_blue, COLOR_RESET))
            .to_string();

        println!("{}", formatted);
    }
}

fn clean_cell_text(text: &str) -> String {
    let mut cleaned = text.trim();
    while let Some(rest) = cleaned.strip_prefix('|') {
        cleaned = rest.trim();
    }
    while let Some(rest) = cleaned.strip_suffix('|') {
        cleaned = rest.trim();
    }
    if let Some(inner) = cleaned
        .strip_prefix("**")
        .and_then(|s| s.strip_suffix("**"))
    {
        cleaned = inner.trim();
    }
    if let Some(inner) = cleaned.strip_prefix('*').and_then(|s| s.strip_suffix('*')) {
        cleaned = inner.trim();
    }
    cleaned.to_string()
}

fn render_table(table_lines: &[&str]) {
    if table_lines.len() < 2 {
        for line in table_lines {
            print_normal_line(line);
        }
        return;
    }

    let headers: Vec<String> = split_row(table_lines[0])
        .into_iter()
        .map(|h| clean_cell_text(&h))
        .collect();
    let num_cols = headers.len();
    if num_cols == 0 {
        for line in table_lines {
            print_normal_line(line);
        }
        return;
    }

    let mut data_rows = Vec::new();
    for &line in &table_lines[2..] {
        let mut cells = split_row(line);
        while cells.len() < num_cols {
            cells.push(String::new());
        }
        cells.truncate(num_cols);
        data_rows.push(cells);
    }

    let term_width = if let Ok((w, _)) = crossterm::terminal::size() {
        w as usize
    } else {
        80
    };

    let separator_overhead = 3 * (num_cols - 1);
    let available_width = term_width
        .saturating_sub(separator_overhead)
        .saturating_sub(2);

    let mut max_content_widths = vec![0; num_cols];
    for col in 0..num_cols {
        let mut max_w = text_display_width(&headers[col]);
        for row in &data_rows {
            max_w = max_w.max(text_display_width(&row[col]));
        }
        max_content_widths[col] = max_w.max(3);
    }

    let total_content_width: usize = max_content_widths.iter().sum();
    let mut col_widths = max_content_widths.clone();

    if total_content_width > available_width {
        let mut remaining_width = available_width;
        let mut large_cols = Vec::new();

        for col in 0..num_cols {
            if max_content_widths[col] <= 15 {
                col_widths[col] = max_content_widths[col];
                remaining_width = remaining_width.saturating_sub(col_widths[col]);
            } else {
                large_cols.push(col);
            }
        }

        if !large_cols.is_empty() {
            let equal_share = remaining_width / large_cols.len();
            let mut extra = remaining_width % large_cols.len();
            for &col in &large_cols {
                let share = equal_share
                    + if extra > 0 {
                        extra -= 1;
                        1
                    } else {
                        0
                    };
                col_widths[col] = share.max(10);
            }
        }
    }

    let mut divider = String::new();
    for col in 0..num_cols {
        if col == 0 {
            divider.push_str(&"─".repeat(col_widths[0] + 1));
        } else if col == num_cols - 1 {
            divider.push_str(&"─".repeat(col_widths[col] + 1));
        } else {
            divider.push_str(&"─".repeat(col_widths[col] + 2));
        }
        if col < num_cols - 1 {
            divider.push('┼');
        }
    }

    let separator = format!(" {}│{} ", LIGHT_WHITE, COLOR_RESET);
    let divider_colored = format!("{}{}{}", LIGHT_WHITE, divider, COLOR_RESET);

    let mut header_cell_lines = Vec::new();
    let mut max_header_lines = 1;
    for col in 0..num_cols {
        let lines = wrap_text(&headers[col], col_widths[col]);
        max_header_lines = max_header_lines.max(lines.len());
        header_cell_lines.push(lines);
    }

    for line_idx in 0..max_header_lines {
        let mut header_line_parts = Vec::new();
        for col in 0..num_cols {
            let text = header_cell_lines[col]
                .get(line_idx)
                .cloned()
                .unwrap_or_default();
            let visible_w = text_display_width(&text);
            let padding_len = col_widths[col].saturating_sub(visible_w);
            let formatted = format_cell_text(&text);
            let colored = format!(
                "{}{}{}{}{}",
                HEADING_BLUE,
                COLOR_BOLD,
                formatted,
                COLOR_RESET,
                " ".repeat(padding_len)
            );
            header_line_parts.push(colored);
        }
        println!("{}", header_line_parts.join(&separator));
    }

    println!("{}", divider_colored);

    for row in data_rows {
        let mut cell_lines = Vec::new();
        let mut max_lines = 1;
        for col in 0..num_cols {
            let lines = wrap_text(&row[col], col_widths[col]);
            max_lines = max_lines.max(lines.len());
            cell_lines.push(lines);
        }

        for line_idx in 0..max_lines {
            let mut row_line_parts = Vec::new();
            for col in 0..num_cols {
                let text = cell_lines[col].get(line_idx).cloned().unwrap_or_default();
                let visible_w = text_display_width(&text);
                let padding_len = col_widths[col].saturating_sub(visible_w);
                let formatted = format_cell_text(&text);
                let padded = format!("{}{}", formatted, " ".repeat(padding_len));
                row_line_parts.push(padded);
            }
            println!("{}", row_line_parts.join(&separator));
        }
    }
}

pub fn print_session_history(session: &crate::session::Session) {
    if session.messages.is_empty() {
        return;
    }
    println!("{}=== Session History ==={}", COLOR_BOLD, COLOR_RESET);
    for msg in &session.messages {
        let role_str = match msg.role.as_str() {
            "user" => format!("{}◇ User:{}", COLOR_BOLD, COLOR_RESET),
            "assistant" => format!("{}🤖 OpenZ:{}", EMERALD_GREEN, COLOR_RESET),
            "tool" => format!("{}🛠 Tool:{}", AURA_GOLD, COLOR_RESET),
            other => format!("{}{}:{}{}", AURA_SLATE, other, COLOR_BOLD, COLOR_RESET),
        };
        let content = msg.content.trim();
        if !content.is_empty() {
            println!("{} {}", role_str, COLOR_RESET);
            print_colored_markdown(content);
            println!();
        } else if let Some(tool_calls) = msg.extra.get("tool_calls").and_then(|v| v.as_array()) {
            let names: Vec<String> = tool_calls
                .iter()
                .filter_map(|tc| {
                    tc.get("function")
                        .and_then(|f| f.get("name"))
                        .and_then(|n| n.as_str())
                })
                .map(|n| n.to_string())
                .collect();
            if !names.is_empty() {
                println!(
                    "{} [Called tool(s): {}] {}",
                    role_str,
                    names.join(", "),
                    COLOR_RESET
                );
            } else {
                println!("{} [Called tool(s)] {}", role_str, COLOR_RESET);
            }
        }
    }
    println!(
        "{}────────────────────────────────────────────────────────────{}",
        LIGHT_WHITE, COLOR_RESET
    );
}

pub fn print_colored_markdown(content: &str) {
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;
    let mut in_code_block = false;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim_start();

        if trimmed.starts_with("```") {
            in_code_block = !in_code_block;
            print_normal_line(line);
            i += 1;
            continue;
        }

        if !in_code_block
            && i + 1 < lines.len()
            && is_table_row(lines[i])
            && is_divider_row(lines[i + 1])
        {
            let mut table_lines = Vec::new();
            table_lines.push(lines[i]);
            i += 1;
            table_lines.push(lines[i]);
            i += 1;
            while i < lines.len() && is_table_row(lines[i]) {
                table_lines.push(lines[i]);
                i += 1;
            }
            render_table(&table_lines);
        } else {
            print_normal_line(line);
            i += 1;
        }
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
    viewport_start: &mut usize,
    selected_index: Option<usize>,
    autocomplete_visible: bool,
    width: usize,
    lines_printed: &mut usize,
) -> anyhow::Result<()> {
    // First, clear the previous rendering below the input line
    if *lines_printed > 1 {
        for _ in 0..(*lines_printed - 1) {
            print!("\r\n\x1b[2K");
        }
        // Move cursor back up to the input line
        print!("\x1b[{}A\r", *lines_printed - 1);
    }
    // Clear the input line itself
    print!("\r\x1b[2K");

    // 1. Calculate token usage
    let session = session_manager.get_or_create(session_key);
    let total_chars: usize = session.messages.iter().map(|m| m.content.len()).sum();
    let approx_tokens = total_chars / 4;

    let model_lower = model.to_lowercase();
    let custom_limit = if let Ok(guard) = CUSTOM_CONTEXT_LIMIT.lock() {
        *guard
    } else {
        None
    };

    let limit_tokens = if let Some(limit) = custom_limit {
        limit
    } else if model_lower.contains("gemini-1.5-pro") || model_lower.contains("gemini-2.5-pro") {
        2_097_152
    } else if model_lower.contains("gemini") {
        1_048_576
    } else if model_lower.contains("claude-3-5")
        || model_lower.contains("claude-3")
        || model_lower.contains("o1-")
        || model_lower.contains("o3-mini")
    {
        200_000
    } else if model_lower.contains("gpt-4") || model_lower.contains("gpt-4o") {
        128_000
    } else if model_lower.contains("deepseek-v4") {
        1_000_000
    } else if model_lower.contains("deepseek-v3")
        || model_lower.contains("deepseek-r1")
        || model_lower.contains("deepseek-chat")
        || model_lower.contains("deepseek-reasoner")
        || model_lower.contains("deepseek")
        || model_lower.contains("llama-3.1")
        || model_lower.contains("llama-3.2")
        || model_lower.contains("llama-3.3")
        || model_lower.contains("llama3.1")
        || model_lower.contains("llama3.2")
        || model_lower.contains("llama3.3")
    {
        128_000
    } else if model_lower.contains("llama-3") || model_lower.contains("llama3") {
        8_192
    } else if model_lower.contains("qwen") {
        128_000
    } else if model_lower.contains("minimax") {
        204_800
    } else {
        128_000
    };

    let limit_str = if limit_tokens >= 1_000_000 {
        format!("{}M", limit_tokens / 1_000_000)
    } else {
        format!("{}K", limit_tokens / 1000)
    };

    let approx_tokens_str = if approx_tokens >= 1000 {
        format!("{:.1}K", approx_tokens as f64 / 1000.0)
    } else {
        format!("{}", approx_tokens)
    };

    let provider_lower = provider.to_lowercase();
    let display_provider = match provider_lower.as_str() {
        "openai" => "OpenAI",
        "anthropic" => "Anthropic",
        "google" => "Google",
        "deepseek" => "DeepSeek",
        other => other,
    };

    let max_model_len = if width < 60 {
        10
    } else if width < 80 {
        16
    } else {
        30
    };

    let display_model = if model.chars().count() > max_model_len {
        let truncated: String = model.chars().take(max_model_len - 1).collect();
        format!("{}…", truncated)
    } else {
        model.to_string()
    };

    // ── MCP pill ──────────────────────────────────────────────────────────────
    const SPIN_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let mcp_done = super::mcp::is_mcp_done();
    let (mcp_loaded, mcp_failed, mcp_total) = super::mcp::get_mcp_stats();

    let (mcp_pill_plain, mcp_pill_colored) = if !mcp_done {
        let frame_idx = MCP_SPIN.fetch_add(1, Ordering::Relaxed) as usize % SPIN_FRAMES.len();
        let frame = SPIN_FRAMES[frame_idx];
        let total = mcp_total;
        let processed = mcp_loaded + mcp_failed;
        if total > 0 {
            (
                format!(" ◇ MCP {}/{} {}  │ ", processed, total, frame),
                format!(
                    " {}◇ MCP {}{}/{} {}{}  {}│{} ",
                    AURA_PURPLE,
                    AURA_GOLD,
                    processed,
                    total,
                    frame,
                    COLOR_RESET,
                    AURA_SLATE,
                    COLOR_RESET
                ),
            )
        } else {
            (
                format!(" ◇ MCP {}  │ ", frame),
                format!(
                    " {}◇ MCP {}{}  {}│{} ",
                    AURA_PURPLE, frame, COLOR_RESET, AURA_SLATE, COLOR_RESET
                ),
            )
        }
    } else if mcp_failed == 0 {
        (
            format!(" ◇ MCP {}✓  │ ", mcp_loaded),
            format!(
                " {}◇ MCP {}{}{}✓{}  {}│{} ",
                AURA_PURPLE,
                AURA_GREEN,
                mcp_loaded,
                AURA_GREEN,
                COLOR_RESET,
                AURA_SLATE,
                COLOR_RESET
            ),
        )
    } else {
        (
            format!(" ◇ MCP {}✓ {}✗  │ ", mcp_loaded, mcp_failed),
            format!(
                " {}◇ MCP {}{}{}✓{} {}{}{}✗{}  {}│{} ",
                AURA_PURPLE,
                AURA_GREEN,
                mcp_loaded,
                AURA_GREEN,
                COLOR_RESET,
                AURA_ROSE,
                mcp_failed,
                AURA_ROSE,
                COLOR_RESET,
                AURA_SLATE,
                COLOR_RESET
            ),
        )
    };

    let pill_plain_len = mcp_pill_plain.chars().count();

    let visible_status_len = pill_plain_len
        + display_provider.chars().count()
        + display_model.chars().count()
        + approx_tokens_str.chars().count()
        + limit_str.chars().count()
        + 11; // spacing, vertical pipes, slash, brackets

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

    // Filter autocomplete dropdown suggestions
    let mut matches = Vec::new();
    if typed_input.starts_with('/') && !typed_input.contains(' ') {
        for &(cmd, desc) in SLASH_COMMANDS {
            if cmd.starts_with(typed_input) {
                matches.push((cmd, desc));
            }
        }
    }

    let status_line = if autocomplete_visible && !matches.is_empty() {
        let line_fill: String = std::iter::repeat_n('─', width).collect();
        format!("{}{}{}", LIGHT_WHITE, line_fill, COLOR_RESET)
    } else {
        format!(
            "{}{}{}[{}]{}{}",
            LIGHT_WHITE, line_fill, RED_ORANGE, status_content, LIGHT_WHITE, COLOR_RESET
        )
    };

    let display_text = if let Some(idx) = selected_index {
        if idx < matches.len() {
            matches[idx].0
        } else {
            typed_input
        }
    } else {
        typed_input
    };

    // Print input line prefix and input using cursor-aware viewport
    let display_chars: Vec<char> = display_text.chars().collect();
    let char_count = display_chars.len();
    let max_input_width = width.saturating_sub(3);

    let active_cursor_idx = if selected_index.is_some() {
        char_count
    } else {
        cursor_idx.min(char_count)
    };

    let mut v_start = *viewport_start;
    v_start = v_start.min(char_count);

    if active_cursor_idx < v_start {
        v_start = active_cursor_idx;
    }

    let mut cursor_offset_width: usize = 0;
    for item in display_chars.iter().take(active_cursor_idx).skip(v_start) {
        cursor_offset_width += char_display_width(*item);
    }

    while cursor_offset_width > max_input_width && v_start < active_cursor_idx {
        cursor_offset_width -= char_display_width(display_chars[v_start]);
        v_start += 1;
    }

    let mut total_width_from_v_start = cursor_offset_width;
    for item in display_chars
        .iter()
        .take(char_count)
        .skip(active_cursor_idx)
    {
        total_width_from_v_start += char_display_width(*item);
    }
    while v_start > 0 {
        let prev_width = char_display_width(display_chars[v_start - 1]);
        if total_width_from_v_start + prev_width <= max_input_width {
            v_start -= 1;
            total_width_from_v_start += prev_width;
        } else {
            break;
        }
    }

    *viewport_start = v_start;

    let mut display_input = String::new();
    let mut display_width = 0;
    let mut cursor_col_offset = 0;

    for (idx, &c) in display_chars.iter().enumerate().skip(v_start) {
        let w = char_display_width(c);
        if display_width + w <= max_input_width {
            display_input.push(c);
            display_width += w;
            if idx < active_cursor_idx {
                cursor_col_offset += w;
            }
        } else {
            break;
        }
    }

    print!("{}> {}{}", LIGHT_WHITE, COLOR_RESET, display_input);
    let mut new_lines_printed = 1;

    // Status line immediately below the input line
    print!("\r\n\x1b[2K{}", status_line);
    new_lines_printed += 1;

    if autocomplete_visible && !matches.is_empty() {
        let max_display = 5;
        let mut start_idx = 0;
        if let Some(idx) = selected_index {
            if idx >= max_display {
                start_idx = idx - max_display + 1;
            }
        }
        let end_idx = (start_idx + max_display).min(matches.len());

        for (i, item) in matches.iter().enumerate().take(end_idx).skip(start_idx) {
            let (cmd, desc) = *item;
            let is_selected = selected_index == Some(i);

            if is_selected {
                print!(
                    "\r\n\x1b[2K> {}{:<30}{}{}{}",
                    RED_ORANGE, cmd, AURA_SLATE, desc, COLOR_RESET
                );
            } else {
                print!(
                    "\r\n\x1b[2K  {:<30}{}{}{}",
                    cmd, AURA_SLATE, desc, COLOR_RESET
                );
            }
            new_lines_printed += 1;
        }

        let rem_below = matches.len() - end_idx;
        let rem_above = start_idx;
        if rem_below > 0 || rem_above > 0 {
            let mut parts = Vec::new();
            if rem_above > 0 {
                parts.push(format!("↑ {} more", rem_above));
            }
            if rem_below > 0 {
                parts.push(format!("↓ {} more", rem_below));
            }
            print!(
                "\r\n\x1b[2K  {}{}{}",
                AURA_SLATE,
                parts.join(" / "),
                COLOR_RESET
            );
            new_lines_printed += 1;
        }

        // Print spacing empty line
        print!("\r\n\x1b[2K");
        new_lines_printed += 1;

        // Print help/navigation instructions at the bottom
        print!(
            "\r\n\x1b[2K  {}↑/↓ Navigate · enter Select · tab Complete{}",
            AURA_SLATE, COLOR_RESET
        );

        let cancel_text = format!("  {}esc to cancel{}", AURA_SLATE, COLOR_RESET);
        let cancel_width = 15;
        let model_display = format!("{}{}{}", AURA_SLATE, model, COLOR_RESET);
        let model_width = model.chars().count();
        let spacing = width.saturating_sub(cancel_width + model_width);
        let spaces: String = std::iter::repeat_n(' ', spacing).collect();

        print!("\r\n\x1b[2K{}{}{}", cancel_text, spaces, model_display);
        new_lines_printed += 2;
    }

    // Move cursor back up to the input line and place it at the active cursor position
    let cursor_col = 3 + cursor_col_offset;
    print!("\x1b[{}A\x1b[{}G", new_lines_printed - 1, cursor_col);

    *lines_printed = new_lines_printed;
    stdout().flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_table_row() {
        assert!(is_table_row("| A | B |"));
        assert!(!is_table_row("Not a table row"));
        assert!(!is_table_row("|"));
    }

    #[test]
    fn test_is_divider_row() {
        assert!(is_divider_row("|---|---|"));
        assert!(is_divider_row("|:---|---:|"));
        assert!(!is_divider_row("| A | B |"));
    }

    #[test]
    fn test_split_row() {
        let cells = split_row("| A | B |");
        assert_eq!(cells, vec!["A", "B"]);

        let cells_escaped = split_row("| A\\|B | C |");
        assert_eq!(cells_escaped, vec!["A|B", "C"]);

        let cells_no_outer = split_row("A | B");
        assert_eq!(cells_no_outer, vec!["A", "B"]);
    }

    #[test]
    fn test_wrap_text() {
        let lines = wrap_text("hello world", 7);
        assert_eq!(lines, vec!["hello", "world"]);
    }

    #[test]
    fn test_horizontal_rule_detection() {
        let line1 = "---";
        let line2 = "----";
        let line3 = "  ---  ";
        let line4 = "--";
        let line5 = "-a-";

        let is_hr = |l: &str| {
            let trimmed = l.trim();
            trimmed.chars().all(|c| c == '-') && trimmed.len() >= 3 && !trimmed.is_empty()
        };

        assert!(is_hr(line1));
        assert!(is_hr(line2));
        assert!(is_hr(line3));
        assert!(!is_hr(line4));
        assert!(!is_hr(line5));
    }
}
