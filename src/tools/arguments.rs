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
            fill_from_aliases(
                &mut new_map,
                "path",
                &[
                    "file_path",
                    "target_path",
                    "target_file",
                    "filepath",
                    "absolute_path",
                    "directory_path",
                    "file",
                ],
            );
            fill_from_aliases(
                &mut new_map,
                "file_path",
                &[
                    "path",
                    "target_path",
                    "target_file",
                    "filepath",
                    "absolute_path",
                    "file",
                ],
            );
            fill_from_aliases(&mut new_map, "query", &["search_query", "q"]);
            fill_from_aliases(&mut new_map, "url", &["uri", "target_url", "page_url"]);
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
            "true" | "1" | "yes" | "y" | "on" => Some(true),
            "false" | "0" | "no" | "n" | "off" => Some(false),
            _ => None,
        },
        serde_json::Value::Number(n) => match n.as_i64() {
            Some(0) => Some(false),
            Some(_) => Some(true),
            None => None,
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

pub fn coerce_u64(v: &serde_json::Value) -> Option<u64> {
    v.as_u64()
        .or_else(|| v.as_i64().filter(|&n| n >= 0).map(|n| n as u64))
        .or_else(|| v.as_f64().filter(|&f| f >= 0.0).map(|f| f as u64))
        .or_else(|| v.as_str().and_then(|s| s.trim().parse::<u64>().ok()))
}

pub fn parse_bool_value(v: &serde_json::Value) -> Option<bool> {
    v.as_bool().or_else(|| coerce_single_bool(v))
}

pub fn parse_u64_value(v: &serde_json::Value) -> Option<u64> {
    coerce_u64(v)
}

pub fn parse_f64_value(v: &serde_json::Value) -> Option<f64> {
    coerce_f64(v)
}

pub fn extract_first_str<'a>(arguments: &'a serde_json::Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|&key| arguments.get(key).and_then(|v| v.as_str()))
}

pub fn extract_trimmed_non_empty_str<'a>(
    arguments: &'a serde_json::Value,
    keys: &[&str],
) -> Option<&'a str> {
    extract_first_str(arguments, keys)
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

pub fn extract_trimmed_str(value: &serde_json::Value, keys: &[&str]) -> Option<String> {
    extract_trimmed_non_empty_str(value, keys).map(ToString::to_string)
}

pub fn to_non_empty_trimmed(opt: Option<&str>) -> Option<String> {
    opt.map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
}

pub fn extract_string_arg(arguments: &serde_json::Value, keys: &[&str]) -> Option<String> {
    arguments
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .or_else(|| {
            keys.iter().find_map(|key| {
                let val = arguments.get(*key)?;
                let raw = val
                    .as_str()
                    .map(str::to_string)
                    .or_else(|| (val.is_number() || val.is_boolean()).then(|| val.to_string()))?;
                let trimmed = raw.trim();
                (!trimmed.is_empty()).then(|| trimmed.to_string())
            })
        })
}

pub fn extract_u64_arg(arguments: &serde_json::Value, keys: &[&str]) -> Option<u64> {
    keys.iter()
        .find_map(|key| arguments.get(*key).and_then(coerce_u64))
}

pub fn get_u64_arg(arguments: &serde_json::Value, keys: &[&str], default: u64) -> u64 {
    extract_u64_arg(arguments, keys).unwrap_or(default)
}

pub fn extract_usize_arg(arguments: &serde_json::Value, keys: &[&str]) -> Option<usize> {
    extract_u64_arg(arguments, keys).map(|u| u as usize)
}

pub fn extract_f32_arg(arguments: &serde_json::Value, keys: &[&str]) -> Option<f32> {
    keys.iter().find_map(|key| {
        let v = arguments.get(*key)?;
        v.as_f64()
            .map(|f| f as f32)
            .or_else(|| v.as_i64().map(|i| i as f32))
            .or_else(|| v.as_str().and_then(|s| s.trim().parse::<f32>().ok()))
    })
}

pub fn extract_bool_arg(arguments: &serde_json::Value, keys: &[&str]) -> Option<bool> {
    keys.iter()
        .find_map(|key| arguments.get(*key).and_then(parse_bool_value))
}

pub fn get_bool_arg(arguments: &serde_json::Value, keys: &[&str], default: bool) -> bool {
    extract_bool_arg(arguments, keys).unwrap_or(default)
}

pub fn extract_normalized_url_from_aliases(
    arguments: &serde_json::Value,
    aliases: &[&str],
    missing_err: &str,
    empty_err: &str,
) -> anyhow::Result<String> {
    let raw_url = match arguments.as_str() {
        Some(s) => s.trim(),
        None => extract_first_str(arguments, aliases)
            .ok_or_else(|| anyhow::anyhow!("{missing_err}"))?
            .trim(),
    };
    (!raw_url.is_empty())
        .then(|| crate::tools::web::normalize_web_url(raw_url))
        .ok_or_else(|| anyhow::anyhow!("{empty_err}"))
}

pub fn value_to_non_empty_string(v: &serde_json::Value) -> Option<String> {
    match v {
        serde_json::Value::String(s) => {
            let trimmed = s.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        }
        serde_json::Value::Number(_) | serde_json::Value::Bool(_) => Some(v.to_string()),
        _ => None,
    }
}

pub fn extract_string_field(val: &serde_json::Value, keys: &[&str]) -> Option<String> {
    value_to_non_empty_string(val).or_else(|| {
        keys.iter()
            .find_map(|key| val.get(*key).and_then(value_to_non_empty_string))
    })
}

pub fn value_to_string_list(v: &serde_json::Value) -> Option<Vec<String>> {
    match v {
        serde_json::Value::Array(arr) => {
            let list: Vec<String> = arr.iter().filter_map(value_to_non_empty_string).collect();
            (!list.is_empty()).then_some(list)
        }
        serde_json::Value::String(s) => {
            let trimmed = s.trim();
            match (trimmed.is_empty(), trimmed.contains(',')) {
                (true, _) => None,
                (false, true) => Some(
                    trimmed
                        .split(',')
                        .map(|part| part.trim().to_string())
                        .filter(|p| !p.is_empty())
                        .collect(),
                ),
                (false, false) => Some(vec![trimmed.to_string()]),
            }
        }
        _ => None,
    }
}

pub fn extract_string_list_field(val: &serde_json::Value, keys: &[&str]) -> Vec<String> {
    keys.iter()
        .find_map(|key| val.get(*key).and_then(value_to_string_list))
        .unwrap_or_default()
}

pub fn extract_trimmed_string_array(value: Option<&serde_json::Value>) -> Vec<String> {
    value
        .and_then(serde_json::Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

pub fn value_to_usize(v: &serde_json::Value) -> Option<usize> {
    coerce_u64(v).map(|u| u as usize)
}

pub fn get_string_field(
    map: &serde_json::Map<String, serde_json::Value>,
    aliases: &[&str],
) -> Option<String> {
    aliases
        .iter()
        .find_map(|&k| map.get(k).and_then(value_to_non_empty_string))
}

pub fn get_usize_field(
    map: &serde_json::Map<String, serde_json::Value>,
    aliases: &[&str],
) -> Option<usize> {
    aliases
        .iter()
        .find_map(|&k| map.get(k).and_then(value_to_usize))
}

pub fn get_f64_field(
    map: &serde_json::Map<String, serde_json::Value>,
    aliases: &[&str],
) -> Option<f64> {
    aliases
        .iter()
        .find_map(|&k| map.get(k).and_then(coerce_f64))
}

pub fn get_bool_field(
    map: &serde_json::Map<String, serde_json::Value>,
    aliases: &[&str],
) -> Option<bool> {
    aliases
        .iter()
        .find_map(|&k| map.get(k).and_then(parse_bool_value))
}

pub fn get_string_vec_field(
    map: &serde_json::Map<String, serde_json::Value>,
    aliases: &[&str],
) -> Option<Vec<String>> {
    aliases.iter().find_map(|&k| {
        map.get(k).and_then(|v| match v {
            serde_json::Value::Array(arr) => {
                let items: Vec<String> =
                    arr.iter().filter_map(value_to_non_empty_string).collect();
                (!items.is_empty()).then_some(items)
            }
            _ => value_to_non_empty_string(v).map(|s| vec![s]),
        })
    })
}

pub fn get_usize_vec_field(
    map: &serde_json::Map<String, serde_json::Value>,
    aliases: &[&str],
) -> Option<Vec<usize>> {
    aliases.iter().find_map(|&k| {
        map.get(k).and_then(|v| match v {
            serde_json::Value::Array(arr) => {
                let items: Vec<usize> = arr.iter().filter_map(value_to_usize).collect();
                (!items.is_empty()).then_some(items)
            }
            _ => value_to_usize(v).map(|n| vec![n]),
        })
    })
}

pub fn resolve_dotted_json_path<'a>(
    root: &'a serde_json::Value,
    path: &str,
) -> Option<&'a serde_json::Value> {
    match path.is_empty() || path == "this" || path == "." {
        true => {
            let from_map = match root {
                serde_json::Value::Object(map) => map.get(path),
                _ => None,
            };
            return Some(from_map.unwrap_or(root));
        }
        false => {}
    }
    let mut current = root;
    for segment in path.split('.') {
        match current {
            serde_json::Value::Object(map) => {
                current = map.get(segment)?;
            }
            _ => return None,
        }
    }
    Some(current)
}

#[cfg(test)]
#[path = "arguments_tests.rs"]
mod tests;
