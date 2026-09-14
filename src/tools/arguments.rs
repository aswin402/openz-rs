//! Canonical tool argument aliases and recursive normalization.
//!
//! Tool schemas historically accepted several casing and naming conventions.
//! Keep those compatibility aliases in one module so execution, presentation,
//! and security code can consume the same definitions.

pub const PATH_KEYS: &[&str] = &[
    "path",
    "file_path",
    "filePath",
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
    let Some(map) = args.as_object() else {
        return Vec::new();
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
                if alias != *key && !new_map.contains_key(&alias) {
                    new_map.insert(alias, normalized_value);
                }
            }
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
        if character.is_uppercase() {
            if !result.is_empty() && !result.ends_with('_') {
                result.push('_');
            }
            result.extend(character.to_lowercase());
        } else {
            result.push(character);
        }
    }
    result
}

#[cfg(test)]
#[path = "arguments_tests.rs"]
mod tests;
