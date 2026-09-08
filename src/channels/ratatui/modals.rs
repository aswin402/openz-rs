use super::app::{ModalState, RatatuiApp};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};

pub(crate) fn render_modal_overlay(f: &mut Frame, app: &RatatuiApp, area: Rect) {
    let theme = &app.theme;

    match &app.modal {
        ModalState::None => {}
        ModalState::ProviderSelect {
            providers,
            selected_idx,
        } => {
            let popup_area = centered_rect(55, 50, area);
            f.render_widget(Clear, popup_area);

            let block = Block::default()
                .title(" Select LLM Provider ")
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.brand_accent))
                .style(Style::default().bg(theme.bg_elevated));

            let items: Vec<ListItem> = providers
                .iter()
                .enumerate()
                .map(|(i, (_name, display))| {
                    let is_selected = i == *selected_idx;
                    let prefix = if is_selected { " › " } else { "   " };
                    let style = if is_selected {
                        Style::default()
                            .fg(theme.bg_primary)
                            .bg(theme.brand_accent)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(theme.text_primary)
                    };
                    ListItem::new(format!("{}{}", prefix, display)).style(style)
                })
                .collect();

            let list = List::new(items).block(block);
            f.render_widget(list, popup_area);
        }
        ModalState::ModelSelect {
            provider_display,
            models,
            filtered_indices,
            selected_idx,
            filter,
            loading,
            ..
        } => {
            let popup_area = centered_rect(75, 70, area);
            f.render_widget(Clear, popup_area);

            let outer_block = Block::default()
                .title(format!(" Select Model ({}) ", provider_display))
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.brand_accent))
                .style(Style::default().bg(theme.bg_elevated));

            let inner_area = outer_block.inner(popup_area);
            f.render_widget(outer_block, popup_area);

            if *loading {
                let loading_p =
                    Paragraph::new(format!("Fetching live models from {}...", provider_display))
                        .style(Style::default().fg(theme.warning))
                        .alignment(Alignment::Center);
                f.render_widget(loading_p, inner_area);
                return;
            }

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3), // Search box
                    Constraint::Min(5),    // Model list
                    Constraint::Length(1), // Help hints
                ])
                .split(inner_area);

            // Search filter box
            let search_text = format!(" Search: {}█", filter);
            let search_box = Paragraph::new(search_text)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(theme.border))
                        .title(" Filter Models "),
                )
                .style(Style::default().fg(theme.text_primary));
            f.render_widget(search_box, chunks[0]);

            // Model list items
            let items: Vec<ListItem> = filtered_indices
                .iter()
                .enumerate()
                .map(|(visual_idx, &real_idx)| {
                    let m = &models[real_idx];
                    let is_selected = visual_idx == *selected_idx;
                    let prefix = if is_selected { " › " } else { "   " };

                    let style = if is_selected {
                        Style::default()
                            .fg(theme.bg_primary)
                            .bg(theme.brand_accent)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(theme.text_primary)
                    };
                    ListItem::new(format!("{}{}", prefix, m)).style(style)
                })
                .collect();

            let list = List::new(items).block(Block::default().borders(Borders::NONE));
            f.render_widget(list, chunks[1]);

            let hints =
                Paragraph::new(" ↑/↓ Navigate · enter Select · type to Filter · esc Cancel")
                    .style(Style::default().fg(theme.muted))
                    .alignment(Alignment::Center);
            f.render_widget(hints, chunks[2]);
        }
        ModalState::Help => {
            let popup_area = centered_rect(65, 60, area);
            f.render_widget(Clear, popup_area);

            let block = Block::default()
                .title(" OpenZ Help & Commands ")
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.brand_accent))
                .style(Style::default().bg(theme.bg_elevated));

            let inner = block.inner(popup_area);
            f.render_widget(block, popup_area);

            let help_text = vec![
                Line::from(vec![Span::styled(
                    "Slash Commands:",
                    Style::default()
                        .fg(theme.brand_accent)
                        .add_modifier(Modifier::BOLD),
                )]),
                Line::from(vec![
                    Span::styled("  /model       ", Style::default().fg(theme.warning)),
                    Span::styled(
                        "Switch active LLM provider and model",
                        Style::default().fg(theme.text_primary),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("  /clear       ", Style::default().fg(theme.warning)),
                    Span::styled(
                        "Clear current conversation timeline",
                        Style::default().fg(theme.text_primary),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("  /history     ", Style::default().fg(theme.warning)),
                    Span::styled(
                        "Restore or switch previous sessions",
                        Style::default().fg(theme.text_primary),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("  /mcps        ", Style::default().fg(theme.warning)),
                    Span::styled(
                        "List configured and active MCP tools",
                        Style::default().fg(theme.text_primary),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("  /memory      ", Style::default().fg(theme.warning)),
                    Span::styled(
                        "View cognitive knowledge graph & facts",
                        Style::default().fg(theme.text_primary),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("  /skills      ", Style::default().fg(theme.warning)),
                    Span::styled(
                        "View active autonomous skills",
                        Style::default().fg(theme.text_primary),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("  /exit        ", Style::default().fg(theme.warning)),
                    Span::styled(
                        "Quit OpenZ interactive session",
                        Style::default().fg(theme.text_primary),
                    ),
                ]),
                Line::from(String::new()),
                Line::from(vec![Span::styled(
                    "Shortcuts:",
                    Style::default()
                        .fg(theme.brand_accent)
                        .add_modifier(Modifier::BOLD),
                )]),
                Line::from(vec![
                    Span::styled("  Enter        ", Style::default().fg(theme.info)),
                    Span::styled(
                        "Send message / Select highlighted item",
                        Style::default().fg(theme.text_primary),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("  Tab          ", Style::default().fg(theme.info)),
                    Span::styled(
                        "Autocomplete slash command",
                        Style::default().fg(theme.text_primary),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("  ↑ / ↓        ", Style::default().fg(theme.info)),
                    Span::styled(
                        "Navigate commands / prompt history",
                        Style::default().fg(theme.text_primary),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("  PgUp / PgDn  ", Style::default().fg(theme.info)),
                    Span::styled(
                        "Scroll conversation timeline",
                        Style::default().fg(theme.text_primary),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("  Mouse Wheel  ", Style::default().fg(theme.info)),
                    Span::styled(
                        "Scroll conversation up / down smoothly",
                        Style::default().fg(theme.text_primary),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("  Esc / Ctrl+C ", Style::default().fg(theme.destructive)),
                    Span::styled(
                        "Cancel active agent turn / Dismiss popup",
                        Style::default().fg(theme.text_primary),
                    ),
                ]),
            ];

            let p = Paragraph::new(help_text);
            f.render_widget(p, inner);
        }
        ModalState::History {
            sessions,
            selected_idx,
        } => {
            let popup_area = centered_rect(65, 60, area);
            f.render_widget(Clear, popup_area);

            let block = Block::default()
                .title(" Restore Chat Session ")
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.brand_accent))
                .style(Style::default().bg(theme.bg_elevated));

            let items: Vec<ListItem> = sessions
                .iter()
                .enumerate()
                .map(|(i, (key, title, time))| {
                    let is_selected = i == *selected_idx;
                    let prefix = if is_selected { " › " } else { "   " };
                    let style = if is_selected {
                        Style::default()
                            .fg(theme.bg_primary)
                            .bg(theme.brand_accent)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(theme.text_primary)
                    };
                    ListItem::new(format!("{}{:<25} {:<20} ({})", prefix, title, key, time))
                        .style(style)
                })
                .collect();

            let list = List::new(items).block(block);
            f.render_widget(list, popup_area);
        }
    }
}

/// Helper function to create a centered Rect for modals
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_centered_rect_bounds() {
        let parent = Rect::new(0, 0, 100, 100);
        let centered = centered_rect(50, 50, parent);
        assert_eq!(centered.width, 50);
        assert_eq!(centered.height, 50);
        assert_eq!(centered.x, 25);
        assert_eq!(centered.y, 25);
    }
}

