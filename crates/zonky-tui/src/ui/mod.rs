mod chat;
mod dashboard;
mod models;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Tabs},
    Frame,
};

use crate::app::{App, Tab};

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Tab bar
            Constraint::Min(1),    // Content
            Constraint::Length(3), // Status bar
        ])
        .split(f.area());

    draw_tabs(f, app, chunks[0]);

    match app.active_tab {
        Tab::Models => models::draw(f, app, chunks[1]),
        Tab::Chat => chat::draw(f, app, chunks[1]),
        Tab::Dashboard => dashboard::draw(f, app, chunks[1]),
        Tab::Settings => draw_settings(f, app, chunks[1]),
    }

    draw_status_bar(f, app, chunks[2]);
}

fn draw_tabs(f: &mut Frame, app: &App, area: Rect) {
    let titles: Vec<Line> = Tab::all()
        .iter()
        .map(|t| {
            let style = if *t == app.active_tab {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            Line::from(Span::styled(t.title(), style))
        })
        .collect();

    let tabs = Tabs::new(titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" ⚡ Zonky "),
        )
        .select(app.active_tab.index())
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .divider("│");

    f.render_widget(tabs, area);
}

fn draw_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let mode = if app.input_mode { "INPUT" } else { "NORMAL" };

    let status = Paragraph::new(Line::from(vec![
        Span::styled(
            format!(" [{mode}] "),
            Style::default()
                .fg(if app.input_mode {
                    Color::Yellow
                } else {
                    Color::Green
                })
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(&app.status_message),
        Span::styled(
            " | Tab: switch | q: quit | /: search ",
            Style::default().fg(Color::DarkGray),
        ),
    ]))
    .block(Block::default().borders(Borders::ALL));

    f.render_widget(status, area);
}

fn draw_settings(f: &mut Frame, app: &App, area: Rect) {
    let text = vec![
        Line::from(vec![
            Span::styled("Backend: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(format!("{:?}", app.config.default_backend)),
        ]),
        Line::from(vec![
            Span::styled("Device: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(format!("{:?}", app.config.default_device)),
        ]),
        Line::from(vec![
            Span::styled("Cache: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(app.config.cache_dir.display().to_string()),
        ]),
        Line::from(vec![
            Span::styled("Server: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(format!(
                "{}:{}",
                app.config.server.host, app.config.server.port
            )),
        ]),
        Line::from(vec![
            Span::styled(
                "Auto-evict: ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!("{}", app.config.auto_evict)),
        ]),
        Line::from(vec![
            Span::styled(
                "Max models: ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!("{}", app.config.max_loaded_models)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Config: ~/.config/zonky/config.toml",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let settings = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title(" Settings "));

    f.render_widget(settings, area);
}
