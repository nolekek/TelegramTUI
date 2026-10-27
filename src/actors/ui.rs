//! Terminal UI Actor and Crossterm Event Loop for TgTui
//! Handles raw terminal setup, 60fps tick rate, and keyboard mapping.

use crate::actors::coordinator::{AppCoordinator, UserAction};
use crate::actors::tdlib::TdlibEvent;
use crate::model::FocusPane;
use crate::theme::Theme;
use crate::ui::render_app;
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::time::Duration;
use tokio::sync::mpsc;

pub struct UiActor {
    app: AppCoordinator,
    theme: Theme,
    tdlib_event_rx: mpsc::Receiver<TdlibEvent>,
}

impl UiActor {
    pub fn new(app: AppCoordinator, theme: Theme, tdlib_event_rx: mpsc::Receiver<TdlibEvent>) -> Self {
        Self {
            app,
            theme,
            tdlib_event_rx,
        }
    }

    pub async fn run(mut self) -> Result<(), Box<dyn std::error::Error>> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let tick_rate = Duration::from_millis(50);
        let mut running = true;

        while running {
            // Draw frame
            terminal.draw(|f| {
                render_app(f, &self.app, &self.theme);
            })?;

            tokio::select! {
                // Incoming TDLib backend events
                Some(event) = self.tdlib_event_rx.recv() => {
                    self.app.handle_tdlib_event(event).await;
                }

                // Poll keyboard events with timeout
                _ = tokio::time::sleep(tick_rate) => {
                    while event::poll(Duration::from_millis(0))? {
                        if let Event::Key(key) = event::read()? {
                            if let Some(action) = Self::map_key_event(&self.app, key) {
                                if !self.app.handle_user_action(action).await {
                                    running = false;
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }

        // Cleanup terminal
        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;

        Ok(())
    }

    fn map_key_event(app: &AppCoordinator, key: event::KeyEvent) -> Option<UserAction> {
        // Global quit check
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return Some(UserAction::Quit);
        }

        if key.code == KeyCode::F(1) {
            return Some(UserAction::ToggleAuthDemo);
        }

        if key.code == KeyCode::Esc {
            return Some(UserAction::CloseModal);
        }

        // Modal specific bindings
        match &app.active_modal {
            crate::model::ActiveModal::ForwardModal { .. } => {
                match key.code {
                    KeyCode::Char(' ') => return Some(UserAction::ToggleForwardHideAuthor),
                    KeyCode::Enter => return Some(UserAction::SubmitForward),
                    KeyCode::Up | KeyCode::Char('k') => return Some(UserAction::PrevItem),
                    KeyCode::Down | KeyCode::Char('j') => return Some(UserAction::NextItem),
                    _ => {}
                }
            }
            crate::model::ActiveModal::AttachFile { .. } => {
                match key.code {
                    KeyCode::Tab => return Some(UserAction::ToggleAttachCompress),
                    KeyCode::Enter => return Some(UserAction::SubmitAttachFile),
                    KeyCode::Backspace => return Some(UserAction::Backspace),
                    KeyCode::Char(c) => return Some(UserAction::InputChar(c)),
                    _ => {}
                }
            }
            crate::model::ActiveModal::ReactionPicker { .. } => {
                if let KeyCode::Char(c) = key.code {
                    return Some(UserAction::InputChar(c));
                }
            }
            crate::model::ActiveModal::Lightbox { .. } => {
                if let KeyCode::Char('o') = key.code {
                    return Some(UserAction::ToggleVoicePlay);
                }
            }
            _ => {}
        }

        // If composing or typing in a modal or auth screen
        let is_typing_mode = app.focus == FocusPane::Composer
            || app.active_modal != crate::model::ActiveModal::None
            || app.auth_state != crate::model::AuthStep::Authenticated;

        if is_typing_mode {
            match key.code {
                KeyCode::Enter => Some(UserAction::ComposerSubmit),
                KeyCode::Backspace => Some(UserAction::Backspace),
                KeyCode::Tab => Some(UserAction::CycleFocus),
                KeyCode::Char(c) => Some(UserAction::InputChar(c)),
                _ => None,
            }
        } else {
            // Normal navigation mode
            match key.code {
                KeyCode::Char('q') => Some(UserAction::Quit),
                KeyCode::Tab => Some(UserAction::CycleFocus),
                KeyCode::Up | KeyCode::Char('k') => Some(UserAction::PrevItem),
                KeyCode::Down | KeyCode::Char('j') => Some(UserAction::NextItem),
                KeyCode::Enter => Some(UserAction::SelectCurrent),
                KeyCode::Char('r') => Some(UserAction::StartReply),
                KeyCode::Char('e') => Some(UserAction::StartEdit),
                KeyCode::Char('d') => Some(UserAction::DeleteSelected),
                KeyCode::Char('+') => Some(UserAction::OpenReactionPicker),
                KeyCode::Char('f') => Some(UserAction::OpenForwardModal),
                KeyCode::Char('a') => Some(UserAction::OpenAttachFile),
                KeyCode::Char('m') => Some(UserAction::OpenContextMenu),
                KeyCode::Char('/') => Some(UserAction::OpenSearch),
                KeyCode::Char('i') => Some(UserAction::OpenProfile),
                KeyCode::Char('?') => Some(UserAction::OpenSettingsHelp),
                KeyCode::Char('p') => Some(UserAction::ToggleVoicePlay),
                KeyCode::Char('1') => Some(UserAction::SelectFolder(0)),
                KeyCode::Char('2') => Some(UserAction::SelectFolder(1)),
                KeyCode::Char('3') => Some(UserAction::SelectFolder(2)),
                KeyCode::Char('4') => Some(UserAction::SelectFolder(3)),
                _ => None,
            }
        }
    }
}
