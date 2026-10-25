pub mod app;
pub mod audio;
pub mod cli;
pub mod event;
pub mod keys;
pub mod onboarding;
pub mod text;
pub mod ui;

use std::io::stdout;
use std::time::Duration;
use crossterm::{
    event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use tg_core::{AuthState, Config, Engine};

use crate::app::{ActivePanel, App, CurrentScreen, NavigationMode, VimMode};
use crate::event::{AppEvent, EventHandler};
use crate::keys::normalize_command_char;
use crate::ui::chat_list::available_tabs;
use crate::ui::modals::{ActiveModal, ProxyModalState, ProxyTypeChoice};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = cli::CliArgs::parse();
    let (mut config, _config_path) = Config::load_or_create_auto(
        args.portable,
        args.config_path.as_deref(),
        args.data_dir.as_deref(),
    )?;

    if args.mock {
        config.use_mock = true;
    }
    if let Some(ref d_dir) = args.downloads_dir {
        config.downloads_dir = Some(d_dir.clone());
    }

    // Clean orphaned temporary media cache on startup
    let _ = config.clean_media_cache();

    let (handle, event_rx) = Engine::start(config.clone()).await?;

    let mut app = App::with_config(handle, config.clone());
    app.proxy_modal_state = ProxyModalState::from_config(config.proxy.as_ref());
    app.sync_message_selection_to_newest();

    // Setup full-screen terminal raw mode immediately (seamless startup)
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Unified event loop with async core channel & crossterm stream
    let mut event_handler = EventHandler::new(event_rx, Duration::from_millis(50));

    while !app.should_quit {
        terminal.draw(|f| ui::render(f, &mut app))?;

        if let Some(event) = event_handler.next().await {
            match event {
                AppEvent::Terminal(crossterm::event::Event::Key(key)) => {
                    if key.kind == KeyEventKind::Press {
                        handle_key_event(&mut app, key).await;
                    }
                }
                AppEvent::Terminal(crossterm::event::Event::Resize(_, _)) => {
                    // Terminal re-renders automatically
                }
                AppEvent::Core(core_event) => {
                    app.handle_core_event(core_event);
                    app.ensure_active_chat_history().await;
                }
                AppEvent::Tick => {
                    app.ensure_active_chat_history().await;
                }
                _ => {}
            }
        } else {
            break;
        }
    }

    // 4. Clean restoration of terminal screen and cursor (and purge session media cache)
    app.cleanup_cache();
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}

async fn handle_key_event(app: &mut App, key: KeyEvent) {
    let state = app.handle.clone_state();

    // 1. In-TUI Authentication Dialogs (Fallback if session expires or initial setup in TUI)
    if state.auth_state != AuthState::Ready {
        match &state.auth_state {
            AuthState::WaitPhoneNumber => match key.code {
                KeyCode::Esc | KeyCode::Tab => {
                    // Esc/Tab switches to QR login instead of quitting the program!
                    let _ = app.handle.request_qr_code().await;
                    app.status_message = "Переключение на вход по QR-коду...".to_string();
                    app.is_status_error = false;
                }
                KeyCode::F(3) => {
                    app.active_modal = ActiveModal::Proxy;
                }
                KeyCode::Enter => {
                    app.handle_submit().await;
                }
                KeyCode::Backspace => {
                    app.auth_modal_state.input_buffer.pop();
                    if app.auth_modal_state.input_buffer == "+" {
                        app.auth_modal_state.input_buffer.clear();
                    }
                }
                KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Char('й') | KeyCode::Char('Й')
                    if app.auth_modal_state.input_buffer.is_empty() =>
                {
                    app.should_quit = true;
                }
                KeyCode::Char(c) => {
                    if c.is_ascii_digit() {
                        if app.auth_modal_state.input_buffer.is_empty() {
                            app.auth_modal_state.input_buffer.push('+');
                        }
                        app.auth_modal_state.input_buffer.push(c);
                        app.is_status_error = false;
                    } else if c == '+' && app.auth_modal_state.input_buffer.is_empty() {
                        app.auth_modal_state.input_buffer.push('+');
                        app.is_status_error = false;
                    }
                }
                _ => {}
            },
            AuthState::WaitCode(_) => match key.code {
                KeyCode::Esc => {
                    // Esc returns to Phone Number screen instead of quitting!
                    let _ = app.handle.send_phone_number("").await;
                    app.status_message = "Возврат на экран ввода номера телефона.".to_string();
                    app.is_status_error = false;
                }
                KeyCode::Char('p') | KeyCode::Char('P') | KeyCode::Char('з') | KeyCode::Char('З')
                    if app.auth_modal_state.input_buffer.is_empty() =>
                {
                    let _ = app.handle.send_phone_number("").await;
                    app.status_message = "Возврат на экран ввода номера телефона.".to_string();
                    app.is_status_error = false;
                }
                KeyCode::Enter => {
                    app.handle_submit().await;
                }
                KeyCode::Backspace => {
                    app.auth_modal_state.input_buffer.pop();
                }
                KeyCode::Char(c) if c.is_ascii_digit() => {
                    app.auth_modal_state.input_buffer.push(c);
                    app.is_status_error = false;
                }
                _ => {}
            },
            AuthState::WaitPassword(_) => {
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && (key.code == KeyCode::Char('v') || key.code == KeyCode::Char('V') || key.code == KeyCode::Char('м') || key.code == KeyCode::Char('М'))
                {
                    app.auth_modal_state.show_password_cleartext =
                        !app.auth_modal_state.show_password_cleartext;
                    return;
                }
                match key.code {
                    KeyCode::Esc => {
                        let _ = app.handle.send_phone_number("").await;
                        app.status_message = "Возврат на экран ввода номера телефона.".to_string();
                    }
                    KeyCode::Enter => {
                        app.handle_submit().await;
                    }
                    KeyCode::Backspace => {
                        app.auth_modal_state.input_buffer.pop();
                    }
                    KeyCode::Char(c) => {
                        app.auth_modal_state.input_buffer.push(c);
                    }
                    _ => {}
                }
            }
            AuthState::WaitQrCode(_) => match key.code {
                KeyCode::Esc | KeyCode::Tab | KeyCode::Enter => {
                    let _ = app.handle.send_phone_number("").await;
                    app.status_message = "Переключение на вход по номеру телефона...".to_string();
                }
                KeyCode::Char('p') | KeyCode::Char('P') | KeyCode::Char('з') | KeyCode::Char('З') => {
                    let _ = app.handle.send_phone_number("").await;
                    app.status_message = "Переключение на вход по номеру телефона...".to_string();
                }
                KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Char('й') | KeyCode::Char('Й') => {
                    app.should_quit = true;
                }
                KeyCode::F(3) => {
                    app.active_modal = ActiveModal::Proxy;
                }
                _ => {}
            },
            _ => {
                if key.code == KeyCode::Char('q') || key.code == KeyCode::Char('й') {
                    app.should_quit = true;
                }
            }
        }
        return;
    }

    // 1.5 Image Viewer Modal (Full Screen Photo View)
    if app.active_modal == ActiveModal::ImageViewer {
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && (key.code == KeyCode::Char('s') || key.code == KeyCode::Char('S') || key.code == KeyCode::Char('ы') || key.code == KeyCode::Char('Ы'))
        {
            app.save_selected_media_permanently().await;
            return;
        }
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Char('й') | KeyCode::Char('Й') => {
                app.active_modal = ActiveModal::None;
                app.image_viewer_state = None;
            }
            KeyCode::Char('o') | KeyCode::Char('O') | KeyCode::Char('щ') | KeyCode::Char('Щ') => {
                if let Some(ref st) = app.image_viewer_state {
                    if let Err(e) = st.open_desktop() {
                        app.status_message = format!("Ошибка открытия: {}", e);
                        app.is_status_error = true;
                    } else {
                        app.status_message = "Фото открыто в стандартном приложении ОС.".to_string();
                    }
                }
            }
            KeyCode::Char('s') | KeyCode::Char('S') | KeyCode::Char('ы') | KeyCode::Char('Ы') => {
                app.save_selected_media_permanently().await;
            }
            _ => {}
        }
        return;
    }

    // 1.6 Help Modal (F1 / ?)
    if app.active_modal == ActiveModal::Help {
        match key.code {
            KeyCode::Esc
            | KeyCode::Enter
            | KeyCode::F(1)
            | KeyCode::Char('q')
            | KeyCode::Char('Q')
            | KeyCode::Char('й')
            | KeyCode::Char('Й')
            | KeyCode::Char('?') => {
                app.active_modal = ActiveModal::None;
            }
            _ => {}
        }
        return;
    }

    // 2. Settings Modal with Interactive Selection
    if app.active_modal == ActiveModal::Settings {
        match key.code {
            KeyCode::Esc => {
                app.settings_modal_state.confirm_logout = false;
                app.active_modal = ActiveModal::None;
            }
            KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('л') => {
                app.settings_modal_state.confirm_logout = false;
                if app.settings_modal_state.selected_index > 0 {
                    app.settings_modal_state.selected_index -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('о') => {
                app.settings_modal_state.confirm_logout = false;
                if app.settings_modal_state.selected_index < 4 {
                    app.settings_modal_state.selected_index += 1;
                }
            }
            KeyCode::Enter | KeyCode::Char(' ') => match app.settings_modal_state.selected_index {
                0 => {
                    app.toggle_navigation_mode();
                }
                1 => {
                    app.settings_modal_state.show_last_message_preview =
                        !app.settings_modal_state.show_last_message_preview;
                }
                2 => {
                    app.settings_modal_state.notifications_enabled =
                        !app.settings_modal_state.notifications_enabled;
                }
                3 => {
                    app.active_modal = ActiveModal::Proxy;
                }
                4 => {
                    if app.settings_modal_state.confirm_logout {
                        let _ = app.handle.log_out().await;
                        app.settings_modal_state.confirm_logout = false;
                        app.active_modal = ActiveModal::None;
                    } else {
                        app.settings_modal_state.confirm_logout = true;
                    }
                }
                _ => {}
            },
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Char('н') | KeyCode::Char('Н') => {
                if app.settings_modal_state.confirm_logout {
                    let _ = app.handle.log_out().await;
                    app.settings_modal_state.confirm_logout = false;
                    app.active_modal = ActiveModal::None;
                }
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Char('т') | KeyCode::Char('Т') => {
                app.settings_modal_state.confirm_logout = false;
            }
            KeyCode::Char('l') | KeyCode::Char('L') | KeyCode::Char('д') | KeyCode::Char('Д') => {
                if app.settings_modal_state.confirm_logout {
                    let _ = app.handle.log_out().await;
                    app.settings_modal_state.confirm_logout = false;
                    app.active_modal = ActiveModal::None;
                } else {
                    app.settings_modal_state.confirm_logout = true;
                }
            }
            _ => {}
        }
        return;
    }

    // 3. Proxy Modal
    if app.active_modal == ActiveModal::Proxy {
        match key.code {
            KeyCode::Esc => app.active_modal = ActiveModal::None,
            KeyCode::Tab | KeyCode::Down => {
                let max_fields = if app.proxy_modal_state.proxy_type == ProxyTypeChoice::Mtproto {
                    7
                } else {
                    6
                };
                app.proxy_modal_state.selected_field =
                    (app.proxy_modal_state.selected_field + 1) % max_fields;
            }
            KeyCode::BackTab | KeyCode::Up => {
                let max_fields = if app.proxy_modal_state.proxy_type == ProxyTypeChoice::Mtproto {
                    7
                } else {
                    6
                };
                app.proxy_modal_state.selected_field = if app.proxy_modal_state.selected_field == 0 {
                    max_fields - 1
                } else {
                    app.proxy_modal_state.selected_field - 1
                };
            }
            KeyCode::Char(' ') => match app.proxy_modal_state.selected_field {
                0 => {
                    app.proxy_modal_state.is_enabled = !app.proxy_modal_state.is_enabled;
                }
                1 => {
                    app.proxy_modal_state.proxy_type = match app.proxy_modal_state.proxy_type {
                        ProxyTypeChoice::Socks5 => ProxyTypeChoice::Http,
                        ProxyTypeChoice::Http => ProxyTypeChoice::Mtproto,
                        ProxyTypeChoice::Mtproto => ProxyTypeChoice::Socks5,
                    };
                }
                _ => {}
            },
            KeyCode::Char('s') | KeyCode::Char('S') | KeyCode::Char('ы') | KeyCode::Char('Ы') => {
                let cfg = app.proxy_modal_state.to_proxy_config();
                if let Err(e) = app.handle.set_proxy(cfg).await {
                    app.status_message = format!("Failed to apply proxy: {}", e);
                    app.is_status_error = true;
                } else {
                    app.status_message = "Proxy settings saved & applied".to_string();
                    app.is_status_error = false;
                }
                app.active_modal = ActiveModal::None;
            }
            KeyCode::Char('d') | KeyCode::Char('D') | KeyCode::Char('в') | KeyCode::Char('В') => {
                app.proxy_modal_state.is_enabled = false;
                if let Err(e) = app.handle.set_proxy(None).await {
                    app.status_message = format!("Failed to disable proxy: {}", e);
                    app.is_status_error = true;
                } else {
                    app.status_message = "Proxy disabled".to_string();
                    app.is_status_error = false;
                }
                app.active_modal = ActiveModal::None;
            }
            KeyCode::Backspace => match app.proxy_modal_state.selected_field {
                2 => {
                    app.proxy_modal_state.server.pop();
                }
                3 => {
                    app.proxy_modal_state.port.pop();
                }
                4 => {
                    app.proxy_modal_state.username.pop();
                }
                5 => {
                    app.proxy_modal_state.password.pop();
                }
                6 => {
                    app.proxy_modal_state.secret.pop();
                }
                _ => {}
            },
            KeyCode::Char(c) => match app.proxy_modal_state.selected_field {
                2 => app.proxy_modal_state.server.push(c),
                3 => {
                    if c.is_ascii_digit() {
                        app.proxy_modal_state.port.push(c);
                    }
                }
                4 => app.proxy_modal_state.username.push(c),
                5 => app.proxy_modal_state.password.push(c),
                6 => app.proxy_modal_state.secret.push(c),
                _ => {}
            },
            _ => {}
        }
        return;
    }

    // 4. Global Hard Interruption, Search & Mode Toggles
    if key.modifiers.contains(KeyModifiers::CONTROL)
        && (key.code == KeyCode::Char('c') || key.code == KeyCode::Char('C') || key.code == KeyCode::Char('с') || key.code == KeyCode::Char('С'))
    {
        app.should_quit = true;
        return;
    }

    // Global quick search invocation (Ctrl+F) across all screens and modes
    if key.modifiers.contains(KeyModifiers::CONTROL)
        && (key.code == KeyCode::Char('f') || key.code == KeyCode::Char('F') || key.code == KeyCode::Char('а') || key.code == KeyCode::Char('А'))
    {
        app.exit_to_chat_search();
        return;
    }

    match key.code {
        KeyCode::F(1) => {
            app.active_modal = if app.active_modal == ActiveModal::Help {
                ActiveModal::None
            } else {
                ActiveModal::Help
            };
            return;
        }
        KeyCode::F(2) => {
            app.active_modal = ActiveModal::Settings;
            return;
        }
        KeyCode::F(3) => {
            app.active_modal = ActiveModal::Proxy;
            return;
        }
        KeyCode::F(4) => {
            app.toggle_navigation_mode();
            return;
        }
        _ => {}
    }

    // 5. Full-Screen Chat Search Screen
    if app.current_screen == CurrentScreen::ChatSearch {
        match key.code {
            KeyCode::Enter => {
                app.open_selected_chat();
                app.ensure_active_chat_history().await;
                return;
            }
            KeyCode::Down | KeyCode::Tab => {
                app.next_chat();
                return;
            }
            KeyCode::Up | KeyCode::BackTab => {
                app.previous_chat();
                return;
            }
            KeyCode::Char('j') | KeyCode::Char('о') | KeyCode::Char('n') | KeyCode::Char('т')
                if key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                app.next_chat();
                return;
            }
            KeyCode::Char('k') | KeyCode::Char('л') | KeyCode::Char('p') | KeyCode::Char('з')
                if key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                app.previous_chat();
                return;
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                app.search_query.clear();
                app.selected_chat_index = 0;
                app.chat_list_state.select(Some(0));
                return;
            }
            KeyCode::Char('[') if key.modifiers.contains(KeyModifiers::ALT) || app.search_query.is_empty() => {
                app.prev_tab();
                return;
            }
            KeyCode::Char(']') if key.modifiers.contains(KeyModifiers::ALT) || app.search_query.is_empty() => {
                app.next_tab();
                return;
            }
            KeyCode::Char('х') | KeyCode::Char('Х') if key.modifiers.contains(KeyModifiers::ALT) => {
                app.prev_tab();
                return;
            }
            KeyCode::Char('ъ') | KeyCode::Char('Ъ') if key.modifiers.contains(KeyModifiers::ALT) => {
                app.next_tab();
                return;
            }
            KeyCode::Char(c @ '1'..='9') if app.search_query.is_empty() => {
                let idx = (c as usize) - ('1' as usize);
                let tab = {
                    let s = app.handle.read_state();
                    let tabs = available_tabs(&s);
                    tabs.get(idx).cloned()
                };
                if let Some(t) = tab {
                    app.switch_tab(t);
                }
                return;
            }
            KeyCode::Backspace => {
                app.search_query.pop();
                app.selected_chat_index = 0;
                app.chat_list_state.select(Some(0));
                return;
            }
            KeyCode::Esc => {
                if !app.search_query.is_empty() {
                    app.search_query.clear();
                    app.selected_chat_index = 0;
                    app.chat_list_state.select(Some(0));
                } else if app.active_chat_id().is_some() {
                    app.current_screen = CurrentScreen::ChatView;
                    app.active_panel = ActivePanel::Messages;
                    app.vim_mode = VimMode::Normal;
                } else {
                    app.should_quit = true;
                }
                return;
            }
            KeyCode::Char(c) => {
                app.search_query.push(c);
                app.selected_chat_index = 0;
                app.chat_list_state.select(Some(0));
                return;
            }
            _ => {}
        }
        return;
    }

    // 6. Full-Screen Chat View: Classic Mode Handler (Direct typing & arrow navigation)
    if app.navigation_mode == NavigationMode::Classic {
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && (key.code == KeyCode::Char('s') || key.code == KeyCode::Char('S') || key.code == KeyCode::Char('ы') || key.code == KeyCode::Char('Ы'))
        {
            app.save_selected_media_permanently().await;
            return;
        }
        match key.code {
            KeyCode::Esc => {
                app.exit_to_chat_search();
            }
            KeyCode::Enter => {
                if !app.input_text.trim().is_empty() {
                    app.handle_submit().await;
                } else if let Some(msg) = app.get_selected_message() {
                    if msg.content.file_id().is_some() {
                        app.trigger_selected_media_action().await;
                    }
                }
            }
            KeyCode::Up => {
                app.previous_message();
                if app.selected_message_index <= 5 {
                    app.fetch_older_history().await;
                }
            }
            KeyCode::Down => {
                app.next_message();
            }
            KeyCode::PageUp => {
                app.scroll_messages_up(10);
                app.fetch_older_history().await;
            }
            KeyCode::PageDown => {
                app.scroll_messages_down(10);
            }
            KeyCode::Backspace => {
                app.delete_char_before_cursor();
            }
            KeyCode::Delete => {
                app.delete_char_at_cursor();
            }
            KeyCode::Left => {
                app.move_cursor_left();
            }
            KeyCode::Right => {
                app.move_cursor_right();
            }
            KeyCode::Home => {
                app.move_cursor_home();
            }
            KeyCode::End => {
                app.move_cursor_end();
            }
            KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                app.delete_word_backward();
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                app.clear_input();
            }
            KeyCode::Char(c) => {
                app.insert_char(c);
            }
            _ => {}
        }
        return;
    }

    // 7. Full-Screen Chat View: Command Mode
    if app.vim_mode == VimMode::Command {
        match key.code {
            KeyCode::Esc => {
                app.vim_mode = VimMode::Normal;
                app.cmd_buffer.clear();
            }
            KeyCode::Backspace => {
                app.cmd_buffer.pop();
                if app.cmd_buffer.is_empty() {
                    app.vim_mode = VimMode::Normal;
                }
            }
            KeyCode::Enter => {
                let cmd = app.cmd_buffer.trim().to_lowercase();
                match cmd.as_str() {
                    "q" | "quit" | "qa" | "q!" => app.should_quit = true,
                    "b" | "buffer" | "find" | "search" => app.exit_to_chat_search(),
                    "w" | "settings" => app.active_modal = ActiveModal::Settings,
                    "proxy" => app.active_modal = ActiveModal::Proxy,
                    _ => app.status_message = format!("Неизвестная команда: :{}", cmd),
                }
                app.cmd_buffer.clear();
                app.vim_mode = VimMode::Normal;
            }
            KeyCode::Char(c) => {
                app.cmd_buffer.push(c);
            }
            _ => {}
        }
        return;
    }

    // 7. Full-Screen Chat View: Insert Mode
    if app.vim_mode == VimMode::Insert {
        match key.code {
            KeyCode::Esc => {
                app.vim_mode = VimMode::Normal;
                app.active_panel = ActivePanel::Messages;
                app.status_message = "-- NORMAL -- [i] Написать  [Esc] Поиск чатов  [j/k] Скролл".to_string();
            }
            KeyCode::Enter => {
                app.handle_submit().await;
                app.vim_mode = VimMode::Normal;
                app.active_panel = ActivePanel::Messages;
            }
            KeyCode::Backspace => {
                app.delete_char_before_cursor();
            }
            KeyCode::Delete => {
                app.delete_char_at_cursor();
            }
            KeyCode::Left => {
                app.move_cursor_left();
            }
            KeyCode::Right => {
                app.move_cursor_right();
            }
            KeyCode::Home => {
                app.move_cursor_home();
            }
            KeyCode::End => {
                app.move_cursor_end();
            }
            KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                app.delete_word_backward();
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                app.clear_input();
            }
            KeyCode::Char(c) => {
                app.insert_char(c);
            }
            _ => {}
        }
        return;
    }

    // 8. Full-Screen Chat View: Normal Mode
    // Global Ctrl+S to permanently save selected message media to downloads directory
    if key.modifiers.contains(KeyModifiers::CONTROL)
        && (key.code == KeyCode::Char('s') || key.code == KeyCode::Char('S') || key.code == KeyCode::Char('ы') || key.code == KeyCode::Char('Ы'))
    {
        app.save_selected_media_permanently().await;
        return;
    }

    // Normalize character commands (Russian layout -> Latin)
    let norm_code = match key.code {
        KeyCode::Char(c) => KeyCode::Char(normalize_command_char(c)),
        other => other,
    };

    // Multi-key sequences
    if app.cmd_buffer == "d" {
        app.cmd_buffer.clear();
        if norm_code == KeyCode::Char('d') {
            app.delete_selected_message().await;
        }
        return;
    } else if app.cmd_buffer == "g" {
        app.cmd_buffer.clear();
        if norm_code == KeyCode::Char('g') {
            app.jump_to_oldest_and_fetch().await;
        }
        return;
    }

    match norm_code {
        // Toggle Help modal
        KeyCode::F(1) | KeyCode::Char('?') => {
            app.active_modal = if app.active_modal == ActiveModal::Help {
                ActiveModal::None
            } else {
                ActiveModal::Help
            };
        }

        // Return to Chat Search / Picker screen (/ or Esc)
        KeyCode::Esc | KeyCode::Char('/') => {
            app.exit_to_chat_search();
        }

        // Stop audio if playing, otherwise return to chat search
        KeyCode::Char('s') => {
            if app.audio_player.is_playing() {
                app.stop_audio();
            } else {
                app.exit_to_chat_search();
            }
        }

        // Enter Command Mode (:q, :b, :find, :w, etc.)
        KeyCode::Char(':') => {
            app.vim_mode = VimMode::Command;
            app.cmd_buffer.clear();
        }

        // Open/play media if message has media, otherwise enter Insert mode
        KeyCode::Enter | KeyCode::Char('o') => {
            if let Some(msg) = app.get_selected_message() {
                if msg.content.file_id().is_some() {
                    app.trigger_selected_media_action().await;
                    return;
                }
            }
            app.vim_mode = VimMode::Insert;
            app.active_panel = ActivePanel::Input;
            app.status_message = "-- INSERT -- (Enter отправить, Esc выйти)".to_string();
        }

        // Enter Insert Mode to compose message
        KeyCode::Char('i') | KeyCode::Char('a') => {
            app.vim_mode = VimMode::Insert;
            app.active_panel = ActivePanel::Input;
            app.status_message = "-- INSERT -- (Enter отправить, Esc выйти)".to_string();
        }

        // Reply to selected message
        KeyCode::Char('r') => {
            app.start_reply();
            app.vim_mode = VimMode::Insert;
            app.active_panel = ActivePanel::Input;
        }

        // Edit selected message
        KeyCode::Char('e') => {
            app.start_edit();
            app.vim_mode = VimMode::Insert;
            app.active_panel = ActivePanel::Input;
        }

        // Ctrl+d / Ctrl+u scrolling
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.scroll_messages_down(10);
        }
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.scroll_messages_up(10);
            app.fetch_older_history().await;
        }

        // Command buffer prefixes
        KeyCode::Char('d') => {
            app.cmd_buffer = "d".to_string();
            app.status_message = "Нажмите 'd' ещё раз для удаления сообщения, или 'D' / Ctrl+S для сохранения на диск".to_string();
        }
        KeyCode::Char('D') => {
            app.save_selected_media_permanently().await;
        }
        KeyCode::Char('g') => {
            app.cmd_buffer = "g".to_string();
        }

        // Jump to newest message
        KeyCode::Char('G') | KeyCode::End => {
            app.jump_to_newest_message();
        }

        // Navigation through messages
        KeyCode::Char('j') | KeyCode::Down => {
            app.next_message();
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.previous_message();
            if app.selected_message_index <= 5 {
                app.fetch_older_history().await;
            }
        }

        // Page scrolling
        KeyCode::PageUp => {
            app.scroll_messages_up(10);
            app.fetch_older_history().await;
        }
        KeyCode::PageDown => {
            app.scroll_messages_down(10);
        }

        // Return to chat search or quit
        KeyCode::Char('q') => {
            app.exit_to_chat_search();
        }

        // Modals & quit shortcuts
        KeyCode::F(2) => app.active_modal = ActiveModal::Settings,
        KeyCode::F(3) => app.active_modal = ActiveModal::Proxy,

        _ => {}
    }
}
