use super::*;
use serde_json::json;

fn assistant_tool_call(arguments: serde_json::Value) -> Message {
    let mut extra = serde_json::Map::new();
    extra.insert(
        "tool_calls".to_string(),
        json!([{
            "name": "openmedia_video_create",
            "arguments": arguments,
            "_openz_arg_fingerprint": "old-file-state"
        }]),
    );
    Message {
        role: "assistant".to_string(),
        content: String::new(),
        timestamp: None,
        extra,
    }
}

#[test]
fn scene_path_calls_with_changed_file_fingerprint_are_not_counted_as_duplicate() {
    let args = json!({
        "scene_path": "/tmp/openz_scene.json",
        "output_path": "/tmp/out.mp4"
    });
    let messages = vec![
        Message {
            role: "user".to_string(),
            content: "make video".to_string(),
            timestamp: None,
            extra: serde_json::Map::new(),
        },
        assistant_tool_call(args.clone()),
    ];

    assert_eq!(
        count_previous_tool_calls(&messages, "openmedia_video_create", &args),
        0
    );
}

#[test]
fn unchanged_scene_path_fingerprint_counts_as_duplicate() {
    let path =
        std::env::temp_dir().join(format!("openz_loop_control_{}.json", std::process::id()));
    std::fs::write(&path, r#"{"width":1}"#).unwrap();
    let args = json!({
        "scene_path": path.to_string_lossy(),
        "output_path": "/tmp/out.mp4"
    });
    let fingerprint = tool_arg_fingerprint(&args).unwrap();
    let mut extra = serde_json::Map::new();
    extra.insert(
        "tool_calls".to_string(),
        json!([{
            "name": "openmedia_video_create",
            "arguments": args.clone(),
            "_openz_arg_fingerprint": fingerprint
        }]),
    );
    let messages = vec![
        Message {
            role: "user".to_string(),
            content: "make video".to_string(),
            timestamp: None,
            extra: serde_json::Map::new(),
        },
        Message {
            role: "assistant".to_string(),
            content: String::new(),
            timestamp: None,
            extra,
        },
    ];

    assert_eq!(
        count_previous_tool_calls(&messages, "openmedia_video_create", &args),
        1
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn research_browser_dependency_errors_get_actionable_hint() {
    let hint = generate_self_healing_hint(
        "searchxyz_search_and_read",
        "Failed to start geckodriver on port 4444",
    );
    assert!(hint.contains("browser(action=\"status\")"));
    assert!(hint.contains("browser fallback"));
}

#[test]
fn openmedia_svg_errors_get_schema_specific_hint() {
    let hint = generate_self_healing_hint(
        "openmedia_create_svg",
        "MCP Error: missing field `content`",
    );
    assert!(hint.contains("width"));
    assert!(hint.contains("elements"));
    assert!(hint.contains("content"));
    assert!(hint.contains("text_anchor"));
}

#[test]
fn openmedia_video_errors_get_schema_specific_hint() {
    let hint = generate_self_healing_hint(
        "openmedia_video_create",
        "MCP Error: missing field `anchor`",
    );
    assert!(hint.contains("type=text"));
    assert!(hint.contains("style.font_weight"));
    assert!(hint.contains("scene_path"));
}

fn assistant_named_tool_call(name: &str, id: &str, arguments: serde_json::Value) -> Message {
    let mut extra = serde_json::Map::new();
    extra.insert(
        "tool_calls".to_string(),
        json!([{ "id": id, "name": name, "arguments": arguments }]),
    );
    Message {
        role: "assistant".to_string(),
        content: String::new(),
        timestamp: None,
        extra,
    }
}

fn tool_result(id: &str, name: &str, content: &str) -> Message {
    let mut extra = serde_json::Map::new();
    extra.insert("tool_call_id".to_string(), json!(id));
    extra.insert("name".to_string(), json!(name));
    Message {
        role: "tool".to_string(),
        content: content.to_string(),
        timestamp: None,
        extra,
    }
}

#[test]
fn repeated_read_only_tools_with_new_state_are_not_loops() {
    let args = json!({ "query": "openz" });
    let messages = vec![
        Message {
            role: "user".to_string(),
            content: "research openz".to_string(),
            timestamp: None,
            extra: serde_json::Map::new(),
        },
        assistant_named_tool_call("web_search", "call_1", args.clone()),
        tool_result("call_1", "web_search", "result page 1"),
        assistant_named_tool_call("web_search", "call_2", args.clone()),
        tool_result("call_2", "web_search", "result page 2"),
    ];

    assert_eq!(count_previous_tool_calls(&messages, "web_search", &args), 0);
}

#[test]
fn repeated_grep_search_with_same_args_counts_even_when_results_differ() {
    let args = json!({ "query": "orchestrate_workflow" });
    let messages = vec![
        Message {
            role: "user".to_string(),
            content: "find orchestrate_workflow".to_string(),
            timestamp: None,
            extra: serde_json::Map::new(),
        },
        assistant_named_tool_call("grep_search", "call_1", args.clone()),
        tool_result("call_1", "grep_search", "src/tools/orchestrator.rs"),
        assistant_named_tool_call("grep_search", "call_2", args.clone()),
        tool_result("call_2", "grep_search", "docs/orchestrator-runtime.md"),
    ];

    assert_eq!(
        count_previous_tool_calls(&messages, "grep_search", &args),
        2
    );
}

#[test]
fn repeated_grep_search_counts_default_arg_variants_as_same_lookup() {
    let args = json!({ "query": "orchestrate_workflow" });
    let messages = vec![
        Message {
            role: "user".to_string(),
            content: "find orchestrate_workflow".to_string(),
            timestamp: None,
            extra: serde_json::Map::new(),
        },
        assistant_named_tool_call(
            "grep_search",
            "call_1",
            json!({ "query": "orchestrate_workflow", "dir": ".", "is_regex": false }),
        ),
        tool_result("call_1", "grep_search", "src/tools/orchestrator.rs"),
    ];

    assert_eq!(
        count_previous_tool_calls(&messages, "grep_search", &args),
        1
    );
}

#[test]
fn repeated_read_file_with_same_args_counts_even_when_results_differ() {
    let args = json!({ "path": "src/tools/orchestrator.rs" });
    let messages = vec![
        Message {
            role: "user".to_string(),
            content: "read orchestrator implementation".to_string(),
            timestamp: None,
            extra: serde_json::Map::new(),
        },
        assistant_named_tool_call("read_file", "call_1", args.clone()),
        tool_result("call_1", "read_file", "first chunk"),
        assistant_named_tool_call("read_file", "call_2", args.clone()),
        tool_result("call_2", "read_file", "second chunk"),
    ];

    assert_eq!(count_previous_tool_calls(&messages, "read_file", &args), 2);
}

#[test]
fn repeated_read_file_counts_alias_arg_variants_as_same_lookup() {
    let args = json!({ "path": "src/tools/orchestrator.rs" });
    let messages = vec![
        Message {
            role: "user".to_string(),
            content: "read orchestrator implementation".to_string(),
            timestamp: None,
            extra: serde_json::Map::new(),
        },
        assistant_named_tool_call(
            "read_file",
            "call_1",
            json!({ "file_path": "src/tools/orchestrator.rs" }),
        ),
        tool_result("call_1", "read_file", "first chunk"),
    ];

    assert_eq!(count_previous_tool_calls(&messages, "read_file", &args), 1);
}

#[test]
fn repeated_read_file_after_state_change_does_not_count_previous_read() {
    let args = json!({ "path": "src/tools/orchestrator.rs" });
    let messages = vec![
        Message {
            role: "user".to_string(),
            content: "edit then verify file".to_string(),
            timestamp: None,
            extra: serde_json::Map::new(),
        },
        assistant_named_tool_call("read_file", "call_1", args.clone()),
        tool_result("call_1", "read_file", "before edit"),
        assistant_named_tool_call(
            "write_file",
            "call_2",
            json!({ "path": "src/tools/orchestrator.rs", "content": "after" }),
        ),
        tool_result("call_2", "write_file", "ok"),
    ];

    assert_eq!(count_previous_tool_calls(&messages, "read_file", &args), 0);
}

#[test]
fn repeated_read_only_tools_with_same_state_count_as_stale() {
    let args = json!({ "query": "openz" });
    let messages = vec![
        Message {
            role: "user".to_string(),
            content: "research openz".to_string(),
            timestamp: None,
            extra: serde_json::Map::new(),
        },
        assistant_named_tool_call("web_search", "call_1", args.clone()),
        tool_result("call_1", "web_search", "same result page"),
        assistant_named_tool_call("web_search", "call_2", args.clone()),
        tool_result("call_2", "web_search", "same result page"),
    ];

    assert_eq!(count_previous_tool_calls(&messages, "web_search", &args), 1);
}

#[test]
fn read_only_stale_repeats_block_after_one_duplicate_signature() {
    let args = json!({ "query": "orchestrate_workflow" });

    assert_eq!(tool_repetition_block_threshold("grep_search", &args), 1);
    assert_eq!(
        tool_repetition_block_threshold("read_file", &json!({ "path": "src/lib.rs" })),
        1
    );
    assert_eq!(
        tool_repetition_block_threshold(
            "write_file",
            &json!({ "path": "/tmp/x", "content": "x" })
        ),
        2
    );
}

#[test]
fn repeated_mutating_tools_still_count_even_with_different_outputs() {
    let args = json!({ "path": "/tmp/file.txt", "content": "x" });
    let messages = vec![
        Message {
            role: "user".to_string(),
            content: "write file".to_string(),
            timestamp: None,
            extra: serde_json::Map::new(),
        },
        assistant_named_tool_call("write_file", "call_1", args.clone()),
        tool_result("call_1", "write_file", "ok 1"),
        assistant_named_tool_call("write_file", "call_2", args.clone()),
        tool_result("call_2", "write_file", "ok 2"),
    ];

    assert_eq!(count_previous_tool_calls(&messages, "write_file", &args), 2);
}

#[test]
fn repeated_browser_snapshots_with_new_state_are_not_loops() {
    let args = json!({ "action": "ui_tree" });
    let messages = vec![
        Message {
            role: "user".to_string(),
            content: "use browser and inspect page".to_string(),
            timestamp: None,
            extra: serde_json::Map::new(),
        },
        assistant_named_tool_call("browser", "call_1", args.clone()),
        tool_result("call_1", "browser", "page state 1"),
        assistant_named_tool_call("browser", "call_2", args.clone()),
        tool_result("call_2", "browser", "page state 2"),
    ];

    assert_eq!(
        count_previous_tool_calls(&messages, "browser", &args),
        0
    );
}

#[test]
fn repeated_browser_snapshots_with_same_state_count_as_stale() {
    let args = json!({ "action": "ui_tree" });
    let messages = vec![
        Message {
            role: "user".to_string(),
            content: "use browser and inspect page".to_string(),
            timestamp: None,
            extra: serde_json::Map::new(),
        },
        assistant_named_tool_call("browser", "call_1", args.clone()),
        tool_result("call_1", "browser", "same page state"),
        assistant_named_tool_call("browser", "call_2", args.clone()),
        tool_result("call_2", "browser", "same page state"),
    ];

    assert_eq!(
        count_previous_tool_calls(&messages, "browser", &args),
        1
    );
}

#[test]
fn exact_non_file_tool_calls_are_still_counted_as_duplicates() {
    let args = json!({ "query": "openz" });
    let mut extra = serde_json::Map::new();
    extra.insert(
        "tool_calls".to_string(),
        json!([{ "name": "web_search", "arguments": args.clone() }]),
    );
    let messages = vec![
        Message {
            role: "user".to_string(),
            content: "search".to_string(),
            timestamp: None,
            extra: serde_json::Map::new(),
        },
        Message {
            role: "assistant".to_string(),
            content: String::new(),
            timestamp: None,
            extra,
        },
    ];

    assert_eq!(count_previous_tool_calls(&messages, "web_search", &args), 1);
}

#[test]
fn hashline_mismatch_errors_recommend_include_hashes_and_anchor_text() {
    let hint = generate_self_healing_hint(
        "replace_lines",
        "Hashline mismatch on line 42: expected 'a3f1', found '09bc'",
    );
    assert!(hint.contains("include_hashes: true"));
    assert!(hint.contains("anchor_text"));
}

#[test]
fn patch_file_hunk_mismatch_recommends_search_replace_blocks_and_edits() {
    let hint = generate_self_healing_hint(
        "patch_file",
        "patch_file failed to match hunk at line 10",
    );
    assert!(hint.contains("<<<<<<< SEARCH"));
    assert!(hint.contains("edits"));
    assert!(hint.contains("include_hashes: true"));
}

#[test]
fn tool_not_found_error_recommends_tool_catalog_and_batch_execute() {
    let hint = generate_self_healing_hint(
        "openmedia_audio_trim",
        "Tool 'openmedia_audio_trim' not found",
    );
    assert!(hint.contains("tool_catalog"));
    assert!(hint.contains("batch_execute"));
}

#[test]
fn failed_state_changing_tool_does_not_reset_repeat_counter() {
    let read_args = json!({ "path": "src/tools/orchestrator.rs" });
    let messages = vec![
        Message {
            role: "user".to_string(),
            content: "edit then verify file".to_string(),
            timestamp: None,
            extra: serde_json::Map::new(),
        },
        assistant_named_tool_call("read_file", "call_1", read_args.clone()),
        tool_result("call_1", "read_file", "before edit"),
        assistant_named_tool_call(
            "patch_file",
            "call_2",
            json!({ "path": "src/tools/orchestrator.rs", "diff": "bad" }),
        ),
        tool_result(
            "call_2",
            "patch_file",
            r#"{"error":"Failed to match hunk"}"#,
        ),
    ];

    assert_eq!(
        count_previous_tool_calls(&messages, "read_file", &read_args),
        1
    );
}

#[test]
fn exec_command_repeats_reset_after_distinct_successful_edit() {
    let test_args = json!({ "command": "cargo test --lib" });
    let messages = vec![
        Message {
            role: "user".to_string(),
            content: "fix failing tests".to_string(),
            timestamp: None,
            extra: serde_json::Map::new(),
        },
        assistant_named_tool_call("exec_command", "call_1", test_args.clone()),
        tool_result("call_1", "exec_command", r#"{"exit_code":0,"stdout":"ok"}"#),
        assistant_named_tool_call(
            "patch_file",
            "call_2",
            json!({ "path": "src/lib.rs", "diff": "@@ ..." }),
        ),
        tool_result("call_2", "patch_file", r#"{"status":"success"}"#),
        assistant_named_tool_call("exec_command", "call_3", test_args.clone()),
        tool_result("call_3", "exec_command", r#"{"exit_code":0,"stdout":"ok"}"#),
        assistant_named_tool_call(
            "patch_file",
            "call_4",
            json!({ "path": "src/main.rs", "diff": "@@ ..." }),
        ),
        tool_result("call_4", "patch_file", r#"{"status":"success"}"#),
    ];

    assert_eq!(
        count_previous_tool_calls(&messages, "exec_command", &test_args),
        0
    );
}

#[test]
fn browser_navigation_resets_after_distinct_browser_interaction() {
    let nav_args = json!({ "action": "navigate", "url": "https://example.com/watch" });
    let messages = vec![
        Message {
            role: "user".to_string(),
            content: "open video and click play".to_string(),
            timestamp: None,
            extra: serde_json::Map::new(),
        },
        assistant_named_tool_call("browser", "call_1", nav_args.clone()),
        tool_result("call_1", "browser", r#"{"status":"success"}"#),
        assistant_named_tool_call(
            "browser",
            "call_2",
            json!({ "action": "click", "index": 0 }),
        ),
        tool_result("call_2", "browser", r#"{"status":"success"}"#),
    ];

    assert_eq!(count_previous_tool_calls(&messages, "browser", &nav_args), 0);
}

#[test]
fn periodic_cycle_detector_catches_two_step_ping_pong_loop() {
    let step_a = json!({ "path": "src/lib.rs", "diff": "same broken diff" });
    let step_b = json!({ "command": "cargo check" });
    let messages = vec![
        Message {
            role: "user".to_string(),
            content: "fix build".to_string(),
            timestamp: None,
            extra: serde_json::Map::new(),
        },
        assistant_named_tool_call("patch_file", "c1", step_a.clone()),
        tool_result("c1", "patch_file", r#"{"error":"Failed to match hunk"}"#),
        assistant_named_tool_call("exec_command", "c2", step_b.clone()),
        tool_result("c2", "exec_command", r#"{"exit_code":101,"stderr":"E0425"}"#),
        assistant_named_tool_call("patch_file", "c3", step_a.clone()),
        tool_result("c3", "patch_file", r#"{"error":"Failed to match hunk"}"#),
        assistant_named_tool_call("exec_command", "c4", step_b.clone()),
        tool_result("c4", "exec_command", r#"{"exit_code":101,"stderr":"E0425"}"#),
    ];

    let cycle = detect_trajectory_cycle(&messages, "patch_file", &step_a)
        .expect("expected 2-step periodic cycle");
    assert_eq!(cycle.period, 2);
    assert_eq!(cycle.repetitions, 2);
    assert_eq!(cycle.cycle_tools, vec!["patch_file", "exec_command"]);
}

#[test]
fn sliding_trajectory_window_ages_out_old_reads_on_long_horizon_turns() {
    let target_args = json!({ "path": "src/main.rs" });
    let mut messages = vec![
        Message {
            role: "user".to_string(),
            content: "audit entire repository".to_string(),
            timestamp: None,
            extra: serde_json::Map::new(),
        },
        assistant_named_tool_call("read_file", "call_init", target_args.clone()),
        tool_result("call_init", "read_file", "fn main() {}"),
    ];

    for idx in 0..(TRAJECTORY_WINDOW_CALLS + 2) {
        let id = format!("call_step_{idx}");
        let other_args = json!({ "path": format!("src/module_{idx}.rs") });
        messages.push(assistant_named_tool_call("read_file", &id, other_args));
        messages.push(tool_result(&id, "read_file", "pub struct S;"));
    }

    assert_eq!(
        count_previous_tool_calls(&messages, "read_file", &target_args),
        0
    );
}

#[test]
fn loop_escalation_resets_consecutive_streak_on_forward_progress() {
    let mut state = super::super::security_approval::LoopEscalationState::default();
    assert!(!state.record_loop_block());
    assert!(!state.record_loop_block());
    assert!(!state.record_loop_block());
    state.record_forward_progress();
    assert_eq!(state.consecutive_blocks, 0);
    assert!(!state.record_loop_block());
    assert!(!state.record_loop_block());
    assert!(!state.record_loop_block());
    assert!(state.record_loop_block());
}

#[test]
fn cron_sessions_block_recursive_cron_spawning() {
    let args = json!({ "name": "child", "schedule": "*/5 * * * *", "prompt": "hi" });
    assert!(
        crate::agent::security::SecurityGuard::forbidden_reason_for_session(
            "cron:heartbeat_audit_5m",
            "schedule_job",
            &args
        )
        .is_some()
    );
    assert!(
        crate::agent::security::SecurityGuard::forbidden_reason_for_session(
            "cron:heartbeat_audit_5m",
            "run_job_now",
            &json!({ "id": "abc" })
        )
        .is_some()
    );
    assert!(
        crate::agent::security::SecurityGuard::forbidden_reason_for_session(
            "cli:main",
            "schedule_job",
            &args
        )
        .is_none()
    );
}

#[test]
fn stage1_diagnostic_tag_does_not_break_periodic_cycle_detection() {
    let patch_args = json!({ "path": "src/main.rs", "patch": "@@ -1 +1 @@\n-old\n+new" });
    let read_args = json!({ "path": "src/main.rs" });
    let err1 = json!({ "error": "hunk mismatch" }).to_string();
    let err2_tagged = json!({
        "error": "hunk mismatch",
        "z_guard_diagnostic": {
            "stage": "diagnostic_tag",
            "repeat_attempt": 2,
            "required_action": "inspect_state_and_vary_parameters_before_retry"
        }
    })
    .to_string();
    let messages = vec![
        Message {
            role: "user".to_string(),
            content: "fix main.rs".to_string(),
            timestamp: None,
            extra: serde_json::Map::new(),
        },
        assistant_named_tool_call("patch_file", "c1", patch_args.clone()),
        tool_result("c1", "patch_file", &err1),
        assistant_named_tool_call("read_file", "c2", read_args.clone()),
        tool_result("c2", "read_file", "fn main() {}"),
        assistant_named_tool_call("patch_file", "c3", patch_args.clone()),
        tool_result("c3", "patch_file", &err2_tagged),
        assistant_named_tool_call("read_file", "c4", read_args),
        tool_result("c4", "read_file", "fn main() {}"),
    ];

    let cycle = detect_trajectory_cycle(&messages, "patch_file", &patch_args)
        .expect("period-2 ping-pong cycle should still match across Stage 1 diagnostic tags");
    assert_eq!(cycle.period, 2);
    assert_eq!(cycle.repetitions, 2);
}

#[test]
fn self_owned_tool_output_and_ccr_retrieval_are_autonomous_even_in_high_mode() {
    let ccr_args = json!({ "ccr_id": "ccr_8f21a9" });
    assert!(!crate::agent::security::SecurityGuard::is_sensitive_with_mode(
        "retrieve_original",
        &ccr_args,
        "high"
    ));

    let tool_out = crate::config::loader::runtime_data_dir()
        .join("tool_outputs")
        .join("output_web_fetch_test.json");
    let file_args = json!({ "ccr_id": format!("file://{}", tool_out.display()) });
    assert!(!crate::agent::security::SecurityGuard::is_sensitive_with_mode(
        "retrieve_original",
        &file_args,
        "high"
    ));

    let ssh_args = json!({ "ccr_id": "file:///root/.ssh/id_rsa" });
    assert!(crate::agent::security::SecurityGuard::is_sensitive_with_mode(
        "retrieve_original",
        &ssh_args,
        "low"
    ));
}

