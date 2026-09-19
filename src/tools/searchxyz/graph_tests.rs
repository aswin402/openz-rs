use super::*;
use crate::tools::searchxyz::{
    coerce_bool_fields, coerce_numeric_fields, map_field_alias,
};

#[test]
fn github_file_limit_error_is_actionable() {
    let message = "Crawl failed for `https://github.com/aswin402/openz-rs`: GitHub ingest file count limit exceeded: files=22, max_files=5";
    let payload = github_file_limit_error_value(message).expect("limit payload");
    assert_eq!(payload["status"], "limit_exceeded");
    assert_eq!(payload["error_kind"], "repo_file_limit_exceeded");
    assert_eq!(payload["files"], 22);
    assert_eq!(payload["max_files"], 5);
    assert_eq!(payload["recommended_max_files"], 22);
    assert!(payload["next_step"]
        .as_str()
        .unwrap()
        .contains("max_files >= 22"));
}

#[test]
fn github_file_limit_auto_retry_defaults_on_for_small_repos() {
    assert!(github_file_limit_should_auto_retry(&json!({}), 22, 5));
    assert!(!github_file_limit_should_auto_retry(
        &json!({ "auto_expand_max_files": false }),
        22,
        5
    ));
    assert!(!github_file_limit_should_auto_retry(&json!({}), 20_000, 5));
}

#[test]
fn unrelated_github_errors_are_not_reclassified() {
    assert!(github_file_limit_error_value("network timeout").is_none());
}

#[test]
fn test_searchxyz_index_relationship_args_aliases() {
    let raw = json!({
        "from": "openz",
        "from_type": "framework",
        "to": "searchxyz",
        "to_type": "subsystem",
        "rel": "integrates"
    });
    let mut normalized = raw.clone();
    map_field_alias(&mut normalized, "source", &["from", "src", "source_entity", "sourceEntity"]);
    map_field_alias(&mut normalized, "source_type", &["sourceType", "from_type", "src_type"]);
    map_field_alias(&mut normalized, "target", &["to", "dst", "target_entity", "targetEntity"]);
    map_field_alias(&mut normalized, "target_type", &["targetType", "to_type", "dst_type"]);
    map_field_alias(&mut normalized, "relationship", &["rel", "relation", "type", "predicate", "edge"]);

    assert_eq!(normalized["source"], "openz");
    assert_eq!(normalized["source_type"], "framework");
    assert_eq!(normalized["target"], "searchxyz");
    assert_eq!(normalized["target_type"], "subsystem");
    assert_eq!(normalized["relationship"], "integrates");
}

#[test]
fn test_searchxyz_query_graph_args_normalization() {
    let raw = json!("openz");
    let mut normalized = if let Some(s) = raw.as_str() {
        json!({ "entity": s.trim() })
    } else {
        raw.clone()
    };
    map_field_alias(&mut normalized, "entity", &["name", "node", "query", "target"]);
    map_field_alias(&mut normalized, "max_depth", &["depth", "maxDepth"]);
    coerce_numeric_fields(&mut normalized, &["max_depth"]);

    assert_eq!(normalized["entity"], "openz");

    let raw_obj = json!({
        "node": "tantivy",
        "depth": "3"
    });
    let mut norm_obj = if let Some(s) = raw_obj.as_str() {
        json!({ "entity": s.trim() })
    } else {
        raw_obj.clone()
    };
    map_field_alias(&mut norm_obj, "entity", &["name", "node", "query", "target"]);
    map_field_alias(&mut norm_obj, "max_depth", &["depth", "maxDepth"]);
    coerce_numeric_fields(&mut norm_obj, &["max_depth"]);

    assert_eq!(norm_obj["entity"], "tantivy");
    assert_eq!(norm_obj["max_depth"], json!(3));
}

#[test]
fn test_searchxyz_read_github_repo_args_normalization() {
    let raw = json!("github.com/aswin402/openz-rs");
    let mut normalized = if let Some(s) = raw.as_str() {
        json!({ "repo_url": crate::tools::web::normalize_web_url(s) })
    } else {
        raw.clone()
    };
    map_field_alias(&mut normalized, "repo_url", &["repoUrl", "url", "repo", "repository", "target", "link"]);
    if let Some(u) = normalized.get("repo_url").and_then(|v| v.as_str()) {
        normalized["repo_url"] = json!(crate::tools::web::normalize_web_url(u));
    }
    map_field_alias(&mut normalized, "max_files", &["maxFiles", "files_limit", "limit"]);
    map_field_alias(&mut normalized, "auto_expand_max_files", &["autoExpandMaxFiles", "auto_expand"]);
    coerce_numeric_fields(&mut normalized, &["max_files"]);
    coerce_bool_fields(&mut normalized, &["auto_expand_max_files"]);

    assert_eq!(normalized["repo_url"], "https://github.com/aswin402/openz-rs");

    let raw_obj = json!({
        "repo": "https://github.com/aswin402/openz-rs",
        "maxFiles": "500",
        "autoExpandMaxFiles": "1"
    });
    let mut norm_obj = if let Some(s) = raw_obj.as_str() {
        json!({ "repo_url": crate::tools::web::normalize_web_url(s) })
    } else {
        raw_obj.clone()
    };
    map_field_alias(&mut norm_obj, "repo_url", &["repoUrl", "url", "repo", "repository", "target", "link"]);
    if let Some(u) = norm_obj.get("repo_url").and_then(|v| v.as_str()) {
        norm_obj["repo_url"] = json!(crate::tools::web::normalize_web_url(u));
    }
    map_field_alias(&mut norm_obj, "max_files", &["maxFiles", "files_limit", "limit"]);
    map_field_alias(&mut norm_obj, "auto_expand_max_files", &["autoExpandMaxFiles", "auto_expand"]);
    coerce_numeric_fields(&mut norm_obj, &["max_files"]);
    coerce_bool_fields(&mut norm_obj, &["auto_expand_max_files"]);

    assert_eq!(norm_obj["repo_url"], "https://github.com/aswin402/openz-rs");
    assert_eq!(norm_obj["max_files"], json!(500));
    assert_eq!(norm_obj["auto_expand_max_files"], json!(true));
}
