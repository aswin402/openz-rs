use crate::level::CavemanLevel;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

static SESSION_TURN_SAVINGS: LazyLock<Mutex<usize>> = LazyLock::new(|| Mutex::new(0));
static SESSION_TURN_COUNT: LazyLock<Mutex<usize>> = LazyLock::new(|| Mutex::new(0));

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CavemanStatsLedger {
    pub total_turns: u64,
    pub total_output_tokens_actual: u64,
    pub total_output_tokens_baseline: u64,
    pub total_output_tokens_saved: u64,
    pub total_compressed_files: u64,
    pub total_input_tokens_saved: u64,
    pub by_level_saved: BTreeMap<String, u64>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CavemanStatsSnapshot {
    pub session_turns: usize,
    pub session_tokens_saved: usize,
    pub lifetime_turns: u64,
    pub lifetime_output_tokens_saved: u64,
    pub lifetime_compressed_files: u64,
    pub lifetime_input_tokens_saved: u64,
    pub lifetime_total_tokens_saved: u64,
    pub compact_lifetime_badge: String,
    pub compact_session_badge: String,
    pub by_level_saved: BTreeMap<String, u64>,
}

impl CavemanStatsSnapshot {
    pub fn non_zero_badge(&self) -> String {
        match self.lifetime_total_tokens_saved > 0 {
            true => self.compact_lifetime_badge.clone(),
            false => String::new(),
        }
    }
}

fn is_cjk_ideograph(ch: char) -> bool {
    let code = ch as u32;
    (0x4E00..=0x9FFF).contains(&code)
        || (0x3400..=0x4DBF).contains(&code)
        || (0xF900..=0xFAFF).contains(&code)
}

/// Deterministic subword + CJK-aware token estimator.
/// Models BPE subword splitting on ASCII words/punctuation alongside ~1.15 tokens/char on CJK scripts.
pub fn estimate_tokens(text: &str) -> usize {
    let trimmed = text.trim();
    match trimmed.is_empty() {
        true => 0,
        false => {
            let mut ascii_alnum_chars = 0usize;
            let mut ascii_punct_chars = 0usize;
            let mut cjk_chars = 0usize;
            let mut other_non_ws = 0usize;

            for ch in trimmed.chars() {
                match (
                    ch.is_whitespace(),
                    ch.is_ascii_alphanumeric(),
                    ch.is_ascii_punctuation(),
                    is_cjk_ideograph(ch),
                ) {
                    (true, _, _, _) => {}
                    (false, true, _, _) => ascii_alnum_chars += 1,
                    (false, false, true, _) => ascii_punct_chars += 1,
                    (false, false, false, true) => cjk_chars += 1,
                    (false, false, false, false) => other_non_ws += 1,
                }
            }

            let word_tokens = ascii_alnum_chars.div_ceil(4);
            let punct_tokens = ascii_punct_chars.div_ceil(2);
            let cjk_tokens = ((cjk_chars * 23) + 19) / 20;
            let other_tokens = other_non_ws.div_ceil(3);
            (word_tokens + punct_tokens + cjk_tokens + other_tokens).max(1)
        }
    }
}

pub fn format_compact_savings(tokens: u64) -> String {
    match (tokens >= 1_000_000, tokens >= 1_000) {
        (true, _) => format!("{:.1}M", (tokens as f64) / 1_000_000.0),
        (false, true) => format!("{:.1}k", (tokens as f64) / 1_000.0),
        (false, false) => tokens.to_string(),
    }
}

fn ledger_path(runtime_data_dir: &Path) -> PathBuf {
    runtime_data_dir.join("opencaveman_stats.json")
}

pub fn load_stats_ledger(runtime_data_dir: &Path) -> CavemanStatsSnapshot {
    let ledger: CavemanStatsLedger = std::fs::read_to_string(ledger_path(runtime_data_dir))
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default();

    let session_saved = SESSION_TURN_SAVINGS
        .lock()
        .map(|g| *g)
        .unwrap_or(0);
    let session_turns = SESSION_TURN_COUNT
        .lock()
        .map(|g| *g)
        .unwrap_or(0);

    let total_lifetime = ledger
        .total_output_tokens_saved
        .saturating_add(ledger.total_input_tokens_saved);

    CavemanStatsSnapshot {
        session_turns,
        session_tokens_saved: session_saved,
        lifetime_turns: ledger.total_turns,
        lifetime_output_tokens_saved: ledger.total_output_tokens_saved,
        lifetime_compressed_files: ledger.total_compressed_files,
        lifetime_input_tokens_saved: ledger.total_input_tokens_saved,
        lifetime_total_tokens_saved: total_lifetime,
        compact_lifetime_badge: format_compact_savings(total_lifetime),
        compact_session_badge: format_compact_savings(session_saved as u64),
        by_level_saved: ledger.by_level_saved,
    }
}

fn save_ledger(runtime_data_dir: &Path, ledger: &CavemanStatsLedger) -> anyhow::Result<()> {
    std::fs::create_dir_all(runtime_data_dir)?;
    let raw = serde_json::to_string_pretty(ledger)?;
    std::fs::write(ledger_path(runtime_data_dir), raw)?;
    Ok(())
}

/// Records output token savings for one completed Caveman assistant turn.
pub fn record_turn_savings(
    runtime_data_dir: &Path,
    level: CavemanLevel,
    assistant_output: &str,
) -> CavemanStatsSnapshot {
    let actual_tokens = estimate_tokens(assistant_output);
    match actual_tokens == 0 {
        true => return load_stats_ledger(runtime_data_dir),
        false => {}
    }

    let multiplier = level.descriptor().baseline_expansion_factor;
    let baseline_tokens = ((actual_tokens as f64) * multiplier).round() as usize;
    let saved_tokens = baseline_tokens.saturating_sub(actual_tokens);

    let _ = SESSION_TURN_SAVINGS.lock().map(|mut g| *g += saved_tokens);
    let _ = SESSION_TURN_COUNT.lock().map(|mut g| *g += 1);

    let mut ledger: CavemanStatsLedger = std::fs::read_to_string(ledger_path(runtime_data_dir))
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default();

    ledger.total_turns = ledger.total_turns.saturating_add(1);
    ledger.total_output_tokens_actual = ledger
        .total_output_tokens_actual
        .saturating_add(actual_tokens as u64);
    ledger.total_output_tokens_baseline = ledger
        .total_output_tokens_baseline
        .saturating_add(baseline_tokens as u64);
    ledger.total_output_tokens_saved = ledger
        .total_output_tokens_saved
        .saturating_add(saved_tokens as u64);
    let entry = ledger
        .by_level_saved
        .entry(level.as_str().to_string())
        .or_insert(0);
    *entry = entry.saturating_add(saved_tokens as u64);
    ledger.updated_at = Some(chrono::Utc::now().to_rfc3339());

    let _ = save_ledger(runtime_data_dir, &ledger);
    load_stats_ledger(runtime_data_dir)
}

/// Records input token savings from compressing a Markdown/memory context file.
pub fn record_compression_savings(
    runtime_data_dir: &Path,
    original_tokens: usize,
    compressed_tokens: usize,
) -> CavemanStatsSnapshot {
    let saved = original_tokens.saturating_sub(compressed_tokens);
    let _ = SESSION_TURN_SAVINGS.lock().map(|mut g| *g += saved);

    let mut ledger: CavemanStatsLedger = std::fs::read_to_string(ledger_path(runtime_data_dir))
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default();

    ledger.total_compressed_files = ledger.total_compressed_files.saturating_add(1);
    ledger.total_input_tokens_saved = ledger
        .total_input_tokens_saved
        .saturating_add(saved as u64);
    ledger.updated_at = Some(chrono::Utc::now().to_rfc3339());

    let _ = save_ledger(runtime_data_dir, &ledger);
    load_stats_ledger(runtime_data_dir)
}

pub fn reset_stats_ledger(runtime_data_dir: &Path) -> anyhow::Result<CavemanStatsSnapshot> {
    let _ = SESSION_TURN_SAVINGS.lock().map(|mut g| *g = 0);
    let _ = SESSION_TURN_COUNT.lock().map(|mut g| *g = 0);
    let empty = CavemanStatsLedger::default();
    save_ledger(runtime_data_dir, &empty)?;
    Ok(load_stats_ledger(runtime_data_dir))
}
