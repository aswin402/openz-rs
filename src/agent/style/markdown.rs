/// Shared zero-regex, zero-`if`/`else` markdown tokenizer, display-width calculator,
/// table layout solver, and brand banner primitives used across both CLI and Ratatui TUIs.

pub const OPENZ_BANNER_PARTS: &[(&str, &str)] = &[
    (" ██████╗ ██████╗ ███████╗███╗   ██╗", "███████╗"),
    ("██╔═══██╗██╔══██╗██╔════╝████╗  ██║", "╚══███╔╝"),
    ("██║   ██║██████╔╝█████╗  ██╔██╗ ██║", "  ███╔╝ "),
    ("██║   ██║██╔═══╝ ██╔══╝  ██║╚██╗██║", " ███╔╝  "),
    ("╚██████╔╝██║     ███████╗██║ ╚████║", "███████╗"),
    (" ╚═════╝ ╚═╝     ╚══════╝╚═╝  ╚═══╝", "╚══════╝"),
];

const PROVIDER_DISPLAY_NAMES: &[(&str, &str)] = &[
    ("openai", "OpenAI"),
    ("anthropic", "Anthropic"),
    ("google", "Google"),
    ("google_ai_studio", "Google AI Studio"),
    ("deepseek", "DeepSeek"),
    ("groq", "Groq"),
    ("ollama", "Ollama"),
    ("openrouter", "OpenRouter"),
    ("minimax", "MiniMax"),
    ("mistral", "Mistral"),
    ("zai", "Z.ai"),
    ("z.ai", "Z.ai"),
    ("nvidia", "NVIDIA"),
    ("opencode", "OpenCode Zen"),
    ("opencode_zen", "OpenCode Zen"),
    ("cerebras", "Cerebras"),
    ("auto", "Auto"),
];

pub fn provider_display_name(provider: &str) -> &str {
    let trimmed = provider.trim();
    PROVIDER_DISPLAY_NAMES
        .iter()
        .find_map(|&(key, label)| key.eq_ignore_ascii_case(trimmed).then_some(label))
        .unwrap_or(trimmed)
}

pub fn terminal_width_or_default(default_width: usize) -> usize {
    crossterm::terminal::size()
        .ok()
        .map(|(w, _)| w as usize)
        .filter(|&w| w > 0)
        .unwrap_or(default_width)
}

pub fn horizontal_divider(width: usize) -> String {
    "─".repeat(width.max(4))
}

pub fn char_display_width(c: char) -> usize {
    let cp = c as u32;
    let is_variation_selector = cp == 0xFE0F;
    let is_wide_glyph = (0x1F000..=0x1FBF9).contains(&cp)
        || matches!(c, '⬢' | '🗑' | '📊' | '✅' | '❌' | '⚠' | '⚡' | 'ℹ');
    match (is_variation_selector, is_wide_glyph) {
        (true, _) => 0,
        (false, true) => 2,
        (false, false) => 1,
    }
}

pub fn str_display_width(s: &str) -> usize {
    s.chars().map(char_display_width).sum()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InlineSegment {
    Plain(String),
    Bold(String),
    Code(String),
    Italic(String),
}

impl InlineSegment {
    pub fn as_text(&self) -> &str {
        match self {
            Self::Plain(s) | Self::Bold(s) | Self::Code(s) | Self::Italic(s) => s,
        }
    }
}

fn find_paired_delimiter(
    text: &str,
    delim: &str,
    allow_empty: bool,
    disallow_newline: bool,
) -> Option<(usize, usize, String)> {
    let dlen = delim.len();
    let start = text.find(delim)?;
    let after_open = &text[start + dlen..];
    let rel_end = after_open.find(delim)?;
    let inner = &after_open[..rel_end];
    let valid_len = allow_empty || !inner.is_empty();
    let valid_newline = !disallow_newline || !inner.contains('\n');
    (valid_len && valid_newline).then(|| (start, start + dlen + rel_end + dlen, inner.to_string()))
}

fn find_single_star_italic(text: &str) -> Option<(usize, usize, String)> {
    let bytes = text.as_bytes();
    let open_idx = (0..bytes.len()).find(|&i| {
        let is_star = bytes[i] == b'*';
        let prev_star = i > 0 && bytes[i - 1] == b'*';
        let next_star = i + 1 < bytes.len() && bytes[i + 1] == b'*';
        is_star && !prev_star && !next_star
    })?;
    let search_start = open_idx + 1;
    let close_idx = (search_start..bytes.len()).find(|&j| {
        let is_star = bytes[j] == b'*';
        let prev_star = j > search_start && bytes[j - 1] == b'*';
        let next_star = j + 1 < bytes.len() && bytes[j + 1] == b'*';
        is_star && !prev_star && !next_star
    })?;
    let inner = &text[search_start..close_idx];
    (!inner.is_empty() && !inner.contains('\n') && !inner.contains('*'))
        .then(|| (open_idx, close_idx + 1, inner.to_string()))
}

pub fn tokenize_inline_markdown(text: &str) -> Vec<InlineSegment> {
    let mut segments = Vec::new();
    let mut remaining = text;

    while !remaining.is_empty() {
        let bold_cand = find_paired_delimiter(remaining, "**", true, false)
            .map(|(s, e, inner)| (s, e, 0u8, InlineSegment::Bold(inner)));
        let code_cand = find_paired_delimiter(remaining, "`", false, false)
            .map(|(s, e, inner)| (s, e, 1u8, InlineSegment::Code(inner)));
        let italic_cand = find_single_star_italic(remaining)
            .map(|(s, e, inner)| (s, e, 2u8, InlineSegment::Italic(inner)));

        let earliest = [bold_cand, code_cand, italic_cand]
            .into_iter()
            .flatten()
            .min_by_key(|cand| (cand.0, cand.2));

        match earliest {
            Some((start, end, _, segment)) => {
                (start > 0).then(|| {
                    segments.push(InlineSegment::Plain(remaining[..start].to_string()));
                });
                segments.push(segment);
                remaining = &remaining[end..];
            }
            None => {
                segments.push(InlineSegment::Plain(remaining.to_string()));
                remaining = "";
            }
        }
    }

    segments
}

pub fn strip_inline_markdown_tokens(text: &str) -> String {
    let no_ansi = super::strip_ansi_escapes(text);
    tokenize_inline_markdown(&no_ansi)
        .into_iter()
        .map(|seg| seg.as_text().to_string())
        .collect::<Vec<String>>()
        .join("")
}

pub fn text_display_width(text: &str) -> usize {
    str_display_width(&strip_inline_markdown_tokens(text))
}

pub fn is_divider_row(line: &str) -> bool {
    let trimmed = line.trim();
    !trimmed.is_empty()
        && trimmed.contains('|')
        && trimmed
            .chars()
            .all(|c| c == '|' || c == '-' || c == ':' || c.is_whitespace())
}

pub fn is_table_row(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.contains('|') && !is_divider_row(line)
}

pub fn split_row(line: &str) -> Vec<String> {
    let trimmed = line.trim();
    match trimmed.is_empty() {
        true => Vec::new(),
        false => {
            let without_leading = trimmed
                .strip_prefix('|')
                .map(str::trim)
                .unwrap_or(trimmed);
            let inner = without_leading
                .strip_suffix('|')
                .map(str::trim)
                .unwrap_or(without_leading);

            let (mut cells, last_cell, pending_escape) = inner.chars().fold(
                (Vec::<String>::new(), String::new(), false),
                |(mut acc, mut cur, escaped), c| match (escaped, c) {
                    (true, '|') => {
                        cur.push('|');
                        (acc, cur, false)
                    }
                    (true, other) => {
                        cur.push('\\');
                        cur.push(other);
                        (acc, cur, false)
                    }
                    (false, '\\') => (acc, cur, true),
                    (false, '|') => {
                        acc.push(cur.trim().to_string());
                        cur.clear();
                        (acc, cur, false)
                    }
                    (false, other) => {
                        cur.push(other);
                        (acc, cur, false)
                    }
                },
            );
            let mut final_cell = last_cell;
            pending_escape.then(|| final_cell.push('\\'));
            cells.push(final_cell.trim().to_string());
            cells
        }
    }
}

pub fn clean_cell_text(text: &str, strip_outer_emphasis: bool) -> String {
    let mut cleaned = text.trim();
    while let Some(rest) = cleaned.strip_prefix('|') {
        cleaned = rest.trim();
    }
    while let Some(rest) = cleaned.strip_suffix('|') {
        cleaned = rest.trim();
    }
    let normalized = match strip_outer_emphasis {
        true => cleaned
            .strip_prefix("**")
            .and_then(|s| s.strip_suffix("**"))
            .map(str::trim)
            .or_else(|| {
                cleaned
                    .strip_prefix('*')
                    .and_then(|s| s.strip_suffix('*'))
                    .map(str::trim)
            })
            .unwrap_or(cleaned),
        false => cleaned,
    };
    normalized.to_string()
}

pub fn compute_table_column_widths(
    max_content_widths: &[usize],
    available_width: usize,
    min_large_col: usize,
) -> Vec<usize> {
    let total_content_width: usize = max_content_widths.iter().sum();
    let mut col_widths = max_content_widths.to_vec();

    (total_content_width > available_width).then(|| {
        let (remaining_width, large_cols) = max_content_widths.iter().enumerate().fold(
            (available_width, Vec::<usize>::new()),
            |(rem, mut large), (col, &w)| match w <= 15 {
                true => (rem.saturating_sub(w), large),
                false => {
                    large.push(col);
                    (rem, large)
                }
            },
        );

        (!large_cols.is_empty()).then(|| {
            let equal_share = remaining_width / large_cols.len();
            let extra = remaining_width % large_cols.len();
            for (idx, &col) in large_cols.iter().enumerate() {
                let bonus = usize::from(idx < extra);
                col_widths[col] = (equal_share + bonus).max(min_large_col);
            }
        });
    });

    col_widths
}

fn split_oversized_word(word: &str, max_width: usize) -> Vec<(String, usize)> {
    let (mut chunks, tail_chunk, tail_w) = word.chars().fold(
        (Vec::<(String, usize)>::new(), String::new(), 0usize),
        |(mut acc, mut chunk, chunk_w), c| {
            let cw = char_display_width(c);
            let exceeds = chunk_w + cw > max_width && chunk_w > 0;
            match exceeds {
                true => {
                    acc.push((chunk, chunk_w));
                    let mut next = String::new();
                    next.push(c);
                    (acc, next, cw)
                }
                false => {
                    chunk.push(c);
                    (acc, chunk, chunk_w + cw)
                }
            }
        },
    );
    (!tail_chunk.is_empty()).then(|| chunks.push((tail_chunk, tail_w)));
    chunks
}

pub fn wrap_display_text(text: &str, max_width: usize) -> Vec<String> {
    match max_width {
        0 => vec![text.to_string()],
        _ => {
            let mut lines = Vec::new();
            for paragraph in text.split('\n') {
                let (mut para_lines, rem_line, _) = paragraph.split_whitespace().fold(
                    (Vec::<String>::new(), String::new(), 0usize),
                    |(mut acc, cur_line, cur_w), word| {
                        let word_w = text_display_width(word);
                        let fits_current =
                            !cur_line.is_empty() && (cur_w + 1 + word_w <= max_width);
                        let fits_fresh = word_w <= max_width;

                        match (cur_line.is_empty(), fits_current, fits_fresh) {
                            (true, _, true) => (acc, word.to_string(), word_w),
                            (true, _, false) => {
                                let chunks = split_oversized_word(word, max_width);
                                for (chunk, _) in chunks {
                                    acc.push(chunk);
                                }
                                (acc, String::new(), 0)
                            }
                            (false, true, _) => {
                                let mut next = cur_line;
                                next.push(' ');
                                next.push_str(word);
                                (acc, next, cur_w + 1 + word_w)
                            }
                            (false, false, true) => {
                                acc.push(cur_line);
                                (acc, word.to_string(), word_w)
                            }
                            (false, false, false) => {
                                acc.push(cur_line);
                                let mut chunks = split_oversized_word(word, max_width);
                                let last_item = chunks.pop();
                                for (chunk, _) in chunks {
                                    acc.push(chunk);
                                }
                                match last_item {
                                    Some((last_str, last_w)) => (acc, last_str, last_w),
                                    None => (acc, String::new(), 0),
                                }
                            }
                        }
                    },
                );
                (!rem_line.is_empty()).then(|| para_lines.push(rem_line));
                (para_lines.is_empty() && paragraph.is_empty())
                    .then(|| para_lines.push(String::new()));
                lines.extend(para_lines);
            }
            lines.is_empty().then(|| lines.push(String::new()));
            lines
        }
    }
}

use super::colors::*;
use std::io::Write;

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

pub fn render_table_lines(table_lines: &[&str]) -> Vec<String> {
    let headers: Vec<String> = table_lines
        .first()
        .map(|first| {
            split_row(first)
                .into_iter()
                .map(|h| clean_cell_text(&h, true))
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
                .map(|col| wrap_display_text(&headers[col], col_widths[col]))
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
                    .map(|col| wrap_display_text(&row[col], col_widths[col]))
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
