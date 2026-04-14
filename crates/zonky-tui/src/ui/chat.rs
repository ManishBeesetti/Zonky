use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

use crate::app::App;

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(3)])
        .split(area);

    draw_messages(f, app, chunks[0]);
    draw_input(f, app, chunks[1]);
}

fn draw_messages(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = if app.chat_messages.is_empty() {
        vec![ListItem::new(Line::from(vec![
            Span::styled(
                "Start chatting! ",
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                "Press 'i' or Enter to type.",
                Style::default().fg(Color::DarkGray),
            ),
        ]))]
    } else {
        app.chat_messages
            .iter()
            .map(|msg| {
                let (prefix, color) = match msg.role.as_str() {
                    "user" => ("You: ", Color::Green),
                    "assistant" => ("AI: ", Color::Cyan),
                    _ => ("System: ", Color::Yellow),
                };

                ListItem::new(Line::from(vec![
                    Span::styled(prefix, Style::default().fg(color).add_modifier(Modifier::BOLD)),
                    Span::raw(&msg.content),
                ]))
            })
            .collect()
    };

    let model_name = app.selected_model.as_deref().unwrap_or("no model loaded");
    let title = format!(" Chat - {model_name} ");

    let messages = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title));

    f.render_widget(messages, area);
}

fn draw_input(f: &mut Frame, app: &App, area: Rect) {
    let input_style = if app.input_mode {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let input_text = if app.input_buffer.is_empty() && !app.input_mode {
        "Press 'i' to start typing..."
    } else {
        &app.input_buffer
    };

    let input = Paragraph::new(input_text)
        .style(input_style)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Message (Enter to send, Esc to cancel) "),
        );

    f.render_widget(input, area);

    // Show cursor in input mode
    if app.input_mode {
        let x = area.x + app.input_buffer.len() as u16 + 1;
        let y = area.y + 1;
        f.set_cursor_position((x, y));
    }
}
