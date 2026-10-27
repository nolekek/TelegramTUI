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

    // Initial state: starts authenticated with rich seed data for immediate usability;
    // user can toggle to Auth Demo (S01) at any time by pressing F1.
    let tdlib_actor = TdlibActor::new(tdlib_cmd_rx, tdlib_event_tx, true);
    tokio::spawn(async move {
        tdlib_actor.run().await;
    });

    let app = AppCoordinator::new(tdlib_cmd_tx);
    let theme = Theme::default();
    let ui_actor = UiActor::new(app, theme, tdlib_event_rx);

    ui_actor.run().await?;

    Ok(())
}
