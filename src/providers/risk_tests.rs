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

#[test]
fn compact_tool_budget_classifier_distinguishes_compact_and_frontier_models() {
    assert!(is_compact_tool_budget_model("llama-3.1-8b-instant"));
    assert!(is_compact_tool_budget_model("gemini-2.5-flash-lite"));
    assert!(is_compact_tool_budget_model("qwen-2.5-7b-free"));
    assert!(!is_compact_tool_budget_model("anthropic/claude-3-5-sonnet"));
    assert!(!is_compact_tool_budget_model("gpt-4o"));
    assert!(!is_compact_tool_budget_model("deepseek-v4-flash-free"));
    assert!(!is_compact_tool_budget_model("MiniMax-M3.1-Flash"));
}

#[test]
fn minimax_does_not_false_positive_on_mini_substring() {
    let risk = classify_model_risk("minimax", "MiniMax-M3.1-Flash");
    assert!(
        !risk
            .reasons
            .iter()
            .any(|r| r.contains("small/weak")),
        "MiniMax must never match 'mini' small-model token: {:?}",
        risk.reasons
    );
}
