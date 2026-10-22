use super::app::{ModalState, RatatuiApp};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};

fn build_selectable_list_item<'a>(
    label: String,
    is_selected: bool,
    theme: &super::theme::Theme,
) -> ListItem<'a> {
    let (prefix, style) = match is_selected {
        true => (
            " › ",
            Style::default()
                .fg(theme.bg_primary)
                .bg(theme.brand_accent)
                .add_modifier(Modifier::BOLD),
        ),
        false => ("   ", Style::default().fg(theme.text_primary)),
    };
    ListItem::new(format!("{}{}", prefix, label)).style(style)
}

pub(crate) fn render_modal_overlay(f: &mut Frame, app: &RatatuiApp, area: Rect) {
    let theme = &app.theme;

    match &app.modal {
        ModalState::None => {}
        ModalState::ExitConfirm { selected_yes } => {
            render_exit_confirm(f, area, theme, *selected_yes);
        }
        ModalState::CommandCatalog {
            filtered_indices,
            selected_index,
            filter,
        } => {
            render_command_catalog(f, area, theme, filtered_indices, *selected_index, filter);
        }
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
                    build_selectable_list_item(display.clone(), i == *selected_idx, theme)
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

            match *loading {
                true => {
                    let loading_p =
                        Paragraph::new(format!("Fetching live models from {}...", provider_display))
                            .style(Style::default().fg(theme.warning))
                            .alignment(Alignment::Center);
                    f.render_widget(loading_p, inner_area);
                }
                false => {
                    let chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([
                            Constraint::Length(3), // Search box
                            Constraint::Min(5),    // Model list
                            Constraint::Length(1), // Help hints
                        ])
                        .split(inner_area);

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

                    let items: Vec<ListItem> = filtered_indices
                        .iter()
                        .enumerate()
                        .map(|(visual_idx, &real_idx)| {
                            build_selectable_list_item(
                                models[real_idx].clone(),
                                visual_idx == *selected_idx,
                                theme,
                            )
                        })
                        .collect();

                    let list = List::new(items).block(Block::default().borders(Borders::NONE));
                    f.render_widget(list, chunks[1]);

                    let hints = Paragraph::new(
                        " ↑/↓ Navigate · enter Select · type to Filter · esc Cancel",
                    )
                    .style(Style::default().fg(theme.muted))
                    .alignment(Alignment::Center);
                    f.render_widget(hints, chunks[2]);
                }
            }
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

            f.render_widget(Paragraph::new(help_text), inner);
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
                    build_selectable_list_item(
                        format!("{:<25} {:<20} ({})", title, key, time),
                        i == *selected_idx,
                        theme,
                    )
                })
                .collect();

            let list = List::new(items).block(block);
            f.render_widget(list, popup_area);
        }
        ModalState::SecurityApproval {
            tool_name,
            description,
            options,
            selected_idx,
            ..
        } => {
            let width = 68.min(area.width.saturating_sub(4)).max(48);
            let height = 12.min(area.height.saturating_sub(2)).max(10);
            let popup_area = centered_rect_exact(width, height, area);
            f.render_widget(Clear, popup_area);

            let block = Block::default()
                .title(Line::from(vec![Span::styled(
                    " 🛡 Tool Permission ",
                    Style::default()
                        .fg(theme.brand_accent)
                        .add_modifier(Modifier::BOLD),
                )]))
                .title_alignment(Alignment::Left)
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(theme.brand_accent))
                .style(Style::default().bg(theme.bg_elevated));

            let inner = block.inner(popup_area);
            f.render_widget(block, popup_area);

            let max_text_width = (inner.width as usize).saturating_sub(10);
            let reason_lines = format_security_reason_lines(description, max_text_width, theme);
            let reason_height = (reason_lines.len() as u16).max(1);

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1),                   // Tool line
                    Constraint::Length(reason_height),       // Reason lines (1 or 2 lines)
                    Constraint::Length(1),                   // Divider
                    Constraint::Length(options.len() as u16), // Options
                    Constraint::Min(0),                      // Spacer
                    Constraint::Length(1),                   // Hint line
                ])
                .split(inner);

            let tool_line = Line::from(vec![
                Span::styled(" Tool: ", Style::default().fg(theme.muted)),
                Span::styled(
                    tool_name.as_str(),
                    Style::default()
                        .fg(theme.brand_white)
                        .add_modifier(Modifier::BOLD),
                ),
            ]);
            f.render_widget(Paragraph::new(tool_line), chunks[0]);
            f.render_widget(Paragraph::new(reason_lines), chunks[1]);

            let div_line = Line::from(vec![Span::styled(
                "─".repeat(inner.width as usize),
                Style::default().fg(theme.border),
            )]);
            f.render_widget(Paragraph::new(div_line), chunks[2]);

            let items: Vec<ListItem> = options
                .iter()
                .enumerate()
                .map(|(i, opt)| {
                    let is_selected = i == *selected_idx;
                    let (prefix, style) = match (is_selected, i == 2) {
                        (true, true) => (
                            " › ",
                            Style::default()
                                .fg(theme.destructive)
                                .bg(theme.bg_input)
                                .add_modifier(Modifier::BOLD),
                        ),
                        (true, false) => (
                            " › ",
                            Style::default()
                                .fg(theme.brand_accent)
                                .bg(theme.bg_input)
                                .add_modifier(Modifier::BOLD),
                        ),
                        (false, _) => ("   ", Style::default().fg(theme.text_primary)),
                    };
                    ListItem::new(format!("{}{}", prefix, opt)).style(style)
                })
                .collect();
            f.render_widget(List::new(items), chunks[3]);

            let hint = Line::from(vec![
                Span::styled("↑/↓ Navigate · ", Style::default().fg(theme.muted)),
                Span::styled("Enter Confirm · ", Style::default().fg(theme.brand_accent)),
                Span::styled("Esc Deny", Style::default().fg(theme.destructive)),
            ]);
            f.render_widget(Paragraph::new(hint).alignment(Alignment::Center), chunks[5]);
        }
    }
}

pub fn render_exit_confirm(
    f: &mut Frame,
    area: Rect,
    theme: &super::theme::Theme,
    selected_yes: bool,
) {
    let popup_area = centered_rect_exact(54, 7, area);
    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(Line::from(vec![Span::styled(
            " ⏻ Exit OpenZ ",
            Style::default()
                .fg(theme.brand_accent)
                .add_modifier(Modifier::BOLD),
        )]))
        .title_alignment(Alignment::Left)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.brand_accent))
        .style(Style::default().bg(theme.bg_elevated));

    let inner_area = block.inner(popup_area);
    f.render_widget(block, popup_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(inner_area);

    let question_p = Paragraph::new(Line::from(vec![Span::styled(
        "Are you sure you want to quit OpenZ?",
        Style::default()
            .fg(theme.text_primary)
            .add_modifier(Modifier::BOLD),
    )]))
    .alignment(Alignment::Center);
    f.render_widget(question_p, chunks[1]);

    let (yep_spans, nope_spans) = match selected_yes {
        true => {
            let yep_style = Style::default()
                .bg(theme.destructive)
                .fg(theme.bg_primary)
                .add_modifier(Modifier::BOLD);
            let nope_style = Style::default().bg(theme.bg_primary).fg(theme.text_primary);
            (
                vec![Span::styled("  ✖ Yep, Quit (Y)  ", yep_style)],
                vec![
                    Span::styled("  ✔ Stay in Session (", nope_style),
                    Span::styled("N", nope_style.add_modifier(Modifier::UNDERLINED)),
                    Span::styled(")  ", nope_style),
                ],
            )
        }
        false => {
            let yep_style = Style::default().bg(theme.bg_primary).fg(theme.text_primary);
            let nope_style = Style::default()
                .bg(theme.brand_accent)
                .fg(theme.bg_primary)
                .add_modifier(Modifier::BOLD);
            (
                vec![
                    Span::styled("  ✖ ", yep_style),
                    Span::styled("Y", yep_style.add_modifier(Modifier::UNDERLINED)),
                    Span::styled("ep, Quit  ", yep_style),
                ],
                vec![Span::styled("  ✔ Stay in Session (N)  ", nope_style)],
            )
        }
    };

    let mut buttons_line = Vec::new();
    buttons_line.extend(yep_spans);
    buttons_line.push(Span::raw("   "));
    buttons_line.extend(nope_spans);

    f.render_widget(
        Paragraph::new(Line::from(buttons_line)).alignment(Alignment::Center),
        chunks[3],
    );
}

pub fn render_command_catalog(
    f: &mut Frame,
    area: Rect,
    theme: &super::theme::Theme,
    filtered_indices: &[usize],
    selected_index: usize,
    filter: &str,
) {
    let popup_area = centered_rect(78, 72, area);
    f.render_widget(Clear, popup_area);

    let root_block = Block::default()
        .title(" 🧭 OpenZ Slash Commands & Capabilities Catalog ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.brand_accent))
        .style(Style::default().bg(theme.bg_elevated));

    let inner_area = root_block.inner(popup_area);
    f.render_widget(root_block, popup_area);

    let v_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Search bar
            Constraint::Min(8),    // Split view
        ])
        .split(inner_area);

    let active_display_index = match filtered_indices.is_empty() {
        true => 0,
        false => selected_index + 1,
    };
    let count_badge = format!(" [{}/{}] ", active_display_index, filtered_indices.len());

    let search_block = Block::default()
        .title(" 🔍 Filter Commands ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.brand_accent))
        .style(Style::default().bg(theme.bg_elevated));

    let search_p = Paragraph::new(Line::from(vec![
        Span::styled(
            "❯ ",
            Style::default()
                .fg(theme.brand_accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            filter,
            Style::default()
                .fg(theme.text_primary)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("█", Style::default().fg(theme.brand_accent)),
        Span::styled(
            format!(
                "{:>width$}",
                count_badge,
                width = (v_chunks[0].width as usize).saturating_sub(filter.chars().count() + 8)
            ),
            Style::default().fg(theme.muted),
        ),
    ]))
    .block(search_block);
    f.render_widget(search_p, v_chunks[0]);

    let h_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .split(v_chunks[1]);

    let list_block = Block::default()
        .title(" Commands ")
        .borders(Borders::RIGHT)
        .border_style(Style::default().fg(theme.border));

    let mut list_lines = Vec::new();
    let max_visible = (h_chunks[0].height as usize).saturating_sub(2).max(1);
    let scroll_offset = compute_scroll_offset(selected_index, max_visible);

    for (idx_rel, &item_idx) in filtered_indices
        .iter()
        .skip(scroll_offset)
        .take(max_visible)
        .enumerate()
    {
        let real_idx = scroll_offset + idx_rel;
        let item = &super::app::PALETTE_COMMANDS[item_idx];
        let is_selected = real_idx == selected_index;

        let (cursor, name_style) = match is_selected {
            true => (
                "▶ ",
                Style::default()
                    .fg(theme.brand_accent)
                    .add_modifier(Modifier::BOLD),
            ),
            false => ("  ", Style::default().fg(theme.text_primary)),
        };

        let shortcut_style = Style::default().fg(theme.warning);
        let shortcut_str = item
            .shortcut
            .map(|s| format!(" [{}]", s))
            .unwrap_or_default();

        list_lines.push(Line::from(vec![
            Span::styled(
                cursor,
                Style::default()
                    .fg(theme.brand_accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("{:<15}", item.slash_name), name_style),
            Span::styled(shortcut_str, shortcut_style),
        ]));
    }

    match filtered_indices.is_empty() {
        true => {
            list_lines.push(Line::from(Span::styled(
                "  No matching commands found",
                Style::default().fg(theme.muted),
            )));
        }
        false => {}
    }

    f.render_widget(Paragraph::new(list_lines).block(list_block), h_chunks[0]);

    // Right Details Pane
    let mut detail_lines = Vec::new();
    match filtered_indices.get(selected_index) {
        Some(&selected_item_idx) => {
            let cmd = &super::app::PALETTE_COMMANDS[selected_item_idx];

            detail_lines.push(Line::from(vec![
                Span::styled(
                    cmd.slash_name,
                    Style::default()
                        .fg(theme.brand_accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  •  {} ({})", cmd.title, cmd.category.label()),
                    Style::default().fg(theme.muted),
                ),
            ]));
            detail_lines.push(Line::from(""));

            match cmd.shortcut {
                Some(shortcut) => {
                    detail_lines.push(Line::from(vec![
                        Span::styled(
                            "⚡ Shortcut: ",
                            Style::default()
                                .fg(theme.warning)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(shortcut, Style::default().fg(theme.text_primary)),
                    ]));
                    detail_lines.push(Line::from(""));
                }
                None => {}
            }

            detail_lines.push(Line::from(vec![Span::styled(
                "📖 Description:",
                Style::default()
                    .fg(theme.success)
                    .add_modifier(Modifier::BOLD),
            )]));
            detail_lines.push(Line::from(vec![Span::styled(
                format!("  {}", cmd.description),
                Style::default().fg(theme.text_primary),
            )]));
            detail_lines.push(Line::from(""));

            detail_lines.push(Line::from(vec![Span::styled(
                "💡 Usage Example:",
                Style::default()
                    .fg(theme.brand_accent)
                    .add_modifier(Modifier::BOLD),
            )]));
            detail_lines.push(Line::from(vec![Span::styled(
                format!("  {}", cmd.example),
                Style::default()
                    .fg(theme.muted)
                    .add_modifier(Modifier::ITALIC),
            )]));
            detail_lines.push(Line::from(""));

            detail_lines.push(Line::from(vec![Span::styled(
                "🤖 Autonomous Intent Routing:",
                Style::default()
                    .fg(theme.brand_accent)
                    .add_modifier(Modifier::BOLD),
            )]));
            detail_lines.push(Line::from(vec![Span::styled(
                "  You can type natural language in the input prompt. OpenZ's",
                Style::default().fg(theme.muted),
            )]));
            detail_lines.push(Line::from(vec![Span::styled(
                "  intent router will recognize and trigger this workflow automatically.",
                Style::default().fg(theme.muted),
            )]));
        }
        None => {}
    }

    f.render_widget(Paragraph::new(detail_lines), h_chunks[1]);
}

/// Helper function to create a centered Rect for modals using percentage constraints
pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
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

/// Computes a centered sub-rectangle using exact width and height in terminal cells.
pub fn centered_rect_exact(width: u16, height: u16, r: Rect) -> Rect {
    let w = width.min(r.width.saturating_sub(2));
    let h = height.min(r.height.saturating_sub(2));
    let x = r.x + (r.width.saturating_sub(w)) / 2;
    let y = r.y + (r.height.saturating_sub(h)) / 2;
    Rect {
        x,
        y,
        width: w,
        height: h,
    }
}

/// Computes the scroll offset for a list widget so that the selected item remains visible.
pub fn compute_scroll_offset(selected_index: usize, max_visible: usize) -> usize {
    match max_visible {
        0 => 0,
        _ => match selected_index < max_visible {
            true => 0,
            false => selected_index.saturating_sub(max_visible - 1),
        },
    }
}

/// Formats the security approval reason into up to 2 wrapped lines.
pub fn format_security_reason_lines(
    description: &str,
    max_text_width: usize,
    theme: &super::theme::Theme,
) -> Vec<Line<'static>> {
    let raw_desc = description.trim();
    let concise_desc = match raw_desc.find(", arguments:") {
        Some(idx) => &raw_desc[..idx],
        None => raw_desc,
    };
    let concise_desc = match concise_desc.strip_prefix("resource_policy_reason:") {
        Some(stripped) => stripped.trim(),
        None => concise_desc,
    };
    let clean_desc = concise_desc.trim().trim_matches('"').trim();
    let flat_desc: String = clean_desc
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" ");

    let safe_width = max_text_width.max(10);
    let wrapped = crate::agent::style::wrap_line(&flat_desc, safe_width);

    match wrapped.as_slice() {
        [] => vec![Line::from(vec![
            Span::styled(" Reason: ", Style::default().fg(theme.muted)),
            Span::styled(
                "Sensitive operation requires approval",
                Style::default().fg(theme.warning),
            ),
        ])],
        [single] => vec![Line::from(vec![
            Span::styled(" Reason: ", Style::default().fg(theme.muted)),
            Span::styled(single.clone(), Style::default().fg(theme.warning)),
        ])],
        [first, second] => vec![
            Line::from(vec![
                Span::styled(" Reason: ", Style::default().fg(theme.muted)),
                Span::styled(first.clone(), Style::default().fg(theme.warning)),
            ]),
            Line::from(vec![
                Span::styled("         ", Style::default().fg(theme.muted)),
                Span::styled(second.clone(), Style::default().fg(theme.warning)),
            ]),
        ],
        [first, second, ..] => {
            let truncated_l2 = match second.chars().count() + 4 > safe_width {
                true => {
                    let take_count = safe_width.saturating_sub(4);
                    second
                        .chars()
                        .take(take_count)
                        .collect::<String>()
                        .trim_end()
                        .to_string()
                }
                false => second.clone(),
            };
            vec![
                Line::from(vec![
                    Span::styled(" Reason: ", Style::default().fg(theme.muted)),
                    Span::styled(first.clone(), Style::default().fg(theme.warning)),
                ]),
                Line::from(vec![
                    Span::styled("         ", Style::default().fg(theme.muted)),
                    Span::styled(
                        format!("{}....", truncated_l2),
                        Style::default().fg(theme.warning),
                    ),
                ]),
            ]
        }
    }
}

#[cfg(test)]
#[path = "modals_tests.rs"]
mod tests;
