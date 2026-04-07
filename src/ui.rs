use crate::app::AppState;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

/// Renders the entire TUI. 
/// 
/// Ratatui uses an immediate-mode rendering model. This means the UI is 
/// redrawn entirely every frame. The state (AppState) is managed outside 
/// the render loop, and the rendering function (ui) simply projects 
/// that state into widgets.
pub fn render(f: &mut Frame, app: &AppState) {
    // 1. Create Layout
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),      // Top: Header
            Constraint::Min(0),         // Middle: Body
            Constraint::Length(1),      // Bottom: Footer
        ])
        .split(f.size());

    let body_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25), // Left: Agent List + Sparkline
            Constraint::Percentage(75), // Right: Metrics + Logs
        ])
        .split(main_chunks[1]);

    // 2. Render Components
    render_header(f, app, main_chunks[0]);
    render_left_pane(f, app, body_chunks[0]);
    render_right_pane(f, app, body_chunks[1]);
    render_footer(f, main_chunks[2]);
}

fn render_left_pane(f: &mut Frame, app: &AppState, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),         // Agent List
            Constraint::Length(7),      // Sparkline area
        ])
        .split(area);

    render_mesh_list(f, app, chunks[0]);
    render_activity_sparkline(f, app, chunks[1]);
}

fn render_right_pane(f: &mut Frame, app: &AppState, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),      // Metrics Summary
            Constraint::Min(0),         // Execution Logs
        ])
        .split(area);

    render_metrics_summary(f, app, chunks[0]);
    render_log_focus(f, app, chunks[1]);
}

fn render_header(f: &mut Frame, app: &AppState, area: Rect) {
    let mesh_count = app.agents.len();
    let event_total = app.total_events_received;

    let content = Line::from(vec![
        Span::styled(" VIEW ", Style::default().bg(Color::Blue).fg(Color::White).add_modifier(Modifier::BOLD)),
        Span::raw(" │ "),
        Span::styled(format!("Mesh: {} agents", mesh_count), Style::default().fg(Color::Cyan)),
        Span::raw(" │ "),
        Span::styled(format!("Total Events: {}", event_total), Style::default().fg(Color::Green)),
        Span::raw(" │ "),
        Span::styled("MODE: Simulated", Style::default().fg(Color::Yellow)),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    
    let p = Paragraph::new(content)
        .block(block)
        .alignment(ratatui::layout::Alignment::Center);
    
    f.render_widget(p, area);
}

fn render_footer(f: &mut Frame, area: Rect) {
    let help_text = " [↑/↓] Select Agent │ [q] Quit │ [c] Ctrl+C Exit ";
    let p = Paragraph::new(help_text)
        .style(Style::default().fg(Color::DarkGray))
        .alignment(ratatui::layout::Alignment::Right);
    f.render_widget(p, area);
}

fn render_mesh_list(f: &mut Frame, app: &AppState, area: Rect) {
    let items: Vec<ListItem> = app
        .agents
        .values()
        .enumerate()
        .map(|(i, agent)| {
            let style = if i == app.selected_agent_idx {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let status_color = match agent.status.as_str() {
                "Idle" => Color::Green,
                "Busy" => Color::Red,
                _ => Color::Gray,
            };

            let content = vec![
                Line::from(vec![
                    Span::styled(format!("{:<15}", agent.id), style),
                    Span::styled(format!("{:<10}", agent.role), Style::default().fg(Color::Cyan)),
                    Span::styled(format!("{}", agent.status.as_str()), Style::default().fg(status_color)),
                ]),
                Line::from(vec![
                    Span::styled(if agent.git_locked { " [GIT_LOCK]" } else { "" }, Style::default().fg(Color::Red)),
                ]),
            ];

            ListItem::new(content)
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().title(" [ Mesh List ] ").borders(Borders::ALL))
        .highlight_style(Style::default().bg(Color::DarkGray))
        .highlight_symbol(">> ");

    f.render_widget(list, area);
}

fn render_event_stream(f: &mut Frame, app: &AppState, area: Rect) {
    let items: Vec<ListItem> = app
        .events
        .iter()
        .map(|event| {
            let time = event.timestamp.format("%H:%M:%S").to_string();
            let content = Line::from(vec![
                Span::styled(format!("[{}] ", time), Style::default().fg(Color::Gray)),
                Span::styled(format!("{}: ", event.agent_id), Style::default().fg(Color::Blue)),
                Span::styled(format!("{}", event.kind), Style::default().fg(Color::Magenta)),
                Span::styled(format!(" -> {}", event.payload), Style::default().fg(Color::White)),
            ]);
            ListItem::new(content)
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().title(" [ Event Stream ] ").borders(Borders::ALL));

    f.render_widget(list, area);
}

fn render_log_focus(f: &mut Frame, app: &AppState, area: Rect) {
    let selected_id = app.get_selected_agent_id();
    
    let content = if let Some(ref id) = selected_id {
        let focused_events: Vec<String> = app
            .events
            .iter()
            .filter(|e| &e.agent_id == id)
            .map(|e| format!("[{}] {}: {}", e.timestamp.format("%H:%M:%S"), e.kind, e.payload))
            .collect();

        if focused_events.is_empty() {
            format!("No recent activity recorded for agent '{}'.", id)
        } else {
            focused_events.join("\n")
        }
    } else {
        "No agent selected.".to_string()
    };

    let title = format!(" [ Log Focus: {} ] ", selected_id.unwrap_or_else(|| "N/A".to_string()));

    let p = Paragraph::new(content)
        .block(Block::default().title(title).borders(Borders::ALL))
        .wrap(Wrap { trim: true });

    f.render_widget(p, area);
}
