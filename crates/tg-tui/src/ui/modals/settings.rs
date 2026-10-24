use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;
use tg_core::CoreState;

use super::{centered_rect_fixed, SettingsModalState};
use crate::app::NavigationMode;

/// Render the interactive Settings modal dialog with all user options and navigation mode switch.
pub fn render_settings_modal(
    frame: &mut Frame,
    state: &CoreState,
    settings: &SettingsModalState,
    navigation_mode: NavigationMode,
    screen: Rect,
) {
    let area = centered_rect_fixed(72, 23, screen);
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_set(symbols::border::ROUNDED)
        .title(" ⚙ Настройки клиента / Client Settings ")
        .title_alignment(Alignment::Center)
        .style(Style::default().fg(Color::Rgb(42, 171, 238))); // Telegram blue

    let user_name = state
        .me
        .as_ref()
        .map(|u| u.full_name())
        .unwrap_or_else(|| "Not authenticated".to_string());

    let username = state
        .me
        .as_ref()
        .and_then(|u| u.username.as_ref())
        .map(|un| format!("@{}", un))
        .unwrap_or_else(|| "None".to_string());

    let phone = state
        .me
        .as_ref()
        .and_then(|u| u.phone_number.as_ref())
        .map(|p| p.as_str())
        .unwrap_or("None");

    let user_id = state
        .me
        .as_ref()
        .map(|u| u.id.0.to_string())
        .unwrap_or_else(|| "N/A".to_string());

    let mut lines = Vec::new();
    lines.push(Line::default());

    // --- Navigation Mode Item (Index 0) ---
    let is_sel_0 = settings.selected_index == 0;
    let mode_style = if is_sel_0 {
        Style::default().bg(Color::Rgb(30, 45, 75)).fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };
    let mode_val = match navigation_mode {
        NavigationMode::Vim => "⚡ VIM Режим (Normal / Insert / Search — j/k, /, i, r, e, dd)",
        NavigationMode::Classic => "🖱 Обычный Режим (Стрелки, прямой ввод, Tab, Enter)",
    };
    lines.push(Line::from(vec![
        Span::styled(if is_sel_0 { " ❯ " } else { "   " }, Style::default().fg(Color::Yellow)),
        Span::styled("Режим управления: ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled(format!("[ {} ]", mode_val), mode_style),
    ]));

    // --- Show Last Message Snippets (Index 1) ---
    let is_sel_1 = settings.selected_index == 1;
    let snip_style = if is_sel_1 {
        Style::default().bg(Color::Rgb(30, 45, 75)).fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };
    let snip_val = if settings.show_last_message_preview { "✔ Включено (2 строки на чат)" } else { "✖ Выключено (компактный список)" };
    lines.push(Line::from(vec![
        Span::styled(if is_sel_1 { " ❯ " } else { "   " }, Style::default().fg(Color::Yellow)),
        Span::styled("Превью сообщений в чатах: ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled(format!("[ {} ]", snip_val), snip_style),
    ]));

    // --- Notifications (Index 2) ---
    let is_sel_2 = settings.selected_index == 2;
    let notif_style = if is_sel_2 {
        Style::default().bg(Color::Rgb(30, 45, 75)).fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };
    let notif_val = if settings.notifications_enabled { "✔ Включены" } else { "✖ Выключены" };
    lines.push(Line::from(vec![
        Span::styled(if is_sel_2 { " ❯ " } else { "   " }, Style::default().fg(Color::Yellow)),
        Span::styled("Уведомления о новых сообщениях: ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled(format!("[ {} ]", notif_val), notif_style),
    ]));

    // --- Proxy Settings (Index 3) ---
    let is_sel_3 = settings.selected_index == 3;
    let proxy_style = if is_sel_3 {
        Style::default().bg(Color::Rgb(30, 45, 75)).fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };
    lines.push(Line::from(vec![
        Span::styled(if is_sel_3 { " ❯ " } else { "   " }, Style::default().fg(Color::Yellow)),
        Span::styled("Сетевой прокси: ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled("[ Настроить SOCKS5 / HTTP / MTProto (Enter) ]", proxy_style),
    ]));

    lines.push(Line::default());
    lines.push(Line::from(vec![
        Span::styled(" ── Информация об аккаунте / Account ──────────────────────", Style::default().fg(Color::DarkGray)),
    ]));

    lines.push(Line::from(vec![
        Span::raw("   "),
        Span::styled("Имя: ", Style::default().fg(Color::Gray)),
        Span::styled(user_name, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        Span::raw("   "),
        Span::styled("Username: ", Style::default().fg(Color::Gray)),
        Span::styled(username, Style::default().fg(Color::Cyan)),
    ]));

    lines.push(Line::from(vec![
        Span::raw("   "),
        Span::styled("Телефон: ", Style::default().fg(Color::Gray)),
        Span::styled(phone, Style::default().fg(Color::White)),
        Span::raw("   "),
        Span::styled("User ID: ", Style::default().fg(Color::Gray)),
        Span::styled(user_id, Style::default().fg(Color::White)),
    ]));

    lines.push(Line::from(vec![
        Span::raw("   "),
        Span::styled("Чатов в кэше: ", Style::default().fg(Color::Gray)),
        Span::styled(state.chats.len().to_string(), Style::default().fg(Color::Green)),
        Span::raw("   "),
        Span::styled("Соединение: ", Style::default().fg(Color::Gray)),
        Span::styled(format!("{:?}", state.connection_state), Style::default().fg(Color::Green)),
    ]));

    lines.push(Line::default());
    lines.push(Line::from(vec![
        Span::styled(" ── Управление сессией / Session ─────────────────────────", Style::default().fg(Color::DarkGray)),
    ]));

    // --- Logout Item (Index 4) ---
    let is_sel_4 = settings.selected_index == 4;
    let logout_style = if is_sel_4 {
        Style::default().bg(Color::Red).fg(Color::White).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::LightRed)
    };
    let logout_text = if settings.confirm_logout {
        "⚠ ПОДТВЕРДИТЕ ВЫХОД: Нажмите 'Y' для выхода или 'N' для отмены"
    } else {
        "Выйти из аккаунта (Log Out)"
    };
    lines.push(Line::from(vec![
        Span::styled(if is_sel_4 { " ❯ " } else { "   " }, Style::default().fg(Color::Yellow)),
        Span::styled(format!("[ {} ]", logout_text), logout_style),
    ]));

    lines.push(Line::default());
    lines.push(Line::from(vec![
        Span::styled(" [↑/↓ или j/k] Выбор  [Enter/Пробел] Переключить  [Esc] Закрыть ", Style::default().fg(Color::DarkGray)),
    ]));

    let paragraph = Paragraph::new(lines)
        .block(block)
        .alignment(Alignment::Left);

    frame.render_widget(paragraph, area);
}
