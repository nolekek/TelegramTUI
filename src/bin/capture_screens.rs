//! Screen Capture Utility for TgTui
//! Renders each screen using Ratatui TestBackend and saves reference views to replica/clone-screens/

use ratatui::{backend::TestBackend, Terminal};
use std::fs;
use std::path::Path;
use tg_tui::actors::coordinator::AppCoordinator;
use tg_tui::actors::tdlib::TdlibActor;
use tg_tui::model::{ActiveModal, AuthStep, FocusPane};
use tg_tui::theme::Theme;
use tg_tui::ui::render_app;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = Path::new("replica/clone-screens");
    fs::create_dir_all(out_dir)?;

    let width = 110;
    let height = 32;
    let theme = Theme::default();

    // 1. S01: Auth Screen
    {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend)?;
        let (tx, _rx) = mpsc::channel(1);
        let mut app = AppCoordinator::new(tx);
        app.auth_state = AuthStep::PhoneInput {
            buffer: "+1 555 019 2834".into(),
            error: None,
        };

        terminal.draw(|f| {
            render_app(f, &app, &theme);
        })?;

        let view = buffer_to_string(terminal.backend().buffer());
        fs::write(out_dir.join("S01_auth.txt"), view)?;
        println!("Captured S01_auth.txt");
    }

    // Setup active state for Shell screens
    let (tx, _rx) = mpsc::channel(1);
    let (event_tx, mut event_rx) = mpsc::channel(64);
    let tdlib = TdlibActor::new(_rx, event_tx, true);
    tokio::spawn(async move {
        tdlib.run().await;
    });

    let mut app = AppCoordinator::new(tx);
    // Allow spawned TdlibActor to emit initial data
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    while let Ok(event) = event_rx.try_recv() {
        app.handle_tdlib_event(event).await;
    }

    // 2. S02_S03_S04: Main Shell, Dialogs, History, Composer
    {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend)?;
        app.focus = FocusPane::Composer;
        app.composer_buffer = "Checking out the clean-room terminal client! **bold**".into();

        terminal.draw(|f| {
            render_app(f, &app, &theme);
        })?;

        let view = buffer_to_string(terminal.backend().buffer());
        fs::write(out_dir.join("S02_S03_S04_main_shell.txt"), view)?;
        println!("Captured S02_S03_S04_main_shell.txt");
    }

    // 3. S07: Message Context Menu
    {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend)?;
        app.active_modal = ActiveModal::ContextMenu { message_idx: 2 };

        terminal.draw(|f| {
            render_app(f, &app, &theme);
        })?;

        let view = buffer_to_string(terminal.backend().buffer());
        fs::write(out_dir.join("S07_context_menu.txt"), view)?;
        println!("Captured S07_context_menu.txt");
    }

    // 4. S05: Chat Profile & Shared Media
    {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend)?;
        app.active_modal = ActiveModal::ChatProfile;

        terminal.draw(|f| {
            render_app(f, &app, &theme);
        })?;

        let view = buffer_to_string(terminal.backend().buffer());
        fs::write(out_dir.join("S05_chat_profile.txt"), view)?;
        println!("Captured S05_chat_profile.txt");
    }

    // 5. S10: Search Modal
    {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend)?;
        app.active_modal = ActiveModal::Search {
            query: "Ratatui".into(),
        };

        terminal.draw(|f| {
            render_app(f, &app, &theme);
        })?;

        let view = buffer_to_string(terminal.backend().buffer());
        fs::write(out_dir.join("S10_search.txt"), view)?;
        println!("Captured S10_search.txt");
    }

    // 6. S12: Settings & Cheatsheet
    {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend)?;
        app.active_modal = ActiveModal::SettingsHelp;

        terminal.draw(|f| {
            render_app(f, &app, &theme);
        })?;

        let view = buffer_to_string(terminal.backend().buffer());
        fs::write(out_dir.join("S12_settings_cheatsheet.txt"), view)?;
        println!("Captured S12_settings_cheatsheet.txt");
    }

    // 7. S08: Lightbox Media Viewer
    {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend)?;
        app.active_modal = ActiveModal::Lightbox {
            title: "architecture_diagram.png".into(),
            dimensions: "1920x1080".into(),
            zoom_percent: 100,
        };

        terminal.draw(|f| {
            render_app(f, &app, &theme);
        })?;

        let view = buffer_to_string(terminal.backend().buffer());
        fs::write(out_dir.join("S08_lightbox.txt"), view)?;
        println!("Captured S08_lightbox.txt");
    }

    // 8. S11: Forward Message Modal
    {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend)?;
        app.active_modal = ActiveModal::ForwardModal {
            message_idx: 2,
            selected_chat_idx: 1,
            hide_author: true,
        };

        terminal.draw(|f| {
            render_app(f, &app, &theme);
        })?;

        let view = buffer_to_string(terminal.backend().buffer());
        fs::write(out_dir.join("S11_forward.txt"), view)?;
        println!("Captured S11_forward.txt");
    }

    // 9. S07-R: Reaction Picker
    {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend)?;
        app.active_modal = ActiveModal::ReactionPicker {
            message_idx: 2,
        };

        terminal.draw(|f| {
            render_app(f, &app, &theme);
        })?;

        let view = buffer_to_string(terminal.backend().buffer());
        fs::write(out_dir.join("S07_reaction_picker.txt"), view)?;
        println!("Captured S07_reaction_picker.txt");
    }

    // 10. S04-A: Attachment Modal
    {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend)?;
        app.active_modal = ActiveModal::AttachFile {
            path_buffer: "C:/Screenshots/diagram.png".into(),
            is_photo: true,
            compress: true,
        };

        terminal.draw(|f| {
            render_app(f, &app, &theme);
        })?;

        let view = buffer_to_string(terminal.backend().buffer());
        fs::write(out_dir.join("S04_attach_file.txt"), view)?;
        println!("Captured S04_attach_file.txt");
    }

    Ok(())
}

fn buffer_to_string(buffer: &ratatui::buffer::Buffer) -> String {
    let mut out = String::new();
    let area = buffer.area;

    for y in 0..area.height {
        for x in 0..area.width {
            let cell = buffer.cell((x, y)).unwrap();
            out.push_str(cell.symbol());
        }
        out.push('\n');
    }
    out
}
