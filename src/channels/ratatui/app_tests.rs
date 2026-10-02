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

    // Writing file
    let activity =
        AgentActivity::from_tool_call("write_file", r#"{"path":"src/channels/ratatui/mod.rs"}"#);
    assert!(matches!(activity, AgentActivity::Writing { .. }));

    // Editing file
    let activity =
        AgentActivity::from_tool_call("patch_file", r#"{"path":"src/channels/ratatui/mod.rs"}"#);
    assert!(matches!(activity, AgentActivity::EditingFile { .. }));

    // Subagent
    let activity =
        AgentActivity::from_tool_call("delegate_task", r#"{"role":"code-reviewer"}"#);
    assert!(matches!(activity, AgentActivity::SubagentWorking { .. }));

    // Media
    let activity =
        AgentActivity::from_tool_call("openmedia_create_chart", "title: \"Benchmark\"");
    assert!(matches!(activity, AgentActivity::MediaGenerating { .. }));

    // Document
    let activity =
        AgentActivity::from_tool_call("opendoc_create_docx", "path: \"report.docx\"");
    assert!(matches!(activity, AgentActivity::DocumentProcessing { .. }));

    // Planning
    let activity =
        AgentActivity::from_tool_call("sequentialthinking", "thought: \"Step 1\"");
    assert_eq!(activity, AgentActivity::Planning);
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

#[test]
fn test_handle_tool_start_and_end() {
    let mut app = RatatuiApp::new("test-model".into(), "test-prov".into(), "test-sess".into());
    assert_eq!(app.messages.len(), 0);

    // 1. Tool start
    app.handle_tool_start("exec_command".into(), "cargo check -p openz".into());
    assert_eq!(app.messages.len(), 1);
    let msg = &app.messages[0];
    assert!(msg.is_tool);
    assert_eq!(msg.tool_name.as_deref(), Some("exec_command"));
    assert_eq!(msg.tool_details.as_deref(), Some("cargo check -p openz"));
    assert_eq!(msg.tool_success, None);

    // 2. Tool end
    app.handle_tool_end(
        "exec_command",
        "Finished dev profile".into(),
        Some("\u{1b}[32m✓ completed\u{1b}[0m".into()),
        true,
        Some(420),
    );
    assert_eq!(app.messages.len(), 1);
    let msg = &app.messages[0];
    assert_eq!(msg.tool_success, Some(true));
    assert_eq!(msg.content, "Finished dev profile");
    assert_eq!(msg.tool_duration_ms, Some(420));
    assert!(msg.tool_summary.is_some());
}

#[test]
fn test_clean_tool_outcome_summary_handles_ansi_and_symbols() {
    use crate::channels::ratatui::timeline::clean_tool_outcome_summary;

    // Test ANSI-wrapped completed
    let (is_succ, text) = clean_tool_outcome_summary("\u{1b}[32m✓ completed\u{1b}[0m", None);
    assert!(is_succ);
    assert_eq!(text, "completed");

    // Test double-tick completed
    let (is_succ, text) = clean_tool_outcome_summary("✓ completed ✓", None);
    assert!(is_succ);
    assert_eq!(text, "completed");

    // Test ANSI-wrapped Failed
    let (is_succ, text) = clean_tool_outcome_summary("\u{1b}[31m✕ Failed: command exited with code 1\u{1b}[0m", None);
    assert!(!is_succ);
    assert_eq!(text, "command exited with code 1");

    // Test diff summary
    let (is_succ, text) = clean_tool_outcome_summary("updated 12 lines (38ms)", Some(true));
    assert!(is_succ);
    assert_eq!(text, "updated 12 lines (38ms)");
}

#[test]
fn test_chat_message_thought_constructor() {
    let thought = ChatMessage::thought("Planning the architecture".into(), Some(1.5));
    assert_eq!(thought.role, "assistant");
    assert_eq!(thought.reasoning.as_deref(), Some("Planning the architecture"));
    assert_eq!(thought.thinking_time, Some(1.5));
    assert!(!thought.is_tool);
    assert!(thought.content.is_empty());
}

#[test]
fn test_agent_activity_from_user_prompt() {
    use crate::channels::ratatui::animation::AgentActivity;

    // Search query
    let act = AgentActivity::from_user_prompt("search for ratatui documentation");
    assert!(matches!(act, AgentActivity::InternetResearch { .. }));

    // Deep research query
    let act = AgentActivity::from_user_prompt("please do a deep research on quantum computing");
    assert!(matches!(act, AgentActivity::DeepResearch { .. }));

    // Writing / Creating
    let act = AgentActivity::from_user_prompt("write a python script to calculate mean");
    assert!(matches!(act, AgentActivity::Writing { .. }));

    // Debugging / Checking
    let act = AgentActivity::from_user_prompt("test openz package with cargo");
    assert!(matches!(act, AgentActivity::Debugging { .. }));

    // Analyzing
    let act = AgentActivity::from_user_prompt("explain how ratatui render pipeline works");
    assert!(matches!(act, AgentActivity::Analyzing { .. }));

    // Planning
    let act = AgentActivity::from_user_prompt("plan the migration to v2");
    assert_eq!(act, AgentActivity::Planning);

    // Default thinking
    let act = AgentActivity::from_user_prompt("hello there");
    assert_eq!(act, AgentActivity::Thinking);
}

#[test]
fn test_render_live_activity_line_formats_all_variants() {
    use crate::channels::ratatui::animation::{render_live_activity_line, AgentActivity, SpinnerStyle};
    use crate::channels::ratatui::theme::Theme;

    let theme = Theme::aura_dark();
    let activities = vec![
        AgentActivity::Thinking,
        AgentActivity::Working,
        AgentActivity::Responding,
        AgentActivity::Generating,
        AgentActivity::Planning,
        AgentActivity::Writing { target: "src/lib.rs".to_string() },
        AgentActivity::Analyzing { task: "benchmark results".to_string() },
        AgentActivity::InternetResearch { query: "Rust tokio".to_string() },
        AgentActivity::DeepResearch { query: "AI architectures".to_string() },
        AgentActivity::RepoResearch { target: "AgentLoop".to_string() },
        AgentActivity::EditingFile { path: "src/main.rs".to_string() },
        AgentActivity::Debugging { step: "cargo clippy".to_string() },
        AgentActivity::ExecutingCommand { command: "ls -la".to_string() },
        AgentActivity::SubagentWorking { role: "Researcher".to_string() },
        AgentActivity::DocumentProcessing { task: "data.xlsx".to_string() },
        AgentActivity::MediaGenerating { task: "flow.svg".to_string() },
        AgentActivity::CompactingContext,
    ];

    for act in activities {
        let line = render_live_activity_line(&act, SpinnerStyle::BrailleWave, 500, 2.3, &theme);
        assert!(!line.spans.is_empty());
        let full_text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(full_text.contains("2.3s"));
    }
}

#[test]
fn test_handle_tool_end_transitions_to_analyzing() {
    use crate::channels::ratatui::animation::AgentActivity;

    let mut app = RatatuiApp::new(
        "test-model".into(),
        "test-provider".into(),
        "cli:test".into(),
    );

    app.handle_tool_start("exec_command".into(), "cargo check -p openz".into());
    assert_eq!(
        app.current_activity,
        Some(AgentActivity::Debugging {
            step: "cargo check -p openz".to_string(),
        })
    );

    app.handle_tool_end(
        "exec_command",
        "Finished dev profile".into(),
        Some("✓ completed".into()),
        true,
        Some(420),
    );

    assert_eq!(
        app.current_activity,
        Some(AgentActivity::Analyzing {
            task: "execution results".to_string(),
        })
    );
}

#[test]
fn test_render_timeline_running_tool_displays_animated_spinner_and_verb() {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use crate::channels::ratatui::timeline::render_timeline;

    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut app = RatatuiApp::new(
        "test-model".into(),
        "test-provider".into(),
        "cli:test".into(),
    );

    app.handle_tool_start("search_web".into(), "Rust async".into());
    // Running tool: tool_success is None

    terminal.draw(|f| {
        render_timeline(f, &mut app, f.area());
    }).unwrap();

    let buffer = terminal.backend().buffer().clone();
    let content: String = (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(content.contains("researching..."), "Expected buffer to contain contextual verb 'researching...', got: {}", content);
}



