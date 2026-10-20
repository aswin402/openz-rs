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

pub fn coerce_u32(v: &serde_json::Value) -> Option<u32> {
    match v {
        serde_json::Value::Number(n) => n.as_u64().map(|u| u as u32),
        serde_json::Value::String(s) => s.trim().parse::<u32>().ok(),
        _ => None,
    }
}

pub fn coerce_usize(v: &serde_json::Value) -> Option<usize> {
    match v {
        serde_json::Value::Number(n) => n.as_u64().map(|u| u as usize),
        serde_json::Value::String(s) => s.trim().parse::<usize>().ok(),
        _ => None,
    }
}

pub fn coerce_f64(v: &serde_json::Value) -> Option<f64> {
    match v {
        serde_json::Value::Number(n) => n.as_f64(),
        serde_json::Value::String(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    }
}

pub fn coerce_f32(v: &serde_json::Value) -> Option<f32> {
    match v {
        serde_json::Value::Number(n) => n.as_f64().map(|f| f as f32),
        serde_json::Value::String(s) => s.trim().parse::<f32>().ok(),
        _ => None,
    }
}

pub fn coerce_number_value(val: &serde_json::Value) -> Option<serde_json::Value> {
    match val {
        serde_json::Value::Number(_) => Some(val.clone()),
        serde_json::Value::String(s) => {
            let trimmed = s.trim();
            trimmed
                .parse::<i64>()
                .ok()
                .map(|i| serde_json::Value::Number(serde_json::Number::from(i)))
                .or_else(|| {
                    trimmed
                        .parse::<u64>()
                        .ok()
                        .map(|u| serde_json::Value::Number(serde_json::Number::from(u)))
                })
                .or_else(|| {
                    trimmed
                        .parse::<f64>()
                        .ok()
                        .and_then(serde_json::Number::from_f64)
                        .map(serde_json::Value::Number)
                })
        }
        _ => None,
    }
}

pub fn coerce_numeric_fields_in_map(
    map: &mut serde_json::Map<String, serde_json::Value>,
    field_names: &[&str],
) {
    for &name in field_names {
        let coerced = map.get(name).and_then(coerce_number_value);
        match coerced {
            Some(c) => {
                map.insert(name.to_string(), c);
            }
            None => {}
        }
    }
}

pub fn coerce_numeric_fields(val: &mut serde_json::Value, field_names: &[&str]) {
    match val.as_object_mut() {
        Some(map) => coerce_numeric_fields_in_map(map, field_names),
        None => {}
    }
}

pub fn coerce_object_numeric_fields(
    arguments: &serde_json::Value,
    fields: &[&str],
) -> serde_json::Value {
    match arguments.as_object().cloned() {
        Some(mut obj) => {
            coerce_numeric_fields_in_map(&mut obj, fields);
            serde_json::Value::Object(obj)
        }
        None => arguments.clone(),
    }
}

pub fn coerce_single_bool(entry: &serde_json::Value) -> Option<bool> {
    match entry {
        serde_json::Value::String(s) => match s.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => Some(true),
            "false" | "0" | "no" | "off" => Some(false),
            _ => None,
        },
        serde_json::Value::Number(n) => match n.as_i64() {
            Some(1) => Some(true),
            Some(0) => Some(false),
            _ => None,
        },
        _ => None,
    }
}

pub fn coerce_bool_fields(val: &mut serde_json::Value, field_names: &[&str]) {
    let map = match val.as_object_mut() {
        Some(m) => m,
        None => return,
    };
    for &name in field_names {
        let coerced = map.get(name).and_then(coerce_single_bool);
        match coerced {
            Some(b) => {
                map.insert(name.to_string(), serde_json::Value::Bool(b));
            }
            None => {}
        }
    }
}

pub fn get_alias(
    map: &serde_json::Map<String, serde_json::Value>,
    aliases: &[&str],
) -> Option<serde_json::Value> {
    aliases.iter().find_map(|alias| map.get(*alias).cloned())
}

pub fn fill_from_aliases(
    map: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
    aliases: &[&str],
) {
    match map.contains_key(key) {
        true => {}
        false => match get_alias(map, aliases) {
            Some(val) => {
                map.insert(key.to_string(), val);
            }
            None => {}
        },
    }
}

pub fn map_field_alias(val: &mut serde_json::Value, canonical: &str, aliases: &[&str]) {
    let map = match val.as_object_mut() {
        Some(m) => m,
        None => return,
    };
    let has_canonical = map.get(canonical).is_some_and(|v| !v.is_null());
    match has_canonical {
        true => {}
        false => {
            let found = aliases
                .iter()
                .find_map(|&alias| map.get(alias).filter(|v| !v.is_null()).cloned());
            match found {
                Some(cloned) => {
                    map.insert(canonical.to_string(), cloned);
                }
                None => {}
            }
        }
    }
}

pub fn rename_map_field(
    map: &mut serde_json::Map<String, serde_json::Value>,
    from_key: &str,
    to_key: &str,
    coerce_num: bool,
) {
    match map.remove(from_key) {
        Some(val) => {
            let final_val = match coerce_num {
                true => coerce_number_value(&val).unwrap_or(val),
                false => val,
            };
            map.insert(to_key.to_string(), final_val);
        }
        None => {}
    }
}

pub fn parse_embedded_json_string(value: &serde_json::Value) -> Option<serde_json::Value> {
    let raw = value.as_str()?.trim();
    match raw.starts_with('{') || raw.starts_with('[') {
        true => serde_json::from_str(raw).ok(),
        false => None,
    }
}

pub fn parse_json_string_field(map: &mut serde_json::Map<String, serde_json::Value>, key: &str) {
    let parsed = match map.get(key) {
        Some(serde_json::Value::String(s)) => serde_json::from_str::<serde_json::Value>(s).ok(),
        _ => None,
    };
    match parsed {
        Some(p) => {
            map.insert(key.to_string(), p);
        }
        None => {}
    }
}

pub fn parse_json_string_fields(
    arguments: &serde_json::Value,
    fields: &[&str],
) -> serde_json::Value {
    let obj = match arguments.as_object() {
        Some(obj) => obj,
        None => return arguments.clone(),
    };

    let mut normalized = obj.clone();
    let mut changed = false;
    for field in fields {
        match normalized.get(*field).and_then(parse_embedded_json_string) {
            Some(parsed) => {
                normalized.insert((*field).to_string(), parsed);
                changed = true;
            }
            None => {}
        }
    }

    match changed {
        true => serde_json::Value::Object(normalized),
        false => arguments.clone(),
    }
}

pub fn wrap_string_arg(val: &serde_json::Value, key: &str) -> serde_json::Value {
    match val.as_str() {
        Some(s) => serde_json::json!({ key: s.trim() }),
        None => val.clone(),
    }
}

#[cfg(test)]
#[path = "arguments_tests.rs"]
mod tests;
