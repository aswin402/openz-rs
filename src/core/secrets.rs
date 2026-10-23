//! Unified secret detection, collection, and redaction utilities.

use serde_json::Value;

const SECRET_PATTERN_DESCRIPTORS: &[&str] = &[
    "telegram_bot_token",
    "sk_api_token",
    "partial_telegram_bot_token",
    "partial_sk_api_token",
];

/// Normalize a key string for secret identification by keeping only ASCII
/// alphanumeric characters in lowercase.
pub fn normalized_secret_key(key: &str) -> String {
    key.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Determine whether a property or environment key name represents a sensitive credential.
pub fn is_secret_key(key: &str) -> bool {
    let norm = normalized_secret_key(key);
    matches!(
        norm.as_str(),
        "apikey"
            | "apitoken"
            | "accesstoken"
            | "authtoken"
            | "bottoken"
            | "clientsecret"
            | "password"
            | "secret"
            | "verifytoken"
            | "webhooksecret"
            | "privatekey"
            | "token"
            | "pat"
    ) || norm.ends_with("token")
        || norm.ends_with("secret")
        || norm.ends_with("apikey")
        || norm.ends_with("pat")
}

/// Recursively inspect a JSON value to determine if it contains any secret keys or fields.
pub fn contains_secret_material(value: &Value) -> bool {
    match value {
        Value::Object(map) => map
            .iter()
            .any(|(key, val)| is_secret_key(key) || contains_secret_material(val)),
        Value::Array(values) => values.iter().any(contains_secret_material),
        _ => false,
    }
}

/// Recursively redact sensitive values from a JSON payload, replacing them with `"********"`.
pub fn redact_secrets_in_json(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut redacted = serde_json::Map::new();
            for (key, val) in map {
                let next_val = match is_secret_key(key) && !val.is_null() {
                    true => Value::String("********".to_string()),
                    false => redact_secrets_in_json(val),
                };
                redacted.insert(key.clone(), next_val);
            }
            Value::Object(redacted)
        }
        Value::Array(values) => Value::Array(values.iter().map(redact_secrets_in_json).collect()),
        other => other.clone(),
    }
}

/// Recursively collect secret string values (length >= 8) for log masking.
pub fn collect_secret_values(value: &Value, secrets: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let candidate = is_secret_key(key)
                    .then(|| child.as_str())
                    .flatten()
                    .map(str::trim)
                    .filter(|secret| secret.len() >= 8);
                match candidate {
                    Some(secret) => secrets.push(secret.to_string()),
                    None => {}
                }
                collect_secret_values(child, secrets);
            }
        }
        Value::Array(arr) => {
            for child in arr {
                collect_secret_values(child, secrets);
            }
        }
        _ => {}
    }
}

/// Mask a secret string for display, preserving prefix/suffix if long enough.
pub fn mask_secret_str(secret: &str) -> String {
    let chars: Vec<char> = secret.trim().chars().collect();
    match chars.len() <= 8 {
        true => "********".to_string(),
        false => {
            let prefix_len = 3.min(chars.len() / 4);
            let suffix_len = 4.min(chars.len() / 4);
            let prefix: String = chars[..prefix_len].iter().collect();
            let suffix: String = chars[chars.len() - suffix_len..].iter().collect();
            format!("{prefix}...{suffix}")
        }
    }
}

/// Return the active secret pattern descriptors used to scrub well-known API keys and credentials.
pub fn secret_patterns() -> &'static [&'static str] {
    SECRET_PATTERN_DESCRIPTORS
}

fn is_token_body_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-')
}

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn at_left_word_boundary(bytes: &[u8], idx: usize) -> bool {
    idx.checked_sub(1)
        .and_then(|prev| bytes.get(prev).copied())
        .map(|b| !is_word_byte(b))
        .unwrap_or(true)
}

fn at_right_word_boundary(bytes: &[u8], end_idx: usize) -> bool {
    bytes
        .get(end_idx)
        .copied()
        .map(|b| !is_word_byte(b))
        .unwrap_or(true)
}

fn match_sk_token_at(bytes: &[u8], idx: usize) -> Option<usize> {
    let valid_start = at_left_word_boundary(bytes, idx) && bytes.get(idx..idx + 3) == Some(b"sk-");
    match valid_start {
        false => None,
        true => {
            let body_len = bytes[idx + 3..]
                .iter()
                .take_while(|&&b| is_token_body_byte(b))
                .count();
            let end_body = idx + 3 + body_len;
            let has_ellipsis = bytes.get(end_body..end_body + 3) == Some(b"...");
            match (
                body_len >= 4 && has_ellipsis,
                body_len >= 16 && at_right_word_boundary(bytes, end_body),
            ) {
                (true, _) => Some(3 + body_len + 3),
                (false, true) => Some(3 + body_len),
                (false, false) => None,
            }
        }
    }
}

fn match_telegram_token_at(bytes: &[u8], idx: usize) -> Option<usize> {
    let digit_len = bytes[idx..]
        .iter()
        .take_while(|&&b| b.is_ascii_digit())
        .count();
    let has_colon = digit_len >= 8 && bytes.get(idx + digit_len) == Some(&b':');
    match has_colon {
        false => None,
        true => {
            let start_body = idx + digit_len + 1;
            let body_len = bytes[start_body..]
                .iter()
                .take_while(|&&b| is_token_body_byte(b))
                .count();
            let end_body = start_body + body_len;
            let has_ellipsis = bytes.get(end_body..end_body + 3) == Some(b"...");
            match (
                body_len >= 3 && has_ellipsis,
                body_len >= 16 && at_right_word_boundary(bytes, end_body),
            ) {
                (true, _) => Some(digit_len + 1 + body_len + 3),
                (false, true) => Some(digit_len + 1 + body_len),
                (false, false) => None,
            }
        }
    }
}

/// Scrub text by replacing matches of known secret token patterns with `[REDACTED_SECRET]`.
/// Returns the sanitized string and the total number of replacements performed.
pub fn scrub_secret_text(text: &str) -> (String, usize) {
    let bytes = text.as_bytes();
    let mut scrubbed = String::with_capacity(text.len());
    let mut replacements = 0usize;
    let mut cursor = 0usize;

    while cursor < bytes.len() {
        let matched_len =
            match_sk_token_at(bytes, cursor).or_else(|| match_telegram_token_at(bytes, cursor));
        match matched_len {
            Some(len) => {
                scrubbed.push_str("[REDACTED_SECRET]");
                replacements = replacements.saturating_add(1);
                cursor += len;
            }
            None => match text[cursor..].chars().next() {
                Some(ch) => {
                    scrubbed.push(ch);
                    cursor += ch.len_utf8();
                }
                None => break,
            },
        }
    }

    (scrubbed, replacements)
}

/// Redact occurrences of specific known secret values from a text string.
pub fn redact_text_with_secrets(text: &str, secrets: &[String]) -> String {
    secrets
        .iter()
        .filter(|s| !s.is_empty())
        .fold(text.to_string(), |result, secret| {
            result.replace(secret, "[REDACTED_SECRET]")
        })
}

/// Collect credentials from both config and environment variables.
pub fn collect_all_environment_and_config_secrets(config: &serde_json::Value) -> Vec<String> {
    let mut secrets = Vec::new();
    collect_secret_values(config, &mut secrets);
    for (key, value) in std::env::vars() {
        let trimmed = value.trim();
        match is_secret_key(&key) && trimmed.len() >= 8 {
            true => secrets.push(trimmed.to_string()),
            false => {}
        }
    }
    secrets.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
    secrets.dedup();
    secrets
}

/// Read and trim a non-empty secret value from a file path resolved via `config::loader::resolve_path`.
pub fn read_secret_file(path: &str) -> Option<String> {
    let path = crate::config::loader::resolve_path(path);
    std::fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
#[path = "secrets_tests.rs"]
mod tests;

