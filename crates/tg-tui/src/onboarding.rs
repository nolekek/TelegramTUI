use std::io::{self, Write};
use std::time::Duration;
use tg_core::{AuthState, CoreEvent, CoreHandle};
use tokio::sync::broadcast;

use crate::ui::qr::print_qr_code_ansi;

/// Run a reliable terminal console wizard on first run if the user is unauthenticated.
/// Once authenticated, session keys are stored in .data/tdlib_db, and subsequent runs
/// skip this wizard entirely and launch immediately into the Vim TUI.
pub async fn run_onboarding_if_needed(
    handle: &CoreHandle,
    event_rx: &mut broadcast::Receiver<CoreEvent>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Give engine 150ms to read cached session from SQLite
    tokio::time::sleep(Duration::from_millis(150)).await;

    // Drain events to get initial state
    while let Ok(event) = event_rx.try_recv() {
        if let CoreEvent::AuthStateChanged(ref auth) = event {
            if *auth == AuthState::Ready {
                return Ok(());
            }
        }
    }

    if handle.read_state().auth_state == AuthState::Ready {
        return Ok(());
    }

    println!();
    println!("╔════════════════════════════════════════════════════════════════╗");
    println!("║             TELEGRAM CLIENT — INITIAL ONBOARDING               ║");
    println!("║         Cross-platform Terminal Client (Win/Linux/macOS)       ║");
    println!("╚════════════════════════════════════════════════════════════════╝");
    println!();
    println!("Choose authentication method:");
    println!("  [1] Log in by QR Code (Recommended: Scan from mobile Telegram)");
    println!("  [2] Log in by Phone Number (SMS / Telegram in-app code)");
    println!("  [3] Open TUI directly (Interactive modal inside TUI)");
    println!("  [q] Quit");
    println!();
    print!("Select [1/2/3/q] (default 1): ");
    io::stdout().flush()?;

    let mut choice = String::new();
    io::stdin().read_line(&mut choice)?;
    let choice = choice.trim();

    if choice.eq_ignore_ascii_case("q") {
        println!("Exiting.");
        std::process::exit(0);
    } else if choice == "2" {
        login_by_phone(handle, event_rx).await?;
    } else if choice == "3" {
        println!("Proceeding directly to TUI interface...");
        return Ok(());
    } else {
        login_by_qr(handle, event_rx).await?;
    }

    // Wait until Ready
    let mut check_count = 0;
    while handle.read_state().auth_state != AuthState::Ready && check_count < 20 {
        if let Ok(event) = tokio::time::timeout(Duration::from_millis(300), event_rx.recv()).await {
            if let Ok(CoreEvent::AuthStateChanged(AuthState::Ready)) = event {
                break;
            }
        }
        check_count += 1;
    }

    println!();
    println!("Authentication verified! Launching Vim TUI interface...");
    tokio::time::sleep(Duration::from_millis(300)).await;
    Ok(())
}

async fn login_by_qr(
    handle: &CoreHandle,
    event_rx: &mut broadcast::Receiver<CoreEvent>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!();
    println!("Requesting QR code from Telegram network...");
    handle.request_qr_code().await?;

    // Wait for WaitQrCode event
    let mut qr_link = String::new();
    let start = std::time::Instant::now();

    while start.elapsed() < Duration::from_secs(10) {
        if let Ok(Ok(event)) = tokio::time::timeout(Duration::from_millis(500), event_rx.recv()).await {
            match event {
                CoreEvent::AuthStateChanged(AuthState::WaitQrCode(info)) => {
                    qr_link = info.link;
                    break;
                }
                CoreEvent::AuthStateChanged(AuthState::Ready) => {
                    return Ok(());
                }
                _ => {}
            }
        }
    }

    if qr_link.is_empty() {
        if let AuthState::WaitQrCode(info) = handle.read_state().auth_state.clone() {
            qr_link = info.link;
        }
    }

    if qr_link.is_empty() {
        println!("Could not obtain QR link. Falling back to phone login.");
        return login_by_phone(handle, event_rx).await;
    }

    println!();
    println!("┌──────────────────────────────────────────────────────────────┐");
    println!("│ 1. Open Telegram on your phone                               │");
    println!("│ 2. Go to Settings > Devices > Link Desktop Device            │");
    println!("│ 3. Point your camera at this QR code:                        │");
    println!("└──────────────────────────────────────────────────────────────┘");
    println!();

    if let Err(e) = print_qr_code_ansi(&qr_link) {
        println!("Could not display ANSI QR code ({}), raw link: {}", e, qr_link);
    }

    println!();
    println!("Waiting for scan confirmation on your mobile Telegram...");

    // Loop waiting for confirmation on phone
    while handle.read_state().auth_state != AuthState::Ready {
        if let Ok(Ok(event)) = tokio::time::timeout(Duration::from_millis(1000), event_rx.recv()).await {
            match event {
                CoreEvent::AuthStateChanged(AuthState::Ready) => {
                    println!("Scan confirmed! Logged in successfully.");
                    break;
                }
                CoreEvent::AuthStateChanged(AuthState::WaitPassword(_)) => {
                    println!();
                    println!("2FA Cloud Password required for this account.");
                    return enter_password(handle, event_rx).await;
                }
                CoreEvent::AuthStateChanged(AuthState::WaitQrCode(_)) => {
                    // Refreshed token ignored in wait loop
                }
                CoreEvent::ErrorOccurred(e) => {
                    println!("Error: {}", e);
                }
                _ => {}
            }
        }
    }

    Ok(())
}

async fn login_by_phone(
    handle: &CoreHandle,
    event_rx: &mut broadcast::Receiver<CoreEvent>,
) -> Result<(), Box<dyn std::error::Error>> {
    print!("Enter phone number (international format e.g. +1234567890 or +79991234567): ");
    io::stdout().flush()?;

    let mut phone = String::new();
    io::stdin().read_line(&mut phone)?;
    let phone = phone.trim().to_string();

    handle.send_phone_number(&phone).await?;
    println!("Sent phone number. Waiting for code...");

    // Wait for WaitCode
    let start = std::time::Instant::now();
    let mut next_state = AuthState::WaitPhoneNumber;

    while start.elapsed() < Duration::from_secs(10) {
        if let Ok(Ok(event)) = tokio::time::timeout(Duration::from_millis(500), event_rx.recv()).await {
            if let CoreEvent::AuthStateChanged(ref st) = event {
                next_state = st.clone();
                match st {
                    AuthState::WaitCode(_) | AuthState::WaitPassword(_) | AuthState::Ready => break,
                    _ => {}
                }
            }
        }
    }

    match next_state {
        AuthState::WaitCode(info) => {
            println!();
            println!("Code sent to: {} ({})", info.phone_number, info.code_type);
            print!("Enter confirmation code: ");
            io::stdout().flush()?;

            let mut code = String::new();
            io::stdin().read_line(&mut code)?;
            let code = code.trim().to_string();

            handle.send_code(&code).await?;
            println!("Sent code. Verifying...");

            // Wait for next state (Ready or 2FA)
            let start = std::time::Instant::now();
            while start.elapsed() < Duration::from_secs(10) {
                if let Ok(Ok(event)) = tokio::time::timeout(Duration::from_millis(500), event_rx.recv()).await {
                    match event {
                        CoreEvent::AuthStateChanged(AuthState::WaitPassword(_)) => {
                            return enter_password(handle, event_rx).await;
                        }
                        CoreEvent::AuthStateChanged(AuthState::Ready) => {
                            return Ok(());
                        }
                        _ => {}
                    }
                }
            }
        }
        AuthState::WaitPassword(_) => {
            return enter_password(handle, event_rx).await;
        }
        AuthState::Ready => {
            return Ok(());
        }
        _ => {}
    }

    Ok(())
}

async fn enter_password(
    handle: &CoreHandle,
    _event_rx: &mut broadcast::Receiver<CoreEvent>,
) -> Result<(), Box<dyn std::error::Error>> {
    print!("Enter Two-Step Verification 2FA Cloud Password: ");
    io::stdout().flush()?;

    let mut pwd = String::new();
    io::stdin().read_line(&mut pwd)?;
    let pwd = pwd.trim().to_string();

    handle.send_password(&pwd).await?;
    println!("Sent 2FA password.");
    Ok(())
}
