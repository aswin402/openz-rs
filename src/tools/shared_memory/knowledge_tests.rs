use super::*;

#[tokio::test]
async fn research_brief_accepts_goal_context_without_action() {
    let marker = uuid::Uuid::new_v4().to_string();
    let tool = ResearchBriefTool;
    let res = tool
        .call(&json!({
            "goal": format!("Hermes Agent comparison {}", marker),
            "context": "Hermes Agent is a Python/TypeScript self-improving AI agent framework."
        }))
        .await
        .unwrap();
    assert_eq!(res.get("status").and_then(|v| v.as_str()), Some("success"));
    let topic = format!("Hermes Agent comparison {}", marker);
    let canonical = canonical_research_topic(&topic);
    let matches = search_research_briefs(&topic, 1).await.unwrap();
    assert!(matches.iter().any(|m| m.topic == canonical));
    assert_eq!(delete_research_brief(&topic).await.unwrap(), 1);
}

#[tokio::test]
async fn research_brief_merges_question_alias_into_existing_repo_topic() {
    let marker = uuid::Uuid::new_v4().to_string();
    let url_topic = format!("https://github.com/example/{marker}?utm_source=chatgpt.com");
    save_research_brief(
        &url_topic,
        "Example repository documentation explains the project purpose and setup workflow.",
        vec![],
        0.7,
        86400,
    )
    .await
    .unwrap();
    save_research_brief(
        &format!("what is {marker}"),
        "Example repository alias update contains a useful description from a follow-up question.",
        vec![],
        0.8,
        86400,
    )
    .await
    .unwrap();
    let url_matches = search_research_briefs(&url_topic, 5).await.unwrap();
    assert!(url_matches
        .iter()
        .any(|m| m.topic == format!("example/{marker}")));
    let question_matches = search_research_briefs(&format!("what is {marker}"), 5)
        .await
        .unwrap();
    let canonical_topic = format!("example/{marker}");
    assert!(question_matches
        .iter()
        .any(|m| { m.topic == canonical_topic && m.summary == "Example repository alias update contains a useful description from a follow-up question." }));
    let _ = delete_research_brief(&url_topic).await;
    let _ = delete_research_brief(&marker).await;
}

#[tokio::test]
async fn repair_research_brief_topics_merges_alias_into_existing_repo_topic() {
    let marker = uuid::Uuid::new_v4().to_string();
    let alias_topic = format!("hermes-{marker}");
    let canonical_topic = format!("nousresearch/hermes-agent-{marker}");
    let source = add_source_bookmark(
        &canonical_topic,
        "repo",
        &format!("https://github.com/NousResearch/hermes-agent-{marker}"),
        vec![alias_topic.clone()],
        "Official Hermes Agent repository",
        0.95,
        604800,
    )
    .await
    .unwrap();
    save_research_brief(
        &canonical_topic,
        "Canonical Hermes Agent brief with current repository context.",
        vec![source.id.clone()],
        0.7,
        604800,
    )
    .await
    .unwrap();

    let id = uuid::Uuid::new_v4().to_string();
    let now = now_rfc3339();
    let source_ids = serde_json::to_string(&vec![source.id.clone()]).unwrap();
    {
        let _lock = get_db_mutex().lock().await;
        with_db(|conn| {
            conn.execute(
                "INSERT INTO research_briefs (id, topic, summary, source_ids, confidence, stale_after_secs, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
                params![
                    id,
                    alias_topic,
                    "Alias Hermes brief should merge into the canonical repository topic.",
                    source_ids,
                    0.9,
                    604800,
                    now
                ],
            )?;
            Ok(())
        })
        .unwrap();
    }

    assert!(repair_research_brief_topics().await.unwrap() >= 1);
    let matches = search_research_briefs(&format!("what is hermes-{marker}"), 5)
        .await
        .unwrap();
    assert!(matches.iter().any(|item| item.topic == canonical_topic));
    assert!(matches.iter().all(|item| item.topic != alias_topic));

    let _ = delete_source(&source.id).await;
    let _ = delete_research_brief(&canonical_topic).await;
    let _ = delete_research_brief(&alias_topic).await;
}

#[tokio::test]
async fn repair_research_brief_topics_renames_website_space_topic() {
    let marker = uuid::Uuid::new_v4().to_string();
    let url = format!("https://sakana.ai/fugu-{marker}/");
    let canonical_topic = format!("sakana.ai/fugu-{marker}");
    let legacy_topic = format!("sakana.ai fugu-{marker}");
    let source = add_source_bookmark(
        &format!("Sakana Fugu {marker}"),
        "website",
        &url,
        vec![format!("sakana fugu-{marker}")],
        "Sakana Fugu product page",
        0.85,
        604800,
    )
    .await
    .unwrap();

    let id = uuid::Uuid::new_v4().to_string();
    let now = now_rfc3339();
    let source_ids = serde_json::to_string(&vec![source.id.clone()]).unwrap();
    {
        let _lock = get_db_mutex().lock().await;
        with_db(|conn| {
            conn.execute(
                "INSERT INTO research_briefs (id, topic, summary, source_ids, confidence, stale_after_secs, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
                params![
                    id,
                    legacy_topic,
                    "Sakana Fugu is a multi-agent orchestration product page.",
                    source_ids,
                    0.8,
                    604800,
                    now
                ],
            )?;
            Ok(())
        })
        .unwrap();
    }

    assert!(repair_research_brief_topics().await.unwrap() >= 1);
    let matches = search_research_briefs(&format!("what is sakana fugu-{marker}"), 5)
        .await
        .unwrap();
    assert!(matches.iter().any(|item| item.topic == canonical_topic));
    assert!(matches.iter().all(|item| item.topic != legacy_topic));

    let _ = delete_source(&source.id).await;
    let _ = delete_research_brief(&canonical_topic).await;
    let _ = delete_research_brief(&legacy_topic).await;
}

#[tokio::test]
async fn research_brief_search_ignores_skipped_placeholder_summaries() {
    let marker = uuid::Uuid::new_v4().to_string();
    let topic = format!("tinyhumansai/openhuman-{marker}");
    let id = uuid::Uuid::new_v4().to_string();
    let now = now_rfc3339();
    {
        let _lock = get_db_mutex().lock().await;
        with_db(|conn| {
            conn.execute(
                "INSERT INTO research_briefs (id, topic, summary, source_ids, confidence, stale_after_secs, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
                params![id, topic, "skipped", "[]", 0.65, 604_800, now],
            )?;
            Ok(())
        })
        .unwrap();
    }

    let matches = search_research_briefs(&format!("what is openhuman-{marker}"), 5)
        .await
        .unwrap();
    assert!(!matches.iter().any(|item| item.topic == topic));

    let _ = delete_research_brief(&topic).await;
}

#[tokio::test]
async fn knowledge_source_crud_searches_aliases() {
    let item = add_source_bookmark(
        "Hermes Agent",
        "docs",
        &format!(
            "https://hermes-agent.nousresearch.com/docs/{}",
            uuid::Uuid::new_v4()
        ),
        vec!["hermes".to_string(), "nous hermes".to_string()],
        "Official Hermes Agent docs",
        0.95,
        86400,
    )
    .await
    .unwrap();
    let matches = search_source_bookmarks("whats hermes", 5).await.unwrap();
    assert!(matches.iter().any(|m| m.id == item.id));
    assert_eq!(
        matches.iter().find(|m| m.id == item.id).unwrap().freshness,
        "fresh"
    );
    assert_eq!(delete_source(&item.id).await.unwrap(), 1);
}

#[test]
fn freshness_status_marks_old_sources_stale() {
    let old = (chrono::Utc::now() - chrono::Duration::seconds(120)).to_rfc3339();
    assert_eq!(freshness_status(Some(&old), 60), "stale");
    assert_eq!(freshness_status(None, 60), "unknown");
}

#[test]
fn display_source_label_repairs_legacy_github_labels() {
    assert_eq!(
        display_source_label(
            "github.com - pulls",
            "https://github.com/tonbistudio/turboquant-pytorch/pulls"
        ),
        "tonbistudio/turboquant-pytorch pull requests"
    );
    assert_eq!(
        display_source_label(
            "github.com - 6",
            "https://github.com/tonbistudio/turboquant-pytorch/issues/6"
        ),
        "tonbistudio/turboquant-pytorch issue #6"
    );
}

#[test]
fn research_brief_quality_rejects_github_ui_chrome() {
    let noisy = "assignee: Filter by this user Sort Sort by Newest Oldest Most commented Least commented Recently updated Least recently updated Best match Most reactions Pull requests list feat: HadamardRotation #12 Footer navigation Terms Privacy Security Status Community Docs Contact Manage cookies You can’t perform that action at this time";
    assert!(!is_useful_research_brief_summary(noisy));
    assert!(is_useful_research_brief_summary(
        "TurboQuant-PyTorch is a PyTorch implementation for compressing LLM KV caches with vector quantization and adaptive bit allocation."
    ));
}

#[tokio::test]
async fn source_search_requires_label_uri_or_alias_anchor() {
    let marker = uuid::Uuid::new_v4().to_string();
    let item = add_source_bookmark(
        &format!("tinygrad llama example {marker}"),
        "repo",
        &format!("https://github.com/tinygrad/examples/llama-{marker}.py"),
        vec![format!("tinygrad {marker}")],
        "TurboQuant PyTorch is mentioned only in this summary text.",
        0.95,
        604800,
    )
    .await
    .unwrap();

    let matches = search_source_bookmarks("turboquant pytorch", 5)
        .await
        .unwrap();
    assert!(matches.iter().all(|m| m.id != item.id));
    assert_eq!(delete_source(&item.id).await.unwrap(), 1);
}

#[tokio::test]
async fn research_brief_search_requires_topic_anchor() {
    let marker = uuid::Uuid::new_v4().to_string();
    let topic = format!("rightnow-ai/openfang-{marker}");
    let brief = save_research_brief(
        &topic,
        "Hermes Agent is discussed inside this comparison summary, but the canonical topic is OpenFang.",
        vec![],
        0.9,
        604800,
    )
    .await
    .unwrap();

    let matches = search_research_briefs("what is hermes agent", 5)
        .await
        .unwrap();
    assert!(matches.iter().all(|m| m.id != brief.id));
    let _ = delete_research_brief(&topic).await;
}

#[tokio::test]
async fn source_search_does_not_return_unrelated_trusted_sources() {
    let marker = uuid::Uuid::new_v4().to_string();
    let item = add_source_bookmark(
        &format!("Rust docs {}", marker),
        "docs",
        &format!("https://doc.rust-lang.org/{}", marker),
        vec![format!("rust {}", marker)],
        "Official Rust documentation",
        1.0,
        604800,
    )
    .await
    .unwrap();
    let matches = search_source_bookmarks("unrelated banana pasta", 5)
        .await
        .unwrap();
    assert!(matches.iter().all(|m| m.id != item.id));
    assert_eq!(delete_source(&item.id).await.unwrap(), 1);
}

#[tokio::test]
async fn source_ranking_prefers_exact_official_sources() {
    let marker = uuid::Uuid::new_v4().to_string();
    let low = add_source_bookmark(
        &format!("Hermes fan note {}", marker),
        "social",
        &format!("https://example.com/hermes-social-{}", marker),
        vec!["hermes".to_string()],
        "Unofficial community mention",
        0.2,
        21600,
    )
    .await
    .unwrap();
    let official = add_source_bookmark(
        &format!("Hermes Agent {}", marker),
        "docs",
        &format!(
            "https://hermes-agent.nousresearch.com/docs/official-{}",
            marker
        ),
        vec![format!("hermes official {}", marker)],
        "Official Hermes Agent docs",
        0.95,
        604800,
    )
    .await
    .unwrap();
    let matches = search_source_bookmarks(&format!("Hermes Agent {}", marker), 2)
        .await
        .unwrap();
    assert_eq!(
        matches.first().map(|m| m.id.as_str()),
        Some(official.id.as_str())
    );
    assert_eq!(delete_source(&low.id).await.unwrap(), 1);
    assert_eq!(delete_source(&official.id).await.unwrap(), 1);
}

#[tokio::test]
async fn knowledge_source_accepts_raw_url_string() {
    let marker = uuid::Uuid::new_v4().to_string();
    let url = format!("https://crates.io/crates/openz-{}", marker);
    let tool = KnowledgeSourceTool;
    let res = tool.call(&json!(url)).await.unwrap();
    assert_eq!(res.get("status").and_then(|v| v.as_str()), Some("success"));
    let source = res.get("source").unwrap();
    assert_eq!(source.get("uri").and_then(|v| v.as_str()), Some(url.as_str()));
    assert_eq!(delete_source(&url).await.unwrap(), 1);
}

#[tokio::test]
async fn knowledge_source_infers_add_with_url_and_title() {
    let marker = uuid::Uuid::new_v4().to_string();
    let url = format!("https://docs.rs/openz-{}", marker);
    let tool = KnowledgeSourceTool;
    let res = tool
        .call(&json!({
            "url": url,
            "title": format!("OpenZ Docs {}", marker),
            "tags": ["docs", "rust"],
            "description": "API documentation for OpenZ"
        }))
        .await
        .unwrap();
    assert_eq!(res.get("status").and_then(|v| v.as_str()), Some("success"));

    // Now test inferred search
    let search_res = tool
        .call(&json!({
            "q": format!("OpenZ Docs {}", marker)
        }))
        .await
        .unwrap();
    assert_eq!(search_res.get("status").and_then(|v| v.as_str()), Some("success"));
    let matches = search_res.get("matches").and_then(|v| v.as_array()).unwrap();
    assert!(matches.iter().any(|m| m.get("uri").and_then(|v| v.as_str()) == Some(url.as_str())));

    // Test inferred get by url
    let get_res = tool.call(&json!({ "url": url })).await.unwrap();
    assert_eq!(get_res.get("status").and_then(|v| v.as_str()), Some("success"));
    let retrieved = get_res.get("source").and_then(|v| v.as_object()).unwrap();
    assert_eq!(retrieved.get("uri").and_then(|v| v.as_str()), Some(url.as_str()));

    assert_eq!(delete_source(&url).await.unwrap(), 1);
}

