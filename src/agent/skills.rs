use anyhow::Result;
use rusqlite::{params, Connection};
use std::fs;
use std::path::PathBuf;

#[path = "skills_ranker.rs"]
pub mod ranker;

pub use ranker::{HIGH_CONFIDENCE_SKILL_SCORE, ScoredSkillMatch};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Skill {
    pub name: String,
    pub content: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SkillRevision {
    pub revision_id: i64,
    pub name: String,
    pub profile: Option<String>,
    pub content: String,
    pub created_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillView {
    pub name: String,
    pub content: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub triggers: Vec<String>,
    pub scope: String,
    pub profile: Option<String>,
    pub source: String,
    pub path: Option<String>,
    pub enabled: bool,
    pub is_protected: bool,
    pub use_count: i64,
    #[serde(default)]
    pub success_count: i64,
    #[serde(default)]
    pub revision_count: usize,
    pub created_at: Option<String>,
    pub last_used: Option<String>,
    pub validation_errors: Vec<String>,
}

pub fn strip_frontmatter(content: &str) -> &str {
    let trimmed = content.trim_start();
    match trimmed
        .strip_prefix("---")
        .and_then(|rest| rest.find("---").map(|idx| &rest[idx + 3..]))
    {
        Some(body) => body.trim_start(),
        None => content,
    }
}

impl Skill {
    pub fn description(&self) -> String {
        extract_frontmatter_field(&self.content, "description")
            .filter(|v| !v.is_empty())
            .or_else(|| {
                strip_frontmatter(&self.content)
                    .lines()
                    .map(str::trim)
                    .find(|trimmed| !trimmed.starts_with('#') && !trimmed.is_empty())
                    .map(|trimmed| trimmed.chars().take(160).collect::<String>())
            })
            .unwrap_or_else(|| {
                format!("Procedural guidelines for {}", self.name.replace('_', " "))
            })
    }

    pub fn triggers(&self) -> Vec<String> {
        let mut triggers = extract_frontmatter_list(&self.content, "triggers");
        match triggers.is_empty() {
            true => {
                triggers.extend(
                    self.name
                        .split(['_', '-'])
                        .filter(|w| w.len() > 2)
                        .map(|w| w.to_lowercase()),
                );
            }
            false => {}
        }
        triggers
    }

    pub fn vault_category(&self) -> Option<String> {
        extract_frontmatter_field(&self.content, "vault_category")
            .filter(|v| !v.is_empty())
            .or_else(|| {
                extract_frontmatter_field(&self.content, "category").filter(|v| !v.is_empty())
            })
    }
}

fn frontmatter_block(content: &str) -> Option<&str> {
    let trimmed = content.trim_start();
    let rest = trimmed.strip_prefix("---")?;
    let end_idx = rest.find("---")?;
    Some(&rest[..end_idx])
}

pub fn extract_frontmatter_field(content: &str, field: &str) -> Option<String> {
    let yaml_block = frontmatter_block(content)?;
    let field_prefix = format!("{}:", field);
    yaml_block.lines().find_map(|line| {
        line.trim()
            .strip_prefix(&field_prefix)
            .map(|val| val.trim().trim_matches(['"', '\'']).to_string())
    })
}

pub fn extract_frontmatter_list(content: &str, field: &str) -> Vec<String> {
    let yaml_block = match frontmatter_block(content) {
        Some(b) => b,
        None => return Vec::new(),
    };
    let field_prefix = format!("{}:", field);
    let mut items = Vec::new();
    let mut collecting_multiline = false;

    for raw_line in yaml_block.lines() {
        let trimmed = raw_line.trim();
        let is_indented = raw_line.starts_with(' ') || raw_line.starts_with('\t');

        match collecting_multiline {
            true => match is_indented || trimmed.starts_with('-') {
                true => {
                    let _ = trimmed
                        .strip_prefix('-')
                        .map(|item| item.trim().trim_matches(['"', '\'']).to_lowercase())
                        .filter(|item| !item.is_empty())
                        .map(|item| items.push(item));
                }
                false => {
                    collecting_multiline = false;
                }
            },
            false => {}
        }

        match !collecting_multiline {
            true => match trimmed.strip_prefix(&field_prefix) {
                Some(rhs) => {
                    let cleaned = rhs.trim();
                    match cleaned.is_empty() {
                        true => {
                            collecting_multiline = true;
                        }
                        false => {
                            let inner = cleaned
                                .strip_prefix('[')
                                .and_then(|s| s.strip_suffix(']'))
                                .unwrap_or(cleaned);
                            items.extend(
                                inner
                                    .split(',')
                                    .map(|part| {
                                        part.trim().trim_matches(['"', '\'']).to_lowercase()
                                    })
                                    .filter(|part| !part.is_empty()),
                            );
                        }
                    }
                }
                None => {}
            },
            false => {}
        }
    }
    items
}

pub fn get_user_profile_path() -> PathBuf {
    crate::agent::core_memory::get_user_profile_path()
}

pub fn load_user_profile() -> String {
    crate::agent::core_memory::load_user_profile()
}

pub fn update_user_profile(new_preferences: &str) -> Result<()> {
    crate::agent::core_memory::update_user_profile(new_preferences)
}

pub fn validate_skill_metadata(name: &str, content: &str) -> Vec<String> {
    let mut errors = Vec::new();
    let trimmed = content.trim_start();
    let checks: [(bool, &str); 5] = [
        (name.trim().is_empty(), "Skill name is required."),
        (
            name.chars()
                .any(|c| !c.is_ascii_lowercase() && !c.is_ascii_digit() && c != '_' && c != '-'),
            "Skill name must use lowercase letters, numbers, dashes, or underscores.",
        ),
        (content.trim().is_empty(), "Skill content is required."),
        (
            !trimmed.starts_with('#') && !trimmed.starts_with("---"),
            "Skill content should start with a Markdown heading or YAML frontmatter.",
        ),
        (
            !scan_skill_content(content).unwrap_or(false),
            "Skill content contains potentially unsafe commands or patterns.",
        ),
    ];
    for (failed, msg) in checks {
        match failed {
            true => errors.push(msg.to_string()),
            false => {}
        }
    }
    errors
}

pub fn get_skills_dir() -> PathBuf {
    crate::config::skills_dir()
}

pub fn get_db_path() -> PathBuf {
    crate::config::loader::runtime_db_path("memory.db")
}

pub fn get_workspace_skills_dir() -> PathBuf {
    crate::config::resolve_path(".openz/skills")
}

fn discover_skill_entry(path: PathBuf) -> Option<(String, PathBuf)> {
    match (
        path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("md"),
        path.is_dir(),
    ) {
        (true, _) => {
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            let valid = !name.is_empty() && name != "INDEX" && !name.ends_with(".bak");
            valid.then_some((name, path))
        }
        (false, true) => {
            let skill_file = path.join("SKILL.md");
            let name = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            let valid = skill_file.exists()
                && skill_file.is_file()
                && !name.is_empty()
                && name != "archive"
                && !name.starts_with('.');
            valid.then_some((name, skill_file))
        }
        (false, false) => None,
    }
}

fn discover_skill_files_in_dir(dir: &std::path::Path) -> Result<Vec<(String, PathBuf)>> {
    match dir.exists() && dir.is_dir() {
        false => Ok(Vec::new()),
        true => {
            let mut items = Vec::new();
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                match discover_skill_entry(entry.path()) {
                    Some(pair) => items.push(pair),
                    None => {}
                }
            }
            Ok(items)
        }
    }
}

fn load_skill_files_from_dir(
    dir: &std::path::Path,
    skills_map: &mut std::collections::HashMap<String, Skill>,
) -> Result<()> {
    for (name, skill_path) in discover_skill_files_in_dir(dir)? {
        let content = fs::read_to_string(&skill_path)?;
        skills_map.insert(name.clone(), Skill { name, content });
    }
    Ok(())
}

fn create_skills_schema(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS skills (
            name TEXT NOT NULL,
            content TEXT NOT NULL,
            profile TEXT NOT NULL DEFAULT '',
            use_count INTEGER DEFAULT 0,
            success_count INTEGER DEFAULT 0,
            last_used TEXT,
            created_at TEXT,
            PRIMARY KEY (name, profile)
        )",
        [],
    )?;
    let _ = conn.execute(
        "ALTER TABLE skills ADD COLUMN success_count INTEGER DEFAULT 0",
        [],
    );
    let _ = conn.execute(
        "DELETE FROM skills
         WHERE rowid NOT IN (
             SELECT MAX(rowid) FROM skills GROUP BY name, COALESCE(profile, '')
         )",
        [],
    );
    let _ = conn.execute(
        "UPDATE skills SET profile = '' WHERE profile IS NULL",
        [],
    );
    conn.execute(
        "CREATE TABLE IF NOT EXISTS skill_revisions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            profile TEXT NOT NULL DEFAULT '',
            content TEXT NOT NULL,
            created_at TEXT NOT NULL
        )",
        [],
    )?;
    Ok(())
}

fn quarantine_malformed_memory_db(path: &std::path::Path) {
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%S");
    let backup = path.with_extension(format!("db.corrupt.{}", stamp));
    let _ = std::fs::rename(path, &backup);
    let _ = std::fs::remove_file(format!("{}-wal", path.display()));
    let _ = std::fs::remove_file(format!("{}-shm", path.display()));
    tracing::warn!(
        source = %path.display(),
        backup = %backup.display(),
        "Malformed OpenZ memory database quarantined; a fresh database will be created."
    );
}

fn is_sqlite_corruption_error(err: &rusqlite::Error) -> bool {
    err.sqlite_error_code().is_some_and(|code| {
        matches!(
            code,
            rusqlite::ErrorCode::DatabaseCorrupt | rusqlite::ErrorCode::NotADatabase
        )
    })
}

pub fn get_connection() -> Result<Connection> {
    let path = get_db_path();
    match path.parent() {
        Some(parent) => fs::create_dir_all(parent)?,
        None => {}
    }

    let mut conn = Connection::open(&path)?;
    match crate::core::sqlite::apply_standard_pragmas(&conn) {
        Ok(()) => {}
        Err(err) => match is_sqlite_corruption_error(&err) {
            true => {
                tracing::warn!(
                    database = %path.display(),
                    error = %err,
                    "OpenZ memory database corrupt during SQLite initialization; quarantining."
                );
                drop(conn);
                quarantine_malformed_memory_db(&path);
                conn = Connection::open(&path)?;
                crate::core::sqlite::apply_standard_pragmas(&conn)?;
            }
            false => return Err(err.into()),
        },
    }

    static SKILLS_INIT_ONCE: std::sync::Once = std::sync::Once::new();
    let mut needs_reopen = false;
    SKILLS_INIT_ONCE.call_once(|| {
        let check = conn.query_row("PRAGMA quick_check", [], |row| row.get::<_, String>(0));
        let corrupt_reason = match check {
            Ok(val) => (val != "ok").then_some(val),
            Err(err) => matches!(
                err.sqlite_error_code(),
                Some(rusqlite::ErrorCode::DatabaseCorrupt | rusqlite::ErrorCode::NotADatabase)
            )
            .then(|| err.to_string()),
        };
        match corrupt_reason {
            Some(reason) => {
                tracing::warn!(
                    database = %path.display(),
                    reason = %reason,
                    "OpenZ memory database quick_check reported corruption."
                );
                needs_reopen = true;
            }
            None => {}
        }
    });

    match needs_reopen {
        true => {
            drop(conn);
            quarantine_malformed_memory_db(&path);
            conn = Connection::open(&path)?;
            crate::core::sqlite::apply_standard_pragmas(&conn)?;
        }
        false => {}
    }

    create_skills_schema(&conn)?;
    static SKILLS_MIGRATE_ONCE: std::sync::Once = std::sync::Once::new();
    SKILLS_MIGRATE_ONCE.call_once(|| {
        let _ = migrate_old_skills_to_db(&conn);
        let _ = initialize_default_subagent_skills(&conn);
    });
    Ok(conn)
}

fn migrate_old_skills_to_db(conn: &Connection) -> Result<()> {
    let global_dir = get_skills_dir();
    match global_dir.exists() && global_dir.is_dir() {
        false => {}
        true => {
            for (name, skill_path) in discover_skill_files_in_dir(&global_dir)? {
                match fs::read_to_string(&skill_path) {
                    Ok(content) => {
                        let now = chrono::Utc::now().to_rfc3339();
                        let _ = conn.execute(
                            "INSERT OR IGNORE INTO skills (name, content, profile, created_at, last_used)
                             VALUES (?1, ?2, '', ?3, ?3)",
                            params![name, content, now],
                        );
                        let is_legacy_flat = skill_path.parent() == Some(global_dir.as_path())
                            && skill_path.extension().and_then(|s| s.to_str()) == Some("md");
                        let _ = is_legacy_flat.then(|| {
                            let pkg_dir = global_dir.join(&name);
                            let pkg_skill_md = pkg_dir.join("SKILL.md");
                            match !pkg_skill_md.exists() {
                                true => {
                                    let _ = fs::create_dir_all(&pkg_dir);
                                    let _ = fs::write(&pkg_skill_md, &content)
                                        .and_then(|_| fs::remove_file(&skill_path));
                                }
                                false => {
                                    let _ = fs::remove_file(&skill_path);
                                }
                            }
                        });
                    }
                    Err(_) => {}
                }
            }
        }
    }
    Ok(())
}


fn initialize_default_subagent_skills(conn: &Connection) -> Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    let default_skills = vec![
        ("planner", "milestone_decomposition", "# Skill: Milestone Decomposition

Decompose high-level tasks into discrete, sequential milestones:
1. Define clear criteria of done (CoD) for each milestone.
2. Maintain a PLAN.md tracking the execution path.
3. List dependencies between tasks to avoid build blockages."),

        ("researcher", "context_hydration", "# Skill: Context Hydration

Compile precise reference contexts from local research archives and external sources:
1. ALWAYS check the local research archive first using `search_research` to see if the page, path, or output is already cached.
2. If not found in the local cache or if the data is stale, run targeted web searches or scrape contents.
3. Consolidate search facts into reference sheets for the parent agent."),

        ("architect", "schema_design", "# Skill: Schema Design

Design clean systems, DB tables, and interface contracts:
1. Draft structured schemas with clear data types and relationships.
2. Outline system hierarchies using Mermaid diagram flows.
3. Document API endpoints with requests/responses format."),

        ("git_ops_agent", "git_hygiene", "# Skill: Git Hygiene

Maintain clean, atomic, and structured version control history:
1. Review staged changes using git status and diff.
2. Create feature branches detached from active branch for testing.
3. Author descriptive, atomic commit messages with clear summaries."),

        ("ast_searcher", "ast_structural_grep", "# Skill: AST Structural Grep

Find codebase structures rapidly using syntax trees:
1. Query function, struct, or class definitions using structural patterns.
2. Compile file outlines to identify entrypoints and dependencies.
3. Locate exact symbol references instead of relying on loose regex text searches."),

        ("database_specialist", "db_query_audit", "# Skill: DB Query Audit

Inspect database health, design performant queries, and audit schemas:
1. Query SQLite tables safely without risking schema corruption.
2. Read system schema metadata to verify index placement.
3. Ensure large queries use indexed fields to prevent slow table scans."),

        ("browser_operator", "browser_testing", "# Skill: Browser Testing

Run web browser automation and crawling tasks:
1. Automate UI regression test flows via OpenZ's native browser tool (ui_tree, click, type, eval_js, screenshot).
2. Crawl site maps to verify link routing and response codes.
3. Extract dynamically rendered DOM layouts and tabular data from pages."),

        ("dependency_manager", "stack_package_onpkg", "# Skill: Stack Package Management

Manage project templates, stack configurations, and dependencies:
1. Scaffold project folders using approved template directories.
2. Audit package configurations to maintain clean dependency trees.
3. Add and update packages safely with correct lockfile synchronization."),

        ("reviewer", "code_diff_audit", "# Skill: Code Diff Audit

Audit modified code for bugs, complexity, and performance:
1. Inspect code changes line-by-line for regression risks.
2. Identify memory leak possibilities, resource closures, and unsafe code blocks.
3. Suggest clean, simplified refactorings following DRY principles."),

        ("code_auditor", "vulnerability_scanning", "# Skill: Vulnerability Scanning

Locate security concerns, hardcoded credentials, and unsafe calls:
1. Scan for hardcoded API keys, certificates, or passwords.
2. Identify unsafe functions that lack bounds checking or sanitization.
3. Audit external packages against known security vulnerability CVE databases."),

        ("debugger", "root_cause_analysis", "# Skill: Root Cause Analysis

Reproduction and isolation of system crash conditions:
1. Retrieve system logs and parse stack traces to locate line failures.
2. Design minimum reproduction steps to isolate the crash trigger.
3. Apply targeted, backward-compatible fixes to prevent future regressions."),

        ("test_engineer", "test_suite_coverage", "# Skill: Test Suite Coverage

Design comprehensive unit, integration, and E2E tests:
1. Write testing suites coverages targeting edge-case parameters.
2. Verify test outcomes and mock external network resources cleanly.
3. Track and maintain high coverage ratios across all code modifications."),

        ("frontend_architect", "ui_ux_styling", "# Skill: Premium UI/UX Design System & Implementation

Design and build distinctive, professional frontend interfaces that avoid templated defaults. Ground all designs in the subject's concrete audience, product type, and brand voice.

---

## 1. Design System Generation

For every user request, first design a structured Design System containing:
1. **Layout Pattern**: Hero-centric, multi-section dashboard, or Socratic broadsheet.
2. **Style Archetype**: 
   * *Soft UI Evolution* (soft shadows, subtle organic depths, gentle hover states).
   * *High-contrast Editorial* (hairline rules, strict grids, high-contrast serif).
   * *Neon Cyberpunk* (sleek dark mode, vibrant vermilion/sage accents).
3. **Palette Harmony**: Define specific hex values for Primary, Secondary, CTA, Background, and Text. Avoid generic colors or excessive AI-purple gradients.
4. **Typography Scale**: Pair distinct display and body faces (e.g. Playfair Display / Inter), setting deliberate font-weights, line-heights, and letter-spacing.

---

## 2. Layout & Typography Rules

* **Ground it in the Subject**: Identify the audience, a single primary goal of the page, and name one characteristic subject element to feature as the hero.
* **Semantic Structure**: Layout blocks must encode truth; do not decorate with sequences (like 01 / 02 / 03) unless the content is a chronological sequence.
* **Type Treatment**: Typography carries the personality. Never treat typography as a neutral delivery vehicle.

---

## 3. Motion & Micro-interactions

* **Deliberate Transitions**: An orchestrated moment lands harder than scattered animations. Pair page-load entries with scroll-triggered reveals.
* **Smooth Transitions**: Hover states must transition smoothly over 150-300ms.
* **Cursor Clues**: Ensure `cursor-pointer` is applied to all clickable elements.

---

## 4. Pre-Delivery Quality Checklist

Before finalizing any frontend design, verify that the code passes these tests:
* **Icon Quality**: Never use emojis as icons. Implement inline SVG vectors (Heroicons or Lucide).
* **Contrast Rating**: Maintain a minimum contrast ratio of 4.5:1 for standard text (WCAG AA rating).
* **Focus States**: Make focus rings outline-visible for keyboard nav accessibility.
* **Responsive Breakpoints**: Design to scale fluidly across 375px (mobile), 768px (tablet), 1024px (desktop), and 1440px (wide screen).
* **A11y Motion**: Wrap complex animations to respect `prefers-reduced-motion`."),

        ("frontend_architect", "chunked_static_site_generation", "# Skill: Chunked Static Site Generation

Use this when creating complete HTML/CSS/JS websites or large generated source files:
1. Do not send a huge `write_file` payload. Keep each file-write or append chunk under about 8KB of content.
2. Prefer creating the directory, writing the document shell, then appending CSS, body sections, and scripts as separate chunks.
3. After writing, verify with `read_file`, `tail`, or a line/byte count that the file starts with the expected header and ends with the expected closing tags.
4. Only open the page after verification succeeds. If a chunk fails, inspect the partial file before appending or rewriting.
5. Save any new reusable pattern with `workflow_memory` or `curate_skill` after the task succeeds."),

        ("media_designer", "segmented_html_video_rendering", "# Skill: Segmented HTML Video Rendering

Use this when rendering HTML/CSS/JS animation videos longer than roughly 20 seconds:
1. Avoid one long 900+ frame render if the renderer warns about timeout risk.
2. Split the timeline into shorter HTML segment files, usually 8-10 seconds each.
3. Render each segment independently, then concatenate with ffmpeg's concat demuxer.
4. Verify final duration and file size before opening the video for the user.
5. Keep intermediate segment files until the final video is confirmed so failed segments can be re-rendered without restarting from scratch."),

        ("docs_lookup_agent", "documentation_synthesis", "# Skill: Documentation Synthesis

Retrieve and synthesize API usage guides:
1. Search for official documentation portals and extract technical specs.
2. Format clear examples showing how to initialize SDKs and call endpoints.
3. Cross-reference configuration flags and parameter defaults."),

        ("document_compiler", "document_templating", "# Skill: Document Templating

Compile professional Word and PDF documents:
1. Inject structured content into pre-defined layouts.
2. Ensure proper page boundaries, page numbers, and margins.
3. Verify tables and text alignment remain neat post-generation."),

        ("presentation_designer", "slide_deck_design", "# Skill: Slide Deck Design

Structure clear, impactful presentation slide decks:
1. Build layouts with distinct headers, subtitles, and bullet points.
2. Pair clear, readable fonts and maintain consistent color palettes.
3. Write clear speaker/presenter notes for each slide."),

        ("code_synthesizer", "code_scaffolding", "# Skill: Code Scaffolding

Generate clean, modular code architectures:
1. Structure logical directory trees and package interfaces.
2. Write self-contained, unit-testable classes and functions.
3. Automate boilerplate setup using project template configurations."),

        ("summarizer_agent", "context_compaction", "# Skill: Context Compaction

Condense massive log files and web scraping outputs:
1. Extract key statistical metrics, errors, and system configs.
2. Organize information using structured bullet points or Markdown tables.
3. Remove redundancy while maintaining all critical engineering facts."),

        ("media_designer", "geometric_drawing", "# Skill: Geometric Drawing

Design and render high-quality geometric graphics, visual flowcharts, custom charts, diagrams, and simple illustrations:
1. Plan pixel coordinate math for canvas boundaries (default: 400x400) carefully to prevent element overlap or text clipping.
2. Group logical blocks together by specifying consecutive rect, circle, or line elements.
3. Use a clear color palette with sufficient background/foreground contrast (e.g. hex colors).
4. Keep label text aligned, centered, and scaled appropriately (typically size 14.0 to 18.0) to ensure readability."),

        ("openz_coordinator", "openz_orchestration_handbook", "# Skill: OpenZ Orchestration Handbook

Manage multi-agent workflows, coordinate subagent delegations, and execute basic orchestrator tools with expert efficiency:

---

## 1. Basic Tools Execution Protocol

* **Filesystem & Code Outline**:
  - Prefer reading specific line ranges via `read_file` over reading entire large files.
  - Locate targets using structural outlines (`code_outline`) or semantic queries (`ast_grep`) rather than generic substring lookups.
  - Limit multi-line replacement scopes to contiguous blocks using `replace_lines` or structured unified diffs/SEARCH-REPLACE blocks using `patch_file`.

* **Command Execution & Safety**:
  - Use `exec_command` to compile and test code (`cargo check`, `cargo test`) immediately after code changes.
  - Avoid running commands with unsafe inputs or shell-expansion triggers.
  - Keep commands non-blocking; monitor status or input streams cleanly.

* **Git Operations**:
  - Review repository modifications using `git status` and `diff`.
  - Author clean, atomic commits with structured descriptions.

* **Database Inspection**:
  - Query tables safely and audit schemas using `db_inspector`. Never corrupt active records or write un-indexed queries.

* **Subagent Delegation**:
  - Use `delegate_task` to offload distinct, concurrent workstreams to specialized subagents (e.g. planner, researcher, reviewer, frontend_architect).

---

## 2. OpenZ Architecture & Features Reference

* **Model Routing Rules**:
  - OpenZ routes LLM requests based on model name prefixes (e.g., `anthropic/claude-3-5-sonnet` routes to Anthropic Messages API, `gpt-4o` routes to OpenAI).

* **Subcommands Reference**:
  - `onboard`: Setup provider API keys.
  - `configure`: Adjust channels (WS, Telegram, Discord, WhatsApp) and MCP servers.
  - `agent`: Start the interactive raw-mode TUI shell.
  - `gateway`: Run WebSockets server & WebUI.
  - `telegram` / `discord` / `whatsapp`: Launch listener channel servers.
  - `sop`: Manage stateful SOP workflows."),

        ("openz", "openz_fullstack_playbook", "# Skill: OpenZ Fullstack Playbook

Execute autonomous end-to-end tasks across codebase search, file manipulation, cognitive memory, bookmarks, procedural skills, and tool catalog inspection with high reliability:
1. Inspect codebase and project layout before making changes (`list_dir`, `read_file`, `grep_search`).
2. Store workspace-critical context using `store_memory` and recall relevant information with `recall_memory`.
3. Save external documentation, package registries, and git repos with `knowledge_source`.
4. Persist newly discovered patterns or repetitive troubleshooting procedures via `curate_skill`.
5. Verify changes with focused compilation checks and tests (`exec_command`)."),

        ("sop_designer", "sop_workflow_design", "# Skill: SOP Workflow Design

Design, structure, and optimize Standard Operating Procedure (SOP) JSON definitions for execution:
1. Break down complex operational flows into isolated, step-by-step instructions.
2. Specify exact dependency chains for each step to prevent race conditions or invalid states.
3. Define strict input/output criteria and parameter boundaries for step data.
4. Establish clear verification logic to confirm the success of each operational stage."),

        ("api_integrator", "api_client_integration", "# Skill: API Client Integration

Research, construct, and integrate REST, GraphQL, or RPC interface connections:
1. Search public/private developer API documentation to extract endpoints, query parameters, and headers.
2. Design standardized OpenAPI definitions mapping integration paths.
3. Write robust asynchronous client implementations equipped with request backoff/retries and rate-limiting limits.
4. Test payload parameters and handle response exception states cleanly."),

        ("performance_tuner", "bottleneck_analysis", "# Skill: Bottleneck Analysis

Isolate and resolve system resource leaks, CPU bottlenecks, and memory blocks:
1. Profile execution times and identify hot code sections.
2. Parse memory profiling traces, dump heap snapshots, and scan for resource leaks.
3. Optimize Rust/Tokio async task architectures to prevent main thread blocking.
4. Improve data structures and minimize allocations in high-frequency loops."),

        ("communication_manager", "multi_channel_notifications", "# Skill: Multi-Channel Notifications

Format, template, and dispatch messages across communication channels:
1. Structure concise, high-priority system alerts and progress notifications.
2. Pair markdown formatting with platform constraints for WebSocket, Telegram, Discord, and WhatsApp.
3. Design clean, modular SMTP email templates using standard communication libraries.
4. Schedule notification intervals to avoid spamming user active channels."),

        ("automation_agent", "system_task_automation", "# Skill: System Task Automation

Automate workflows, interact with user interfaces, and schedule actions:
1. Conduct browser automation (scraping, E2E testing, visual QA) using OpenZ's native `browser` tool and `crawl_site` / `web_fetch`.
2. Structure and dispatch HTTP API calls and webhooks securely.
3. Manage and configure recurring background cron jobs and schedules.
4. Execute operations on the local file system and coordinate background alerts safely."),

        ("coding_agent", "sandboxed_code_execution", "# Skill: Sandboxed Code Execution

Iteratively write, refactor, and verify code blocks inside compilation harnesses:
1. Generate clean, maintainable, and modular code blocks conforming to DRY and SOLID design principles.
2. Invoke cargo commands (build, check, test) to audit Rust compilation and type check correctness.
3. Read compiler warnings and test outputs, isolate lines containing errors, and fix them iteratively.
4. Refactor complexity hotspots and write robust unit tests for new modules.")
    ];

    for (profile, name, content) in default_skills {
        let _ = conn.execute(
            "INSERT OR IGNORE INTO skills (name, content, profile, created_at, last_used)
             VALUES (?1, ?2, ?3, ?4, ?4)",
            params![name, content, profile, now],
        );
        let _ = conn.execute(
            "UPDATE skills SET content = ?2
             WHERE name = ?1 AND profile = ?3
               AND (content LIKE '%view_file%' OR content LIKE '%replace_file_content%')",
            params![name, content, profile],
        );
    }
    Ok(())
}

pub fn load_skills() -> Result<Vec<Skill>> {
    load_skills_with_profile(None)
}

fn discover_workspace_and_external_dirs(
    skills_config: &crate::config::schema::SkillsConfig,
) -> Vec<(PathBuf, &'static str)> {
    let mut roots = Vec::new();
    match skills_config.workspace_skills_enabled {
        true => {
            for rel in [".openz/skills", ".agents/skills", ".claude/skills"] {
                roots.push((crate::config::resolve_path(rel), "workspace"));
            }
        }
        false => {}
    }
    let _ = dirs::home_dir().map(|home| {
        roots.push((home.join(".agents").join("skills"), "external"));
    });
    for external_dir in &skills_config.external_dirs {
        roots.push((crate::config::resolve_path(external_dir), "external"));
    }
    roots
}

fn add_file_skill_views(
    dir: &std::path::Path,
    source: &str,
    is_protected: bool,
    overwrite_existing: bool,
    views: &mut std::collections::HashMap<String, SkillView>,
) -> Result<()> {
    for (name, skill_path) in discover_skill_files_in_dir(dir)? {
        let should_insert = overwrite_existing || !views.contains_key(&name);
        match should_insert {
            false => {}
            true => {
                let content = fs::read_to_string(&skill_path)?;
                let skill_ref = Skill {
                    name: name.clone(),
                    content: content.clone(),
                };
                views.insert(
                    name.clone(),
                    SkillView {
                        description: skill_ref.description(),
                        triggers: skill_ref.triggers(),
                        validation_errors: validate_skill_metadata(&name, &content),
                        name,
                        content,
                        scope: source.to_string(),
                        profile: None,
                        source: source.to_string(),
                        path: Some(skill_path.display().to_string()),
                        enabled: true,
                        is_protected,
                        use_count: 0,
                        success_count: 0,
                        revision_count: 0,
                        created_at: None,
                        last_used: None,
                    },
                );
            }
        }
    }
    Ok(())
}

fn reconcile_global_disk_skill_views(
    dir: &std::path::Path,
    views: &mut std::collections::HashMap<String, SkillView>,
) {
    let entries = match discover_skill_files_in_dir(dir) {
        Ok(items) => items,
        Err(_) => return,
    };
    let conn_opt = get_connection().ok();
    for (name, skill_path) in entries {
        let content = match fs::read_to_string(&skill_path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let path_display = skill_path.display().to_string();
        let skill_ref = Skill {
            name: name.clone(),
            content: content.clone(),
        };
        match views.get_mut(&name) {
            Some(existing_view) => {
                existing_view.path = Some(path_display);
                let changed = existing_view.content != content;
                let _ = changed.then(|| {
                    let prev = existing_view.content.clone();
                    existing_view.description = skill_ref.description();
                    existing_view.triggers = skill_ref.triggers();
                    existing_view.validation_errors = validate_skill_metadata(&name, &content);
                    existing_view.content = content.clone();
                    let _ = conn_opt.as_ref().map(|conn| {
                        let _ = record_skill_revision_snapshot(conn, &name, None, &prev);
                        let _ = conn.execute(
                            "UPDATE skills SET content = ?1 WHERE name = ?2 AND COALESCE(profile, '') = ''",
                            params![content, name],
                        );
                    });
                });
            }
            None => {
                let now = chrono::Utc::now().to_rfc3339();
                let _ = conn_opt.as_ref().map(|conn| {
                    let _ = conn.execute(
                        "INSERT OR IGNORE INTO skills (name, content, profile, created_at, last_used)
                         VALUES (?1, ?2, '', ?3, ?3)",
                        params![name, content, now],
                    );
                });
                views.insert(
                    name.clone(),
                    SkillView {
                        description: skill_ref.description(),
                        triggers: skill_ref.triggers(),
                        validation_errors: validate_skill_metadata(&name, &content),
                        name,
                        content,
                        scope: "global".to_string(),
                        profile: None,
                        source: "global".to_string(),
                        path: Some(path_display),
                        enabled: true,
                        is_protected: false,
                        use_count: 0,
                        success_count: 0,
                        revision_count: 0,
                        created_at: None,
                        last_used: None,
                    },
                );
            }
        }
    }
}

pub fn load_skill_views() -> Result<Vec<SkillView>> {
    let mut views = std::collections::HashMap::<String, SkillView>::new();

    match get_connection() {
        Ok(conn) => {
            let mut rev_counts = std::collections::HashMap::<(String, String), usize>::new();
            let _ = conn
                .prepare("SELECT name, profile, COUNT(*) FROM skill_revisions GROUP BY name, profile")
                .and_then(|mut rev_stmt| {
                    let rows = rev_stmt.query_map([], |r| {
                        Ok((
                            r.get::<_, String>(0)?,
                            r.get::<_, String>(1)?,
                            r.get::<_, i64>(2).unwrap_or(0) as usize,
                        ))
                    })?;
                    for (rname, rprof, rcnt) in rows.flatten() {
                        rev_counts.insert((rname, rprof), rcnt);
                    }
                    Ok(())
                });

            let mut stmt = conn.prepare(
                "SELECT name, content, COALESCE(profile, ''), use_count, COALESCE(success_count, 0), created_at, last_used FROM skills",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3).unwrap_or(0),
                    row.get::<_, i64>(4).unwrap_or(0),
                    row.get::<_, Option<String>>(5).unwrap_or(None),
                    row.get::<_, Option<String>>(6).unwrap_or(None),
                ))
            })?;

            let db_path = get_db_path().display().to_string();
            for row in rows.flatten() {
                let (name, content, raw_prof, use_count, success_count, created_at, last_used) = row;
                let profile = (!raw_prof.is_empty()).then_some(raw_prof.clone());
                let scope = profile
                    .as_ref()
                    .map(|p| format!("subagent:{p}"))
                    .unwrap_or_else(|| "global".to_string());
                let key = profile
                    .as_ref()
                    .map(|p| format!("{p}/{name}"))
                    .unwrap_or_else(|| name.clone());
                let is_protected = profile.is_some();
                let revision_count = rev_counts
                    .get(&(name.clone(), raw_prof))
                    .copied()
                    .unwrap_or(0);
                let skill_ref = Skill {
                    name: name.clone(),
                    content: content.clone(),
                };
                views.insert(
                    key,
                    SkillView {
                        description: skill_ref.description(),
                        triggers: skill_ref.triggers(),
                        validation_errors: validate_skill_metadata(&name, &content),
                        name,
                        content,
                        scope,
                        profile,
                        source: "database".to_string(),
                        path: Some(db_path.clone()),
                        enabled: true,
                        is_protected,
                        use_count,
                        success_count,
                        revision_count,
                        created_at,
                        last_used,
                    },
                );
            }
        }
        Err(_) => {}
    }

    reconcile_global_disk_skill_views(&get_skills_dir(), &mut views);

    let skills_config = crate::config::loader::load_config()
        .map(|config| config.skills)
        .unwrap_or_default();
    for (dir, source) in discover_workspace_and_external_dirs(&skills_config) {
        add_file_skill_views(&dir, source, true, true, &mut views)?;
    }

    for spec in opencaveman::builtin_caveman_skills() {
        views
            .entry(spec.name.to_string())
            .or_insert_with(|| {
                let skill_ref = Skill {
                    name: spec.name.to_string(),
                    content: spec.content.to_string(),
                };
                SkillView {
                    description: skill_ref.description(),
                    triggers: skill_ref.triggers(),
                    validation_errors: validate_skill_metadata(spec.name, spec.content),
                    name: spec.name.to_string(),
                    content: spec.content.to_string(),
                    scope: "builtin".to_string(),
                    profile: None,
                    source: "opencaveman".to_string(),
                    path: Some(format!("opencaveman://skills/{}", spec.name)),
                    enabled: true,
                    is_protected: true,
                    use_count: 0,
                    success_count: 0,
                    revision_count: 0,
                    created_at: None,
                    last_used: None,
                }
            });
    }

    Ok(views.into_values().collect())
}

pub fn load_skills_with_profile(profile_name: Option<&str>) -> Result<Vec<Skill>> {
    let mut skills_map = std::collections::HashMap::new();

    match get_connection() {
        Ok(conn) => {
            let mut stmt = conn.prepare(
                "SELECT name, content FROM skills WHERE COALESCE(profile, '') = '' OR profile = ?1",
            )?;
            let profile_str = profile_name.unwrap_or("");
            let rows = stmt.query_map(params![profile_str], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;

            for (name, content) in rows.flatten() {
                skills_map.insert(name.clone(), Skill { name, content });
            }
        }
        Err(_) => {}
    }

    let global_skills_dir = get_skills_dir();
    let _ = load_skill_files_from_dir(&global_skills_dir, &mut skills_map);

    match profile_name {
        Some(prof) => {
            let profile_skills_dir = crate::config::runtime_data_dir()
                .join("subagents")
                .join(prof)
                .join("skills");
            let _ = load_skill_files_from_dir(&profile_skills_dir, &mut skills_map);
        }
        None => {}
    }

    let skills_config = crate::config::loader::load_config()
        .map(|config| config.skills)
        .unwrap_or_default();
    for (dir, _) in discover_workspace_and_external_dirs(&skills_config) {
        load_skill_files_from_dir(&dir, &mut skills_map)?;
    }

    match profile_name.is_none() {
        true => {
            for spec in opencaveman::builtin_caveman_skills() {
                skills_map
                    .entry(spec.name.to_string())
                    .or_insert_with(|| Skill {
                        name: spec.name.to_string(),
                        content: spec.content.to_string(),
                    });
            }
        }
        false => {}
    }

    Ok(skills_map.into_values().collect())
}

fn checked_within_last_24h(check_path: &std::path::Path) -> bool {
    check_path
        .exists()
        .then(|| fs::read_to_string(check_path).ok())
        .flatten()
        .and_then(|content| serde_json::from_str::<String>(&content).ok())
        .and_then(|timestamp_str| chrono::DateTime::parse_from_rfc3339(&timestamp_str).ok())
        .is_some_and(|last_checked| {
            let last_checked_utc = last_checked.with_timezone(&chrono::Utc);
            chrono::Utc::now().signed_duration_since(last_checked_utc)
                < chrono::Duration::hours(24)
        })
}

fn archive_skill_artifacts_on_disk(name: &str, content: &str) -> Result<()> {
    let archive_pkg_dir = get_skills_dir().join("archive").join(name);
    fs::create_dir_all(&archive_pkg_dir)?;
    let _ = fs::write(archive_pkg_dir.join("SKILL.md"), content);
    let active_flat = get_skills_dir().join(format!("{}.md", name));
    let _ = fs::remove_file(&active_flat);
    let active_pkg = get_skills_dir().join(name);
    match active_pkg.is_dir() {
        true => {
            let _ = fs::remove_dir_all(&active_pkg);
        }
        false => {}
    }
    Ok(())
}

fn skill_signature_tokens(skill: &Skill) -> std::collections::HashSet<String> {
    let mut combined = skill.triggers().join(" ");
    combined.push(' ');
    combined.push_str(&skill.description());
    for line in skill.content.lines().map(str::trim).filter(|l| l.starts_with('#')) {
        combined.push(' ');
        combined.push_str(line);
    }
    ranker::tokenize_stemmed(&combined, true).into_iter().collect()
}

pub fn consolidate_learned_skills() -> Result<usize> {
    let conn = get_connection()?;
    let mut stmt = conn.prepare(
        "SELECT name, content, COALESCE(use_count, 0), COALESCE(success_count, 0)
         FROM skills
         WHERE COALESCE(profile, '') = ''",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2).unwrap_or(0),
            row.get::<_, i64>(3).unwrap_or(0),
        ))
    })?;

    let mut active_rows = Vec::new();
    let mut actions_taken = 0usize;

    for (name, content, uses, successes) in rows.flatten() {
        let low_efficacy = uses >= 5 && successes == 0;
        match low_efficacy {
            true => {
                let _ = record_skill_revision_snapshot(&conn, &name, None, &content);
                let _ = archive_skill_artifacts_on_disk(&name, &content);
                let _ = conn.execute(
                    "DELETE FROM skills WHERE name = ?1 AND COALESCE(profile, '') = ''",
                    params![name],
                );
                actions_taken += 1;
            }
            false => active_rows.push((name, content, uses, successes)),
        }
    }

    active_rows.sort_by(|a, b| {
        let util_a = ranker::compute_utility_multiplier(a.2, a.3);
        let util_b = ranker::compute_utility_multiplier(b.2, b.3);
        util_b
            .partial_cmp(&util_a)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.2.cmp(&a.2))
            .then_with(|| a.0.cmp(&b.0))
    });

    let mut archived_dups = std::collections::HashSet::<String>::new();
    let count = active_rows.len();
    for i in 0..count {
        let primary_name = active_rows[i].0.clone();
        match archived_dups.contains(&primary_name) {
            true => continue,
            false => {}
        }
        let primary_skill = Skill {
            name: primary_name.clone(),
            content: active_rows[i].1.clone(),
        };
        let sig_a = skill_signature_tokens(&primary_skill);
        match sig_a.len() >= 3 {
            false => continue,
            true => {}
        }

        for j in (i + 1)..count {
            let candidate_name = active_rows[j].0.clone();
            match archived_dups.contains(&candidate_name) {
                true => continue,
                false => {}
            }
            let candidate_skill = Skill {
                name: candidate_name.clone(),
                content: active_rows[j].1.clone(),
            };
            let sig_b = skill_signature_tokens(&candidate_skill);
            let sim = match sig_b.len() >= 3 {
                false => 0.0,
                true => {
                    let inter = sig_a.intersection(&sig_b).count() as f32;
                    let union = sig_a.union(&sig_b).count().max(1) as f32;
                    inter / union
                }
            };
            let is_duplicate = sim >= 0.80;
            let _ = is_duplicate.then(|| {
                let _ = record_skill_revision_snapshot(
                    &conn,
                    &candidate_name,
                    None,
                    &candidate_skill.content,
                );
                let _ = archive_skill_artifacts_on_disk(&candidate_name, &candidate_skill.content);
                let _ = conn.execute(
                    "DELETE FROM skills WHERE name = ?1 AND COALESCE(profile, '') = ''",
                    params![candidate_name],
                );
                archived_dups.insert(candidate_name);
                actions_taken += 1;
            });
        }
    }

    Ok(actions_taken)
}

pub fn archive_stale_skills() -> Result<()> {
    let check_path = crate::config::runtime_data_dir().join("last_stale_skills_check.json");
    match checked_within_last_24h(&check_path) {
        true => return Ok(()),
        false => {}
    }

    let _ = consolidate_learned_skills();

    match get_connection() {
        Ok(conn) => {
            let now = chrono::Utc::now();
            let stale_duration = chrono::Duration::days(30);

            let mut stmt = conn.prepare(
                "SELECT name, COALESCE(profile, ''), last_used, content FROM skills",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })?;

            let mut to_delete = Vec::new();
            for (name, profile_key, last_used_opt, content) in rows.flatten() {
                let is_stale = profile_key.is_empty()
                    && last_used_opt
                        .as_deref()
                        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                        .is_some_and(|last_used_date| {
                            let last_used_utc = last_used_date.with_timezone(&chrono::Utc);
                            now.signed_duration_since(last_used_utc) > stale_duration
                        });
                match is_stale {
                    true => {
                        archive_skill_artifacts_on_disk(&name, &content)?;
                        to_delete.push((name, profile_key));
                    }
                    false => {}
                }
            }

            for (name, profile_key) in to_delete {
                let _ = conn.execute(
                    "DELETE FROM skills WHERE name = ?1 AND COALESCE(profile, '') = ?2",
                    params![name, profile_key],
                );

                let aura_blue = "\x1b[38;2;96;165;250m";
                let color_reset = "\x1b[0m";
                crate::channels::send_notification(&format!(
                    "{}◇ [Self-Improvement] Skill '{}' archived to filesystem due to 30 days of database inactivity.{}",
                    aura_blue, name, color_reset
                ));
            }

            let now_str = chrono::Utc::now().to_rfc3339();
            match serde_json::to_string(&now_str) {
                Ok(serialized) => {
                    let _ = fs::write(&check_path, serialized);
                }
                Err(_) => {}
            }
        }
        Err(_) => {}
    }
    Ok(())
}

fn load_profile_skill_names(profile_name: Option<&str>) -> std::collections::HashSet<String> {
    let mut profile_skills = std::collections::HashSet::new();
    let prof = match profile_name {
        Some(prof) => prof,
        None => return profile_skills,
    };
    let conn = match get_connection() {
        Ok(conn) => conn,
        Err(_) => return profile_skills,
    };
    let mut stmt = match conn.prepare("SELECT name FROM skills WHERE profile = ?1") {
        Ok(stmt) => stmt,
        Err(_) => return profile_skills,
    };
    let rows = match stmt.query_map(params![prof], |r| r.get::<_, String>(0)) {
        Ok(rows) => rows,
        Err(_) => return profile_skills,
    };
    for name in rows.flatten() {
        profile_skills.insert(name);
    }
    profile_skills
}

fn load_skill_usage_stats(
    profile_name: Option<&str>,
) -> std::collections::HashMap<String, (i64, i64)> {
    let mut stats = std::collections::HashMap::new();
    let conn = match get_connection() {
        Ok(c) => c,
        Err(_) => return stats,
    };
    let profile_str = profile_name.unwrap_or("");
    let mut stmt = match conn.prepare(
        "SELECT name, use_count, COALESCE(success_count, 0)
         FROM skills
         WHERE COALESCE(profile, '') = '' OR profile = ?1",
    ) {
        Ok(s) => s,
        Err(_) => return stats,
    };
    let rows = match stmt.query_map(params![profile_str], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, i64>(1).unwrap_or(0),
            r.get::<_, i64>(2).unwrap_or(0),
        ))
    }) {
        Ok(r) => r,
        Err(_) => return stats,
    };
    for (name, uses, successes) in rows.flatten() {
        stats.insert(name, (uses, successes));
    }
    stats
}

pub fn record_skill_activation(name: &str, profile_name: Option<&str>) {
    let safe_name = sanitize_skill_name(name);
    let profile_key = profile_name.unwrap_or("");
    let now = chrono::Utc::now().to_rfc3339();
    let _ = get_connection().map(|conn| {
        let updated = conn
            .execute(
                "UPDATE skills SET use_count = use_count + 1, last_used = ?1
                 WHERE name = ?2 AND COALESCE(profile, '') = ?3",
                params![now, safe_name, profile_key],
            )
            .unwrap_or(0);
        let _ = (updated == 0 && !profile_key.is_empty()).then(|| {
            let _ = conn.execute(
                "UPDATE skills SET use_count = use_count + 1, last_used = ?1
                 WHERE name = ?2 AND COALESCE(profile, '') = ''",
                params![now, safe_name],
            );
        });
    });
}

pub fn record_skill_outcome(name: &str, profile_name: Option<&str>, succeeded: bool) {
    match succeeded {
        false => {}
        true => {
            let safe_name = sanitize_skill_name(name);
            let profile_key = profile_name.unwrap_or("");
            let _ = get_connection().map(|conn| {
                let updated = conn
                    .execute(
                        "UPDATE skills
                         SET success_count = COALESCE(success_count, 0) + 1
                         WHERE name = ?1 AND COALESCE(profile, '') = ?2",
                        params![safe_name, profile_key],
                    )
                    .unwrap_or(0);
                let _ = (updated == 0 && !profile_key.is_empty()).then(|| {
                    let _ = conn.execute(
                        "UPDATE skills
                         SET success_count = COALESCE(success_count, 0) + 1
                         WHERE name = ?1 AND COALESCE(profile, '') = ''",
                        params![safe_name],
                    );
                });
            });
        }
    }
}

pub fn load_ranked_skill_matches_with_profile(
    user_content: &str,
    session_messages: &[crate::session::Message],
    profile_name: Option<&str>,
) -> Result<Vec<ScoredSkillMatch>> {
    let all_skills = load_skills_with_profile(profile_name)?;
    let profile_skills = load_profile_skill_names(profile_name);
    let usage_stats = load_skill_usage_stats(profile_name);
    Ok(ranker::rank_skills_bm25f(
        all_skills,
        user_content,
        session_messages,
        &profile_skills,
        &usage_stats,
    ))
}

pub fn load_relevant_skills_with_profile(
    user_content: &str,
    session_messages: &[crate::session::Message],
    profile_name: Option<&str>,
) -> Result<Vec<Skill>> {
    load_scored_relevant_skills_with_profile(user_content, session_messages, profile_name)
        .map(|scored| scored.into_iter().map(|(_, skill)| skill).collect())
}

pub fn load_scored_relevant_skills_with_profile(
    user_content: &str,
    session_messages: &[crate::session::Message],
    profile_name: Option<&str>,
) -> Result<Vec<(f32, Skill)>> {
    load_ranked_skill_matches_with_profile(user_content, session_messages, profile_name).map(
        |matches| {
            matches
                .into_iter()
                .map(|m| (m.score, m.skill))
                .collect()
        },
    )
}

fn locate_skill_package_dir(name: &str, profile: Option<&str>) -> Option<PathBuf> {
    let safe_name = sanitize_skill_name(name);
    let skills_config = crate::config::loader::load_config()
        .map(|config| config.skills)
        .unwrap_or_default();
    let mut candidate_roots = Vec::new();
    for (dir, _) in discover_workspace_and_external_dirs(&skills_config) {
        candidate_roots.push(dir);
    }
    let _ = profile.map(|prof| {
        candidate_roots.push(
            crate::config::runtime_data_dir()
                .join("subagents")
                .join(prof)
                .join("skills"),
        );
    });
    candidate_roots.push(get_skills_dir());

    candidate_roots.into_iter().find_map(|root| {
        let pkg_dir = root.join(&safe_name);
        (pkg_dir.join("SKILL.md").is_file()).then_some(pkg_dir)
    })
}

fn list_relative_files_under(pkg_dir: &std::path::Path, subdir: &str) -> Vec<String> {
    let target = pkg_dir.join(subdir);
    match target.is_dir() {
        false => Vec::new(),
        true => {
            let mut out = Vec::new();
            let _ = fs::read_dir(&target).map(|entries| {
                for entry in entries.flatten() {
                    let p = entry.path();
                    let _ = p
                        .is_file()
                        .then(|| {
                            p.file_name()
                                .and_then(|n| n.to_str())
                                .map(|name| format!("{subdir}/{name}"))
                        })
                        .flatten()
                        .map(|rel| out.push(rel));
                }
            });
            out.sort();
            out
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SkillBundleInspection {
    pub skill: Skill,
    pub references: Vec<String>,
    pub scripts: Vec<String>,
    pub templates: Vec<String>,
    pub assets: Vec<String>,
}

pub fn inspect_skill_bundle(
    name: &str,
    profile: Option<&str>,
) -> Result<Option<SkillBundleInspection>> {
    let safe_name = sanitize_skill_name(name);
    let skills = load_skills_with_profile(profile)?;
    let found = skills
        .into_iter()
        .find(|s| s.name == safe_name || sanitize_skill_name(&s.name) == safe_name);
    Ok(found.map(|skill| {
        let (references, scripts, templates, assets) =
            match locate_skill_package_dir(&skill.name, profile) {
                Some(pkg_dir) => (
                    list_relative_files_under(&pkg_dir, "references"),
                    list_relative_files_under(&pkg_dir, "scripts"),
                    list_relative_files_under(&pkg_dir, "templates"),
                    list_relative_files_under(&pkg_dir, "assets"),
                ),
                None => (Vec::new(), Vec::new(), Vec::new(), Vec::new()),
            };
        SkillBundleInspection {
            skill,
            references,
            scripts,
            templates,
            assets,
        }
    }))
}

pub fn inspect_skill_package(
    name: &str,
    profile: Option<&str>,
) -> Result<Option<(Skill, Vec<String>, Vec<String>)>> {
    inspect_skill_bundle(name, profile)
        .map(|opt| opt.map(|b| (b.skill, b.references, b.scripts)))
}

fn validate_skill_subpath(relative_path: &str) -> Result<&str> {
    let trimmed = relative_path.trim().trim_start_matches('/');
    let subdir_rest = trimmed
        .strip_prefix("references/")
        .or_else(|| trimmed.strip_prefix("scripts/"))
        .or_else(|| trimmed.strip_prefix("templates/"))
        .or_else(|| trimmed.strip_prefix("assets/"));
    let valid = subdir_rest.is_some_and(|rest| {
        !rest.trim().is_empty()
            && !rest.ends_with('/')
            && !trimmed.contains("..")
            && !trimmed.contains('\0')
    });
    match valid {
        true => Ok(trimmed),
        false => anyhow::bail!(
            "Skill package path '{}' must be inside references/, scripts/, templates/, or assets/ without '..'",
            relative_path
        ),
    }
}

pub fn read_skill_reference(
    name: &str,
    relative_path: &str,
    profile: Option<&str>,
) -> Result<String> {
    let pkg_dir = locate_skill_package_dir(name, profile)
        .ok_or_else(|| anyhow::anyhow!("Skill '{}' has no package directory", name))?;
    let trimmed = validate_skill_subpath(relative_path)?;
    let canonical_pkg = pkg_dir.canonicalize()?;
    let canonical_target = pkg_dir.join(trimmed).canonicalize()?;
    match canonical_target.starts_with(&canonical_pkg) && canonical_target.is_file() {
        true => Ok(fs::read_to_string(canonical_target)?),
        false => anyhow::bail!("Reference file '{}' is outside skill package", relative_path),
    }
}

fn writable_skill_package_dir(safe_name: &str, profile: Option<&str>) -> PathBuf {
    match profile {
        Some(prof) => crate::config::runtime_data_dir()
            .join("subagents")
            .join(prof)
            .join("skills")
            .join(safe_name),
        None => get_skills_dir().join(safe_name),
    }
}

pub fn write_skill_package_file(
    name: &str,
    relative_path: &str,
    file_content: &str,
    profile: Option<&str>,
) -> Result<PathBuf> {
    let safe_name = sanitize_skill_name(name);
    let trimmed = validate_skill_subpath(relative_path)?;
    match scan_skill_content(file_content).unwrap_or(false) {
        true => {}
        false => anyhow::bail!(
            "Skill supporting file '{}' rejected: contains potentially unsafe commands or patterns.",
            relative_path
        ),
    }

    let skills = load_skills_with_profile(profile)?;
    let existing = skills
        .into_iter()
        .find(|s| s.name == safe_name || sanitize_skill_name(&s.name) == safe_name)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Skill '{}' does not exist yet; create SKILL.md via curate_skill(action='add') first",
                safe_name
            )
        })?;

    let pkg_dir = writable_skill_package_dir(&safe_name, profile);
    fs::create_dir_all(&pkg_dir)?;
    let skill_md_path = pkg_dir.join("SKILL.md");
    match !skill_md_path.is_file() {
        true => fs::write(&skill_md_path, &existing.content)?,
        false => {}
    }

    let target_path = pkg_dir.join(trimmed);
    let parent = target_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Invalid parent directory for '{}'", relative_path))?;
    fs::create_dir_all(parent)?;
    let canonical_pkg = pkg_dir.canonicalize()?;
    let canonical_parent = parent.canonicalize()?;
    match canonical_parent.starts_with(&canonical_pkg) {
        true => {}
        false => anyhow::bail!("Supporting file path '{}' escapes skill package", relative_path),
    }

    fs::write(&target_path, file_content)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let make_exec =
            trimmed.starts_with("scripts/") && file_content.trim_start().starts_with("#!");
        let _ = make_exec.then(|| {
            let _ = fs::set_permissions(&target_path, fs::Permissions::from_mode(0o755));
        });
    }
    Ok(target_path)
}

pub fn remove_skill_package_file(
    name: &str,
    relative_path: &str,
    profile: Option<&str>,
) -> Result<()> {
    let safe_name = sanitize_skill_name(name);
    let trimmed = validate_skill_subpath(relative_path)?;
    let pkg_dir = writable_skill_package_dir(&safe_name, profile);
    match pkg_dir.is_dir() {
        false => anyhow::bail!("Skill package '{}' does not exist", safe_name),
        true => {
            let canonical_pkg = pkg_dir.canonicalize()?;
            let canonical_target = pkg_dir.join(trimmed).canonicalize()?;
            match canonical_target.starts_with(&canonical_pkg) && canonical_target.is_file() {
                true => {
                    fs::remove_file(canonical_target)?;
                    Ok(())
                }
                false => anyhow::bail!("Supporting file '{}' is outside skill package", relative_path),
            }
        }
    }
}

pub fn patch_skill(
    name: &str,
    old_text: Option<&str>,
    new_text: &str,
    profile: Option<&str>,
) -> Result<Skill> {
    let safe_name = sanitize_skill_name(name);
    let skills = load_skills_with_profile(profile)?;
    let existing = skills
        .into_iter()
        .find(|s| s.name == safe_name || sanitize_skill_name(&s.name) == safe_name)
        .ok_or_else(|| anyhow::anyhow!("Skill '{}' not found for patch", name))?;

    let updated_content = match old_text.map(str::trim).filter(|s| !s.is_empty()) {
        Some(needle) => match existing.content.contains(needle) {
            true => existing.content.replacen(needle, new_text, 1),
            false => anyhow::bail!(
                "old_text snippet was not found in skill '{}'; call curate_skill(action='view', skill_name='{}') first",
                safe_name,
                safe_name
            ),
        },
        None => format!("{}\n\n{}\n", existing.content.trim_end(), new_text.trim()),
    };

    match profile {
        Some(prof) => save_subagent_skill(prof, &safe_name, &updated_content)?,
        None => save_skill(&safe_name, &updated_content)?,
    }

    Ok(Skill {
        name: safe_name,
        content: updated_content,
    })
}

fn is_shell_name(tok: &str) -> bool {
    matches!(tok, "bash" | "sh" | "zsh" | "dash")
}

fn line_has_unsafe_skill_pattern(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    let trimmed = lower.trim();
    let tokens: Vec<&str> = trimmed.split_whitespace().collect();

    let has_fetcher = tokens
        .iter()
        .any(|t| matches!(*t, "curl" | "wget") || t.ends_with("/curl") || t.ends_with("/wget"));

    let pipes_to_shell = lower
        .split(['|', ';'])
        .skip(1)
        .filter_map(|seg| seg.split_whitespace().next())
        .any(is_shell_name);

    let ends_with_pipe_shell = lower
        .rsplit_once('|')
        .is_some_and(|(_, rhs)| is_shell_name(rhs.trim()));

    let has_exfil_keyword = ["evil.com", "leak", "steal", "exfiltrat"]
        .iter()
        .any(|kw| lower.contains(kw));

    let has_rm_rf_root = tokens
        .windows(3)
        .any(|w| w[0] == "rm" && w[1] == "-rf" && w[2].starts_with('/'));

    let has_chmod_777 = tokens.windows(2).any(|w| w[0] == "chmod" && w[1] == "777");

    let has_dev_tcp = lower
        .split_once("/dev/tcp/")
        .is_some_and(|(_, rest)| rest.chars().next().is_some_and(|c| c.is_ascii_digit()));

    let has_nc_exec = tokens
        .windows(3)
        .any(|w| w[0] == "nc" && w[1] == "-e" && w[2].starts_with('/'));

    let has_bash_reverse = tokens
        .windows(3)
        .any(|w| w[0] == "bash" && w[1] == "-i" && w[2].starts_with(">&"));

    (has_fetcher && (pipes_to_shell || has_exfil_keyword))
        || ends_with_pipe_shell
        || has_rm_rf_root
        || has_chmod_777
        || has_dev_tcp
        || has_nc_exec
        || has_bash_reverse
}

pub fn scan_skill_content(content: &str) -> Result<bool> {
    Ok(!content.lines().any(line_has_unsafe_skill_pattern))
}

fn sanitize_skill_name(name: &str) -> String {
    name.trim()
        .to_lowercase()
        .replace(|c: char| !c.is_alphanumeric() && c != '_' && c != '-', "_")
}

fn normalize_and_validate_skill(name: &str, content: &str) -> Result<(String, String)> {
    let safe_name = sanitize_skill_name(name);
    let trimmed = content.trim_start();
    let has_header = trimmed.starts_with('#') || trimmed.starts_with("---");
    let normalized_content = match has_header {
        true => content.to_string(),
        false => format!("# {}\n\n{}", safe_name.replace('_', " "), content.trim()),
    };

    match scan_skill_content(&normalized_content).unwrap_or(false) {
        true => Ok((safe_name, normalized_content)),
        false => anyhow::bail!(
            "Skill validation failed: content contains potentially unsafe commands or patterns."
        ),
    }
}

fn current_skill_content_in_db(
    conn: &Connection,
    safe_name: &str,
    profile: Option<&str>,
) -> Option<String> {
    let profile_key = profile.unwrap_or("");
    conn.query_row(
        "SELECT content FROM skills WHERE name = ?1 AND COALESCE(profile, '') = ?2 ORDER BY rowid DESC LIMIT 1",
        params![safe_name, profile_key],
        |row| row.get::<_, String>(0),
    )
    .ok()
    .or_else(|| {
        let base_dir = match profile {
            Some(prof) => crate::config::runtime_data_dir()
                .join("subagents")
                .join(prof)
                .join("skills"),
            None => get_skills_dir(),
        };
        let pkg_md = base_dir.join(safe_name).join("SKILL.md");
        let flat_md = base_dir.join(format!("{}.md", safe_name));
        pkg_md
            .is_file()
            .then(|| fs::read_to_string(pkg_md).ok())
            .flatten()
            .or_else(|| {
                flat_md
                    .is_file()
                    .then(|| fs::read_to_string(flat_md).ok())
                    .flatten()
            })
    })
}

fn record_skill_revision_snapshot(
    conn: &Connection,
    safe_name: &str,
    profile: Option<&str>,
    previous_content: &str,
) -> Result<()> {
    let profile_key = profile.unwrap_or("");
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO skill_revisions (name, profile, content, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![safe_name, profile_key, previous_content, now],
    )?;
    conn.execute(
        "DELETE FROM skill_revisions
         WHERE name = ?1 AND profile = ?2
           AND id NOT IN (
               SELECT id FROM skill_revisions
               WHERE name = ?1 AND profile = ?2
               ORDER BY id DESC LIMIT 10
           )",
        params![safe_name, profile_key],
    )?;
    Ok(())
}

pub fn list_skill_revisions(name: &str, profile: Option<&str>) -> Result<Vec<SkillRevision>> {
    let safe_name = sanitize_skill_name(name);
    let profile_key = profile.unwrap_or("");
    let conn = get_connection()?;
    let mut stmt = conn.prepare(
        "SELECT id, name, profile, content, created_at
         FROM skill_revisions
         WHERE name = ?1 AND profile = ?2
         ORDER BY id DESC",
    )?;
    let rows = stmt.query_map(params![safe_name, profile_key], |row| {
        let prof_raw: String = row.get(2)?;
        Ok(SkillRevision {
            revision_id: row.get(0)?,
            name: row.get(1)?,
            profile: (!prof_raw.is_empty()).then_some(prof_raw),
            content: row.get(3)?,
            created_at: row.get(4)?,
        })
    })?;
    Ok(rows.flatten().collect())
}

pub fn rollback_skill(
    name: &str,
    profile: Option<&str>,
    revision_id: Option<i64>,
) -> Result<Skill> {
    let safe_name = sanitize_skill_name(name);
    let profile_key = profile.unwrap_or("");
    let conn = get_connection()?;
    let (rev_id, target_content): (i64, String) = match revision_id {
        Some(explicit_id) => conn.query_row(
            "SELECT id, content FROM skill_revisions WHERE name = ?1 AND profile = ?2 AND id = ?3",
            params![safe_name, profile_key, explicit_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?,
        None => conn.query_row(
            "SELECT id, content FROM skill_revisions WHERE name = ?1 AND profile = ?2 ORDER BY id DESC LIMIT 1",
            params![safe_name, profile_key],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?,
    };

    let _ = conn.execute("DELETE FROM skill_revisions WHERE id = ?1", params![rev_id]);
    match profile {
        Some(prof) => save_subagent_skill(prof, &safe_name, &target_content)?,
        None => save_skill(&safe_name, &target_content)?,
    }
    Ok(Skill {
        name: safe_name,
        content: target_content,
    })
}

pub fn save_skill(name: &str, content: &str) -> Result<()> {
    let (safe_name, normalized_content) = normalize_and_validate_skill(name, content)?;

    let conn = get_connection()?;
    let _ = current_skill_content_in_db(&conn, &safe_name, None)
        .filter(|existing| existing != &normalized_content)
        .map(|existing| record_skill_revision_snapshot(&conn, &safe_name, None, &existing));

    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO skills (name, content, profile, created_at, last_used)
         VALUES (?1, ?2, '', ?3, ?3)
         ON CONFLICT(name, profile) DO UPDATE SET content = ?2, last_used = ?3",
        params![safe_name, normalized_content, now],
    )?;

    let global_skills_dir = get_skills_dir();
    let pkg_dir = global_skills_dir.join(&safe_name);
    let _ = fs::create_dir_all(&pkg_dir);
    let _ = fs::write(pkg_dir.join("SKILL.md"), &normalized_content);
    let legacy_flat = global_skills_dir.join(format!("{}.md", safe_name));
    let _ = fs::remove_file(&legacy_flat);

    Ok(())
}

pub fn delete_skill(name: &str) -> Result<()> {
    delete_skill_with_profile(name, None)
}

pub fn delete_skill_with_profile(name: &str, profile_name: Option<&str>) -> Result<()> {
    let safe_name = sanitize_skill_name(name);
    let conn = get_connection()?;
    let _ = current_skill_content_in_db(&conn, &safe_name, profile_name).map(|existing| {
        record_skill_revision_snapshot(&conn, &safe_name, profile_name, &existing)
    });

    match profile_name {
        Some(prof) => {
            conn.execute(
                "DELETE FROM skills WHERE name = ?1 AND profile = ?2",
                params![safe_name, prof],
            )?;
            let prof_dir = crate::config::runtime_data_dir()
                .join("subagents")
                .join(prof)
                .join("skills");
            let _ = fs::remove_file(prof_dir.join(format!("{}.md", safe_name)));
            let pkg_dir = prof_dir.join(&safe_name);
            match pkg_dir.is_dir() {
                true => {
                    let _ = fs::remove_dir_all(&pkg_dir);
                }
                false => {}
            }
        }
        None => {
            conn.execute(
                "DELETE FROM skills WHERE name = ?1 AND COALESCE(profile, '') = ''",
                params![safe_name],
            )?;
            let file_path = get_skills_dir().join(format!("{}.md", safe_name));
            let _ = fs::remove_file(&file_path);
            let dir_path = get_skills_dir().join(&safe_name);
            match dir_path.is_dir() {
                true => {
                    let _ = fs::remove_dir_all(&dir_path);
                }
                false => {}
            }
        }
    }

    Ok(())
}

pub fn clear_skills() -> Result<()> {
    let conn = get_connection()?;
    conn.execute("DELETE FROM skills", [])?;

    let skills_dir = get_skills_dir();
    match (skills_dir.is_dir(), fs::read_dir(&skills_dir)) {
        (true, Ok(entries)) => {
            for entry in entries.flatten() {
                let path = entry.path();
                let is_md =
                    path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("md");
                let is_pkg = path.is_dir()
                    && path.file_name().and_then(|n| n.to_str()) != Some("archive")
                    && path.join("SKILL.md").is_file();
                match (is_md, is_pkg) {
                    (true, _) => {
                        let _ = fs::remove_file(path);
                    }
                    (false, true) => {
                        let _ = fs::remove_dir_all(path);
                    }
                    (false, false) => {}
                }
            }
        }
        _ => {}
    }

    Ok(())
}

pub fn save_subagent_skill(profile: &str, name: &str, content: &str) -> Result<()> {
    let (safe_name, normalized_content) = normalize_and_validate_skill(name, content)?;

    let conn = get_connection()?;
    let _ = current_skill_content_in_db(&conn, &safe_name, Some(profile))
        .filter(|existing| existing != &normalized_content)
        .map(|existing| {
            record_skill_revision_snapshot(&conn, &safe_name, Some(profile), &existing)
        });

    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO skills (name, content, profile, created_at, last_used)
         VALUES (?1, ?2, ?3, ?4, ?4)
         ON CONFLICT(name, profile) DO UPDATE SET content = ?2, last_used = ?4",
        params![safe_name, normalized_content, profile, now],
    )?;

    let profile_skills_dir = crate::config::runtime_data_dir()
        .join("subagents")
        .join(profile)
        .join("skills");
    let pkg_dir = profile_skills_dir.join(&safe_name);
    let _ = fs::create_dir_all(&pkg_dir);
    let _ = fs::write(pkg_dir.join("SKILL.md"), &normalized_content);
    let _ = fs::remove_file(profile_skills_dir.join(format!("{}.md", safe_name)));

    Ok(())
}

#[cfg(test)]
#[path = "skills_tests.rs"]
mod tests;
