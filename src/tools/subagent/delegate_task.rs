use super::evaluator_optimizer::validate_schema;
use super::{build_provider_for_model, CancellationToken, DELEGATION_DEPTH};
use crate::agent::style::*;
use crate::agent::AgentLoop;
use crate::config::schema::Config;
use crate::providers::LLMProvider;
use crate::session::SessionManager;
use crate::tools::Tool;
use crate::tools::ToolRegistry;
use anyhow::{anyhow, Result};
use serde_json::Value;
use std::sync::Arc;

pub struct DelegateTaskTool {
    pub config: Config,
    pub parent_provider: Arc<dyn LLMProvider>,
    pub session_manager: SessionManager,
    pub parent_tools: Vec<Arc<dyn Tool>>,
    pub cancellation_token: CancellationToken,
}

#[async_trait::async_trait]
impl Tool for DelegateTaskTool {
    fn name(&self) -> &str {
        "delegate_task"
    }

    fn description(&self) -> &str {
        "Delegate a specific subtask or research item to a focused subagent. The subagent runs in an isolated workspace, executes tools to accomplish the goal, and returns a summary."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "goal": {
                    "type": "string",
                    "description": "The specific goal/task for the subagent to accomplish. Be clear and detailed."
                },
                "context": {
                    "type": "string",
                    "description": "Additional context, details, files, or background information needed for the task."
                },
                "model": {
                    "type": "string",
                    "description": "Optional model override name (e.g., 'gpt-4o-mini', 'claude-3-5-haiku') for the subagent."
                },
                "json_schema": {
                    "type": "object",
                    "description": "Optional: A JSON Schema definition that the subagent's final output summary MUST strictly conform to."
                }
            },
            "required": ["goal"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        crate::agent::style::spinner::IS_SILENT.scope(crate::agent::style::is_silent(), async {
        let current_depth = DELEGATION_DEPTH.try_with(|d| *d).unwrap_or(0);
        if current_depth >= 3 {
            crate::tui_println!("{}⚠️ Delegation depth limit reached ({}). Aborting nested delegate_task.{}", AURA_GOLD, current_depth, COLOR_RESET);
            return Err(anyhow!("Delegation limit reached. Max nesting depth is 3."));
        }

        let goal = arguments.get("goal").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'goal' argument"))?;
        let context = arguments.get("context").and_then(|v| v.as_str()).unwrap_or("");
        let model_override = arguments.get("model").and_then(|v| v.as_str());
        let json_schema = arguments.get("json_schema").cloned();

        let clean_goal = ensure_markdown_images(goal);
        let clean_context = ensure_markdown_images(context);

        let has_images = crate::providers::parse_multimodal_content(&clean_goal).await.iter().any(|p| matches!(p, crate::providers::ContentPart::Image { .. }))
            || crate::providers::parse_multimodal_content(&clean_context).await.iter().any(|p| matches!(p, crate::providers::ContentPart::Image { .. }));

        let mut selected_model = self.config.agents.defaults.model.clone();
        let mut selected_fallback_models: Vec<String> = Vec::new();
        let provider = if let Some(m) = model_override {
            match build_provider_for_model(&self.config, m) {
                Ok(p) => {
                    selected_model = m.to_string();
                    p
                }
                Err(e) => {
                    crate::tui_println!("{}⚠️ Failed to configure subagent model '{}' ({}). Falling back to parent model.{}", AURA_GOLD, m, e, COLOR_RESET);
                    self.parent_provider.clone()
                }
            }
        } else if has_images && !crate::providers::model_supports_vision(&self.config.agents.defaults.model) {
            let mut resolved_provider = None;
            let dynamic_fallbacks: Vec<String> = self
                .config
                .get_dynamic_fallbacks("vision_agent")
                .into_iter()
                .filter(|model| crate::providers::model_supports_vision(model))
                .collect();
            for (idx, fallback_model) in dynamic_fallbacks.iter().enumerate() {
                if let Ok(p) = build_provider_for_model(&self.config, fallback_model) {
                    crate::tui_println!("{}  ✓ Auto-routed vision task to subagent model '{}'{}", EMERALD_GREEN, fallback_model, COLOR_RESET);
                    selected_model = fallback_model.clone();
                    selected_fallback_models = dynamic_fallbacks[idx + 1..].to_vec();
                    resolved_provider = Some(p);
                    break;
                }
            }
            resolved_provider.unwrap_or_else(|| self.parent_provider.clone())
        } else {
            self.parent_provider.clone()
        };

        let mut child_config = self.config.clone();
        child_config.agents.defaults.model = selected_model.clone();
        child_config.agents.defaults.fallback_models = selected_fallback_models
            .iter()
            .map(|model| serde_json::json!(model))
            .collect();
        let child_registry = ToolRegistry::new_with_context(
            child_config.clone(),
            provider.clone(),
            self.session_manager.clone(),
        );
        for tool in &self.parent_tools {
            let name = tool.name();
            if name != "delegate_task" && name != "parallel_research" && name != "evaluator_optimizer_loop" {
                child_registry.register(tool.clone());
            }
        }

        let child_session_id = format!("subagent:{}", &uuid::Uuid::new_v4().to_string()[..8]);
        let child_agent = AgentLoop::new(
            child_config,
            provider,
            child_registry,
            self.session_manager.clone(),
        );

        let mut subagent_prompt = format!(
            "You are a focused subagent. Complete the following task using the tools available.\n\n\
            TASK:\n{}\n\n\
            CONTEXT:\n{}\n\n\
            When finished, provide a clear, concise summary of what you did and found.",
            clean_goal, clean_context
        );

        // Automatically scan goal and context for image paths and append markdown image links
        let mut image_paths = Vec::new();
        if let Ok(path_regex) = regex::Regex::new(r"(?:file://)?(/[a-zA-Z0-9_\-\./]+|~/[a-zA-Z0-9_\-\./]+)") {
            for cap in path_regex.captures_iter(&format!("{} {}", clean_goal, clean_context)) {
                if let Some(mat) = cap.get(1) {
                    let path_str = mat.as_str();
                    let resolved_path = crate::config::resolve_path(path_str);

                    let mut final_path = None;
                    if resolved_path.exists() && resolved_path.is_file() {
                        final_path = Some(resolved_path);
                    } else {
                        for ext in &["png", "jpg", "jpeg", "webp", "gif"] {
                            let path_with_ext = resolved_path.with_extension(ext);
                            if path_with_ext.exists() && path_with_ext.is_file() {
                                final_path = Some(path_with_ext);
                                break;
                            }
                        }
                    }

                    if let Some(path) = final_path {
                        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                        if ["png", "jpg", "jpeg", "webp", "gif"].contains(&ext.as_str()) {
                            let canonical = path.to_string_lossy().to_string();
                            if !image_paths.contains(&canonical) {
                                image_paths.push(canonical);
                            }
                        }
                    }
                }
            }
        }
        // Fallback to default clipboard image if no specific path was found but task mentions an image
        if image_paths.is_empty() {
            let default_clip = crate::config::resolve_path("~/.openz/clipboard_image_0.png");
            if default_clip.exists() && default_clip.is_file() {
                let text_lower = format!("{} {}", clean_goal, clean_context).to_lowercase();
                if text_lower.contains("image") || text_lower.contains("picture") || text_lower.contains("screenshot") {
                    image_paths.push(default_clip.to_string_lossy().to_string());
                }
            }
        }

        for img in image_paths {
            subagent_prompt.push_str(&format!(" ![](file://{})", img));
        }

        if let Some(ref schema) = json_schema {
            subagent_prompt.push_str(&format!(
                "\n\nCRITICAL REQUIREMENT: Your final response MUST be a raw JSON object strictly conforming to this JSON Schema:\n{}\nDo not wrap it in markdown code blocks, do not add any conversational text. Return only the raw valid JSON.",
                serde_json::to_string_pretty(schema).unwrap_or_default()
            ));
        }

        let branch_id = format!("branch_{}", &uuid::Uuid::new_v4().to_string()[..8]);
        let mut has_branch = false;
        {
            let tool = crate::tools::graph_memory::CreateDatabaseBranchTool;
            match tool.call(&serde_json::json!({ "branchId": branch_id })).await {
                Ok(_) => {
                    crate::tui_println!("{}  ✓ Isolated simulation space branch '{}' created{}", EMERALD_GREEN, branch_id, COLOR_RESET);
                    has_branch = true;
                }
                Err(e) => {
                    tracing::warn!("Failed to create database branch: {:?}", e);
                }
            }
        }

        let parent_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let workspace_dir = match create_isolated_workspace(&parent_dir) {
            Ok(dir) => {
                crate::tui_println!("{}  ✓ Isolated workspace worktree created at {:?}{}", EMERALD_GREEN, dir, COLOR_RESET);
                dir
            }
            Err(e) => {
                crate::tui_println!("{}⚠️  Failed to create isolated workspace ({:?}). Running in active workspace.{}", AURA_GOLD, e, COLOR_RESET);
                parent_dir.clone()
            }
        };

        let _worktree_guard = WorktreeGuard::new(parent_dir.clone(), workspace_dir.clone());

        if !crate::agent::style::is_silent() {
            let prefix = crate::agent::style::get_tree_prefix(false);
            crate::tui_println!(
                "{}{}{}● {}{}Subagent{} {}using {}{}",
                AURA_SLATE, prefix, COLOR_RESET,
                RED_ORANGE, COLOR_BOLD, COLOR_RESET,
                AURA_SLATE, selected_model, COLOR_RESET
            );
        }
        let spinner_msg = crate::agent::style::get_tree_spinner_msg("subagent", "");

        struct CancelOnDrop {
            token: CancellationToken,
            completed: bool,
        }
        impl Drop for CancelOnDrop {
            fn drop(&mut self) {
                if !self.completed {
                    self.token.cancel();
                }
            }
        }
        let mut cancel_guard = CancelOnDrop {
            token: self.cancellation_token.clone(),
            completed: false,
        };

        let mut run_res = {
            let p_ref = &subagent_prompt;
            let c_ref = &child_session_id;
            let child_agent_ref = &child_agent;
            let run_res_fut = crate::config::loader::ACTIVE_WORKSPACE.scope(workspace_dir.clone(), async {
                DELEGATION_DEPTH.scope(current_depth + 1, async {
                    tokio::select! {
                        biased;
                        _ = self.cancellation_token.wait_for_cancellation() => {
                            Err(anyhow!("Subagent task cancelled"))
                        }
                        res = child_agent_ref.run(p_ref, c_ref) => res,
                    }
                }).await
            });
            let run_res_timeout = tokio::time::timeout(std::time::Duration::from_secs(300), run_res_fut);
            match with_spinner(&spinner_msg, run_res_timeout).await {
                Ok(res) => res,
                Err(_) => Err(anyhow!("Subagent execution timed out after 5 minutes")),
            }
        };
        cancel_guard.completed = true;

        if let Some(ref schema) = json_schema {
            let mut attempts = 0;
            while let Ok(ref mut res) = run_res {
                    let text_output = res.content.trim();
                    let clean_json_str = if text_output.starts_with("```json") {
                        text_output.strip_prefix("```json").unwrap().strip_suffix("```").unwrap_or(text_output).trim()
                    } else if text_output.starts_with("```") {
                        text_output.strip_prefix("```").unwrap().strip_suffix("```").unwrap_or(text_output).trim()
                    } else {
                        text_output
                    };

                    let parsed_val: Result<Value, _> = serde_json::from_str(clean_json_str);
                    match parsed_val {
                        Ok(val) => {
                            match validate_schema(&val, schema) {
                                Ok(_) => {
                                    res.content = clean_json_str.to_string();
                                    break;
                                }
                                Err(e) => {
                                    if attempts >= 2 {
                                        run_res = Err(anyhow!("Subagent output failed schema validation: {}", e));
                                        break;
                                    }
                                    attempts += 1;
                                    crate::tui_println!(
                                        "{}▲ [Reflection] Subagent JSON schema validation failed: {}. Retrying attempt {} of 2...{}",
                                        AURA_GOLD, e, attempts, COLOR_RESET
                                    );
                                    let retry_prompt = format!(
                                        "Your previous response did not conform to the JSON Schema. Validation Error: {}\n\n\
                                        Please correct your response. Return ONLY the raw valid JSON matching the schema.",
                                        e
                                    );
                                    let p_ref = &retry_prompt;
                                    let c_ref = &child_session_id;
                                    let child_agent_ref = &child_agent;
                                    let run_res_fut = crate::config::loader::ACTIVE_WORKSPACE.scope(workspace_dir.clone(), async {
                                        DELEGATION_DEPTH.scope(current_depth + 1, async {
                                            tokio::select! {
                                                biased;
                                                _ = self.cancellation_token.wait_for_cancellation() => {
                                                    Err(anyhow!("Subagent task cancelled"))
                                                }
                                                res = child_agent_ref.run(p_ref, c_ref) => res,
                                            }
                                        }).await
                                    });
                                    let run_res_timeout = tokio::time::timeout(std::time::Duration::from_secs(300), run_res_fut);
                                    run_res = match with_spinner(&spinner_msg, run_res_timeout).await {
                                        Ok(r) => r,
                                        Err(_) => Err(anyhow!("Subagent execution timed out after 5 minutes")),
                                    };
                                }
                            }
                        }
                        Err(e) => {
                            if attempts >= 2 {
                                run_res = Err(anyhow!("Subagent output failed to parse as JSON: {}. Parse Error: {}", e, text_output));
                                break;
                            }
                            attempts += 1;
                            crate::tui_println!(
                                "{}▲ [Reflection] Subagent output is not valid JSON: {}. Retrying attempt {} of 2...{}",
                                AURA_GOLD, e, attempts, COLOR_RESET
                            );
                            let retry_prompt = format!(
                                "Your previous response was not valid JSON. Parse Error: {}\n\n\
                                Please correct your response. Return ONLY the raw valid JSON matching the schema.",
                                e
                            );
                            let p_ref = &retry_prompt;
                            let c_ref = &child_session_id;
                            let child_agent_ref = &child_agent;
                            let run_res_fut = crate::config::loader::ACTIVE_WORKSPACE.scope(workspace_dir.clone(), async {
                                DELEGATION_DEPTH.scope(current_depth + 1, async {
                                    tokio::select! {
                                        biased;
                                        _ = self.cancellation_token.wait_for_cancellation() => {
                                            Err(anyhow!("Subagent task cancelled"))
                                        }
                                        res = child_agent_ref.run(p_ref, c_ref) => res,
                                    }
                                }).await
                            });
                            let run_res_timeout = tokio::time::timeout(std::time::Duration::from_secs(300), run_res_fut);
                            run_res = match with_spinner(&spinner_msg, run_res_timeout).await {
                                Ok(r) => r,
                                Err(_) => Err(anyhow!("Subagent execution timed out after 5 minutes")),
                            };
                        }
                    }
            }
        }

        if has_branch {
            if run_res.is_ok() {
                match crate::tools::graph_memory::CommitDatabaseBranchTool.call(&serde_json::json!({})).await {
                    Ok(_) => crate::tui_println!("{}  ✓ Committed simulation space branch '{}'{}", EMERALD_GREEN, branch_id, COLOR_RESET),
                    Err(e) => tracing::warn!("Failed to commit database branch: {:?}", e),
                }
            } else {
                match crate::tools::graph_memory::RollbackDatabaseBranchTool.call(&serde_json::json!({})).await {
                    Ok(_) => crate::tui_println!("{}  ✓ Rolled back simulation space branch '{}'{}", AURA_GOLD, branch_id, COLOR_RESET),
                    Err(e) => tracing::warn!("Failed to rollback database branch: {:?}", e),
                }
            }
        }

        if run_res.is_ok() && workspace_dir != parent_dir {
            if let Err(e) = sync_changes_back(&workspace_dir, &parent_dir) {
                if !crate::agent::style::is_silent() {
                    let leaf_prefix = crate::agent::style::get_tree_prefix(true);
                    crate::tui_println!("{}{}{}↶ Failed to sync changes back to active workspace: {}{}", AURA_SLATE, leaf_prefix, AURA_GOLD, e, COLOR_RESET);
                }
            } else {
                if !crate::agent::style::is_silent() {
                    let leaf_prefix = crate::agent::style::get_tree_prefix(true);
                    crate::tui_println!("{}{}{}✓ Synchronized changes back to active workspace{}", AURA_SLATE, leaf_prefix, AURA_GREEN, COLOR_RESET);
                }
            }
        }

        match run_res {
            Ok(res) => {
                if !crate::agent::style::is_silent() {
                    let leaf_prefix = crate::agent::style::get_tree_prefix(true);
                    crate::tui_println!("{}{}{}✓ Complete{}", AURA_SLATE, leaf_prefix, AURA_GREEN, COLOR_RESET);
                }

                // Run evolution review
                let _ = run_evolution_review(&self.parent_provider, "subagent", &clean_goal, &clean_context, &res.content).await;

                Ok(serde_json::json!({
                    "status": "success",
                    "session_id": child_session_id,
                    "summary": res.content
                }))
            }
            Err(e) => {
                if !crate::agent::style::is_silent() {
                    let leaf_prefix = crate::agent::style::get_tree_prefix(true);
                    crate::tui_println!("{}{}{}✗{} Subagent execution failed: {}{}", AURA_SLATE, leaf_prefix, COLOR_RESET, ERROR_RED, e, COLOR_RESET);
                }
                Ok(serde_json::json!({
                    "status": "error",
                    "error": format!("Subagent execution failed: {:?}", e)
                }))
            }
        }
        }).await
    }
}

pub fn ensure_markdown_images(text: &str) -> String {
    let re = match regex::Regex::new(
        r"(?i)(file://[^\s\)\(]+\.(?:png|jpg|jpeg|webp|gif)|https?://[^\s\)\(]+\.(?:png|jpg|jpeg|webp|gif)|/[^\s\)\(]+\.(?:png|jpg|jpeg|webp|gif))",
    ) {
        Ok(r) => r,
        Err(_) => return text.to_string(),
    };

    let mut result = text.to_string();
    let mut matches: Vec<_> = re.find_iter(text).collect();
    matches.reverse();

    for mat in matches {
        let start = mat.start();
        let end = mat.end();
        let matched_str = mat.as_str();

        let mut already_formatted = false;
        if start > 0 {
            let before = &text[..start];
            if before.ends_with('(') || before.ends_with("](") {
                already_formatted = true;
            }
        }

        if !already_formatted {
            let replacement = format!("![]({})", matched_str);
            result.replace_range(start..end, &replacement);
        }
    }
    result
}

pub struct WorktreeGuard {
    pub parent_dir: std::path::PathBuf,
    pub worktree_dir: std::path::PathBuf,
    pub active: bool,
}

impl WorktreeGuard {
    pub fn new(parent_dir: std::path::PathBuf, worktree_dir: std::path::PathBuf) -> Self {
        Self {
            parent_dir,
            worktree_dir,
            active: true,
        }
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }
}

impl Drop for WorktreeGuard {
    fn drop(&mut self) {
        if self.active && self.worktree_dir != self.parent_dir {
            cleanup_isolated_workspace(&self.parent_dir, &self.worktree_dir);
        }
    }
}

fn dir_size(path: &std::path::Path) -> u64 {
    if !path.exists() {
        return 0;
    }
    if path.is_file() {
        return path.metadata().map(|m| m.len()).unwrap_or(0);
    }
    let mut size = 0;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            size += dir_size(&entry.path());
        }
    }
    size
}

fn enforce_disk_quota() {
    let worktrees_dir = crate::config::resolve_path("~/.openz/worktrees");
    if !worktrees_dir.exists() || !worktrees_dir.is_dir() {
        return;
    }

    let quota: u64 = 5 * 1024 * 1024 * 1024; // 5 GB
    let mut total_size = dir_size(&worktrees_dir);
    if total_size <= quota {
        return;
    }

    let mut worktrees = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&worktrees_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if name.starts_with("openz_worktree_") {
                    if let Ok(metadata) = std::fs::metadata(&path) {
                        if let Ok(modified) = metadata.modified() {
                            worktrees.push((path, modified));
                        }
                    }
                }
            }
        }
    }

    worktrees.sort_by(|a, b| a.1.cmp(&b.1));

    for (path, _) in worktrees {
        if total_size <= quota {
            break;
        }
        let size = dir_size(&path);
        if std::fs::remove_dir_all(&path).is_ok() {
            total_size = total_size.saturating_sub(size);
        }
    }
}

pub fn cleanup_stale_resources() {
    // 1. Run git worktree prune in current directory if it's a git repo
    let parent_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let git_check = std::process::Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(&parent_dir)
        .output();
    if let Ok(out) = git_check {
        if out.status.success() && String::from_utf8_lossy(&out.stdout).trim() == "true" {
            let _ = std::process::Command::new("git")
                .args(["worktree", "prune"])
                .current_dir(&parent_dir)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
        }
    }

    let ttl_seconds = 3600; // 1 hour TTL for active worktrees

    // 2. Clean dedicated directory (~/.openz/worktrees)
    let worktrees_dir = crate::config::resolve_path("~/.openz/worktrees");
    if worktrees_dir.exists() && worktrees_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&worktrees_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if name.starts_with("openz_worktree_") && is_older_than(&path, ttl_seconds) {
                        let _ = std::fs::remove_dir_all(&path);
                    }
                }
            }
        }
    }

    // 3. Clean legacy /tmp/openz_worktree_* directories
    let tmp_dir = std::env::temp_dir();
    if let Ok(entries) = std::fs::read_dir(&tmp_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if name.starts_with("openz_worktree_") && is_older_than(&path, ttl_seconds) {
                    let _ = std::fs::remove_dir_all(&path);
                }
            }
        }
    }

    let seven_days_in_seconds = 7 * 24 * 3600;

    // 4. Clean tool_outputs (~/.openz/tool_outputs)
    let tool_outputs_dir = crate::config::resolve_path("~/.openz/tool_outputs");
    if tool_outputs_dir.exists() && tool_outputs_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&tool_outputs_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && is_older_than(&path, seven_days_in_seconds) {
                    let _ = std::fs::remove_file(&path);
                }
            }
        }
    }

    // 5. Clean traces (~/.openz/traces)
    let traces_dir = crate::config::resolve_path("~/.openz/traces");
    if traces_dir.exists() && traces_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&traces_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && is_older_than(&path, seven_days_in_seconds) {
                    let _ = std::fs::remove_file(&path);
                }
            }
        }
    }

    // 6. Clean cron_logs (~/.openz/cron_logs)
    let cron_logs_dir = crate::config::resolve_path("~/.openz/cron_logs");
    if cron_logs_dir.exists() && cron_logs_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&cron_logs_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && is_older_than(&path, seven_days_in_seconds) {
                    let _ = std::fs::remove_file(&path);
                }
            }
        }
    }
}

fn is_older_than(path: &std::path::Path, seconds: u64) -> bool {
    if let Ok(metadata) = std::fs::metadata(path) {
        if let Ok(modified) = metadata.modified() {
            if let Ok(elapsed) = modified.elapsed() {
                return elapsed.as_secs() > seconds;
            }
        }
    }
    false
}

pub fn create_isolated_workspace(parent_dir: &std::path::Path) -> Result<std::path::PathBuf> {
    enforce_disk_quota();

    let worktrees_dir = crate::config::resolve_path("~/.openz/worktrees");
    if !worktrees_dir.exists() {
        let _ = std::fs::create_dir_all(&worktrees_dir);
    }
    let temp_dir = worktrees_dir.join(format!(
        "openz_worktree_{}",
        &uuid::Uuid::new_v4().to_string()[..8]
    ));

    // 1. Check if parent_dir is a git repository
    let git_check = std::process::Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(parent_dir)
        .output();

    let is_git = match git_check {
        Ok(out) => out.status.success() && String::from_utf8_lossy(&out.stdout).trim() == "true",
        Err(_) => false,
    };

    if is_git {
        // 2. Create git worktree
        let worktree_add = std::process::Command::new("git")
            .args(["worktree", "add", "--detach", temp_dir.to_str().unwrap()])
            .current_dir(parent_dir)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();

        match worktree_add {
            Ok(status) if status.success() => {
                // 3. Sync uncommitted changes (modified, added, deleted, untracked files)
                if let Ok(status_out) = std::process::Command::new("git")
                    .args(["status", "--porcelain"])
                    .current_dir(parent_dir)
                    .output()
                {
                    let stdout = String::from_utf8_lossy(&status_out.stdout);
                    for line in stdout.lines() {
                        if line.len() < 4 {
                            continue;
                        }
                        let status_code = &line[..2];
                        let file_path_str = &line[3..];

                        let file_path = if status_code.starts_with('R') {
                            if let Some(pos) = file_path_str.find(" -> ") {
                                &file_path_str[pos + 4..]
                            } else {
                                file_path_str
                            }
                        } else {
                            file_path_str
                        };

                        let src = parent_dir.join(file_path);
                        let dst = temp_dir.join(file_path);

                        if status_code.contains('D') {
                            let _ = std::fs::remove_file(&dst);
                        } else {
                            if src.exists() {
                                if let Some(parent) = dst.parent() {
                                    let _ = std::fs::create_dir_all(parent);
                                }
                                let _ = std::fs::copy(&src, &dst);
                            }
                        }
                    }
                }
                return Ok(temp_dir);
            }
            _ => {
                // If git worktree add fails, fallback to recursive copy
            }
        }
    }

    // Fallback: Copy workspace files recursively (skipping heavy dirs)
    std::fs::create_dir_all(&temp_dir)?;
    copy_dir_recursive_filtered(parent_dir, &temp_dir)?;
    Ok(temp_dir)
}

pub fn copy_dir_recursive_filtered(src: &std::path::Path, dst: &std::path::Path) -> Result<()> {
    if !src.exists() {
        return Ok(());
    }

    if src.is_dir() {
        let name = src.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name == "target"
            || name == "node_modules"
            || name == ".git"
            || name == ".fastembed_cache"
            || name == ".sediment"
            || name == "logs"
        {
            return Ok(());
        }

        std::fs::create_dir_all(dst)?;
        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            let entry_path = entry.path();
            let entry_name = entry_path.file_name().unwrap();
            copy_dir_recursive_filtered(&entry_path, &dst.join(entry_name))?;
        }
    } else {
        if let Ok(metadata) = src.symlink_metadata() {
            if metadata.file_type().is_file() {
                std::fs::copy(src, dst)?;
            }
        }
    }
    Ok(())
}

pub fn cleanup_isolated_workspace(parent_dir: &std::path::Path, worktree_dir: &std::path::Path) {
    let git_check = std::process::Command::new("git")
        .args(["worktree", "list"])
        .current_dir(parent_dir)
        .output();

    let is_worktree = match git_check {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            stdout.contains(worktree_dir.to_str().unwrap_or("____invalid____"))
        }
        Err(_) => false,
    };

    if is_worktree {
        let _ = std::process::Command::new("git")
            .args([
                "worktree",
                "remove",
                "--force",
                worktree_dir.to_str().unwrap(),
            ])
            .current_dir(parent_dir)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();

        let _ = std::process::Command::new("git")
            .args(["worktree", "prune"])
            .current_dir(parent_dir)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    } else {
        let _ = std::fs::remove_dir_all(worktree_dir);
    }
}

pub fn sync_changes_back(src_dir: &std::path::Path, dst_dir: &std::path::Path) -> Result<()> {
    let git_check = std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(src_dir)
        .output();

    if let Ok(status_out) = git_check {
        let stdout = String::from_utf8_lossy(&status_out.stdout);
        for line in stdout.lines() {
            if line.len() < 4 {
                continue;
            }
            let status_code = &line[..2];
            let file_path_str = &line[3..];

            let file_path = if status_code.starts_with('R') {
                if let Some(pos) = file_path_str.find(" -> ") {
                    &file_path_str[pos + 4..]
                } else {
                    file_path_str
                }
            } else {
                file_path_str
            };

            let src = src_dir.join(file_path);
            let dst = dst_dir.join(file_path);

            if status_code.contains('D') {
                let _ = std::fs::remove_file(&dst);
            } else {
                if src.exists() {
                    if let Some(parent) = dst.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    let _ = std::fs::copy(&src, &dst);
                }
            }
        }
    } else {
        copy_dir_recursive_filtered(src_dir, dst_dir)?;
    }
    Ok(())
}

pub async fn run_evolution_review(
    provider: &std::sync::Arc<dyn LLMProvider>,
    profile_name: &str,
    goal: &str,
    context: &str,
    summary: &str,
) -> Result<()> {
    let system_prompt = "You are a specialized Subagent Reviewer. Your task is to evaluate if a subagent successfully completed its task, and if so, extract any procedural skills or guidelines discovered during execution.\n\n\
        Review the subagent's goal, the context, and the summary of what it did and found.\n\n\
        Perform two tasks:\n\
        1. SUCCESS EVALUATION: Decide if the subagent succeeded in accomplishing the goal (true or false).\n\
        2. SKILL EXTRACTION: If the subagent succeeded, extract any reusable procedural guidelines, rules, tool usage lessons, or coding patterns it discovered. Avoid general descriptions; make them actionable instructions for future runs. Format the extracted guidelines in Markdown with a clear title (# Skill: ...), a description of when to use it, specific guidelines, and examples.\n\n\
        Provide your response as a raw JSON object with the following structure:\n\n\
        JSON Format:\n\
        {\n\
          \"success\": true,\n\
          \"skill_name\": \"cargo_check_workaround\",\n\
          \"skill_content\": \"# Skill: Cargo Check Workaround\\n\\nWhen cargo check fails with X, do Y...\"\n\
        }\n\n\
        Do not output any introductory or conversational text, only the raw JSON.";

    let user_prompt = format!(
        "Subagent Profile: {}\n\
         Goal: {}\n\
         Context: {}\n\
         Subagent Summary of Work:\n{}\n\n\
         Please review the above execution, evaluate success, and extract any reusable skills.",
        profile_name, goal, context, summary
    );

    let messages = vec![crate::session::Message {
        role: "user".to_string(),
        content: user_prompt,
        timestamp: Some(chrono::Utc::now().to_rfc3339()),
        extra: serde_json::Map::new(),
    }];

    let settings = crate::providers::GenerationSettings {
        temperature: 0.1,
        max_tokens: 1536,
        reasoning_effort: None,
    };

    let spinner_msg = format!(
        "{}◇ [Evolution] Evaluating subagent success & extracting skills...{}",
        AURA_PURPLE, COLOR_RESET
    );
    let resp = with_spinner(
        &spinner_msg,
        provider.chat(system_prompt, &messages, &[], &settings),
    )
    .await?;
    let content = resp
        .content
        .ok_or_else(|| anyhow!("No content returned from AI"))?;

    // Parse JSON
    let mut clean_json = content.trim();
    if clean_json.starts_with("```json") {
        clean_json = clean_json.strip_prefix("```json").unwrap();
    } else if clean_json.starts_with("```") {
        clean_json = clean_json.strip_prefix("```").unwrap();
    }
    if clean_json.ends_with("```") {
        clean_json = clean_suffix_ticks(clean_json);
    }
    let clean_json = clean_json.trim();

    #[derive(serde::Deserialize)]
    struct ReviewRes {
        success: bool,
        skill_name: String,
        skill_content: String,
    }

    if let Ok(review) = serde_json::from_str::<ReviewRes>(clean_json) {
        if review.success {
            let s_name = review.skill_name.trim().to_lowercase().replace(' ', "_");
            let s_content = review.skill_content.trim();
            if !s_name.is_empty() && !s_content.is_empty() {
                crate::agent::skills::save_subagent_skill(profile_name, &s_name, s_content)?;
                crate::tui_println!(
                    "{}✓ [Evolution] Extracted and saved skill '{}' for subagent '{}'{}",
                    EMERALD_GREEN,
                    s_name,
                    profile_name,
                    COLOR_RESET
                );
            }
        } else {
            crate::tui_println!(
                "{}▲ [Evolution] Subagent task evaluation: Unsuccessful. No skill files updated.{}",
                AURA_GOLD,
                COLOR_RESET
            );
        }
    }

    Ok(())
}

fn clean_suffix_ticks(s: &str) -> &str {
    if let Some(stripped) = s.strip_suffix("```") {
        stripped
    } else {
        s
    }
}
