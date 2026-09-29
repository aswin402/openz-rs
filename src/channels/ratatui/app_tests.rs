use super::*;

fn app_with_scroll(max_scroll: u32) -> RatatuiApp {
    let mut app = RatatuiApp::new(
        "test-model".into(),
        "test-provider".into(),
        "cli:test".into(),
    );
    app.max_scroll = max_scroll;
    app
}

#[test]
fn branch_cache_is_keyed_by_workspace() {
    let first = std::env::temp_dir().join(format!("openz_branch_a_{}", uuid::Uuid::new_v4()));
    let second = std::env::temp_dir().join(format!("openz_branch_b_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&first).unwrap();
    std::fs::create_dir_all(&second).unwrap();

    let first_key = first.canonicalize().unwrap();
    let second_key = second.canonicalize().unwrap();

    {
        let mut guard = super::BRANCH_CACHE.lock().unwrap();
        guard.clear();
        guard.insert(
            first_key.clone(),
            (Instant::now(), Some("main".to_string())),
        );
        guard.insert(
            second_key.clone(),
            (Instant::now(), Some("feature".to_string())),
        );
    }

    assert_eq!(RatatuiApp::get_git_branch(&first).as_deref(), Some("main"));
    assert_eq!(
        RatatuiApp::get_git_branch(&second).as_deref(),
        Some("feature")
    );

    let _ = std::fs::remove_dir_all(first);
    let _ = std::fs::remove_dir_all(second);
}

#[test]
fn scroll_up_from_auto_scroll_lands_near_bottom() {
    let mut app = app_with_scroll(100);
    assert!(app.auto_scroll);
    app.scroll_up(3);
    assert!(!app.auto_scroll);
    assert_eq!(app.scroll_offset, 97);
}

#[test]
fn scroll_up_clamps_at_zero() {
    let mut app = app_with_scroll(10);
    app.scroll_up(2); // leave auto mode
    app.scroll_up(50); // way past the top
    assert_eq!(app.scroll_offset, 0);
    assert!(!app.auto_scroll);
}

#[test]
fn scroll_down_reengages_auto_scroll_at_bottom() {
    let mut app = app_with_scroll(100);
    app.scroll_up(10);
    assert_eq!(app.scroll_offset, 90);
    app.scroll_down(5);
    assert_eq!(app.scroll_offset, 95);
    assert!(!app.auto_scroll);
    app.scroll_down(5); // reaches the bottom edge
    assert_eq!(app.scroll_offset, 100);
    assert!(app.auto_scroll);
}

#[test]
fn scroll_down_is_noop_when_auto_scrolling() {
    let mut app = app_with_scroll(100);
    app.scroll_to_bottom(); // anchor the field at max, auto mode on
    app.scroll_down(40); // auto mode absorbs it — no manual scroll starts
    assert_eq!(app.scroll_offset, 100);
    assert!(app.auto_scroll);
}

#[test]
fn scroll_to_top_and_bottom_edges() {
    let mut app = app_with_scroll(100);
    app.scroll_to_top();
    assert_eq!(app.scroll_offset, 0);
    assert!(!app.auto_scroll);
    app.scroll_to_bottom();
    assert_eq!(app.scroll_offset, 100);
    assert!(app.auto_scroll);
}

#[test]
fn apply_sync_session_preserves_recent_notices_only() {
    let mut app = app_with_scroll(50);
    for i in 0..7 {
        app.messages
            .push(ChatMessage::notice(format!("note {}", i)));
    }
    app.messages
        .push(ChatMessage::simple("user", "hello".into()));
    app.messages
        .push(ChatMessage::simple("assistant", "hi".into()));
    app.scroll_to_top();

    let disk = vec![
        ChatMessage::simple("user", "from disk".into()),
        ChatMessage::simple("assistant", "disk reply".into()),
    ];
    app.apply_sync_session(disk);

    assert_eq!(app.messages.len(), 7); // 2 disk + 5 kept notices (capped)
    assert_eq!(app.messages[0].content, "from disk");
    assert!(!app.messages[0].ephemeral);
    // Oldest two notices dropped, newest five kept in order
    let notices: Vec<&str> = app.messages[2..]
        .iter()
        .map(|m| m.content.as_str())
        .collect();
    assert_eq!(
        notices,
        vec!["note 2", "note 3", "note 4", "note 5", "note 6"]
    );
    assert!(app.auto_scroll); // re-anchored to bottom
}

#[test]
fn from_session_message_extracts_extras() {
    let mut extra = serde_json::Map::new();
    extra.insert("tool_name".into(), serde_json::json!("read_file"));
    extra.insert("tool_details".into(), serde_json::json!("path=src/main.rs"));
    extra.insert(
        "reasoning_content".into(),
        serde_json::json!("let me think"),
    );
    extra.insert("thinking_time_secs".into(), serde_json::json!(1.25));
    let msg = crate::session::Message {
        role: "tool".into(),
        content: "file contents".into(),
        timestamp: None,
        extra,
    };

    let chat = ChatMessage::from_session_message(&msg);
    assert!(chat.is_tool);
    assert_eq!(chat.tool_name.as_deref(), Some("read_file"));
    assert_eq!(chat.tool_details.as_deref(), Some("path=src/main.rs"));
    assert_eq!(chat.reasoning.as_deref(), Some("let me think"));
    assert_eq!(chat.thinking_time, Some(1.25));
    assert!(!chat.ephemeral);
}

#[test]
fn notice_is_ephemeral_and_defaults_are_not() {
    let notice = ChatMessage::notice("keep me".into());
    assert!(notice.ephemeral);
    assert!(!ChatMessage::simple("user", "x".into()).ephemeral);
    assert!(!ChatMessage::tool_start("t".into(), "d".into()).ephemeral);
}

#[test]
fn test_prompt_queue_fifo_order() {
    let mut app = RatatuiApp::new(
        "test-model".into(),
        "test-provider".into(),
        "cli:test".into(),
    );
    assert!(app.queued_prompts.is_empty());
    assert_eq!(app.pop_next_prompt(), None);

    app.queue_prompt("first prompt".to_string());
    app.queue_prompt("second prompt".to_string());
    app.queue_prompt("third prompt".to_string());

    assert_eq!(app.queued_prompts.len(), 3);
    assert_eq!(app.pop_next_prompt(), Some("first prompt".to_string()));
    assert_eq!(app.pop_next_prompt(), Some("second prompt".to_string()));
    assert_eq!(app.queued_prompts.len(), 1);
    assert_eq!(app.pop_next_prompt(), Some("third prompt".to_string()));
    assert_eq!(app.pop_next_prompt(), None);
    assert!(app.queued_prompts.is_empty());
}

#[test]
fn test_wrapped_paragraph_line_count_and_max_scroll() {
    use ratatui::text::Line;
    use ratatui::widgets::{Paragraph, Wrap};

    // 1 line with 200 characters
    let long_line = "A".repeat(200);
    let p = Paragraph::new(vec![Line::from(long_line)]).wrap(Wrap { trim: false });
    // In an 80-column viewport, a 200-char line wraps to 3 visual lines (80 + 80 + 40)
    let line_count = p.line_count(80);
    assert_eq!(line_count, 3);

    // If viewport height is 2, max_scroll should be 1
    let max_scroll = (line_count as u32).saturating_sub(2);
    assert_eq!(max_scroll, 1);
}

#[test]
fn test_from_session_messages_resolves_tool_calls_map() {
    let mut assistant_extra = serde_json::Map::new();
    assistant_extra.insert(
        "tool_calls".into(),
        serde_json::json!([
            {
                "id": "call_123",
                "type": "function",
                "function": {
                    "name": "write_file",
                    "arguments": "{\"path\":\"src/lib.rs\",\"content\":\"pub fn test() {}\"}"
                }
            }
        ]),
    );

    let assistant_msg = crate::session::Message {
        role: "assistant".into(),
        content: String::new(),
        timestamp: None,
        extra: assistant_extra,
    };

    let mut tool_extra = serde_json::Map::new();
    tool_extra.insert("tool_call_id".into(), serde_json::json!("call_123"));

    let tool_msg = crate::session::Message {
        role: "tool".into(),
        content: "{\"status\":\"success\",\"bytes\":18}".into(),
        timestamp: None,
        extra: tool_extra,
    };

    let chat_msgs = ChatMessage::from_session_messages(&[assistant_msg, tool_msg]);
    assert_eq!(chat_msgs.len(), 2);

    let tool_chat = &chat_msgs[1];
    assert!(tool_chat.is_tool);
    assert_eq!(tool_chat.tool_name.as_deref(), Some("Write"));
    assert_eq!(tool_chat.tool_details.as_deref(), Some("lib.rs"));
    assert_eq!(tool_chat.tool_success, Some(true));
    assert!(tool_chat.tool_summary.is_some());
}

#[test]
fn test_update_approx_tokens() {
    let mut app = RatatuiApp::new(
        "test-model".into(),
        "test-provider".into(),
        "cli:test".into(),
    );
    assert_eq!(app.approx_tokens, 0);

    app.messages.push(ChatMessage::simple("user", "12345678".to_string())); // 8 chars -> 2 tokens
    app.update_approx_tokens();
    assert_eq!(app.approx_tokens, 2);
}

#[test]
fn test_has_active_slash_query_and_search_text() {
    let mut app = RatatuiApp::new(
        "test-model".into(),
        "test-provider".into(),
        "cli:test".into(),
    );
    assert!(!app.has_active_slash_query());

    // Normal text
    app.typed_input = "hello world".chars().collect();
    assert!(!app.has_active_slash_query());

    // Single slash
    app.typed_input = "/".chars().collect();
    assert!(app.has_active_slash_query());
    assert_eq!(app.slash_search_text(), "");

    // Slash with search query
    app.typed_input = "/model".chars().collect();
    assert!(app.has_active_slash_query());
    assert_eq!(app.slash_search_text(), "model");

    // Command with space (arguments entered) is no longer a palette query
    app.typed_input = "/model gpt-4o".chars().collect();
    assert!(!app.has_active_slash_query());
}

#[test]
fn test_matching_palette_commands_and_selection() {
    let mut app = RatatuiApp::new(
        "test-model".into(),
        "test-provider".into(),
        "cli:test".into(),
    );
    app.typed_input = "/".chars().collect();

    let all_matches = app.matching_palette_commands();
    assert!(!all_matches.is_empty());
    assert_eq!(all_matches.len(), PALETTE_COMMANDS.len());

    // Selected palette command defaults to 0
    let selected = app.selected_palette_command().unwrap();
    assert_eq!(selected.slash_name, PALETTE_COMMANDS[0].slash_name);

    // Filter by query "help"
    app.typed_input = "/help".chars().collect();
    let help_matches = app.matching_palette_commands();
    assert!(!help_matches.is_empty());
    assert!(help_matches.iter().any(|&idx| PALETTE_COMMANDS[idx].slash_name == "/help"));
}

#[test]
fn test_cycle_slash_category() {
    let mut app = RatatuiApp::new(
        "test-model".into(),
        "test-provider".into(),
        "cli:test".into(),
    );
    assert_eq!(app.slash_category_idx, 0); // CommandCategory::All

    // Cycle forward: All -> System -> Agent -> Tools -> All
    app.cycle_slash_category(true);
    assert_eq!(app.slash_category_idx, 1);
    app.cycle_slash_category(true);
    assert_eq!(app.slash_category_idx, 2);
    app.cycle_slash_category(true);
    assert_eq!(app.slash_category_idx, 3);
    app.cycle_slash_category(true);
    assert_eq!(app.slash_category_idx, 0);

    // Cycle backward: All -> Tools -> Agent -> System -> All
    app.cycle_slash_category(false);
    assert_eq!(app.slash_category_idx, 3);
    app.cycle_slash_category(false);
    assert_eq!(app.slash_category_idx, 2);
}

#[test]
fn test_sanitize_thought_text() {
    let raw = "<think>Let me break this down step by step.</think>";
    let sanitized = sanitize_thought_text(raw);
    assert_eq!(sanitized, "Let me break this down step by step.");

    let mixed = "<thought>Thinking about auth</thought> and <antThinking>internal logic</antThinking>";
    let sanitized_mixed = sanitize_thought_text(mixed);
    assert_eq!(sanitized_mixed, "Thinking about auth and internal logic");
}

#[test]
fn test_extract_thoughts_from_content() {
    // 1. Single closed <think> tag with assistant message
    let raw = "<think>Pondering life and code.</think>\nHere is the answer to your question.";
    let (thought, content) = extract_thoughts_from_content(raw);
    assert_eq!(thought.as_deref(), Some("Pondering life and code."));
    assert_eq!(content, "Here is the answer to your question.");

    // 2. Unclosed <think> tag (streaming in progress)
    let raw_unclosed = "<think>Still reasoning through the architecture...";
    let (thought_unclosed, content_unclosed) = extract_thoughts_from_content(raw_unclosed);
    assert_eq!(
        thought_unclosed.as_deref(),
        Some("Still reasoning through the architecture...")
    );
    assert_eq!(content_unclosed, "");

    // 3. Alternative <thought> tag
    let raw_thought = "<thought>Verify file exists</thought>I will check the directory.";
    let (thought_alt, content_alt) = extract_thoughts_from_content(raw_thought);
    assert_eq!(thought_alt.as_deref(), Some("Verify file exists"));
    assert_eq!(content_alt, "I will check the directory.");

    // 4. Normal text without any tags
    let raw_plain = "Standard response without thoughts.";
    let (thought_none, content_plain) = extract_thoughts_from_content(raw_plain);
    assert_eq!(thought_none, None);
    assert_eq!(content_plain, "Standard response without thoughts.");
}

#[test]
fn test_agent_activity_from_tool_call() {
    use crate::channels::ratatui::animation::AgentActivity;

    // Executing command
    let activity = AgentActivity::from_tool_call(
        "exec_command",
        r#"{"command":"cargo test -p openz -j 1"}"#,
    );
    assert!(matches!(activity, AgentActivity::Debugging { .. }));

    let activity = AgentActivity::from_tool_call("exec_command", r#"{"command":"echo hello"}"#);
    assert!(matches!(activity, AgentActivity::ExecutingCommand { .. }));

    // Internet research
    let activity =
        AgentActivity::from_tool_call("web_search", r#"{"query":"Rust Ratatui crate"}"#);
    assert!(matches!(activity, AgentActivity::InternetResearch { .. }));

    // Repo research
    let activity = AgentActivity::from_tool_call("grep_search", r#"{"query":"AgentActivity"}"#);
    assert!(matches!(activity, AgentActivity::RepoResearch { .. }));

    // Editing file
    let activity =
        AgentActivity::from_tool_call("write_file", r#"{"path":"src/channels/ratatui/mod.rs"}"#);
    assert!(matches!(activity, AgentActivity::EditingFile { .. }));

    // Subagent
    let activity =
        AgentActivity::from_tool_call("delegate_task", r#"{"role":"code-reviewer"}"#);
    assert!(matches!(activity, AgentActivity::SubagentWorking { .. }));
}

#[test]
fn test_shimmer_spans_and_lerp_color() {
    use crate::channels::ratatui::animation::{lerp_color, render_shimmer_spans};
    use ratatui::style::Color;

    let c1 = Color::Rgb(255, 0, 0);
    let c2 = Color::Rgb(0, 0, 255);

    // At t = 0.0 -> red
    let c_start = lerp_color(c1, c2, 0.0);
    assert_eq!(c_start, Color::Rgb(255, 0, 0));

    // At t = 1.0 -> blue
    let c_end = lerp_color(c1, c2, 1.0);
    assert_eq!(c_end, Color::Rgb(0, 0, 255));

    // At t = 0.5 -> intermediate
    let c_mid = lerp_color(c1, c2, 0.5);
    assert_eq!(c_mid, Color::Rgb(128, 0, 128));

    // render_shimmer_spans count
    let spans = render_shimmer_spans("OpenZ", c1, c2, 1000, 150.0, 0.45);
    assert_eq!(spans.len(), 5);
}



