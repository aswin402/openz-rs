//! OpenZ Ratatui Theme — Aura Dark + Red-Orange Brand Palette
//! Ported and enhanced with Dalton Menezes' official Aura Theme specifications

use ratatui::style::{Color, Modifier, Style};

/// Complete Aura Dark + Red-Orange OpenZ Theme
#[derive(Debug, Clone)]
pub struct Theme {
    pub bg_primary: Color,   // #15141b (Aura Deep Dark)
    pub bg_elevated: Color,  // #21202e (Aura Elevated Surface)
    pub bg_input: Color,     // #29263c (Aura Input Dock)
    pub brand_accent: Color, // #ff5500 (OpenZ Vibrant Red-Orange)
    pub brand_white: Color,  // #f0f0f0 (OpenZ Crisp White)
    pub success: Color,      // #61ffca (Aura Mint Green)
    pub destructive: Color,  // #ff6767 (Aura Coral Red)
    pub warning: Color,      // #ffca85 (Aura Warm Amber)
    pub highlight: Color,    // #f694ff (Aura Pink / Highlight)
    pub info: Color,         // #82e2ff (Aura Cyan Blue)
    pub text_primary: Color, // #edecee (Aura Bright Text)
    pub muted: Color,        // #8a8a93 (Aura Slate Comment)
    pub border: Color,       // #3d375e (Aura Line Border)
}

// ── Static Color Constants for Backward Compatibility ────────────────────────
pub const RED_ORANGE: Color = Color::Rgb(255, 85, 0);
pub const BRAND_WHITE: Color = Color::Rgb(240, 240, 240);
pub const AURA_PURPLE: Color = Color::Rgb(162, 119, 255);
pub const AURA_BLUE: Color = Color::Rgb(130, 226, 255);
pub const AURA_GREEN: Color = Color::Rgb(97, 255, 202);
pub const AURA_GOLD: Color = Color::Rgb(255, 202, 133);
pub const AURA_ROSE: Color = Color::Rgb(255, 103, 103);
pub const AURA_SLATE: Color = Color::Rgb(138, 138, 147);
pub const EMERALD: Color = Color::Rgb(97, 255, 202);
pub const HEADING_BLUE: Color = Color::Rgb(130, 226, 255);
pub const LIGHT_WHITE: Color = Color::Rgb(240, 240, 240);
pub const FOREGROUND: Color = Color::Rgb(237, 236, 238);
pub const DIM_TEXT: Color = Color::Rgb(138, 138, 147);

impl Theme {
    /// Official OpenZ Aura Dark Theme (Default)
    pub fn aura_dark() -> Self {
        Self {
            bg_primary: Color::Rgb(21, 20, 27),      // #15141b
            bg_elevated: Color::Rgb(33, 32, 46),     // #21202e
            bg_input: Color::Rgb(41, 38, 60),        // #29263c
            brand_accent: Color::Rgb(255, 85, 0),    // #ff5500 (Red-Orange)
            brand_white: Color::Rgb(240, 240, 240),  // #f0f0f0 (Crisp White)
            success: Color::Rgb(97, 255, 202),       // #61ffca (Mint)
            destructive: Color::Rgb(255, 103, 103),  // #ff6767 (Coral Red)
            warning: Color::Rgb(255, 202, 133),      // #ffca85 (Warm Amber)
            highlight: Color::Rgb(246, 148, 255),    // #f694ff (Pink)
            info: Color::Rgb(130, 226, 255),         // #82e2ff (Cyan)
            text_primary: Color::Rgb(237, 236, 238), // #edecee (Bright)
            muted: Color::Rgb(138, 138, 147),        // #8a8a93 (Slate)
            border: Color::Rgb(61, 55, 94),          // #3d375e (Border)
        }
    }

    /// Official OpenZ Aura Soft Dark Theme
    pub fn aura_soft_dark() -> Self {
        Self {
            bg_primary: Color::Rgb(18, 16, 22),  // #121016
            bg_elevated: Color::Rgb(28, 26, 36), // #1c1a24
            bg_input: Color::Rgb(36, 33, 49),    // #242131
            brand_accent: Color::Rgb(255, 85, 0),
            brand_white: Color::Rgb(240, 240, 240),
            success: Color::Rgb(97, 255, 202),
            destructive: Color::Rgb(255, 103, 103),
            warning: Color::Rgb(255, 202, 133),
            highlight: Color::Rgb(246, 148, 255),
            info: Color::Rgb(130, 226, 255),
            text_primary: Color::Rgb(237, 236, 238),
            muted: Color::Rgb(110, 110, 125),
            border: Color::Rgb(50, 45, 75),
        }
    }

    /// Fallback 256-color palette
    pub fn ansi_256() -> Self {
        Self {
            bg_primary: Color::Indexed(234),
            bg_elevated: Color::Indexed(235),
            bg_input: Color::Indexed(236),
            brand_accent: Color::Indexed(202), // Vibrant Orange
            brand_white: Color::Indexed(255),  // Pure White
            success: Color::Indexed(84),       // Mint
            destructive: Color::Indexed(203),  // Coral Red
            warning: Color::Indexed(215),      // Warm Orange
            highlight: Color::Indexed(213),    // Pink
            info: Color::Indexed(117),         // Cyan
            text_primary: Color::White,
            muted: Color::Indexed(244),
            border: Color::Indexed(239),
        }
    }

    pub fn default_theme() -> Self {
        Self::aura_dark()
    }
}

// ── Semantic Style Helpers ──────────────────────────────────────────────────

pub fn user_prefix_style() -> Style {
    Style::default().fg(RED_ORANGE).add_modifier(Modifier::BOLD)
}

pub fn user_text_style() -> Style {
    Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::BOLD)
}

pub fn assistant_text_style() -> Style {
    Style::default().fg(FOREGROUND)
}

pub fn thinking_bullet_style() -> Style {
    Style::default().fg(RED_ORANGE)
}

pub fn thinking_time_style() -> Style {
    Style::default().fg(AURA_GOLD).add_modifier(Modifier::BOLD)
}

pub fn reasoning_style() -> Style {
    Style::default().fg(AURA_SLATE)
}

pub fn tool_bullet_style() -> Style {
    Style::default().fg(RED_ORANGE)
}

pub fn tool_name_style() -> Style {
    Style::default()
        .fg(LIGHT_WHITE)
        .add_modifier(Modifier::BOLD)
}

pub fn tool_details_style() -> Style {
    Style::default().fg(AURA_GOLD)
}

pub fn success_style() -> Style {
    Style::default().fg(AURA_GREEN)
}

pub fn error_style() -> Style {
    Style::default().fg(AURA_ROSE)
}

pub fn divider_style() -> Style {
    Style::default().fg(Color::Rgb(61, 55, 94))
}

pub fn input_prefix_style() -> Style {
    Style::default().fg(RED_ORANGE).add_modifier(Modifier::BOLD)
}

pub fn placeholder_style() -> Style {
    Style::default().fg(AURA_SLATE)
}

pub fn status_bar_style() -> Style {
    Style::default().fg(AURA_SLATE)
}

pub fn status_accent_style() -> Style {
    Style::default().fg(AURA_BLUE)
}

pub fn heading_style() -> Style {
    Style::default()
        .fg(HEADING_BLUE)
        .add_modifier(Modifier::BOLD)
}

pub fn code_style() -> Style {
    Style::default().fg(AURA_GREEN)
}

pub fn bold_style() -> Style {
    Style::default().fg(FOREGROUND).add_modifier(Modifier::BOLD)
}

pub fn list_marker_style() -> Style {
    Style::default().fg(RED_ORANGE)
}

pub const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
