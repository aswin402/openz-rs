//! Subagent profile tool allowlisting, capability filtering, workspace requirement, and model candidate resolution.

use crate::config::schema::Config;
use crate::subagents::SubagentProfile;
use crate::tools::Tool;
use std::sync::Arc;

/// Returns the static list of tool names allowed for a given subagent profile name,
/// or `None` if the profile has unrestricted tool access.
pub fn static_allowlist_for_subagent(subagent_name: &str) -> Option<&'static [&'static str]> {
    match subagent_name {
        "planner" => Some(&[
            "read_file",
            "list_dir",
            "find_files",
            "code_outline",
            "parallel_research",
            "evaluator_optimizer_loop",
        ]),
        "researcher" => Some(&[
            "read_file",
            "list_dir",
            "find_files",
            "web_fetch",
            "web_search",
            "read_doc",
            "semantic_search",
            "crawl_website",
            "obscura_browser",
        ]),
        "architect" => Some(&[
            "read_file",
            "write_file",
            "list_dir",
            "find_files",
            "code_outline",
            "ast_grep",
            "db_inspector",
        ]),
        "git_ops_agent" => Some(&["read_file", "list_dir", "git_manager"]),
        "ast_searcher" => Some(&[
            "read_file",
            "list_dir",
            "find_files",
            "ast_grep",
            "code_outline",
            "grep_search",
        ]),
        "database_specialist" => Some(&["read_file", "list_dir", "db_inspector"]),
        "browser_operator" => Some(&[
            "read_file",
            "list_dir",
            "web_fetch",
            "crawl_website",
            "obscura_browser",
        ]),
        "dependency_manager" => Some(&[
            "read_file",
            "write_file",
            "list_dir",
            "cargo_manager",
            "onpkg",
        ]),
        "frontend_architect" => Some(&["read_file", "write_file", "list_dir", "generate_image"]),
        "docs_lookup_agent" => Some(&[
            "read_file",
            "list_dir",
            "web_fetch",
            "web_search",
            "rust_docs",
        ]),
        "media_designer" => Some(&["read_file", "write_file", "list_dir", "generate_image"]),
        "sop_designer" => Some(&["read_file", "write_file", "list_dir"]),
        "api_integrator" => Some(&[
            "read_file",
            "write_file",
            "list_dir",
            "web_fetch",
            "web_search",
            "exec_command",
        ]),
        "performance_tuner" => Some(&["read_file", "list_dir", "system_info", "exec_command"]),
        "communication_manager" => Some(&["read_file", "list_dir", "check_port"]),
        "document_compiler" => Some(&[
            "read_file",
            "write_file",
            "list_dir",
            "find_files",
            "read_doc",
            "exec_command",
            "compile_template",
        ]),
        "presentation_designer" => Some(&[
            "read_file",
            "write_file",
            "list_dir",
            "find_files",
            "exec_command",
            "generate_image",
            "compile_template",
        ]),
        "code_synthesizer" => Some(&[
            "read_file",
            "write_file",
            "list_dir",
            "find_files",
            "onpkg",
            "code_outline",
            "cargo_manager",
        ]),
        "summarizer_agent" => Some(&["read_file", "write_file", "list_dir", "grep_search"]),
        "automation_agent" => Some(&[
            "read_file",
            "write_file",
            "list_dir",
            "find_files",
            "gsd_browser",
            "obscura_browser",
            "crawl_website",
            "web_fetch",
            "schedule_job",
            "list_jobs",
            "remove_job",
            "pause_job",
            "resume_job",
            "get_job",
            "get_job_logs",
            "run_job_now",
            "exec_command",
            "manage_mcp",
        ]),
        "coding_agent" => Some(&[
            "read_file",
            "write_file",
            "list_dir",
            "find_files",
            "code_outline",
            "ast_grep",
            "grep_search",
            "exec_command",
            "cargo_manager",
        ]),
        "reviewer" | "code_auditor" => Some(&[
            "read_file",
            "list_dir",
            "code_outline",
            "ast_grep",
            "grep_search",
        ]),
        "debugger" => Some(&[
            "read_file",
            "write_file",
            "list_dir",
            "code_outline",
            "grep_search",
            "exec_command",
            "cargo_manager",
        ]),
        "test_engineer" => Some(&[
            "read_file",
            "write_file",
            "list_dir",
            "exec_command",
            "cargo_manager",
        ]),
        "devops_agent" => Some(&["read_file", "write_file", "list_dir", "exec_command"]),
        "refactor_agent" => Some(&[
            "read_file",
            "write_file",
            "list_dir",
            "code_outline",
            "ast_grep",
            "grep_search",
        ]),
        "memory_manager" | "self_improvement" | "skill_improvement" => {
            Some(&["read_file", "write_file", "list_dir", "find_files"])
        }
        "openz_maintainer" => Some(&[
            "read_file",
            "write_file",
            "list_dir",
            "exec_command",
            "cargo_manager",
        ]),
        "mcps_manager" => Some(&[
            "read_file",
            "write_file",
            "list_dir",
            "manage_mcp",
            "exec_command",
        ]),
        "vision_agent" => Some(&[
            "read_file",
            "list_dir",
            "find_files",
            "generate_image",
            "read_doc",
        ]),
        "skill_creator" => Some(&[
            "read_file",
            "write_file",
            "list_dir",
            "exec_command",
            "cargo_manager",
        ]),
        "documentation_agent" => Some(&["read_file", "write_file", "list_dir", "find_files"]),
        "diagram_designer" => Some(&[
            "read_file",
            "write_file",
            "list_dir",
            "openmedia_diagram_generate_mermaid",
        ]),
        "video_animator" => Some(&[
            "read_file",
            "write_file",
            "list_dir",
            "openmedia_video_create",
            "openmedia_video_preview",
        ]),
        _ => None,
    }
}

/// Collects all tool names mentioned in static subagent allowlists, sorted and deduplicated.
pub fn all_static_subagent_allowlist_tools() -> Vec<&'static str> {
    let mut out = Vec::new();
    for profile in crate::subagents::DEFAULT_SUBAGENT_NAMES {
        if let Some(tools) = static_allowlist_for_subagent(profile) {
            out.extend_from_slice(tools);
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// Filters a slice of tools for a specialized subagent based on its static allowlist.
/// Always prunes dangerous/interactive tools such as `send_remote_input`.
pub fn filter_tools_for_subagent(
    subagent_name: &str,
    all_tools: &[Arc<dyn Tool>],
) -> Vec<Arc<dyn Tool>> {
    let allowed_names = static_allowlist_for_subagent(subagent_name);

    let mut filtered: Vec<Arc<dyn Tool>> = if let Some(allowed) = allowed_names {
        all_tools
            .iter()
            .filter(|tool| {
                allowed
                    .iter()
                    .any(|allowed_name| crate::tools::tool_names_match(allowed_name, tool.name()))
            })
            .cloned()
            .collect()
    } else {
        all_tools.to_vec()
    };
    filtered.retain(|t| t.name() != "send_remote_input");
    filtered
}

/// Determines whether a subagent profile requires an isolated git worktree / scratch workspace.
pub fn profile_needs_workspace(profile_name: &str) -> bool {
    match profile_name {
        "orchestrator" | "architect" | "git_ops_agent" | "dependency_manager" |
        "frontend_architect" | "media_designer" | "sop_designer" | "api_integrator" |
        "performance_tuner" | "document_compiler" | "presentation_designer" |
        "code_synthesizer" | "automation_agent" | "coding_agent" | "debugger" |
        "test_engineer" | "devops_agent" | "refactor_agent" | "openz_maintainer" |
        "mcps_manager" => true,
        _ => false, // Skip isolated workspace setup for read-only, analytical, and config-focused agents
    }
}

/// Builds the ordered list of LLM models to try for a given subagent profile run,
/// cascading from explicit profile overrides, fallback models, role dynamic fallbacks,
/// and finally the default agent model as last resort.
pub fn delegate_profile_models_to_try(
    config: &Config,
    profile: &SubagentProfile,
) -> Vec<String> {
    let mut models_to_try = Vec::new();

    // 1. Add primary model from profile override
    if let Some(m) = &profile.model {
        if !m.trim().is_empty() {
            models_to_try.push(m.trim().to_string());
        }
    }

    // 2. Add fallback models from profile overrides
    if let Some(fallbacks) = &profile.fallbacks {
        for fallback in fallbacks {
            if !fallback.trim().is_empty() && !models_to_try.contains(&fallback.trim().to_string()) {
                models_to_try.push(fallback.trim().to_string());
            }
        }
    }

    // 3. If no profile overrides were specified, populate with system dynamic fallbacks for this subagent role
    if profile.model.is_none() && profile.fallbacks.is_none() {
        let dynamic_fallbacks = config.get_dynamic_fallbacks(&profile.name);
        for fallback in dynamic_fallbacks {
            if !models_to_try.contains(&fallback) {
                models_to_try.push(fallback);
            }
        }
    }

    // 4. Finally, append our main agent model as the absolute last resort fallback
    let default_model = config.agents.defaults.model.clone();
    if !models_to_try.contains(&default_model) {
        models_to_try.push(default_model);
    } else {
        // Move the default model to the end of the list if it is already present
        if let Some(pos) = models_to_try.iter().position(|m| m == &default_model) {
            models_to_try.remove(pos);
            models_to_try.push(default_model);
        }
    }
    super::limit_subagent_models_to_try(&mut models_to_try);
    models_to_try
}

#[cfg(test)]
#[path = "allowlist_tests.rs"]
mod tests;
