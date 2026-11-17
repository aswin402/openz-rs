//! Native `opencaveman` tool bridging the `opencaveman` workspace crate into OpenZ's
//! tool execution pipeline, skills, workflows, backups, and telemetry.

use crate::config::schema::Config;
use crate::providers::{GenerationSettings, LLMProvider};
use crate::session::Message;
use crate::tools::Tool;
use anyhow::Result;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct OpenCavemanTool {
    pub provider: Arc<dyn LLMProvider>,
    pub config: Config,
}

fn first_str<'a>(args: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| args.get(*key).and_then(Value::as_str))
        .map(str::trim)
        .filter(|val| !val.is_empty())
}

fn first_bool(args: &Value, keys: &[&str]) -> Option<bool> {
    keys.iter().find_map(|key| match args.get(*key) {
        Some(Value::Bool(b)) => Some(*b),
        Some(Value::String(s)) => match s.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "on" | "yes" | "enable" | "enabled" => Some(true),
            "false" | "0" | "off" | "no" | "disable" | "disabled" => Some(false),
            _ => None,
        },
        _ => None,
    })
}

fn resolve_target_path(raw_path: &str) -> PathBuf {
    let candidate = Path::new(raw_path);
    match candidate.is_absolute() {
        true => candidate.to_path_buf(),
        false => crate::tools::subagent::workspace::current_workspace_root().join(candidate),
    }
}

fn backup_root_dir() -> PathBuf {
    crate::config::runtime_data_dir()
        .join("backups")
        .join("opencaveman")
}

async fn request_llm_markdown_rewrite(
    provider: &Arc<dyn LLMProvider>,
    config: &Config,
    source: &str,
    level: opencaveman::CavemanLevel,
) -> Option<String> {
    let descriptor = level.descriptor();
    let system_prompt = format!(
        "Compress technical Markdown prose at caveman '{}' intensity ({}). \
         STRICT INVARIANTS: keep every YAML frontmatter block, fenced code block, inline backtick span, URL, file path, Markdown heading level, and table byte-identical. \
         Compress ONLY natural-language sentences between structural anchors. Output ONLY the compressed Markdown document with no wrapper commentary.",
        descriptor.id, descriptor.description
    );
    let user_msg = Message {
        role: "user".to_string(),
        content: source.to_string(),
        timestamp: None,
        extra: serde_json::Map::new(),
    };
    let settings = GenerationSettings {
        temperature: 0.0,
        max_tokens: config.agents.defaults.max_tokens.max(4096),
        reasoning_effort: None,
    };
    provider
        .chat(&system_prompt, &[user_msg], &[], &settings)
        .await
        .ok()
        .and_then(|resp| resp.content)
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

#[async_trait::async_trait]
impl Tool for OpenCavemanTool {
    fn name(&self) -> &str {
        "opencaveman"
    }

    fn description(&self) -> &str {
        "Native OpenCaveman engine for 'caveman' terse mode management, AST-verified Markdown/memory compression with out-of-tree backups, terse Conventional Commit formatting, single-line code review formatting, Auto-Clarity diagnostics, and token-savings telemetry."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": [
                        "status",
                        "levels",
                        "set_mode",
                        "compress_file",
                        "compress_text",
                        "list_backups",
                        "restore_backup",
                        "commit_message",
                        "code_review",
                        "auto_clarity",
                        "stats",
                        "reset_stats"
                    ],
                    "description": "OpenCaveman operation to execute."
                },
                "enabled": {
                    "type": "boolean",
                    "description": "Enable or disable 'caveman' mode (for action='set_mode')."
                },
                "level": {
                    "type": "string",
                    "enum": [
                        "lite",
                        "full",
                        "ultra",
                        "symbolic_lite",
                        "symbolic_full",
                        "symbolic_ultra"
                    ],
                    "description": "Intensity tier for 'caveman' mode or compression."
                },
                "surface": {
                    "type": "string",
                    "enum": ["global", "tui", "webui"],
                    "description": "Optional UI surface profile to update alongside global defaults (for action='set_mode')."
                },
                "path": {
                    "type": "string",
                    "description": "Target Markdown/text file path (for action='compress_file' or 'restore_backup')."
                },
                "backup_id": {
                    "type": "string",
                    "description": "Backup identifier or original path (for action='restore_backup')."
                },
                "dry_run": {
                    "type": "boolean",
                    "description": "Preview AST-safe compression metrics without modifying the file on disk."
                },
                "use_llm": {
                    "type": "boolean",
                    "description": "Attempt an LLM prose rewrite verified by pulldown-cmark AST anchors (falls back to deterministic AST compression on any drift)."
                },
                "text": {
                    "type": "string",
                    "description": "Inline Markdown/prose text (for action='compress_text' or 'auto_clarity')."
                },
                "diff": {
                    "type": "string",
                    "description": "Unified git diff text (for action='commit_message' or 'code_review')."
                },
                "commit_type": {
                    "type": "string",
                    "description": "Conventional Commit type such as feat, fix, refactor, perf, docs, test, chore (for action='commit_message')."
                },
                "scope": {
                    "type": "string",
                    "description": "Optional Conventional Commit scope (for action='commit_message')."
                },
                "summary": {
                    "type": "string",
                    "description": "Imperative commit summary (for action='commit_message')."
                },
                "bullets": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional why/impact bullet lines (for action='commit_message')."
                },
                "findings": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "file": { "type": "string" },
                            "line": { "type": "integer" },
                            "severity": { "type": "string", "enum": ["bug", "security", "perf", "risk", "nit"] },
                            "message": { "type": "string" },
                            "problem": { "type": "string" },
                            "fix": { "type": "string" }
                        },
                        "required": ["line", "severity"]
                    },
                    "description": "Structured review findings to normalize into caveman format (for action='code_review')."
                }
            }
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let raw_action = first_str(arguments, &["action", "command", "op"]).unwrap_or("status");
        let action = match raw_action.to_ascii_lowercase().as_str() {
            "status" | "info" | "get" => "status",
            "levels" | "tiers" | "modes" => "levels",
            "set_mode" | "set" | "configure" | "toggle" => "set_mode",
            "compress_file" | "compress" => "compress_file",
            "compress_text" | "compact_text" => "compress_text",
            "list_backups" | "backups" => "list_backups",
            "restore_backup" | "restore" => "restore_backup",
            "commit_message" | "commit" | "caveman_commit" => "commit_message",
            "code_review" | "review" | "caveman_review" => "code_review",
            "auto_clarity" | "clarity" => "auto_clarity",
            "stats" | "caveman_stats" | "telemetry" => "stats",
            "reset_stats" | "clear_stats" => "reset_stats",
            other => anyhow::bail!("Unsupported opencaveman action '{other}'"),
        };

        let live_cfg = crate::config::loader::load_config().unwrap_or_else(|_| self.config.clone());
        let data_dir = crate::config::runtime_data_dir();

        match action {
            "status" => {
                let level = opencaveman::CavemanLevel::from_config_str(
                    &live_cfg.agents.defaults.caveman_level,
                );
                let stats = opencaveman::load_stats_ledger(&data_dir);
                Ok(serde_json::json!({
                    "mode": "caveman",
                    "engine": "opencaveman",
                    "enabled": live_cfg.agents.defaults.caveman_mode,
                    "level": level.slug(),
                    "active_descriptor": level.descriptor(),
                    "available_levels": opencaveman::all_level_descriptors(),
                    "compact_badge": stats.compact_lifetime_badge,
                    "stats": stats,
                }))
            }
            "levels" => Ok(serde_json::json!({
                "mode": "caveman",
                "engine": "opencaveman",
                "levels": opencaveman::all_level_descriptors(),
            })),
            "set_mode" => {
                let mut cfg = crate::config::loader::load_config()?;
                let enabled_opt = first_bool(arguments, &["enabled", "caveman_mode", "active"]);
                let level_opt = first_str(arguments, &["level", "caveman_level", "intensity"])
                    .map(|raw| opencaveman::normalize_level_slug(raw).to_string());
                match enabled_opt {
                    Some(enabled) => cfg.agents.defaults.caveman_mode = enabled,
                    None => {}
                }
                match &level_opt {
                    Some(level_slug) => cfg.agents.defaults.caveman_level = level_slug.clone(),
                    None => {}
                }
                let surface = first_str(arguments, &["surface"]).unwrap_or("global");
                match matches!(surface, "tui" | "webui") {
                    true => {
                        let profile = cfg.ui_profile_mut(surface);
                        match enabled_opt {
                            Some(enabled) => profile.caveman_mode = Some(enabled),
                            None => {}
                        }
                        match &level_opt {
                            Some(level_slug) => profile.caveman_level = Some(level_slug.clone()),
                            None => {}
                        }
                    }
                    false => {
                        for surf in ["tui", "webui"] {
                            match cfg.ui_profile(surf).is_some() {
                                true => {
                                    let profile = cfg.ui_profile_mut(surf);
                                    match enabled_opt {
                                        Some(enabled) => profile.caveman_mode = Some(enabled),
                                        None => {}
                                    }
                                    match &level_opt {
                                        Some(level_slug) => {
                                            profile.caveman_level = Some(level_slug.clone());
                                        }
                                        None => {}
                                    }
                                }
                                false => {}
                            }
                        }
                    }
                }
                crate::config::loader::save_config(&cfg)?;
                let effective_level =
                    opencaveman::CavemanLevel::from_config_str(&cfg.agents.defaults.caveman_level);
                Ok(serde_json::json!({
                    "success": true,
                    "mode": "caveman",
                    "enabled": cfg.agents.defaults.caveman_mode,
                    "level": effective_level.slug(),
                    "descriptor": effective_level.descriptor(),
                }))
            }
            "compress_file" => {
                let raw_path = first_str(arguments, &["path", "file", "target_path"])
                    .ok_or_else(|| anyhow::anyhow!("'path' is required for compress_file"))?;
                let target_path = resolve_target_path(raw_path);
                let level = first_str(arguments, &["level", "caveman_level"])
                    .and_then(opencaveman::CavemanLevel::parse_flexible)
                    .unwrap_or_else(|| {
                        opencaveman::CavemanLevel::from_config_str(
                            &live_cfg.agents.defaults.caveman_level,
                        )
                    });
                let dry_run = first_bool(arguments, &["dry_run", "dryRun"]).unwrap_or(false);
                let use_llm = first_bool(arguments, &["use_llm", "useLlm"]).unwrap_or(false);

                let llm_candidate = match use_llm {
                    false => None,
                    true => {
                        let precheck_source = std::fs::read_to_string(&target_path)?;
                        match opencaveman::scan_secret_exposure(
                            Some(target_path.as_path()),
                            &precheck_source,
                        ) {
                            Some(rejection) => {
                                anyhow::bail!(
                                    "Refused to compress sensitive file '{}': {} ({})",
                                    target_path.display(),
                                    rejection.reason,
                                    rejection.indicator
                                );
                            }
                            None => {
                                request_llm_markdown_rewrite(
                                    &self.provider,
                                    &live_cfg,
                                    &precheck_source,
                                    level,
                                )
                                .await
                            }
                        }
                    }
                };

                let outcome = opencaveman::compress_file_with_backup(
                    &target_path,
                    &data_dir,
                    level,
                    llm_candidate.as_deref(),
                    dry_run,
                )?;
                Ok(serde_json::to_value(outcome)?)
            }
            "compress_text" => {
                let text = first_str(arguments, &["text", "content", "markdown"])
                    .ok_or_else(|| anyhow::anyhow!("'text' is required for compress_text"))?;
                let level = first_str(arguments, &["level", "caveman_level"])
                    .and_then(opencaveman::CavemanLevel::parse_flexible)
                    .unwrap_or_else(|| {
                        opencaveman::CavemanLevel::from_config_str(
                            &live_cfg.agents.defaults.caveman_level,
                        )
                    });
                let compressed = opencaveman::compress_markdown_deterministic(text, level);
                let fidelity = opencaveman::verify_compression_fidelity(text, &compressed);
                let orig_tokens = opencaveman::estimate_tokens(text);
                let comp_tokens = opencaveman::estimate_tokens(&compressed);
                Ok(serde_json::json!({
                    "level": level.slug(),
                    "original_tokens": orig_tokens,
                    "compressed_tokens": comp_tokens,
                    "saved_tokens": orig_tokens.saturating_sub(comp_tokens),
                    "fidelity": fidelity,
                    "compressed_text": compressed,
                }))
            }
            "list_backups" => {
                let entries = opencaveman::list_backups(&data_dir);
                Ok(serde_json::json!({
                    "backup_root": backup_root_dir().display().to_string(),
                    "count": entries.len(),
                    "backups": entries,
                }))
            }
            "restore_backup" => {
                let query = first_str(arguments, &["backup_id", "path", "id"])
                    .ok_or_else(|| anyhow::anyhow!("'backup_id' or 'path' is required"))?;
                let restored = opencaveman::restore_backup(&data_dir, query)?;
                Ok(serde_json::json!({
                    "success": true,
                    "restored": restored,
                }))
            }
            "commit_message" => {
                let diff = first_str(arguments, &["diff"]).unwrap_or("");
                let scope_override = first_str(arguments, &["scope"]);
                let analysis = (!diff.is_empty())
                    .then(|| opencaveman::analyze_diff_for_commit(diff, scope_override));
                let commit_type = first_str(arguments, &["commit_type", "type"])
                    .or_else(|| analysis.as_ref().map(|a| a.inferred_type.as_str()))
                    .unwrap_or("chore");
                let scope = scope_override
                    .map(str::to_string)
                    .or_else(|| analysis.as_ref().and_then(|a| a.inferred_scope.clone()));
                let summary = first_str(arguments, &["summary", "text"]);
                let raw_subject = match (summary, &analysis) {
                    (Some(sum), _) => match sum.contains(':') {
                        true => sum.to_string(),
                        false => match &scope {
                            Some(sc) => format!("{commit_type}({sc}): {sum}"),
                            None => format!("{commit_type}: {sum}"),
                        },
                    },
                    (None, Some(a)) => a.suggested_header.clone(),
                    (None, None) => match &scope {
                        Some(sc) => format!("{commit_type}({sc}): update workspace changes"),
                        None => format!("{commit_type}: update workspace changes"),
                    },
                };
                let bullets: Vec<String> = arguments
                    .get("bullets")
                    .and_then(Value::as_array)
                    .map(|arr| {
                        arr.iter()
                            .filter_map(Value::as_str)
                            .map(str::to_string)
                            .collect()
                    })
                    .or_else(|| analysis.as_ref().map(|a| a.bullet_points.clone()))
                    .unwrap_or_default();
                let formatted = opencaveman::validate_and_format_commit(&raw_subject, &bullets);
                Ok(serde_json::json!({
                    "analysis": analysis,
                    "commit": formatted,
                }))
            }
            "code_review" => {
                let diff = first_str(arguments, &["diff"]).unwrap_or("");
                let hunk_maps = (!diff.is_empty())
                    .then(|| opencaveman::analyze_diff_for_review(diff))
                    .unwrap_or_default();
                let findings: Vec<opencaveman::ReviewFinding> = arguments
                    .get("findings")
                    .and_then(Value::as_array)
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|item| {
                                let line = item.get("line").and_then(Value::as_u64)? as usize;
                                let severity = item
                                    .get("severity")
                                    .and_then(Value::as_str)
                                    .unwrap_or("nit")
                                    .to_string();
                                let file = item
                                    .get("file")
                                    .and_then(Value::as_str)
                                    .map(str::to_string);
                                let direct_msg = item
                                    .get("message")
                                    .and_then(Value::as_str)
                                    .map(str::trim)
                                    .filter(|s| !s.is_empty())
                                    .map(str::to_string);
                                let composed_msg = direct_msg.or_else(|| {
                                    let prob = item
                                        .get("problem")
                                        .and_then(Value::as_str)
                                        .map(str::trim)
                                        .filter(|s| !s.is_empty())?;
                                    let fix_part = item
                                        .get("fix")
                                        .and_then(Value::as_str)
                                        .map(str::trim)
                                        .filter(|s| !s.is_empty());
                                    Some(match fix_part {
                                        Some(fx) => {
                                            format!("{}. {}", prob.trim_end_matches('.'), fx)
                                        }
                                        None => prob.to_string(),
                                    })
                                })?;
                                Some(opencaveman::ReviewFinding {
                                    file,
                                    line,
                                    severity,
                                    message: composed_msg,
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let formatted = opencaveman::normalize_review_output(&findings);
                Ok(serde_json::json!({
                    "hunks": hunk_maps,
                    "finding_count": findings.len(),
                    "formatted_review": formatted,
                }))
            }
            "auto_clarity" => {
                let prompt = first_str(arguments, &["text", "prompt", "query"]).unwrap_or("");
                let signal = opencaveman::classify_auto_clarity(prompt);
                let command_intent = opencaveman::detect_command_intent(prompt);
                Ok(serde_json::json!({
                    "auto_clarity": signal,
                    "command_intent_detected": command_intent.is_some(),
                }))
            }
            "stats" => {
                let stats = opencaveman::load_stats_ledger(&data_dir);
                Ok(serde_json::json!({
                    "mode": "caveman",
                    "enabled": live_cfg.agents.defaults.caveman_mode,
                    "level": opencaveman::normalize_level_slug(&live_cfg.agents.defaults.caveman_level),
                    "compact_badge": stats.compact_lifetime_badge,
                    "stats": stats,
                }))
            }
            "reset_stats" => {
                let stats = opencaveman::reset_stats_ledger(&data_dir)?;
                Ok(serde_json::json!({
                    "success": true,
                    "stats": stats,
                }))
            }
            _ => unreachable!(),
        }
    }
}
