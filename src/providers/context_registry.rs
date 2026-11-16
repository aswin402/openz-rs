//! Universal Dynamic Model Capability Registry & Algorithmic Token Budgeting Engine.
//!
//! Resolves per-model `context_window`, `max_output_tokens`, `supports_vision`, `supports_tools`,
//! `supports_reasoning`, and `parameter_billions` without hardcoded vendor model lists via a
//! 6-tier cascading architecture inspired by OpenCode (`models.dev`), Cline/Roo Code (`OpenRouter`),
//! Pi Agent (`models.json`), Ollama (`/api/show`), LM Studio (`/api/v1/models`), and reactive
//! HTTP 400 context-overflow self-learning.

use crate::config::schema::Config;
use crate::session::Message;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const DEFAULT_PROMPT_BUDGET_RATIO: f64 = 0.25;
pub const DEFAULT_TOOL_OUTPUT_RATIO: f64 = 0.08;
pub const DEFAULT_COMPACTION_THRESHOLD_RATIO: f64 = 0.80;
pub const DEFAULT_KEEP_RECENT_RATIO: f64 = 0.20;
pub const FALLBACK_CONTEXT_WINDOW_TOKENS: usize = 128_000;
pub const FALLBACK_MAX_OUTPUT_TOKENS: usize = 8_192;
pub const CHARS_PER_TOKEN: f64 = 4.0;
pub const MIN_PROMPT_BUDGET_CHARS: usize = 8_000;
pub const MIN_TOOL_OUTPUT_CHARS: usize = 4_000;
pub const MAX_DEFAULT_INLINE_TOOL_OUTPUT_CHARS: usize = 65_536;
const CATALOG_CACHE_TTL_SECS: u64 = 86_400;
const MODELS_DEV_API_URL: &str = "https://models.dev/api.json";
const OPENROUTER_MODELS_API_URL: &str = "https://openrouter.ai/api/v1/models";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelSpec {
    pub context_window: usize,
    pub max_output_tokens: usize,
    pub supports_vision: bool,
    pub supports_tools: bool,
    pub supports_reasoning: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameter_billions: Option<u32>,
    pub source: String,
}

impl Default for ModelSpec {
    fn default() -> Self {
        Self {
            context_window: FALLBACK_CONTEXT_WINDOW_TOKENS,
            max_output_tokens: FALLBACK_MAX_OUTPUT_TOKENS,
            supports_vision: false,
            supports_tools: true,
            supports_reasoning: false,
            parameter_billions: None,
            source: "default_fallback".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UserModelEntry {
    #[serde(alias = "context", alias = "contextWindow", alias = "max_input_tokens")]
    pub context_window: Option<usize>,
    #[serde(alias = "output", alias = "maxTokens", alias = "max_tokens")]
    pub max_output_tokens: Option<usize>,
    #[serde(alias = "vision")]
    pub supports_vision: Option<bool>,
    #[serde(alias = "tool_call", alias = "tools")]
    pub supports_tools: Option<bool>,
    #[serde(alias = "reasoning", alias = "thinking")]
    pub supports_reasoning: Option<bool>,
}

#[derive(Debug, Clone, Default)]
pub struct ModelsCatalogFile {
    pub models: HashMap<String, ModelSpec>,
    pub patterns: HashMap<String, usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct CachedSpecsStore {
    #[serde(default)]
    updated_at_epoch_secs: u64,
    #[serde(default)]
    learned_limits: HashMap<String, usize>,
    #[serde(default)]
    provider_models: HashMap<String, ModelSpec>,
    #[serde(default)]
    public_catalog: HashMap<String, ModelSpec>,
}

#[derive(Default)]
struct MemoryRegistryState {
    loaded_dir: Option<PathBuf>,
    store: CachedSpecsStore,
}

static REGISTRY_STATE: OnceLock<RwLock<MemoryRegistryState>> = OnceLock::new();
static SYNC_IN_FLIGHT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn registry_lock() -> &'static RwLock<MemoryRegistryState> {
    REGISTRY_STATE.get_or_init(|| RwLock::new(MemoryRegistryState::default()))
}

fn now_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn cache_file_path_in(dir: &Path) -> PathBuf {
    dir.join("model_specs_cache.json")
}

fn models_catalog_path_in(dir: &Path) -> PathBuf {
    dir.join("models.json")
}

fn ensure_store_loaded_for_dir(dir: &Path) {
    let already_loaded = match registry_lock().read() {
        Ok(guard) => guard.loaded_dir.as_deref() == Some(dir),
        Err(_) => false,
    };
    match already_loaded {
        true => return,
        false => {}
    }

    let disk_store = std::fs::read_to_string(cache_file_path_in(dir))
        .ok()
        .and_then(|raw| serde_json::from_str::<CachedSpecsStore>(&raw).ok())
        .unwrap_or_default();

    match registry_lock().write() {
        Ok(mut guard) => {
            guard.loaded_dir = Some(dir.to_path_buf());
            guard.store = disk_store;
        }
        Err(_) => {}
    }
}

fn persist_store_for_dir(dir: &Path) {
    let snapshot = match registry_lock().read() {
        Ok(guard) => guard.store.clone(),
        Err(_) => return,
    };
    let _ = std::fs::create_dir_all(dir);
    let target = cache_file_path_in(dir);
    let tmp = target.with_extension(format!("tmp.{}", std::process::id()));
    match serde_json::to_string(&snapshot) {
        Ok(encoded) => match std::fs::write(&tmp, encoded.as_bytes()) {
            Ok(()) => {
                let _ = std::fs::rename(&tmp, &target);
            }
            Err(_) => {
                let _ = std::fs::remove_file(&tmp);
            }
        },
        Err(_) => {}
    }
}

// ─── Slug Normalization, Version-Prefix Trie Keys & Structural Grammar ──────

const VARIANT_COLON_TAGS: &[&str] = &[
    "free", "extended", "exacto", "nitro", "online", "floor", "beta", "latest",
];

const NON_VERSION_TRAILERS: &[&str] = &[
    "latest",
    "preview",
    "exp",
    "experimental",
    "beta",
    "alpha",
    "instruct",
    "chat",
    "it",
    "hf",
    "gguf",
    "awq",
    "gptq",
    "fp8",
    "bf16",
];

fn is_revision_or_build_token(tok: &str) -> bool {
    NON_VERSION_TRAILERS.contains(&tok)
        || (tok.len() >= 3 && tok.len() <= 8 && tok.bytes().all(|b| b.is_ascii_digit()))
}

fn is_alnum_generation_token(tok: &str) -> bool {
    let bytes = tok.as_bytes();
    bytes.len() >= 2
        && bytes[0].is_ascii_alphabetic()
        && bytes[1..].iter().all(|b| b.is_ascii_digit())
        && parse_single_param_token_b(tok).is_none()
}

fn is_date_stamp_token(tok: &str) -> bool {
    (tok.len() == 8 || tok.len() == 4) && tok.bytes().all(|b| b.is_ascii_digit()) && {
        tok.starts_with("202") || tok.len() == 8
    }
}

fn strip_variant_colon_suffix(raw: &str) -> String {
    match raw.rsplit_once(':') {
        Some((base, tag)) => {
            let tag_lc = tag.trim().to_ascii_lowercase();
            match VARIANT_COLON_TAGS.contains(&tag_lc.as_str()) {
                true => base.to_string(),
                false => format!("{base}-{tag_lc}"),
            }
        }
        None => raw.to_string(),
    }
}

pub fn bare_model_slice(raw: &str) -> &str {
    let trimmed = raw.trim();
    let after_colon = match trimmed.find("::") {
        Some(idx) => &trimmed[idx + 2..],
        None => trimmed,
    };
    match after_colon.rsplit_once('/') {
        Some((_, tail)) => tail,
        None => after_colon,
    }
}

pub fn tokenize_model_slug(raw: &str) -> Vec<String> {
    let cleaned = strip_variant_colon_suffix(bare_model_slice(raw));
    let mut tokens: Vec<String> = cleaned
        .to_ascii_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();

    while tokens
        .last()
        .is_some_and(|last| last == "latest" || is_date_stamp_token(last))
        && tokens.len() > 1
    {
        tokens.pop();
    }
    tokens
}

pub fn canonical_model_key(raw: &str) -> String {
    tokenize_model_slug(raw).join("-")
}

pub fn provider_qualified_key(provider_opt: Option<&str>, raw_model: &str) -> Option<String> {
    let explicit_prov = raw_model
        .trim()
        .split_once('/')
        .map(|(p, _)| p.trim().to_ascii_lowercase());
    let prov = provider_opt
        .map(|p| p.trim().to_ascii_lowercase())
        .filter(|p| !p.is_empty() && p != "auto")
        .or(explicit_prov)?;
    let model_key = canonical_model_key(raw_model);
    (!model_key.is_empty()).then(|| format!("{prov}/{model_key}"))
}

fn trimmed_revision_tokens<'a>(tokens: &'a [String]) -> Vec<&'a str> {
    let mut core: Vec<&'a str> = tokens.iter().map(String::as_str).collect();
    while core.len() > 2 && core.last().is_some_and(|last| is_revision_or_build_token(last)) {
        core.pop();
    }
    core
}

fn param_stripped_tokens<'a>(tokens: &'a [&'a str]) -> Option<String> {
    let filtered: Vec<&str> = tokens
        .iter()
        .copied()
        .filter(|tok| !is_revision_or_build_token(tok) && parse_single_param_token_b(tok).is_none())
        .collect();
    (filtered.len() >= 2 && filtered.len() < tokens.len()).then(|| filtered.join("-"))
}

fn parse_single_param_token_b(tok: &str) -> Option<u32> {
    let lower = tok.trim().to_ascii_lowercase();
    match lower.strip_suffix('b') {
        None => None,
        Some(num_part) => match num_part.split_once('x') {
            Some((experts_s, size_s)) => {
                let experts = experts_s.parse::<f64>().ok()?;
                let size = size_s.parse::<f64>().ok()?;
                let prod = (experts * size).round() as u32;
                (prod > 0 && prod <= 10_000).then_some(prod)
            }
            None => {
                let val = num_part.parse::<f64>().ok()?;
                let rounded = val.ceil() as u32;
                (val > 0.0 && rounded <= 10_000).then_some(rounded)
            }
        },
    }
}

/// Extract total parameter count (in billions) numerically from any model slug
/// (e.g. `qwen2.5-coder-32b` -> `Some(32)`, `mixtral-8x22b` -> `Some(176)`, `llama-3.2-1.5b` -> `Some(2)`).
pub fn parse_parameter_billions(model: &str) -> Option<u32> {
    let lower = strip_variant_colon_suffix(bare_model_slice(model)).to_ascii_lowercase();
    lower
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '.')
        .filter(|s| !s.is_empty())
        .find_map(parse_single_param_token_b)
}

fn parse_explicit_context_token(tok: &str) -> Option<usize> {
    let lower = tok.to_ascii_lowercase();
    match lower.strip_suffix('k') {
        Some(digits) => {
            let n = digits.parse::<usize>().ok()?;
            matches!(n, 4 | 8 | 16 | 32 | 64 | 100 | 128 | 200 | 256 | 500 | 512 | 1000).then(|| {
                match n {
                    100 | 200 | 500 | 1000 => n * 1_000,
                    _ => n * 1_024,
                }
            })
        }
        None => match lower.strip_suffix('m') {
            Some(digits) => {
                let n = digits.parse::<usize>().ok()?;
                (1..=10).contains(&n).then_some(n * 1_000_000)
            }
            None => None,
        },
    }
}

pub fn infer_vision_from_slug_grammar(model: &str, tokens: &[String]) -> bool {
    let raw_lc = model.trim().to_ascii_lowercase();
    let mivi_prefix = raw_lc == "mivi" || raw_lc.starts_with("mivi/");

    let vision_tokens = [
        "vision",
        "vl",
        "vlm",
        "multimodal",
        "omni",
        "pixtral",
        "paligemma",
        "qvq",
        "mivi",
        "gemini",
    ];
    let has_vision_token = tokens
        .iter()
        .any(|t| vision_tokens.contains(&t.as_str()));

    let claude_vision = tokens
        .windows(2)
        .any(|w| w[0] == "claude" && w[1].parse::<u32>().is_ok_and(|v| v >= 3));
    let gemma_vision = tokens
        .windows(2)
        .any(|w| w[0] == "gemma" && w[1].parse::<u32>().is_ok_and(|v| v >= 3));
    let gpt_vision = tokens
        .windows(2)
        .any(|w| w[0] == "gpt" && (w[1] == "4o" || w[1].parse::<u32>().is_ok_and(|v| v >= 5)))
        || tokens
            .windows(3)
            .any(|w| w[0] == "gpt" && w[1] == "4" && w[2] == "turbo");
    let o_series_vision = tokens.first().is_some_and(|first| {
        (first == "o1" && !tokens.iter().any(|t| t == "mini" || t == "preview"))
            || (first.starts_with('o') && first[1..].parse::<u32>().is_ok_and(|v| v >= 3))
    });

    mivi_prefix
        || has_vision_token
        || claude_vision
        || gemma_vision
        || gpt_vision
        || o_series_vision
}

pub fn infer_spec_from_slug_grammar(model: &str) -> ModelSpec {
    let tokens = tokenize_model_slug(model);
    let explicit_ctx = tokens.iter().find_map(|t| parse_explicit_context_token(t));
    let param_b = parse_parameter_billions(model);
    let supports_vision = infer_vision_from_slug_grammar(model, &tokens);

    let reasoning_tokens = [
        "reasoner",
        "reasoning",
        "thinking",
        "r1",
        "qwq",
        "qvq",
        "o1",
        "o3",
        "o4",
    ];
    let has_reasoning_token = tokens
        .iter()
        .any(|t| reasoning_tokens.contains(&t.as_str()))
        && !tokens.iter().any(|t| t == "mini" && tokens.first().is_some_and(|f| f == "o1"));

    let context_window = explicit_ctx.unwrap_or(FALLBACK_CONTEXT_WINDOW_TOKENS);
    let max_output_tokens = match has_reasoning_token {
        true => 32_768.min(context_window / 2).max(4_096),
        false => FALLBACK_MAX_OUTPUT_TOKENS.min(context_window / 2).max(2_048),
    };

    ModelSpec {
        context_window,
        max_output_tokens,
        supports_vision,
        supports_tools: true,
        supports_reasoning: has_reasoning_token,
        parameter_billions: param_b,
        source: "slug_grammar".to_string(),
    }
}

fn insert_spec_into_map(
    map: &mut HashMap<String, ModelSpec>,
    provider_opt: Option<&str>,
    raw_id: &str,
    spec: ModelSpec,
) {
    let tokens = tokenize_model_slug(raw_id);
    match tokens.is_empty() {
        true => return,
        false => {}
    }
    let canon = tokens.join("-");
    match provider_qualified_key(provider_opt, raw_id) {
        Some(pq) => {
            map.insert(pq, spec.clone());
        }
        None => {}
    }
    // Also index explicit `<vendor>/<slug>` inside raw_id (e.g. OpenRouter `"minimax/minimax-m3.1-flash"`)
    match provider_qualified_key(None, raw_id) {
        Some(vendor_pq) => {
            map.entry(vendor_pq).or_insert_with(|| spec.clone());
        }
        None => {}
    }
    let should_replace_canon = match map.get(&canon) {
        None => true,
        Some(existing) => spec.context_window > existing.context_window,
    };
    match should_replace_canon {
        true => {
            map.insert(canon.clone(), spec.clone());
        }
        false => {}
    }

    let trimmed = trimmed_revision_tokens(&tokens);
    let trimmed_key = trimmed.join("-");
    match trimmed_key.len() >= 2 && trimmed_key != canon {
        true => {
            map.entry(trimmed_key).or_insert_with(|| spec.clone());
        }
        false => {}
    }

    match param_stripped_tokens(&trimmed) {
        Some(param_free_key) => {
            map.entry(param_free_key).or_insert_with(|| spec.clone());
        }
        None => {}
    }

    // Index alphanumeric major-generation root (e.g. `minimax-m3`, `minimax-m2`, `deepseek-v3`)
    match tokens.len() >= 3 && is_alnum_generation_token(&tokens[1]) {
        true => {
            let gen_root = format!("{}-{}", tokens[0], tokens[1]);
            map.entry(gen_root).or_insert(spec);
        }
        false => {}
    }
}

fn lookup_in_spec_map(
    map: &HashMap<String, ModelSpec>,
    provider_opt: Option<&str>,
    model: &str,
) -> Option<ModelSpec> {
    match provider_qualified_key(provider_opt, model).and_then(|pq| map.get(&pq).cloned()) {
        Some(found) => return Some(found),
        None => {}
    }
    match provider_qualified_key(None, model).and_then(|pq| map.get(&pq).cloned()) {
        Some(found) => return Some(found),
        None => {}
    }

    let tokens = tokenize_model_slug(model);
    match tokens.is_empty() {
        true => return None,
        false => {}
    }

    let full_key = tokens.join("-");
    match map.get(&full_key) {
        Some(found) => return Some(found.clone()),
        None => {}
    }

    let trimmed = trimmed_revision_tokens(&tokens);
    let trimmed_key = trimmed.join("-");
    match map.get(&trimmed_key) {
        Some(found) => return Some(found.clone()),
        None => {}
    }

    match param_stripped_tokens(&trimmed).and_then(|k| map.get(&k).cloned()) {
        Some(found) => return Some(found),
        None => {}
    }

    // Longest-prefix hierarchical lookup down to length >= 2 (family + major version),
    // stopping before dropping a pure numeric minor version digit onto an incompatible base.
    let mut prefix_len = tokens.len().saturating_sub(1);
    while prefix_len >= 2 {
        let dropped_tok = &tokens[prefix_len];
        let dropped_is_minor_digit =
            dropped_tok.len() == 1 && dropped_tok.bytes().all(|b| b.is_ascii_digit());
        let prev_is_pure_digit = tokens[prefix_len - 1]
            .bytes()
            .all(|b| b.is_ascii_digit());
        // Prevent `llama-3-1` (`["llama", "3", "1"]`) from collapsing onto `llama-3` (`["llama", "3"]`),
        // while allowing `minimax-m3-1-flash` -> `minimax-m3-1` -> `minimax-m3` (`prev` is `"m3"`).
        match dropped_is_minor_digit && prev_is_pure_digit {
            true => break,
            false => {}
        }
        let candidate_key = tokens[..prefix_len].join("-");
        match map.get(&candidate_key) {
            Some(found) => return Some(found.clone()),
            None => {}
        }
        prefix_len -= 1;
    }

    None
}

// ─── JSON Schema Ingesters (models.dev, OpenRouter, /v1/models, Ollama) ─────

fn json_usize(val: Option<&Value>) -> Option<usize> {
    val.and_then(|v| {
        v.as_u64()
            .map(|n| n as usize)
            .or_else(|| v.as_f64().filter(|&f| f > 0.0).map(|f| f.round() as usize))
            .or_else(|| v.as_str().and_then(|s| s.trim().parse::<usize>().ok()))
    })
    .filter(|&n| n >= 512)
}

fn array_contains_ci(val: Option<&Value>, needle: &str) -> bool {
    val.and_then(Value::as_array).is_some_and(|arr| {
        arr.iter()
            .filter_map(Value::as_str)
            .any(|item| item.eq_ignore_ascii_case(needle))
    })
}

fn parse_models_dev_entry(model_id: &str, obj: &serde_json::Map<String, Value>) -> Option<ModelSpec> {
    let limit = obj.get("limit").and_then(Value::as_object);
    let context_window = json_usize(limit.and_then(|l| l.get("context")))
        .or_else(|| json_usize(limit.and_then(|l| l.get("input"))))
        .or_else(|| json_usize(obj.get("context_length")))?;
    let max_output_tokens = json_usize(limit.and_then(|l| l.get("output")))
        .unwrap_or_else(|| FALLBACK_MAX_OUTPUT_TOKENS.min(context_window / 2).max(2_048));

    let modalities_in = obj
        .get("modalities")
        .and_then(Value::as_object)
        .and_then(|m| m.get("input"));
    let supports_vision = array_contains_ci(modalities_in, "image")
        || array_contains_ci(modalities_in, "video")
        || obj
            .get("attachment")
            .and_then(Value::as_bool)
            .unwrap_or(false);
    let supports_tools = obj
        .get("tool_call")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let supports_reasoning = obj
        .get("reasoning")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    Some(ModelSpec {
        context_window,
        max_output_tokens,
        supports_vision,
        supports_tools,
        supports_reasoning,
        parameter_billions: parse_parameter_billions(model_id),
        source: "models_dev".to_string(),
    })
}

fn parse_provider_or_openrouter_entry(
    model_id: &str,
    obj: &serde_json::Map<String, Value>,
    source_tag: &str,
) -> Option<ModelSpec> {
    let top_prov = obj.get("top_provider").and_then(Value::as_object);
    let context_window = json_usize(obj.get("loaded_context_length"))
        .or_else(|| json_usize(obj.get("context_length")))
        .or_else(|| json_usize(obj.get("max_context_length")))
        .or_else(|| json_usize(obj.get("context_window")))
        .or_else(|| json_usize(obj.get("max_model_len")))
        .or_else(|| json_usize(obj.get("inputTokenLimit")))
        .or_else(|| json_usize(obj.get("max_input_tokens")))
        .or_else(|| json_usize(top_prov.and_then(|tp| tp.get("context_length"))))?;

    let max_output_tokens = json_usize(top_prov.and_then(|tp| tp.get("max_completion_tokens")))
        .or_else(|| json_usize(obj.get("max_completion_tokens")))
        .or_else(|| json_usize(obj.get("max_output_tokens")))
        .or_else(|| json_usize(obj.get("outputTokenLimit")))
        .or_else(|| json_usize(obj.get("max_tokens")))
        .unwrap_or_else(|| FALLBACK_MAX_OUTPUT_TOKENS.min(context_window / 2).max(2_048));

    let arch = obj.get("architecture").and_then(Value::as_object);
    let caps = obj.get("capabilities").and_then(Value::as_object);
    let modality_str = arch
        .and_then(|a| a.get("modality"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let supported_params = obj.get("supported_parameters");

    let grammar = infer_spec_from_slug_grammar(model_id);
    let supports_vision = array_contains_ci(arch.and_then(|a| a.get("input_modalities")), "image")
        || modality_str.to_ascii_lowercase().contains("image")
        || caps
            .and_then(|c| c.get("vision"))
            .and_then(Value::as_bool)
            .unwrap_or(false)
        || array_contains_ci(obj.get("capabilities"), "vision")
        || grammar.supports_vision;

    let supports_tools = caps
        .and_then(|c| c.get("function_calling").or_else(|| c.get("tool_choice")))
        .and_then(Value::as_bool)
        .or_else(|| {
            supported_params
                .and_then(Value::as_array)
                .map(|_| array_contains_ci(supported_params, "tools"))
        })
        .unwrap_or(true);

    let supports_reasoning = array_contains_ci(supported_params, "reasoning")
        || array_contains_ci(supported_params, "include_reasoning")
        || caps
            .and_then(|c| c.get("reasoning").or_else(|| c.get("thinking")))
            .and_then(Value::as_bool)
            .unwrap_or(false)
        || grammar.supports_reasoning;

    Some(ModelSpec {
        context_window,
        max_output_tokens,
        supports_vision,
        supports_tools,
        supports_reasoning,
        parameter_billions: parse_parameter_billions(model_id),
        source: source_tag.to_string(),
    })
}

fn parse_ollama_num_ctx_from_parameters(params_str: &str) -> Option<usize> {
    params_str.lines().find_map(|line| {
        let mut parts = line.split_whitespace();
        let key = parts.next()?;
        let val = parts.next()?;
        key.eq_ignore_ascii_case("num_ctx")
            .then(|| val.parse::<usize>().ok())
            .flatten()
            .filter(|&n| n >= 512)
    })
}

// ─── Character-Class BPE Token Estimator (Zero-Regex, Single-Pass) ──────────

pub fn estimate_text_tokens(text: &str) -> usize {
    match text.is_empty() {
        true => return 0,
        false => {}
    }
    let mut quarter_tokens: usize = 0;
    let mut alnum_run: usize = 0;

    let flush_run = |run_len: usize| -> usize {
        match (run_len == 0, run_len <= 4, run_len >= 64) {
            (true, _, _) => 0,
            (false, true, _) => 4,
            (false, false, true) => ((run_len + 2) / 3) * 4,
            (false, false, false) => ((run_len * 10 + 41) / 42) * 4,
        }
    };

    for ch in text.chars() {
        let code = ch as u32;
        let is_ascii_ident = ch.is_ascii_alphanumeric() || ch == '_';
        match is_ascii_ident {
            true => {
                alnum_run += 1;
            }
            false => {
                quarter_tokens += flush_run(alnum_run);
                alnum_run = 0;
                match (ch.is_ascii_whitespace(), code > 127) {
                    (true, _) => {}
                    (false, true) => quarter_tokens += 6, // ~1.5 BPE tokens for CJK / multibyte
                    (false, false) => quarter_tokens += 3, // ~0.75 BPE tokens for code/JSON punctuation
                }
            }
        }
    }
    quarter_tokens += flush_run(alnum_run);
    quarter_tokens.div_ceil(4).max(1)
}

fn count_markdown_image_refs(text: &str) -> usize {
    text.matches("![").count()
}

// ─── Zero-Regex Context-Overflow Error Parser ───────────────────────────────

const OVERFLOW_PHRASES: &[&str] = &[
    "maximum context length",
    "context_length_exceeded",
    "context length exceeded",
    "prompt is too long",
    "too many tokens",
    "max_seq_len",
    "max_model_len",
    "maximum number of tokens",
    "exceeds the context window",
    "input length and `max_tokens` exceed context limit",
];

fn extract_plausible_token_numbers(text: &str) -> Vec<usize> {
    text.split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .filter_map(|digits| digits.parse::<usize>().ok())
        .filter(|&n| (1_024..=10_000_000).contains(&n))
        .collect()
}

fn extract_anchored_limit_from_error(error_text: &str) -> Option<usize> {
    let lower = error_text.to_ascii_lowercase();
    let words: Vec<&str> = lower
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .filter(|s| !s.is_empty())
        .collect();

    let parse_tok = |w: &str| -> Option<usize> {
        w.parse::<usize>()
            .ok()
            .filter(|&n| (1_024..=10_000_000).contains(&n))
    };

    // Pattern 1: `<N> maximum` (e.g. Anthropic: "214500 tokens > 200000 maximum")
    for pair in words.windows(2) {
        match matches!(pair[1], "maximum" | "max") {
            true => match parse_tok(pair[0]) {
                Some(n) => return Some(n),
                None => {}
            },
            false => {}
        }
    }

    // Pattern 2: Forward anchor (`context length is <N>`, `max_model_len <N>`, `context limit <N>`)
    for (idx, &w) in words.iter().enumerate() {
        let is_anchor = matches!(w, "max_model_len" | "max_seq_len")
            || (matches!(w, "length" | "limit" | "window")
                && idx > 0
                && matches!(words[idx - 1], "context" | "token" | "input"));
        match is_anchor {
            true => {
                let end = (idx + 5).min(words.len());
                for &cand in &words[idx + 1..end] {
                    match parse_tok(cand) {
                        Some(n) => return Some(n),
                        None => {}
                    }
                }
            }
            false => {}
        }
    }

    let mut large: Vec<usize> = extract_plausible_token_numbers(error_text)
        .into_iter()
        .filter(|&n| n >= 2_048)
        .collect();
    large.sort_unstable();
    large.dedup();
    large.first().copied()
}

pub struct DynamicContextRegistry;

impl DynamicContextRegistry {
    pub fn resolve_model_spec(model: &str, config: Option<&Config>) -> ModelSpec {
        let dir = crate::config::loader::config_dir();
        Self::resolve_model_spec_at(&dir, model, config)
    }

    pub fn resolve_model_spec_at(
        config_dir: &Path,
        model: &str,
        config: Option<&Config>,
    ) -> ModelSpec {
        ensure_store_loaded_for_dir(config_dir);

        let provider_hint = config
            .map(|c| c.agents.defaults.provider.as_str())
            .filter(|p| !p.is_empty() && *p != "auto");

        let mut base = Self::lookup_user_catalog_at(config_dir, model)
            .or_else(|| match registry_lock().read() {
                Ok(guard) => lookup_in_spec_map(&guard.store.provider_models, provider_hint, model)
                    .or_else(|| {
                        lookup_in_spec_map(&guard.store.public_catalog, provider_hint, model)
                    }),
                Err(_) => None,
            })
            .unwrap_or_else(|| infer_spec_from_slug_grammar(model));

        // Tier 2: Reactive runtime learned limit overrides upstream theoretical ceiling
        // (e.g. local vLLM `--max-model-len 16384` on a 128k model).
        let canon = canonical_model_key(model);
        let learned = match registry_lock().read() {
            Ok(guard) => provider_qualified_key(provider_hint, model)
                .and_then(|pq| guard.store.learned_limits.get(&pq).copied())
                .or_else(|| guard.store.learned_limits.get(&canon).copied()),
            Err(_) => None,
        };
        match learned.filter(|&lim| lim >= 512) {
            Some(lim) => {
                base.context_window = lim;
                base.max_output_tokens = base.max_output_tokens.min(lim / 2).max(512);
                base.source = "learned_runtime".to_string();
            }
            None => {}
        }

        // Tier 1: Explicit `config.agents.defaults.context_limit` always wins.
        match config.and_then(|c| c.agents.defaults.context_limit).filter(|&v| v > 0) {
            Some(explicit) => {
                base.context_window = explicit;
                base.source = "config_override".to_string();
            }
            None => {}
        }

        base
    }

    /// Resolve the true context window (in tokens) for a given model.
    pub fn resolve_context_window(model: &str, config: &Config) -> usize {
        Self::resolve_model_spec(model, Some(config)).context_window
    }

    /// Resolve the model's maximum output generation tokens.
    pub fn resolve_max_output_tokens(model: &str, config: &Config) -> usize {
        Self::resolve_model_spec(model, Some(config)).max_output_tokens
    }

    /// Resolve prompt budget (in characters) dynamically based on context window and ratio.
    pub fn resolve_prompt_budget_chars(model: &str, config: &Config) -> usize {
        match config.agents.defaults.prompt_budget_limit.filter(|&v| v > 0) {
            Some(explicit) => return explicit,
            None => {}
        }

        let window_tokens = Self::resolve_context_window(model, config);
        let ratio = config
            .agents
            .defaults
            .prompt_budget_ratio
            .unwrap_or(DEFAULT_PROMPT_BUDGET_RATIO)
            .clamp(0.05, 0.60);

        let budget = (window_tokens as f64 * ratio * CHARS_PER_TOKEN).round() as usize;
        budget.max(MIN_PROMPT_BUDGET_CHARS)
    }

    /// Resolve inline tool output limit (in characters) dynamically based on context window and ratio,
    /// bounded by `MAX_DEFAULT_INLINE_TOOL_OUTPUT_CHARS` unless explicitly overridden.
    pub fn resolve_tool_output_limit_chars(model: &str, config: &Config) -> usize {
        match config.agents.defaults.tool_output_limit.filter(|&v| v > 0) {
            Some(explicit) => return explicit,
            None => {}
        }

        let window_tokens = Self::resolve_context_window(model, config);
        let ratio = config
            .agents
            .defaults
            .tool_output_ratio
            .unwrap_or(DEFAULT_TOOL_OUTPUT_RATIO)
            .clamp(0.01, 0.30);

        let raw_limit = (window_tokens as f64 * ratio * CHARS_PER_TOKEN).round() as usize;
        let capped = match config.agents.defaults.tool_output_ratio.is_some() {
            true => raw_limit,
            false => raw_limit.min(MAX_DEFAULT_INLINE_TOOL_OUTPUT_CHARS),
        };
        capped.max(MIN_TOOL_OUTPUT_CHARS)
    }

    /// Estimate tokens from raw character count (legacy helper).
    pub fn estimate_tokens_from_chars(char_count: usize) -> usize {
        (char_count as f64 / CHARS_PER_TOKEN).ceil() as usize
    }

    /// Estimate tokens for mounted OpenAI/Anthropic JSON tool definitions.
    pub fn estimate_tools_tokens(tools: &[Value]) -> usize {
        tools
            .iter()
            .map(|t| estimate_text_tokens(&t.to_string()) + 8)
            .sum()
    }

    /// Estimate total tokens across conversation messages and system prompt using character-class BPE heuristics.
    pub fn estimate_total_tokens(messages: &[Message], system_prompt: &str) -> usize {
        Self::estimate_request_tokens(messages, system_prompt, &[])
    }

    /// Full request token estimate: `system_prompt` + `messages` (including tool_calls & images) + `tools`.
    pub fn estimate_request_tokens(
        messages: &[Message],
        system_prompt: &str,
        tools: &[Value],
    ) -> usize {
        let mut total = estimate_text_tokens(system_prompt) + Self::estimate_tools_tokens(tools);
        for msg in messages {
            total += 4 + estimate_text_tokens(&msg.content);
            total += count_markdown_image_refs(&msg.content) * 850;
            for (k, v) in &msg.extra {
                total += estimate_text_tokens(k);
                total += match v.as_str() {
                    Some(s) => estimate_text_tokens(s),
                    None => estimate_text_tokens(&v.to_string()),
                };
            }
        }
        total
    }

    /// Estimate tokens of a single `Message`.
    pub fn estimate_message_tokens(msg: &Message) -> usize {
        let mut total = 4 + estimate_text_tokens(&msg.content);
        total += count_markdown_image_refs(&msg.content) * 850;
        for (k, v) in &msg.extra {
            total += estimate_text_tokens(k);
            total += match v.as_str() {
                Some(s) => estimate_text_tokens(s),
                None => estimate_text_tokens(&v.to_string()),
            };
        }
        total
    }

    /// Determine whether estimated request tokens have reached the compaction threshold.
    pub fn should_compact_context(model: &str, estimated_tokens: usize, config: &Config) -> bool {
        let window_tokens = Self::resolve_context_window(model, config);
        let ratio = config
            .agents
            .defaults
            .compaction_threshold_ratio
            .unwrap_or(DEFAULT_COMPACTION_THRESHOLD_RATIO)
            .clamp(0.30, 0.95);

        let threshold = (window_tokens as f64 * ratio).round() as usize;
        estimated_tokens >= threshold
    }

    /// Calculate recent message count fallback when token sizes are uniform.
    pub fn resolve_keep_recent_count(total_msgs: usize, config: &Config) -> usize {
        let ratio = config
            .agents
            .defaults
            .keep_recent_ratio
            .unwrap_or(DEFAULT_KEEP_RECENT_RATIO)
            .clamp(0.05, 0.50);

        let count = (total_msgs as f64 * ratio).round() as usize;
        count.max(4)
    }

    /// Calculate token budget reserved for verbatim recent turns during compaction.
    pub fn resolve_keep_recent_budget_tokens(model: &str, config: &Config) -> usize {
        let window_tokens = Self::resolve_context_window(model, config);
        let ratio = config
            .agents
            .defaults
            .keep_recent_ratio
            .unwrap_or(DEFAULT_KEEP_RECENT_RATIO)
            .clamp(0.05, 0.50);
        ((window_tokens as f64) * ratio).round().max(1_024.0) as usize
    }

    /// Token-budget sliding-window split index aligned to a `"user"` message boundary so
    /// assistant `tool_calls` and subsequent `"tool"` results are never orphaned.
    pub fn resolve_compaction_split_by_tokens(
        messages: &[Message],
        model: &str,
        config: &Config,
    ) -> usize {
        let len = messages.len();
        match len <= 2 {
            true => return 0,
            false => {}
        }
        let budget = Self::resolve_keep_recent_budget_tokens(model, config);
        let max_keep_msgs = Self::resolve_keep_recent_count(len, config).max(6);

        let mut kept_tokens = 0usize;
        let mut kept_msgs = 0usize;
        let mut candidate_split = len.saturating_sub(1);

        let mut idx = len;
        while idx > 1 {
            idx -= 1;
            let msg_tokens = Self::estimate_message_tokens(&messages[idx]);
            let exceeds = (kept_tokens + msg_tokens > budget && kept_msgs >= 2)
                || kept_msgs >= max_keep_msgs;
            match exceeds {
                true => break,
                false => {
                    kept_tokens += msg_tokens;
                    kept_msgs += 1;
                    match messages[idx].role == "user" {
                        true => candidate_split = idx,
                        false => {}
                    }
                }
            }
        }

        match messages.get(candidate_split).is_some_and(|m| m.role == "user") && candidate_split > 0
        {
            true => candidate_split,
            false => (1..len)
                .rev()
                .find(|&i| messages[i].role == "user")
                .unwrap_or(len / 2),
        }
    }

    pub fn is_known_in_live_catalog(model: &str) -> bool {
        let dir = crate::config::loader::config_dir();
        ensure_store_loaded_for_dir(&dir);
        match registry_lock().read() {
            Ok(guard) => {
                lookup_in_spec_map(&guard.store.provider_models, None, model).is_some()
                    || lookup_in_spec_map(&guard.store.public_catalog, None, model).is_some()
            }
            Err(_) => false,
        }
    }

    pub fn is_context_overflow_error(error_text: &str) -> bool {
        let lower = error_text.to_ascii_lowercase();
        OVERFLOW_PHRASES.iter().any(|phrase| lower.contains(phrase))
    }

    /// Parse provider HTTP 400/413/422 context-overflow error bodies without regex,
    /// persist the discovered limit in `model_specs_cache.json`, and return it.
    pub fn learn_context_limit_from_error(model: &str, error_text: &str) -> Option<usize> {
        let dir = crate::config::loader::config_dir();
        Self::learn_context_limit_from_error_at(&dir, model, error_text)
    }

    pub fn learn_context_limit_from_error_at(
        config_dir: &Path,
        model: &str,
        error_text: &str,
    ) -> Option<usize> {
        match Self::is_context_overflow_error(error_text) {
            false => return None,
            true => {}
        }
        let learned = extract_anchored_limit_from_error(error_text)?;

        ensure_store_loaded_for_dir(config_dir);
        let canon = canonical_model_key(model);
        match (!canon.is_empty(), registry_lock().write()) {
            (true, Ok(mut guard)) => {
                guard.store.learned_limits.insert(canon, learned);
            }
            _ => {}
        }
        persist_store_for_dir(config_dir);
        Some(learned)
    }

    // ─── Live Payload Ingesters ──────────────────────────────────────────────

    pub fn ingest_models_dev_json_at(config_dir: &Path, payload: &Value) -> usize {
        ensure_store_loaded_for_dir(config_dir);
        let root = match payload.as_object() {
            Some(obj) => obj,
            None => return 0,
        };
        let mut count = 0usize;
        match registry_lock().write() {
            Ok(mut guard) => {
                for (provider_id, prov_val) in root {
                    let models_obj = prov_val
                        .as_object()
                        .and_then(|p| p.get("models"))
                        .and_then(Value::as_object);
                    match models_obj {
                        None => {}
                        Some(models) => {
                            for (model_id, model_val) in models {
                                match model_val
                                    .as_object()
                                    .and_then(|m| parse_models_dev_entry(model_id, m))
                                {
                                    Some(spec) => {
                                        insert_spec_into_map(
                                            &mut guard.store.public_catalog,
                                            Some(provider_id),
                                            model_id,
                                            spec,
                                        );
                                        count += 1;
                                    }
                                    None => {}
                                }
                            }
                        }
                    }
                }
                match count > 0 {
                    true => guard.store.updated_at_epoch_secs = now_epoch_secs(),
                    false => {}
                }
            }
            Err(_) => return 0,
        }
        match count > 0 {
            true => persist_store_for_dir(config_dir),
            false => {}
        }
        count
    }

    pub fn ingest_openrouter_models_json_at(config_dir: &Path, payload: &Value) -> usize {
        ensure_store_loaded_for_dir(config_dir);
        let arr = match payload.get("data").and_then(Value::as_array) {
            Some(a) => a,
            None => return 0,
        };
        let mut count = 0usize;
        match registry_lock().write() {
            Ok(mut guard) => {
                for item in arr {
                    let obj = match item.as_object() {
                        Some(o) => o,
                        None => continue,
                    };
                    let id = match obj.get("id").and_then(Value::as_str) {
                        Some(s) => s,
                        None => continue,
                    };
                    match parse_provider_or_openrouter_entry(id, obj, "openrouter_catalog") {
                        Some(spec) => {
                            insert_spec_into_map(
                                &mut guard.store.public_catalog,
                                Some("openrouter"),
                                id,
                                spec,
                            );
                            count += 1;
                        }
                        None => {}
                    }
                }
                match count > 0 {
                    true => guard.store.updated_at_epoch_secs = now_epoch_secs(),
                    false => {}
                }
            }
            Err(_) => return 0,
        }
        match count > 0 {
            true => persist_store_for_dir(config_dir),
            false => {}
        }
        count
    }

    pub fn ingest_provider_models_json(provider_name: &str, payload: &Value) -> usize {
        let dir = crate::config::loader::config_dir();
        Self::ingest_provider_models_json_at(&dir, provider_name, payload)
    }

    pub fn ingest_provider_models_json_at(
        config_dir: &Path,
        provider_name: &str,
        payload: &Value,
    ) -> usize {
        ensure_store_loaded_for_dir(config_dir);
        let items = payload
            .get("data")
            .and_then(Value::as_array)
            .or_else(|| payload.get("models").and_then(Value::as_array));
        let arr = match items {
            Some(a) => a,
            None => return 0,
        };
        let mut count = 0usize;
        match registry_lock().write() {
            Ok(mut guard) => {
                for item in arr {
                    let obj = match item.as_object() {
                        Some(o) => o,
                        None => continue,
                    };
                    let raw_id = obj
                        .get("id")
                        .or_else(|| obj.get("name"))
                        .and_then(Value::as_str)
                        .map(|s| s.strip_prefix("models/").unwrap_or(s));
                    let id = match raw_id {
                        Some(s) => s,
                        None => continue,
                    };
                    match parse_provider_or_openrouter_entry(id, obj, "provider_endpoint") {
                        Some(spec) => {
                            insert_spec_into_map(
                                &mut guard.store.provider_models,
                                Some(provider_name),
                                id,
                                spec,
                            );
                            count += 1;
                        }
                        None => {}
                    }
                }
            }
            Err(_) => return 0,
        }
        match count > 0 {
            true => persist_store_for_dir(config_dir),
            false => {}
        }
        count
    }

    pub fn ingest_ollama_show_json_at(
        config_dir: &Path,
        model: &str,
        payload: &Value,
    ) -> Option<ModelSpec> {
        ensure_store_loaded_for_dir(config_dir);
        let obj = payload.as_object()?;
        let num_ctx = obj
            .get("parameters")
            .and_then(Value::as_str)
            .and_then(parse_ollama_num_ctx_from_parameters);
        let info_ctx = obj
            .get("model_info")
            .and_then(Value::as_object)
            .and_then(|info| {
                info.iter().find_map(|(k, v)| {
                    k.ends_with(".context_length")
                        .then(|| json_usize(Some(v)))
                        .flatten()
                })
            });
        let context_window = num_ctx.or(info_ctx)?;

        let caps = obj.get("capabilities");
        let grammar = infer_spec_from_slug_grammar(model);
        let supports_vision = array_contains_ci(caps, "vision") || grammar.supports_vision;
        let supports_tools = match caps.and_then(Value::as_array) {
            Some(_) => array_contains_ci(caps, "tools") || array_contains_ci(caps, "completion"),
            None => true,
        };
        let supports_reasoning = array_contains_ci(caps, "thinking") || grammar.supports_reasoning;
        let param_b = obj
            .get("details")
            .and_then(|d| d.get("parameter_size"))
            .and_then(Value::as_str)
            .and_then(parse_single_param_token_b)
            .or(grammar.parameter_billions);

        let spec = ModelSpec {
            context_window,
            max_output_tokens: FALLBACK_MAX_OUTPUT_TOKENS.min(context_window / 2).max(1_024),
            supports_vision,
            supports_tools,
            supports_reasoning,
            parameter_billions: param_b,
            source: "ollama_show".to_string(),
        };

        match registry_lock().write() {
            Ok(mut guard) => {
                insert_spec_into_map(
                    &mut guard.store.provider_models,
                    Some("ollama"),
                    model,
                    spec.clone(),
                );
            }
            Err(_) => {}
        }
        persist_store_for_dir(config_dir);
        Some(spec)
    }

    // ─── Asynchronous Multi-Source Sync ──────────────────────────────────────

    pub async fn ensure_active_model_synced(config: &Config) {
        let dir = crate::config::loader::config_dir();
        ensure_store_loaded_for_dir(&dir);

        let model = config.agents.defaults.model.trim().to_string();
        let provider = config.agents.defaults.provider.trim().to_string();
        let (cache_empty, cache_stale, model_cached) = match registry_lock().read() {
            Ok(guard) => {
                let empty = guard.store.public_catalog.is_empty();
                let age = now_epoch_secs().saturating_sub(guard.store.updated_at_epoch_secs);
                let cached =
                    lookup_in_spec_map(&guard.store.provider_models, Some(&provider), &model)
                        .is_some()
                        || lookup_in_spec_map(&guard.store.public_catalog, Some(&provider), &model)
                            .is_some();
                (empty, age >= CATALOG_CACHE_TTL_SECS, cached)
            }
            Err(_) => (true, true, false),
        };

        let is_ollama = provider.contains("ollama") || model.to_ascii_lowercase().starts_with("ollama");
        match is_ollama && !model_cached {
            true => {
                Self::probe_ollama_show(&dir, config, &model).await;
            }
            false => {}
        }

        match !cache_empty && !cache_stale && model_cached {
            true => return,
            false => {}
        }

        let already_syncing =
            SYNC_IN_FLIGHT.swap(true, std::sync::atomic::Ordering::AcqRel);
        match already_syncing {
            true => return,
            false => {}
        }

        let cfg_clone = config.clone();
        match !model_cached {
            // Cold miss for active model: bound wait to 2.5s so the very first turn already has accurate 1M/200k limits
            true => {
                let _ = tokio::time::timeout(
                    Duration::from_millis(2_500),
                    Self::refresh_remote_catalogs(&dir, &cfg_clone),
                )
                .await;
                SYNC_IN_FLIGHT.store(false, std::sync::atomic::Ordering::Release);
            }
            // Warm hit with stale TTL: refresh quietly in background
            false => {
                tokio::spawn(async move {
                    Self::refresh_remote_catalogs(&dir, &cfg_clone).await;
                    SYNC_IN_FLIGHT.store(false, std::sync::atomic::Ordering::Release);
                });
            }
        }
    }

    async fn probe_ollama_show(config_dir: &Path, config: &Config, model: &str) {
        let (_, api_base) = config.resolve_provider_config("ollama");
        let root = api_base.trim_end_matches('/').trim_end_matches("/v1");
        let show_url = format!("{root}/api/show");
        let bare = bare_model_slice(model);
        let client = crate::core::http::http_client_with_timeout(Duration::from_millis(1_500));
        let resp = client
            .post(&show_url)
            .json(&serde_json::json!({ "model": bare }))
            .send()
            .await;
        match resp {
            Ok(r) => match r.status().is_success() {
                true => match r.json::<Value>().await {
                    Ok(json) => {
                        let _ = Self::ingest_ollama_show_json_at(config_dir, bare, &json);
                    }
                    Err(_) => {}
                },
                false => {}
            },
            Err(_) => {}
        }
    }

    async fn refresh_remote_catalogs(config_dir: &Path, config: &Config) {
        let client = crate::core::http::http_client_with_timeout(Duration::from_secs(4));
        let c1 = client.clone();
        let c2 = client.clone();

        let models_dev_fut = async move {
            let resp = c1.get(MODELS_DEV_API_URL).send().await.ok()?;
            resp.status().is_success().then_some(())?;
            resp.json::<Value>().await.ok()
        };
        let openrouter_fut = async move {
            let resp = c2.get(OPENROUTER_MODELS_API_URL).send().await.ok()?;
            resp.status().is_success().then_some(())?;
            resp.json::<Value>().await.ok()
        };

        let (md_json, or_json) = tokio::join!(models_dev_fut, openrouter_fut);
        match or_json {
            Some(ref val) => {
                let _ = Self::ingest_openrouter_models_json_at(config_dir, val);
            }
            None => {}
        }
        match md_json {
            Some(ref val) => {
                let _ = Self::ingest_models_dev_json_at(config_dir, val);
            }
            None => {}
        }

        // Also probe active provider's own /models endpoint (e.g. LM Studio, Mistral, custom gateway)
        let prov = config.agents.defaults.provider.trim();
        let eff_prov = match prov.is_empty() || prov == "auto" {
            true => None,
            false => Some(prov),
        };
        match eff_prov {
            Some(p) => {
                let _ = crate::channels::fetch_provider_models(p, config).await;
            }
            None => {}
        }
    }

    // ─── User `~/.openz/models.json` Loader (Supports Flat & Rich Schemas) ──

    fn lookup_user_catalog_at(config_dir: &Path, model: &str) -> Option<ModelSpec> {
        let catalog = Self::load_user_catalog_at(config_dir)?;
        lookup_in_spec_map(&catalog.models, None, model).or_else(|| {
            let norm = model.trim().to_ascii_lowercase();
            catalog.patterns.iter().find_map(|(pat, &limit)| {
                let pat_norm = pat.trim().to_ascii_lowercase();
                (!pat_norm.is_empty() && norm.contains(&pat_norm) && limit >= 512).then(|| {
                    let mut spec = infer_spec_from_slug_grammar(model);
                    spec.context_window = limit;
                    spec.source = "user_catalog".to_string();
                    spec
                })
            })
        })
    }

    fn load_user_catalog_at(config_dir: &Path) -> Option<ModelsCatalogFile> {
        let path = models_catalog_path_in(config_dir);
        path.is_file().then_some(())?;
        let raw = std::fs::read_to_string(&path).ok()?;
        let val: Value = serde_json::from_str(&raw).ok()?;
        let root = val.as_object()?;

        let mut out = ModelsCatalogFile::default();

        let parse_entry_val = |key: &str, v: &Value| -> Option<ModelSpec> {
            match v.as_u64() {
                Some(limit) => (limit >= 512).then(|| {
                    let mut s = infer_spec_from_slug_grammar(key);
                    s.context_window = limit as usize;
                    s.source = "user_catalog".to_string();
                    s
                }),
                None => {
                    let entry: UserModelEntry = serde_json::from_value(v.clone()).ok()?;
                    let ctx = entry.context_window.filter(|&n| n >= 512)?;
                    let base = infer_spec_from_slug_grammar(key);
                    Some(ModelSpec {
                        context_window: ctx,
                        max_output_tokens: entry
                            .max_output_tokens
                            .filter(|&n| n >= 256)
                            .unwrap_or(base.max_output_tokens),
                        supports_vision: entry.supports_vision.unwrap_or(base.supports_vision),
                        supports_tools: entry.supports_tools.unwrap_or(base.supports_tools),
                        supports_reasoning: entry
                            .supports_reasoning
                            .unwrap_or(base.supports_reasoning),
                        parameter_billions: base.parameter_billions,
                        source: "user_catalog".to_string(),
                    })
                }
            }
        };

        match root.get("models").and_then(Value::as_object) {
            Some(models_map) => {
                for (k, v) in models_map {
                    match parse_entry_val(k, v) {
                        Some(spec) => insert_spec_into_map(&mut out.models, None, k, spec),
                        None => {}
                    }
                }
            }
            None => {}
        }

        match root.get("patterns").and_then(Value::as_object) {
            Some(pat_map) => {
                for (k, v) in pat_map {
                    match json_usize(Some(v)) {
                        Some(limit) => {
                            out.patterns.insert(k.to_ascii_lowercase(), limit);
                        }
                        None => {}
                    }
                }
            }
            None => {}
        }

        // Support flat `{ "model-name": 200000 }` or `{ "model-name": { "context_window": ... } }`
        match out.models.is_empty() && out.patterns.is_empty() {
            true => {
                for (k, v) in root {
                    match parse_entry_val(k, v) {
                        Some(spec) => insert_spec_into_map(&mut out.models, None, k, spec),
                        None => {}
                    }
                }
            }
            false => {}
        }

        (!out.models.is_empty() || !out.patterns.is_empty()).then_some(out)
    }
}
