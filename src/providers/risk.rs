use crate::channels::model_catalog::provider_models_by_name;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelRisk {
    pub risky: bool,
    pub tier: &'static str,
    pub reasons: Vec<&'static str>,
}

fn model_name_suggests_strong(model_lc: &str) -> bool {
    model_lc.contains("70b")
        || model_lc.contains("deepseek-v4")
        || model_lc.contains("claude")
        || model_lc.contains("gpt-4")
        || (model_lc.contains("nemotron") && model_lc.contains("ultra"))
}

pub fn classify_model_risk(provider: &str, model: &str) -> ModelRisk {
    let model_lc = model.trim().to_lowercase();
    let known = provider_models_by_name(provider)
        .map(|p| {
            p.models
                .iter()
                .any(|m| m.eq_ignore_ascii_case(model.trim()))
        })
        .unwrap_or(false);

    let mut reasons = Vec::new();
    if !known {
        reasons.push("not in OpenZ curated model catalog");
    }
    if model_lc.contains("free") && !model_name_suggests_strong(&model_lc) {
        reasons.push("free-tier model may be rate-limited or unstable");
    }
    if [
        "1b", "2b", "3b", "4b", "6b", "7b", "8b", "9b", "small", "mini", "lite",
    ]
    .iter()
    .any(|needle| model_lc.contains(needle))
    {
        reasons.push("small/weak model may ignore context or tool instructions");
    }
    if ["preview", "experimental", "beta", "pickle", "mimo", "hy3"]
        .iter()
        .any(|needle| model_lc.contains(needle))
    {
        reasons.push("model name suggests experimental or unknown behavior");
    }

    let risky = !reasons.is_empty();
    let tier = if risky {
        "risky"
    } else if model_name_suggests_strong(&model_lc) {
        "strong"
    } else {
        "standard"
    };

    ModelRisk {
        risky,
        tier,
        reasons,
    }
}

#[cfg(test)]
mod tests {
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
}
