//! Unified secret detection, collection, and redaction utilities.

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
}
