//! Terminal-facing reasoning and Markdown event formatting for the run loop.

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

#[allow(dead_code)]
pub(super) fn format_markdown_line(line: &str) -> String {
    crate::channels::cli::render::format_markdown_line(line)
}

pub(super) fn stream_content_chunk(
    text: &str,
    streamer: &mut crate::channels::cli::render::StreamingMarkdownRenderer,
    content_streaming_started: &mut bool,
) {
    if text.is_empty() {
        return;
    }
    streamer.push_chunk(text);
    if streamer.has_started() {
        *content_streaming_started = true;
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
