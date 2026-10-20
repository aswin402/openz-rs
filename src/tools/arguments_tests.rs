use super::*;

#[test]
fn tool_argument_aliases_normalize_consistently() {
    let normalized = normalize_tool_args(&serde_json::json!({
        "CommandLine": "just check openz",
        "Query": "provider catalog",
        "UrlContent": "https://example.com",
        "OutputPath": "/tmp/output.png",
        "TargetFile": "src/main.rs",
        "sessionId": "session-1",
    }));

    assert_eq!(normalized["command"], "just check openz");
    assert_eq!(normalized["query"], "provider catalog");
    assert_eq!(normalized["url"], "https://example.com");
    assert_eq!(normalized["output_path"], "/tmp/output.png");
    assert_eq!(normalized["target_file"], "src/main.rs");
    assert_eq!(normalized["session_id"], "session-1");
}

#[test]
fn typed_argument_kinds_share_compatibility_aliases() {
    let args = serde_json::json!({
        "AbsolutePath": "/tmp/example.txt",
        "CommandLine": "cargo check",
        "Query": "openz",
        "UrlContent": "https://example.com",
        "OutputPath": "/tmp/output.png",
        "sessionId": "session-1",
        "chat_id": "123",
    });

    assert_eq!(argument_values(&args, ToolArgumentKind::Path), vec![
        "/tmp/example.txt".to_string()
    ]);
    assert_eq!(argument_values(&args, ToolArgumentKind::Command), vec![
        "cargo check".to_string()
    ]);
    assert_eq!(argument_values(&args, ToolArgumentKind::Query), vec![
        "openz".to_string()
    ]);
    assert_eq!(argument_values(&args, ToolArgumentKind::Url), vec![
        "https://example.com".to_string()
    ]);
    assert_eq!(argument_values(&args, ToolArgumentKind::Output), vec![
        "/tmp/output.png".to_string()
    ]);
    assert_eq!(argument_values(&args, ToolArgumentKind::Session), vec![
        "session-1".to_string()
    ]);
    assert_eq!(argument_values(&args, ToolArgumentKind::Target), vec![
        "123".to_string()
    ]);
}

#[test]
fn cross_tool_coercion_and_alias_helpers_work_consistently() {
    let mut val = serde_json::json!({
        "width": "1920",
        "fps": "29.97",
        "enabled": "yes",
        "silent": "0",
        "q": "rust async",
        "old_count": "42",
        "embedded": "{\"nested\":true}",
    });

    coerce_numeric_fields(&mut val, &["width", "fps"]);
    coerce_bool_fields(&mut val, &["enabled", "silent"]);
    map_field_alias(&mut val, "query", &["q", "search"]);

    assert_eq!(val["width"], serde_json::json!(1920));
    assert_eq!(val["fps"], serde_json::json!(29.97));
    assert_eq!(val["enabled"], serde_json::json!(true));
    assert_eq!(val["silent"], serde_json::json!(false));
    assert_eq!(val["query"], serde_json::json!("rust async"));

    let parsed = parse_json_string_fields(&val, &["embedded"]);
    assert_eq!(parsed["embedded"], serde_json::json!({ "nested": true }));

    let mut map = parsed.as_object().cloned().expect("object");
    rename_map_field(&mut map, "old_count", "count", true);
    fill_from_aliases(&mut map, "search_query", &["query"]);
    parse_json_string_field(&mut map, "embedded");

    assert_eq!(map.get("count"), Some(&serde_json::json!(42)));
    assert_eq!(map.get("search_query"), Some(&serde_json::json!("rust async")));
    assert_eq!(coerce_u32(&serde_json::json!("1080")), Some(1080));
    assert_eq!(coerce_usize(&serde_json::json!("64")), Some(64));
    assert_eq!(coerce_f64(&serde_json::json!("3.5")), Some(3.5));
    assert_eq!(coerce_f32(&serde_json::json!("1.25")), Some(1.25));
    assert_eq!(
        wrap_string_arg(&serde_json::json!("  hello  "), "text"),
        serde_json::json!({ "text": "hello" })
    );
    assert_eq!(
        coerce_object_numeric_fields(&serde_json::json!({ "limit": "25" }), &["limit"]),
        serde_json::json!({ "limit": 25 })
    );
}

#[test]
fn multi_key_extractors_and_url_normalization_work_consistently() {
    let args = serde_json::json!({
        "targetUrl": "example.com/docs",
        "maxLength": "4096",
        "timeoutSecs": 120,
        "respectRobots": "false",
        "importance": "0.85",
        "label": "  memory-tag  ",
    });

    assert_eq!(
        extract_normalized_url_from_aliases(
            &args,
            &["url", "targetUrl"],
            "missing url",
            "empty url"
        )
        .expect("valid url"),
        "https://example.com/docs"
    );
    assert_eq!(extract_usize_arg(&args, &["max_length", "maxLength"]), Some(4096));
    assert_eq!(get_u64_arg(&args, &["timeout_secs", "timeoutSecs"], 30), 120);
    assert!(!get_bool_arg(&args, &["respect_robots", "respectRobots"], true));
    assert_eq!(extract_f32_arg(&args, &["weight", "importance"]), Some(0.85));
    assert_eq!(
        extract_string_arg(&args, &["tag", "label"]),
        Some("memory-tag".to_string())
    );
    assert_eq!(
        extract_trimmed_str(&args, &["tag", "label"]),
        Some("memory-tag".to_string())
    );
}

#[test]
fn map_field_and_dotted_path_helpers_work_consistently() {
    let val = serde_json::json!({
        "thoughtNumber": "3",
        "confidence": "0.95",
        "nextThoughtNeeded": "y",
        "tags": ["alpha", 42],
        "steps": ["1", 2],
        "user": {
            "profile": {
                "name": "Aswin"
            }
        }
    });
    let map = val.as_object().expect("object");

    assert_eq!(get_usize_field(map, &["thought_number", "thoughtNumber"]), Some(3));
    assert_eq!(get_f64_field(map, &["score", "confidence"]), Some(0.95));
    assert_eq!(get_bool_field(map, &["next_thought_needed", "nextThoughtNeeded"]), Some(true));
    assert_eq!(
        get_string_vec_field(map, &["tags"]),
        Some(vec!["alpha".to_string(), "42".to_string()])
    );
    assert_eq!(get_usize_vec_field(map, &["steps"]), Some(vec![1, 2]));
    assert_eq!(
        resolve_dotted_json_path(&val, "user.profile.name"),
        Some(&serde_json::json!("Aswin"))
    );
    assert_eq!(resolve_dotted_json_path(&val, "this"), Some(&val));
}

#[test]
fn string_field_and_list_extractors_coerce_csv_arrays_and_scalars() {
    let val = serde_json::json!({
        "entityName": "  OpenZ  ",
        "observations": "fast, async , , rust",
        "items": [" alpha ", "", "beta"]
    });
    assert_eq!(
        extract_string_field(&val, &["name", "entityName"]),
        Some("OpenZ".to_string())
    );
    assert_eq!(
        extract_string_list_field(&val, &["observations"]),
        vec!["fast".to_string(), "async".to_string(), "rust".to_string()]
    );
    assert_eq!(
        extract_trimmed_string_array(val.get("items")),
        vec!["alpha".to_string(), "beta".to_string()]
    );
}
