use super::app::RatatuiApp;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

/// Authentic OpenZ ASCII banner lines: "OPEN" (white), "Z" (brand orange).
pub use crate::agent::style::markdown::OPENZ_BANNER_PARTS as BANNER_PARTS;

/// Renders the Zen Welcome Screen when the conversation timeline is empty.
pub fn render_welcome_screen(frame: &mut Frame, app: &RatatuiApp, area: Rect) {
    let theme = &app.theme;

    // 1. Fill entire background
    let bg_block = Block::default()
        .borders(Borders::NONE)
        .style(Style::default().bg(theme.bg_primary));
    frame.render_widget(bg_block, area);

    // Fallback for constrained terminal dimensions
    match area.height < 10 || area.width < 34 {
        true => return,
        false => {}
    }

    let show_full_banner = area.width >= 50 && area.height >= 16;
    let logo_height = match show_full_banner {
        true => 8,
        false => 2,
    };

    // ── 2. Centered Input Dock ──────────────────────────────────────────────
    let input_width = (area.width * 65 / 100)
        .clamp(42, 76)
        .min(area.width.saturating_sub(4));
    let dock_height = super::ui::compute_input_dock_height(app, input_width);

    // Vertical layout hierarchy: Centered Banner/Details -> Centered Input Dock -> Bottom Bar
    let vert_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),              // 0: Top spacer
            Constraint::Length(logo_height), // 1: Brand logo banner + version
            Constraint::Length(1),           // 2: Spacer
            Constraint::Length(dock_height), // 3: Centered Dynamic Input Dock
            Constraint::Min(1),              // 4: Bottom spacer
            Constraint::Length(1),           // 5: Bottom Edge Bar
        ])
        .split(area);

    // ── 1. Brand Logo & Version Lockup ──────────────────────────────────────
    let mut logo_lines = Vec::new();

    match show_full_banner {
        true => {
            for (white_part, orange_part) in BANNER_PARTS {
                logo_lines.push(Line::from(vec![
                    Span::styled(
                        white_part.to_string(),
                        Style::default()
                            .fg(theme.brand_white)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        orange_part.to_string(),
                        Style::default()
                            .fg(theme.brand_accent)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
            }

            logo_lines.push(Line::from(String::new()));
            logo_lines.push(Line::from(vec![Span::styled(
                format!("v{}", env!("CARGO_PKG_VERSION")),
                Style::default().fg(theme.muted),
            )]));
        }
        false => {
            logo_lines.push(Line::from(vec![
                Span::styled(
                    "OpenZ ",
                    Style::default()
                        .fg(theme.brand_accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("v{}", env!("CARGO_PKG_VERSION")),
                    Style::default().fg(theme.muted),
                ),
            ]));
        }
    }

    let logo_para = Paragraph::new(logo_lines).alignment(Alignment::Center);
    frame.render_widget(logo_para, vert_chunks[1]);

    let input_x = vert_chunks[3].x + (vert_chunks[3].width.saturating_sub(input_width)) / 2;
    let centered_input_rect = Rect {
        x: input_x,
        y: vert_chunks[3].y,
        width: input_width,
        height: vert_chunks[3].height,
    };

    super::ui::render_input_dock_with_placeholder(
        frame,
        app,
        centered_input_rect,
        "Ask OpenZ anything or type '/' for commands...",
    );

    // ── 3. Bottom Edge Bar ──────────────────────────────────────────────────
    let mut left_spans = vec![
        Span::raw(" "),
        Span::styled(app.cwd_display.clone(), Style::default().fg(theme.info)),
    ];

    match RatatuiApp::get_git_branch(&app.workspace_root) {
        Some(branch) => {
            left_spans.push(Span::styled(" · ", Style::default().fg(theme.muted)));
            left_spans.push(Span::styled(
                format!("git:{}", branch),
                Style::default().fg(theme.success),
            ));
        }
        None => {}
    }

    let (mcp_loaded, _, _) = crate::tools::mcp::get_mcp_stats();
    match mcp_loaded > 0 {
        true => {
            left_spans.push(Span::styled(" · ", Style::default().fg(theme.muted)));
            left_spans.push(Span::styled(
                format!("mcp:{} active", mcp_loaded),
                Style::default().fg(theme.brand_accent),
            ));
        }
        false => {}
    }

    let provider_model = format!("{} | {}", app.provider, app.model);
    let right_width = (provider_model.len() + 2).max(25) as u16;
    let right_spans = vec![
        Span::styled(
            provider_model,
            Style::default()
                .fg(theme.warning)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
    ];

    let bottom_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(20), Constraint::Length(right_width)])
        .split(vert_chunks[5]);

    frame.render_widget(Paragraph::new(Line::from(left_spans)), bottom_layout[0]);
    frame.render_widget(
        Paragraph::new(Line::from(right_spans)).alignment(Alignment::Right),
        bottom_layout[1],
    );
}
