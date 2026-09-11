use super::*;

#[test]
fn canonical_research_topic_strips_tool_phrasing_and_preserves_url_fragments() {
    assert_eq!(
        canonical_research_topic("https://9router.com/#get-started"),
        "9router.com/get-started"
    );
    assert_eq!(
        canonical_research_topic("research this https://9router.com/#get-started"),
        "9router.com/get-started"
    );
    assert_eq!(
        canonical_research_topic("call web_search with query rust tokio async runtime"),
        "rust/tokio"
    );
    assert_eq!(
        canonical_research_topic("web search query rust tokio async runtime"),
        "rust/tokio"
    );
}

#[test]
fn github_labels_are_human_readable() {
    assert_eq!(
        label_for_url("https://github.com/tonbistudio/turboquant-pytorch"),
        "tonbistudio/turboquant-pytorch"
    );
    assert_eq!(
        label_for_url("https://github.com/tonbistudio/turboquant-pytorch/issues/6"),
        "tonbistudio/turboquant-pytorch issue #6"
    );
    assert_eq!(
        label_for_url("https://github.com/barbel-bb/turboquant-cache/pull/12"),
        "barbel-bb/turboquant-cache PR #12"
    );
}

#[test]
fn result_summary_prefers_signal_over_navigation_noise() {
    let result = serde_json::json!({
        "url": "https://github.com/example/openhuman",
        "title": "OpenHuman",
        "content": "OpenHuman GitHub Website Discord More English Overview Getting Started Troubleshooting Features Realtime Mascot Memory Third-party Integrations The Orchestrator Workflows Pricing Billing Legal Terms OpenHuman is a local-first personal AI agent that builds persistent memory, coordinates workflows, and performs deep research across your files and web sources. It stores user context locally and keeps automation approval-gated."
    });

    let summary = result_summary(&result);
    assert!(summary.contains("OpenHuman is a local-first personal AI agent"));
    assert!(summary.contains("persistent memory"));
    assert!(!summary.contains("GitHub Website Discord More English Overview"));
    assert!(!summary.contains("Pricing Billing Legal Terms"));
}

#[tokio::test]
async fn auto_capture_saves_sources_and_brief_from_search_results() {
    let marker = uuid::Uuid::new_v4().to_string();
    let topic = format!("Hermes Agent {}", marker);
    let result = serde_json::json!([
        {
            "title": format!("Hermes Agent docs {}", marker),
            "url": format!("https://hermes-agent.nousresearch.com/docs/{}", marker),
            "snippet": "Official Hermes Agent documentation"
        }
    ]);
    let summary = auto_capture_research_memory(
        "web_search",
        &serde_json::json!({"query": topic}),
        &result,
        "what is hermes agent",
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(summary.sources_saved, 1);
    assert!(summary.brief_saved);
    assert_eq!(
        canonical_research_topic("https://github.com/mem0ai/mem0?utm_source=chatgpt.com"),
        "mem0ai/mem0"
    );
    assert_eq!(
        canonical_research_topic("https://sakana.ai/fugu/"),
        "sakana.ai/fugu"
    );
    assert_eq!(
        canonical_research_topic("https://www.duix.com/pricing?utm_source=test"),
        "duix.com/pricing"
    );
    assert_eq!(canonical_research_topic("what is mem0"), "mem0");
    assert_eq!(canonical_research_topic("hey whats hermes"), "hermes");
    assert_eq!(
        canonical_research_topic("ok now tell me about mem0"),
        "mem0"
    );
    assert_eq!(
        canonical_research_topic(
            "https://github.com/tinyhumansai/openhuman research about this and tell me about this"
        ),
        "tinyhumansai/openhuman"
    );
    let matches = crate::tools::shared_memory::search_source_bookmarks(&marker, 5)
        .await
        .unwrap();
    assert!(matches.iter().any(|m| m.uri.contains(&marker)));
    for item in matches.into_iter().filter(|m| m.uri.contains(&marker)) {
        let _ = crate::tools::shared_memory::delete_source(&item.id).await;
    }
    let _ =
        crate::tools::shared_memory::delete_research_brief(&format!("Hermes Agent {}", marker))
            .await;
}
#[tokio::test]
async fn auto_capture_skips_refresh_only_url_checks() {
    let marker = uuid::Uuid::new_v4().to_string();
    let url = format!("https://github.com/example/recheck-{marker}");
    let result = serde_json::json!({
        "url": url,
        "content": "Example project is an open-source research repository with useful implementation notes."
    });

    let summary = auto_capture_research_memory(
        "web_fetch",
        &serde_json::json!({"url": format!("https://github.com/example/recheck-{marker}")}),
        &result,
        &format!("check again https://github.com/example/recheck-{marker}"),
    )
    .await
    .unwrap();

    assert!(summary.is_none());
}

#[tokio::test]
async fn auto_capture_skips_download_and_display_workflows() {
    let marker = uuid::Uuid::new_v4().to_string();
    let url = format!("https://example.com/wallpaper-{marker}.jpg");
    let result = serde_json::json!({
        "title": format!("Wallpaper {marker}"),
        "url": url,
        "content": "A dark desktop wallpaper download result."
    });

    let summary = auto_capture_research_memory(
        "web_search",
        &serde_json::json!({"query": format!("hollow knight dark wallpaper 4k {marker}")}),
        &result,
        &format!(
            "find a good platform to download a dark wallpaper image and show it to me {marker}"
        ),
    )
    .await
    .unwrap();

    assert!(summary.is_none());
    let matches = crate::tools::shared_memory::search_research_briefs(&marker, 5)
        .await
        .unwrap();
    assert!(matches.iter().all(|item| !item.topic.contains(&marker)));
}

#[tokio::test]
async fn auto_capture_skips_non_research_debug_turns() {
    let marker = uuid::Uuid::new_v4().to_string();
    let url = format!("https://example.com/debug-{marker}");
    let result = serde_json::json!({
        "url": url,
        "content": "Example.com is a documentation page used for a quick runtime check."
    });

    let summary = auto_capture_research_memory(
        "web_fetch",
        &serde_json::json!({"url": format!("https://example.com/debug-{marker}")}),
        &result,
        "why u needed source code",
    )
    .await
    .unwrap();

    assert!(summary.is_none());
    let matches = crate::tools::shared_memory::search_research_briefs(&marker, 5)
        .await
        .unwrap();
    assert!(matches.iter().all(|item| !item.topic.contains(&marker)));
}

#[tokio::test]
async fn auto_capture_ignores_skipped_saved_brief_results() {
    let marker = uuid::Uuid::new_v4().to_string();
    let result = serde_json::json!({
        "status": "skipped",
        "reason": "Skipped web/search lookup: a fresh saved research brief already matches this non-latest query."
    });

    let summary = auto_capture_research_memory(
        "web_fetch",
        &serde_json::json!({"url": format!("https://github.com/example/{marker}")}),
        &result,
        &format!("hey whats {marker}"),
    )
    .await
    .unwrap();

    assert!(summary.is_none());
    let matches = crate::tools::shared_memory::search_research_briefs(&marker, 5)
        .await
        .unwrap();
    assert!(!matches.iter().any(|item| item.topic.contains(&marker)));
}

#[tokio::test]
async fn auto_capture_repo_brief_uses_week_ttl() {
    let marker = uuid::Uuid::new_v4().to_string();
    let url = format!("https://github.com/example/dox-{marker}");
    let user_content = format!("{url} hey research about this and tell me about this");
    let result = serde_json::json!({
        "title": format!("DOX {marker}"),
        "url": url,
        "content": "DOX is a self-documenting AGENTS.md framework for AI coding agents."
    });

    let summary = auto_capture_research_memory(
        "web_fetch",
        &serde_json::json!({"url": url}),
        &result,
        &user_content,
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(summary.topic, format!("example/dox-{marker}"));
    let briefs = crate::tools::shared_memory::search_research_briefs(&summary.topic, 1)
        .await
        .unwrap();
    assert_eq!(briefs[0].topic, summary.topic);
    assert!(briefs[0].stale_after_secs >= 604_800);

    let sources = crate::tools::shared_memory::search_source_bookmarks(&marker, 5)
        .await
        .unwrap();
    for source in sources.into_iter().filter(|s| s.uri.contains(&marker)) {
        let _ = crate::tools::shared_memory::delete_source(&source.id).await;
    }
    let _ = crate::tools::shared_memory::delete_research_brief(&summary.topic).await;
}

#[tokio::test]
async fn auto_capture_uses_canonical_website_url_topic() {
    let marker = uuid::Uuid::new_v4().to_string();
    let url = format!("https://sakana.ai/fugu-{marker}/");
    let result = serde_json::json!({
        "title": format!("Sakana Fugu {marker}"),
        "url": url,
        "content": "Sakana Fugu is a multi-agent orchestration model exposed through an OpenAI-compatible API."
    });

    let summary = auto_capture_research_memory(
        "web_fetch",
        &serde_json::json!({"url": format!("https://sakana.ai/fugu-{marker}/")}),
        &result,
        &format!("hey research about this https://sakana.ai/fugu-{marker}/"),
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(summary.topic, format!("sakana.ai/fugu-{marker}"));
    let matches = crate::tools::shared_memory::search_research_briefs(
        &format!("what is sakana fugu-{marker}"),
        5,
    )
    .await
    .unwrap();
    assert!(matches.iter().any(|item| item.topic == summary.topic));

    let sources = crate::tools::shared_memory::search_source_bookmarks(&marker, 5)
        .await
        .unwrap();
    for source in sources.into_iter().filter(|s| s.uri.contains(&marker)) {
        let _ = crate::tools::shared_memory::delete_source(&source.id).await;
    }
    let _ = crate::tools::shared_memory::delete_research_brief(&summary.topic).await;
}

#[tokio::test]
async fn auto_capture_prefers_existing_repo_topic_for_short_alias() {
    let marker = uuid::Uuid::new_v4().to_string();
    let repo_url = format!("https://github.com/NousResearch/hermes-agent-{marker}");
    let source = crate::tools::shared_memory::add_source_bookmark(
        &format!("NousResearch/hermes-agent-{marker}"),
        "repo",
        &repo_url,
        vec![format!("hermes-{marker}")],
        "Official Hermes Agent repository",
        0.95,
        604800,
    )
    .await
    .unwrap();
    let result = serde_json::json!({
        "title": format!("Hermes Agent docs {marker}"),
        "url": format!("https://hermes-agent.nousresearch.com/docs/{marker}"),
        "content": "Hermes Agent is a self-improving AI agent framework with tools, skills, and messaging channels."
    });

    let summary = auto_capture_research_memory(
        "web_fetch",
        &serde_json::json!({"url": format!("https://hermes-agent.nousresearch.com/docs/{marker}")}),
        &result,
        &format!("so whats hermes-{marker}"),
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(summary.topic, format!("nousresearch/hermes-agent-{marker}"));
    let _ = crate::tools::shared_memory::delete_source(&source.id).await;
    let _ = crate::tools::shared_memory::delete_research_brief(&summary.topic).await;
}

#[tokio::test]
async fn auto_capture_uses_repo_topic_from_result_url_for_simple_followup() {
    let marker = uuid::Uuid::new_v4().to_string();
    let repo = format!("openhuman-{marker}");
    let repo_url = format!("https://github.com/tinyhumansai/{repo}");
    let result = serde_json::json!({
        "title": format!("OpenHuman {marker}"),
        "url": repo_url,
        "content": "OpenHuman is a local-first personal AI agent platform with memory, workflows, integrations, and research tools."
    });

    let summary = auto_capture_research_memory(
        "web_fetch",
        &serde_json::json!({"query": format!("what is openhuman-{marker}")}),
        &result,
        &format!("hey whats openhuman-{marker}"),
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(summary.topic, format!("tinyhumansai/{repo}"));
    let generic = crate::tools::shared_memory::search_research_briefs(
        &format!("what is openhuman-{marker}"),
        1,
    )
    .await
    .unwrap();
    assert_eq!(generic[0].topic, summary.topic);

    let sources = crate::tools::shared_memory::search_source_bookmarks(&marker, 5)
        .await
        .unwrap();
    for source in sources.into_iter().filter(|s| s.uri.contains(&marker)) {
        let _ = crate::tools::shared_memory::delete_source(&source.id).await;
    }
    let _ = crate::tools::shared_memory::delete_research_brief(&summary.topic).await;
    let _ = crate::tools::shared_memory::delete_research_brief(&format!("openhuman-{marker}"))
        .await;
}

#[tokio::test]
async fn auto_capture_followup_with_repo_url_keeps_canonical_repo_topic() {
    let marker = uuid::Uuid::new_v4().to_string();
    let url = format!("https://github.com/example/dox-{marker}");
    let result = serde_json::json!({
        "title": format!("DOX {marker}"),
        "url": url,
        "content": "DOX is a self-documenting AGENTS.md framework for AI coding agents."
    });

    let summary = auto_capture_research_memory(
        "web_fetch",
        &serde_json::json!({"url": url}),
        &result,
        &format!("hey whats dox-{marker}"),
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(summary.topic, format!("example/dox-{marker}"));
    let generic = crate::tools::shared_memory::search_research_briefs(
        &format!("what is dox-{marker}"),
        1,
    )
    .await
    .unwrap();
    assert_eq!(generic[0].topic, summary.topic);

    let sources = crate::tools::shared_memory::search_source_bookmarks(&marker, 5)
        .await
        .unwrap();
    for source in sources.into_iter().filter(|s| s.uri.contains(&marker)) {
        let _ = crate::tools::shared_memory::delete_source(&source.id).await;
    }
    let _ = crate::tools::shared_memory::delete_research_brief(&summary.topic).await;
    let _ = crate::tools::shared_memory::delete_research_brief(&format!("dox-{marker}")).await;
}
