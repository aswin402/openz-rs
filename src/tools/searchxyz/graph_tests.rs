use super::*;
use crate::tools::searchxyz::{coerce_bool_fields, coerce_numeric_fields, map_field_alias};

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
fn test_searchxyz_read_github_repo_metadata() {
    let tool = SearchXyzReadGithubRepoTool;
    let meta = tool.metadata();
    assert_eq!(tool.name(), "searchxyz_read_github_repo");
    assert_eq!(meta.domain, "web");
    assert!(meta.uses_network);
    assert!(meta.writes_disk);
    assert!(meta.spawns_process);
}

#[test]
fn test_searchxyz_read_github_repo_args_normalization() {
    let raw = json!("github.com/aswin402/openz-rs");
    let mut normalized = crate::tools::searchxyz::wrap_url_string_arg(&raw, "repo_url");
    map_field_alias(
        &mut normalized,
        "repo_url",
        &["repoUrl", "url", "repo", "repository", "target", "link"],
    );
    crate::tools::searchxyz::normalize_url_field(&mut normalized, "repo_url");
    map_field_alias(
        &mut normalized,
        "max_files",
        &["maxFiles", "files_limit", "limit"],
    );
    map_field_alias(
        &mut normalized,
        "auto_expand_max_files",
        &["autoExpandMaxFiles", "auto_expand"],
    );
    coerce_numeric_fields(&mut normalized, &["max_files"]);
    coerce_bool_fields(&mut normalized, &["auto_expand_max_files"]);

    assert_eq!(
        normalized["repo_url"],
        "https://github.com/aswin402/openz-rs"
    );

    let raw_obj = json!({
        "repo": "https://github.com/aswin402/openz-rs",
        "maxFiles": "500",
        "autoExpandMaxFiles": "1"
    });
    let mut norm_obj = crate::tools::searchxyz::wrap_url_string_arg(&raw_obj, "repo_url");
    map_field_alias(
        &mut norm_obj,
        "repo_url",
        &["repoUrl", "url", "repo", "repository", "target", "link"],
    );
    crate::tools::searchxyz::normalize_url_field(&mut norm_obj, "repo_url");
    map_field_alias(
        &mut norm_obj,
        "max_files",
        &["maxFiles", "files_limit", "limit"],
    );
    map_field_alias(
        &mut norm_obj,
        "auto_expand_max_files",
        &["autoExpandMaxFiles", "auto_expand"],
    );
    coerce_numeric_fields(&mut norm_obj, &["max_files"]);
    coerce_bool_fields(&mut norm_obj, &["auto_expand_max_files"]);

    assert_eq!(norm_obj["repo_url"], "https://github.com/aswin402/openz-rs");
    assert_eq!(norm_obj["max_files"], json!(500));
    assert_eq!(norm_obj["auto_expand_max_files"], json!(true));
}
