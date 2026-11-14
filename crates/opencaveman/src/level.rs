use serde::{Deserialize, Serialize};

/// Script family governing subword vs. character-density compression ratios.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CavemanScriptFamily {
    LatinControlled,
    ClassicalWenyan,
}

/// Canonical `caveman` compression intensity tiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CavemanLevel {
    Lite,
    #[default]
    Full,
    Ultra,
    WenyanLite,
    WenyanFull,
    WenyanUltra,
}

/// Complete declarative metadata for one `CavemanLevel` — drives UI menus,
/// config validation, savings multipliers, and prompt compilation from one source of truth.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CavemanLevelDescriptor {
    pub level: CavemanLevel,
    pub id: &'static str,
    pub title: &'static str,
    pub short_badge: &'static str,
    pub description: &'static str,
    pub script_family: CavemanScriptFamily,
    /// Calibrated verbose-to-compressed token expansion factor (`baseline = actual * multiplier`).
    pub baseline_expansion_factor: f64,
    /// Expected average output token savings percentage (0..100).
    pub target_savings_pct: u8,
    pub drops_articles: bool,
    pub telegraphic_clauses: bool,
    pub aliases: &'static [&'static str],
    pub directive_rules: &'static [&'static str],
    pub example_bad: &'static str,
    pub example_good: &'static str,
}

pub const LEVEL_DESCRIPTORS: &[CavemanLevelDescriptor] = &[
    CavemanLevelDescriptor {
        level: CavemanLevel::Lite,
        id: "lite",
        title: "Lite (Crisp Professional)",
        short_badge: "LITE",
        description: "Drops pleasantries, hedging, and filler while preserving complete grammatical sentences and articles.",
        script_family: CavemanScriptFamily::LatinControlled,
        baseline_expansion_factor: 1.45,
        target_savings_pct: 31,
        drops_articles: false,
        telegraphic_clauses: false,
        aliases: &["light", "mild", "soft", "clean", "professional"],
        directive_rules: &[
            "Keep full grammatical sentences and natural articles (a/an/the).",
            "Delete all throat-clearing, pleasantries, recaps, apologies, and hedging (maybe/perhaps/basically/just).",
            "Lead with the direct answer or root cause in sentence one.",
            "Preserve code, file paths, CLI flags, symbols, URLs, and error strings byte-for-byte.",
        ],
        example_bad: "Sure! I'd be happy to help explain why your component is re-rendering.",
        example_good: "Your component re-renders because `options` creates a new object reference every render. Memoize it with `useMemo`.",
    },
    CavemanLevelDescriptor {
        level: CavemanLevel::Full,
        id: "full",
        title: "Full (Smart Caveman)",
        short_badge: "FULL",
        description: "Classic smart caveman: drops articles and filler, uses tight fragments, keeps 100% technical precision.",
        script_family: CavemanScriptFamily::LatinControlled,
        baseline_expansion_factor: 2.35,
        target_savings_pct: 57,
        drops_articles: true,
        telegraphic_clauses: false,
        aliases: &["default", "standard", "classic", "caveman", "normal_caveman"],
        directive_rules: &[
            "Respond terse like smart caveman. All technical substance stay. Only fluff die.",
            "Drop articles (a/an/the), filler (just/really/basically/actually), pleasantries, and hedging.",
            "Fragments OK. Short active verbs. One fact per clause.",
            "Pattern: [subject] [action] [cause]. [fix or next step].",
            "Preserve code blocks, file paths, commands, identifiers, numbers, and error logs byte-for-byte.",
        ],
        example_bad: "Sure! I'd be happy to help you fix that authentication issue.",
        example_good: "Bug in auth middleware: token expiry check inverted. Flip `<` to `>`.",
    },
    CavemanLevelDescriptor {
        level: CavemanLevel::Ultra,
        id: "ultra",
        title: "Ultra (Telegraphic Signal)",
        short_badge: "ULTRA",
        description: "Maximum English compression: telegraphic signal tokens, standard technical abbreviations, zero prose connectives.",
        script_family: CavemanScriptFamily::LatinControlled,
        baseline_expansion_factor: 3.30,
        target_savings_pct: 70,
        drops_articles: true,
        telegraphic_clauses: true,
        aliases: &["ultracave", "max", "extreme", "telegraphic"],
        directive_rules: &[
            "Telegraphic ultra-dense mode. Emit pure technical signal; zero connective prose.",
            "Drop articles, copulas (is/are/was), pronouns, and conjunctions where unambiguous.",
            "Use standard engineering shorthand (`cfg`, `fn`, `impl`, `auth`, `db`, `req`, `res`, `err`, `->`, `=>`).",
            "Answer yes/no questions with `Yes.` or `No.` plus <= 1 causal fragment.",
            "Preserve code blocks, paths, commands, and exact identifiers byte-for-byte.",
        ],
        example_bad: "The database connection pool is timing out because max_connections is too low.",
        example_good: "DB pool timeout: `max_connections` too low -> raise `10` to `32` in `db.toml`.",
    },
    CavemanLevelDescriptor {
        level: CavemanLevel::WenyanLite,
        id: "wenyan_lite",
        title: "Wenyan Lite (Semi-Classical)",
        short_badge: "文·LITE",
        description: "Concise semi-classical Chinese prose for explanations while keeping all code, paths, and English identifiers verbatim.",
        script_family: CavemanScriptFamily::ClassicalWenyan,
        baseline_expansion_factor: 2.60,
        target_savings_pct: 62,
        drops_articles: true,
        telegraphic_clauses: false,
        aliases: &["wenyan-lite", "semi-wenyan"],
        directive_rules: &[
            "Use concise semi-classical Chinese (淺近文言) for natural-language explanations to exploit high CJK token density.",
            "Keep EVERY code symbol, identifier, file path, CLI command, API name, URL, and code fence verbatim in original ASCII/English.",
            "Never translate variable names, crate names, error messages, or config keys.",
            "Omit greetings, closing remarks, and rhetorical padding.",
        ],
        example_bad: "這個問題是因為中間件裡的權杖過期檢查邏輯寫反了，建議將小於號改為大於號。",
        example_good: "`auth` 中間件 `exp` 校驗顛倒，改 `<` 為 `>` 即癒。",
    },
    CavemanLevelDescriptor {
        level: CavemanLevel::WenyanFull,
        id: "wenyan_full",
        title: "Wenyan Full (Classical Megacave)",
        short_badge: "文·FULL",
        description: "Dense Classical Chinese (文言文) phrasing with byte-exact technical symbols for ~72% token reduction.",
        script_family: CavemanScriptFamily::ClassicalWenyan,
        baseline_expansion_factor: 3.55,
        target_savings_pct: 72,
        drops_articles: true,
        telegraphic_clauses: true,
        aliases: &["wenyan", "wenyan-full", "megacave", "classical"],
        directive_rules: &[
            "Respond in terse Classical Chinese (文言文): 辭約意豐，直指病根，絕去寒暄。",
            "Keep EVERY code block, file path, identifier, CLI flag, version, and error literal byte-exact in ASCII.",
            "Structure: 【病】root cause → 【治】exact fix.",
            "Zero modern filler particles; pack maximum semantic density per character.",
        ],
        example_bad: "資料庫連線池逾時是因為最大連線數設定過低。",
        example_good: "【病】DB 池逾時，`max_connections` 過隘。【治】於 `db.toml` 增至 `32`。",
    },
    CavemanLevelDescriptor {
        level: CavemanLevel::WenyanUltra,
        id: "wenyan_ultra",
        title: "Wenyan Ultra (Extreme Classical)",
        short_badge: "文·ULTRA",
        description: "Ultra-compact 4–8 character classical clauses paired with exact ASCII symbols for peak CJK token compression.",
        script_family: CavemanScriptFamily::ClassicalWenyan,
        baseline_expansion_factor: 4.10,
        target_savings_pct: 76,
        drops_articles: true,
        telegraphic_clauses: true,
        aliases: &["wenyan-ultra", "megacave-ultra"],
        directive_rules: &[
            "Extreme Classical Chinese (極簡文言): 4–8 character clauses max per point, zero particles where clear.",
            "All code, identifiers, CLI flags, file paths, and error strings remain 100% verbatim ASCII.",
            "Emit only causal chain + patch.",
        ],
        example_bad: "連線池耗盡，請調高設定。",
        example_good: "`db.toml` `max_connections` 隘→改 `32`。",
    },
];

/// Active combined configuration state (`caveman_mode` boolean + `caveman_level`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CavemanConfigState {
    pub enabled: bool,
    pub level: CavemanLevel,
}

impl Default for CavemanConfigState {
    fn default() -> Self {
        Self {
            enabled: true,
            level: CavemanLevel::Full,
        }
    }
}

pub fn all_level_descriptors() -> &'static [CavemanLevelDescriptor] {
    LEVEL_DESCRIPTORS
}

fn sanitize_token(raw: &str) -> String {
    raw.trim()
        .chars()
        .map(|ch| match ch {
            '-' | ' ' => '_',
            other => other.to_ascii_lowercase(),
        })
        .collect()
}

impl CavemanLevel {
    pub fn descriptor(self) -> &'static CavemanLevelDescriptor {
        LEVEL_DESCRIPTORS
            .iter()
            .find(|d| d.level == self)
            .unwrap_or(&LEVEL_DESCRIPTORS[1])
    }

    pub fn as_str(self) -> &'static str {
        self.descriptor().id
    }

    pub fn slug(self) -> &'static str {
        self.descriptor().id
    }

    pub fn title(self) -> &'static str {
        self.descriptor().title
    }

    pub fn short_badge(self) -> &'static str {
        self.descriptor().short_badge
    }

    pub fn parse_flexible(raw: &str) -> Option<Self> {
        let normalized = sanitize_token(raw);
        let original_lower = raw.trim().to_ascii_lowercase();
        LEVEL_DESCRIPTORS
            .iter()
            .find(|desc| {
                desc.id == normalized
                    || desc.aliases.iter().any(|alias| {
                        *alias == original_lower || sanitize_token(alias) == normalized
                    })
            })
            .map(|desc| desc.level)
    }

    pub fn from_config_str(raw: &str) -> Self {
        Self::parse_flexible(raw).unwrap_or_default()
    }

    pub fn cycle_next(self) -> Self {
        let idx = LEVEL_DESCRIPTORS
            .iter()
            .position(|d| d.level == self)
            .unwrap_or(1);
        let next_idx = (idx + 1) % LEVEL_DESCRIPTORS.len();
        LEVEL_DESCRIPTORS[next_idx].level
    }

    pub fn cycle_prev(self) -> Self {
        let idx = LEVEL_DESCRIPTORS
            .iter()
            .position(|d| d.level == self)
            .unwrap_or(1);
        let prev_idx = (idx + LEVEL_DESCRIPTORS.len() - 1) % LEVEL_DESCRIPTORS.len();
        LEVEL_DESCRIPTORS[prev_idx].level
    }
}

pub fn normalize_level_slug(raw: &str) -> &'static str {
    CavemanLevel::from_config_str(raw).as_str()
}
