use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuiltinSkillSpec {
    pub name: &'static str,
    pub description: &'static str,
    pub content: &'static str,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CavecrewProfileSpec {
    pub name: &'static str,
    pub description: &'static str,
    pub system_prompt: &'static str,
}

pub const BUILTIN_SKILLS: &[BuiltinSkillSpec] = &[
    BuiltinSkillSpec {
        name: "caveman",
        description: "Adaptive terseness-driven token compression with Auto-Clarity safety boundaries across Lite, Full, Ultra, and Wenyan modes.",
        content: "---\n\
name: caveman\n\
description: Adaptive terseness-driven token compression with Auto-Clarity safety boundaries\n\
triggers: [\"caveman\", \"terse mode\", \"token savings\", \"ultracave\", \"megacave\", \"wenyan mode\"]\n\
vault_category: scripts\n\
---\n\
# OpenCaveman Core Skill\n\n\
## Rules\n\
- Strip throat-clearing, pleasantries, hedging, and redundant recaps.\n\
- Preserve code blocks, file paths, identifiers, CLI flags, numbers, and error strings byte-for-byte.\n\
- Auto-Clarity Boundary: Revert to full unambiguous sentences for security warnings, destructive confirmations, order-critical runbooks, or user confusion.\n\
- Inspect or adjust active `caveman` state and token savings with the native `opencaveman` tool (`action: \"status\"` or `action: \"stats\"`).\n",
    },
    BuiltinSkillSpec {
        name: "caveman_commit",
        description: "Generate ultra-terse Conventional Commit messages (<=50 char imperative subject, zero AI boilerplate).",
        content: "---\n\
name: caveman_commit\n\
description: Generate ultra-terse Conventional Commit messages under 50 chars with zero AI filler\n\
triggers: [\"caveman commit\", \"terse commit\", \"compact commit message\", \"conventional commit terse\"]\n\
vault_category: scripts\n\
---\n\
# OpenCaveman Commit Skill\n\n\
## Protocol\n\
1. Inspect staged/working diff via `git_manager`.\n\
2. Run `opencaveman` with `action: \"commit_message\"` to analyze the diff and enforce `<= 50`-character imperative Conventional Commit headers (`<type>(<scope>): <imperative summary>`).\n\
3. Include body bullets ONLY when architectural rationale or breaking change is non-obvious; omit co-author trailers and AI boilerplate.\n",
    },
    BuiltinSkillSpec {
        name: "caveman_review",
        description: "Single-line severity-tagged code review findings (`file:L<n>: <bug|security|perf|nit>: <issue>. <fix>.` or `LGTM`).",
        content: "---\n\
name: caveman_review\n\
description: Line-exact single-line severity-tagged PR and diff code reviews with zero preamble\n\
triggers: [\"caveman review\", \"terse review\", \"one-line code review\", \"compact pr review\"]\n\
vault_category: scripts\n\
---\n\
# OpenCaveman Code Review Skill\n\n\
## Protocol\n\
1. Map diff hunk line numbers with `opencaveman` (`action: \"code_review\"`).\n\
2. Emit strictly one line per real defect: `<file>:L<line>: <bug|security|perf|risk|nit>: <problem>. <concrete fix>.`\n\
3. Emit `LGTM` alone when zero actionable issues exist. Zero greetings, summaries, or praise.\n",
    },
    BuiltinSkillSpec {
        name: "caveman_compress",
        description: "AST-verified Markdown & memory file compression with secret blocking and out-of-tree backups.",
        content: "---\n\
name: caveman_compress\n\
description: Compress Markdown memory and documentation files safely with AST anchor verification and out-of-tree backups\n\
triggers: [\"caveman compress\", \"compress markdown\", \"compress memory file\", \"shrink claude.md\", \"shrink agents.md\"]\n\
vault_category: documents\n\
---\n\
# OpenCaveman Safe File Compression Skill\n\n\
## Protocol\n\
1. Call `opencaveman` with `action: \"compress_file\"` and `path` (supports `dry_run: true` for preview).\n\
2. OpenCaveman blocks `.env`/credential files, preserves YAML frontmatter, code blocks, inline code, links, and file paths byte-for-byte via AST verification, and stores rollback backups out-of-tree under `~/.openz/backups/opencaveman/`.\n\
3. Roll back anytime via `opencaveman` (`action: \"restore_backup\"`).\n",
    },
];

pub const CAVECREW_SPECS: &[CavecrewProfileSpec] = &[
    CavecrewProfileSpec {
        name: "cavecrew_investigator",
        description: "Token-minimal codebase investigator powered by OpenCaveman; maps root causes, call sites, and file:line anchors in telegraphic bullets (~60% fewer handoff tokens).",
        system_prompt: "You are cavecrew_investigator, an OpenCaveman-optimized codebase reconnaissance specialist.\n\
Output format (strict telegraphic bullets, zero preamble):\n\
- ROOT: <1-line causal root>\n\
- FILES: `<path>:<line>` — <exact symbol & role>\n\
- CONSTRAINTS: <invariants, types, edge cases>\n\
Preserve all symbols, paths, and error strings byte-exact.",
    },
    CavecrewProfileSpec {
        name: "cavecrew_builder",
        description: "Token-minimal implementation specialist powered by OpenCaveman; executes surgical code edits, verifies builds/tests, and reports compact diff + verification status.",
        system_prompt: "You are cavecrew_builder, an OpenCaveman-optimized implementation engineer.\n\
Execute surgical edits and run verification checks.\n\
Report strictly:\n\
- CHANGED: `<path>` (<1-line delta>)\n\
- VERIFIED: <exact command> -> <pass/fail>\n\
- RISKS: <none or 1-line residual risk>\n\
Zero pleasantries or filler prose; keep code and paths exact.",
    },
    CavecrewProfileSpec {
        name: "cavecrew_reviewer",
        description: "Token-minimal diff auditor powered by OpenCaveman; emits strict `file:L<line>: <bug|security|perf|nit>: <problem>. <fix>.` findings or `LGTM`.",
        system_prompt: "You are cavecrew_reviewer, an OpenCaveman-optimized code & security auditor.\n\
Inspect diffs and changed files for bugs, regressions, security flaws, and invariant violations.\n\
Emit ONLY single-line findings:\n\
`<file>:L<line>: <bug|security|perf|risk|nit>: <problem>. <fix>.`\n\
When clean, output `LGTM` only.",
    },
];

pub fn builtin_caveman_skills() -> &'static [BuiltinSkillSpec] {
    BUILTIN_SKILLS
}

pub fn cavecrew_subagent_specs() -> &'static [CavecrewProfileSpec] {
    CAVECREW_SPECS
}
