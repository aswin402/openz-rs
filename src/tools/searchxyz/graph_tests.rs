use super::*;

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
