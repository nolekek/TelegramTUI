pub mod attach_modal;
pub mod auth;
pub mod composer;
pub mod context_menu;
pub mod dialogs;
pub mod forward_modal;
pub mod lightbox;
pub mod messages;
pub mod profile;
pub mod reaction_picker;
pub mod search;
pub mod settings;

use crate::actors::coordinator::AppCoordinator;
use crate::model::{ActiveModal, AuthStep, FocusPane};
use crate::theme::Theme;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::Modifier,
    text::{Line, Span},
    widgets::{Block, Paragraph},
    Frame,
};

pub fn render_app(f: &mut Frame, app: &AppCoordinator, theme: &Theme) {
    let size = f.area();

    // 1. If unauthenticated, show S01 Auth Gate
    if app.auth_state != AuthStep::Authenticated {
        auth::render_auth_screen(f, app, theme, size);
        return;
    }

    // 2. Base fill
    let bg_block = Block::default().style(theme.base());
    f.render_widget(bg_block, size);

    // 3. Layout: Top Bar + Main Shell + Status Footer
    let vertical_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Top Brand Bar
            Constraint::Min(8),    // 2-Column Main Workspace
            Constraint::Length(1), // Bottom Status / Keybinding line
        ])
        .split(size);

    // Top Brand Bar
    let active_pane_name = match app.focus {
        FocusPane::Folders => "Folders",
        FocusPane::Dialogs => "Chats",
        FocusPane::Messages => "Conversation",
        FocusPane::Composer => "Composer",
    };

    let brand_line = Line::from(vec![
        Span::styled(" TgTui ", theme.badge_unread(false).add_modifier(Modifier::BOLD)),
        Span::raw(" | Telegram Desktop Terminal Client | Focus: "),
        Span::styled(active_pane_name, theme.message_author()),
        Span::raw(" | "),
        Span::styled("[?] Help", theme.dialog_snippet()),
        Span::raw(" | "),
        Span::styled("[F1] Auth Demo", theme.dialog_snippet()),
    ]);
    f.render_widget(Paragraph::new(brand_line), vertical_chunks[0]);

    // Main 2-Column Split: Dialogs (Left 30%), Conversation (Right 70%)
    let horizontal_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(32), // Left: Folders + Dialogs List
            Constraint::Percentage(68), // Right: Messages + Composer
        ])
        .split(vertical_chunks[1]);

    // Left Column
    dialogs::render_dialogs_panel(f, app, theme, horizontal_chunks[0]);

    // Right Column: Messages (Top) + Composer (Bottom)
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(6),    // History
            Constraint::Length(4), // Composer
        ])
        .split(horizontal_chunks[1]);

    messages::render_messages_panel(f, app, theme, right_chunks[0]);
    composer::render_composer_panel(f, app, theme, right_chunks[1]);

    // Bottom Status / Notification line
    let status_text = if let Some((msg, time)) = &app.status_notification {
        if time.elapsed().as_secs() < 4 {
            Line::from(vec![
                Span::styled("⚡ ", theme.warning()),
                Span::styled(msg.as_str(), theme.dialog_item(true)),
            ])
        } else {
            default_status_line(theme)
        }
    } else {
        default_status_line(theme)
    };
    f.render_widget(Paragraph::new(status_text), vertical_chunks[2]);

    // Render Active Modals on top
    match &app.active_modal {
        ActiveModal::ContextMenu { .. } => {
            context_menu::render_context_menu(f, app, theme, size);
        }
        ActiveModal::ChatProfile => {
            profile::render_profile_modal(f, app, theme, size);
        }
        ActiveModal::Search { .. } => {
            search::render_search_modal(f, app, theme, size);
        }
        ActiveModal::SettingsHelp => {
            settings::render_settings_help_modal(f, app, theme, size);
        }
        ActiveModal::ReactionPicker { .. } => {
            reaction_picker::render_reaction_picker(f, app, theme, size);
        }
        ActiveModal::ForwardModal { .. } => {
            forward_modal::render_forward_modal(f, app, theme, size);
        }
        ActiveModal::Lightbox { .. } => {
            lightbox::render_lightbox(f, app, theme, size);
        }
        ActiveModal::AttachFile { .. } => {
            attach_modal::render_attach_modal(f, app, theme, size);
        }
        ActiveModal::None => {}
    }
}

fn default_status_line(theme: &Theme) -> Line<'static> {
    Line::from(vec![
        Span::styled("[Tab] ", theme.dialog_snippet()),
        Span::raw("Pane | "),
        Span::styled("[j/k] ", theme.dialog_snippet()),
        Span::raw("Nav | "),
        Span::styled("[r] ", theme.dialog_snippet()),
        Span::raw("Reply | "),
        Span::styled("[+] ", theme.badge_unread(false)),
        Span::raw("React | "),
        Span::styled("[f] ", theme.badge_unread(false)),
        Span::raw("Fwd | "),
        Span::styled("[a] ", theme.badge_unread(false)),
        Span::raw("Attach | "),
        Span::styled("[e] ", theme.dialog_snippet()),
        Span::raw("Edit | "),
        Span::styled("[m] ", theme.dialog_snippet()),
        Span::raw("Menu | "),
        Span::styled("[q] ", theme.danger()),
        Span::raw("Quit"),
    ])
}
