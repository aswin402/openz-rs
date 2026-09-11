//! # Native Tool Subsystem Architecture
//!
//! OpenZ provides an in-process, high-performance native tool registry governed by
//! fine-grained security policies, argument normalizers, and dynamic intent scoping.
//!
//! ## Subsystem Organization
//!
//! 1. **Core Tool Engine**:
//!    - [`arguments`]: Parameter normalization across casing styles and aliases.
//!    - [`metadata`]: Classification, `ToolRisk`, approval requirements, and domain tags.
//!    - [`defs`]: Curated static tool specifications (`STATIC_TOOL_DEFS`).
//!    - [`registry`]: Canonical registration deduplication and static tool drift validation.
//!    - [`routing`]: Prompt intent scoring and dynamic domain activation.
//!    - [`resource_policy`]: Execution resource quotas, sandbox policy, and syscall limits.
//!    - [`scope_engine`]: Dynamic 128-tool payload context budgeting per model turn.
//!
//! 2. **Grouped Domain Subsystems (11 Subdirectories)**:
//!    - [`browser`]: CDP, Firefox, and Obscura browser automation.
//!    - [`graph_memory`]: Entity/relation/observation graph memory & SQLite database branching.
//!    - [`headroom`]: Context compression and original retrieval (CCR).
//!    - [`memory_extra`]: Working memory, episodic recall, smart storage, and reflection telemetry.
//!    - [`opendoc`]: In-process PDF, DOCX, XLSX, and PPTX manipulation and OCR.
//!    - [`openmedia`]: SVG animation, video rendering, image transforms, and timelines.
//!    - [`searchxyz`]: Multi-engine web search, deep research, and sitemap crawling.
//!    - [`self_management`]: Autonomous diagnostics, session recovery, tool inventory, and skills.
//!    - [`sequential_thinking`]: Structured multi-step hypothesis and reasoning graph engine.
//!    - [`shared_memory`]: Team-shared persistent knowledge memory and research briefs.
//!    - [`subagent`]: Delegation, subagent profiles, and cooperative cancellation tokens.
//!
//! 3. **Loose Native Tools**:
//!    - Organized by operational domain: filesystem, shell/execution, code intelligence,
//!      web/network, system/utility, automation/messaging, visuals, and MCP bridging.
//!
//! See [`src/tools/README.md`](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/openz/src/tools/README.md)
//! for full architectural documentation and registration guidelines.

use anyhow::Result;

// ─── Core Tool Engine Modules ───
pub mod arguments;
pub mod metadata;
pub mod registry;
pub(crate) mod routing;

pub use metadata::{
    canonical_tool_name, compact_presentation_name, normalize_tool_name, presentation_name,
    tool_names_match, ToolMetadata, ToolRisk, ToolSpec,
};
pub use registry::*;

pub const MIN_TOOL_TIMEOUT_SECS: u64 = 5;
pub const MAX_TOOL_TIMEOUT_SECS: u64 = 1_800;

pub fn clamp_tool_timeout_secs(timeout_secs: u64) -> u64 {
    timeout_secs.clamp(MIN_TOOL_TIMEOUT_SECS, MAX_TOOL_TIMEOUT_SECS)
}

pub fn to_snake_case(s: &str) -> String {
    arguments::to_snake_case(s)
}

pub fn normalize_tool_args(args: &serde_json::Value) -> serde_json::Value {
    arguments::normalize_tool_args(args)
}

#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> serde_json::Value;
    fn metadata(&self) -> ToolMetadata {
        ToolMetadata::infer(self.name())
    }
    async fn call(&self, arguments: &serde_json::Value) -> Result<serde_json::Value>;
}

pub mod defs;
pub use defs::{all_tool_specs, static_tool_def_names, tool_spec, StaticToolDef, STATIC_TOOL_DEFS};

// ─────────────────────────────────────────────────────────────────────────────
// 1. Core Tool Subsystem Engine Modules
// ─────────────────────────────────────────────────────────────────────────────

/// Resource quotas, seccomp sandbox policy, and execution sandboxing.
pub mod resource_policy;

/// Dynamic turn-by-turn tool scoping and 128-tool model context budgeting.
pub mod scope_engine;
pub use scope_engine as scope;

// ─────────────────────────────────────────────────────────────────────────────
// 2. Grouped Domain Subsystems (11 Subdirectories)
// ─────────────────────────────────────────────────────────────────────────────

/// Browser automation and CDP integration (`firefox_browser`, `gsd_browser`, `obscura_browser`).
pub mod browser;
pub use browser::{
    broker as browser_broker,
    common as browser_common,
    firefox,
    gsd as gsd_browser,
    obscura,
    status as browser_status,
};

/// Entity-relation knowledge graph memory and database branching checkpoints.
pub mod graph_memory;

/// Context Compression & Retrieval (CCR) and token cache management (Headroom).
pub mod headroom;

/// Working memory, episodic recall, smart facts, and telemetry reflection.
pub mod memory_extra;

/// In-process PDF, DOCX, XLSX, and PPTX document parsing and manipulation.
pub mod opendoc;

/// SVG animation, timeline synthesis, image transforms, and programmatic video rendering.
pub mod openmedia;

/// Multi-engine web discovery, deep research, and repository indexing.
#[path = "searchxyz/mod.rs"]
pub mod searchxyz;

/// Autonomous self-diagnostics, inventory inspection, and session management.
pub mod self_management;

/// Multi-step structured hypothesis tracking and reasoning graph analysis.
pub mod sequential_thinking;

/// Team-shared persistent key-value and research brief knowledge storage.
pub mod shared_memory;

/// Subagent task delegation, evaluation loops, and cooperative cancellation tokens.
pub mod subagent;

// ─────────────────────────────────────────────────────────────────────────────
// 3. Loose Native Tools (Categorized by Functional Domain)
// ─────────────────────────────────────────────────────────────────────────────

// ── Filesystem & Documents ──
pub mod doc_reader;
pub mod filesystem;
pub mod notes;
pub mod watcher;

// ── Shell & Sandbox Execution ──
pub mod shell;
pub mod wasm_sandbox;

// ── Code Intelligence & Build Tooling ──
pub mod ast_grep;
pub mod cargo_manager;
pub mod compiler_auto_heal;
pub mod docs_mcp;
pub mod git_manager;
pub mod github;
pub mod github_mcp;
pub mod grep;
pub mod js_format;
pub mod onpkg;
pub mod outline;
pub mod rust_docs;

// ── Web & Network ──
pub mod crawl;
pub mod network;
pub mod social_search;
pub mod web;
pub mod web_search;

// ── System & Utilities ──
pub mod clipboard;
pub mod db_inspector;
pub mod desktop_notify;
pub mod device_inventory;
pub mod get_logs;
pub mod manage_whitelist;
pub mod open;
pub mod system_info;
pub mod task_manager;

// ── Automation, Orchestration & Messaging ──
pub mod cron;
pub mod orchestrator;
pub mod remote;
pub mod sop;
pub mod telegram_send;

// ── Media & Visuals ──
pub mod html_video;
pub mod image_generator;
pub mod mermaid;
pub mod svg_animator;
pub mod template_compiler;
pub mod video;

// ── Search & Vector Embeddings ──
pub mod semantic_search;

// ── MCP Protocol & Management ──
pub mod mcp;
pub mod mcp_manager;

#[cfg(test)]
mod static_def_tests {
    use super::*;

    /// Regression guard: these tools were previously referenced by misnamed
    /// match arms (html_video, crawl_site, svg_animator, mermaid) so their
    /// intended timeouts and network flags silently never applied.
    #[test]
    fn curated_defs_apply_intended_timeouts() {
        assert_eq!(
            ToolMetadata::infer("html_to_video").recommended_timeout_secs,
            Some(900)
        );
        assert_eq!(
            ToolMetadata::infer("generate_video").recommended_timeout_secs,
            Some(900)
        );
        assert_eq!(
            ToolMetadata::infer("crawl_website").recommended_timeout_secs,
            Some(600)
        );
        assert_eq!(
            ToolMetadata::infer("create_animated_svg").recommended_timeout_secs,
            Some(300)
        );
        assert_eq!(
            ToolMetadata::infer("generate_image").recommended_timeout_secs,
            Some(300)
        );
        assert_eq!(
            ToolMetadata::infer("render_mermaid").recommended_timeout_secs,
            Some(300)
        );
        assert_eq!(
            ToolMetadata::infer("semantic_search").recommended_timeout_secs,
            Some(300)
        );
        assert_eq!(
            ToolMetadata::infer("python_sandbox").recommended_timeout_secs,
            Some(180)
        );
    }

    #[test]
    fn crawl_website_is_a_network_tool() {
        let metadata = ToolMetadata::infer("crawl_website");
        assert!(metadata.uses_network);
        assert_eq!(metadata.domain, "web");
    }

    #[test]
    fn curated_defs_have_no_duplicate_names() {
        let names = static_tool_def_names();
        let unique: std::collections::BTreeSet<_> = names.iter().collect();
        assert_eq!(
            names.len(),
            unique.len(),
            "duplicate STATIC_TOOL_DEFS entry"
        );
    }
}
