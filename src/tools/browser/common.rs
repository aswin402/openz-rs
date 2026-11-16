//! Shared argument alias slices and normalization helpers for OpenZ's native browser subsystem.

use serde_json::Value;

pub(crate) const URL_ALIASES: &[&str] = &[
    "url",
    "target_url",
    "targetUrl",
    "uri",
    "link",
    "target",
    "site",
    "website",
    "domain",
    "host",
    "page",
    "endpoint",
    "href",
    "address",
];

pub(crate) const ACTION_ALIASES: &[&str] = &["action", "act", "command", "cmd", "method"];

pub(crate) const SELECTOR_ALIASES: &[&str] = &[
    "selector",
    "css_selector",
    "cssSelector",
    "css",
    "element",
    "target",
];

pub(crate) const REF_ID_ALIASES: &[&str] = &[
    "ref_id",
    "refId",
    "ref",
    "element_index",
    "elementIndex",
    "target_ref",
    "targetRef",
];

pub(crate) const TEXT_ALIASES: &[&str] = &["text", "value", "content", "input", "string"];

pub(crate) const PATH_ALIASES: &[&str] = &[
    "path",
    "output",
    "output_path",
    "outputPath",
    "file_path",
    "file",
    "dest",
];

pub(crate) const SCRIPT_ALIASES: &[&str] = &["script", "expression", "code", "js", "expr"];

pub(crate) use crate::tools::arguments::extract_trimmed_non_empty_str;

pub fn resolve_browser_object_action_and_url(
    arguments: &Value,
    default_no_url_action: &str,
) -> (String, Option<String>) {
    let url_opt = extract_trimmed_non_empty_str(arguments, URL_ALIASES).map(ToString::to_string);
    let action = extract_trimmed_non_empty_str(arguments, ACTION_ALIASES)
        .map(ToString::to_string)
        .unwrap_or_else(|| match url_opt.is_some() {
            true => "navigate".to_string(),
            false => default_no_url_action.to_string(),
        });
    (action, url_opt)
}

#[cfg(test)]
#[path = "common_tests.rs"]
mod tests;
