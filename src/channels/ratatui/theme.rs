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

    pub fn cyberpunk() -> Self {
        Self {
            bg_primary: Color::Rgb(13, 10, 26),
            bg_elevated: Color::Rgb(24, 18, 43),
            bg_input: Color::Rgb(34, 24, 60),
            brand_accent: Color::Rgb(255, 42, 133),
            brand_white: Color::Rgb(245, 245, 255),
            success: Color::Rgb(0, 255, 179),
            destructive: Color::Rgb(255, 71, 87),
            warning: Color::Rgb(255, 211, 42),
            highlight: Color::Rgb(0, 216, 255),
            info: Color::Rgb(0, 216, 255),
            text_primary: Color::Rgb(240, 238, 255),
            muted: Color::Rgb(144, 134, 176),
            border: Color::Rgb(84, 48, 128),
        }
    }

    pub fn oceanic() -> Self {
        Self {
            bg_primary: Color::Rgb(11, 21, 29),
            bg_elevated: Color::Rgb(18, 33, 45),
            bg_input: Color::Rgb(24, 45, 61),
            brand_accent: Color::Rgb(56, 189, 248),
            brand_white: Color::Rgb(240, 249, 255),
            success: Color::Rgb(52, 211, 153),
            destructive: Color::Rgb(248, 113, 113),
            warning: Color::Rgb(251, 191, 36),
            highlight: Color::Rgb(129, 140, 248),
            info: Color::Rgb(103, 232, 249),
            text_primary: Color::Rgb(226, 232, 240),
            muted: Color::Rgb(148, 163, 184),
            border: Color::Rgb(45, 74, 99),
        }
    }

    pub fn emerald() -> Self {
        Self {
            bg_primary: Color::Rgb(10, 20, 16),
            bg_elevated: Color::Rgb(17, 34, 27),
            bg_input: Color::Rgb(24, 48, 38),
            brand_accent: Color::Rgb(16, 185, 129),
            brand_white: Color::Rgb(236, 253, 245),
            success: Color::Rgb(52, 211, 153),
            destructive: Color::Rgb(248, 113, 113),
            warning: Color::Rgb(250, 204, 21),
            highlight: Color::Rgb(110, 231, 183),
            info: Color::Rgb(45, 212, 191),
            text_primary: Color::Rgb(229, 245, 238),
            muted: Color::Rgb(134, 167, 153),
            border: Color::Rgb(39, 82, 64),
        }
    }

    pub fn light() -> Self {
        Self {
            bg_primary: Color::Rgb(248, 250, 252),
            bg_elevated: Color::Rgb(241, 245, 249),
            bg_input: Color::Rgb(226, 232, 240),
            brand_accent: Color::Rgb(234, 88, 12),
            brand_white: Color::Rgb(15, 23, 42),
            success: Color::Rgb(5, 150, 105),
            destructive: Color::Rgb(220, 38, 38),
            warning: Color::Rgb(217, 119, 6),
            highlight: Color::Rgb(124, 58, 237),
            info: Color::Rgb(2, 132, 199),
            text_primary: Color::Rgb(15, 23, 42),
            muted: Color::Rgb(100, 116, 139),
            border: Color::Rgb(203, 213, 225),
        }
    }

    pub fn available_themes() -> &'static [(&'static str, &'static str)] {
        &[
            ("aura_dark", "Aura Dark + Red-Orange (Default)"),
            ("cyberpunk", "Cyberpunk Neon Magenta & Cyan"),
            ("oceanic", "Oceanic Deep Blue & Sky"),
            ("emerald", "Emerald Matrix Forest"),
            ("light", "Crisp Daylight Slate"),
        ]
    }

    pub fn canonical_name(name: &str) -> Option<&'static str> {
        match name.trim().to_lowercase().as_str() {
            "aura_dark" | "aura" | "dark" | "default" => Some("aura_dark"),
            "cyberpunk" | "neon" => Some("cyberpunk"),
            "oceanic" | "ocean" => Some("oceanic"),
            "emerald" | "matrix" => Some("emerald"),
            "light" | "daylight" => Some("light"),
            _ => None,
        }
    }

    pub fn from_name(name: &str) -> Self {
        match Self::canonical_name(name).unwrap_or("aura_dark") {
            "cyberpunk" => Self::cyberpunk(),
            "oceanic" => Self::oceanic(),
            "emerald" => Self::emerald(),
            "light" => Self::light(),
            _ => Self::aura_dark(),
        }
    }
}

pub const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
