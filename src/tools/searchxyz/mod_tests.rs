use super::*;
use crate::tools::Tool;

#[test]
fn test_openz_embedded_paths_use_openz_dir_for_defaults() {
    std::env::remove_var("SEARCHXYZ_INDEX_PATH");
    std::env::remove_var("SEARCHXYZ_CACHE_PATH");
    let mut config = Config::default();
    apply_openz_embedded_paths(&mut config);
    assert!(config.index.path.ends_with(".openz/searchxyz/index"));
    assert!(config.cache.path.ends_with(".openz/searchxyz/cache.json"));
}

#[test]
fn test_searchxyz_tools_metadata() {
    assert_eq!(SearchXyzDoctorTool.name(), "searchxyz_doctor");
    assert_eq!(SearchXyzSearchWebTool.name(), "searchxyz_search_web");
    assert_eq!(
        SearchXyzBrowserSearchTool.name(),
        "searchxyz_browser_search"
    );
    assert_eq!(SearchXyzReadUrlTool.name(), "searchxyz_read_url");
    assert_eq!(
        SearchXyzSearchAndReadTool.name(),
        "searchxyz_search_and_read"
    );
    assert_eq!(SearchXyzRecallTool.name(), "searchxyz_recall");
    assert_eq!(SearchXyzListSourcesTool.name(), "searchxyz_list_sources");
    assert_eq!(SearchXyzDeepResearchTool.name(), "searchxyz_deep_research");
    assert_eq!(SearchXyzIndexContentTool.name(), "searchxyz_index_content");
    assert_eq!(SearchXyzSiteMapTool.name(), "searchxyz_site_map");
    assert_eq!(
        SearchXyzIndexRelationshipTool.name(),
        "searchxyz_index_relationship"
    );
    assert_eq!(SearchXyzQueryGraphTool.name(), "searchxyz_query_graph");
    assert_eq!(
        SearchXyzReadGithubRepoTool.name(),
        "searchxyz_read_github_repo"
    );
    assert_eq!(
        SearchXyzExportResearchTool.name(),
        "searchxyz_export_research"
    );
    assert_eq!(
        SearchXyzImportResearchTool.name(),
        "searchxyz_import_research"
    );
    assert_eq!(SearchXyzDeleteSourceTool.name(), "searchxyz_delete_source");
    assert_eq!(SearchXyzClearIndexTool.name(), "searchxyz_clear_index");
}

#[test]
fn test_searchxyz_coerce_number_value() {
    assert_eq!(
        coerce_number_value(&serde_json::json!("42")),
        Some(serde_json::json!(42))
    );
    assert_eq!(
        coerce_number_value(&serde_json::json!("-10")),
        Some(serde_json::json!(-10))
    );
    assert_eq!(
        coerce_number_value(&serde_json::json!(100)),
        Some(serde_json::json!(100))
    );
    assert_eq!(coerce_number_value(&serde_json::json!("invalid")), None);
}

#[test]
fn test_searchxyz_coerce_numeric_fields() {
    let mut val = serde_json::json!({
        "query": "rust",
        "max_results": "10",
        "limit": "50",
        "preserve": "text"
    });
    coerce_numeric_fields(&mut val, &["max_results", "limit"]);
    assert_eq!(val["max_results"], serde_json::json!(10));
    assert_eq!(val["limit"], serde_json::json!(50));
    assert_eq!(val["preserve"], serde_json::json!("text"));
}

#[test]
fn test_searchxyz_coerce_bool_fields() {
    let mut val = serde_json::json!({
        "confirm": "true",
        "dry_run": "FALSE",
        "flag_1": "1",
        "flag_0": "0",
        "flag_yes": "yes",
        "flag_no": "NO",
        "num_1": 1,
        "num_0": 0,
        "other": "not_bool"
    });
    coerce_bool_fields(
        &mut val,
        &[
            "confirm", "dry_run", "flag_1", "flag_0", "flag_yes", "flag_no", "num_1", "num_0",
            "other",
        ],
    );
    assert_eq!(val["confirm"], serde_json::json!(true));
    assert_eq!(val["dry_run"], serde_json::json!(false));
    assert_eq!(val["flag_1"], serde_json::json!(true));
    assert_eq!(val["flag_0"], serde_json::json!(false));
    assert_eq!(val["flag_yes"], serde_json::json!(true));
    assert_eq!(val["flag_no"], serde_json::json!(false));
    assert_eq!(val["num_1"], serde_json::json!(true));
    assert_eq!(val["num_0"], serde_json::json!(false));
    assert_eq!(val["other"], serde_json::json!("not_bool"));
}

#[test]
fn test_searchxyz_map_field_alias() {
    let mut val = serde_json::json!({
        "limit": 25,
        "q": "rust async",
        "link": "https://example.com"
    });
    map_field_alias(&mut val, "max_results", &["limit", "count"]);
    map_field_alias(&mut val, "query", &["q", "search"]);
    map_field_alias(&mut val, "url", &["link", "uri"]);
    assert_eq!(val["max_results"], serde_json::json!(25));
    assert_eq!(val["query"], serde_json::json!("rust async"));
    assert_eq!(val["url"], serde_json::json!("https://example.com"));
}

#[test]
fn test_searchxyz_normalize_query_arg() {
    // Direct string
    let from_str = normalize_query_arg(&serde_json::json!("  openz search  "));
    assert_eq!(from_str["query"], "openz search");

    // Object with q alias
    let from_alias = normalize_query_arg(&serde_json::json!({ "q": "tokio" }));
    assert_eq!(from_alias["query"], "tokio");

    // Existing query preserved
    let existing = normalize_query_arg(&serde_json::json!({ "query": "actix", "q": "other" }));
    assert_eq!(existing["query"], "actix");
}

#[test]
fn test_searchxyz_normalize_url_arg() {
    // Direct string scheme-less
    let from_str = normalize_url_arg(&serde_json::json!("docs.rs/tokio"));
    assert_eq!(from_str["url"], "https://docs.rs/tokio");

    // Wrapped in brackets and quotes
    let from_wrapped = normalize_url_arg(&serde_json::json!("<https://example.com>"));
    assert_eq!(from_wrapped["url"], "https://example.com");

    // Object with uri alias and scheme-less
    let from_alias = normalize_url_arg(&serde_json::json!({ "uri": "github.com/aswin402/openz-rs" }));
    assert_eq!(from_alias["url"], "https://github.com/aswin402/openz-rs");

    // Existing url normalized
    let existing = normalize_url_arg(&serde_json::json!({ "url": "example.org" }));
    assert_eq!(existing["url"], "https://example.org");
}

