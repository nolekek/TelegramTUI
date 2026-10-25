pub mod chat_list;
pub mod image;
pub mod messages;
pub mod modals;
pub mod qr;
pub mod status_bar;

use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;
use tg_core::AuthState;

use crate::app::{App, CurrentScreen, NavigationMode, VimMode};
use crate::ui::chat_list::render_chat_search;
use crate::ui::messages::render_messages;
use crate::ui::modals::{
    auth::render_auth_modal, help::render_help_modal, image_viewer::render_image_viewer,
    proxy::render_proxy_modal, settings::render_settings_modal, ActiveModal,
};
use crate::ui::status_bar::render_status_bar;

/// Main UI render coordinator with modern styling, full-screen chat switcher, and Vim workflow.
pub fn render(frame: &mut Frame, app: &mut App) {
    let screen = frame.area();
    let state = app.handle.clone_state();

    let root_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Top Header
            Constraint::Min(6),    // Main Workspace
            Constraint::Length(1), // Footer Status Bar / Command Line
        ])
        .split(screen);

    let header_area = root_layout[0];
    let body_area = root_layout[1];
    let status_area = root_layout[2];

    // 1. Sleek Modern Header
    let user_info = state
        .me
        .as_ref()
        .map(|u| {
            if let Some(ref un) = u.username {
                format!("{} (@{})", u.full_name(), un)
            } else {
                u.full_name()
            }
        })
        .unwrap_or_else(|| "Не авторизован".to_string());

    let (auth_pill, auth_style) = match &state.auth_state {
        AuthState::Ready => (" ● В сети ", Style::default().bg(Color::Rgb(20, 60, 40)).fg(Color::Rgb(115, 218, 154)).add_modifier(Modifier::BOLD)),
        AuthState::WaitPhoneNumber => (" ⚠ Вход в аккаунт ", Style::default().bg(Color::Rgb(70, 50, 20)).fg(Color::Rgb(250, 200, 100))),
        AuthState::WaitCode(_) => (" ⏳ Ввод кода ", Style::default().bg(Color::Rgb(70, 50, 20)).fg(Color::Rgb(250, 200, 100))),
        AuthState::WaitPassword(_) => (" 🔒 2FA Пароль ", Style::default().bg(Color::Rgb(60, 30, 80)).fg(Color::Rgb(210, 160, 255))),
        AuthState::WaitQrCode(_) => (" 📷 Вход по QR ", Style::default().bg(Color::Rgb(20, 60, 70)).fg(Color::Rgb(100, 230, 230))),
        AuthState::LoggingOut => (" 🔄 Выход... ", Style::default().bg(Color::Rgb(40, 45, 60)).fg(Color::White)),
        _ => (" 🔄 Подключение... ", Style::default().bg(Color::Rgb(40, 45, 60)).fg(Color::White)),
    };

    let mode_pill = match app.navigation_mode {
        NavigationMode::Vim => match app.vim_mode {
            VimMode::Normal => Span::styled(" ⚡ VIM: NORMAL ", Style::default().bg(Color::Rgb(30, 60, 90)).fg(Color::Rgb(125, 207, 255)).add_modifier(Modifier::BOLD)),
            VimMode::Insert => Span::styled(" ⚡ VIM: INSERT ", Style::default().bg(Color::Rgb(70, 50, 20)).fg(Color::Rgb(250, 200, 100)).add_modifier(Modifier::BOLD)),
            VimMode::Command => Span::styled(" ⚡ VIM: COMMAND ", Style::default().bg(Color::Rgb(60, 30, 80)).fg(Color::Rgb(210, 160, 255)).add_modifier(Modifier::BOLD)),
            VimMode::Search => Span::styled(" ⚡ VIM: SEARCH ", Style::default().bg(Color::Rgb(20, 60, 70)).fg(Color::Rgb(100, 230, 230)).add_modifier(Modifier::BOLD)),
        },
        NavigationMode::Classic => Span::styled(" 🖱 КЛАССИКА ", Style::default().bg(Color::Rgb(30, 50, 80)).fg(Color::Rgb(140, 200, 255)).add_modifier(Modifier::BOLD)),
    };

    let active_chat_title = if app.current_screen == CurrentScreen::ChatView {
        app.active_chat_id()
            .and_then(|id| state.get_chat(id).map(|c| format!("💬 {}", c.title)))
            .unwrap_or_else(|| "💬 Чат".to_string())
    } else {
        "🔍 ПОИСК ЧАТОВ".to_string()
    };

    let header_line = Line::from(vec![
        Span::styled(
            " ✈ TELEGRAM ",
            Style::default()
                .fg(Color::Rgb(88, 166, 255))
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("│ "),
        mode_pill,
        Span::raw(" │ "),
        Span::styled(active_chat_title, Style::default().fg(Color::Rgb(125, 207, 255)).add_modifier(Modifier::BOLD)),
        Span::raw(" │ "),
        Span::styled("👤 ", Style::default().fg(Color::Rgb(187, 154, 247))),
        Span::styled(user_info, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        Span::raw(" │ "),
        Span::styled(auth_pill, auth_style),
        Span::raw(" │ "),
        Span::styled("[F2] Настройки  [F3] Прокси  [F4] Режим", Style::default().fg(Color::Rgb(108, 112, 134))),
    ]);

    let header_widget = Paragraph::new(header_line).block(
        Block::default()
            .borders(Borders::ALL)
            .border_set(symbols::border::ROUNDED)
            .border_style(Style::default().fg(Color::Rgb(70, 80, 110))),
    );
    frame.render_widget(header_widget, header_area);

    // 2. In-TUI Auth Modal fallback if unauthenticated
    if state.auth_state != AuthState::Ready {
        let err_opt = if app.is_status_error { Some(app.status_message.as_str()) } else { None };
        render_auth_modal(frame, &state.auth_state, &app.auth_modal_state, err_opt, screen);
        render_status_bar(
            frame,
            app.navigation_mode,
            app.vim_mode,
            &app.status_message,
            app.is_status_error,
            &state.connection_state,
            None,
            status_area,
        );
        return;
    }

    // 3. Screen Dispatching
    match app.current_screen {
        CurrentScreen::ChatSearch => {
            render_chat_search(
                frame,
                &state,
                app.active_tab.clone(),
                &app.search_query,
                app.selected_chat_index,
                &mut app.chat_list_state,
                body_area,
            );
        }
        CurrentScreen::ChatView => {
            let active_chat = app.active_chat_id();
            let show_input = app.navigation_mode == NavigationMode::Classic || app.vim_mode == VimMode::Insert;
            let (msg_area, input_chunk) = if show_input {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Min(4),
                        Constraint::Length(3),
                    ])
                    .split(body_area);
                (chunks[0], Some(chunks[1]))
            } else {
                (body_area, None)
            };

            render_messages(
                frame,
                &state,
                active_chat,
                app.selected_message_index,
                app.message_scroll,
                true,
                msg_area,
            );

            if let Some(i_area) = input_chunk {
                let title = if let Some((_, ref snippet)) = app.reply_target {
                    format!(" ↩ Ответ на: \"{}\" (Enter — отправить, Esc — выйти) ", snippet)
                } else if let Some(_) = app.edit_target {
                    " ✎ Редактирование сообщения (Enter — сохранить, Esc — выйти) ".to_string()
                } else if let Some(chat_id) = active_chat {
                    let name = state.get_chat(chat_id).map(|c| c.title.as_str()).unwrap_or("Чат");
                    if app.navigation_mode == NavigationMode::Classic {
                        format!(" ✉ Сообщение в [{}] (Enter — отправить, Esc — к чатам) ", name)
                    } else {
                        format!(" [INSERT] ✉ Написать в [{}] (Enter — отправить, Esc — Normal) ", name)
                    }
                } else {
                    " ✉ Написать сообщение (Enter — отправить, Esc — выйти) ".to_string()
                };

                let input_widget = Paragraph::new(app.input_text.clone())
                    .style(Style::default().fg(Color::White))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_set(symbols::border::ROUNDED)
                            .title(title)
                            .border_style(Style::default().fg(Color::Rgb(88, 166, 255)).add_modifier(Modifier::BOLD)),
                    );
                frame.render_widget(input_widget, i_area);

                // Set hardware terminal cursor when in insert mode
                if i_area.width > 0 && i_area.height > 0 {
                    let max_cursor_x = i_area.x.saturating_add(i_area.width.saturating_sub(1));
                    let cursor_x = (i_area.x.saturating_add(1).saturating_add(app.cursor_pos as u16)).min(max_cursor_x);
                    let cursor_y = i_area.y.saturating_add(1);
                    if cursor_y < i_area.y.saturating_add(i_area.height) {
                        frame.set_cursor_position((cursor_x, cursor_y));
                    }
                }
            }
        }
    }

    // 4. Status Line / Command Line
    if app.vim_mode == VimMode::Command {
        let cmd_line = Line::from(vec![
            Span::styled(":", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(&app.cmd_buffer, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled("█", Style::default().fg(Color::Yellow)),
        ]);
        let cmd_widget = Paragraph::new(cmd_line)
            .style(Style::default().bg(Color::Rgb(15, 18, 25)));
        frame.render_widget(cmd_widget, status_area);
    } else {
        render_status_bar(
            frame,
            app.navigation_mode,
            app.vim_mode,
            &app.status_message,
            app.is_status_error,
            &state.connection_state,
            None,
            status_area,
        );
    }

    // 5. Modal Dialog Overlays (Settings & Proxy & Image Viewer)
    match app.active_modal {
        ActiveModal::Settings => {
            render_settings_modal(frame, &state, &app.settings_modal_state, app.navigation_mode, screen);
        }
        ActiveModal::Proxy => {
            render_proxy_modal(frame, &app.proxy_modal_state, screen);
        }
        ActiveModal::ImageViewer => {
            if let Some(ref iv_state) = app.image_viewer_state {
                render_image_viewer(frame, iv_state, screen);
            }
        }
        ActiveModal::Help => {
            render_help_modal(frame, screen);
        }
        _ => {}
    }
}
