use std::path::PathBuf;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::symbols;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::ui::image::{open_file_in_os, render_image_halfblocks};

#[derive(Debug, Clone)]
pub struct ImageViewerState {
    pub file_path: PathBuf,
    pub caption: String,
    pub width: i32,
    pub height: i32,
    pub file_size: u64,
}

impl ImageViewerState {
    pub fn new(file_path: PathBuf, caption: String, width: i32, height: i32, file_size: u64) -> Self {
        Self {
            file_path,
            caption,
            width,
            height,
            file_size,
        }
    }

    pub fn open_desktop(&self) -> Result<(), String> {
        open_file_in_os(&self.file_path)
    }
}

pub fn render_image_viewer(frame: &mut Frame, state: &ImageViewerState, area: Rect) {
    // 90% width, 90% height centered
    let popup_w = (area.width * 90 / 100).max(30);
    let popup_h = (area.height * 90 / 100).max(15);
    let popup_x = area.x + (area.width.saturating_sub(popup_w)) / 2;
    let popup_y = area.y + (area.height.saturating_sub(popup_h)) / 2;
    let popup_area = Rect::new(popup_x, popup_y, popup_w, popup_h);

    frame.render_widget(Clear, popup_area);

    let title_span = Span::styled(
        format!(" 📷 Photo Viewer ({}x{}) ", state.width, state.height),
        Style::default()
            .fg(Color::Rgb(42, 171, 238))
            .add_modifier(Modifier::BOLD),
    );

    let block = Block::default()
        .borders(Borders::ALL)
        .border_set(symbols::border::ROUNDED)
        .border_style(Style::default().fg(Color::Rgb(42, 171, 238)))
        .title(title_span);

    frame.render_widget(block.clone(), popup_area);

    let inner = block.inner(popup_area);
    if inner.width < 10 || inner.height < 5 {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(4),    // Image area
            Constraint::Length(3), // Caption / Controls area
        ])
        .split(inner);

    // 1. Render half-block image in top chunk
    if let Some(rendered_lines) = render_image_halfblocks(&state.file_path, chunks[0].width, chunks[0].height) {
        let img_p = Paragraph::new(rendered_lines)
            .alignment(Alignment::Center);
        frame.render_widget(img_p, chunks[0]);
    } else {
        let err_p = Paragraph::new(vec![
            Line::default(),
            Line::from(vec![
                Span::styled(
                    "Image could not be rendered in terminal.",
                    Style::default().fg(Color::LightRed),
                ),
            ]),
            Line::from(vec![
                Span::styled(
                    "Press [o] to open the image in your OS default image viewer.",
                    Style::default().fg(Color::DarkGray),
                ),
            ]),
        ])
        .alignment(Alignment::Center);
        frame.render_widget(err_p, chunks[0]);
    }

    // 2. Render Caption and bottom controls
    let mut footer_lines = Vec::new();
    if !state.caption.is_empty() {
        footer_lines.push(Line::from(vec![
            Span::styled("Caption: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(&state.caption, Style::default().fg(Color::White)),
        ]));
    }

    footer_lines.push(Line::from(vec![
        Span::styled("[Esc / q] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::styled("Close  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[o] ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::styled("Desktop  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[Ctrl+S / s] ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled("Save to Downloads", Style::default().fg(Color::DarkGray)),
    ]));

    let footer_p = Paragraph::new(footer_lines)
        .wrap(Wrap { trim: true })
        .alignment(Alignment::Center);
    frame.render_widget(footer_p, chunks[1]);
}
