//! `opencaveman` — Native Rust terseness-driven token compression engine for OpenZ.
//!
//! Provides data-driven `caveman` intensity levels, ASD-STE100-inspired controlled
//! grammar directives, deterministic Auto-Clarity safety classification, AST-verified
//! Markdown & memory compression with out-of-tree backups, commit/review formatters,
//! built-in procedural skills, `cavecrew` subagent profiles, and token-savings telemetry.

pub mod ast_guard;
pub mod compressor;
pub mod level;
pub mod policy;
pub mod skills;
pub mod stats;
pub mod workflows;

pub use ast_guard::{
    extract_structural_anchors, scan_secret_exposure, verify_compression_fidelity, FidelityReport,
    SecretRejection, StructuralManifest,
};
pub use compressor::{
    compress_file_with_backup, compress_markdown_deterministic, list_backups, restore_backup,
    validate_or_fallback_compression, BackupEntry, BackupStore, CompressionOutcome,
};
pub use level::{
    all_level_descriptors, normalize_level_slug, CavemanConfigState, CavemanLevel,
    CavemanLevelDescriptor, CavemanScriptFamily,
};
pub use policy::{
    classify_auto_clarity, compile_dynamic_auto_clarity, compile_static_directive,
    detect_command_intent, AutoClarityReason, AutoClaritySignal, CavemanCommandIntent,
};
pub use skills::{
    builtin_caveman_skills, cavecrew_subagent_specs, BuiltinSkillSpec, CavecrewProfileSpec,
};
pub use stats::{
    estimate_tokens, format_compact_savings, load_stats_ledger, record_compression_savings,
    record_turn_savings, reset_stats_ledger, CavemanStatsLedger, CavemanStatsSnapshot,
};
pub use workflows::{
    analyze_diff_for_commit, analyze_diff_for_review, normalize_review_output,
    validate_and_format_commit, CommitAnalysis, DiffHunkMap, ReviewFinding, ValidatedCommitMessage,
};

#[cfg(test)]
mod tests;
