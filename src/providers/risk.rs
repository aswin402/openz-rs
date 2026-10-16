use crate::channels::model_catalog::provider_models_by_name;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelRisk {
    pub risky: bool,
    pub tier: &'static str,
    pub reasons: Vec<&'static str>,
}

const STRONG_MODEL_KEYWORDS: &[&str] = &["70b", "deepseek-v4", "claude", "gpt-4"];
const SMALL_MODEL_KEYWORDS: &[&str] = &[
    "1b", "2b", "3b", "4b", "6b", "7b", "8b", "9b", "small", "mini", "lite",
];
const EXPERIMENTAL_MODEL_KEYWORDS: &[&str] = &[
    "preview",
    "experimental",
    "beta",
    "pickle",
    "mimo",
    "hy3",
];

fn model_name_suggests_strong(model_lc: &str) -> bool {
    STRONG_MODEL_KEYWORDS
        .iter()
        .any(|needle| model_lc.contains(needle))
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

    let strong = model_name_suggests_strong(&model_lc);
    let checks: [(bool, &'static str); 4] = [
        (!known, "not in OpenZ curated model catalog"),
        (
            model_lc.contains("free") && !strong,
            "free-tier model may be rate-limited or unstable",
        ),
        (
            SMALL_MODEL_KEYWORDS
                .iter()
                .any(|needle| model_lc.contains(needle)),
            "small/weak model may ignore context or tool instructions",
        ),
        (
            EXPERIMENTAL_MODEL_KEYWORDS
                .iter()
                .any(|needle| model_lc.contains(needle)),
            "model name suggests experimental or unknown behavior",
        ),
    ];

    let reasons: Vec<&'static str> = checks
        .into_iter()
        .filter_map(|(triggered, reason)| triggered.then_some(reason))
        .collect();

    let risky = !reasons.is_empty();
    let tier = match (risky, strong) {
        (true, _) => "risky",
        (false, true) => "strong",
        (false, false) => "standard",
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
