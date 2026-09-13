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

#[test]
fn test_collect_all_environment_and_config_secrets_dedup_interleaved_same_length() {
    let config = serde_json::json!({
        "keys": [
            { "api_key": "secret_bbbbbb" },
            { "api_key": "secret_aaaaaa" },
            { "api_key": "secret_bbbbbb" }
        ]
    });
    let secrets = collect_all_environment_and_config_secrets(&config);
    let b_count = secrets.iter().filter(|s| *s == "secret_bbbbbb").count();
    assert_eq!(
        b_count, 1,
        "identical secrets with same length must be deduplicated even when interleaved"
    );
    assert!(secrets.contains(&"secret_aaaaaa".to_string()));
}

#[test]
fn test_redact_text_with_secrets_defensive_empty_strings() {
    let secrets = vec!["".to_string(), "target_secret_123".to_string()];
    let text = "Text containing target_secret_123 and normal words.";
    let redacted = redact_text_with_secrets(text, &secrets);
    assert_eq!(
        redacted,
        "Text containing [REDACTED_SECRET] and normal words."
    );
}
