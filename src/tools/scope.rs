use crate::agent::agent_loop::intent::{IntentDecision, TurnIntent};
use crate::tools::ToolMetadata;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ToolPack {
    Core,
    RepoRead,
    RepoWrite,
    LocalExec,
    WebResearch,
    Memory,
    Cron,
    Subagent,
    Orchestrator,
    Media,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolScopeDecision {
    pub packs: Vec<ToolPack>,
    pub allowed_names: BTreeSet<&'static str>,
    pub max_visible_tools: usize,
    pub reasons: Vec<&'static str>,
}

#[derive(Debug, Clone)]
pub struct ToolScopeEngine {
    core_tools: &'static [&'static str],
}

impl Default for ToolScopeEngine {
    fn default() -> Self {
        Self {
            core_tools: &[
                "request_tool_scope",
                "tool_catalog",
                "openz_inventory",
                "retrieve_original",
            ],
        }
    }
}

impl ToolScopeEngine {
    pub fn decide(&self, decision: &IntentDecision, max_visible_tools: usize) -> ToolScopeDecision {
        let mut allowed_names: BTreeSet<&'static str> = self.core_tools.iter().copied().collect();
        let packs = pack_for_intent(decision.intent).to_vec();
        for pack in &packs {
            for name in static_names_for_pack(*pack) {
                allowed_names.insert(name);
            }
        }

        ToolScopeDecision {
            packs,
            allowed_names,
            max_visible_tools,
            reasons: decision.reasons.clone(),
        }
    }
}

pub fn pack_for_intent(intent: TurnIntent) -> &'static [ToolPack] {
    match intent {
        TurnIntent::DirectAnswer => &[ToolPack::Core],
        TurnIntent::LocalRepoRead => &[ToolPack::Core, ToolPack::RepoRead],
        TurnIntent::LocalExecution => &[ToolPack::Core, ToolPack::RepoRead, ToolPack::LocalExec],
        TurnIntent::ExternalResearch => &[ToolPack::Core, ToolPack::WebResearch],
        TurnIntent::MemoryLookup => &[ToolPack::Core, ToolPack::Memory],
        TurnIntent::CronManagement => &[ToolPack::Core, ToolPack::Cron],
        TurnIntent::Orchestration => &[ToolPack::Core, ToolPack::Orchestrator, ToolPack::Subagent],
        TurnIntent::MediaOrDocument => &[ToolPack::Core, ToolPack::Media, ToolPack::RepoRead],
        TurnIntent::Unknown => &[ToolPack::Core, ToolPack::RepoRead],
    }
}

pub fn tool_matches_pack(name: &str, metadata: &ToolMetadata, packs: &[ToolPack]) -> bool {
    packs.iter().any(|pack| match pack {
        ToolPack::Core => matches!(
            name,
            "request_tool_scope" | "tool_catalog" | "openz_inventory" | "retrieve_original"
        ),
        ToolPack::RepoRead => {
            matches!(metadata.domain, "filesystem" | "code" | "git") && !metadata.writes_disk
        }
        ToolPack::RepoWrite => {
            matches!(metadata.domain, "filesystem" | "git") && metadata.writes_disk
        }
        ToolPack::LocalExec => {
            matches!(metadata.domain, "shell" | "code") || name == "cargo_manager"
        }
        ToolPack::WebResearch => {
            matches!(metadata.domain, "web" | "search") || name.contains("browser")
        }
        ToolPack::Memory => matches!(metadata.domain, "memory" | "context" | "reasoning"),
        ToolPack::Cron => {
            metadata.domain == "cron" || matches!(name, "schedule_job" | "list_jobs" | "remove_job")
        }
        ToolPack::Subagent => metadata.domain == "subagent" && name != "orchestrate_workflow",
        ToolPack::Orchestrator => name == "orchestrate_workflow",
        ToolPack::Media => matches!(metadata.domain, "media" | "document"),
    })
}

fn static_names_for_pack(pack: ToolPack) -> &'static [&'static str] {
    match pack {
        ToolPack::Core => &[
            "request_tool_scope",
            "tool_catalog",
            "openz_inventory",
            "retrieve_original",
        ],
        ToolPack::RepoRead => &[
            "list_dir",
            "read_file",
            "grep_search",
            "code_outline",
            "git_manager",
            "semantic_search",
        ],
        ToolPack::RepoWrite => &["write_file", "patch_file", "replace_lines", "git_manager"],
        ToolPack::LocalExec => &["exec_command", "python_sandbox", "cargo_manager"],
        ToolPack::WebResearch => &[
            "web_search",
            "web_fetch",
            "crawl_website",
            "searchxyz_browser_search",
            "searchxyz_search_web",
            "searchxyz_read_url",
            "searchxyz_search_and_read",
            "searchxyz_deep_research",
            "searchxyz_read_github_repo",
            "retrieve_original",
        ],
        ToolPack::Memory => &[
            "proactive_recall",
            "recall_memory",
            "search_memory",
            "read_graph",
            "open_nodes",
            "smart_store",
        ],
        ToolPack::Cron => &[
            "schedule_job",
            "list_jobs",
            "get_job",
            "get_job_logs",
            "pause_job",
            "resume_job",
            "run_job_now",
            "remove_job",
        ],
        ToolPack::Subagent => &[
            "delegate_task",
            "parallel_research",
            "evaluator_optimizer_loop",
        ],
        ToolPack::Orchestrator => &["orchestrate_workflow"],
        ToolPack::Media => &[
            "read_document",
            "generate_image",
            "html_to_video",
            "generate_video",
            "create_animated_svg",
            "render_mermaid",
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::agent_loop::intent::{IntentDecision, KnowledgePolicy, TurnIntent};

    #[test]
    fn local_repo_read_pack_includes_grep_and_read_not_web() {
        let decision = IntentDecision {
            intent: TurnIntent::LocalRepoRead,
            knowledge_policy: KnowledgePolicy::UseLocalContext,
            reasons: vec!["local_repo_query"],
        };
        let scope = ToolScopeEngine::default().decide(&decision, 12);
        assert!(scope.allowed_names.contains("grep_search"));
        assert!(scope.allowed_names.contains("read_file"));
        assert!(!scope.allowed_names.contains("web_fetch"));
    }

    #[test]
    fn external_research_pack_includes_web_and_source_tools() {
        let decision = IntentDecision {
            intent: TurnIntent::ExternalResearch,
            knowledge_policy: KnowledgePolicy::RequireLiveResearch,
            reasons: vec!["live_research_intent"],
        };
        let scope = ToolScopeEngine::default().decide(&decision, 12);
        assert!(scope.allowed_names.contains("web_fetch"));
        assert!(scope.allowed_names.contains("web_search"));
        assert!(scope.allowed_names.contains("retrieve_original"));
    }

    #[test]
    fn direct_answer_keeps_only_escape_hatch_and_inventory() {
        let decision = IntentDecision {
            intent: TurnIntent::DirectAnswer,
            knowledge_policy: KnowledgePolicy::ModelOk,
            reasons: vec!["direct_answer_default"],
        };
        let scope = ToolScopeEngine::default().decide(&decision, 12);
        assert!(scope.allowed_names.contains("request_tool_scope"));
        assert!(scope.allowed_names.contains("openz_inventory"));
        assert!(!scope.allowed_names.contains("exec_command"));
    }
}
