//! Terminal-facing reasoning and Markdown event formatting for the run loop.

pub(super) fn summarize_auto_capture_topics(
    capture_summaries: &[crate::tools::shared_memory::AutoCaptureSummary],
) -> String {
    let mut seen = std::collections::HashSet::new();
    capture_summaries
        .iter()
        .filter_map(|capture| {
            let topic = capture.topic.trim();
            (!topic.is_empty() && seen.insert(topic.to_string())).then(|| topic.to_string())
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
    let _ = (text.chars().count() > 360).then(|| {
        text = text.chars().take(357).collect::<String>();
        text.push_str("...");
    });
    text
}

#[allow(unused_imports)]
pub(super) use crate::agent::style::format_markdown_line;

pub(super) fn stream_content_chunk(
    text: &str,
    streamer: &mut crate::agent::style::StreamingMarkdownRenderer,
    content_streaming_started: &mut bool,
) {
    let should_stream =
        !crate::channels::ratatui::IS_RATATUI_ACTIVE.load(std::sync::atomic::Ordering::Relaxed)
            && !text.is_empty();
    let _ = should_stream.then(|| {
        streamer.push_chunk(text);
        let _ = streamer.has_started().then(|| {
            *content_streaming_started = true;
        });
    });
}

pub(super) fn publish_auto_capture_notice(
    session_key: &str,
    show_memory_notices: bool,
    capture_summaries: &[crate::tools::shared_memory::AutoCaptureSummary],
) {
    let _ = (!capture_summaries.is_empty()).then(|| {
        let sources_saved: usize = capture_summaries.iter().map(|capture| capture.sources_saved).sum();
        let briefs_saved = count_unique_auto_capture_brief_topics(capture_summaries);
        let topics = summarize_auto_capture_topics(capture_summaries);
        let output_visibility = crate::agent::events::OutputVisibility {
            memory_notices: show_memory_notices,
            ..Default::default()
        };
        let _ = (crate::agent::events::AgentEvent::MemoryCaptureSummary {
            sources_saved,
            briefs_saved,
            topics: topics.clone(),
        })
        .public_text(&output_visibility)
        .map(|notice| {
            crate::channels::send_notification(&notice);
            crate::channels::websocket::publish_activity_notice(
                session_key,
                "memory",
                "Memory stored",
                format!(
                    "{} source(s), {} brief(s) | {}",
                    sources_saved, briefs_saved, topics
                ),
            );
        });
    });
}
