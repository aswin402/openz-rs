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
    pub allowed_names: BTreeSet<String>,
    pub max_visible_tools: usize,
    pub reasons: Vec<&'static str>,
}

#[derive(Debug, Clone)]
#[derive(Default)]
pub struct ToolScopeEngine {
}


impl ToolScopeEngine {
    pub fn decide(&self, decision: &IntentDecision, max_visible_tools: usize) -> ToolScopeDecision {
        let packs = pack_for_intent(decision.intent).to_vec();
        let mut allowed_names = BTreeSet::new();
        for pack in &packs {
            for spec in crate::tools::all_tool_specs() {
                if spec.packs.iter().any(|label| *label == pack_label(*pack)) {
                    allowed_names.insert(spec.name.to_string());
                }
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
    if let Some(spec) = crate::tools::tool_spec(name) {
        return packs.iter().any(|pack| {
            spec.packs.iter().any(|label| *label == pack_label(*pack))
        });
    }

    packs.iter().any(|pack| match pack {
        ToolPack::Core => false,
        ToolPack::RepoRead => {
            matches!(metadata.domain, "filesystem" | "code" | "git") && !metadata.writes_disk
        }
        ToolPack::RepoWrite => {
            matches!(metadata.domain, "filesystem" | "git") && metadata.writes_disk
        }
        ToolPack::LocalExec => {
            matches!(metadata.domain, "shell" | "code")
        }
        ToolPack::WebResearch => {
            matches!(metadata.domain, "web" | "search") || name.contains("browser")
        }
        ToolPack::Memory => matches!(metadata.domain, "memory" | "context" | "reasoning"),
        ToolPack::Cron => metadata.domain == "cron",
        ToolPack::Subagent => metadata.domain == "subagent",
        ToolPack::Orchestrator => false,
        ToolPack::Media => matches!(metadata.domain, "media" | "document"),
    })
}

fn pack_label(pack: ToolPack) -> &'static str {
    match pack {
        ToolPack::Core => "core",
        ToolPack::RepoRead => "repo_read",
        ToolPack::RepoWrite => "repo_write",
        ToolPack::LocalExec => "local_exec",
        ToolPack::WebResearch => "web_research",
        ToolPack::Memory => "memory",
        ToolPack::Cron => "cron",
        ToolPack::Subagent => "subagent",
        ToolPack::Orchestrator => "orchestrator",
        ToolPack::Media => "media",
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
