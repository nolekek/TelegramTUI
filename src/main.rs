//! TgTui — High-performance Telegram Terminal Client (TUI)
//! Clean-room rebuild of Telegram Desktop core messaging architecture.

use tg_tui::actors::coordinator::AppCoordinator;
use tg_tui::actors::tdlib::TdlibActor;
use tg_tui::actors::ui::UiActor;
use tg_tui::config::AppConfig;
use tg_tui::theme::Theme;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Install global panic hook to guarantee terminal state restoration on any crash
    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = crossterm::terminal::disable_raw_mode();
        let mut stdout = std::io::stdout();
        let _ = crossterm::execute!(
            stdout,
            crossterm::terminal::LeaveAlternateScreen,
            crossterm::cursor::Show
        );
        default_panic(info);
    }));

    // Load and validate environment / config.toml
    let config = AppConfig::load();
    if let Err(validation_errors) = config.validate() {
        for err in validation_errors {
            eprintln!("[Config Warning]: {}", err);
        }
    }

    // Channels for Actor communication
    let (tdlib_cmd_tx, tdlib_cmd_rx) = mpsc::channel(64);
    let (tdlib_event_tx, tdlib_event_rx) = mpsc::channel(64);

    // Session persistence: if session.json exists, start authenticated;
    // otherwise, start cleanly at the Authentication Gate (S01: Phone -> SMS -> 2FA).
    let session_saved = std::path::Path::new("session.json").exists();
    let tdlib_actor = TdlibActor::new(tdlib_cmd_rx, tdlib_event_tx, session_saved);
    tokio::spawn(async move {
        tdlib_actor.run().await;
    });

    let app = AppCoordinator::new(tdlib_cmd_tx);
    let theme = Theme::default();
    let ui_actor = UiActor::new(app, theme, tdlib_event_rx);

    ui_actor.run().await?;

    Ok(())
}
