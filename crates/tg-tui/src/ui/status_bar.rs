use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use tg_core::ConnectionState;

use crate::app::{NavigationMode, VimMode};

/// Render the bottom status bar with Navigation/Vim mode badge, system messages, and mode-specific shortcuts.
pub fn render_status_bar(
    frame: &mut Frame,
    navigation_mode: NavigationMode,
    vim_mode: VimMode,
    status_message: &str,
    is_error: bool,
    connection_state: &ConnectionState,
    audio_status: Option<&str>,
    area: Rect,
) {
    let (mode_badge, mode_style) = match navigation_mode {
        NavigationMode::Classic => (
            " 🖱 КЛАССИКА ",
            Style::default()
                .bg(Color::Rgb(30, 50, 80))
                .fg(Color::Rgb(140, 200, 255))
                .add_modifier(Modifier::BOLD),
        ),
        NavigationMode::Vim => match vim_mode {
            VimMode::Normal => (
                " ⚡ VIM: NORMAL ",
                Style::default()
                    .bg(Color::Rgb(30, 60, 90))
                    .fg(Color::Rgb(125, 207, 255))
                    .add_modifier(Modifier::BOLD),
            ),
            VimMode::Insert => (
                " ⚡ VIM: INSERT ",
                Style::default()
                    .bg(Color::Rgb(70, 50, 20))
                    .fg(Color::Rgb(250, 200, 100))
                    .add_modifier(Modifier::BOLD),
            ),
            VimMode::Search => (
                " ⚡ VIM: SEARCH ",
                Style::default()
                    .bg(Color::Rgb(20, 60, 70))
                    .fg(Color::Rgb(100, 230, 230))
                    .add_modifier(Modifier::BOLD),
            ),
            VimMode::Command => (
                " ⚡ VIM: COMMAND ",
                Style::default()
                    .bg(Color::Rgb(60, 30, 80))
                    .fg(Color::Rgb(210, 160, 255))
                    .add_modifier(Modifier::BOLD),
            ),
        },
    };

    let (conn_text, conn_color) = match connection_state {
        ConnectionState::WaitingForNetwork => ("[🔴 Нет сети]", Color::Rgb(247, 118, 142)),
        ConnectionState::ConnectingToProxy => ("[🟡 Прокси...]", Color::Rgb(250, 200, 100)),
        ConnectionState::Connecting => ("[🟡 Подключение...]", Color::Rgb(250, 200, 100)),
        ConnectionState::Updating => ("[🔄 Синхронизация...]", Color::Rgb(125, 207, 255)),
        ConnectionState::Ready => ("[🟢 В сети]", Color::Rgb(115, 218, 154)),
    };

    let msg_style = if is_error {
        Style::default().fg(Color::Rgb(247, 118, 142)).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Rgb(220, 224, 232))
    };

    let hint_text = match navigation_mode {
        NavigationMode::Classic => "[↑/↓] Скролл  [Ctrl+F / Esc] Поиск чатов  [F2] Настройки  [F4] Vim режим",
        NavigationMode::Vim => match vim_mode {
            VimMode::Normal => "[/ / s] Поиск  [j/k] Скролл  [i/Enter] Написать  [r] Ответ  [dd] Удалить  [Esc] К чатам  [:] Меню",
            VimMode::Insert => "[Enter] Отправить  [Esc] В обычный режим (Normal)  [Ctrl+F] Поиск чатов",
            VimMode::Search => "[Enter] Открыть  [Ctrl+j/k] Выбор  [Esc] Назад",
            VimMode::Command => "[Enter] Выполнить  [Esc] Отмена  (:q выход, :find поиск, :w настройки)",
        },
    };

    let mut spans = vec![
        Span::styled(mode_badge, mode_style),
        Span::raw(" "),
        Span::styled(format!("{} ", conn_text), Style::default().fg(conn_color)),
        Span::raw("│ "),
        Span::styled(status_message, msg_style),
    ];

    if let Some(audio) = audio_status {
        spans.push(Span::raw(" │ "));
        spans.push(Span::styled(
            audio,
            Style::default()
                .fg(Color::LightGreen)
                .add_modifier(Modifier::BOLD),
        ));
    }

    spans.push(Span::raw(" │ "));
    spans.push(Span::styled(hint_text, Style::default().fg(Color::DarkGray)));

    let line = Line::from(spans);

    let paragraph = Paragraph::new(line)
        .style(Style::default().bg(Color::Rgb(18, 20, 26)))
        .alignment(Alignment::Left);

    frame.render_widget(paragraph, area);
}
