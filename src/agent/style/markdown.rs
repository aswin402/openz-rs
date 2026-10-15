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
