use crate::level::CavemanLevel;
use serde::{Deserialize, Serialize};

/// Category explaining why Caveman compression must yield to crystal-clear prose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutoClarityReason {
    SecurityWarning,
    IrreversibleOperation,
    OrderSensitiveRunbook,
    UserConfusion,
}

impl AutoClarityReason {
    pub fn label(self) -> &'static str {
        match self {
            Self::SecurityWarning => "security-critical risk or vulnerability",
            Self::IrreversibleOperation => "irreversible or destructive operation",
            Self::OrderSensitiveRunbook => "order-sensitive multi-step procedure",
            Self::UserConfusion => "explicit user clarification request",
        }
    }
}

/// Result of deterministic Auto-Clarity turn analysis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoClaritySignal {
    pub suspend_compression: bool,
    pub reasons: Vec<AutoClarityReason>,
}

/// Strict command-position natural-language mode switch intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CavemanCommandIntent {
    DisableCaveman,
    EnableLevel(CavemanLevel),
}

struct ReasonPatternRule {
    reason: AutoClarityReason,
    phrases: &'static [&'static str],
    single_tokens: &'static [&'static str],
    min_score: usize,
}

const AUTO_CLARITY_RULES: &[ReasonPatternRule] = &[
    ReasonPatternRule {
        reason: AutoClarityReason::SecurityWarning,
        phrases: &[
            "security vulnerability",
            "privilege escalation",
            "sql injection",
            "command injection",
            "remote code execution",
            "private key",
            "expose credentials",
            "auth bypass",
            "cve-",
        ],
        single_tokens: &["rce", "xss", "csrf", "ssrf", "exploit", "vulnerability"],
        min_score: 2,
    },
    ReasonPatternRule {
        reason: AutoClarityReason::IrreversibleOperation,
        phrases: &[
            "drop table",
            "drop database",
            "rm -rf",
            "force push",
            "git reset --hard",
            "delete production",
            "wipe disk",
            "permanent deletion",
            "irreversible",
        ],
        single_tokens: &["destructive", "production_db", "format_disk"],
        min_score: 2,
    },
    ReasonPatternRule {
        reason: AutoClarityReason::OrderSensitiveRunbook,
        phrases: &[
            "step-by-step migration",
            "exact sequence",
            "disaster recovery",
            "rollback procedure",
            "incident response",
            "zero-downtime migration",
        ],
        single_tokens: &["runbook", "cutover"],
        min_score: 2,
    },
    ReasonPatternRule {
        reason: AutoClarityReason::UserConfusion,
        phrases: &[
            "i do not understand",
            "i don't understand",
            "explain in detail",
            "explain clearly",
            "too cryptic",
            "too terse",
            "in plain english",
            "elaborate please",
            "what do you mean by",
        ],
        single_tokens: &["unclear", "confusing"],
        min_score: 2,
    },
];

const DISABLE_COMMAND_PHRASES: &[&str] = &[
    "stop caveman",
    "disable caveman",
    "caveman off",
    "normal mode",
    "exit caveman",
    "turn off caveman",
];

fn normalize_words(input: &str) -> Vec<String> {
    input
        .split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
        .filter(|part| !part.is_empty())
        .map(|part| part.to_ascii_lowercase())
        .collect()
}

/// Deterministic zero-regex Auto-Clarity classifier.
pub fn classify_auto_clarity(user_prompt: &str) -> AutoClaritySignal {
    let lowered = user_prompt.to_ascii_lowercase();
    let tokens = normalize_words(user_prompt);
    let mut reasons = Vec::new();

    for rule in AUTO_CLARITY_RULES {
        let phrase_score: usize = rule
            .phrases
            .iter()
            .filter(|phrase| lowered.contains(**phrase))
            .count()
            * 2;
        let token_score: usize = rule
            .single_tokens
            .iter()
            .filter(|needle| tokens.iter().any(|tok| tok == **needle))
            .count()
            * 2;
        let triggered = (phrase_score + token_score) >= rule.min_score;
        match triggered {
            true => reasons.push(rule.reason),
            false => {}
        }
    }

    AutoClaritySignal {
        suspend_compression: !reasons.is_empty(),
        reasons,
    }
}

/// Strict command-position intent detector.
///
/// Only triggers when the entire prompt is a short standalone directive (`<= 4` words)
/// so conversational questions like `"how do I exit vim normal mode?"` never misfire.
pub fn detect_command_intent(user_prompt: &str) -> Option<CavemanCommandIntent> {
    let trimmed = user_prompt
        .trim()
        .trim_matches(|c: char| matches!(c, '.' | '!' | ';'));
    let words: Vec<String> = trimmed
        .split_whitespace()
        .map(|w| w.to_ascii_lowercase())
        .collect();

    let within_command_bound = !words.is_empty() && words.len() <= 4 && !trimmed.contains('?');
    match within_command_bound {
        false => None,
        true => {
            let joined = words.join(" ");
            let is_disable = DISABLE_COMMAND_PHRASES.contains(&joined.as_str());
            match is_disable {
                true => Some(CavemanCommandIntent::DisableCaveman),
                false => match words.as_slice() {
                    [first] => match first.as_str() {
                        "ultracave" => Some(CavemanCommandIntent::EnableLevel(CavemanLevel::Ultra)),
                        "megacave" => {
                            Some(CavemanCommandIntent::EnableLevel(CavemanLevel::WenyanFull))
                        }
                        _ => None,
                    },
                    [first, second] => match first.as_str() {
                        "caveman" => match second.as_str() {
                            "off" | "stop" | "disable" => Some(CavemanCommandIntent::DisableCaveman),
                            "on" | "enable" => {
                                Some(CavemanCommandIntent::EnableLevel(CavemanLevel::Full))
                            }
                            other => CavemanLevel::parse_flexible(other)
                                .map(CavemanCommandIntent::EnableLevel),
                        },
                        _ => None,
                    },
                    _ => None,
                },
            }
        }
    }
}

/// Compiles the KV-cache-stable static system prompt block for `StaticPromptPrefix`.
pub fn compile_static_directive(enabled: bool, level: CavemanLevel) -> String {
    match enabled {
        false => String::new(),
        true => {
            let desc = level.descriptor();
            let mut out = format!(
                "\n\n[OpenCaveman Terseness Protocol — Mode: caveman ({})]\n",
                desc.id
            );
            for rule in desc.directive_rules {
                out.push_str("- ");
                out.push_str(rule);
                out.push('\n');
            }
            out.push_str(&format!(
                "- Avoid: \"{}\"\n- Prefer: \"{}\"\n",
                desc.example_bad, desc.example_good
            ));
            out.push_str(
                "- Auto-Clarity Boundary: Temporarily revert to full, unambiguous sentences for security warnings, irreversible/destructive confirmations, order-critical multi-step sequences, or when the user asks for detailed clarification; resume caveman terseness immediately afterward.",
            );
            out
        }
    }
}

/// Compiles an optional turn-scoped Auto-Clarity suspension notice for `DynamicPromptSuffix`.
/// Keeps `StaticPromptPrefix` byte-identical across turns while honoring safety overrides.
pub fn compile_dynamic_auto_clarity(enabled: bool, user_prompt: &str) -> String {
    match enabled {
        false => String::new(),
        true => {
            let signal = classify_auto_clarity(user_prompt);
            match signal.suspend_compression {
                false => String::new(),
                true => {
                    let labels: Vec<&str> = signal.reasons.iter().map(|r| r.label()).collect();
                    format!(
                        "\n\n[OpenCaveman Auto-Clarity Override Active]\nDetected {}. Write critical warnings, preconditions, and step ordering in complete, unambiguous prose for this turn. Keep code, commands, and paths exact.",
                        labels.join(", ")
                    )
                }
            }
        }
    }
}
