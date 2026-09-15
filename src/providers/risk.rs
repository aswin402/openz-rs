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
#[path = "risk_tests.rs"]
mod tests;

