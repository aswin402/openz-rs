use serde::{Deserialize, Serialize};

/// Compression style family governing controlled prose vs. symbolic engineering notation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CavemanScriptFamily {
    LatinControlled,
    SymbolicNotation,
}

/// Canonical `caveman` compression intensity tiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CavemanLevel {
    Lite,
    #[default]
    Full,
    Ultra,
    SymbolicLite,
    SymbolicFull,
    SymbolicUltra,
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
        level: CavemanLevel::SymbolicLite,
        id: "symbolic_lite",
        title: "Symbolic Lite (Structured Shorthand)",
        short_badge: "SYM·LITE",
        description: "Compact structured English shorthand using arrows and standard engineering abbreviations while keeping causal verbs clear.",
        script_family: CavemanScriptFamily::SymbolicNotation,
        baseline_expansion_factor: 2.60,
        target_savings_pct: 62,
        drops_articles: true,
        telegraphic_clauses: false,
        aliases: &["symbolic-lite", "shorthand"],
        directive_rules: &[
            "Use compact English shorthand and causal arrows (`->`, `=>`) for explanations instead of full paragraphs.",
            "Keep EVERY code symbol, identifier, file path, CLI command, API name, URL, and code fence verbatim in original ASCII/English.",
            "Never alter variable names, crate names, error messages, or config keys.",
            "Omit greetings, closing remarks, and rhetorical padding.",
        ],
        example_bad: "This authentication failure happens because the token expiration check in the middleware is inverted, so change the less-than operator to greater-than.",
        example_good: "`auth` middleware `exp` check inverted -> flip `<` to `>`.",
    },
    CavemanLevelDescriptor {
        level: CavemanLevel::SymbolicFull,
        id: "symbolic_full",
        title: "Symbolic Full (Causal Notation)",
        short_badge: "SYM·FULL",
        description: "Dense `[CAUSE] -> [FIX]` English symbolic notation with byte-exact technical identifiers for ~72% token reduction.",
        script_family: CavemanScriptFamily::SymbolicNotation,
        baseline_expansion_factor: 3.55,
        target_savings_pct: 72,
        drops_articles: true,
        telegraphic_clauses: true,
        aliases: &["symbolic", "symbolic-full", "megacave", "notation"],
        directive_rules: &[
            "Respond in dense English causal notation: state root cause and exact remediation with zero prose filler.",
            "Keep EVERY code block, file path, identifier, CLI flag, version, and error literal byte-exact in ASCII.",
            "Structure: `[CAUSE] <root cause> -> [FIX] <exact fix>`.",
            "Zero conversational connectives; pack maximum technical signal per token.",
        ],
        example_bad: "The database connection pool is timing out because the maximum connection limit is configured too low.",
        example_good: "[CAUSE] DB pool timeout (`max_connections` too low) -> [FIX] raise to `32` in `db.toml`.",
    },
    CavemanLevelDescriptor {
        level: CavemanLevel::SymbolicUltra,
        id: "symbolic_ultra",
        title: "Symbolic Ultra (Atomic Delta)",
        short_badge: "SYM·ULTRA",
        description: "Ultra-compact 3–6 token English `target: issue -> patch` deltas paired with exact ASCII symbols for peak token compression.",
        script_family: CavemanScriptFamily::SymbolicNotation,
        baseline_expansion_factor: 4.10,
        target_savings_pct: 76,
        drops_articles: true,
        telegraphic_clauses: true,
        aliases: &["symbolic-ultra", "megacave-ultra", "atomic"],
        directive_rules: &[
            "Extreme English atomic-delta notation: 3–6 words max per point, zero articles or auxiliary verbs.",
            "All code, identifiers, CLI flags, file paths, and error strings remain 100% verbatim ASCII.",
            "Emit only `<target>: <issue> -> <patch>`.",
        ],
        example_bad: "The connection pool is exhausted, please increase the setting in your configuration file.",
        example_good: "`db.toml` `max_connections` low -> set `32`.",
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
