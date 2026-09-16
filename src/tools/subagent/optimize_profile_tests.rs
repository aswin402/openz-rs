use super::*;

#[test]
fn subagent_settings_validation_rejects_more_than_three_fallbacks() {
    let args = serde_json::json!({
        "fallbacks": ["a", "b", "c", "d"]
    });
    let result = parse_subagent_settings(&args);
    assert!(result.is_err());
}
