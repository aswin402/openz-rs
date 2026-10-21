use anyhow::Result;
use rusqlite::{params, Connection};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Skill {
    pub name: String,
    pub content: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillView {
    pub name: String,
    pub content: String,
    pub scope: String,
    pub profile: Option<String>,
    pub source: String,
    pub path: Option<String>,
    pub enabled: bool,
    pub is_protected: bool,
    pub use_count: i64,
    pub created_at: Option<String>,
    pub last_used: Option<String>,
    pub validation_errors: Vec<String>,
}

impl Skill {
    pub fn description(&self) -> String {
        extract_frontmatter_field(&self.content, "description")
            .or_else(|| {
                self.content
                    .lines()
                    .map(str::trim)
                    .find(|trimmed| {
                        !trimmed.starts_with('#')
                            && !trimmed.starts_with("---")
                            && !trimmed.is_empty()
                    })
                    .map(|trimmed| trimmed.chars().take(160).collect::<String>())
            })
            .unwrap_or_else(|| {
                format!("Procedural guidelines for {}", self.name.replace('_', " "))
            })
    }

    pub fn triggers(&self) -> Vec<String> {
        let mut triggers: Vec<String> = extract_frontmatter_field(&self.content, "triggers")
            .map(|trig_str| {
                let cleaned_str = trig_str.trim();
                let inner = cleaned_str
                    .strip_prefix('[')
                    .and_then(|s| s.strip_suffix(']'))
                    .unwrap_or(cleaned_str);
                inner
                    .split(',')
                    .map(|part| part.trim().trim_matches(['"', '\'']).to_lowercase())
                    .filter(|cleaned| !cleaned.is_empty())
                    .collect()
            })
            .unwrap_or_default();

        match triggers.is_empty() {
            true => {
                triggers.extend(
                    self.name
                        .split('_')
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
            .or_else(|| extract_frontmatter_field(&self.content, "category"))
    }
}

pub fn extract_frontmatter_field(content: &str, field: &str) -> Option<String> {
    let trimmed = content.trim_start();
    let rest = trimmed.strip_prefix("---")?;
    let end_idx = rest.find("---")?;
    let yaml_block = &rest[..end_idx];
    let field_prefix = format!("{}:", field);
    yaml_block.lines().find_map(|line| {
        line.trim()
            .strip_prefix(&field_prefix)
            .map(|val| val.trim().trim_matches(['"', '\'']).to_string())
    })
}

pub fn get_user_profile_path() -> PathBuf {
    crate::config::runtime_data_dir().join("USER.md")
}

pub fn load_user_profile() -> String {
    let path = get_user_profile_path();
    match path.is_file() {
        true => fs::read_to_string(path).unwrap_or_default(),
        false => String::new(),
    }
}

fn clean_bullet_line(line: &str) -> Option<&str> {
    let l = line.trim();
    match l.starts_with('-') || l.starts_with('*') {
        true => Some(
            l[1..]
                .trim()
                .trim_start_matches("**")
                .trim_end_matches("**")
                .trim(),
        ),
        false => None,
    }
}

pub fn update_user_profile(new_preferences: &str) -> Result<()> {
    let trimmed_input = new_preferences.trim();
    match trimmed_input.is_empty() {
        true => return Ok(()),
        false => {}
    }

    let path = get_user_profile_path();
    match path.parent() {
        Some(parent) => {
            let _ = fs::create_dir_all(parent);
        }
        None => {}
    }

    let existing = match path.is_file() {
        true => fs::read_to_string(&path).unwrap_or_default(),
        false => String::new(),
    };

    let mut existing_bullets: Vec<String> = Vec::new();
    for text in existing.lines().filter_map(clean_bullet_line) {
        let should_add = !text.is_empty()
            && !existing_bullets
                .iter()
                .any(|b| b.eq_ignore_ascii_case(text));
        match should_add {
            true => existing_bullets.push(text.to_string()),
            false => {}
        }
    }

    for line in trimmed_input.lines() {
        let text = clean_bullet_line(line).unwrap_or_else(|| line.trim());
        let should_add = text.len() > 3
            && !existing_bullets.iter().any(|b| {
                b.eq_ignore_ascii_case(text) || b.contains(text) || text.contains(b.as_str())
            });
        match should_add {
            true => existing_bullets.push(text.to_string()),
            false => {}
        }
    }

    let mut updated_content = String::from("# User Profile & Preferences 👤\n\nDurable user preferences, styling tastes, persona rules, and coding conventions across sessions.\n\n## Preferences\n");
    for bullet in &existing_bullets {
        updated_content.push_str(&format!("- {}\n", bullet));
    }

    fs::write(path, updated_content)?;
    Ok(())
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
            let valid = skill_file.exists() && skill_file.is_file() && !name.is_empty();
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
            profile TEXT,
            use_count INTEGER DEFAULT 0,
            last_used TEXT,
            created_at TEXT,
            PRIMARY KEY (name, profile)
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
            for entry in fs::read_dir(&global_dir)? {
                let entry = entry?;
                let path = entry.path();
                let is_md =
                    path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("md");
                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string();
                let valid = is_md && !name.is_empty() && name != "INDEX" && !name.ends_with(".bak");
                match valid {
                    false => continue,
                    true => match fs::read_to_string(&path) {
                        Ok(content) => {
                            let now = chrono::Utc::now().to_rfc3339();
                            let _ = conn.execute(
                                "INSERT OR IGNORE INTO skills (name, content, profile, created_at, last_used)
                                 VALUES (?1, ?2, NULL, ?3, ?3)",
                                params![name, content, now],
                            );
                        }
                        Err(_) => {}
                    },
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
1. Automate UI regression test flows via Playwright scripts.
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
  - Prefer reading specific line ranges via `view_file` over reading entire large files.
  - Locate targets using structural outlines (`code_outline`) or semantic queries (`ast_grep`) rather than generic substring lookups.
  - Limit multi-line replacement scopes to contiguous blocks using `replace_file_content` or non-contiguous chunks using `multi_replace_file_content`.

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
1. Inspect codebase and project layout before making changes (`list_dir`, `view_file`, `grep_search`).
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
1. Conduct browser automation (scraping, E2E testing, visual QA) using Playwright, CDP, or web scrapers.
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
    }
    Ok(())
}

pub fn load_skills() -> Result<Vec<Skill>> {
    load_skills_with_profile(None)
}

fn add_file_skill_views(
    dir: &std::path::Path,
    source: &str,
    views: &mut std::collections::HashMap<String, SkillView>,
) -> Result<()> {
    for (name, skill_path) in discover_skill_files_in_dir(dir)? {
        let content = fs::read_to_string(&skill_path)?;
        views.insert(
            name.clone(),
            SkillView {
                validation_errors: validate_skill_metadata(&name, &content),
                name,
                content,
                scope: source.to_string(),
                profile: None,
                source: source.to_string(),
                path: Some(skill_path.display().to_string()),
                enabled: true,
                is_protected: true,
                use_count: 0,
                created_at: None,
                last_used: None,
            },
        );
    }
    Ok(())
}

pub fn load_skill_views() -> Result<Vec<SkillView>> {
    let mut views = std::collections::HashMap::<String, SkillView>::new();

    match get_connection() {
        Ok(conn) => {
            let mut stmt = conn.prepare(
                "SELECT name, content, profile, use_count, created_at, last_used FROM skills",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, i64>(3).unwrap_or(0),
                    row.get::<_, Option<String>>(4).unwrap_or(None),
                    row.get::<_, Option<String>>(5).unwrap_or(None),
                ))
            })?;

            let db_path = get_db_path().display().to_string();
            for row in rows.flatten() {
                let (name, content, profile, use_count, created_at, last_used) = row;
                let scope = profile
                    .as_ref()
                    .map(|p| format!("subagent:{p}"))
                    .unwrap_or_else(|| "global".to_string());
                let key = profile
                    .as_ref()
                    .map(|p| format!("{p}/{name}"))
                    .unwrap_or_else(|| name.clone());
                let is_protected = profile.is_some();
                views.insert(
                    key,
                    SkillView {
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
                        created_at,
                        last_used,
                    },
                );
            }
        }
        Err(_) => {}
    }

    let skills_config = crate::config::loader::load_config()
        .map(|config| config.skills)
        .unwrap_or_default();
    match skills_config.workspace_skills_enabled {
        true => add_file_skill_views(&get_workspace_skills_dir(), "workspace", &mut views)?,
        false => {}
    }
    for external_dir in skills_config.external_dirs {
        let dir = crate::config::resolve_path(&external_dir);
        add_file_skill_views(&dir, "external", &mut views)?;
    }

    Ok(views.into_values().collect())
}

pub fn load_skills_with_profile(profile_name: Option<&str>) -> Result<Vec<Skill>> {
    let mut skills_map = std::collections::HashMap::new();

    match get_connection() {
        Ok(conn) => {
            let mut stmt = conn.prepare(
                "SELECT name, content FROM skills WHERE profile IS NULL OR profile = ?",
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
    match skills_config.workspace_skills_enabled {
        true => load_skill_files_from_dir(&get_workspace_skills_dir(), &mut skills_map)?,
        false => {}
    }
    for external_dir in skills_config.external_dirs {
        let dir = crate::config::resolve_path(&external_dir);
        load_skill_files_from_dir(&dir, &mut skills_map)?;
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

pub fn archive_stale_skills() -> Result<()> {
    let check_path = crate::config::runtime_data_dir().join("last_stale_skills_check.json");
    match checked_within_last_24h(&check_path) {
        true => return Ok(()),
        false => {}
    }

    match get_connection() {
        Ok(conn) => {
            let now = chrono::Utc::now();
            let stale_duration = chrono::Duration::days(30);

            let mut stmt = conn.prepare("SELECT name, profile, last_used, content FROM skills")?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })?;

            let mut to_delete = Vec::new();
            for (name, profile, last_used_opt, content) in rows.flatten() {
                let is_stale = last_used_opt
                    .as_deref()
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .is_some_and(|last_used_date| {
                        let last_used_utc = last_used_date.with_timezone(&chrono::Utc);
                        now.signed_duration_since(last_used_utc) > stale_duration
                    });
                match is_stale {
                    true => {
                        let archive_dir = get_skills_dir().join("archive");
                        fs::create_dir_all(&archive_dir)?;
                        let dest_path = archive_dir.join(format!("{}.md", name));
                        let _ = fs::write(dest_path, content);
                        to_delete.push((name, profile));
                    }
                    false => {}
                }
            }

            for (name, profile) in to_delete {
                let _ = conn.execute(
                    "DELETE FROM skills WHERE name = ?1 AND profile IS ?2",
                    params![name, profile],
                );

                let aura_blue = "\x1b[38;2;96;165;250m";
                let color_reset = "\x1b[0m";
                crate::channels::cli::send_notification(&format!(
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

fn is_stop_word(w: &str) -> bool {
    matches!(
        w,
        "the"
            | "and"
            | "for"
            | "but"
            | "not"
            | "you"
            | "your"
            | "this"
            | "that"
            | "with"
            | "from"
            | "have"
            | "are"
            | "was"
            | "were"
            | "been"
            | "has"
            | "had"
            | "will"
            | "would"
            | "should"
            | "could"
            | "can"
            | "about"
            | "there"
            | "their"
            | "here"
            | "them"
            | "then"
            | "than"
            | "standard"
            | "openz"
            | "agent"
            | "tool"
            | "logic"
            | "code"
            | "implementation"
            | "project"
            | "file"
            | "custom"
            | "user"
            | "does"
            | "done"
            | "some"
            | "what"
            | "when"
            | "where"
            | "which"
            | "who"
            | "how"
            | "why"
    )
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
    let mut stmt = match conn.prepare("SELECT name FROM skills WHERE profile = ?") {
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

pub fn load_relevant_skills_with_profile(
    user_content: &str,
    session_messages: &[crate::session::Message],
    profile_name: Option<&str>,
) -> Result<Vec<Skill>> {
    let all_skills = load_skills_with_profile(profile_name)?;
    match all_skills.is_empty() {
        true => return Ok(Vec::new()),
        false => {}
    }

    let profile_skills = load_profile_skill_names(profile_name);

    let mut search_context = user_content.to_lowercase();
    for msg in session_messages.iter().rev().take(3) {
        search_context.push(' ');
        search_context.push_str(&msg.content.to_lowercase());
    }

    let search_words: Vec<&str> = search_context
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 3 && !is_stop_word(w))
        .collect();

    let mut relevant_scored = Vec::new();
    for skill in all_skills {
        let is_profile_specific = profile_skills.contains(&skill.name);
        let triggers = skill.triggers();
        let trigger_match = triggers
            .iter()
            .any(|t| !t.trim().is_empty() && search_context.contains(&t.to_lowercase()));
        let name_exact_match = search_context.contains(&skill.name.to_lowercase());
        let name_match = skill
            .name
            .split('_')
            .any(|word| word.len() > 2 && search_context.contains(word));

        let desc_sample = skill
            .content
            .chars()
            .take(500)
            .collect::<String>()
            .to_lowercase();
        let content_match = search_words.iter().any(|word| desc_sample.contains(*word));

        let score = [
            (trigger_match, 10.0f32),
            (name_exact_match, 8.0f32),
            (is_profile_specific, 5.0f32),
            (name_match, 3.0f32),
            (content_match, 1.0f32),
        ]
        .into_iter()
        .fold(0.0f32, |acc, (matched, weight)| match matched {
            true => acc + weight,
            false => acc,
        });

        match score > 0.0 {
            true => relevant_scored.push((score, skill)),
            false => {}
        }
    }

    relevant_scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let relevant: Vec<Skill> = relevant_scored.into_iter().map(|(_, s)| s).collect();

    match (!relevant.is_empty(), get_connection()) {
        (true, Ok(conn)) => {
            let now = chrono::Utc::now().to_rfc3339();
            for skill in &relevant {
                match profile_skills.contains(&skill.name) {
                    true => {
                        let _ = conn.execute(
                            "UPDATE skills SET use_count = use_count + 1, last_used = ?1 WHERE name = ?2 AND profile = ?3",
                            params![now, skill.name, profile_name.unwrap_or("")],
                        );
                    }
                    false => {
                        let _ = conn.execute(
                            "UPDATE skills SET use_count = use_count + 1, last_used = ?1 WHERE name = ?2 AND profile IS NULL",
                            params![now, skill.name],
                        );
                    }
                }
            }
        }
        _ => {}
    }

    Ok(relevant)
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
    name.to_lowercase()
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

pub fn save_skill(name: &str, content: &str) -> Result<()> {
    let (safe_name, normalized_content) = normalize_and_validate_skill(name, content)?;

    let conn = get_connection()?;
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO skills (name, content, profile, created_at, last_used)
         VALUES (?1, ?2, NULL, ?3, ?3)
         ON CONFLICT(name, profile) DO UPDATE SET content = ?2, last_used = ?3",
        params![safe_name, normalized_content, now],
    )?;

    let global_skills_dir = get_skills_dir();
    let _ = fs::create_dir_all(&global_skills_dir);
    let skill_file = global_skills_dir.join(format!("{}.md", safe_name));
    let _ = fs::write(&skill_file, &normalized_content);

    Ok(())
}

pub fn delete_skill(name: &str) -> Result<()> {
    delete_skill_with_profile(name, None)
}

pub fn delete_skill_with_profile(name: &str, profile_name: Option<&str>) -> Result<()> {
    let safe_name = sanitize_skill_name(name);
    let conn = get_connection()?;

    match profile_name {
        Some(prof) => {
            conn.execute(
                "DELETE FROM skills WHERE name = ?1 AND profile = ?2",
                params![safe_name, prof],
            )?;
            let file_path = crate::config::runtime_data_dir()
                .join("subagents")
                .join(prof)
                .join("skills")
                .join(format!("{}.md", safe_name));
            let _ = fs::remove_file(&file_path);
        }
        None => {
            conn.execute(
                "DELETE FROM skills WHERE name = ?1 AND profile IS NULL",
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
                match is_md {
                    true => {
                        let _ = fs::remove_file(path);
                    }
                    false => {}
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
    let _ = fs::create_dir_all(&profile_skills_dir);
    let skill_file = profile_skills_dir.join(format!("{}.md", safe_name));
    let _ = fs::write(&skill_file, &normalized_content);

    Ok(())
}

#[cfg(test)]
#[path = "skills_tests.rs"]
mod tests;
