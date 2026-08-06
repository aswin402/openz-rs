//! OpenZ Ratatui theme — color palette ported from agent/style/colors.rs

use ratatui::style::{Color, Modifier, Style};

// ── Brand & Accent ──────────────────────────────────────────────────────────
pub const RED_ORANGE: Color = Color::Rgb(255, 69, 0);
pub const BRAND_PURPLE: Color = Color::Rgb(89, 0, 255);

// ── Aura Palette ────────────────────────────────────────────────────────────
pub const AURA_PURPLE: Color = Color::Rgb(255, 0, 191);
pub const AURA_BLUE: Color = Color::Rgb(130, 170, 255);
pub const AURA_GREEN: Color = Color::Rgb(0, 255, 0);
pub const AURA_GOLD: Color = Color::Rgb(229, 192, 123);
pub const AURA_ROSE: Color = Color::Rgb(255, 0, 0);
pub const AURA_SLATE: Color = Color::Rgb(107, 122, 153);

// ── Functional Colors ───────────────────────────────────────────────────────
pub const EMERALD: Color = Color::Rgb(16, 185, 129);
pub const HEADING_BLUE: Color = Color::Rgb(0, 175, 255);
pub const LIGHT_WHITE: Color = Color::Rgb(220, 220, 220);
pub const FOREGROUND: Color = Color::Rgb(248, 248, 242);
pub const DIM_TEXT: Color = Color::Rgb(98, 114, 164);

// ── Semantic Styles ─────────────────────────────────────────────────────────

/// Style for user message prefix `> `
pub fn user_prefix_style() -> Style {
    Style::default().fg(RED_ORANGE)
}

/// Style for user message text
pub fn user_text_style() -> Style {
    Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
}

/// Style for assistant/default text
pub fn assistant_text_style() -> Style {
    Style::default().fg(FOREGROUND)
}

/// Style for the `● Thought for Xs` line
pub fn thinking_bullet_style() -> Style {
    Style::default().fg(RED_ORANGE)
}

/// Style for the thinking time text
pub fn thinking_time_style() -> Style {
    Style::default().fg(RED_ORANGE).add_modifier(Modifier::BOLD)
}

/// Style for reasoning content (`L ...`)
pub fn reasoning_style() -> Style {
    Style::default().fg(AURA_SLATE)
}

/// Style for tool bullet `● Tool Name`
pub fn tool_bullet_style() -> Style {
    Style::default().fg(RED_ORANGE)
}

/// Style for tool name
pub fn tool_name_style() -> Style {
    Style::default().fg(LIGHT_WHITE).add_modifier(Modifier::BOLD)
}

/// Style for tool args/details
pub fn tool_details_style() -> Style {
    Style::default().fg(AURA_SLATE)
}

/// Style for success checkmark `✓`
pub fn success_style() -> Style {
    Style::default().fg(AURA_GREEN)
}

/// Style for error marker `✗`
pub fn error_style() -> Style {
    Style::default().fg(AURA_ROSE)
}

/// Style for divider lines
pub fn divider_style() -> Style {
    Style::default().fg(AURA_SLATE)
}

/// Style for the input prompt `› `
pub fn input_prefix_style() -> Style {
    Style::default().fg(DIM_TEXT)
}

/// Style for placeholder text in empty input
pub fn placeholder_style() -> Style {
    Style::default().fg(AURA_SLATE)
}

/// Style for status bar text
pub fn status_bar_style() -> Style {
    Style::default().fg(AURA_SLATE)
}

/// Style for status bar accent (model/provider names)
pub fn status_accent_style() -> Style {
    Style::default().fg(DIM_TEXT)
}

/// Style for MCP diamond `◇`
pub fn mcp_diamond_style() -> Style {
    Style::default().fg(AURA_PURPLE)
}

/// Style for MCP count with success
pub fn mcp_success_style() -> Style {
    Style::default().fg(AURA_GREEN)
}

/// Style for warning text
pub fn warning_style() -> Style {
    Style::default().fg(AURA_GOLD)
}

/// Style for markdown headings
pub fn heading_style() -> Style {
    Style::default().fg(HEADING_BLUE).add_modifier(Modifier::BOLD)
}

/// Style for inline code
pub fn code_style() -> Style {
    Style::default().fg(EMERALD)
}

/// Style for bold text
pub fn bold_style() -> Style {
    Style::default().fg(FOREGROUND).add_modifier(Modifier::BOLD)
}

/// Style for list markers
pub fn list_marker_style() -> Style {
    Style::default().fg(AURA_GREEN)
}

/// Spinner frames for the animated working indicator
pub const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
