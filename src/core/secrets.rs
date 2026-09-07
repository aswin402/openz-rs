//! Unified secret detection, collection, and redaction utilities.

use std::sync::LazyLock;

use serde_json::Value;

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
    matches!(
        normalized_secret_key(key).as_str(),
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
    )
}

/// Recursively inspect a JSON value to determine if it contains any secret keys or fields.
pub fn contains_secret_material(value: &Value) -> bool {
    match value {
        Value::Object(map) => map.iter().any(|(key, val)| {
            is_secret_key(key) || contains_secret_material(val)
        }),
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
                if is_secret_key(key) && !val.is_null() {
                    redacted.insert(key.clone(), Value::String("********".to_string()));
                } else {
                    redacted.insert(key.clone(), redact_secrets_in_json(val));
                }
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
                if is_secret_key(key) {
                    if let Some(secret) = child.as_str() {
                        let secret = secret.trim();
                        if secret.len() >= 8 {
                            secrets.push(secret.to_string());
                        }
                    }
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
    let trimmed = secret.trim();
    if trimmed.len() <= 8 {
        "********".to_string()
    } else {
        let prefix_len = 3.min(trimmed.len() / 4);
        let suffix_len = 4.min(trimmed.len() / 4);
        let prefix = &trimmed[..prefix_len];
        let suffix = &trimmed[trimmed.len() - suffix_len..];
        format!("{}...{}", prefix, suffix)
    }
}

static SECRET_PATTERNS: LazyLock<Vec<regex::Regex>> = LazyLock::new(|| {
    vec![
        regex::Regex::new(r"\d{8,}:[A-Za-z0-9_-]{16,}\b").expect("valid telegram token regex"),
        regex::Regex::new(r"\bsk-[A-Za-z0-9_-]{16,}\b").expect("valid sk token regex"),
        regex::Regex::new(r"\d{8,}:[A-Za-z0-9_-]{3,}\.\.\.")
            .expect("valid partial telegram token regex"),
        regex::Regex::new(r"\bsk-[A-Za-z0-9_-]{4,}\.\.\.").expect("valid partial sk token regex"),
    ]
});

/// Return the compiled regular expressions matching well-known API keys and credentials.
pub fn secret_patterns() -> &'static [regex::Regex] {
    &SECRET_PATTERNS
}

/// Scrub text by replacing matches of known secret token regex patterns with `[REDACTED_SECRET]`.
/// Returns the sanitized string and the total number of replacements performed.
pub fn scrub_secret_text(text: &str) -> (String, usize) {
    let mut scrubbed = text.to_string();
    let mut replacements = 0usize;
    for pattern in secret_patterns() {
        let count = pattern.find_iter(&scrubbed).count();
        if count > 0 {
            scrubbed = pattern
                .replace_all(&scrubbed, "[REDACTED_SECRET]")
                .into_owned();
            replacements = replacements.saturating_add(count);
        }
    }
    (scrubbed, replacements)
}

/// Redact occurrences of specific known secret values from a text string.
pub fn redact_text_with_secrets(text: &str, secrets: &[String]) -> String {
    secrets.iter().fold(text.to_string(), |result, secret| {
        result.replace(secret, "[REDACTED_SECRET]")
    })
}

/// Collect credentials from both config and environment variables.
pub fn collect_all_environment_and_config_secrets(config: &serde_json::Value) -> Vec<String> {
    let mut secrets = Vec::new();
    collect_secret_values(config, &mut secrets);
    for (key, value) in std::env::vars() {
        if is_secret_key(&key) && value.trim().len() >= 8 {
            secrets.push(value.trim().to_string());
        }
    }
    secrets.sort_by_key(|v| std::cmp::Reverse(v.len()));
    secrets.dedup();
    secrets
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_secret_key_matching() {
        assert!(is_secret_key("api_key"));
        assert!(is_secret_key("API_KEY"));
        assert!(is_secret_key("apiKey"));
        assert!(is_secret_key("bot_token"));
        assert!(is_secret_key("clientSecret"));
        assert!(is_secret_key("webhook_secret"));
        assert!(is_secret_key("password"));
        assert!(is_secret_key("private_key"));
        assert!(!is_secret_key("username"));
        assert!(!is_secret_key("model"));
        assert!(!is_secret_key("port"));
    }

    #[test]
    fn test_redact_secrets_in_json() {
        let input = serde_json::json!({
            "model": "claude-3-5-sonnet",
            "api_key": "sk-secret123456",
            "nested": {
                "bot_token": "tok-987654321",
                "normal": "value"
            }
        });

        let redacted = redact_secrets_in_json(&input);
        assert_eq!(redacted["model"], "claude-3-5-sonnet");
        assert_eq!(redacted["api_key"], "********");
        assert_eq!(redacted["nested"]["bot_token"], "********");
        assert_eq!(redacted["nested"]["normal"], "value");
    }

    #[test]
    fn test_collect_secret_values() {
        let input = serde_json::json!({
            "api_key": "sk-secret123456",
            "short_secret": "abc",
            "normal": "normal-value-here"
        });

        let mut secrets = Vec::new();
        collect_secret_values(&input, &mut secrets);
        assert_eq!(secrets, vec!["sk-secret123456"]);
    }

    #[test]
    fn test_mask_secret_str() {
        assert_eq!(mask_secret_str("short"), "********");
        assert_eq!(mask_secret_str("sk-1234567890abcdef"), "sk-...cdef");
    }

    #[test]
    fn test_scrub_secret_text_patterns() {
        let input = "sk-12345678901234567890 and bot 12345678:abcdefghijklmnopqrst";
        let (scrubbed, count) = scrub_secret_text(input);
        assert_eq!(count, 2);
        assert!(!scrubbed.contains("sk-12345678901234567890"));
        assert!(!scrubbed.contains("12345678:abcdefghijklmnopqrst"));
        assert!(scrubbed.contains("[REDACTED_SECRET]"));
    }

    #[test]
    fn test_redact_text_with_secrets_replaces_known_keys() {
        let secrets = vec!["supersecretpass123".to_string()];
        let redacted = redact_text_with_secrets("My password is supersecretpass123!", &secrets);
        assert_eq!(redacted, "My password is [REDACTED_SECRET]!");
    }

    #[test]
    fn test_collect_all_environment_and_config_secrets() {
        let config = serde_json::json!({
            "api_key": "config-secret-key-12345"
        });
        let secrets = collect_all_environment_and_config_secrets(&config);
        assert!(secrets.contains(&"config-secret-key-12345".to_string()));
    }

    #[test]
    fn test_secret_patterns_not_empty() {
        assert!(!secret_patterns().is_empty());
    }
}
