//! Terminal-facing reasoning and Markdown event formatting for the run loop.

use crate::agent::style::*;
use std::io::Write;

pub(super) fn summarize_auto_capture_topics(
    capture_summaries: &[crate::tools::shared_memory::AutoCaptureSummary],
) -> String {
    let mut seen = std::collections::HashSet::new();
    capture_summaries
        .iter()
        .filter_map(|capture| {
            let topic = capture.topic.trim();
            if topic.is_empty() || !seen.insert(topic.to_string()) {
                None
            } else {
                Some(topic.to_string())
            }
        })
        .take(3)
        .collect::<Vec<_>>()
        .join(", ")
}

pub(super) fn count_unique_auto_capture_brief_topics(
    capture_summaries: &[crate::tools::shared_memory::AutoCaptureSummary],
) -> usize {
    capture_summaries
        .iter()
        .filter(|capture| capture.brief_saved)
        .map(|capture| capture.topic.trim())
        .filter(|topic| !topic.is_empty())
        .collect::<std::collections::HashSet<_>>()
        .len()
}

pub(super) fn normalize_tui_thought_display(mode: &str) -> &'static str {
    match mode.trim().to_lowercase().as_str() {
        "off" | "none" | "hide" | "hidden" => "off",
        "compact" | "summary" | "summarized" => "compact",
        _ => "full",
    }
}

pub(super) fn should_show_tui_thoughts(mode: &str) -> bool {
    normalize_tui_thought_display(mode) != "off"
}

pub(super) fn should_send_public_reasoning_progress(mode: &str) -> bool {
    should_show_tui_thoughts(mode)
}

pub(super) fn compact_reasoning_summary(reasoning: &str) -> String {
    let mut text = reasoning.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.chars().count() > 360 {
        text = text.chars().take(357).collect::<String>();
        text.push_str("...");
    }
    text
}

pub(super) fn format_markdown_line(line: &str) -> String {
    static RE_BOLD: std::sync::OnceLock<Option<regex::Regex>> = std::sync::OnceLock::new();
    static RE_CODE: std::sync::OnceLock<Option<regex::Regex>> = std::sync::OnceLock::new();
    static RE_ITALIC: std::sync::OnceLock<Option<regex::Regex>> = std::sync::OnceLock::new();

    let re_bold = RE_BOLD
        .get_or_init(|| regex::Regex::new(r"\*\*(.*?)\*\*").ok())
        .as_ref();
    let re_code = RE_CODE
        .get_or_init(|| regex::Regex::new(r"`(.*?)`").ok())
        .as_ref();
    let re_italic = RE_ITALIC
        .get_or_init(|| regex::Regex::new(r"\*(.*?)\*").ok())
        .as_ref();

    let light_blue = "\x1b[38;2;135;206;250m";

    let trimmed = line.trim();
    if trimmed.chars().all(|c| c == '-') && trimmed.len() >= 3 && !trimmed.is_empty() {
        return format!("{}──────{}", LIGHT_WHITE, COLOR_RESET);
    }

    if line.trim_start().starts_with('#') {
        return format!("{}{}{}", HEADING_BLUE, line, COLOR_RESET);
    }

    let mut formatted = line.to_string();
    formatted = formatted
        .replace('✔', &format!("{}{}{}", EMERALD_GREEN, "✔", COLOR_RESET))
        .replace("✅", &format!("{}{}{}", EMERALD_GREEN, "✅", COLOR_RESET))
        .replace('✓', &format!("{}{}{}", EMERALD_GREEN, "✓", COLOR_RESET))
        .replace('✖', &format!("{}{}{}", ERROR_RED, "✖", COLOR_RESET))
        .replace("❌", &format!("{}{}{}", ERROR_RED, "❌", COLOR_RESET))
        .replace('✗', &format!("{}{}{}", ERROR_RED, "✗", COLOR_RESET));

    if let Some(re_bold) = re_bold {
        formatted = re_bold
            .replace_all(
                &formatted,
                &format!("{}{}$1{}", RED_ORANGE, COLOR_BOLD, COLOR_RESET),
            )
            .to_string();
    }
    if let Some(re_code) = re_code {
        formatted = re_code
            .replace_all(&formatted, &format!("{}$1{}", light_blue, COLOR_RESET))
            .to_string();
    }
    if let Some(re_italic) = re_italic {
        formatted = re_italic
            .replace_all(&formatted, &format!("{}$1{}", light_blue, COLOR_RESET))
            .to_string();
    }

    formatted
}

pub(super) fn stream_content_chunk(
    text: &str,
    current_line_buffer: &mut String,
    silent: bool,
    content_streaming_started: &mut bool,
) {
    for c in text.chars() {
        if c == '\r' {
            continue;
        }
        if c == '\n' {
            if !silent {
                *content_streaming_started = true;
                print!("\r\x1b[2K");
                print!("{}", format_markdown_line(current_line_buffer));
                print!("\r\n");
                let _ = std::io::stdout().flush();
            }
            current_line_buffer.clear();
        } else {
            current_line_buffer.push(c);
            if !silent {
                *content_streaming_started = true;
                print!("{}", c);
                let _ = std::io::stdout().flush();
            }
        }
    }
}

pub(super) fn publish_auto_capture_notice(
    session_key: &str,
    show_memory_notices: bool,
    capture_summaries: &[crate::tools::shared_memory::AutoCaptureSummary],
) {
    if capture_summaries.is_empty() {
        return;
    }

    let sources_saved: usize = capture_summaries.iter().map(|capture| capture.sources_saved).sum();
    let briefs_saved = count_unique_auto_capture_brief_topics(capture_summaries);
    let topics = summarize_auto_capture_topics(capture_summaries);
    let output_visibility = crate::agent::events::OutputVisibility {
        memory_notices: show_memory_notices,
        ..Default::default()
    };
    if let Some(notice) = (crate::agent::events::AgentEvent::MemoryCaptureSummary {
        sources_saved,
        briefs_saved,
        topics: topics.clone(),
    })
    .public_text(&output_visibility)
    {
        crate::channels::cli::send_notification(&notice);
        crate::channels::websocket::publish_activity_notice(
            session_key,
            "memory",
            "Memory stored",
            format!(
                "{} source(s), {} brief(s) | {}",
                sources_saved, briefs_saved, topics
            ),
        );
    }
}
