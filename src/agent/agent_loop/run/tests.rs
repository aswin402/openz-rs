//! Unit tests for agent loop run execution, tool call normalization, and artifact handling.

use super::artifacts::*;
use super::events::*;
use super::research::*;
use super::tool_pipeline::*;
use super::turn::*;
use serde_json::json;

#[test]
fn web_fetch_latest_intent_injects_revalidate_cache_mode() {
    let call = crate::providers::ToolCallRequest {
        id: "call_1".to_string(),
        name: "web_fetch".to_string(),
        arguments: json!({ "url": "https://example.com" }),
    };
    let normalized = auto_adjust_tool_call_for_user_intent(
        call,
        "check the latest version from https://example.com",
    );
    assert_eq!(normalized.arguments["cache_mode"], "revalidate");
}

#[test]
fn web_fetch_explicit_cache_mode_is_preserved() {
    let call = crate::providers::ToolCallRequest {
        id: "call_1".to_string(),
        name: "web_fetch".to_string(),
        arguments: json!({ "url": "https://example.com", "cache_mode": "prefer_cache" }),
    };
    let normalized = auto_adjust_tool_call_for_user_intent(call, "check latest");
    assert_eq!(normalized.arguments["cache_mode"], "prefer_cache");
}

#[test]
fn read_file_for_saved_tool_output_is_rewritten_to_retrieve_original() {
    let output_path = crate::config::loader::runtime_data_dir()
        .join("tool_outputs")
        .join("output_big_tool.json");
    let file_url = format!("file://{}", output_path.display());
    let call = crate::providers::ToolCallRequest {
        id: "call_read_output".to_string(),
        name: "read_file".to_string(),
        arguments: json!({ "path": file_url }),
    };
    let normalized = auto_adjust_tool_call_for_user_intent(call, "show the full output");
    assert_eq!(normalized.name, "retrieve_original");
    assert_eq!(normalized.id, "call_read_output");
    assert_eq!(normalized.arguments["ccr_id"], file_url);
}

#[test]
fn read_file_for_normal_project_file_is_preserved() {
    let call = crate::providers::ToolCallRequest {
        id: "call_read_file".to_string(),
        name: "read_file".to_string(),
        arguments: json!({ "path": "src/main.rs" }),
    };
    let normalized = auto_adjust_tool_call_for_user_intent(call, "show the file");
    assert_eq!(normalized.name, "read_file");
    assert_eq!(normalized.arguments["path"], "src/main.rs");
}

#[test]
fn first_edit_call_is_replaced_with_scope_context() {
    let call = crate::providers::ToolCallRequest {
        id: "call_edit".to_string(),
        name: "patch_file".to_string(),
        arguments: json!({ "path": "src/main.rs", "patch": "---" }),
    };
    let mut scoped = std::collections::HashSet::new();
    let normalized = auto_scope_context_before_edit(call, &mut scoped);
    assert_eq!(normalized.name, "scope_context");
    assert_eq!(normalized.id, "call_edit");
    assert_eq!(normalized.arguments["target_path"], "src/main.rs");
    assert!(scoped.contains("src/main.rs"));
}

#[test]
fn first_write_to_new_file_scopes_existing_parent_directory() {
    let call = crate::providers::ToolCallRequest {
        id: "call_new_file".to_string(),
        name: "write_file".to_string(),
        arguments: json!({ "path": "src/__openz_auto_scope_new_file.rs", "content": "" }),
    };
    let mut scoped = std::collections::HashSet::new();
    let normalized = auto_scope_context_before_edit(call, &mut scoped);
    assert_eq!(normalized.name, "scope_context");
    assert_eq!(normalized.arguments["target_path"], "src");
    assert!(scoped.contains("src"));
}

#[test]
fn second_edit_call_for_same_path_is_preserved() {
    let call = crate::providers::ToolCallRequest {
        id: "call_edit".to_string(),
        name: "patch_file".to_string(),
        arguments: json!({ "path": "src/main.rs", "patch": "---" }),
    };
    let mut scoped = std::collections::HashSet::from(["src/main.rs".to_string()]);
    let normalized = auto_scope_context_before_edit(call, &mut scoped);
    assert_eq!(normalized.name, "patch_file");
}

#[test]
fn show_intent_and_generated_output_path_builds_auto_open_call() {
    let call = crate::providers::ToolCallRequest {
        id: "call_image".to_string(),
        name: "generate_image".to_string(),
        arguments: json!({ "output_path": "/tmp/demo.png" }),
    };
    let result = json!({ "status": "success", "output_path": "/tmp/demo.png" });
    let mut opened = std::collections::HashSet::new();
    let open_call = auto_open_artifact_call_after_tool(
        "make an image and show it",
        &call,
        &result,
        &mut opened,
    )
    .expect("auto open call");
    assert_eq!(open_call.name, "open_path");
    assert_eq!(open_call.id, "auto_open_call_image");
    assert_eq!(open_call.arguments["target"], "/tmp/demo.png");
}

#[test]
fn generated_artifact_without_show_intent_does_not_auto_open() {
    let call = crate::providers::ToolCallRequest {
        id: "call_image".to_string(),
        name: "generate_image".to_string(),
        arguments: json!({ "output_path": "/tmp/demo.png" }),
    };
    let result = json!({ "status": "success", "output_path": "/tmp/demo.png" });
    let mut opened = std::collections::HashSet::new();
    assert!(
        auto_open_artifact_call_after_tool("make an image", &call, &result, &mut opened)
            .is_none()
    );
}

#[test]
fn auto_open_dedupes_same_artifact_path() {
    let call = crate::providers::ToolCallRequest {
        id: "call_video".to_string(),
        name: "html_to_video".to_string(),
        arguments: json!({ "output_path": "/tmp/demo.mp4" }),
    };
    let result = json!({ "status": "success", "output_path": "/tmp/demo.mp4" });
    let mut opened = std::collections::HashSet::new();
    assert!(
        auto_open_artifact_call_after_tool("show the video", &call, &result, &mut opened)
            .is_some()
    );
    assert!(
        auto_open_artifact_call_after_tool("show the video", &call, &result, &mut opened)
            .is_none()
    );
}

#[test]
fn failed_open_path_builds_device_inventory_suggestion_call() {
    let call = crate::providers::ToolCallRequest {
        id: "call_open".to_string(),
        name: "open_path".to_string(),
        arguments: json!({ "target": "/tmp/demo.pdf" }),
    };
    let result = json!({ "error": "Failed to open '/tmp/demo.pdf': no application found" });
    let mut suggested = std::collections::HashSet::new();

    let suggest_call =
        auto_device_inventory_suggest_call_after_open_failure(&call, &result, &mut suggested)
            .expect("device inventory suggestion call");

    assert_eq!(suggest_call.name, "device_inventory");
    assert_eq!(suggest_call.id, "auto_device_inventory_call_open");
    assert_eq!(suggest_call.arguments["action"], "suggest");
    assert_eq!(suggest_call.arguments["category"], "pdf_viewer");
    assert_eq!(suggest_call.arguments["target"], "/tmp/demo.pdf");
}

#[test]
fn failed_open_path_suggestion_dedupes_same_target() {
    let call = crate::providers::ToolCallRequest {
        id: "call_open".to_string(),
        name: "open_path".to_string(),
        arguments: json!({ "target": "/tmp/demo.png" }),
    };
    let result = json!({ "error": "Failed to open '/tmp/demo.png': no application found" });
    let mut suggested = std::collections::HashSet::new();

    assert!(
        auto_device_inventory_suggest_call_after_open_failure(&call, &result, &mut suggested)
            .is_some()
    );
    assert!(
        auto_device_inventory_suggest_call_after_open_failure(&call, &result, &mut suggested)
            .is_none()
    );
}

#[test]
fn browser_backed_readers_count_as_research_lookup_tools() {
    assert!(is_research_lookup_tool("obscura_browser"));
    assert!(is_research_lookup_tool("firefox_browser"));
    assert!(is_research_lookup_tool("gsd_browser"));
    assert!(is_research_lookup_tool("searchxyz_browser_search"));
}

#[test]
fn tui_thought_display_modes_normalize() {
    assert_eq!(normalize_tui_thought_display("full"), "full");
    assert_eq!(normalize_tui_thought_display("summary"), "compact");
    assert_eq!(normalize_tui_thought_display("off"), "off");
    assert!(should_show_tui_thoughts("full"));
    assert!(should_show_tui_thoughts("compact"));
    assert!(!should_show_tui_thoughts("off"));
}

#[test]
fn direct_research_url_deduplicates_fragments_across_readers() {
    let first = direct_research_url(
        "web_fetch",
        &serde_json::json!({"url": "https://9router.com/#get-started"}),
    );
    let second = direct_research_url(
        "searchxyz_read_url",
        &serde_json::json!({"url": "https://9router.com/"}),
    );
    assert_eq!(first, second);
}

#[test]
fn direct_research_url_ignores_non_url_research_tools() {
    assert!(
        direct_research_url(
            "web_search",
            &serde_json::json!({"query": "9router get started"}),
        )
        .is_none()
    );
}

#[test]
fn direct_page_scope_requires_explicit_broader_intent() {
    assert!(direct_page_research_only(
        "research this https://9router.com/#get-started"
    ));
    assert!(!direct_page_research_only(
        "research this https://9router.com/#get-started and compare related sources"
    ));
    assert!(!direct_page_research_only(
        "scrape the source code and assets from https://example.com/game"
    ));
}

#[test]
fn public_reasoning_progress_is_hidden_by_default() {
    assert!(!should_send_public_reasoning_progress("off"));
    assert!(!should_send_public_reasoning_progress("hidden"));
    assert!(should_send_public_reasoning_progress("compact"));
    assert!(should_send_public_reasoning_progress("full"));
}

#[test]
fn compact_reasoning_summary_truncates_long_text() {
    let raw = "word ".repeat(120);
    let compact = compact_reasoning_summary(&raw);
    assert!(compact.chars().count() <= 360);
    assert!(compact.ends_with("..."));
}

#[test]
fn compact_reasoning_summary_never_returns_full_long_reasoning() {
    let raw = "internal step ".repeat(80);
    let compact = compact_reasoning_summary(&raw);
    assert!(compact.chars().count() <= 360);
    assert_ne!(compact, raw);
}

#[tokio::test]
async fn fresh_brief_blocks_non_latest_research_tools() {
    let marker = uuid::Uuid::new_v4().to_string();
    let topic = format!("OpenHuman {marker}");
    crate::tools::shared_memory::save_research_brief(
        &topic,
        "OpenHuman is a local-first personal AI agent.",
        vec![],
        0.8,
        86400,
    )
    .await
    .unwrap();
    assert!(
        fresh_research_brief_blocks_lookup(
            &format!("what is openhuman {marker}"),
            "web_fetch",
            &serde_json::json!({}),
        )
        .await
    );
    assert!(
        !fresh_research_brief_blocks_lookup(
            &format!("latest openhuman {marker} release"),
            "web_fetch",
            &serde_json::json!({}),
        )
        .await
    );
    assert!(
        !fresh_research_brief_blocks_lookup(
            &format!("check this https://example.com/{marker}"),
            "web_fetch",
            &serde_json::json!({}),
        )
        .await
    );
    assert!(
        fresh_research_brief_blocks_lookup(
            &format!("what is openhuman {marker}"),
            "web_fetch",
            &serde_json::json!({ "url": format!("https://example.com/{marker}") }),
        )
        .await
    );
    assert!(
        !fresh_research_brief_blocks_lookup(
            &format!("go and check again openhuman {marker}"),
            "web_search",
            &serde_json::json!({}),
        )
        .await
    );
    assert!(
        !fresh_research_brief_blocks_lookup(
            &format!("what is openhuman {marker}"),
            "read_file",
            &serde_json::json!({}),
        )
        .await
    );
    assert!(
        !fresh_research_brief_blocks_lookup(
            &format!("research about openhuman {marker} and tell me about this"),
            "web_fetch",
            &serde_json::json!({}),
        )
        .await
    );
    let _ = crate::tools::shared_memory::delete_research_brief(&topic).await;
}

#[test]
fn auto_capture_notice_dedupes_topics() {
    let summaries = vec![
        crate::tools::shared_memory::AutoCaptureSummary {
            sources_saved: 5,
            brief_saved: true,
            topic: "hermes".to_string(),
        },
        crate::tools::shared_memory::AutoCaptureSummary {
            sources_saved: 5,
            brief_saved: true,
            topic: "hermes".to_string(),
        },
        crate::tools::shared_memory::AutoCaptureSummary {
            sources_saved: 1,
            brief_saved: false,
            topic: "mem0".to_string(),
        },
    ];
    assert_eq!(summarize_auto_capture_topics(&summaries), "hermes, mem0");
    assert_eq!(count_unique_auto_capture_brief_topics(&summaries), 1);
}

#[test]
fn resolves_tool_timeout_with_bounded_explicit_override() {
    assert_eq!(
        resolve_tool_timeout_secs(
            "web_fetch",
            &serde_json::json!({ "_timeout_secs": 1 }),
            Some(600),
            300,
        ),
        crate::tools::MIN_TOOL_TIMEOUT_SECS
    );
    assert_eq!(
        resolve_tool_timeout_secs(
            "web_fetch",
            &serde_json::json!({ "_timeout_secs": 999_999 }),
            Some(600),
            300,
        ),
        crate::tools::MAX_TOOL_TIMEOUT_SECS
    );
}

#[test]
fn delegate_timeout_raises_outer_tool_timeout() {
    assert_eq!(
        resolve_tool_timeout_secs(
            "delegate_task",
            &serde_json::json!({ "timeout_secs": 900 }),
            Some(600),
            300,
        ),
        900
    );
    assert_eq!(
        resolve_tool_timeout_secs(
            "reviewer",
            &serde_json::json!({ "timeout_secs": 900 }),
            None,
            300,
        ),
        900
    );
}

#[test]
fn parallel_research_uses_largest_task_timeout_for_outer_tool() {
    assert_eq!(
        resolve_tool_timeout_secs(
            "parallel_research",
            &serde_json::json!({
                "tasks": [
                    { "goal": "quick", "timeout_secs": 120 },
                    { "goal": "deep", "timeout_secs": 900 }
                ]
            }),
            Some(600),
            300,
        ),
        900
    );
}

#[test]
fn tool_timeout_does_not_count_as_user_cancel() {
    assert!(!should_cancel_turn_after_tool_error(
        "Tool execution timed out after 300s"
    ));
    assert!(should_cancel_turn_after_tool_error("Cancelled by user"));
    assert!(should_cancel_turn_after_tool_error(
        "Subagent task cancelled"
    ));
}

#[test]
fn provider_turn_lock_key_is_opt_in_for_fragile_models() {
    assert!(
        provider_turn_lock_key_for_mode("opencode_zen", "deepseek-v4-flash-free", None).is_none()
    );
    assert!(
        provider_turn_lock_key_for_mode(
            "opencode_zen",
            "deepseek-v4-flash-free",
            Some("fragile")
        )
        .is_some()
    );
    assert!(
        provider_turn_lock_key_for_mode("openrouter", "qwen/qwen3:free", Some("free")).is_some()
    );
    assert!(provider_turn_lock_key_for_mode("openai", "gpt-4o", Some("fragile")).is_none());
    assert!(provider_turn_lock_key_for_mode("openai", "gpt-4o", Some("all")).is_some());
}

#[test]
fn process_tool_guard_respects_resource_limit() {
    let first = crate::tools::resource_policy::try_acquire_process_tool(1)
        .expect("first process slot should be available");
    let err = crate::tools::resource_policy::try_acquire_process_tool(1)
        .expect_err("second process slot should be blocked");
    assert!(err.contains("process tool limit reached"));
    drop(first);
    assert!(crate::tools::resource_policy::try_acquire_process_tool(1).is_ok());
}
