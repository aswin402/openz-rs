//! Canonical tool argument aliases and recursive normalization.
//!
//! Tool schemas historically accepted several casing and naming conventions.
//! Keep those compatibility aliases in one module so execution, presentation,
//! and security code can consume the same definitions.

pub const PATH_KEYS: &[&str] = &[
    "path",
    "file_path",
    "filePath",
    "target_path",
    "target_file",
    "TargetFile",
    "filepath",
    "file",
    "Path",
    "AbsolutePath",
    "DirectoryPath",
];
pub const COMMAND_KEYS: &[&str] = &["command", "Command", "CommandLine", "command_line"];
pub const OUTPUT_KEYS: &[&str] = &["output_path", "outputPath", "OutputPath"];
pub const QUERY_KEYS: &[&str] = &["query", "Query"];
pub const URL_KEYS: &[&str] = &["url", "Url", "UrlContent"];
pub const SESSION_KEYS: &[&str] = &[
    "session_id",
    "sessionId",
    "session",
    "session_key",
    "sessionKey",
];
pub const TARGET_KEYS: &[&str] = &[
    "target",
    "target_id",
    "targetId",
    "chat_id",
    "chatId",
    "channel_id",
    "channelId",
    "recipient",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolArgumentKind {
    Path,
    Command,
    Query,
    Url,
    Output,
    Session,
    Target,
    Generic,
}

pub const fn argument_keys(kind: ToolArgumentKind) -> &'static [&'static str] {
    match kind {
        ToolArgumentKind::Path => PATH_KEYS,
        ToolArgumentKind::Command => COMMAND_KEYS,
        ToolArgumentKind::Query => QUERY_KEYS,
        ToolArgumentKind::Url => URL_KEYS,
        ToolArgumentKind::Output => OUTPUT_KEYS,
        ToolArgumentKind::Session => SESSION_KEYS,
        ToolArgumentKind::Target => TARGET_KEYS,
        ToolArgumentKind::Generic => &[],
    }
}

pub fn argument_values(
    args: &serde_json::Value,
    kind: ToolArgumentKind,
) -> Vec<String> {
    let map = match args.as_object() {
        Some(m) => m,
        None => return Vec::new(),
    };
    argument_keys(kind)
        .iter()
        .filter_map(|key| map.get(*key).and_then(|value| value.as_str()))
        .map(str::to_owned)
        .collect()
}

pub fn normalize_tool_args(args: &serde_json::Value) -> serde_json::Value {
    match args {
        serde_json::Value::Object(map) => {
            let mut new_map = serde_json::Map::new();
            for (key, value) in map {
                let alias = match key.as_str() {
                    "Path" => "path".to_string(),
                    "CommandLine" | "Command" | "command_line" | "cmd" => "command".to_string(),
                    "Query" => "query".to_string(),
                    "Url" | "UrlContent" => "url".to_string(),
                    "Action" => "action".to_string(),
                    "text" | "content_str" => "content".to_string(),
                    "diff" => "patch".to_string(),
                    "ImageName" | "OutputPath" => "output_path".to_string(),
                    other => to_snake_case(other),
                };
                let normalized_value = normalize_tool_args(value);
                new_map.insert(key.clone(), normalized_value.clone());
                let should_insert_alias = alias != *key && !new_map.contains_key(&alias);
                let _ = should_insert_alias.then(|| new_map.insert(alias, normalized_value));
            }
            let fallback_path = (!new_map.contains_key("path"))
                .then(|| {
                    [
                        "file_path",
                        "target_file",
                        "filepath",
                        "absolute_path",
                        "directory_path",
                        "file",
                    ]
                    .into_iter()
                    .find_map(|candidate| new_map.get(candidate).cloned())
                })
                .flatten();
            let _ = fallback_path.map(|val| new_map.insert("path".to_string(), val));
            serde_json::Value::Object(new_map)
        }
        serde_json::Value::Array(values) => {
            serde_json::Value::Array(values.iter().map(normalize_tool_args).collect())
        }
        other => other.clone(),
    }
}

pub fn to_snake_case(value: &str) -> String {
    let mut result = String::new();
    for character in value.chars() {
        match character.is_uppercase() {
            true => {
                let needs_sep = !result.is_empty() && !result.ends_with('_');
                let _ = needs_sep.then(|| result.push('_'));
                result.extend(character.to_lowercase());
            }
            false => result.push(character),
        }
    }
    result
}

#[cfg(test)]
#[path = "arguments_tests.rs"]
mod tests;
