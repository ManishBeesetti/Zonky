use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

use crate::app::App;

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    draw_local_models(f, app, chunks[0]);
    draw_search_panel(f, app, chunks[1]);
}

fn draw_local_models(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = if app.local_models.is_empty() {
        vec![ListItem::new(Span::styled(
            "No local models. Press / to search HuggingFace.",
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        app.local_models
            .iter()
            .map(|m| {
                let style = if app.selected_model.as_deref() == Some(m) {
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                ListItem::new(Span::styled(m.as_str(), style))
            })
            .collect()
    };

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Local Models "),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );

    f.render_widget(list, area);
}

fn draw_search_panel(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(1)])
        .split(area);

    // Search input
    let search_style = if app.input_mode {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let search_text = if app.search_query.is_empty() && !app.input_mode {
        "Press / to search HuggingFace..."
    } else {
        &app.search_query
    };

    let search = Paragraph::new(search_text)
        .style(search_style)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Search HuggingFace "),
        );

    f.render_widget(search, chunks[0]);

    // Search results
    let items: Vec<ListItem> = if app.search_results.is_empty() {
        vec![ListItem::new(Span::styled(
            "Search results will appear here",
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        app.search_results
            .iter()
            .map(|r| ListItem::new(Span::raw(r.as_str())))
            .collect()
    };

    let results = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Search Results "),
    );

    f.render_widget(results, chunks[1]);
}
