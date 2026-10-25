use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use super::centered_rect;

pub fn render_help_modal(frame: &mut Frame, area: Rect) {
    let popup_area = centered_rect(80, 85, area);
    frame.render_widget(Clear, popup_area);

    let title_span = Span::styled(
        " 📖 TelegramTUI — Справка и горячие клавиши ",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    );

    let block = Block::default()
        .borders(Borders::ALL)
        .border_set(symbols::border::ROUNDED)
        .border_style(Style::default().fg(Color::Cyan))
        .title(title_span);

    frame.render_widget(block.clone(), popup_area);

    let inner = block.inner(popup_area);
    if inner.width < 10 || inner.height < 5 {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(4),
            Constraint::Length(1),
        ])
        .split(inner);

    let rows = vec![
        Line::from(vec![
            Span::styled("🎬 МЕДИА И ФАЙЛЫ (КЭШ И СКАЧИВАНИЕ)", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("  Enter / o   ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw("Открыть/воспроизвести медиа (Фото в терминале, Аудио через rodio, Видео в плеере)"),
        ]),
        Line::from(vec![
            Span::styled("  Ctrl+S / D  ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw("Сохранить файл насовсем в папку downloads/ (кэш чистится при выходе)"),
        ]),
        Line::from(vec![
            Span::styled("  s           ", Style::default().fg(Color::LightRed).add_modifier(Modifier::BOLD)),
            Span::raw("Остановить воспроизведение текущего аудио/голосового"),
        ]),
        Line::default(),
        Line::from(vec![
            Span::styled("💬 ЧАТ И СООБЩЕНИЯ (NORMAL РЕЖИМ)", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("  i / a       ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw("Перейти в режим ввода (Insert) для написания сообщения"),
        ]),
        Line::from(vec![
            Span::styled("  r           ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw("Ответить на выбранное сообщение (Reply)"),
        ]),
        Line::from(vec![
            Span::styled("  e           ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw("Редактировать выбранное сообщение (Edit)"),
        ]),
        Line::from(vec![
            Span::styled("  dd          ", Style::default().fg(Color::LightRed).add_modifier(Modifier::BOLD)),
            Span::raw("Удалить выбранное сообщение"),
        ]),
        Line::from(vec![
            Span::styled("  j / k / ↑ / ↓ ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::raw("Навигация по сообщениям (с автоподгрузкой старых сообщений)"),
        ]),
        Line::from(vec![
            Span::styled("  Ctrl+D / Ctrl+U ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::raw("Быстрый скролл сообщений (по 10 сообщений)"),
        ]),
        Line::from(vec![
            Span::styled("  G / gg      ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::raw("Переход в самый низ (новые) / в самый верх (старые)"),
        ]),
        Line::from(vec![
            Span::styled("  Esc / / / q ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::raw("Выйти в окно списка и поиска чатов"),
        ]),
        Line::default(),
        Line::from(vec![
            Span::styled("🔍 ПОИСК И ВКЛАДКИ ЧАТОВ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("  [ / ]       ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::raw("Переключение вкладок: Все / Личные / Группы / Каналы / Боты (Alt+х / Alt+ъ)"),
        ]),
        Line::from(vec![
            Span::styled("  1..5        ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::raw("Быстрый выбор вкладки по цифре"),
        ]),
        Line::from(vec![
            Span::styled("  Ctrl+U      ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::raw("Очистить строку поиска"),
        ]),
        Line::default(),
        Line::from(vec![
            Span::styled("⚙ СИСТЕМНЫЕ КЛАВИШИ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("  F1 / ?      ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw("Открыть/закрыть это окно справки"),
        ]),
        Line::from(vec![
            Span::styled("  F2 / :w     ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw("Настройки (Vim/Classic, превью сообщений, логаут)"),
        ]),
        Line::from(vec![
            Span::styled("  F3 / :proxy ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw("Настройки Прокси (SOCKS5, HTTP, MTProto)"),
        ]),
        Line::from(vec![
            Span::styled("  F4          ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw("Переключить режим навигации (Vim <-> Classic)"),
        ]),
        Line::from(vec![
            Span::styled("  Ctrl+C      ", Style::default().fg(Color::LightRed).add_modifier(Modifier::BOLD)),
            Span::raw("Принудительный выход из приложения"),
        ]),
        Line::default(),
        Line::from(vec![
            Span::styled("  Поддерживается русская раскладка (йцукен) для всех команд без переключения языка!", Style::default().fg(Color::DarkGray)),
        ]),
    ];

    let content_p = Paragraph::new(rows)
        .wrap(Wrap { trim: false });
    frame.render_widget(content_p, chunks[0]);

    let footer_p = Paragraph::new(Line::from(vec![
        Span::styled("[Esc / F1 / ? / Enter] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::styled("Закрыть справку", Style::default().fg(Color::DarkGray)),
    ]))
    .alignment(Alignment::Center);
    frame.render_widget(footer_p, chunks[1]);
}
