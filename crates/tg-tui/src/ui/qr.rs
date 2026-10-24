use qrcode::{Color as QrColor, EcLevel, QrCode};
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

/// Generate terminal lines using Unicode half-blocks (`▀` and `▄`) to render a QR code.
/// Packs 2 vertical QR modules into 1 character cell to correct for ~1:2 terminal cell aspect ratio.
/// Uses standard inverted color scheme (Black modules on White background) for mobile camera scannability.
pub fn generate_qr_halfblock_lines(link: &str) -> Result<Vec<Line<'static>>, String> {
    let code = QrCode::with_error_correction_level(link.as_bytes(), EcLevel::L)
        .or_else(|_| QrCode::new(link.as_bytes()))
        .map_err(|e| format!("Failed to generate QR: {}", e))?;
    let width = code.width();
    let colors = code.to_colors();

    // Quiet zone margin: 2 modules of white margin on each border
    let margin = 2;
    let full_width = width + margin * 2;
    let full_height = width + margin * 2;

    let get_color = |x: usize, y: usize| -> QrColor {
        if x < margin || x >= margin + width || y < margin || y >= margin + width {
            QrColor::Light
        } else {
            colors[(y - margin) * width + (x - margin)]
        }
    };

    let mut lines = Vec::new();

    // Step by 2 vertical rows per character cell
    for y in (0..full_height).step_by(2) {
        let mut spans = Vec::new();
        for x in 0..full_width {
            let top_is_dark = get_color(x, y) == QrColor::Dark;
            let bot_is_dark = if y + 1 < full_height {
                get_color(x, y + 1) == QrColor::Dark
            } else {
                false
            };

            // Using Unicode half-blocks:
            // '▀' paints upper half in fg, lower half in bg
            // '▄' paints lower half in fg, upper half in bg
            let (ch, fg, bg) = match (top_is_dark, bot_is_dark) {
                (true, true) => ('█', Color::Black, Color::Black),
                (true, false) => ('▀', Color::Black, Color::White),
                (false, true) => ('▄', Color::Black, Color::White),
                (false, false) => ('█', Color::White, Color::White),
            };

            spans.push(Span::styled(ch.to_string(), Style::default().fg(fg).bg(bg)));
        }
        lines.push(Line::from(spans));
    }

    Ok(lines)
}

/// Print a QR code directly to standard console stdout using explicit ANSI escape codes.
/// Guarantees that black and white modules are rendered with full contrast on any terminal.
pub fn print_qr_code_ansi(link: &str) -> Result<(), String> {
    let code = QrCode::with_error_correction_level(link.as_bytes(), EcLevel::L)
        .or_else(|_| QrCode::new(link.as_bytes()))
        .map_err(|e| format!("Failed to generate QR: {}", e))?;
    let width = code.width();
    let colors = code.to_colors();

    let margin = 2;
    let full_width = width + margin * 2;
    let full_height = width + margin * 2;

    let get_color = |x: usize, y: usize| -> QrColor {
        if x < margin || x >= margin + width || y < margin || y >= margin + width {
            QrColor::Light
        } else {
            colors[(y - margin) * width + (x - margin)]
        }
    };

    println!();
    for y in (0..full_height).step_by(2) {
        let mut row = String::from("   ");
        for x in 0..full_width {
            let top_is_dark = get_color(x, y) == QrColor::Dark;
            let bot_is_dark = if y + 1 < full_height {
                get_color(x, y + 1) == QrColor::Dark
            } else {
                false
            };

            let cell = match (top_is_dark, bot_is_dark) {
                (true, true) => "\x1b[30;40m█\x1b[0m",
                (true, false) => "\x1b[30;47m▀\x1b[0m",
                (false, true) => "\x1b[30;47m▄\x1b[0m",
                (false, false) => "\x1b[37;47m█\x1b[0m",
            };
            row.push_str(cell);
        }
        println!("{}", row);
    }
    println!();
    Ok(())
}

/// Render a QR code inside a Ratatui frame area with instructions.
pub fn render_qr_widget(frame: &mut Frame, link: &str, area: Rect) {
    let buf_area = frame.area();
    let render_area = area.intersection(buf_area);
    if render_area.is_empty() || render_area.width == 0 || render_area.height == 0 {
        return;
    }

    let target_x = render_area.x;
    let target_y = render_area.y;
    if target_x >= buf_area.x + buf_area.width || target_y >= buf_area.y + buf_area.height {
        return;
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Log in via QR Code ")
        .title_alignment(Alignment::Center)
        .style(Style::default().fg(Color::Cyan));

    match generate_qr_halfblock_lines(link) {
        Ok(mut qr_lines) => {
            // Add helpful instruction lines underneath the QR code
            qr_lines.push(Line::default());
            qr_lines.push(Line::from(vec![
                Span::styled("1. ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw("Open Telegram on your mobile device"),
            ]));
            qr_lines.push(Line::from(vec![
                Span::styled("2. ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw("Go to Settings > Devices > Link Desktop Device"),
            ]));
            qr_lines.push(Line::from(vec![
                Span::styled("3. ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw("Scan this screen with your camera"),
            ]));
            qr_lines.push(Line::default());
            qr_lines.push(Line::from(vec![
                Span::styled("[P] ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::raw("Log in with phone number instead  |  "),
                Span::styled("[Esc] ", Style::default().fg(Color::DarkGray)),
                Span::raw("Cancel"),
            ]));

            let paragraph = Paragraph::new(qr_lines)
                .block(block)
                .alignment(Alignment::Center);

            frame.render_widget(paragraph, render_area);
        }
        Err(err) => {
            let error_line = Line::from(vec![
                Span::styled("Error: ", Style::default().fg(Color::Red)),
                Span::raw(err),
            ]);
            let paragraph = Paragraph::new(vec![error_line])
                .block(block)
                .alignment(Alignment::Center);
            frame.render_widget(paragraph, render_area);
        }
    }
}
