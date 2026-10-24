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
            "grep_search",
            "ast_grep",
            "parallel_research",
            "evaluator_optimizer_loop",
            "orchestrate_workflow",
            "sequentialthinking",
            "analyze_graph",
            "summarize_reasoning",
            "reasoning_templates",
            "tool_catalog",
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
            "searchxyz_recall",
            "searchxyz_query_graph",
            "searchxyz_index_content",
            "searchxyz_index_relationship",
            "searchxyz_list_sources",
            "diagnose_system",
            "search_research",
            "rust_docs",
            "tool_catalog",
        ]),
        "architect" => Some(&[
            "read_file",
            "write_file",
            "list_dir",
            "find_files",
            "code_outline",
            "ast_grep",
            "db_inspector",
            "render_mermaid",
            "tool_catalog",
        ]),
        "git_ops_agent" => Some(&[
            "read_file",
            "list_dir",
            "git_manager",
            "tool_catalog",
        ]),
        "ast_searcher" => Some(&[
            "read_file",
            "list_dir",
            "find_files",
            "ast_grep",
            "code_outline",
            "grep_search",
            "tool_catalog",
        ]),
        "database_specialist" => Some(&[
            "read_file",
            "list_dir",
            "db_inspector",
            "db_write",
            "search_text",
            "create_database_branch",
            "commit_database_branch",
            "rollback_database_branch",
            "diagnose_system",
            "tool_catalog",
        ]),
        "browser_operator" => Some(&[
            "read_file",
            "list_dir",
            "web_fetch",
            "crawl_website",
            "obscura_browser",
            "gsd_browser",
            "tool_catalog",
        ]),
        "dependency_manager" => Some(&[
            "read_file",
            "write_file",
            "list_dir",
            "cargo_manager",
            "onpkg",
            "rust_docs",
            "tool_catalog",
        ]),
        "frontend_architect" => Some(&[
            "read_file",
            "write_file",
            "list_dir",
            "generate_image",
            "openmedia_create_svg",
            "openmedia_rasterize_svg",
            "tool_catalog",
        ]),
        "docs_lookup_agent" => Some(&[
            "read_file",
            "list_dir",
            "web_fetch",
            "web_search",
            "rust_docs",
            "docs_list_docsets",
            "tool_catalog",
        ]),
        "media_designer" => Some(&[
            "read_file",
            "write_file",
            "list_dir",
            "generate_image",
            "openmedia_create_icon",
            "openmedia_create_chart",
            "openmedia_create_svg",
            "openmedia_rasterize_svg",
            "render_mermaid",
            "openmedia_animate_generate_spinner",
            "create_animated_svg",
            "tool_catalog",
        ]),
        "sop_designer" => Some(&[
            "read_file",
            "write_file",
            "list_dir",
            "trigger_sop",
            "orchestrate_workflow",
            "workflow_memory",
            "tool_catalog",
        ]),
        "api_integrator" => Some(&[
            "read_file",
            "write_file",
            "list_dir",
            "web_fetch",
            "web_search",
            "exec_command",
            "tool_catalog",
        ]),
        "performance_tuner" => Some(&[
            "read_file",
            "list_dir",
            "system_info",
            "exec_command",
            "tool_catalog",
        ]),
        "communication_manager" => Some(&[
            "read_file",
            "list_dir",
            "check_port",
            "telegram_send_message",
            "telegram_send_document",
            "desktop_notify",
            "manage_whitelist",
            "tool_catalog",
        ]),
        "document_compiler" => Some(&[
            "read_file",
            "write_file",
            "list_dir",
            "find_files",
            "read_doc",
            "opendoc_create_docx",
            "opendoc_docx_add_paragraph",
            "opendoc_docx_add_table",
            "opendoc_create_xlsx",
            "opendoc_create_pptx",
            "opendoc_pptx_add_slide",
            "opendoc_search_document",
            "opendoc_chunk_for_embedding",
            "exec_command",
            "compile_template",
            "tool_catalog",
        ]),
        "presentation_designer" => Some(&[
            "read_file",
            "write_file",
            "list_dir",
            "find_files",
            "read_doc",
            "opendoc_create_pptx",
            "opendoc_pptx_add_slide",
            "openmedia_create_chart",
            "openmedia_rasterize_svg",
            "exec_command",
            "generate_image",
            "compile_template",
            "tool_catalog",
        ]),
        "code_synthesizer" => Some(&[
            "read_file",
            "write_file",
            "list_dir",
            "find_files",
            "onpkg",
            "code_outline",
            "cargo_manager",
            "patch_file",
            "replace_lines",
            "tool_catalog",
        ]),
        "summarizer_agent" => Some(&[
            "read_file",
            "write_file",
            "list_dir",
            "grep_search",
            "tool_catalog",
        ]),
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
            "tool_catalog",
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
            "patch_file",
            "replace_lines",
            "tool_catalog",
        ]),
        "reviewer" | "code_auditor" => Some(&[
            "read_file",
            "list_dir",
            "find_files",
            "code_outline",
            "ast_grep",
            "grep_search",
            "tool_catalog",
        ]),
        "debugger" => Some(&[
            "read_file",
            "write_file",
            "list_dir",
            "find_files",
            "code_outline",
            "grep_search",
            "exec_command",
            "cargo_manager",
            "patch_file",
            "replace_lines",
            "tool_catalog",
        ]),
        "test_engineer" => Some(&[
            "read_file",
            "write_file",
            "list_dir",
            "exec_command",
            "cargo_manager",
            "find_files",
            "code_outline",
            "tool_catalog",
        ]),
        "devops_agent" => Some(&[
            "read_file",
            "write_file",
            "list_dir",
            "find_files",
            "exec_command",
            "tool_catalog",
        ]),
        "refactor_agent" => Some(&[
            "read_file",
            "write_file",
            "list_dir",
            "find_files",
            "code_outline",
            "ast_grep",
            "grep_search",
            "patch_file",
            "replace_lines",
            "tool_catalog",
        ]),
        "memory_manager" => Some(&[
            "read_file",
            "write_file",
            "list_dir",
            "find_files",
            "store_memory",
            "recall_memory",
            "clear_memory",
            "delete_memory",
            "update_memory",
            "archive_research",
            "search_research",
            "knowledge_source",
            "research_brief",
            "workflow_memory",
            "set_working_memory",
            "get_working_memory",
            "evict_expired_working_memory",
            "promote_working_memory",
            "smart_store",
            "extract_and_store_facts",
            "proactive_recall",
            "invalidate_fact",
            "forget_memory",
            "query_fact_history",
            "query_as_of",
            "search_text",
            "hybrid_search",
            "memory_stats",
            "compact_memories",
            "create_entities",
            "create_relations",
            "add_observations",
            "delete_entities",
            "delete_observations",
            "delete_relations",
            "read_graph",
            "search_nodes",
            "open_nodes",
            "traverse_graph",
            "find_path",
            "analyze_graph_communities",
            "detect_and_resolve_conflicts",
            "tool_catalog",
        ]),
        "self_improvement" | "skill_improvement" => Some(&[
            "read_file",
            "write_file",
            "list_dir",
            "find_files",
            "curate_skill",
            "workflow_memory",
            "store_memory",
            "recall_memory",
            "proactive_recall",
            "smart_store",
            "log_execution_episode",
            "log_reflection",
            "retrieve_episodic_reflections",
            "record_tool_performance",
            "query_tool_performance",
            "openz_inventory",
            "diagnose_system",
            "tool_catalog",
        ]),
        "openz_maintainer" => Some(&[
            "read_file",
            "write_file",
            "list_dir",
            "exec_command",
            "cargo_manager",
            "tool_catalog",
        ]),
        "mcps_manager" => Some(&[
            "read_file",
            "write_file",
            "list_dir",
            "manage_mcp",
            "exec_command",
            "tool_catalog",
        ]),
        "vision_agent" => Some(&[
            "read_file",
            "list_dir",
            "find_files",
            "generate_image",
            "read_doc",
            "openmedia_rasterize_svg",
            "tool_catalog",
        ]),
        "skill_creator" => Some(&[
            "read_file",
            "write_file",
            "list_dir",
            "exec_command",
            "cargo_manager",
            "curate_skill",
            "workflow_memory",
            "openz_inventory",
            "tool_catalog",
        ]),
        "documentation_agent" => Some(&[
            "read_file",
            "write_file",
            "list_dir",
            "find_files",
            "tool_catalog",
        ]),
        "diagram_designer" => Some(&[
            "read_file",
            "write_file",
            "list_dir",
            "render_mermaid",
            "openmedia_create_svg",
            "openmedia_create_chart",
            "openmedia_create_icon",
            "openmedia_rasterize_svg",
            "tool_catalog",
        ]),
        "video_animator" => Some(&[
            "read_file",
            "write_file",
            "list_dir",
            "generate_video",
            "html_to_video",
            "create_animated_svg",
            "openmedia_video_create",
            "openmedia_video_preview",
            "openmedia_rasterize_svg",
            "tool_catalog",
        ]),
        _ => None,
    }
}

/// Collects all tool names mentioned in static subagent allowlists, sorted and deduplicated.
pub fn all_static_subagent_allowlist_tools() -> Vec<&'static str> {
    let mut out = Vec::new();
    for tools in crate::subagents::DEFAULT_SUBAGENT_NAMES
        .iter()
        .filter_map(|profile| static_allowlist_for_subagent(profile))
    {
        out.extend_from_slice(tools);
    }
    out.sort_unstable();
    out.dedup();
    out
}

fn is_universal_subagent_meta_tool(tool_name: &str) -> bool {
    matches!(
        crate::tools::canonical_tool_name(tool_name).as_str(),
        "tool_catalog"
            | "request_tool_scope"
            | "retrieve_original"
            | "search_cache"
            | "scope_context"
    )
}

/// Filters a slice of tools for a specialized subagent profile.
///
/// Precedence:
/// 1. If the profile configures `"tools"` or `"allowed_tools"` in its extra metadata, filters by those names.
/// 2. If the profile configures `"domains"` or `"allowed_domains"` in its extra metadata, filters by matching domain names.
/// 3. Otherwise, applies the built-in `static_allowlist_for_subagent(&profile.name)`.
/// 4. If none matches (unrestricted custom subagent), allows all parent tools except `send_remote_input`.
/// 5. Automatically retains universal meta/CCR tools (`tool_catalog`, `request_tool_scope`, `retrieve_original`, `search_cache`, `scope_context`) so any subagent can discover tools and inspect compressed outputs.
pub fn filter_tools_for_profile(
    profile: &SubagentProfile,
    all_tools: &[Arc<dyn Tool>],
) -> Vec<Arc<dyn Tool>> {
    let explicit_tools: Option<Vec<String>> = profile
        .extra
        .get("tools")
        .or_else(|| profile.extra.get("allowed_tools"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|val| val.as_str().map(str::to_string))
                .collect()
        });

    let explicit_domains: Option<Vec<String>> = profile
        .extra
        .get("domains")
        .or_else(|| profile.extra.get("allowed_domains"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|val| val.as_str().map(|s| s.to_lowercase()))
                .collect()
        });

    let mut filtered: Vec<Arc<dyn Tool>> = match (
        explicit_tools.as_ref(),
        explicit_domains.as_ref(),
        static_allowlist_for_subagent(&profile.name),
    ) {
        (Some(tool_names), _, _) => all_tools
            .iter()
            .filter(|tool| {
                tool_names
                    .iter()
                    .any(|name| crate::tools::tool_names_match(name, tool.name()))
                    || is_universal_subagent_meta_tool(tool.name())
            })
            .cloned()
            .collect(),
        (None, Some(domains), _) => all_tools
            .iter()
            .filter(|tool| {
                let d = tool.metadata().domain.to_lowercase();
                domains.iter().any(|dom| dom == &d)
                    || matches!(tool.name(), "read_file" | "list_dir" | "write_file")
                    || is_universal_subagent_meta_tool(tool.name())
            })
            .cloned()
            .collect(),
        (None, None, Some(allowed)) => all_tools
            .iter()
            .filter(|tool| {
                allowed
                    .iter()
                    .any(|allowed_name| crate::tools::tool_names_match(allowed_name, tool.name()))
                    || is_universal_subagent_meta_tool(tool.name())
            })
            .cloned()
            .collect(),
        (None, None, None) => all_tools.to_vec(),
    };

    filtered.retain(|t| t.name() != "send_remote_input");
    filtered
}

/// Filters a slice of tools for a specialized subagent based on its static allowlist.
/// Always prunes dangerous/interactive tools such as `send_remote_input`.
pub fn filter_tools_for_subagent(
    subagent_name: &str,
    all_tools: &[Arc<dyn Tool>],
) -> Vec<Arc<dyn Tool>> {
    let dummy_profile = SubagentProfile {
        name: subagent_name.to_string(),
        description: String::new(),
        system_prompt: String::new(),
        model: None,
        fallbacks: None,
        extra: serde_json::Map::new(),
    };
    filter_tools_for_profile(&dummy_profile, all_tools)
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

fn push_unique_trimmed_model(models_to_try: &mut Vec<String>, raw_model: &str) {
    let trimmed = raw_model.trim();
    match !trimmed.is_empty() && !models_to_try.iter().any(|existing| existing == trimmed) {
        true => models_to_try.push(trimmed.to_string()),
        false => {}
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
    match profile.model.as_deref() {
        Some(m) => push_unique_trimmed_model(&mut models_to_try, m),
        None => {}
    }

    // 2. Add fallback models from profile overrides
    for fallback in profile.fallbacks.iter().flatten() {
        push_unique_trimmed_model(&mut models_to_try, fallback);
    }

    // 3. If no profile overrides were specified, populate with system dynamic fallbacks for this subagent role
    match profile.model.is_none() && profile.fallbacks.is_none() {
        true => {
            for fallback in config.get_dynamic_fallbacks(&profile.name) {
                push_unique_trimmed_model(&mut models_to_try, &fallback);
            }
        }
        false => {}
    }

    // 4. Finally, append our main agent model as the absolute last resort fallback
    let default_model = config.agents.defaults.model.clone();
    match models_to_try.iter().position(|m| m == &default_model) {
        Some(pos) => {
            models_to_try.remove(pos);
            models_to_try.push(default_model);
        }
        None => {
            models_to_try.push(default_model);
        }
    }
    super::limit_subagent_models_to_try(&mut models_to_try);
    models_to_try
}

#[cfg(test)]
#[path = "allowlist_tests.rs"]
mod tests;
