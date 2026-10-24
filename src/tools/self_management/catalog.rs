use crate::tools::Tool;
use anyhow::Result;
use serde_json::Value;

fn is_pipeline_expr_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | '[' | ']')
}

fn traverse_json_path<'a>(root: &'a Value, segments: &[&str]) -> Option<&'a Value> {
    let effective_segments = match segments.first().copied() {
        Some("output") => &segments[1..],
        _ => segments,
    };
    effective_segments.iter().try_fold(root, |curr, &seg| match curr {
        Value::Object(map) => map.get(seg),
        Value::Array(arr) => seg.parse::<usize>().ok().and_then(|idx| arr.get(idx)),
        _ => None,
    })
}

fn resolve_pipeline_expr<'a>(
    expr: &str,
    prev_output: Option<&'a Value>,
    indexed_outputs: &'a [Value],
    named_outputs: &'a std::collections::HashMap<String, Value>,
) -> Option<&'a Value> {
    let parts: Vec<&str> = expr.split('.').filter(|s| !s.is_empty()).collect();
    let head = *parts.first()?;
    let tail = &parts[1..];
    let root_val = match (
        head == "prev",
        head.strip_prefix("steps[").and_then(|s| s.strip_suffix(']')),
    ) {
        (true, _) => prev_output,
        (false, Some(idx_str)) => idx_str
            .parse::<usize>()
            .ok()
            .and_then(|idx| indexed_outputs.get(idx)),
        (false, None) => named_outputs.get(head),
    }?;
    traverse_json_path(root_val, tail)
}

fn value_to_inline_string(val: &Value) -> String {
    match val {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn substitute_brace_templates(
    input: &str,
    prev_output: Option<&Value>,
    indexed_outputs: &[Value],
    named_outputs: &std::collections::HashMap<String, Value>,
) -> String {
    let mut parts = input.split("{{");
    let first = parts.next().unwrap_or("");
    let mut out = String::from(first);
    for segment in parts {
        match segment.split_once("}}") {
            Some((expr, rest)) => {
                let replacement =
                    resolve_pipeline_expr(expr.trim(), prev_output, indexed_outputs, named_outputs)
                        .map(value_to_inline_string)
                        .unwrap_or_else(|| format!("{{{{{}}}}}", expr));
                out.push_str(&replacement);
                out.push_str(rest);
            }
            None => {
                out.push_str("{{");
                out.push_str(segment);
            }
        }
    }
    out
}

fn substitute_dollar_templates(
    input: &str,
    prev_output: Option<&Value>,
    indexed_outputs: &[Value],
    named_outputs: &std::collections::HashMap<String, Value>,
) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut idx = 0;
    let mut out = String::with_capacity(input.len());
    while idx < chars.len() {
        match chars[idx] == '$' {
            false => {
                out.push(chars[idx]);
                idx += 1;
            }
            true => {
                let start = idx + 1;
                let mut end = start;
                while end < chars.len() && is_pipeline_expr_char(chars[end]) {
                    end += 1;
                }
                let trimmed_end = (start..end)
                    .rev()
                    .find(|&pos| chars[pos] != '.')
                    .map(|pos| pos + 1)
                    .unwrap_or(start);
                let expr: String = chars[start..trimmed_end].iter().collect();
                match (!expr.is_empty())
                    .then(|| {
                        resolve_pipeline_expr(&expr, prev_output, indexed_outputs, named_outputs)
                    })
                    .flatten()
                {
                    Some(resolved) => {
                        out.push_str(&value_to_inline_string(resolved));
                        idx = trimmed_end;
                    }
                    None => {
                        out.push('$');
                        idx += 1;
                    }
                }
            }
        }
    }
    out
}

fn interpolate_step_value(
    value: &Value,
    prev_output: Option<&Value>,
    indexed_outputs: &[Value],
    named_outputs: &std::collections::HashMap<String, Value>,
) -> Value {
    match value {
        Value::String(s) => {
            let trimmed = s.trim();
            let exact_expr = trimmed
                .strip_prefix("{{")
                .and_then(|inner| inner.strip_suffix("}}"))
                .map(str::trim)
                .or_else(|| {
                    trimmed
                        .strip_prefix('$')
                        .filter(|rest| !rest.is_empty() && rest.chars().all(is_pipeline_expr_char))
                });
            match exact_expr.and_then(|expr| {
                resolve_pipeline_expr(expr, prev_output, indexed_outputs, named_outputs)
            }) {
                Some(resolved) => resolved.clone(),
                None => {
                    let after_braces = substitute_brace_templates(
                        s,
                        prev_output,
                        indexed_outputs,
                        named_outputs,
                    );
                    let after_dollars = substitute_dollar_templates(
                        &after_braces,
                        prev_output,
                        indexed_outputs,
                        named_outputs,
                    );
                    Value::String(after_dollars)
                }
            }
        }
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| {
                    interpolate_step_value(item, prev_output, indexed_outputs, named_outputs)
                })
                .collect(),
        ),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(k, v)| {
                    (
                        k.clone(),
                        interpolate_step_value(v, prev_output, indexed_outputs, named_outputs),
                    )
                })
                .collect(),
        ),
        other => other.clone(),
    }
}

fn value_contains_pipeline_reference(value: &Value) -> bool {
    match value {
        Value::String(s) => {
            s.contains("$prev")
                || s.contains("$steps[")
                || s.contains("{{")
                || s.split('$').skip(1).any(|seg| {
                    seg.chars()
                        .take_while(|c| is_pipeline_expr_char(*c))
                        .collect::<String>()
                        .contains('.')
                })
        }
        Value::Array(items) => items.iter().any(value_contains_pipeline_reference),
        Value::Object(map) => map.values().any(value_contains_pipeline_reference),
        _ => false,
    }
}

fn attach_step_telemetry(mut step_res: Value, step_id: Option<&str>, duration_ms: u64) -> Value {
    step_res["duration_ms"] = serde_json::json!(duration_ms);
    let _ = step_id.map(|id| {
        step_res["id"] = serde_json::json!(id);
    });
    step_res
}

pub struct ToolCatalogTool {
    registry: crate::tools::ToolRegistry,
}

impl ToolCatalogTool {
    pub fn new(registry: crate::tools::ToolRegistry) -> Self {
        Self { registry }
    }

    async fn run_batch_step(
        &self,
        item: &Value,
        defaults: &crate::config::schema::AgentDefaults,
        runtime: &crate::tools::resource_policy::RuntimeResourceSnapshot,
        prev_output: Option<&Value>,
        indexed_outputs: &[Value],
        named_outputs: &std::collections::HashMap<String, Value>,
    ) -> (Option<String>, Value) {
        let step_started = std::time::Instant::now();
        let step_id = item
            .get("id")
            .or_else(|| item.get("step_id"))
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let raw_name = item
            .get("tool")
            .or_else(|| item.get("name"))
            .and_then(|v| v.as_str())
            .map(str::trim)
            .unwrap_or("");
        let canonical_name = self.registry.resolve_canonical_name(raw_name);
        let raw_args = item
            .get("arguments")
            .or_else(|| item.get("args"))
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));
        let interpolated_args =
            interpolate_step_value(&raw_args, prev_output, indexed_outputs, named_outputs);
        let normalized_args = crate::tools::normalize_tool_args(&interpolated_args);

        let is_empty_name = canonical_name.is_empty();
        let is_recursive_catalog = canonical_name == "tool_catalog";
        let is_forbidden =
            crate::agent::security::SecurityGuard::is_forbidden(&canonical_name, &normalized_args);
        let is_sensitive = crate::agent::security::SecurityGuard::is_sensitive_with_mode(
            &canonical_name,
            &normalized_args,
            &defaults.security_mode,
        );

        let (executed_tool, raw_step_res) = match (
            is_empty_name,
            is_recursive_catalog,
            is_forbidden,
            is_sensitive,
        ) {
            (true, _, _, _) => (
                None,
                serde_json::json!({
                    "tool": raw_name,
                    "status": "error",
                    "error": "Missing 'tool' name in batch_execute step"
                }),
            ),
            (false, true, _, _) => (
                None,
                serde_json::json!({
                    "tool": canonical_name,
                    "status": "blocked",
                    "error": "Recursive tool_catalog calls are not permitted inside batch_execute"
                }),
            ),
            (false, false, true, _) => (
                None,
                serde_json::json!({
                    "tool": canonical_name,
                    "status": "blocked",
                    "error": "Execution denied by host: command is forbidden by security rules"
                }),
            ),
            (false, false, false, true) => (
                None,
                serde_json::json!({
                    "tool": canonical_name,
                    "status": "requires_approval",
                    "error": "Tool requires interactive approval and cannot run inside batch_execute; invoke it as a direct tool call"
                }),
            ),
            (false, false, false, false) => match self.registry.get(&canonical_name) {
                None => (
                    None,
                    serde_json::json!({
                        "tool": canonical_name,
                        "status": "not_found",
                        "error": format!("Tool '{}' not found in registry", canonical_name)
                    }),
                ),
                Some(tool) => {
                    let metadata = tool.metadata();
                    let decision = crate::tools::resource_policy::ToolResourcePolicy::evaluate(
                        &metadata, defaults, runtime,
                    );
                    match decision {
                        crate::tools::resource_policy::ToolResourceDecision::Block { reason } => (
                            None,
                            serde_json::json!({
                                "tool": canonical_name,
                                "status": "blocked_by_resource_policy",
                                "error": reason
                            }),
                        ),
                        crate::tools::resource_policy::ToolResourceDecision::RequireApproval {
                            reason,
                        } => (
                            None,
                            serde_json::json!({
                                "tool": canonical_name,
                                "status": "requires_approval",
                                "error": reason
                            }),
                        ),
                        crate::tools::resource_policy::ToolResourceDecision::Allow => {
                            self.registry.record_tool_used(&canonical_name);
                            let timeout_secs = metadata
                                .recommended_timeout_secs
                                .unwrap_or(defaults.tool_timeout_secs)
                                .clamp(5, 300);
                            let timed = tokio::time::timeout(
                                std::time::Duration::from_secs(timeout_secs),
                                tool.call(&normalized_args),
                            )
                            .await;
                            match timed {
                                Ok(Ok(output)) => (
                                    Some(canonical_name.clone()),
                                    serde_json::json!({
                                        "tool": canonical_name,
                                        "status": "success",
                                        "output": output
                                    }),
                                ),
                                Ok(Err(err)) => (
                                    Some(canonical_name.clone()),
                                    serde_json::json!({
                                        "tool": canonical_name,
                                        "status": "error",
                                        "error": err.to_string()
                                    }),
                                ),
                                Err(_) => (
                                    Some(canonical_name.clone()),
                                    serde_json::json!({
                                        "tool": canonical_name,
                                        "status": "timeout",
                                        "error": format!("Timed out after {}s", timeout_secs)
                                    }),
                                ),
                            }
                        }
                    }
                }
            },
        };
        let duration_ms = step_started.elapsed().as_millis() as u64;
        (
            executed_tool,
            attach_step_telemetry(raw_step_res, step_id, duration_ms),
        )
    }
    fn is_read_only_batch_step(&self, item: &Value) -> bool {
        let raw_name = item
            .get("tool")
            .or_else(|| item.get("name"))
            .and_then(|v| v.as_str())
            .map(str::trim)
            .unwrap_or("");
        let canonical_name = self.registry.resolve_canonical_name(raw_name);
        let is_mutating_name = matches!(
            canonical_name.as_str(),
            "" | "tool_catalog"
                | "write_file"
                | "patch_file"
                | "replace_lines"
                | "zenflow_edit"
                | "exec_command"
                | "db_write"
                | "store_memory"
                | "clear_memory"
                | "set_working_memory"
                | "evict_expired_working_memory"
                | "create_entities"
                | "create_relations"
                | "add_observations"
                | "delete_entities"
                | "delete_observations"
                | "delete_relations"
                | "request_tool_scope"
                | "optimize_tool_scope"
                | "manage_config"
                | "manage_sessions"
                | "manage_backups"
                | "curate_skill"
        );
        match (is_mutating_name, self.registry.get(&canonical_name)) {
            (true, _) | (false, None) => false,
            (false, Some(tool)) => tool.metadata().is_read_only(),
        }
    }
}

#[async_trait::async_trait]
impl Tool for ToolCatalogTool {
    fn name(&self) -> &str {
        "tool_catalog"
    }

    fn description(&self) -> &str {
        "Search, discover, hot-mount, and batch-execute native tools from the 240-tool catalog using BM25 relevance (alias: tool_search). Also lists registered tools by domain, risk, or resource status."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Optional search keywords to discover relevant tools in the 240-tool catalog using BM25 lexical relevance (e.g. 'sqlite inspector', 'video animation', 'diff compression', 'cron job')."
                },
                "mount": {
                    "type": "boolean",
                    "description": "If true (default when query is provided), automatically hot-mounts the top matched tools into active context for immediate execution in subsequent steps."
                },
                "batch_execute": {
                    "type": "array",
                    "description": "Optional list of up to 8 safe tool calls [{tool, arguments}] to execute immediately in a single turn without an extra LLM round-trip.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "tool": { "type": "string", "description": "Tool name or alias to execute." },
                            "arguments": { "type": "object", "description": "JSON arguments object for the tool." }
                        },
                        "required": ["tool"]
                    }
                },
                "domain": {
                    "type": "string",
                    "description": "Optional domain filter, such as filesystem, shell, web, subagent, memory, document, media, or self_management."
                },
                "risk": {
                    "type": "string",
                    "enum": ["low", "medium", "high"],
                    "description": "Optional risk filter."
                },
                "include_schema": {
                    "type": "boolean",
                    "description": "Include each tool JSON parameter schema. Default false to keep output compact."
                },
                "only_exposed": {
                    "type": "boolean",
                    "description": "If true, only return tools currently included in the provider tool payload."
                },
                "prompt": {
                    "type": "string",
                    "description": "Optional prompt/task text to explain prompt-aware tool routing for that turn."
                },
                "resource_overrides": {
                    "type": "object",
                    "description": "Optional diagnostic overrides for resource-policy preview, such as allow_network_tools, min_free_disk_gb, free_disk_gb, active_process_tools, max_concurrent_process_tools, and warn_before_expensive_tools.",
                    "additionalProperties": true
                }
            }
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let str_arg = arguments
            .as_str()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_lowercase);
        let (raw_domain, raw_risk) = match str_arg.as_deref() {
            Some("low" | "medium" | "high") => (None, str_arg),
            Some(_) => (str_arg, None),
            None => (None, None),
        };

        let query_str = crate::tools::arguments::extract_trimmed_non_empty_str(
            arguments,
            &["query", "search", "tool", "name"],
        );
        let mount =
            crate::tools::arguments::get_bool_arg(arguments, &["mount"], query_str.is_some());
        let explicit_include_schema =
            crate::tools::arguments::extract_bool_arg(arguments, &["include_schema"]);
        let include_schema = explicit_include_schema.unwrap_or(false);
        let only_exposed =
            crate::tools::arguments::get_bool_arg(arguments, &["only_exposed"], false);
        let domain_filter = arguments
            .get("domain")
            .and_then(|v| v.as_str())
            .or(raw_domain.as_deref());
        let risk_filter = arguments
            .get("risk")
            .and_then(|v| v.as_str())
            .or(raw_risk.as_deref());
        let prompt = arguments
            .get("prompt")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let mut defaults = crate::config::loader::load_config()
            .map(|config| config.agents.defaults)
            .unwrap_or_default();
        let mut runtime = crate::tools::resource_policy::RuntimeResourceSnapshot::current();
        arguments
            .get("resource_overrides")
            .and_then(|v| v.as_object())
            .map(|overrides| {
                overrides
                    .get("allow_network_tools")
                    .and_then(|v| v.as_bool())
                    .map(|value| defaults.allow_network_tools = value);
                overrides
                    .get("min_free_disk_gb")
                    .and_then(|v| v.as_f64())
                    .map(|value| defaults.min_free_disk_gb = value);
                overrides
                    .get("max_concurrent_process_tools")
                    .and_then(|v| v.as_u64())
                    .map(|value| defaults.max_concurrent_process_tools = value as usize);
                overrides
                    .get("warn_before_expensive_tools")
                    .and_then(|v| v.as_bool())
                    .map(|value| defaults.warn_before_expensive_tools = value);
                overrides
                    .get("free_disk_gb")
                    .and_then(|v| v.as_f64())
                    .map(|value| runtime.free_disk_gb = Some(value));
                overrides
                    .get("active_process_tools")
                    .and_then(|v| v.as_u64())
                    .map(|value| runtime.active_process_tools = value as usize);
            });

        let mut batch_results = Vec::new();
        let mut batch_mounted_tools = Vec::new();
        let mut batch_execution_mode: Option<&'static str> = None;
        let mut batch_total_duration_ms: Option<u64> = None;
        match arguments.get("batch_execute").and_then(|v| v.as_array()) {
            Some(steps) => {
                let batch_started = std::time::Instant::now();
                let capped: Vec<&Value> = steps.iter().take(8).collect();
                let has_pipeline_refs = capped.iter().any(|step| {
                    step.get("arguments")
                        .or_else(|| step.get("args"))
                        .is_some_and(value_contains_pipeline_reference)
                });
                let run_concurrent = capped.len() > 1
                    && !has_pipeline_refs
                    && capped
                        .iter()
                        .all(|step| self.is_read_only_batch_step(step));
                let empty_named = std::collections::HashMap::new();
                let outcomes = match run_concurrent {
                    true => {
                        batch_execution_mode = Some("concurrent");
                        futures_util::future::join_all(capped.iter().map(|step| {
                            self.run_batch_step(
                                step,
                                &defaults,
                                &runtime,
                                None,
                                &[],
                                &empty_named,
                            )
                        }))
                        .await
                    }
                    false => {
                        (!capped.is_empty()).then(|| {
                            batch_execution_mode = Some("sequential");
                        });
                        let mut seq = Vec::with_capacity(capped.len());
                        let mut indexed_outputs: Vec<Value> = Vec::with_capacity(capped.len());
                        let mut named_outputs: std::collections::HashMap<String, Value> =
                            std::collections::HashMap::new();
                        let mut prev_output: Option<Value> = None;
                        for step in capped {
                            let (executed_tool, step_res) = self
                                .run_batch_step(
                                    step,
                                    &defaults,
                                    &runtime,
                                    prev_output.as_ref(),
                                    &indexed_outputs,
                                    &named_outputs,
                                )
                                .await;
                            let step_output = step_res
                                .get("output")
                                .cloned()
                                .unwrap_or(Value::Null);
                            let _ = step
                                .get("id")
                                .or_else(|| step.get("step_id"))
                                .and_then(|v| v.as_str())
                                .map(str::trim)
                                .filter(|s| !s.is_empty())
                                .map(|id| {
                                    named_outputs.insert(id.to_string(), step_output.clone());
                                });
                            indexed_outputs.push(step_output.clone());
                            prev_output = Some(step_output);
                            seq.push((executed_tool, step_res));
                        }
                        seq
                    }
                };
                for (executed_tool, step_res) in outcomes {
                    executed_tool.map(|name| batch_mounted_tools.push(name));
                    batch_results.push(step_res);
                }
                batch_total_duration_ms = Some(batch_started.elapsed().as_millis() as u64);
            }
            None => {}
        }

        let mut all_entries = self
            .registry
            .catalog_entries_for_prompt(include_schema, prompt);

        query_str.map(|query| {
            let static_tools = self.registry.read_tools();
            let bm25_scores = crate::tools::routing::compute_bm25_scores(query, &static_tools);
            drop(static_tools);

            for entry in &mut all_entries {
                let name = entry["name"].as_str().unwrap_or("");
                let score = bm25_scores.get(name).copied().unwrap_or(0.0);
                entry["bm25_score"] = serde_json::json!(score);
            }

            all_entries.sort_by(|a, b| {
                let sa = a["bm25_score"].as_f64().unwrap_or(0.0);
                let sb = b["bm25_score"].as_f64().unwrap_or(0.0);
                sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal)
            });
        });

        let tool_meta_map: std::collections::HashMap<String, crate::tools::ToolMetadata> = self
            .registry
            .tool_inventory_snapshot()
            .into_iter()
            .map(|(name, _, meta)| (name, meta))
            .collect();
        for entry in &mut all_entries {
            let entry_name = entry["name"].as_str().unwrap_or("");
            let inferred = tool_meta_map
                .get(entry_name)
                .cloned()
                .unwrap_or_else(|| crate::tools::ToolMetadata::infer(entry_name));
            let risk = match entry["risk"].as_str().unwrap_or("low") {
                "high" => crate::tools::ToolRisk::High,
                "medium" => crate::tools::ToolRisk::Medium,
                _ => crate::tools::ToolRisk::Low,
            };
            let metadata = crate::tools::ToolMetadata {
                presentation_name: inferred.presentation_name,
                domain: inferred.domain,
                risk,
                uses_network: entry["uses_network"].as_bool().unwrap_or(inferred.uses_network),
                writes_disk: entry["writes_disk"].as_bool().unwrap_or(inferred.writes_disk),
                spawns_process: entry["spawns_process"]
                    .as_bool()
                    .unwrap_or(inferred.spawns_process),
                requires_approval: entry["requires_approval"]
                    .as_bool()
                    .unwrap_or(inferred.requires_approval),
                priority: entry["priority"].as_u64().unwrap_or(inferred.priority as u64) as u8,
                aliases: inferred.aliases,
                examples: inferred.examples,
                when_to_use: inferred.when_to_use,
                when_not_to_use: inferred.when_not_to_use,
                recommended_timeout_secs: inferred.recommended_timeout_secs,
            };
            let decision = crate::tools::resource_policy::ToolResourcePolicy::evaluate(
                &metadata, &defaults, &runtime,
            );
            entry["resource_policy"] = serde_json::json!({
                "decision": decision.as_str(),
                "reason": decision.reason(),
                "free_disk_gb": runtime.free_disk_gb,
                "active_process_tools": runtime.active_process_tools,
                "min_free_disk_gb": defaults.min_free_disk_gb,
                "allow_network_tools": defaults.allow_network_tools,
                "max_concurrent_process_tools": defaults.max_concurrent_process_tools,
                "warn_before_expensive_tools": defaults.warn_before_expensive_tools,
            });
        }
        let exposed_count = all_entries
            .iter()
            .filter(|entry| entry["exposed_to_model"].as_bool().unwrap_or(false))
            .count();
        let mut entries: Vec<Value> = all_entries
            .into_iter()
            .filter(|entry| {
                let passes_query = query_str.is_none()
                    || entry
                        .get("bm25_score")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0)
                        > 0.0;
                let passes_exposed =
                    !only_exposed || entry["exposed_to_model"].as_bool().unwrap_or(false);
                let passes_domain =
                    domain_filter.is_none_or(|domain| entry["domain"].as_str() == Some(domain));
                let passes_risk =
                    risk_filter.is_none_or(|risk| entry["risk"].as_str() == Some(risk));
                passes_query && passes_exposed && passes_domain && passes_risk
            })
            .collect();

        (explicit_include_schema.is_none() && query_str.is_some()).then(|| {
            let static_tools = self.registry.read_tools();
            for entry in entries.iter_mut().take(8) {
                let _ = entry["name"]
                    .as_str()
                    .and_then(|name| static_tools.get(name))
                    .map(|tool| {
                        entry["parameters"] = tool.parameters();
                    });
            }
        });

        let mut mounted_tools = batch_mounted_tools;
        let mut mounted_domains = Vec::new();
        (mount && query_str.is_some()).then(|| {
            for entry in entries.iter().take(8) {
                entry["name"]
                    .as_str()
                    .map(|name| mounted_tools.push(name.to_string()));
                entry["domain"].as_str().map(|domain| {
                    let domain_string = domain.to_string();
                    (!mounted_domains.contains(&domain_string))
                        .then(|| mounted_domains.push(domain_string));
                });
            }
        });
        (!mounted_tools.is_empty()).then(|| {
            self.registry
                .request_tool_scope(mounted_tools.clone(), mounted_domains.clone());
        });

        let mut domains = std::collections::BTreeMap::<String, usize>::new();
        let mut risks = std::collections::BTreeMap::<String, usize>::new();
        for entry in &entries {
            entry["domain"].as_str().map(|domain| {
                *domains.entry(domain.to_string()).or_default() += 1;
            });
            entry["risk"].as_str().map(|risk| {
                *risks.entry(risk.to_string()).or_default() += 1;
            });
        }

        let is_pure_batch_call = !batch_results.is_empty()
            && query_str.is_none()
            && domain_filter.is_none()
            && risk_filter.is_none()
            && !only_exposed
            && !include_schema;
        let mut response = serde_json::json!({
            "success": true,
            "tool_count": entries.len(),
            "exposed_count": exposed_count,
            "selected_domains": self.registry.selected_domains_for_prompt(prompt),
            "domains": domains,
            "risks": risks,
        });
        match is_pure_batch_call {
            true => {
                response["tools_omitted"] = serde_json::json!(true);
            }
            false => {
                response["tools"] = serde_json::json!(entries);
            }
        }
        (!mounted_tools.is_empty()).then(|| {
            response["mounted"] = serde_json::json!(true);
            response["mounted_tools"] = serde_json::json!(mounted_tools);
            response["message"] = serde_json::json!(
                "Found matching tools and hot-mounted them into active context for immediate execution in subsequent steps."
            );
        });
        (!batch_results.is_empty()).then(|| {
            response["batch_executed_count"] = serde_json::json!(batch_results.len());
            response["batch_execution_mode"] =
                serde_json::json!(batch_execution_mode.unwrap_or("sequential"));
            response["batch_total_duration_ms"] =
                serde_json::json!(batch_total_duration_ms.unwrap_or(0));
            response["batch_results"] = serde_json::json!(batch_results);
        });

        Ok(response)
    }
}

#[cfg(test)]
#[path = "catalog_tests.rs"]
mod tests;
