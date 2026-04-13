use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::App;

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),  // GPU info
            Constraint::Length(5),  // Memory info
            Constraint::Min(1),    // Loaded models
        ])
        .split(area);

    draw_gpu_info(f, app, chunks[0]);
    draw_memory_info(f, app, chunks[1]);
    draw_loaded_models(f, app, chunks[2]);
}

fn draw_gpu_info(f: &mut Frame, app: &App, area: Rect) {
    let devices = app.manager.devices();
    let summary = zonky_core::gpu::device_summary();

    let mut lines = vec![Line::from(vec![
        Span::styled("Compute: ", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(&summary),
    ])];

    for device in devices {
        if device.is_gpu() {
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(
                    device.device_name(),
                    Style::default().fg(Color::Cyan),
                ),
                Span::raw(format!("  ({}B free)", bytesize::ByteSize(device.vram_free()))),
            ]));
        }
    }

    let block = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(" GPU "));

    f.render_widget(block, area);
}

fn draw_memory_info(f: &mut Frame, _app: &App, area: Rect) {
    // System RAM info
    let lines = vec![
        Line::from(vec![
            Span::styled("System RAM: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("(use 'free -h' for details)"),
        ]),
    ];

    let block = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(" Memory "));

    f.render_widget(block, area);
}

fn draw_loaded_models(f: &mut Frame, app: &App, area: Rect) {
    let lines: Vec<Line> = if app.local_models.is_empty() {
        vec![Line::from(Span::styled(
            "No models loaded",
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        app.local_models
            .iter()
            .map(|m| {
                Line::from(vec![
                    Span::styled("● ", Style::default().fg(Color::Green)),
                    Span::raw(m.as_str()),
                ])
            })
            .collect()
    };

    let block = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Loaded Models "),
        );

    f.render_widget(block, area);
}
