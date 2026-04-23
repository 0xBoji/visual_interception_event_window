use crate::app::{Agent, AppState, Event, ViewMode};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph, Sparkline, Wrap},
    Frame,
};

const MESH_PULSE_SAMPLES: usize = 50;
const BG_APP: Color = Color::Rgb(34, 36, 52);
const BG_PANEL: Color = Color::Rgb(43, 46, 66);
const BG_PANEL_ALT: Color = Color::Rgb(50, 54, 77);
const FG_PRIMARY: Color = Color::Rgb(232, 235, 255);
const FG_MUTED: Color = Color::Rgb(164, 169, 206);
const BORDER_PRIMARY: Color = Color::Rgb(112, 87, 255);
const BORDER_SECONDARY: Color = Color::Rgb(93, 204, 255);
const BORDER_TERTIARY: Color = Color::Rgb(224, 106, 255);
const BORDER_ACTIVITY: Color = Color::Rgb(246, 211, 101);

/// Renders the entire TUI.
///
/// Ratatui uses an immediate-mode rendering model. This means the UI is
/// redrawn entirely every frame. The state (AppState) is managed outside
/// the render loop, and the rendering function (ui) simply projects
/// that state into widgets.
pub fn render(f: &mut Frame, app: &AppState) {
    f.render_widget(
        Block::default().style(Style::default().bg(BG_APP).fg(FG_PRIMARY)),
        f.size(),
    );

    if app.ui.view_mode == ViewMode::Grid {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(1),
            ])
            .split(f.size());

        render_grid_tabs(f, app, chunks[0], chunks[1]);
        render_workspace(f, app, chunks[1]);
        render_footer(f, chunks[2]);
        return;
    }

    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(7),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(f.size());

    render_header(f, app, main_chunks[0]);
    render_overview(f, app, main_chunks[1]);
    render_workspace(f, app, main_chunks[2]);
    render_footer(f, main_chunks[3]);
}

fn render_grid_tabs(f: &mut Frame, app: &AppState, area: Rect, workspace_area: Rect) {
    let page_size = grid_column_count(workspace_area.width) * grid_row_count(workspace_area.height);
    let visible_ids = app.visible_agents_page(page_size);
    let selected_id = app.get_selected_agent_id();
    let page = app.current_grid_page(page_size) + 1;
    let total_pages = app.grid_page_count(page_size);

    let mut spans = vec![Span::styled(
        " VIEW ",
        Style::default()
            .bg(BORDER_PRIMARY)
            .fg(FG_PRIMARY)
            .add_modifier(Modifier::BOLD),
    )];

    for id in visible_ids {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            format!(" {} ", truncate_text(&id, 14)),
            if selected_id.as_deref() == Some(id.as_str()) {
                Style::default()
                    .bg(BORDER_SECONDARY)
                    .fg(BG_APP)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().bg(BG_PANEL_ALT).fg(FG_PRIMARY)
            },
        ));
    }

    spans.push(Span::raw(" "));
    spans.push(Span::styled(
        format!(
            " {} • {}/{} • filter:{} • search:{} ",
            stream_label(app),
            page,
            total_pages,
            app.filter_label(),
            if app.ui.search_query.is_empty() {
                "inactive".to_string()
            } else {
                truncate_text(&app.ui.search_query, 10)
            }
        ),
        Style::default().fg(FG_MUTED),
    ));

    f.render_widget(
        Paragraph::new(Line::from(spans))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(BORDER_PRIMARY))
                    .style(Style::default().bg(BG_PANEL_ALT)),
            )
            .style(Style::default().fg(FG_PRIMARY).bg(BG_PANEL_ALT))
            .alignment(Alignment::Left),
        area,
    );
}

fn render_workspace(f: &mut Frame, app: &AppState, area: Rect) {
    if app.ui.view_mode == ViewMode::Grid {
        render_agent_grid(f, app, area);
        return;
    }

    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(32), Constraint::Percentage(68)])
        .split(area);

    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(8)])
        .split(columns[0]);

    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(11), Constraint::Min(0)])
        .split(columns[1]);

    render_mesh_list(f, app, left[0]);
    render_activity_sparkline(f, app, left[1]);
    render_metrics_summary(f, app, right[0]);
    render_log_focus(f, app, right[1]);
}

fn render_header(f: &mut Frame, app: &AppState, area: Rect) {
    let summary = app.get_agent_status_summary();
    let stream_label = stream_label(app);
    let stream_color = stream_color(app);

    let content = Line::from(vec![
        Span::styled(
            " VIEW ",
            Style::default()
                .bg(BORDER_PRIMARY)
                .fg(FG_PRIMARY)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled("STREAM", Style::default().fg(FG_MUTED)),
        Span::raw(" "),
        Span::styled(
            stream_label,
            Style::default()
                .fg(stream_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" │ "),
        Span::styled(
            format!("ONLINE {}", summary.online),
            Style::default().fg(stream_color_value("online")),
        ),
        Span::raw(" │ "),
        Span::styled(
            format!("BUSY {}", summary.busy),
            Style::default().fg(status_color("busy")),
        ),
        Span::raw(" │ "),
        Span::styled(
            format!("OFFLINE {}", summary.offline),
            Style::default().fg(status_color("offline")),
        ),
        Span::raw(" │ "),
        Span::styled(
            format!("EVENTS {}", app.registry.total_events_received),
            Style::default().fg(BORDER_SECONDARY),
        ),
        Span::raw(" │ "),
        Span::styled(
            match app.ui.view_mode {
                ViewMode::Grid => "GRID",
                ViewMode::Focus => "FOCUS",
            },
            Style::default().fg(FG_MUTED),
        ),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(BORDER_PRIMARY))
        .style(Style::default().bg(BG_PANEL_ALT));

    f.render_widget(
        Paragraph::new(content)
            .style(Style::default().fg(FG_PRIMARY).bg(BG_PANEL_ALT))
            .block(block)
            .alignment(Alignment::Center),
        area,
    );
}

fn render_overview(f: &mut Frame, app: &AppState, area: Rect) {
    let cards = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(24),
            Constraint::Percentage(24),
            Constraint::Percentage(26),
            Constraint::Percentage(26),
        ])
        .split(area);

    render_stat_card(
        f,
        cards[0],
        "Mesh Health",
        BORDER_PRIMARY,
        build_mesh_health_lines(app),
    );
    render_stat_card(
        f,
        cards[1],
        "Event Levels",
        BORDER_TERTIARY,
        build_event_level_lines(app),
    );
    render_stat_card(
        f,
        cards[2],
        "Focus",
        BORDER_SECONDARY,
        build_focus_lines(app),
    );
    render_stat_card(
        f,
        cards[3],
        "Latest Signal",
        stream_color(app),
        build_latest_signal_lines(app),
    );
}

fn render_stat_card(
    f: &mut Frame,
    area: Rect,
    title: &str,
    accent: Color,
    lines: Vec<Line<'static>>,
) {
    let title = Line::from(vec![Span::styled(
        format!(" {title} "),
        Style::default().fg(accent).add_modifier(Modifier::BOLD),
    )]);

    f.render_widget(
        Paragraph::new(Text::from(lines))
            .block(
                Block::default()
                    .title(title)
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(accent))
                    .style(Style::default().bg(BG_PANEL)),
            )
            .style(Style::default().fg(FG_PRIMARY).bg(BG_PANEL))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn build_mesh_health_lines(app: &AppState) -> Vec<Line<'static>> {
    let summary = app.get_agent_status_summary();

    vec![
        metric_line("TOTAL", summary.total.to_string(), FG_PRIMARY),
        metric_line(
            "ONLINE",
            summary.online.to_string(),
            stream_color_value("online"),
        ),
        metric_line("BUSY", summary.busy.to_string(), status_color("busy")),
        metric_line(
            "OFFLINE",
            summary.offline.to_string(),
            status_color("offline"),
        ),
    ]
}

fn build_event_level_lines(app: &AppState) -> Vec<Line<'static>> {
    let summary = app.get_event_level_summary();

    vec![
        metric_line("INFO", summary.info.to_string(), log_level_color("info")),
        metric_line("WARN", summary.warn.to_string(), log_level_color("warn")),
        metric_line("ERROR", summary.error.to_string(), log_level_color("error")),
        metric_line(
            "SUCCESS",
            summary.success.to_string(),
            log_level_color("success"),
        ),
    ]
}

fn build_focus_lines(app: &AppState) -> Vec<Line<'static>> {
    if let Some(agent) = app.get_selected_agent() {
        vec![
            metric_line("AGENT", truncate_text(&agent.id, 16), FG_PRIMARY),
            metric_line("ROLE", truncate_text(&agent.role, 16), BORDER_SECONDARY),
            metric_line(
                "STATUS",
                agent.status.to_uppercase(),
                status_color(&agent.status),
            ),
            metric_line("PROJ", truncate_text(&agent.project, 16), BORDER_TERTIARY),
        ]
    } else {
        vec![
            metric_line("MODE", "OVERVIEW".to_string(), BORDER_SECONDARY),
            metric_line("FILTER", app.filter_label().to_uppercase(), FG_PRIMARY),
            metric_line("FOCUS", "ALL AGENTS".to_string(), FG_PRIMARY),
            Line::from(format!(
                "Search {}",
                if app.ui.search_query.is_empty() {
                    "press / to search".to_string()
                } else {
                    format!("'{}'", truncate_text(&app.ui.search_query, 14))
                }
            )),
        ]
    }
}

fn build_latest_signal_lines(app: &AppState) -> Vec<Line<'static>> {
    if let Some(event) = app.registry.events.front() {
        vec![
            metric_line("STATE", stream_label(app).to_string(), stream_color(app)),
            metric_line(
                "SOURCE",
                format!("[{}]", event.component.to_uppercase()),
                log_level_color(&event.level),
            ),
            metric_line(
                "TIME",
                event.timestamp.format("%H:%M:%S").to_string(),
                FG_MUTED,
            ),
            Line::from(truncate_text(&event.payload, 28)),
        ]
    } else {
        vec![
            metric_line("STATE", stream_label(app).to_string(), stream_color(app)),
            Line::from("Waiting for CAMP signals."),
            Line::from("The dashboard is live and ready."),
            Line::from("As agents appear, this panel updates."),
        ]
    }
}

fn metric_line(label: &str, value: String, value_color: Color) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("{label:<8}"),
            Style::default().fg(FG_MUTED).add_modifier(Modifier::BOLD),
        ),
        Span::styled(value, Style::default().fg(value_color)),
    ])
}

fn render_footer(f: &mut Frame, area: Rect) {
    let help_text = " [Tab] Grid/Focus │ [j/k] Move │ [f] Filter │ [/] Search │ [PgUp/PgDn] Jump │ [Esc] Clear │ [q] Quit ";

    f.render_widget(
        Paragraph::new(help_text)
            .style(Style::default().fg(FG_MUTED).bg(BG_PANEL_ALT))
            .alignment(Alignment::Right),
        area,
    );
}

fn render_activity_sparkline(f: &mut Frame, app: &AppState, area: Rect) {
    let title = if let Some(agent) = app.get_selected_agent() {
        format!(" [ Activity • {} ] ", truncate_text(&agent.id, 14))
    } else {
        " [ Mesh Pulse (50s) ] ".to_string()
    };
    let data = activity_samples(app);

    let sparkline = Sparkline::default()
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(BORDER_ACTIVITY))
                .style(Style::default().bg(BG_PANEL_ALT)),
        )
        .data(&data)
        .style(Style::default().fg(Color::Yellow));

    f.render_widget(sparkline, area);
}

fn activity_samples(app: &AppState) -> Vec<u64> {
    if let Some(agent) = app.get_selected_agent() {
        let (left, right) = agent.activity.as_slices();
        let mut combined = left.to_vec();
        combined.extend_from_slice(right);
        return combined;
    }

    if app.registry.agents.is_empty() {
        return vec![0; MESH_PULSE_SAMPLES];
    }

    let mut combined = vec![0; MESH_PULSE_SAMPLES];
    for agent in app.registry.agents.values() {
        let samples = agent.activity.iter().copied().collect::<Vec<_>>();
        for (idx, value) in samples.into_iter().enumerate().take(MESH_PULSE_SAMPLES) {
            combined[idx] += value;
        }
    }

    combined
}

fn render_metrics_summary(f: &mut Frame, app: &AppState, area: Rect) {
    let title = if let Some(agent) = app.get_selected_agent() {
        format!(" [ Agent Drill-Down • {} ] ", truncate_text(&agent.id, 16))
    } else {
        " [ Agent Drill-Down ] ".to_string()
    };

    let content = if let Some(agent) = app.get_selected_agent() {
        build_agent_details(agent)
    } else {
        Text::from(vec![
            Line::from(vec![
                Span::styled("OVERVIEW MODE", Style::default().fg(BORDER_SECONDARY)),
                Span::raw("  "),
                Span::styled("No agent focused", Style::default().fg(FG_PRIMARY)),
            ]),
            Line::from("Pick an agent with j/k to inspect role, branch, tokens, metadata, and capabilities."),
            Line::from(
                "Use f to cycle filters or / to search by id, project, role, or branch.",
            ),
        ])
    };

    f.render_widget(
        Paragraph::new(content)
            .block(
                Block::default()
                    .title(title)
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(BORDER_SECONDARY))
                    .style(Style::default().bg(BG_PANEL)),
            )
            .style(Style::default().fg(FG_PRIMARY).bg(BG_PANEL))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn build_agent_details(agent: &Agent) -> Text<'static> {
    let capabilities = if agent.capabilities.is_empty() {
        "none declared".to_string()
    } else {
        truncate_text(&agent.capabilities.join(", "), 46)
    };

    let mut lines = vec![
        Line::from(vec![
            Span::styled("STATUS", Style::default().fg(FG_MUTED)),
            Span::raw(" "),
            Span::styled(
                agent.status.to_uppercase(),
                Style::default()
                    .fg(status_color(&agent.status))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("   "),
            Span::styled("LAST SEEN", Style::default().fg(FG_MUTED)),
            Span::raw(" "),
            Span::styled(
                format_relative_time(agent.last_seen),
                Style::default().fg(FG_PRIMARY),
            ),
        ]),
        Line::from(vec![
            Span::styled("ID", Style::default().fg(FG_MUTED)),
            Span::raw(" "),
            Span::styled(
                truncate_text(&agent.id, 18),
                Style::default().fg(BORDER_SECONDARY),
            ),
            Span::raw("   "),
            Span::styled("ROLE", Style::default().fg(FG_MUTED)),
            Span::raw(" "),
            Span::styled(
                truncate_text(&agent.role, 18),
                Style::default().fg(FG_PRIMARY),
            ),
        ]),
        Line::from(vec![
            Span::styled("PROJECT", Style::default().fg(FG_MUTED)),
            Span::raw(" "),
            Span::styled(
                truncate_text(&agent.project, 18),
                Style::default().fg(BORDER_TERTIARY),
            ),
            Span::raw("   "),
            Span::styled("BRANCH", Style::default().fg(FG_MUTED)),
            Span::raw(" "),
            Span::styled(
                truncate_text(&agent.branch, 18),
                Style::default().fg(Color::Rgb(255, 208, 122)),
            ),
        ]),
        Line::from(vec![
            Span::styled("TOKENS", Style::default().fg(FG_MUTED)),
            Span::raw(" "),
            Span::styled(
                format_tokens(agent.tokens),
                Style::default()
                    .fg(stream_color_value("online"))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("   "),
            Span::styled("PORT", Style::default().fg(FG_MUTED)),
            Span::raw(" "),
            Span::styled(agent.port.to_string(), Style::default().fg(FG_PRIMARY)),
            Span::raw("   "),
            Span::styled("ADDRS", Style::default().fg(FG_MUTED)),
            Span::raw(" "),
            Span::styled(
                agent.addresses.len().to_string(),
                Style::default().fg(FG_PRIMARY),
            ),
        ]),
        Line::from(vec![
            Span::styled("CAPS", Style::default().fg(FG_MUTED)),
            Span::raw(" "),
            Span::styled(capabilities, Style::default().fg(BORDER_SECONDARY)),
        ]),
    ];

    let metadata_lines = build_metadata_lines(agent);
    if !metadata_lines.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![Span::styled(
            "METADATA",
            Style::default()
                .fg(BORDER_TERTIARY)
                .add_modifier(Modifier::BOLD),
        )]));
        lines.extend(metadata_lines);
    }

    Text::from(lines)
}

fn format_tokens(tokens: u64) -> String {
    let text = tokens.to_string();
    let chars: Vec<char> = text.chars().rev().collect();
    let mut result = Vec::new();

    for (idx, character) in chars.into_iter().enumerate() {
        if idx > 0 && idx % 3 == 0 {
            result.push(',');
        }
        result.push(character);
    }

    result.into_iter().rev().collect()
}

fn render_mesh_list(f: &mut Frame, app: &AppState, area: Rect) {
    let visible_ids = app.visible_agent_ids();
    let items = if visible_ids.is_empty() {
        vec![ListItem::new(vec![
            Line::from(vec![Span::styled(
                "No agents match the current filter/search.",
                Style::default().fg(FG_MUTED),
            )]),
            Line::from(vec![Span::styled(
                "Press f to cycle filters or Esc to clear search.",
                Style::default().fg(FG_MUTED),
            )]),
        ])]
    } else {
        visible_ids
            .iter()
            .enumerate()
            .filter_map(|(idx, id)| {
                app.registry
                    .agents
                    .get(id)
                    .map(|agent| build_agent_list_item(agent, idx == app.ui.selected_agent_idx))
            })
            .collect::<Vec<_>>()
    };

    let title = format!(
        " [ Mesh Roster • {} • filter:{} • search:{} ] ",
        app.visible_agent_count(),
        app.filter_label(),
        if app.ui.search_query.is_empty() {
            String::new()
        } else {
            truncate_text(&app.ui.search_query, 12)
        }
    );
    let list = List::new(items)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(BORDER_PRIMARY))
                .style(Style::default().bg(BG_PANEL_ALT)),
        )
        .style(Style::default().fg(FG_PRIMARY).bg(BG_PANEL_ALT))
        .highlight_style(
            Style::default()
                .bg(Color::Rgb(79, 70, 229))
                .fg(FG_PRIMARY)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("› ");

    f.render_widget(list, area);
}

fn render_agent_grid(f: &mut Frame, app: &AppState, area: Rect) {
    let columns = grid_column_count(area.width);
    let rows = grid_row_count(area.height);
    let page_size = columns * rows;
    let visible_ids = app.visible_agents_page(page_size);

    if visible_ids.is_empty() {
        f.render_widget(
            Paragraph::new(Text::from(vec![
                Line::from(vec![Span::styled(
                    "No agents match the current grid view.",
                    Style::default().fg(FG_PRIMARY).add_modifier(Modifier::BOLD),
                )]),
                Line::from("Use f to cycle filters, / to search, or Esc to clear."),
            ]))
            .block(
                Block::default()
                    .title(" [ Grid Overview ] ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(BORDER_PRIMARY))
                    .style(Style::default().bg(BG_PANEL)),
            )
            .style(Style::default().fg(FG_PRIMARY).bg(BG_PANEL))
            .alignment(Alignment::Center),
            area,
        );
        return;
    }

    let row_constraints = vec![Constraint::Ratio(1, rows as u32); rows];
    let col_constraints = vec![Constraint::Ratio(1, columns as u32); columns];
    let row_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(row_constraints)
        .split(area);
    let selected_id = app.get_selected_agent_id();

    for (row_index, row_area) in row_chunks.iter().enumerate() {
        let col_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(col_constraints.clone())
            .split(*row_area);

        for (col_index, cell_area) in col_chunks.iter().enumerate() {
            let tile_index = row_index * columns + col_index;
            if let Some(id) = visible_ids.get(tile_index) {
                if let Some(agent) = app.registry.agents.get(id) {
                    render_agent_tile(
                        f,
                        agent,
                        app,
                        *cell_area,
                        selected_id.as_deref() == Some(id.as_str()),
                    );
                }
            }
        }
    }
}

fn render_agent_tile(f: &mut Frame, agent: &Agent, app: &AppState, area: Rect, selected: bool) {
    let border = if selected {
        BORDER_SECONDARY
    } else {
        BORDER_PRIMARY
    };
    let background = if selected { BG_PANEL } else { BG_PANEL_ALT };
    let recent_events = app.get_recent_events(Some(&agent.id), 2);
    let log_lines = if recent_events.is_empty() {
        vec![Line::from(vec![Span::styled(
            "Waiting for signal…",
            Style::default().fg(FG_MUTED),
        )])]
    } else {
        recent_events
            .into_iter()
            .map(|event| {
                Line::from(vec![
                    Span::styled(
                        format!("[{}]", event.component.to_uppercase()),
                        Style::default()
                            .fg(log_level_color(&event.level))
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" "),
                    Span::styled(
                        truncate_text(
                            &event.payload,
                            usize::from(area.width.saturating_sub(10)).max(10),
                        ),
                        Style::default().fg(FG_PRIMARY),
                    ),
                ])
            })
            .collect::<Vec<_>>()
    };

    let lines = vec![
        Line::from(vec![
            Span::styled(
                truncate_text(
                    &agent.id,
                    usize::from(area.width.saturating_sub(14)).max(10),
                ),
                Style::default().fg(FG_PRIMARY).add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            status_pill(&agent.status, status_color(&agent.status)),
        ]),
        Line::from(vec![
            Span::styled(
                truncate_text(&agent.project, 12),
                Style::default().fg(FG_MUTED),
            ),
            Span::raw(" · "),
            Span::styled(
                truncate_text(&agent.role, 10),
                Style::default().fg(FG_MUTED),
            ),
        ]),
        Line::from(vec![
            Span::styled("activity ", Style::default().fg(FG_MUTED)),
            Span::styled(
                trendline(agent, usize::from(area.width.saturating_sub(12)).max(8)),
                Style::default().fg(status_color(&agent.status)),
            ),
        ]),
        Line::from(vec![
            Span::styled("tokens ", Style::default().fg(FG_MUTED)),
            Span::styled(format_tokens(agent.tokens), Style::default().fg(FG_PRIMARY)),
        ]),
        log_lines.first().cloned().unwrap_or_else(Line::default),
        log_lines.get(1).cloned().unwrap_or_else(Line::default),
    ];

    f.render_widget(
        Paragraph::new(Text::from(lines))
            .block(
                Block::default()
                    .title(Line::from(vec![Span::styled(
                        format!(
                            " {} ",
                            if selected {
                                "Selected Agent"
                            } else {
                                "Agent Snapshot"
                            }
                        ),
                        Style::default().fg(border).add_modifier(Modifier::BOLD),
                    )]))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(border))
                    .style(Style::default().bg(background)),
            )
            .style(Style::default().fg(FG_PRIMARY).bg(background))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn build_agent_list_item(agent: &Agent, is_selected: bool) -> ListItem<'static> {
    let emphasis = if is_selected {
        Style::default().fg(FG_PRIMARY).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(FG_PRIMARY)
    };

    let secondary = Style::default().fg(FG_MUTED);
    let status_tint = status_color(&agent.status);

    ListItem::new(vec![
        Line::from(vec![
            Span::styled(truncate_text(&agent.id, 18), emphasis),
            Span::raw("  "),
            status_pill(&agent.status, status_tint),
        ]),
        Line::from(vec![
            Span::styled(truncate_text(&agent.project, 12), secondary),
            Span::raw(" · "),
            Span::styled(truncate_text(&agent.role, 10), secondary),
            Span::raw("  "),
            Span::styled(trendline(agent, 12), Style::default().fg(status_tint)),
        ]),
    ])
}

fn build_metadata_lines(agent: &Agent) -> Vec<Line<'static>> {
    let preferred_keys = ["cwd", "model", "last_file", "last_tool", "messages", "cost"];

    preferred_keys
        .iter()
        .filter_map(|key| {
            agent.metadata.get(*key).map(|value| {
                Line::from(vec![
                    Span::styled(
                        format!("{:<10}", key.to_uppercase()),
                        Style::default().fg(FG_MUTED),
                    ),
                    Span::styled(truncate_text(value, 38), Style::default().fg(FG_PRIMARY)),
                ])
            })
        })
        .collect()
}

fn log_level_color(level: &str) -> Color {
    match level.to_ascii_lowercase().as_str() {
        "error" => Color::Red,
        "warn" => Color::Yellow,
        "success" => Color::Green,
        _ => Color::Cyan,
    }
}

fn build_log_line(event: &Event) -> Line<'static> {
    let level_color = log_level_color(&event.level);

    Line::from(vec![
        Span::styled(
            format!("{} ", event.timestamp.format("%H:%M:%S")),
            Style::default().fg(FG_MUTED),
        ),
        Span::styled(
            format!("[{}]", event.component.to_uppercase()),
            Style::default()
                .fg(level_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(event.payload.clone(), Style::default().fg(level_color)),
    ])
}

fn render_log_focus(f: &mut Frame, app: &AppState, area: Rect) {
    let selected_id = app.get_selected_agent_id();
    let feed_limit = usize::from(area.height.saturating_sub(2));
    let events = app.get_recent_events(selected_id.as_deref(), feed_limit);

    let content = if events.is_empty() {
        if let Some(id) = selected_id.as_ref() {
            Text::from(format!("No recent activity recorded for agent '{id}'."))
        } else {
            Text::from("Waiting for live mesh activity.")
        }
    } else {
        Text::from(events.into_iter().map(build_log_line).collect::<Vec<_>>())
    };

    let scope = selected_id.unwrap_or_else(|| "ALL".to_string());
    let title = format!(" [ Live Feed • {scope} ] ");

    f.render_widget(
        Paragraph::new(content)
            .block(
                Block::default()
                    .title(title)
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(BORDER_TERTIARY))
                    .style(Style::default().bg(BG_PANEL)),
            )
            .style(Style::default().fg(FG_PRIMARY).bg(BG_PANEL))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn stream_label(app: &AppState) -> &'static str {
    if !app.registry.events.is_empty() {
        "LIVE"
    } else if !app.registry.agents.is_empty() {
        "TRACKING"
    } else {
        "AWAITING"
    }
}

fn stream_color(app: &AppState) -> Color {
    match stream_label(app) {
        "LIVE" => stream_color_value("online"),
        "TRACKING" => BORDER_SECONDARY,
        _ => BORDER_ACTIVITY,
    }
}

fn status_color(status: &str) -> Color {
    match status.to_ascii_lowercase().as_str() {
        "busy" => Color::Rgb(255, 184, 76),
        "offline" => Color::Rgb(128, 132, 162),
        _ => stream_color_value("online"),
    }
}

fn stream_color_value(_label: &str) -> Color {
    Color::Rgb(109, 234, 170)
}

fn status_pill(status: &str, color: Color) -> Span<'static> {
    Span::styled(
        format!(" {} ", status.to_uppercase()),
        Style::default()
            .fg(color)
            .bg(BG_APP)
            .add_modifier(Modifier::BOLD),
    )
}

fn trendline(agent: &Agent, width: usize) -> String {
    let values = agent.activity.iter().copied().collect::<Vec<_>>();
    let start = values.len().saturating_sub(width);
    let slice = &values[start..];
    let max = slice.iter().copied().max().unwrap_or(0);
    let blocks = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

    slice
        .iter()
        .map(|value| {
            if max == 0 {
                '·'
            } else {
                let index = ((*value * (blocks.len() as u64 - 1)) / max) as usize;
                blocks[index]
            }
        })
        .collect()
}

fn grid_column_count(width: u16) -> usize {
    if width >= 180 {
        3
    } else if width >= 110 {
        2
    } else {
        1
    }
}

fn grid_row_count(height: u16) -> usize {
    if height >= 36 {
        2
    } else {
        1
    }
}

fn truncate_text(value: &str, max_chars: usize) -> String {
    let chars = value.chars().collect::<Vec<_>>();
    if chars.len() <= max_chars {
        return value.to_string();
    }

    if max_chars <= 1 {
        return "…".to_string();
    }

    chars[..max_chars - 1].iter().collect::<String>() + "…"
}

fn format_relative_time(timestamp: chrono::DateTime<chrono::Local>) -> String {
    let elapsed = chrono::Local::now() - timestamp;

    if elapsed.num_seconds() < 60 {
        format!("{}s ago", elapsed.num_seconds().max(0))
    } else if elapsed.num_minutes() < 60 {
        format!("{}m ago", elapsed.num_minutes())
    } else {
        format!("{}h ago", elapsed.num_hours())
    }
}

#[cfg(test)]
mod tests {
    use super::render_log_focus;
    use crate::app::{Agent, AppState, Event, ViewMode};
    use chrono::Local;
    use ratatui::{backend::TestBackend, style::Color, Terminal};
    use std::collections::{BTreeMap, VecDeque};

    fn test_agent() -> Agent {
        Agent {
            id: "agent-1".to_string(),
            instance_name: "agent-1".to_string(),
            role: "observer".to_string(),
            project: "view".to_string(),
            branch: "main".to_string(),
            status: "Idle".to_string(),
            capabilities: Vec::new(),
            port: 0,
            addresses: Vec::new(),
            metadata: BTreeMap::new(),
            last_seen: Local::now(),
            tokens: 0,
            activity: VecDeque::from(vec![0; 50]),
        }
    }

    fn render_log_focus_row(event: Event) -> (String, ratatui::buffer::Buffer) {
        let mut app = AppState::new();
        let agent = test_agent();
        app.update_agent(agent);
        app.add_event(event);

        let backend = TestBackend::new(60, 5);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| render_log_focus(frame, &app, ratatui::layout::Rect::new(0, 0, 60, 5)))
            .expect("render succeeds");

        let buffer = terminal.backend().buffer().clone();
        let row = (1..59)
            .map(|x| buffer.get(x, 1).symbol())
            .collect::<String>();

        (row, buffer)
    }

    #[test]
    fn render_log_focus_should_render_unpadded_uppercase_component_prefix() {
        let (row, _) = render_log_focus_row(Event {
            timestamp: Local::now(),
            agent_id: "agent-1".to_string(),
            kind: "UPDATED".to_string(),
            component: "tick".to_string(),
            level: "warn".to_string(),
            payload: "Job started".to_string(),
        });

        assert!(
            row.contains("[TICK] Job started"),
            "expected dynamic uppercase prefix without padding, got row: {row:?}"
        );
    }

    #[test]
    fn render_log_focus_should_color_prefix_and_payload_from_level_only() {
        let (row, buffer) = render_log_focus_row(Event {
            timestamp: Local::now(),
            agent_id: "agent-1".to_string(),
            kind: "UPDATED".to_string(),
            component: "wasp".to_string(),
            level: "error".to_string(),
            payload: "Execution failed".to_string(),
        });

        let prefix = "[WASP] Execution failed";
        let start = row
            .find(prefix)
            .expect("rendered log row should contain the prefix and payload");
        let prefix_x = (start + row[..start].chars().count()) as u16 + 1;
        let payload_x = prefix_x + "[WASP] ".chars().count() as u16;

        assert_eq!(buffer.get(prefix_x, 1).fg, Color::Red);
        assert_eq!(buffer.get(payload_x, 1).fg, Color::Red);
    }

    #[test]
    fn render_log_focus_should_show_global_feed_when_no_agent_is_selected() {
        let mut app = AppState::new();
        app.add_event(Event {
            timestamp: Local::now(),
            agent_id: "agent-9".to_string(),
            kind: "UPDATED".to_string(),
            component: "camp".to_string(),
            level: "info".to_string(),
            payload: "Mesh heartbeat".to_string(),
        });

        let backend = TestBackend::new(60, 5);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| render_log_focus(frame, &app, ratatui::layout::Rect::new(0, 0, 60, 5)))
            .expect("render succeeds");

        let buffer = terminal.backend().buffer().clone();
        let row = (1..59)
            .map(|x| buffer.get(x, 1).symbol())
            .collect::<String>();

        assert!(
            row.contains("[CAMP] Mesh heartbeat"),
            "expected global live feed when no agent is selected, got row: {row:?}"
        );
    }

    #[test]
    fn render_should_show_multiple_agents_in_grid_mode() {
        let mut app = AppState::new();
        let mut alpha = test_agent();
        alpha.id = "alpha".to_string();
        alpha.project = "tick".to_string();
        let mut beta = test_agent();
        beta.id = "beta".to_string();
        beta.project = "wasp".to_string();
        app.update_agent(alpha);
        app.update_agent(beta);
        app.ui.view_mode = ViewMode::Grid;

        let backend = TestBackend::new(140, 30);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| super::render(frame, &app))
            .expect("render succeeds");

        let buffer = terminal.backend().buffer().clone();
        let text = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();

        assert!(text.contains("alpha"));
        assert!(text.contains("beta"));
        assert!(text.contains("Selected Agent") || text.contains("Agent Snapshot"));
    }

    #[test]
    fn render_grid_mode_should_show_tabs_instead_of_overview_cards() {
        let mut app = AppState::new();
        let mut alpha = test_agent();
        alpha.id = "workspace-1".to_string();
        let mut beta = test_agent();
        beta.id = "workspace-2".to_string();
        app.update_agent(alpha);
        app.update_agent(beta);
        app.ui.view_mode = ViewMode::Grid;

        let backend = TestBackend::new(160, 32);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| super::render(frame, &app))
            .expect("render succeeds");

        let buffer = terminal.backend().buffer().clone();
        let text = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();

        assert!(text.contains("workspace-1"));
        assert!(text.contains("workspace-2"));
        assert!(!text.contains("Mesh Health"));
    }
}
