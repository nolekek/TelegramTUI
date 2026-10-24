use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use super::{centered_rect_fixed, ProxyModalState, ProxyTypeChoice};

/// Render the Proxy configuration modal dialog.
pub fn render_proxy_modal(frame: &mut Frame, proxy_state: &ProxyModalState, screen: Rect) {
    let area = centered_rect_fixed(66, 21, screen);
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Network Proxy Configuration ")
        .title_alignment(Alignment::Center)
        .style(Style::default().fg(Color::Yellow));

    let f_style = |idx: usize| {
        if proxy_state.selected_field == idx {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        }
    };

    let checkbox = if proxy_state.is_enabled { "[X] Enabled" } else { "[ ] Disabled" };

    let (s_radio, h_radio, m_radio) = match proxy_state.proxy_type {
        ProxyTypeChoice::Socks5 => ("(*) SOCKS5", "( ) HTTP", "( ) MTProto"),
        ProxyTypeChoice::Http => ("( ) SOCKS5", "(*) HTTP", "( ) MTProto"),
        ProxyTypeChoice::Mtproto => ("( ) SOCKS5", "( ) HTTP", "(*) MTProto"),
    };

    let mut lines = vec![
        Line::default(),
        Line::from(vec![
            Span::styled("1. Proxy State: ", f_style(0)),
            Span::styled(checkbox, if proxy_state.is_enabled { Style::default().fg(Color::Green) } else { Style::default().fg(Color::Red) }),
            Span::styled("  (Space to toggle)", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled("2. Protocol:    ", f_style(1)),
            Span::styled(format!("{}  {}  {}", s_radio, h_radio, m_radio), Style::default().fg(Color::Cyan)),
            Span::styled("  (Space to cycle)", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled("3. Server/Host: ", f_style(2)),
            Span::styled(
                if proxy_state.selected_field == 2 {
                    format!("{}_", proxy_state.server)
                } else {
                    proxy_state.server.clone()
                },
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("4. Port:        ", f_style(3)),
            Span::styled(
                if proxy_state.selected_field == 3 {
                    format!("{}_", proxy_state.port)
                } else {
                    proxy_state.port.clone()
                },
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("5. Username:    ", f_style(4)),
            Span::styled(
                if proxy_state.selected_field == 4 {
                    format!("{}_", proxy_state.username)
                } else if proxy_state.username.is_empty() {
                    "(None)".to_string()
                } else {
                    proxy_state.username.clone()
                },
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("6. Password:    ", f_style(5)),
            Span::styled(
                if proxy_state.selected_field == 5 {
                    format!("{}_", "*".repeat(proxy_state.password.len()))
                } else if proxy_state.password.is_empty() {
                    "(None)".to_string()
                } else {
                    "*".repeat(proxy_state.password.len())
                },
                Style::default().fg(Color::White),
            ),
        ]),
    ];

    if proxy_state.proxy_type == ProxyTypeChoice::Mtproto {
        lines.push(Line::from(vec![
            Span::styled("7. MTProto Sec: ", f_style(6)),
            Span::styled(
                if proxy_state.selected_field == 6 {
                    format!("{}_", proxy_state.secret)
                } else if proxy_state.secret.is_empty() {
                    "(None)".to_string()
                } else {
                    proxy_state.secret.clone()
                },
                Style::default().fg(Color::White),
            ),
        ]));
    } else {
        lines.push(Line::default());
    }

    lines.push(Line::default());
    lines.push(Line::from(vec![
        Span::styled("[Tab/Up/Down] ", Style::default().fg(Color::Yellow)),
        Span::raw("Navigate  |  "),
        Span::styled("[S] ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::raw("Save & Apply  |  "),
        Span::styled("[D] ", Style::default().fg(Color::Red)),
        Span::raw("Disable  |  "),
        Span::styled("[Esc] ", Style::default().fg(Color::DarkGray)),
        Span::raw("Close"),
    ]));

    let paragraph = Paragraph::new(lines)
        .block(block)
        .alignment(Alignment::Left);

    frame.render_widget(paragraph, area);
}
