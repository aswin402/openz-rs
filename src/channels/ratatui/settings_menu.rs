//! Interactive `/settings` modal for the OpenZ Ratatui TUI.
//!
//! Descriptor-driven configuration menu supporting `caveman` mode enable/disable,
//! `caveman` intensity tier selection (`lite`..`symbolic_ultra`), response streaming,
//! reasoning display, security policy, BPF sandboxing, and TUI color themes —
//! without requiring dedicated slash commands.

use super::app::RatatuiApp;
use super::modals::centered_rect_exact;
use super::theme::Theme;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TuiSettingId {
    CavemanMode,
    CavemanLevel,
    Streaming,
    ThoughtDisplay,
    SecurityMode,
    Sandbox,
    Theme,
}

#[derive(Debug, Clone, Copy)]
pub struct TuiSettingDescriptor {
    pub id: TuiSettingId,
    pub label: &'static str,
    pub subtitle: &'static str,
}

pub const TUI_SETTING_DESCRIPTORS: &[TuiSettingDescriptor] = &[
    TuiSettingDescriptor {
        id: TuiSettingId::CavemanMode,
        label: "Caveman Mode",
        subtitle: "Terse ASD-STE100 controlled grammar with Auto-Clarity safety overrides",
    },
    TuiSettingDescriptor {
        id: TuiSettingId::CavemanLevel,
        label: "Caveman Intensity",
        subtitle: "Select caveman compression tier: lite, full, ultra, or symbolic notation",
    },
    TuiSettingDescriptor {
        id: TuiSettingId::Streaming,
        label: "Response Streaming",
        subtitle: "Stream assistant tokens live into the TUI conversation timeline",
    },
    TuiSettingDescriptor {
        id: TuiSettingId::ThoughtDisplay,
        label: "Reasoning Display",
        subtitle: "Control model chain-of-thought visibility (off, compact, full)",
    },
    TuiSettingDescriptor {
        id: TuiSettingId::SecurityMode,
        label: "Security Policy",
        subtitle: "Interactive approval strictness (low, medium, high)",
    },
    TuiSettingDescriptor {
        id: TuiSettingId::Sandbox,
        label: "BPF Sandbox",
        subtitle: "Linux seccomp syscall allowlist for exec_command subprocesses",
    },
    TuiSettingDescriptor {
        id: TuiSettingId::Theme,
        label: "TUI Color Theme",
        subtitle: "Terminal color palette persisted to the TUI surface profile",
    },
];

const THOUGHT_MODES: &[&str] = &["off", "compact", "full"];
const SECURITY_MODES: &[&str] = &["low", "medium", "high"];

fn cycle_slice_option(options: &[&'static str], current: &str, forward: bool) -> &'static str {
    let cur_idx = options
        .iter()
        .position(|item| item.eq_ignore_ascii_case(current.trim()))
        .unwrap_or(0);
    let len = options.len();
    let next_idx = match forward {
        true => (cur_idx + 1) % len,
        false => (cur_idx + len - 1) % len,
    };
    options[next_idx]
}

#[derive(Debug, Clone)]
pub struct TuiSettingsModalState {
    pub selected_idx: usize,
    pub caveman_mode: bool,
    pub caveman_level: opencaveman::CavemanLevel,
    pub streaming: bool,
    pub thought_display: String,
    pub security_mode: String,
    pub enable_sandbox: bool,
    pub theme_name: String,
    pub caveman_stats: opencaveman::CavemanStatsSnapshot,
}

impl TuiSettingsModalState {
    pub fn from_live_session(
        session_manager: &crate::session::SessionManager,
        session_key: &str,
        fallback_config: &crate::config::schema::Config,
        active_theme_name: &str,
    ) -> Self {
        let raw_cfg = crate::config::loader::load_config()
            .unwrap_or_else(|_| fallback_config.clone());
        let mut effective_cfg = raw_cfg.with_ui_surface("tui");
        match session_manager.load(session_key) {
            Ok(session) => {
                crate::agent::agent_loop::apply_session_overrides(
                    &mut effective_cfg,
                    &session.metadata,
                );
            }
            Err(_) => {}
        }
        let defaults = &effective_cfg.agents.defaults;
        let stats = opencaveman::load_stats_ledger(&crate::config::runtime_data_dir());
        Self {
            selected_idx: 0,
            caveman_mode: defaults.caveman_mode,
            caveman_level: opencaveman::CavemanLevel::from_config_str(&defaults.caveman_level),
            streaming: defaults.streaming,
            thought_display: defaults.tui_thought_display.clone(),
            security_mode: defaults.security_mode.clone(),
            enable_sandbox: defaults.enable_sandbox,
            theme_name: active_theme_name.to_string(),
            caveman_stats: stats,
        }
    }

    pub fn move_prev(&mut self) {
        let len = TUI_SETTING_DESCRIPTORS.len();
        self.selected_idx = (self.selected_idx + len - 1) % len;
    }

    pub fn move_next(&mut self) {
        let len = TUI_SETTING_DESCRIPTORS.len();
        self.selected_idx = (self.selected_idx + 1) % len;
    }

    pub fn selected_descriptor(&self) -> &'static TuiSettingDescriptor {
        let idx = self
            .selected_idx
            .min(TUI_SETTING_DESCRIPTORS.len().saturating_sub(1));
        &TUI_SETTING_DESCRIPTORS[idx]
    }

    pub fn step_selected(&mut self, forward: bool) {
        match self.selected_descriptor().id {
            TuiSettingId::CavemanMode => {
                self.caveman_mode = !self.caveman_mode;
            }
            TuiSettingId::CavemanLevel => {
                self.caveman_level = match forward {
                    true => self.caveman_level.cycle_next(),
                    false => self.caveman_level.cycle_prev(),
                };
            }
            TuiSettingId::Streaming => {
                self.streaming = !self.streaming;
            }
            TuiSettingId::ThoughtDisplay => {
                self.thought_display =
                    cycle_slice_option(THOUGHT_MODES, &self.thought_display, forward).to_string();
            }
            TuiSettingId::SecurityMode => {
                self.security_mode =
                    cycle_slice_option(SECURITY_MODES, &self.security_mode, forward).to_string();
            }
            TuiSettingId::Sandbox => {
                self.enable_sandbox = !self.enable_sandbox;
            }
            TuiSettingId::Theme => {
                let themes = Theme::available_themes();
                let cur = themes
                    .iter()
                    .position(|(name, _)| *name == self.theme_name)
                    .unwrap_or(0);
                let len = themes.len().max(1);
                let next = match forward {
                    true => (cur + 1) % len,
                    false => (cur + len - 1) % len,
                };
                if_less_theme_assign(&mut self.theme_name, themes.get(next).map(|(n, _)| *n));
            }
        }
    }

    pub fn formatted_value(&self, id: TuiSettingId) -> String {
        match id {
            TuiSettingId::CavemanMode => match self.caveman_mode {
                true => "ON  (caveman)".to_string(),
                false => "OFF (standard)".to_string(),
            },
            TuiSettingId::CavemanLevel => {
                let desc = self.caveman_level.descriptor();
                format!("◀ {} (-{}%) ▶", desc.id, desc.target_savings_pct)
            }
            TuiSettingId::Streaming => match self.streaming {
                true => "Enabled".to_string(),
                false => "Disabled".to_string(),
            },
            TuiSettingId::ThoughtDisplay => format!("◀ {} ▶", self.thought_display),
            TuiSettingId::SecurityMode => format!("◀ {} ▶", self.security_mode),
            TuiSettingId::Sandbox => match self.enable_sandbox {
                true => "Enabled".to_string(),
                false => "Disabled".to_string(),
            },
            TuiSettingId::Theme => format!("◀ {} ▶", self.theme_name),
        }
    }

    pub async fn persist_and_sync(
        &self,
        app: &mut RatatuiApp,
        session_manager: &crate::session::SessionManager,
        session_key: &str,
    ) {
        app.set_theme(&self.theme_name);
        app.caveman_mode = self.caveman_mode;
        app.caveman_level = self.caveman_level.slug().to_string();
        app.caveman_saved_badge = self.caveman_stats.non_zero_badge();

        match crate::config::loader::load_config() {
            Ok(mut cfg) => {
                let profile = cfg.ui_profile_mut("tui");
                profile.caveman_mode = Some(self.caveman_mode);
                profile.caveman_level = Some(self.caveman_level.slug().to_string());
                profile.streaming = Some(self.streaming);
                profile.tui_thought_display = Some(self.thought_display.clone());
                profile.security_mode = Some(self.security_mode.clone());
                profile.enable_sandbox = Some(self.enable_sandbox);
                profile.theme = Some(self.theme_name.clone());
                let _ = crate::config::loader::save_config(&cfg);
            }
            Err(_) => {}
        }

        let mut overrides = serde_json::Map::new();
        overrides.insert(
            "caveman_mode".to_string(),
            serde_json::Value::Bool(self.caveman_mode),
        );
        overrides.insert(
            "caveman_level".to_string(),
            serde_json::Value::String(self.caveman_level.slug().to_string()),
        );
        overrides.insert(
            "streaming".to_string(),
            serde_json::Value::Bool(self.streaming),
        );
        let _ =
            crate::agent::agent_loop::save_session_overrides(session_manager, session_key, &overrides)
                .await;
    }
}

fn if_less_theme_assign(slot: &mut String, next: Option<&str>) {
    match next {
        Some(name) => *slot = name.to_string(),
        None => {}
    }
}

pub fn render_settings_modal(
    f: &mut Frame,
    area: Rect,
    theme: &Theme,
    state: &TuiSettingsModalState,
) {
    let popup_area = centered_rect_exact(74, 19, area);
    f.render_widget(Clear, popup_area);

    let outer_block = Block::default()
        .title(" ⚙ OpenZ Settings (TUI Profile) ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.brand_accent))
        .style(Style::default().bg(theme.bg_elevated));

    let inner = outer_block.inner(popup_area);
    f.render_widget(outer_block, popup_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Caveman telemetry banner
            Constraint::Min(8),    // Interactive rows
            Constraint::Length(2), // Detail subtitle for selected row
            Constraint::Length(1), // Footer key hints
        ])
        .split(inner);

    let active_desc = state.caveman_level.descriptor();
    let stats_line = match state.caveman_stats.lifetime_turns > 0
        || state.caveman_stats.lifetime_compressed_files > 0
    {
        true => format!(
            "  ⛏ opencaveman · saved {} tokens ({} turns · {} files compressed)",
            state.caveman_stats.compact_lifetime_badge,
            state.caveman_stats.lifetime_turns,
            state.caveman_stats.lifetime_compressed_files
        ),
        false => format!(
            "  ⛏ opencaveman · active tier '{}' (~{}% target savings) · Auto-Clarity enabled",
            active_desc.id, active_desc.target_savings_pct
        ),
    };
    let banner = Paragraph::new(vec![
        Line::from(vec![Span::styled(
            stats_line,
            Style::default()
                .fg(theme.warning)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![Span::styled(
            "─".repeat(inner.width as usize),
            Style::default().fg(theme.border),
        )]),
    ]);
    f.render_widget(banner, chunks[0]);

    let row_width = chunks[1].width as usize;
    let mut row_lines = Vec::with_capacity(TUI_SETTING_DESCRIPTORS.len());
    for (idx, desc) in TUI_SETTING_DESCRIPTORS.iter().enumerate() {
        let is_selected = idx == state.selected_idx;
        let prefix = match is_selected {
            true => " ❯ ",
            false => "   ",
        };
        let left = format!("{}{:<20}", prefix, desc.label);
        let right = state.formatted_value(desc.id);
        let pad_len = row_width.saturating_sub(left.chars().count() + right.chars().count() + 2);
        let padding = " ".repeat(pad_len);

        match is_selected {
            true => {
                let full = format!("{}{}{}  ", left, padding, right);
                let trail = " ".repeat(row_width.saturating_sub(full.chars().count()));
                row_lines.push(Line::from(vec![Span::styled(
                    format!("{}{}", full, trail),
                    Style::default()
                        .bg(theme.brand_accent)
                        .fg(theme.bg_primary)
                        .add_modifier(Modifier::BOLD),
                )]));
            }
            false => {
                let val_color = match (desc.id, state.caveman_mode) {
                    (TuiSettingId::CavemanMode, true) | (TuiSettingId::CavemanLevel, true) => {
                        theme.success
                    }
                    (TuiSettingId::CavemanMode, false) => theme.muted,
                    _ => theme.info,
                };
                row_lines.push(Line::from(vec![
                    Span::styled(left, Style::default().fg(theme.text_primary)),
                    Span::raw(padding),
                    Span::styled(
                        format!("{}  ", right),
                        Style::default().fg(val_color).add_modifier(Modifier::BOLD),
                    ),
                ]));
            }
        }
    }
    f.render_widget(Paragraph::new(row_lines), chunks[1]);

    let detail_text = match state.selected_descriptor().id {
        TuiSettingId::CavemanLevel => format!("  {}: {}", active_desc.title, active_desc.description),
        _ => format!("  {}", state.selected_descriptor().subtitle),
    };
    let detail_block = Paragraph::new(vec![
        Line::from(vec![Span::styled(
            "─".repeat(inner.width as usize),
            Style::default().fg(theme.border),
        )]),
        Line::from(vec![Span::styled(
            detail_text,
            Style::default().fg(theme.muted),
        )]),
    ]);
    f.render_widget(detail_block, chunks[2]);

    let hints = Paragraph::new(
        " ↑/↓ Select · ←/→ or Enter/Space Adjust · Esc Close (auto-saved) ",
    )
    .style(Style::default().fg(theme.muted))
    .alignment(Alignment::Center);
    f.render_widget(hints, chunks[3]);
}
