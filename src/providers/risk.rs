use crate::channels::model_catalog::provider_models_by_name;
use crate::providers::context_registry::{parse_parameter_billions, DynamicContextRegistry};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelRisk {
    pub risky: bool,
    pub tier: &'static str,
    pub reasons: Vec<&'static str>,
}

const STRONG_FAMILY_TOKENS: &[&str] = &["claude", "minimax"];
const SMALL_TIER_TOKENS: &[&str] = &["small", "mini", "lite", "nano", "micro"];
const EXPERIMENTAL_TOKENS: &[&str] = &["preview", "experimental", "beta", "pickle", "mimo", "hy3"];

fn split_slug_tokens(model_lc: &str) -> Vec<&str> {
    model_lc
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|s| !s.is_empty())
        .collect()
}

pub fn model_name_suggests_strong(model: &str) -> bool {
    let model_lc = model.trim().to_ascii_lowercase();
    let tokens = split_slug_tokens(&model_lc);
    let param_strong = parse_parameter_billions(&model_lc).is_some_and(|b| b >= 70);
    let family_strong = STRONG_FAMILY_TOKENS
        .iter()
        .any(|needle| tokens.contains(needle));
    let gpt_strong = tokens
        .windows(2)
        .any(|w| w[0] == "gpt" && (w[1].starts_with('4') || w[1].parse::<u32>().is_ok_and(|v| v >= 4)));
    let deepseek_v4_plus = tokens.windows(2).any(|w| {
        w[0] == "deepseek"
            && w[1]
                .strip_prefix('v')
                .and_then(|d| d.parse::<u32>().ok())
                .is_some_and(|v| v >= 4)
    });
    let nemotron_ultra = tokens.contains(&"nemotron") && tokens.contains(&"ultra");

    param_strong || family_strong || gpt_strong || deepseek_v4_plus || nemotron_ultra
}

pub fn is_compact_tool_budget_model(model: &str) -> bool {
    (!model_name_suggests_strong(model)) && {
        let lowered = model.to_ascii_lowercase();
        let tokens = split_slug_tokens(&lowered);
        let small_params = parse_parameter_billions(&lowered).is_some_and(|b| b <= 9);
        small_params
            || SMALL_TIER_TOKENS
                .iter()
                .chain(EXPERIMENTAL_TOKENS.iter())
                .chain(std::iter::once(&"free"))
                .any(|needle| tokens.contains(needle))
    }
}

pub fn classify_model_risk(provider: &str, model: &str) -> ModelRisk {
    let model_lc = model.trim().to_ascii_lowercase();
    let tokens = split_slug_tokens(&model_lc);
    let in_curated = provider_models_by_name(provider)
        .map(|p| {
            p.models
                .iter()
                .any(|m| m.eq_ignore_ascii_case(model.trim()))
        })
        .unwrap_or(false);
    let known = in_curated || DynamicContextRegistry::is_known_in_live_catalog(model);

    let strong = model_name_suggests_strong(&model_lc);
    let small_params = parse_parameter_billions(&model_lc).is_some_and(|b| b <= 9);
    let has_small_token = SMALL_TIER_TOKENS.iter().any(|t| tokens.contains(t));
    let has_experimental_token = EXPERIMENTAL_TOKENS.iter().any(|t| tokens.contains(t));
    let has_free_token = tokens.contains(&"free");

    let checks: [(bool, &'static str); 4] = [
        (!known, "not in OpenZ curated model catalog"),
        (
            has_free_token && !strong,
            "free-tier model may be rate-limited or unstable",
        ),
        (
            small_params || has_small_token,
            "small/weak model may ignore context or tool instructions",
        ),
        (
            has_experimental_token,
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
