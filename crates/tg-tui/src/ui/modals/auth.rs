use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;
use tg_core::AuthState;

use super::{centered_rect_fixed, AuthModalState};
use crate::ui::qr::generate_qr_halfblock_lines;

/// Render the Telegram Desktop styled authentication interface based on the current `AuthState`.
pub fn render_auth_modal(
    frame: &mut Frame,
    auth_state: &AuthState,
    auth_modal: &AuthModalState,
    screen: Rect,
) {
    if screen.width < 10 || screen.height < 6 {
        return;
    }

    match auth_state {
        AuthState::WaitQrCode(qr_info) => {
            let qr_res = generate_qr_halfblock_lines(&qr_info.link);
            let qr_lines_count = qr_res.as_ref().map(|l| l.len()).unwrap_or(16);
            let qr_width = qr_res
                .as_ref()
                .ok()
                .and_then(|l| l.first())
                .map(|l| l.spans.len())
                .unwrap_or(29);

            // Compute ideal adaptive card dimensions:
            // 3 lines of header + qr_lines + 3 lines of footer = qr_lines_count + 6
            let needed_height = (qr_lines_count as u16 + 6).max(18);
            let target_height = needed_height.min(screen.height);
            let target_width = ((qr_width as u16) + 12).max(56).min(screen.width);

            let area = centered_rect_fixed(target_width, target_height, screen);
            frame.render_widget(Clear, area);

            let block = Block::default()
                .borders(Borders::ALL)
                .border_set(ratatui::symbols::border::ROUNDED)
                .title(" ✈ Telegram Desktop — Quick Login ")
                .title_alignment(Alignment::Center)
                .style(Style::default().fg(Color::Cyan));

            let mut lines = Vec::new();
            lines.push(Line::from(vec![
                Span::styled("Log in to Telegram by QR Code", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            ]));
            lines.push(Line::from(vec![
                Span::styled("1. Open Telegram  |  2. Settings > Devices > Link Desktop", Style::default().fg(Color::DarkGray)),
            ]));
            if qr_info.link.contains("MOCK_") {
                lines.push(Line::from(vec![
                    Span::styled("[!] ОФФЛАЙН MOCK-РЕЖИМ (тестовый токен): включите use_mock=false в config.toml", Style::default().fg(Color::Yellow)),
                ]));
            }
            lines.push(Line::default());

            match qr_res {
                Ok(qr_lines) => {
                    // Render ALL lines of the QR code completely without any truncation!
                    for ql in qr_lines {
                        lines.push(ql);
                    }
                }
                Err(err) => {
                    lines.push(Line::from(vec![
                        Span::styled("Failed to generate QR: ", Style::default().fg(Color::Red)),
                        Span::raw(err),
                    ]));
                }
            }

            lines.push(Line::default());
            lines.push(Line::from(vec![
                Span::styled("[P] ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::raw("Log in by phone instead  |  "),
                Span::styled("[F3] ", Style::default().fg(Color::Cyan)),
                Span::raw("Proxy  |  "),
                Span::styled("[Esc] ", Style::default().fg(Color::DarkGray)),
                Span::raw("Quit"),
            ]));

            let paragraph = Paragraph::new(lines)
                .block(block)
                .alignment(Alignment::Center);

            frame.render_widget(paragraph, area);
        }

        AuthState::WaitPhoneNumber => {
            let area = centered_rect_fixed(66, 18, screen);
            frame.render_widget(Clear, area);

            let block = Block::default()
                .borders(Borders::ALL)
                .title(" ✈ Telegram Desktop — Log In ")
                .title_alignment(Alignment::Center)
                .style(Style::default().fg(Color::Cyan));

            let lines = vec![
                Line::from(vec![
                    Span::styled("TELEGRAM DESKTOP", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                ]),
                Line::from(vec![
                    Span::styled("The world's fastest messaging app.", Style::default().fg(Color::DarkGray)),
                ]),
                Line::default(),
                // Tab bar
                Line::from(vec![
                    Span::styled("[ ▣ QR Code (Q) ]", Style::default().fg(Color::White)),
                    Span::raw("      "),
                    Span::styled("[ ☎ Phone Number ]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                    Span::raw(" (Active)"),
                ]),
                Line::default(),
                Line::from(vec![
                    Span::styled("Your Phone Number", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                ]),
                Line::from(vec![
                    Span::styled("Please confirm your country code and enter your phone number.", Style::default().fg(Color::DarkGray)),
                ]),
                Line::from(vec![
                    Span::styled("Examples: +1 (US)  +7 (RU/KZ)  +44 (UK)  +49 (DE)  +33 (FR)", Style::default().fg(Color::DarkGray)),
                ]),
                Line::default(),
                Line::from(vec![
                    Span::styled("  Phone Number: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                    Span::styled(format!("{}_", auth_modal.input_buffer), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                ]),
                Line::default(),
                Line::from(vec![
                    Span::styled("[Enter] ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                    Span::raw("Next  |  "),
                    Span::styled("[Q] ", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
                    Span::raw("Log in with QR  |  "),
                    Span::styled("[F3] ", Style::default().fg(Color::Cyan)),
                    Span::raw("Proxy  |  "),
                    Span::styled("[Esc] ", Style::default().fg(Color::DarkGray)),
                    Span::raw("Quit"),
                ]),
            ];

            let paragraph = Paragraph::new(lines)
                .block(block)
                .alignment(Alignment::Center);

            frame.render_widget(paragraph, area);
        }

        AuthState::WaitCode(code_info) => {
            let area = centered_rect_fixed(66, 18, screen);
            frame.render_widget(Clear, area);

            let block = Block::default()
                .borders(Borders::ALL)
                .title(" ✈ Telegram Desktop — Verification ")
                .title_alignment(Alignment::Center)
                .style(Style::default().fg(Color::Cyan));

            let phone_display = if !code_info.phone_number.is_empty() {
                code_info.phone_number.as_str()
            } else {
                "your Telegram account"
            };

            // Format stylized 5-digit boxes
            let buf_chars: Vec<char> = auth_modal.input_buffer.chars().collect();
            let mut digit_boxes = Vec::new();
            digit_boxes.push(Span::raw("Code:  "));
            for i in 0..5 {
                let ch = buf_chars.get(i).copied().unwrap_or('_');
                digit_boxes.push(Span::styled(format!("[ {} ] ", ch), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
            }

            let lines = vec![
                Line::from(vec![
                    Span::styled("TELEGRAM DESKTOP", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                ]),
                Line::default(),
                Line::from(vec![
                    Span::styled("Check Your Telegram App", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                ]),
                Line::from(vec![
                    Span::styled(format!("We have sent you a message with the code to: {}", phone_display), Style::default().fg(Color::DarkGray)),
                ]),
                Line::from(vec![
                    Span::styled("(Or enter the code sent via SMS)", Style::default().fg(Color::DarkGray)),
                ]),
                Line::default(),
                Line::from(digit_boxes),
                Line::default(),
                Line::from(vec![
                    Span::styled("Mock tip: '11111' to login  |  '22222' to test 2FA password", Style::default().fg(Color::DarkGray)),
                ]),
                Line::default(),
                Line::from(vec![
                    Span::styled("[Enter] ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                    Span::raw("Confirm  |  "),
                    Span::styled("[P] ", Style::default().fg(Color::Yellow)),
                    Span::raw("Change Phone  |  "),
                    Span::styled("[Esc] ", Style::default().fg(Color::DarkGray)),
                    Span::raw("Quit"),
                ]),
            ];

            let paragraph = Paragraph::new(lines)
                .block(block)
                .alignment(Alignment::Center);

            frame.render_widget(paragraph, area);
        }

        AuthState::WaitPassword(pwd_hint) => {
            let area = centered_rect_fixed(66, 18, screen);
            frame.render_widget(Clear, area);

            let block = Block::default()
                .borders(Borders::ALL)
                .title(" ✈ Telegram Desktop — Two-Step Verification ")
                .title_alignment(Alignment::Center)
                .style(Style::default().fg(Color::Magenta));

            let masked_pwd: String = if auth_modal.show_password_cleartext {
                auth_modal.input_buffer.clone()
            } else {
                "•".repeat(auth_modal.input_buffer.chars().count())
            };

            let hint_str = pwd_hint
                .hint
                .as_ref()
                .map(|h| format!("Hint: \"{}\"", h))
                .unwrap_or_else(|| "Default mock password is 'rust'".to_string());

            let toggle_label = if auth_modal.show_password_cleartext {
                "Hide"
            } else {
                "Show"
            };

            let lines = vec![
                Line::from(vec![
                    Span::styled("TELEGRAM DESKTOP", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
                ]),
                Line::default(),
                Line::from(vec![
                    Span::styled("Two-Step Verification", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                ]),
                Line::from(vec![
                    Span::styled("Your account is protected with an additional cloud password.", Style::default().fg(Color::DarkGray)),
                ]),
                Line::from(vec![
                    Span::styled(hint_str, Style::default().fg(Color::Yellow)),
                ]),
                Line::default(),
                Line::from(vec![
                    Span::styled("  Password: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                    Span::styled(format!("{}_", masked_pwd), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                ]),
                Line::default(),
                Line::from(vec![
                    Span::styled("[Ctrl+V] ", Style::default().fg(Color::Cyan)),
                    Span::raw(format!("{} Password", toggle_label)),
                ]),
                Line::default(),
                Line::from(vec![
                    Span::styled("[Enter] ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                    Span::raw("Submit  |  "),
                    Span::styled("[Esc] ", Style::default().fg(Color::DarkGray)),
                    Span::raw("Back"),
                ]),
            ];

            let paragraph = Paragraph::new(lines)
                .block(block)
                .alignment(Alignment::Center);

            frame.render_widget(paragraph, area);
        }

        AuthState::WaitRegistration => {
            let area = centered_rect_fixed(60, 14, screen);
            frame.render_widget(Clear, area);

            let block = Block::default()
                .borders(Borders::ALL)
                .title(" ✈ Telegram Desktop — Registration ")
                .title_alignment(Alignment::Center)
                .style(Style::default().fg(Color::Cyan));

            let lines = vec![
                Line::from(vec![
                    Span::styled("Complete Your Profile", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                ]),
                Line::from(vec![
                    Span::styled("Enter your first and last name to finish registration.", Style::default().fg(Color::DarkGray)),
                ]),
                Line::default(),
                Line::from(vec![
                    Span::styled("First Name: ", Style::default().fg(Color::Yellow)),
                    Span::raw(&auth_modal.first_name),
                ]),
                Line::from(vec![
                    Span::styled("Last Name:  ", Style::default().fg(Color::Yellow)),
                    Span::raw(&auth_modal.last_name),
                ]),
                Line::default(),
                Line::from(vec![
                    Span::styled("[Enter] ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                    Span::raw("Register  |  "),
                    Span::styled("[Esc] ", Style::default().fg(Color::DarkGray)),
                    Span::raw("Quit"),
                ]),
            ];

            let paragraph = Paragraph::new(lines)
                .block(block)
                .alignment(Alignment::Center);

            frame.render_widget(paragraph, area);
        }

        AuthState::WaitParameters => {
            let area = centered_rect_fixed(56, 8, screen);
            frame.render_widget(Clear, area);

            let block = Block::default()
                .borders(Borders::ALL)
                .border_set(ratatui::symbols::border::ROUNDED)
                .title(" ✈ Telegram Client ")
                .title_alignment(Alignment::Center)
                .style(Style::default().fg(Color::Cyan));

            let lines = vec![
                Line::default(),
                Line::from(vec![
                    Span::styled("Восстановление сохранённой сессии...", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                ]),
                Line::from(vec![
                    Span::styled("Проверка локальной базы данных TDLib...", Style::default().fg(Color::DarkGray)),
                ]),
            ];

            let paragraph = Paragraph::new(lines)
                .block(block)
                .alignment(Alignment::Center);

            frame.render_widget(paragraph, area);
        }

        AuthState::LoggingOut => {
            let area = centered_rect_fixed(50, 8, screen);
            frame.render_widget(Clear, area);

            let block = Block::default()
                .borders(Borders::ALL)
                .title(" Logging Out ")
                .title_alignment(Alignment::Center)
                .style(Style::default().fg(Color::Red));

            let lines = vec![
                Line::default(),
                Line::from(vec![
                    Span::styled("Terminating session and clearing local cache...", Style::default().fg(Color::White)),
                ]),
            ];

            let paragraph = Paragraph::new(lines)
                .block(block)
                .alignment(Alignment::Center);

            frame.render_widget(paragraph, area);
        }

        _ => {}
    }
}
