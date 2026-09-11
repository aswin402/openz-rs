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
    assert!(hint.contains("inspect_browsers"));
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
fn repeated_gsd_browser_snapshots_with_new_state_are_not_loops() {
    let args = json!({ "action": "snapshot" });
    let messages = vec![
        Message {
            role: "user".to_string(),
            content: "use browser and inspect page".to_string(),
            timestamp: None,
            extra: serde_json::Map::new(),
        },
        assistant_named_tool_call("gsd_browser", "call_1", args.clone()),
        tool_result("call_1", "gsd_browser", "page state 1"),
        assistant_named_tool_call("gsd_browser", "call_2", args.clone()),
        tool_result("call_2", "gsd_browser", "page state 2"),
    ];

    assert_eq!(
        count_previous_tool_calls(&messages, "gsd_browser", &args),
        0
    );
}

#[test]
fn repeated_gsd_browser_snapshots_with_same_state_count_as_stale() {
    let args = json!({ "action": "snapshot" });
    let messages = vec![
        Message {
            role: "user".to_string(),
            content: "use browser and inspect page".to_string(),
            timestamp: None,
            extra: serde_json::Map::new(),
        },
        assistant_named_tool_call("gsd_browser", "call_1", args.clone()),
        tool_result("call_1", "gsd_browser", "same page state"),
        assistant_named_tool_call("gsd_browser", "call_2", args.clone()),
        tool_result("call_2", "gsd_browser", "same page state"),
    ];

    assert_eq!(
        count_previous_tool_calls(&messages, "gsd_browser", &args),
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
