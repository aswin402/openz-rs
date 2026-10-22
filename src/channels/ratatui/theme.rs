//! OpenZ Ratatui Theme — Aura Dark + Red-Orange Brand Palette
//! Ported and enhanced with Dalton Menezes' official Aura Theme specifications

use ratatui::style::Color;

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

    pub fn default_theme() -> Self {
        Self::aura_dark()
    }
}

pub const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
