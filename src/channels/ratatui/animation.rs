use super::theme::Theme;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

/// Quadrant rotating square blocks spinner
pub const SQUARE_SPINNER_FRAMES: &[&str] = &["▖", "▘", "▝", "▗"];

/// Classic high-speed smooth Braille orbital spinner
pub const BRAILLE_SPINNER_FRAMES: &[&str] = &[
    "⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏",
];

/// Concentric stepped squares spinner
pub const CONCENTRIC_SPINNER_FRAMES: &[&str] = &["▫", "◽", "◻", "⬜", "◻", "◽"];

/// Pulsating block density spinner
pub const PULSE_SPINNER_FRAMES: &[&str] = &["░", "▒", "▓", "█", "▓", "▒"];

/// Dual-Pillars brand spinner (alternating left and right pillar states)
pub const BRAND_SPINNER_FRAMES: &[(&str, &str)] = &[
    ("▰", "▱"),
    ("▰", "▰"),
    ("▱", "▰"),
    ("▱", "▱"),
];

/// Rotating corner frame quadrants
pub const CORNER_ANGLES_FRAMES: &[&str] = &["◰", "◳", "◲", "◱"];

/// Horizontal scanning progress beam
pub const SCANNER_BAR_FRAMES: &[&str] = &[
    "▏", "▎", "▍", "▌", "▋", "▊", "▉", "█", "▉", "▊", "▋", "▌", "▍", "▎",
];

/// Dual orbital particle dots
pub const PARTICLE_ORBIT_FRAMES: &[&str] = &["⠋", "⠙", "⠚", "⠞", "⠦", "⠴", "⠲", "⠳"];

/// Supported loading animation styles
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpinnerStyle {
    #[default]
    BrailleWave,
    DualPillars,
    Quadrants,
    PulseMatrix,
    Concentric,
    CornerAngles,
    ScannerBar,
    ParticleOrbit,
}

impl SpinnerStyle {
    pub fn from_id(id: &str) -> Self {
        match id {
            "braille_wave" => SpinnerStyle::BrailleWave,
            "dual_pillars" => SpinnerStyle::DualPillars,
            "quadrants" => SpinnerStyle::Quadrants,
            "pulse_matrix" => SpinnerStyle::PulseMatrix,
            "concentric" => SpinnerStyle::Concentric,
            "corner_angles" => SpinnerStyle::CornerAngles,
            "scanner_bar" => SpinnerStyle::ScannerBar,
            "particle_orbit" => SpinnerStyle::ParticleOrbit,
            _ => SpinnerStyle::BrailleWave,
        }
    }

    pub fn to_id(self) -> &'static str {
        match self {
            SpinnerStyle::DualPillars => "dual_pillars",
            SpinnerStyle::Quadrants => "quadrants",
            SpinnerStyle::PulseMatrix => "pulse_matrix",
            SpinnerStyle::Concentric => "concentric",
            SpinnerStyle::BrailleWave => "braille_wave",
            SpinnerStyle::CornerAngles => "corner_angles",
            SpinnerStyle::ScannerBar => "scanner_bar",
            SpinnerStyle::ParticleOrbit => "particle_orbit",
        }
    }
}

/// Represents the specific activity the agent is executing
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentActivity {
    Thinking,
    Working,
    Responding,
    Generating,
    Planning,
    Writing { target: String },
    Analyzing { task: String },
    InternetResearch { query: String },
    DeepResearch { query: String },
    RepoResearch { target: String },
    EditingFile { path: String },
    Debugging { step: String },
    ExecutingCommand { command: String },
    SubagentWorking { role: String },
    DocumentProcessing { task: String },
    MediaGenerating { task: String },
    CompactingContext,
}

#[derive(Debug, Clone, Copy)]
enum ToolCategory {
    DeepResearch,
    InternetResearch,
    DocumentProcessing,
    MediaGenerating,
    Writing,
    EditingFile,
    CommandExec,
    Planning,
    SubagentWorking,
    RepoResearch,
    CompactingContext,
    Working,
}

fn classify_tool_category(tool_lower: &str) -> ToolCategory {
    const DEEP_RESEARCH_PATTERNS: &[&str] = &["deep_research", "deep research"];
    const INTERNET_EXACT: &[&str] = &[
        "search_web",
        "web_fetch",
        "fetch_or_browse",
        "browser_navigate",
    ];
    const INTERNET_CONTAINS: &[&str] = &["searchxyz", "crawl", "web", "fetch", "browse"];
    const DOC_CONTAINS: &[&str] = &["opendoc", "read_doc", "doc_reader"];
    const MEDIA_CONTAINS: &[&str] = &[
        "openmedia",
        "generate_video",
        "generate_image",
        "svg_animator",
        "html_video",
        "video",
        "image",
    ];
    const WRITE_EXACT: &[&str] = &["write_file", "write_to_file", "create_file"];
    const EDIT_EXACT: &[&str] = &[
        "patch_file",
        "replace_file_content",
        "multi_replace_file_content",
        "edit_file",
        "replace_lines",
        "ast_replace_node",
    ];
    const EDIT_CONTAINS: &[&str] = &["patch", "edit", "replace"];
    const EXEC_EXACT: &[&str] = &["exec_cmd", "sandbox_exec", "run_command", "exec_command"];
    const EXEC_CONTAINS: &[&str] = &["exec", "cmd", "command", "bash", "terminal", "cargo"];
    const PLAN_CONTAINS: &[&str] = &["sequentialthinking", "reasoning", "think", "plan"];
    const SUBAGENT_CONTAINS: &[&str] = &["subagent", "orchestrat", "delegate", "sop", "swarm"];
    const REPO_EXACT: &[&str] = &[
        "read_file",
        "find_files",
        "grep_search",
        "code_graph",
        "semantic_search",
        "ast_grep",
        "code_outline",
        "recall_query",
        "explore_codebase",
    ];
    const REPO_CONTAINS: &[&str] = &["graph", "search", "explore", "read", "find"];
    const COMPACT_CONTAINS: &[&str] = &["compact", "headroom"];

    let matches_any = |patterns: &[&str]| patterns.iter().any(|p| tool_lower.contains(p));
    let equals_any = |patterns: &[&str]| patterns.contains(&tool_lower);
    let is_write = equals_any(WRITE_EXACT)
        || (tool_lower.contains("write") && !tool_lower.contains("db_write"));

    let candidates: &[(bool, ToolCategory)] = &[
        (matches_any(DEEP_RESEARCH_PATTERNS), ToolCategory::DeepResearch),
        (
            equals_any(INTERNET_EXACT) || matches_any(INTERNET_CONTAINS),
            ToolCategory::InternetResearch,
        ),
        (matches_any(DOC_CONTAINS), ToolCategory::DocumentProcessing),
        (matches_any(MEDIA_CONTAINS), ToolCategory::MediaGenerating),
        (is_write, ToolCategory::Writing),
        (
            equals_any(EDIT_EXACT) || matches_any(EDIT_CONTAINS),
            ToolCategory::EditingFile,
        ),
        (
            equals_any(EXEC_EXACT) || matches_any(EXEC_CONTAINS),
            ToolCategory::CommandExec,
        ),
        (matches_any(PLAN_CONTAINS), ToolCategory::Planning),
        (matches_any(SUBAGENT_CONTAINS), ToolCategory::SubagentWorking),
        (
            equals_any(REPO_EXACT) || matches_any(REPO_CONTAINS),
            ToolCategory::RepoResearch,
        ),
        (
            matches_any(COMPACT_CONTAINS),
            ToolCategory::CompactingContext,
        ),
    ];

    candidates
        .iter()
        .find_map(|(matched, cat)| match *matched {
            true => Some(*cat),
            false => None,
        })
        .unwrap_or(ToolCategory::Working)
}

fn truncate_summary_text(s: &str) -> String {
    match s.len() > 36 {
        true => format!("{}…", &s[..35]),
        false => s.to_string(),
    }
}

impl AgentActivity {
    /// Classifies a tool call and its arguments into an activity
    pub fn from_tool_call(tool: &str, args_str: &str) -> Self {
        let tool_lower = tool.to_ascii_lowercase();
        match classify_tool_category(&tool_lower) {
            ToolCategory::DeepResearch => {
                let query = Self::extract_arg_or_default(
                    args_str,
                    &["query", "topic", "prompt", "target"],
                    "research topic",
                );
                AgentActivity::DeepResearch { query }
            }
            ToolCategory::InternetResearch => {
                let query = Self::extract_arg_or_default(
                    args_str,
                    &["query", "url", "target"],
                    "web query",
                );
                AgentActivity::InternetResearch { query }
            }
            ToolCategory::DocumentProcessing => {
                let task = Self::extract_arg_or_default(
                    args_str,
                    &["file_path", "path", "title"],
                    "document",
                );
                AgentActivity::DocumentProcessing { task }
            }
            ToolCategory::MediaGenerating => {
                let task = Self::extract_arg_or_default(
                    args_str,
                    &["title", "output_path", "prompt", "diagram_type"],
                    "media asset",
                );
                AgentActivity::MediaGenerating { task }
            }
            ToolCategory::Writing => {
                let target = Self::extract_arg_or_default(
                    args_str,
                    &["target_file", "path", "file_path", "target"],
                    "file",
                );
                AgentActivity::Writing { target }
            }
            ToolCategory::EditingFile => {
                let path = Self::extract_arg_or_default(
                    args_str,
                    &["target_file", "path", "file_path", "target"],
                    "file",
                );
                AgentActivity::EditingFile { path }
            }
            ToolCategory::CommandExec => {
                let cmd = Self::extract_arg_or_default(
                    args_str,
                    &["command", "command_line", "cmd"],
                    "command",
                );
                let cmd_lower = cmd.to_ascii_lowercase();
                const DEBUG_KEYWORDS: &[&str] = &[
                    "check", "test", "lint", "clippy", "tsc", "pytest", "vitest", "diff",
                ];
                match DEBUG_KEYWORDS.iter().any(|k| cmd_lower.contains(k)) {
                    true => AgentActivity::Debugging { step: cmd },
                    false => AgentActivity::ExecutingCommand { command: cmd },
                }
            }
            ToolCategory::Planning => AgentActivity::Planning,
            ToolCategory::SubagentWorking => {
                let role = Self::extract_arg_or_default(
                    args_str,
                    &["role", "name", "profile", "task"],
                    "agent task",
                );
                AgentActivity::SubagentWorking { role }
            }
            ToolCategory::RepoResearch => {
                let target = Self::extract_arg_or_default(
                    args_str,
                    &["query", "path", "symbol", "name", "pattern", "target"],
                    "codebase",
                );
                AgentActivity::RepoResearch { target }
            }
            ToolCategory::CompactingContext => AgentActivity::CompactingContext,
            ToolCategory::Working => AgentActivity::Working,
        }
    }

    fn extract_arg_or_default(args_str: &str, keys: &[&str], default: &str) -> String {
        let trimmed = args_str.trim();
        match trimmed.is_empty() {
            true => return default.to_string(),
            false => {}
        }

        // 1. Try JSON parsing
        match serde_json::from_str::<serde_json::Value>(trimmed) {
            Ok(v) => {
                for key in keys {
                    match v.get(*key).and_then(|val| val.as_str()).map(str::trim) {
                        Some(s) => match s.is_empty() {
                            false => return truncate_summary_text(s),
                            true => {}
                        },
                        None => {}
                    }
                }
            }
            Err(_) => {}
        }

        // 2. Try parsing key: "value" from formatted strings
        for key in keys {
            let pattern = format!("{}:", key);
            match trimmed.find(&pattern) {
                Some(pos) => {
                    let rest = trimmed[pos + pattern.len()..].trim();
                    let extracted = match rest.strip_prefix('"') {
                        Some(stripped) => match stripped.find('"') {
                            Some(end_quote) => &stripped[..end_quote],
                            None => stripped,
                        },
                        None => rest.split(',').next().unwrap_or(rest).trim(),
                    };
                    let clean = extracted.trim();
                    match clean.is_empty() {
                        false => return truncate_summary_text(clean),
                        true => {}
                    }
                }
                None => {}
            }
        }

        // 3. If it starts with quotes, extract quoted content
        match trimmed.strip_prefix('"') {
            Some(stripped) => match stripped.find('"') {
                Some(end_quote) => {
                    let clean = &stripped[..end_quote];
                    match clean.is_empty() {
                        false => return truncate_summary_text(clean),
                        true => {}
                    }
                }
                None => {}
            },
            None => {}
        }

        // 4. Simple non-empty string under 36 chars without commas/colons
        match !trimmed.contains(':') && !trimmed.contains('{') && trimmed.len() <= 36 {
            true => trimmed.trim_matches('"').to_string(),
            false => default.to_string(),
        }
    }

    /// Derives an initial context-aware activity from the user's prompt text
    pub fn from_user_prompt(prompt: &str) -> Self {
        let trimmed = prompt.trim();
        let lower = trimmed.to_ascii_lowercase();

        const RESEARCH_PREFIXES: &[&str] =
            &["search ", "find ", "look up ", "what is ", "who is "];
        const WRITE_PREFIXES: &[&str] = &[
            "write ",
            "create ",
            "implement ",
            "build ",
            "code ",
            "generate ",
            "add ",
        ];
        const DEBUG_PREFIXES: &[&str] =
            &["test ", "check ", "debug ", "fix ", "verify ", "run "];
        const ANALYZE_PREFIXES: &[&str] =
            &["analyze ", "explain ", "review ", "why ", "how ", "audit "];
        const PLAN_PREFIXES: &[&str] = &["plan ", "design ", "outline ", "architect "];

        let starts_any = |prefixes: &[&str]| prefixes.iter().any(|p| lower.starts_with(p));
        let is_research = starts_any(RESEARCH_PREFIXES)
            || lower.contains("deep research")
            || lower.contains("research ");

        match (
            is_research,
            lower.contains("deep research"),
            starts_any(WRITE_PREFIXES),
            starts_any(DEBUG_PREFIXES),
            starts_any(ANALYZE_PREFIXES),
            starts_any(PLAN_PREFIXES),
        ) {
            (true, true, _, _, _, _) => AgentActivity::DeepResearch {
                query: Self::extract_prompt_summary(trimmed),
            },
            (true, false, _, _, _, _) => AgentActivity::InternetResearch {
                query: Self::extract_prompt_summary(trimmed),
            },
            (false, _, true, _, _, _) => AgentActivity::Writing {
                target: Self::extract_prompt_summary(trimmed),
            },
            (false, _, false, true, _, _) => AgentActivity::Debugging {
                step: Self::extract_prompt_summary(trimmed),
            },
            (false, _, false, false, true, _) => AgentActivity::Analyzing {
                task: Self::extract_prompt_summary(trimmed),
            },
            (false, _, false, false, false, true) => AgentActivity::Planning,
            _ => AgentActivity::Thinking,
        }
    }

    fn extract_prompt_summary(prompt: &str) -> String {
        let first_line = prompt.lines().next().unwrap_or(prompt).trim();
        let cleaned = first_line
            .strip_prefix("please ")
            .or_else(|| first_line.strip_prefix("Please "))
            .or_else(|| first_line.strip_prefix("can you "))
            .or_else(|| first_line.strip_prefix("Can you "))
            .unwrap_or(first_line);

        truncate_summary_text(cleaned)
    }
}

/// Extracts RGB components from any Ratatui Color safely
pub fn color_to_rgb(color: Color) -> (u8, u8, u8) {
    match color {
        Color::Rgb(r, g, b) => (r, g, b),
        Color::Reset => (200, 200, 200),
        Color::Black => (18, 18, 24),
        Color::Red => (255, 100, 100),
        Color::Green => (100, 255, 160),
        Color::Yellow => (255, 215, 100),
        Color::Blue => (100, 180, 255),
        Color::Magenta => (240, 120, 255),
        Color::Cyan => (100, 240, 255),
        Color::Gray => (140, 140, 150),
        Color::DarkGray => (80, 80, 90),
        Color::LightRed => (255, 140, 140),
        Color::LightGreen => (140, 255, 180),
        Color::LightYellow => (255, 235, 140),
        Color::LightBlue => (140, 200, 255),
        Color::LightMagenta => (255, 160, 255),
        Color::LightCyan => (140, 250, 255),
        Color::White => (245, 245, 245),
        Color::Indexed(idx) => (idx, idx, idx),
    }
}

/// Computes linear interpolation between two colors
pub fn lerp_color(c1: Color, c2: Color, factor: f32) -> Color {
    let t = factor.clamp(0.0, 1.0);
    let (r1, g1, b1) = color_to_rgb(c1);
    let (r2, g2, b2) = color_to_rgb(c2);

    let r = (r1 as f32 + (r2 as f32 - r1 as f32) * t).round() as u8;
    let g = (g1 as f32 + (g2 as f32 - g1 as f32) * t).round() as u8;
    let b = (b1 as f32 + (b2 as f32 - b1 as f32) * t).round() as u8;

    Color::Rgb(r, g, b)
}

/// Renders a string where each character is styled along a dynamic sine wave gradient
pub fn render_shimmer_spans(
    text: &str,
    c1: Color,
    c2: Color,
    millis: u64,
    speed: f32,
    frequency: f32,
) -> Vec<Span<'static>> {
    let mut spans = Vec::with_capacity(text.chars().count());
    let base_phase = (millis as f32) / speed;

    for (i, ch) in text.chars().enumerate() {
        let phase = base_phase - (i as f32 * frequency);
        let factor = (phase.sin() * 0.5 + 0.5).clamp(0.0, 1.0);
        let color = lerp_color(c1, c2, factor);
        spans.push(Span::styled(
            ch.to_string(),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ));
    }
    spans
}

/// Renders the spinner glyph spans according to the selected SpinnerStyle
pub fn render_spinner_spans(
    style: SpinnerStyle,
    millis: u64,
    c1: Color,
    c2: Color,
    theme: &Theme,
) -> Vec<Span<'static>> {
    match style {
        SpinnerStyle::DualPillars => {
            let spinner_idx = ((millis / 120) as usize) % BRAND_SPINNER_FRAMES.len();
            let (left_glyph, right_glyph) = BRAND_SPINNER_FRAMES[spinner_idx];
            let left_color = match left_glyph {
                "▰" => theme.brand_accent,
                _ => theme.muted,
            };
            let right_color = match right_glyph {
                "▰" => theme.info,
                _ => theme.muted,
            };
            vec![
                Span::styled(
                    left_glyph,
                    Style::default().fg(left_color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{}  ", right_glyph),
                    Style::default()
                        .fg(right_color)
                        .add_modifier(Modifier::BOLD),
                ),
            ]
        }
        SpinnerStyle::Quadrants => {
            let idx = ((millis / 80) as usize) % SQUARE_SPINNER_FRAMES.len();
            vec![Span::styled(
                format!("{}  ", SQUARE_SPINNER_FRAMES[idx]),
                Style::default().fg(c1).add_modifier(Modifier::BOLD),
            )]
        }
        SpinnerStyle::PulseMatrix => {
            let idx = ((millis / 100) as usize) % PULSE_SPINNER_FRAMES.len();
            vec![Span::styled(
                format!("{}  ", PULSE_SPINNER_FRAMES[idx]),
                Style::default().fg(c1).add_modifier(Modifier::BOLD),
            )]
        }
        SpinnerStyle::Concentric => {
            let idx = ((millis / 120) as usize) % CONCENTRIC_SPINNER_FRAMES.len();
            vec![Span::styled(
                format!("{}  ", CONCENTRIC_SPINNER_FRAMES[idx]),
                Style::default().fg(c1).add_modifier(Modifier::BOLD),
            )]
        }
        SpinnerStyle::BrailleWave => {
            let idx = ((millis / 80) as usize) % BRAILLE_SPINNER_FRAMES.len();
            vec![Span::styled(
                format!("{}  ", BRAILLE_SPINNER_FRAMES[idx]),
                Style::default()
                    .fg(theme.brand_accent)
                    .add_modifier(Modifier::BOLD),
            )]
        }
        SpinnerStyle::CornerAngles => {
            let idx = ((millis / 100) as usize) % CORNER_ANGLES_FRAMES.len();
            vec![Span::styled(
                format!("{}  ", CORNER_ANGLES_FRAMES[idx]),
                Style::default().fg(c1).add_modifier(Modifier::BOLD),
            )]
        }
        SpinnerStyle::ScannerBar => {
            let idx = ((millis / 70) as usize) % SCANNER_BAR_FRAMES.len();
            vec![Span::styled(
                format!("{}  ", SCANNER_BAR_FRAMES[idx]),
                Style::default().fg(c1).add_modifier(Modifier::BOLD),
            )]
        }
        SpinnerStyle::ParticleOrbit => {
            let idx = ((millis / 80) as usize) % PARTICLE_ORBIT_FRAMES.len();
            vec![Span::styled(
                format!("{}  ", PARTICLE_ORBIT_FRAMES[idx]),
                Style::default().fg(c2).add_modifier(Modifier::BOLD),
            )]
        }
    }
}

/// Renders the complete live activity line with loading spinner,
/// dynamic shimmering gradient text, and elapsed timer.
pub fn render_live_activity_line(
    activity: &AgentActivity,
    spinner_style: SpinnerStyle,
    millis: u64,
    elapsed_secs: f64,
    theme: &Theme,
) -> Line<'static> {
    let (label, c1, c2) = match activity {
        AgentActivity::Thinking => (
            "Thinking...".to_string(),
            theme.brand_accent,
            theme.brand_white,
        ),
        AgentActivity::Working => (
            "Working...".to_string(),
            theme.brand_accent,
            theme.brand_white,
        ),
        AgentActivity::Responding => (
            "Generating response...".to_string(),
            theme.info,
            theme.brand_white,
        ),
        AgentActivity::Generating => (
            "Generating response...".to_string(),
            theme.info,
            theme.brand_white,
        ),
        AgentActivity::Planning => (
            "Planning next steps...".to_string(),
            theme.highlight,
            theme.brand_white,
        ),
        AgentActivity::Writing { target } => (
            format!("Writing {}...", target),
            theme.success,
            theme.brand_white,
        ),
        AgentActivity::Analyzing { task } => (
            format!("Analyzing {}...", task),
            theme.warning,
            theme.brand_white,
        ),
        AgentActivity::InternetResearch { query } => (
            format!("Searching web: \"{}\"...", query),
            theme.info,
            theme.brand_white,
        ),
        AgentActivity::DeepResearch { query } => (
            format!("Deep research: \"{}\"...", query),
            theme.info,
            theme.brand_white,
        ),
        AgentActivity::RepoResearch { target } => (
            format!("Searching codebase: {}...", target),
            theme.info,
            theme.brand_white,
        ),
        AgentActivity::EditingFile { path } => (
            format!("Editing {}...", path),
            theme.info,
            theme.brand_white,
        ),
        AgentActivity::Debugging { step } => (
            format!("Running check: {}...", step),
            theme.warning,
            theme.brand_white,
        ),
        AgentActivity::ExecutingCommand { command } => (
            format!("Executing: {}...", command),
            theme.info,
            theme.brand_white,
        ),
        AgentActivity::SubagentWorking { role } => (
            format!("Subagent active: {}...", role),
            theme.brand_accent,
            theme.brand_white,
        ),
        AgentActivity::DocumentProcessing { task } => (
            format!("Processing document: {}...", task),
            theme.warning,
            theme.brand_white,
        ),
        AgentActivity::MediaGenerating { task } => (
            format!("Generating media: {}...", task),
            theme.highlight,
            theme.brand_white,
        ),
        AgentActivity::CompactingContext => (
            "Compacting memory...".to_string(),
            theme.muted,
            theme.brand_white,
        ),
    };

    let mut spans = render_spinner_spans(spinner_style, millis, c1, c2, theme);

    // Dynamic wave shimmer across the text: speed 160.0, frequency 0.32
    let shimmer_spans = render_shimmer_spans(&label, c1, c2, millis, 160.0, 0.32);
    spans.extend(shimmer_spans);

    // Elapsed timer & interrupt prompt
    spans.push(Span::styled(
        format!(" ({:.1}s • esc to interrupt)", elapsed_secs),
        Style::default().fg(theme.muted),
    ));

    Line::from(spans)
}
