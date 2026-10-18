use super::*;
use crate::tools::searchxyz::{
    coerce_bool_fields, coerce_numeric_fields, map_field_alias, normalize_query_arg,
};

#[test]
fn test_searchxyz_recall_args_normalization() {
    let raw = json!("tokio async runtime");
    let mut normalized = normalize_query_arg(&raw);
    map_field_alias(&mut normalized, "max_results", &["limit", "count", "maxResults"]);
    map_field_alias(&mut normalized, "semantic", &["vector", "embedding"]);
    coerce_numeric_fields(&mut normalized, &["max_results"]);
    coerce_bool_fields(&mut normalized, &["semantic"]);

    assert_eq!(normalized["query"], "tokio async runtime");

    let raw_obj = json!({
        "term": "actix-web",
        "limit": "15",
        "vector": "0"
    });
    let mut norm_obj = normalize_query_arg(&raw_obj);
    map_field_alias(&mut norm_obj, "max_results", &["limit", "count", "maxResults"]);
    map_field_alias(&mut norm_obj, "semantic", &["vector", "embedding"]);
    coerce_numeric_fields(&mut norm_obj, &["max_results"]);
    coerce_bool_fields(&mut norm_obj, &["semantic"]);

    assert_eq!(norm_obj["query"], "actix-web");
    assert_eq!(norm_obj["max_results"], json!(15));
    assert_eq!(norm_obj["semantic"], json!(false));
}

#[test]
fn test_searchxyz_list_sources_args_normalization() {
    let raw = json!("read_url");
    let mut normalized = crate::tools::searchxyz::wrap_string_arg(&raw, "source");
    map_field_alias(&mut normalized, "limit", &["max_results", "count"]);
    map_field_alias(&mut normalized, "offset", &["skip"]);
    map_field_alias(&mut normalized, "source", &["source_name", "type", "filter"]);
    coerce_numeric_fields(&mut normalized, &["limit", "offset"]);

    assert_eq!(normalized["source"], "read_url");

    let raw_obj = json!({
        "filter": "github",
        "count": "100",
        "skip": "20"
    });
    let mut norm_obj = crate::tools::searchxyz::wrap_string_arg(&raw_obj, "source");
    map_field_alias(&mut norm_obj, "limit", &["max_results", "count"]);
    map_field_alias(&mut norm_obj, "offset", &["skip"]);
    map_field_alias(&mut norm_obj, "source", &["source_name", "type", "filter"]);
    coerce_numeric_fields(&mut norm_obj, &["limit", "offset"]);

    assert_eq!(norm_obj["source"], "github");
    assert_eq!(norm_obj["limit"], json!(100));
    assert_eq!(norm_obj["offset"], json!(20));
}

#[test]
fn test_searchxyz_index_content_args_normalization() {
    let raw = json!({
        "link": "example.org/docs",
        "name": "Example Documentation",
        "text": "This is indexed content text."
    });
    let mut normalized = raw.clone();
    map_field_alias(&mut normalized, "url", &["uri", "link", "id", "source"]);
    normalize_index_content_url(&mut normalized);
    map_field_alias(&mut normalized, "title", &["name", "heading", "subject"]);
    map_field_alias(&mut normalized, "content", &["text", "body", "data"]);

    assert_eq!(normalized["url"], "https://example.org/docs");
    assert_eq!(normalized["title"], "Example Documentation");
    assert_eq!(normalized["content"], "This is indexed content text.");
}

#[test]
fn test_searchxyz_delete_source_args_normalization() {
    let raw = json!("example.com/page");
    let mut normalized = wrap_delete_source_arg(&raw);
    map_field_alias(&mut normalized, "url", &["uri", "link", "source", "target"]);
    map_field_alias(&mut normalized, "confirm", &["force", "yes", "confirmed"]);
    crate::tools::searchxyz::normalize_url_field(&mut normalized, "url");
    coerce_bool_fields(&mut normalized, &["confirm"]);

    assert_eq!(normalized["url"], "https://example.com/page");
    assert_eq!(normalized["confirm"], json!(true));

    let raw_obj = json!({
        "uri": "https://example.com/doc",
        "force": "yes"
    });
    let mut norm_obj = wrap_delete_source_arg(&raw_obj);
    map_field_alias(&mut norm_obj, "url", &["uri", "link", "source", "target"]);
    map_field_alias(&mut norm_obj, "confirm", &["force", "yes", "confirmed"]);
    crate::tools::searchxyz::normalize_url_field(&mut norm_obj, "url");
    coerce_bool_fields(&mut norm_obj, &["confirm"]);

    assert_eq!(norm_obj["url"], "https://example.com/doc");
    assert_eq!(norm_obj["confirm"], json!(true));
}

#[test]
fn test_searchxyz_clear_index_args_normalization() {
    let raw = json!("confirm");
    let mut normalized = wrap_clear_index_arg(&raw);
    map_field_alias(&mut normalized, "confirm", &["force", "yes", "confirmed"]);
    coerce_bool_fields(&mut normalized, &["confirm"]);
    assert_eq!(normalized["confirm"], json!(true));

    let raw_obj = json!({ "yes": "1" });
    let mut norm_obj = wrap_clear_index_arg(&raw_obj);
    map_field_alias(&mut norm_obj, "confirm", &["force", "yes", "confirmed"]);
    coerce_bool_fields(&mut norm_obj, &["confirm"]);
    assert_eq!(norm_obj["confirm"], json!(true));
}
