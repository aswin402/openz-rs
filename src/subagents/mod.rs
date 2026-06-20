use serde::{Serialize, Deserialize};
use std::fs;
use std::path::PathBuf;
use anyhow::{Result, anyhow, Context};
use crate::config::resolve_path;
use crate::config::schema::Config;
use crate::providers::GenerationSettings;
use crate::session::Message;
use inquire::{Text, Confirm};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SubagentProfile {
    pub name: String,         // e.g. "twitter_researcher"
    pub description: String,  // What it does
    pub system_prompt: String,// System prompt tailored for its role
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,        // Primary model override
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallbacks: Option<Vec<String>>, // Fallback models override
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

pub fn subagents_file_path() -> PathBuf {
    resolve_path("~/.openz/subagents.json")
}

pub fn load_profiles() -> Result<Vec<SubagentProfile>> {
    let path = subagents_file_path();
    let defaults = vec![
        SubagentProfile {
            name: "orchestrator".to_string(),
            description: "Coordinates complex, multi-stage project deliverables by planning and delegating subtasks to other specialized subagents.".to_string(),
            system_prompt: "You are the central Orchestrator. When given a complex goal, do not execute the technical steps yourself. Instead, create a step-by-step execution plan and delegate each subtask to the most appropriate specialized subagent (e.g., planner, researcher, coder, reviewer, debugger) using their tool calls. Compile their outputs and present the final results to the user.".to_string(),
            model: None,
            fallbacks: None,
            extra: serde_json::Map::new(),
        },
        SubagentProfile {
            name: "planner".to_string(),
            description: "Decomposes complex goals, manages workstreams, and tracks milestones.".to_string(),
            system_prompt: "You are a specialized Planner. Decompose the user's high-level task into clear, sequential milestones. Outline what needs to be checked, built, and verified at each stage.".to_string(),
            model: None,
            fallbacks: None,
            extra: serde_json::Map::new(),
        },
        SubagentProfile {
            name: "researcher".to_string(),
            description: "Searches the web, reads files, and gathers project context.".to_string(),
            system_prompt: "You are a specialized Researcher. ALWAYS check the local research cache first using the `search_research` tool before making external web queries or fetching URLs. Conduct thorough web searches, analyze codebase directories, read relevant files, and retrieve external documentation to compile complete reference contexts.".to_string(),
            model: None,
            fallbacks: None,
            extra: serde_json::Map::new(),
        },
        SubagentProfile {
            name: "architect".to_string(),
            description: "Designs system database schemas, directory layouts, and API contracts.".to_string(),
            system_prompt: "You are a specialized Architect. Design robust, performant system architectures, database tables, and API endpoints. Document your layouts clearly using structured schemas and Mermaid diagram definitions.".to_string(),
            model: None,
            fallbacks: None,
            extra: serde_json::Map::new(),
        },
        SubagentProfile {
            name: "skill_creator".to_string(),
            description: "Writes specialized helper scripts and creates new native shell tools dynamically.".to_string(),
            system_prompt: "You are a specialized Skill Creator. Design and write automated bash or Python scripts to solve recurring workflow bottlenecks. Focus on robust error handling and type safety.".to_string(),
            model: None,
            fallbacks: None,
            extra: serde_json::Map::new(),
        },
        SubagentProfile {
            name: "reviewer".to_string(),
            description: "Audits changed code files for security, logical bugs, and testing coverage.".to_string(),
            system_prompt: "You are a specialized Reviewer. Scan changed codebase files to identify security vulnerabilities, logical bugs, performance regressions, or style violations. Outline precise remediation recommendations.".to_string(),
            model: None,
            fallbacks: None,
            extra: serde_json::Map::new(),
        },
        SubagentProfile {
            name: "code_auditor".to_string(),
            description: "Performs security audits on source code.".to_string(),
            system_prompt: "You are a specialized Code Auditor. Scan source code files, identify security vulnerabilities, potential exploits, insecure dependency usage, or coding flaws. Provide clear remediation advice and secure alternatives.".to_string(),
            model: None,
            fallbacks: None,
            extra: serde_json::Map::new(),
        },
        SubagentProfile {
            name: "debugger".to_string(),
            description: "Diagnoses execution errors, reproduces bugs, and isolates root causes.".to_string(),
            system_prompt: "You are a specialized Debugger. Analyze system logs, stack traces, and failure modes to isolate root causes. Propose precise code changes to fix bugs and prevent regressions.".to_string(),
            model: None,
            fallbacks: None,
            extra: serde_json::Map::new(),
        },
        SubagentProfile {
            name: "test_engineer".to_string(),
            description: "Designs QA test suites and writes unit, integration, and E2E tests.".to_string(),
            system_prompt: "You are a specialized Test Engineer. Write comprehensive unit, integration, and end-to-end test cases. Ensure high code coverage and robust validation of edge conditions.".to_string(),
            model: None,
            fallbacks: None,
            extra: serde_json::Map::new(),
        },
        SubagentProfile {
            name: "devops_agent".to_string(),
            description: "Containerizes apps, drafts CI/CD pipelines, and manages infrastructure configs.".to_string(),
            system_prompt: "You are a specialized DevOps Agent. Write Dockerfiles, multi-stage builds, CI/CD workflow manifests (e.g. GitHub Actions), and infrastructure configuration scripts.".to_string(),
            model: None,
            fallbacks: None,
            extra: serde_json::Map::new(),
        },
        SubagentProfile {
            name: "refactor_agent".to_string(),
            description: "Cleans up code complexity, applies patterns, and optimizes code structure.".to_string(),
            system_prompt: "You are a specialized Refactoring Agent. Analyze source code to identify complexity hotspots, code smells, or duplicate blocks. Refactor code for optimal maintainability and DRY principles.".to_string(),
            model: None,
            fallbacks: None,
            extra: serde_json::Map::new(),
        },
        SubagentProfile {
            name: "memory_manager".to_string(),
            description: "Consolidates project facts, user preferences, and session context.".to_string(),
            system_prompt: "You are a specialized Memory Manager. Audit conversation transcripts to extract and save persistent project guidelines, user developer preferences, and critical decisions into markdown state files.".to_string(),
            model: None,
            fallbacks: None,
            extra: serde_json::Map::new(),
        },
        SubagentProfile {
            name: "vision_agent".to_string(),
            description: "Analyzes wireframes, mockups, and UI visual layout aesthetics.".to_string(),
            system_prompt: "You are a specialized Vision Agent. Review UI screenshots, frontend layouts, wireframe assets, or image outputs to evaluate visual contrast, alignment, styling quality, and pixel-perfect aesthetics. You can also analyze any image and describe its contents in detail.".to_string(),
            model: None,
            fallbacks: None,
            extra: serde_json::Map::new(),
        },
        SubagentProfile {
            name: "documentation_agent".to_string(),
            description: "Generates code docstrings, updates READMEs, and writes guides.".to_string(),
            system_prompt: "You are a specialized Documentation Agent. Maintain codebase clarity by writing docstrings, documenting module relations, updating README.md files, and writing onboarding guides.".to_string(),
            model: None,
            fallbacks: None,
            extra: serde_json::Map::new(),
        },
        SubagentProfile {
            name: "self_improvement".to_string(),
            description: "Curates, updates, and refines agent memories and procedural skills.".to_string(),
            system_prompt: "You are a specialized Self-Improvement Agent. Analyze user queries, feedback, style complaints, and task transcripts. Refine long-term memory facts, create or update procedural skills, write reusable style guidelines, and organize them under ~/.openz/skills/ so the agent learns and grows.".to_string(),
            model: None,
            fallbacks: None,
            extra: serde_json::Map::new(),
        },
        SubagentProfile {
            name: "skill_improvement".to_string(),
            description: "Audits, optimizes, and refines active agent skills inside ~/.openz/skills/.".to_string(),
            system_prompt: "You are a specialized Skill Improvement Agent. Your job is to audit, optimize, and refine active procedural skills inside ~/.openz/skills/. You have full access to read, list, add, and modify these markdown skill files using standard file tools. Read the existing files in ~/.openz/skills/, analyze compiler feedback, execution logs, or user styling preferences, and optimize, restructure, merge, or rewrite the skills to make the agent more accurate, efficient, and warning-free.".to_string(),
            model: None,
            fallbacks: None,
            extra: serde_json::Map::new(),
        },
        SubagentProfile {
            name: "openz_maintainer".to_string(),
            description: "Diagnoses internal errors, performance bugs, or loop detections inside OpenZ itself.".to_string(),
            system_prompt: "You are a specialized OpenZ Maintainer Agent. Your job is to debug, fix, and maintain the OpenZ application and codebase. If there are internal errors, system crashes, loop detection events, or performance regressions, review the OpenZ codebase and log files, diagnose the root cause, write code fixes, run compilation checks, and ensure system health.".to_string(),
            model: None,
            fallbacks: None,
            extra: serde_json::Map::new(),
        },
        SubagentProfile {
            name: "mcps_manager".to_string(),
            description: "Installs, configures, audits, and manages Model Context Protocol (MCP) servers and tools.".to_string(),
            system_prompt: "You are a specialized MCP Manager Agent. Your job is to install, configure, audit, and manage Model Context Protocol (MCP) servers and tools in OpenZ. You can read/write OpenZ configurations, verify system packages (node, npm, python, pip, uv, cargo), run installation commands for MCP package dependencies, and update the mcp_servers block in ~/.openz/config.json using standard tools. Use the 'exec_command' tool to test if dependencies are installed or to test run an MCP server.".to_string(),
            model: None,
            fallbacks: None,
            extra: serde_json::Map::new(),
        },
        SubagentProfile {
            name: "git_ops_agent".to_string(),
            description: "Handles version control, diff reviews, repository status, branching, and commits.".to_string(),
            system_prompt: "You are a specialized Git Operations Agent. Use the `git_manager` tool to inspect repository status, view diffs, stage files, create branches, and make clean, atomic commits with structured descriptions. Review git history when needed to understand code changes or solve conflicts.".to_string(),
            model: None,
            fallbacks: None,
            extra: serde_json::Map::new(),
        },
        SubagentProfile {
            name: "ast_searcher".to_string(),
            description: "Explores structural code architecture using AST structural grep, outline, and regex grep tools.".to_string(),
            system_prompt: "You are a specialized AST Searcher. Analyze structural organization of code using structural search tool `ast_grep`, file outlines via `code_outline`, and textual grep using `grep_search`. Locate class, struct, function definitions and find exact semantic matches to guide architectural decisions or code edits.".to_string(),
            model: None,
            fallbacks: None,
            extra: serde_json::Map::new(),
        },
        SubagentProfile {
            name: "database_specialist".to_string(),
            description: "Performs queries, reads tables, and inspects SQLite databases.".to_string(),
            system_prompt: "You are a specialized Database Specialist. Inspect and manage database schemas, tables, and records. Use the `db_inspector` tool to query the local SQLite databases, read schema structures, insert/update data, or audit db files safely. Always ensure performance indexing and query safety.".to_string(),
            model: None,
            fallbacks: None,
            extra: serde_json::Map::new(),
        },
        SubagentProfile {
            name: "browser_operator".to_string(),
            description: "Runs web browser automation, scrapes pages, and performs web crawling.".to_string(),
            system_prompt: "You are a specialized Browser Operator. Automate browser-based testing, site crawling, and scraping. Use the `gsd_browser` tool to run headless browser sessions (via Playwright), `obscura` for CDP browser automation, and `crawl` for site-wide multi-threaded crawling. Scrape tables, fetch DOM data, test frontend UI forms, and report issues.".to_string(),
            model: None,
            fallbacks: None,
            extra: serde_json::Map::new(),
        },
        SubagentProfile {
            name: "dependency_manager".to_string(),
            description: "Manages package dependencies, scaffolding stacks, and config files via onpkg.".to_string(),
            system_prompt: "You are a specialized Dependency and Package Manager. Use the package manager specified in `onpkg.json` or run `onpkg` tool commands directly to search stacks, add template files, scaffold projects, install new dependencies, or update configurations. Ensure package lockfiles are clean and dependency versions are compatible.".to_string(),
            model: None,
            fallbacks: None,
            extra: serde_json::Map::new(),
        },
        SubagentProfile {
            name: "frontend_architect".to_string(),
            description: "Designs and writes responsive, modern frontend interfaces, styling, and animations.".to_string(),
            system_prompt: "You are a specialized Frontend Architect. Write beautiful, responsive, and interactive frontend interfaces. Focus on modern typography, colors, layout components, Tailwind CSS styling, responsive breakpoints, and smooth micro-animations. Avoid basic browser defaults and adhere to premium design aesthetics.".to_string(),
            model: None,
            fallbacks: None,
            extra: serde_json::Map::new(),
        },
        SubagentProfile {
            name: "docs_lookup_agent".to_string(),
            description: "Queries external developer portals, API reference documents, and package manuals.".to_string(),
            system_prompt: "You are a specialized Documentation Lookup Agent. Search, fetch, and analyze developer documentation portals, API manuals, and package references. Compile precise usage examples, API options, and configuration maps to solve code integration questions.".to_string(),
            model: None,
            fallbacks: None,
            extra: serde_json::Map::new(),
        },
        SubagentProfile {
            name: "document_compiler".to_string(),
            description: "Compiles, extracts, and formats Word (.docx) and PDF files programmatically.".to_string(),
            system_prompt: "You are a specialized Document Compiler Agent. Parse, edit, and compile Word (.docx) documents and PDF files. Focus on template-driven generation, injecting structured data into branded documents, formatting page margins, tables, and typography, and using the `doc_reader` tool to inspect files.".to_string(),
            model: None,
            fallbacks: None,
            extra: serde_json::Map::new(),
        },
        SubagentProfile {
            name: "presentation_designer".to_string(),
            description: "Designs and structures PowerPoint (.pptx) and HTML presentations.".to_string(),
            system_prompt: "You are a specialized Presentation Designer Agent. Design and structure slide decks, outlines, speaker notes, and presentation layouts. Use python-pptx or HTML/CSS templates, ensuring clear title slides, bullet hierarchies, clean typography, and professional slides alignment.".to_string(),
            model: None,
            fallbacks: None,
            extra: serde_json::Map::new(),
        },
        SubagentProfile {
            name: "code_synthesizer".to_string(),
            description: "Generates boilerplate code, modular structures, and scaffolds project folders.".to_string(),
            system_prompt: "You are a specialized Code Synthesizer Agent. Scaffolding project directories, implementing clean boilerplate code structures, and writing modular packages. Ensure separation of concerns, DRY principles, and integrate with `onpkg` stack templates.".to_string(),
            model: None,
            fallbacks: None,
            extra: serde_json::Map::new(),
        },
        SubagentProfile {
            name: "summarizer_agent".to_string(),
            description: "Synthesizes large logs, scrapings, database tables, and long traces into high-density summaries.".to_string(),
            system_prompt: "You are a specialized Summarizer Agent. Parse and synthesize large text files, database rows, execution logs, and scraped pages. Condense data into high-density reference contexts, preserving essential metrics, error codes, and parameters without loss of technical value.".to_string(),
            model: None,
            fallbacks: None,
            extra: serde_json::Map::new(),
        },
        SubagentProfile {
            name: "media_designer".to_string(),
            description: "Generates custom geometric images, diagrams, charts, and illustrations using native image generation tools.".to_string(),
            system_prompt: "You are a specialized Media Designer. Your main role is to design and generate high-quality geometric graphics, visual flowcharts, custom charts, diagrams, and simple illustrations.\nUse the `generate_image` tool to render drawings, shapes, and texts precisely onto PNG buffers.\nWhen drawing:\n1. Plan your coordinates (x, y) and dimensions (width, height) carefully.\n2. Pair color harmonies with appropriate background/foreground contrast. Use hex color codes (e.g. #2563EB, #1F2937, #F3F4F6).\n3. Ensure text overlays are correctly centered or aligned using appropriate font-sizes and boundary checks to prevent overflow or clipping.\n4. You also understand OpenZ's core architecture, its default subagents, system features, and can operate basic filesystem/orchestrator tools like read, write, and list_dir to maintain configurations and verify assets.".to_string(),
            model: None,
            fallbacks: None,
            extra: serde_json::Map::new(),
        },
        SubagentProfile {
            name: "openz_coordinator".to_string(),
            description: "Coordinates complex workflows, delegates tasks to subagents, and manages OpenZ configurations and channels.".to_string(),
            system_prompt: "You are the OpenZ Coordinator & Orchestration Expert. Your role is to guide complex multi-step workflows, manage configurations, and delegate tasks to specialized subagents.\nYou are an expert in using orchestrator tools (filesystem read/write, shell execution, git control, SQLite db inspection, subagent delegation).\nYou understand all OpenZ features:\n- Commands: onboard, configure, agent (TUI), gateway (WS + WebUI), telegram, discord, whatsapp, subagent, sop, mcp-bridge.\n- Routing: keyword model prefix routing (e.g. anthropic/claude-3-5-sonnet).\n- Config: config.json configuration schema for 13 providers, 4 channels, and MCP servers.\n- Safeguards: SecurityGuard command/file write interceptions.\n- Mechanics: Context compactor, length auto-continuation, self-improvement background curator, dynamic subagent tool conversion.\nCoordinate subagents efficiently, verify changes using compiler/test checks, and keep the repository clean.".to_string(),
            model: None,
            fallbacks: None,
            extra: serde_json::Map::new(),
        },
        SubagentProfile {
            name: "sop_designer".to_string(),
            description: "Designs, validates, and simulates stateful Standard Operating Procedure (SOP) JSON workflows.".to_string(),
            system_prompt: "You are a specialized SOP Designer. Your main role is to design, test, and optimize stateful Standard Operating Procedure (SOP) JSON definitions for the OpenZ SOP engine. Ensure that JSON workflows contain structured steps, clear dependencies, parameter boundaries, and verification checks. Avoid dependency cycles and specify clear exit criteria.".to_string(),
            model: None,
            fallbacks: None,
            extra: serde_json::Map::new(),
        },
        SubagentProfile {
            name: "api_integrator".to_string(),
            description: "Discovers public/private APIs, designs OpenAPI specs, writes integration clients, and tests REST/GraphQL endpoints.".to_string(),
            system_prompt: "You are a specialized API Integrator. Your role is to explore public and private API manuals, design OpenAPI specifications, and write robust, asynchronous integration client packages. Focus on proper authentication setups, header configurations, error handling, rate limiting, and backoff/retry patterns.".to_string(),
            model: None,
            fallbacks: None,
            extra: serde_json::Map::new(),
        },
        SubagentProfile {
            name: "performance_tuner".to_string(),
            description: "Identifies runtime bottlenecks, analyzes profiling traces/flamegraphs, and optimizes async task scheduling.".to_string(),
            system_prompt: "You are a specialized Performance Tuner. Your role is to identify and resolve performance bottlenecks, memory leaks, and CPU/IO blockages. Analyze execution logs, profile traces, heap snapshots, and system metrics. Propose performance-optimized Rust structures, clean memory allocations, and proper Tokio async task scheduling configurations.".to_string(),
            model: None,
            fallbacks: None,
            extra: serde_json::Map::new(),
        },
        SubagentProfile {
            name: "communication_manager".to_string(),
            description: "Manages multi-channel messages, drafts notification templates, and configures email/SMTP routing.".to_string(),
            system_prompt: "You are a specialized Communication Manager. Your role is to format and dispatch structured notifications across OpenZ channels (WebSocket, Telegram, Discord, WhatsApp) and draft clean SMTP email templates using standard communication libraries. Ensure notifications are concise, clear, and prioritize important system alerts.".to_string(),
            model: None,
            fallbacks: None,
            extra: serde_json::Map::new(),
        },
        SubagentProfile {
            name: "automation_agent".to_string(),
            description: "Automates system tasks, clicks through UIs, manages cron schedules, webhooks, and browser interactions.".to_string(),
            system_prompt: "You are a specialized Automation Agent. Your role is to interact with the world: click through user interfaces, execute API requests, trigger webhooks, schedule recurring cron jobs, and perform browser automation (crawling, scraping, testing). Ensure all external integrations are executed correctly and safely.".to_string(),
            model: None,
            fallbacks: None,
            extra: serde_json::Map::new(),
        },
        SubagentProfile {
            name: "coding_agent".to_string(),
            description: "Generates, refactors, tests, and debugs code in sandboxed environments iteratively.".to_string(),
            system_prompt: "You are a specialized Coding Agent. Your role is to write, refactor, test, and debug code in sandboxed environments. You read compiler and test feedback, iteratively fix syntax/type errors, and verify the correctness of implementations using available test harnesses and compilation tools.".to_string(),
            model: None,
            fallbacks: None,
            extra: serde_json::Map::new(),
        },
        SubagentProfile {
            name: "diagram_designer".to_string(),
            description: "Creates and renders visual schemas, flowcharts, and diagrams using Mermaid syntax.".to_string(),
            system_prompt: "You are a specialized Diagram Designer. Your main role is to design and render structural graphs, sequence charts, and process layouts. Use the `render_mermaid` tool to generate and verify clean SVG diagrams, ensuring correct Mermaid syntax is used.".to_string(),
            model: None,
            fallbacks: None,
            extra: serde_json::Map::new(),
        },
        SubagentProfile {
            name: "video_animator".to_string(),
            description: "Programmatically designs and renders animations and videos using rendering libraries.".to_string(),
            system_prompt: "You are a specialized Video Animator. Your main role is to programmatically generate and animate short videos (MP4s). Use the `generate_video` tool to construct layers, paths, text animations, and transitions into a compilation timeline, then render it to video.".to_string(),
            model: None,
            fallbacks: None,
            extra: serde_json::Map::new(),
        }
    ];

    if !path.exists() {
        save_profiles(&defaults)?;
        return Ok(defaults);
    }

    let content = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read subagents file at {:?}", path))?;

    // Attempt to parse the content as a general JSON Value first
    let parsed_json: serde_json::Value = match serde_json::from_str(&content) {
        Ok(val) => val,
        Err(e) => {
            let backup_path = path.with_extension(format!("corrupt.{}", chrono::Utc::now().timestamp()));
            let _ = fs::copy(&path, &backup_path);
            eprintln!(
                "⚠️ Warning: Failed to parse subagents.json ({:?}). A backup was created at {:?}. Reverting to defaults.",
                e, backup_path
            );
            save_profiles(&defaults)?;
            return Ok(defaults);
        }
    };

    let mut loaded_profiles = Vec::new();
    let mut has_errors = false;

    if let serde_json::Value::Array(arr) = parsed_json {
        for item in arr {
            match serde_json::from_value::<SubagentProfile>(item.clone()) {
                Ok(profile) => {
                    loaded_profiles.push(profile);
                }
                Err(e) => {
                    has_errors = true;
                    eprintln!("⚠️ Warning: Failed to parse subagent profile: {:?}. Error: {:?}", item, e);
                }
            }
        }
    } else {
        has_errors = true;
        eprintln!("⚠️ Warning: subagents.json is not a JSON array. Reverting to defaults.");
    }

    if has_errors && loaded_profiles.is_empty() {
        let backup_path = path.with_extension(format!("corrupt.{}", chrono::Utc::now().timestamp()));
        let _ = fs::copy(&path, &backup_path);
        loaded_profiles = defaults.clone();
        save_profiles(&loaded_profiles)?;
        return Ok(loaded_profiles);
    }

    let mut migrated = false;
    for default_profile in defaults {
        if !loaded_profiles.iter().any(|p| p.name == default_profile.name) {
            loaded_profiles.push(default_profile);
            migrated = true;
        }
    }

    let default_names = vec![
        "planner", "researcher", "architect", "skill_creator", "reviewer",
        "code_auditor", "debugger", "test_engineer", "devops_agent",
        "refactor_agent", "memory_manager", "vision_agent", "documentation_agent",
        "self_improvement", "skill_improvement", "openz_maintainer", "mcps_manager",
        "git_ops_agent", "ast_searcher", "database_specialist", "browser_operator",
        "dependency_manager", "frontend_architect", "docs_lookup_agent",
        "document_compiler", "presentation_designer", "code_synthesizer",
        "summarizer_agent", "media_designer", "openz_coordinator",
        "sop_designer", "api_integrator", "performance_tuner", "communication_manager",
        "automation_agent", "coding_agent", "diagram_designer", "video_animator"
    ];

    for profile in &mut loaded_profiles {
        if default_names.contains(&profile.name.as_str()) {
            let is_old_default_model = match profile.model.as_deref() {
                Some("gpt-4o-mini") | Some("claude-3-5-sonnet") | Some("gpt-4o") | Some("google_ai_studio/gemini-2.0-flash") => true,
                _ => false,
            };
            if is_old_default_model {
                profile.model = None;
                profile.fallbacks = None;
                migrated = true;
            }
        }

        if let Some(ref mut fbs) = profile.fallbacks {
            fbs.retain(|s| !s.is_empty());
            if fbs.len() < 3 {
                while fbs.len() < 2 {
                    fbs.push(String::new());
                }
                fbs.push("openrouter/free".to_string());
                migrated = true;
            }
        }
    }

    if migrated || has_errors {
        save_profiles(&loaded_profiles)?;
    }

    Ok(loaded_profiles)
}

pub fn save_profiles(profiles: &[SubagentProfile]) -> Result<()> {
    let path = subagents_file_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(profiles)?;
    fs::write(&path, content)
        .with_context(|| format!("Failed to write subagents to {:?}", path))?;
    Ok(())
}

pub async fn run_subagent_manager(config: Config) -> Result<()> {
    let _ = load_profiles()?;
    let active_mdl = config.agents.defaults.model.clone();
    
    loop {
        let choices = vec![
            "List / Manage Subagents".to_string(),
            "Create New Subagent".to_string(),
            "Exit".to_string(),
        ];
        
        let choice_idx = match crate::agent::style::select_menu_custom(
            "Choose an option:",
            &choices,
            &active_mdl,
            Some("OpenZ Subagent Manager"),
            true,
        ) {
            Ok(Some(idx)) => idx,
            _ => break, // Exit on Esc
        };

        match choice_idx {
            0 => {
                if let Err(e) = manage_menu(&config).await {
                    eprintln!("Error managing subagents: {}", e);
                }
            }
            1 => {
                if let Err(e) = create_menu(&config).await {
                    eprintln!("Error creating subagent: {}", e);
                }
            }
            _ => {
                println!("Exiting subagent manager.");
                break;
            }
        }
    }
    Ok(())
}

async fn manage_menu(config: &Config) -> Result<()> {
    let mut profiles = load_profiles()?;
    if profiles.is_empty() {
        println!("No subagents currently configured.");
        return Ok(());
    }

    let active_mdl = config.agents.defaults.model.clone();
    let mut subagent_names: Vec<String> = profiles.iter().map(|p| p.name.clone()).collect();
    subagent_names.push("Back".to_string());

    loop {
        let name_choice_idx = match crate::agent::style::select_menu_custom(
            "Select a subagent to manage:",
            &subagent_names,
            &active_mdl,
            Some("List Subagents"),
            true,
        ) {
            Ok(Some(idx)) => idx,
            _ => break, // Go back on Esc
        };

        if name_choice_idx == profiles.len() {
            break; // Back option
        }

        let name_choice = &subagent_names[name_choice_idx];

        if let Some(pos) = profiles.iter().position(|p| p.name == *name_choice) {
            loop {
                let profile = &profiles[pos];
                println!("\n--- Subagent Details: {} ---", profile.name);
                println!("Description: {}", profile.description);
                println!("Primary Model: {}", profile.model.as_deref().unwrap_or("(default)"));
                let fallbacks_display = profile.fallbacks.clone().unwrap_or_else(|| config.get_dynamic_fallbacks(&profile.name));
                println!("Fallback Models: {:?}", fallbacks_display);
                println!("System Prompt:\n{}", profile.system_prompt);
                println!("------------------------------------");

                let options = vec![
                    "Modify Subagent".to_string(),
                    "Delete Subagent".to_string(),
                    "Back".to_string(),
                ];
                
                let action_idx = match crate::agent::style::select_menu_custom(
                    "Select action:",
                    &options,
                    &active_mdl,
                    Some(&format!("Manage: {}", profile.name)),
                    true,
                ) {
                    Ok(Some(idx)) => idx,
                    _ => break, // Go back on Esc
                };

                match action_idx {
                    0 => {
                        let mut modified = profile.clone();
                        modified.description = Text::new("Edit Description:")
                            .with_initial_value(&profile.description)
                            .prompt()?;
                        modified.system_prompt = Text::new("Edit System Prompt:")
                            .with_initial_value(&profile.system_prompt)
                            .prompt()?;
                        if let Some(selected_model) = prompt_choose_model("Edit Primary Model:", profile.model.as_deref().unwrap_or(""), config).await? {
                            modified.model = Some(selected_model);
                        }

                        let mut fallbacks = Vec::new();
                        for idx in 1..=3 {
                            let default_val = profile.fallbacks.as_ref().and_then(|f| f.get(idx - 1).cloned()).unwrap_or_default();
                            let label = format!("Edit Fallback Model {} (Exit/Esc to skip):", idx);
                            if let Some(fallback) = prompt_choose_model(&label, &default_val, config).await? {
                                fallbacks.push(fallback);
                            }
                        }
                        modified.fallbacks = Some(fallbacks);

                        profiles[pos] = modified;
                        save_profiles(&profiles)?;
                        println!("✅ Subagent modified successfully.");
                    }
                    1 => {
                        let confirm = Confirm::new(&format!("Are you sure you want to delete {}?", profile.name))
                            .with_default(false)
                            .prompt()?;
                        if confirm {
                            profiles.remove(pos);
                            save_profiles(&profiles)?;
                            println!("✅ Subagent deleted successfully.");
                            subagent_names = profiles.iter().map(|p| p.name.clone()).collect();
                            subagent_names.push("Back".to_string());
                            break;
                        }
                    }
                    _ => break,
                }
            }
        }
    }

    Ok(())
}

async fn create_menu(config: &Config) -> Result<()> {
    let creation_types = vec![
        "Create Manually".to_string(),
        "Create with AI (Ask OpenZ)".to_string(),
        "Back".to_string(),
    ];
    let active_mdl = config.agents.defaults.model.clone();

    loop {
        let choice_idx = match crate::agent::style::select_menu_custom(
            "How would you like to create the subagent?",
            &creation_types,
            &active_mdl,
            Some("Create Subagent"),
            true,
        ) {
            Ok(Some(idx)) => idx,
            _ => break, // Go back on Esc
        };

        if choice_idx == 2 {
            break; // Back option
        }

        match choice_idx {
            0 => {
                let name = Text::new("Enter Subagent Name (snake_case):").prompt()?;
                if name.trim().is_empty() {
                    return Err(anyhow!("Name cannot be empty."));
                }

                let description = Text::new("Enter Description:").prompt()?;
                let system_prompt = Text::new("Enter System Prompt:").prompt()?;
                let model = prompt_choose_model("Choose Primary Model (Enter/Esc for default):", &config.agents.defaults.model, config).await?;
                
                let mut fallbacks = Vec::new();
                for idx in 1..=3 {
                    let label = format!("Choose Fallback Model {} (Exit/Esc to skip):", idx);
                    if let Some(fallback) = prompt_choose_model(&label, "", config).await? {
                        fallbacks.push(fallback);
                    }
                }
                let fallbacks_opt = if fallbacks.is_empty() { None } else { Some(fallbacks) };

                let new_profile = SubagentProfile {
                    name: name.trim().to_string(),
                    description,
                    system_prompt,
                    model,
                    fallbacks: fallbacks_opt,
                    extra: serde_json::Map::new(),
                };

                let mut profiles = load_profiles()?;
                profiles.push(new_profile);
                save_profiles(&profiles)?;
                println!("✅ Subagent manual creation complete.");
                break;
            }
            1 => {
                let task_description = Text::new("Describe the specific task or role you want this subagent to perform:").prompt()?;
                if task_description.trim().is_empty() {
                    return Err(anyhow!("Description cannot be empty."));
                }

                println!("🧠 Asking OpenZ to design this subagent for you...");
                let ai_designed = ask_openz_to_design(config, &task_description).await?;

                println!("\n--- AI Designed Subagent Proposed ---");
                println!("Name: {}", ai_designed.name);
                println!("Description: {}", ai_designed.description);
                println!("Primary Model: {}", ai_designed.model.as_deref().unwrap_or("(default)"));
                let fallbacks_display = ai_designed.fallbacks.clone().unwrap_or_else(|| config.get_dynamic_fallbacks(&ai_designed.name));
                println!("Fallback Models: {:?}", fallbacks_display);
                println!("System Prompt:\n{}", ai_designed.system_prompt);
                println!("------------------------------------");

                let save_choice = Confirm::new("Save this AI-designed subagent?").with_default(true).prompt()?;
                if save_choice {
                    let mut profiles = load_profiles()?;
                    profiles.push(ai_designed);
                    save_profiles(&profiles)?;
                    println!("✅ AI-designed subagent saved successfully.");
                }
                break;
            }
            _ => {}
        }
    }

    Ok(())
}

async fn ask_openz_to_design(config: &Config, task_description: &str) -> Result<SubagentProfile> {
    // 1. Build provider
    let provider = crate::cli::build_agent_loop(config.clone()).await?.provider;

    // 2. Query LLM to generate profile JSON
    let system_prompt = "You are a specialized agent creator. Given a user's description of a task, design a custom subagent. \
        Return the output as a clean JSON block exactly matching this schema:\n\
        {\n\
          \"name\": \"snake_case_name\",\n\
          \"description\": \"One sentence summary of the subagent's role\",\n\
          \"system_prompt\": \"Detailed system prompt containing instructions, rules, and formats for this agent\"\n\
        }\n\
        Do not return any conversational text or markdown blocks, only the raw JSON.";

    let prompt = format!("Task description: {}", task_description);
    let messages = vec![Message {
        role: "user".to_string(),
        content: prompt,
        timestamp: Some(chrono::Utc::now().to_rfc3339()),
        extra: serde_json::Map::new(),
    }];

    let settings = GenerationSettings {
        temperature: 0.2,
        max_tokens: 1024,
        reasoning_effort: None,
    };

    let resp = provider.chat(system_prompt, &messages, &[], &settings).await?;
    let content = resp.content.ok_or_else(|| anyhow!("No design returned from AI"))?;

    // Parse JSON safely
    let cleaned_content = content
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim()
        .to_string();

    let ai_profile: SubagentProfile = serde_json::from_str(&cleaned_content)
        .with_context(|| format!("Failed to parse AI response as SubagentProfile. Response was: {}", content))?;

    Ok(ai_profile)
}


async fn prompt_choose_model(prompt_label: &str, current_model: &str, config: &Config) -> Result<Option<String>> {
    #[allow(dead_code)]
    struct ProviderModels {
        name: &'static str,
        display: &'static str,
        models: &'static [&'static str],
    }

    let all_providers = &[
        ProviderModels {
            name: "openai",
            display: "OpenAI (5)",
            models: &["gpt-4o", "gpt-4o-mini", "o1", "o1-mini", "o3-mini"],
        },
        ProviderModels {
            name: "anthropic",
            display: "Anthropic (3)",
            models: &["claude-3-5-sonnet-20241022", "claude-3-5-haiku-20241022", "claude-3-opus-20240229"],
        },
        ProviderModels {
            name: "openrouter",
            display: "OpenRouter (5)",
            models: &[
                "google/gemini-2.5-pro",
                "google/gemini-2.5-flash",
                "anthropic/claude-3.5-sonnet",
                "meta-llama/llama-3.3-70b-instruct",
                "deepseek/deepseek-r1",
            ],
        },
        ProviderModels {
            name: "deepseek",
            display: "DeepSeek (2)",
            models: &["deepseek-chat", "deepseek-reasoner"],
        },
        ProviderModels {
            name: "groq",
            display: "Groq (5)",
            models: &[
                "deepseek-r1-distill-llama-70b",
                "llama-3.3-70b-versatile",
                "llama-3.1-8b-instant",
                "mixtral-8x7b-32768",
                "gemma2-9b-it",
            ],
        },
        ProviderModels {
            name: "ollama",
            display: "Ollama (5)",
            models: &["llama3", "mistral", "phi3", "qwen2.5", "deepseek-r1"],
        },
        ProviderModels {
            name: "minimax",
            display: "minimax.io (6)",
            models: &[
                "MiniMax-M3",
                "MiniMax-M2.7",
                "MiniMax-M2.5",
                "MiniMax-M2.1",
                "MiniMax-M2",
                "MiniMax-M1",
            ],
        },
        ProviderModels {
            name: "mistral",
            display: "Mistral AI (5)",
            models: &[
                "mistral-large-latest",
                "pixtral-large-latest",
                "mistral-moderation-latest",
                "codestral-latest",
                "mistral-small-latest",
            ],
        },
        ProviderModels {
            name: "z.ai",
            display: "z.ai (Zhipu GLM) (5)",
            models: &[
                "glm-5.1",
                "glm-5",
                "glm-5v-turbo",
                "glm-4.7",
                "glm-4.7-flash",
            ],
        },
        ProviderModels {
            name: "nvidia",
            display: "NVIDIA NIM (5)",
            models: &[
                "meta/llama3-70b-instruct",
                "nvidia/llama-3.1-nemotron-70b-instruct",
                "meta/llama-3.1-70b-instruct",
                "mistralai/mixtral-8x22b-instruct-v0.1",
                "google/gemma-2-27b-it",
            ],
        },
        ProviderModels {
            name: "opencode_zen",
            display: "OpenCode Zen (4)",
            models: &[
                "deepseek-v4-flash-free",
                "mimo-v2.5-free",
                "north-mini-code-free",
                "nemotron-3-ultra-free",
            ],
        },
        ProviderModels {
            name: "cerebras",
            display: "Cerebras (3)",
            models: &[
                "llama-3.3-70b",
                "llama3.1-8b",
                "llama3.1-70b",
            ],
        },
        ProviderModels {
            name: "google_ai_studio",
            display: "Google AI Studio (Gemini) (4)",
            models: &[
                "gemini-2.5-pro",
                "gemini-2.5-flash",
                "gemini-2.0-flash",
                "gemini-1.5-pro",
            ],
        },
        ProviderModels {
            name: "cohere",
            display: "Cohere (3)",
            models: &[
                "command-r7-12-2025",
                "command-r-plus-08-2024",
                "command-r-08-2024",
            ],
        },
        ProviderModels {
            name: "llm7",
            display: "LLM7 (3)",
            models: &[
                "gpt-4o",
                "gpt-4o-mini",
                "claude-3-5-sonnet",
            ],
        },
        ProviderModels {
            name: "sambanova",
            display: "SambaNova (3)",
            models: &[
                "Meta-Llama-3.3-70B-Instruct",
                "Qwen2.5-72B-Instruct",
                "QwQ-32B",
            ],
        },
        ProviderModels {
            name: "huggingface",
            display: "Hugging Face Inference (3)",
            models: &[
                "meta-llama/Llama-3.3-70B-Instruct",
                "Qwen/QwQ-32B",
                "deepseek-ai/DeepSeek-R1",
            ],
        },
    ];

    let mut provider_list = Vec::new();
    for p in all_providers {
        if config.is_provider_configured(p.name) {
            provider_list.push(p);
        }
    }

    if provider_list.is_empty() {
        println!("{}⚠️ No LLM providers configured! Please run 'openz configure' first.{}", crate::agent::style::colors::AURA_GOLD, crate::agent::style::colors::COLOR_RESET);
        return Ok(None);
    }

    let mut provider_options: Vec<String> = provider_list.iter().map(|p| p.display.to_string()).collect();
    provider_options.push("Exit".to_string());

    match crate::agent::style::select_menu_custom(prompt_label, &provider_options, current_model, Some("Select Provider"), true)? {
        Some(prov_idx) => {
            if prov_idx == provider_list.len() {
                return Ok(None);
            }
            let prov_info = provider_list[prov_idx];
            
            let mut model_options = match crate::channels::fetch_provider_models(prov_info.name, config).await {
                Some(models) => models,
                None => prov_info.models.iter().map(|&m| m.to_string()).collect(),
            };
            model_options.push("Type manually (Custom Model)".to_string());
            model_options.push("Exit".to_string());
            
            match crate::agent::style::select_menu_custom(
                &format!("Choose a model from {}:", prov_info.display),
                &model_options,
                current_model,
                None,
                false,
            )? {
                Some(model_idx) => {
                    if model_idx == model_options.len() - 1 {
                        return Ok(None);
                    }
                    let mut final_model = if model_idx == model_options.len() - 2 {
                        let custom_model = inquire::Text::new("Enter custom model name:")
                            .with_initial_value(current_model)
                            .prompt()?;
                        if custom_model.trim().is_empty() {
                            return Ok(None);
                        }
                        custom_model.trim().to_string()
                    } else {
                        model_options[model_idx].clone()
                    };

                    let prefix = format!("{}/", prov_info.name);
                    let prefix_alt = if prov_info.name == "google_ai_studio" {
                        "google-ai-studio/".to_string()
                    } else if prov_info.name == "z.ai" {
                        "z_ai/".to_string()
                    } else if prov_info.name == "opencode_zen" {
                        "opencode-zen/".to_string()
                    } else {
                        "".to_string()
                    };

                    if !final_model.starts_with(&prefix) && (prefix_alt.is_empty() || !final_model.starts_with(&prefix_alt)) {
                        final_model = format!("{}{}", prefix, final_model);
                    }

                    Ok(Some(final_model))
                }
                None => Ok(None),
            }
        }
        None => Ok(None),
    }
}
