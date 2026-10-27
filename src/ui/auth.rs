//! S01: Authentication & 2FA Gate Screen
//! Renders Phone input, OTP SMS code verification, and Cloud Password steps.

use crate::actors::coordinator::AppCoordinator;
use crate::model::AuthStep;
use crate::theme::Theme;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::Modifier,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub fn render_auth_screen(f: &mut Frame, app: &AppCoordinator, theme: &Theme, area: Rect) {
    let block = Block::default()
        .title(" Telegram Desktop Authentication (S01) ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(theme.border_focused())
        .border_type(theme.border_type)
        .style(theme.base());

    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Logo / Header
            Constraint::Length(8), // Interactive Form Box
            Constraint::Min(4),    // Instructions
        ])
        .margin(1)
        .split(inner);

    // Header
    let header = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("TgTui", theme.message_author().add_modifier(Modifier::BOLD)),
            Span::raw(" — Official Clean-Room Client"),
        ]),
        Line::from(Span::styled(
            "Connect securely to Telegram MTProto network",
            theme.dialog_snippet(),
        )),
    ])
    .alignment(Alignment::Center);
    f.render_widget(header, chunks[0]);

    // Active Step Form
    match &app.auth_state {
        AuthStep::PhoneInput { buffer, error } => {
            let mut lines = vec![
                Line::from(Span::styled("Step 1 of 3: Enter your phone number", theme.message_author())),
                Line::from(Span::styled("Include country code (e.g. +1234567890):", theme.dialog_snippet())),
                Line::from(""),
                Line::from(vec![
                    Span::raw("> "),
                    Span::styled(
                        if buffer.is_empty() { "+1..." } else { buffer.as_str() },
                        if buffer.is_empty() { theme.dialog_snippet() } else { theme.dialog_item(true) },
                    ),
                    Span::styled("█", theme.border_focused()),
                ]),
            ];

            if let Some(err) = error {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(err.as_str(), theme.danger())));
            }

            let form = Paragraph::new(lines).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(theme.border())
                    .border_type(theme.border_type),
            );
            f.render_widget(form, chunks[1]);
        }
        AuthStep::CodeInput { phone, buffer, error } => {
            let mut lines = vec![
                Line::from(Span::styled("Step 2 of 3: Verification Code", theme.message_author())),
                Line::from(Span::styled(format!("Code sent to Telegram app on {}:", phone), theme.dialog_snippet())),
                Line::from(""),
                Line::from(vec![
                    Span::raw("Code: "),
                    Span::styled(
                        if buffer.is_empty() { "12345" } else { buffer.as_str() },
                        if buffer.is_empty() { theme.dialog_snippet() } else { theme.dialog_item(true) },
                    ),
                    Span::styled("█", theme.border_focused()),
                ]),
            ];

            if let Some(err) = error {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(err.as_str(), theme.danger())));
            }

            let form = Paragraph::new(lines).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(theme.border())
                    .border_type(theme.border_type),
            );
            f.render_widget(form, chunks[1]);
        }
        AuthStep::Password2FA { buffer, error } => {
            let masked: String = "•".repeat(buffer.len());
            let mut lines = vec![
                Line::from(Span::styled("Step 3 of 3: Two-Step Verification", theme.message_author())),
                Line::from(Span::styled("Enter your Cloud Password to unlock account:", theme.dialog_snippet())),
                Line::from(""),
                Line::from(vec![
                    Span::raw("Password: "),
                    Span::styled(
                        if masked.is_empty() { "(type cloud password)" } else { masked.as_str() },
                        if masked.is_empty() { theme.dialog_snippet() } else { theme.dialog_item(true) },
                    ),
                    Span::styled("█", theme.border_focused()),
                ]),
            ];

            if let Some(err) = error {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(err.as_str(), theme.danger())));
            }

            let form = Paragraph::new(lines).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(theme.border())
                    .border_type(theme.border_type),
            );
            f.render_widget(form, chunks[1]);
        }
        AuthStep::Authenticated => {}
    }

    // Help Footer
    let instructions = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("Enter", theme.badge_unread(false)),
            Span::raw(" Submit | "),
            Span::styled("Backspace", theme.dialog_snippet()),
            Span::raw(" Delete | "),
            Span::styled("F1", theme.dialog_snippet()),
            Span::raw(" Skip Auth / Toggle Shell Demo | "),
            Span::styled("Ctrl+C / q", theme.danger()),
            Span::raw(" Exit"),
        ]),
    ])
    .alignment(Alignment::Center);
    f.render_widget(instructions, chunks[2]);
}
