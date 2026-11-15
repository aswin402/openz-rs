use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchFailureKind {
    RateLimited,
    Blocked,
    Timeout,
    NoUsableResults,
    Other,
}

impl SearchFailureKind {
    pub fn as_str(self) -> &'static str {
        match self {
            SearchFailureKind::RateLimited => "rate_limited",
            SearchFailureKind::Blocked => "blocked",
            SearchFailureKind::Timeout => "timeout",
            SearchFailureKind::NoUsableResults => "no_usable_results",
            SearchFailureKind::Other => "other",
        }
    }

    pub fn should_auto_cooldown(self) -> bool {
        matches!(
            self,
            SearchFailureKind::RateLimited
                | SearchFailureKind::Blocked
                | SearchFailureKind::Timeout
        )
    }
}

pub fn classify_search_failure(message: &str) -> SearchFailureKind {
    let normalized = message.to_ascii_lowercase();
    const FAILURE_RULES: &[(&[&str], SearchFailureKind)] = &[
        (
            &["429", "too many requests", "rate limit", "rate-limit"],
            SearchFailureKind::RateLimited,
        ),
        (
            &[
                "403",
                "forbidden",
                "captcha",
                "blocked",
                "unusual traffic",
                "verify you are human",
            ],
            SearchFailureKind::Blocked,
        ),
        (&["timed out", "timeout", "deadline"], SearchFailureKind::Timeout),
        (
            &["0 usable results", "all search backends exhausted"],
            SearchFailureKind::NoUsableResults,
        ),
    ];
    FAILURE_RULES
        .iter()
        .find_map(|(patterns, kind)| {
            patterns
                .iter()
                .any(|pat| normalized.contains(pat))
                .then_some(*kind)
        })
        .unwrap_or(SearchFailureKind::Other)
}

pub fn search_failure_cooldown_secs(kind: SearchFailureKind) -> u64 {
    match kind {
        SearchFailureKind::RateLimited => 1800,
        SearchFailureKind::Blocked => 3600,
        SearchFailureKind::Timeout => 300,
        SearchFailureKind::NoUsableResults => 600,
        SearchFailureKind::Other => 120,
    }
}

#[derive(Debug, Clone)]
struct BackendCooldown {
    kind: SearchFailureKind,
    until: Instant,
}

static SEARCH_BACKEND_COOLDOWNS: OnceLock<Mutex<HashMap<String, BackendCooldown>>> =
    OnceLock::new();

#[cfg(test)]
pub static COOLDOWN_TEST_LOCK: Mutex<()> = Mutex::new(());

fn search_backend_cooldowns() -> &'static Mutex<HashMap<String, BackendCooldown>> {
    SEARCH_BACKEND_COOLDOWNS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn record_search_backend_failure(backends: &[String], kind: SearchFailureKind) {
    match backends.is_empty() {
        true => return,
        false => {}
    }

    let until = Instant::now() + std::time::Duration::from_secs(search_failure_cooldown_secs(kind));
    let mut cooldowns = search_backend_cooldowns()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    for backend in backends.iter().filter(|b| !b.trim().is_empty()) {
        cooldowns.insert(backend.clone(), BackendCooldown { kind, until });
    }
}

pub fn record_transient_backend_error(backend: &str, message: &str) {
    let trimmed = backend.trim();
    match trimmed.is_empty() {
        true => return,
        false => {}
    }
    let kind = classify_search_failure(message);
    match kind.should_auto_cooldown() {
        true => record_search_backend_failure(&[trimmed.to_string()], kind),
        false => {}
    }
}

pub fn clear_backend_cooldown(backend: &str) {
    search_backend_cooldowns()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(backend);
}

pub fn is_backend_cooling_down(backend: &str) -> Option<(SearchFailureKind, u64)> {
    let now = Instant::now();
    let mut cooldowns = search_backend_cooldowns()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    cooldowns.retain(|_, cooldown| cooldown.until > now);
    cooldowns.get(backend).map(|cooldown| {
        (
            cooldown.kind,
            cooldown.until.saturating_duration_since(now).as_secs().max(1),
        )
    })
}

pub fn shortest_cooldown_backend(names: &[&str]) -> Option<String> {
    let now = Instant::now();
    let mut cooldowns = search_backend_cooldowns()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    cooldowns.retain(|_, cooldown| cooldown.until > now);
    names
        .iter()
        .filter_map(|name| {
            cooldowns.get(*name).map(|cd| {
                (
                    *name,
                    cd.until.saturating_duration_since(now).as_millis(),
                )
            })
        })
        .min_by_key(|(_, remaining_ms)| *remaining_ms)
        .map(|(name, _)| name.to_string())
}

pub fn active_search_backend_cooldowns() -> Vec<Value> {
    let now = Instant::now();
    let mut cooldowns = search_backend_cooldowns()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    cooldowns.retain(|_, cooldown| cooldown.until > now);

    let mut active = cooldowns
        .iter()
        .map(|(backend, cooldown)| {
            json!({
                "backend": backend,
                "error_kind": cooldown.kind.as_str(),
                "cooldown_remaining_secs": cooldown.until.saturating_duration_since(now).as_secs().max(1),
            })
        })
        .collect::<Vec<_>>();
    active.sort_by(|a, b| {
        a["backend"]
            .as_str()
            .unwrap_or_default()
            .cmp(b["backend"].as_str().unwrap_or_default())
    });
    active
}

#[cfg(test)]
pub fn clear_search_backend_cooldowns_for_tests() {
    search_backend_cooldowns()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clear();
}

pub fn search_failure_error_value(
    raw_error: &str,
    kind: SearchFailureKind,
    backends: &[String],
) -> Value {
    json!({
        "status": "search_failed",
        "error_kind": kind.as_str(),
        "retryable": true,
        "cooldown_secs": search_failure_cooldown_secs(kind),
        "affected_backends": backends,
        "raw_error": raw_error,
        "next_step": "Use searchxyz_browser_search for provider-free browser discovery, search_research for local indexed content, or web_fetch for a known URL."
    })
}
