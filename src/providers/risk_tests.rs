use super::*;

#[test]
fn model_risk_marks_unknown_free_models() {
    let risk = classify_model_risk("opencode_zen", "big-pickle");
    assert!(risk.risky);
    assert!(risk
        .reasons
        .iter()
        .any(|reason| reason.contains("not in OpenZ curated")));
}

#[test]
fn model_risk_allows_known_strong_default() {
    let risk = classify_model_risk("opencode_zen", "deepseek-v4-flash-free");
    assert!(!risk.risky);
    assert_eq!(risk.tier, "strong");
}

#[test]
fn model_risk_warns_for_small_models() {
    let risk = classify_model_risk("groq", "llama-3.1-8b-instant");
    assert!(risk.risky);
    assert!(risk
        .reasons
        .iter()
        .any(|reason| reason.contains("small/weak")));
}

#[test]
fn model_risk_warns_for_experimental_models() {
    let risk = classify_model_risk("openai", "gpt-4o-mini-experimental");
    assert!(risk.risky);
    assert!(risk
        .reasons
        .iter()
        .any(|reason| reason.contains("experimental or unknown behavior")));
}

#[test]
fn model_risk_classifies_known_standard_model() {
    // "mivi" provider has "mivi" model which is known, not strong, not weak, not free, not experimental
    let risk = classify_model_risk("mivi", "mivi");
    assert!(!risk.risky);
    assert_eq!(risk.tier, "standard");
    assert!(risk.reasons.is_empty());
}
