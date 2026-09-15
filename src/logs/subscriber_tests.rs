use super::*;

#[test]
fn secret_scrubber_masks_credentials_inside_log_text() {
    let secrets = vec![
        "bot123:token-value".to_string(),
        "provider-secret".to_string(),
    ];
    let redacted = redact_text_with_secrets(
        "curl https://api.telegram.org/bot123:token-value/sendMessage key=provider-secret",
        &secrets,
    );
    assert!(!redacted.contains("bot123:token-value"));
    assert!(!redacted.contains("provider-secret"));
    assert_eq!(redacted.matches("[REDACTED_SECRET]").count(), 2);
}
