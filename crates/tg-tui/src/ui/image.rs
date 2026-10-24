use std::collections::HashMap;
use std::path::{Path, PathBuf};
use image::imageops::FilterType;
use image::GenericImageView;
use parking_lot::RwLock;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

static IMAGE_CACHE: RwLock<Option<HashMap<(PathBuf, u16, u16), Vec<Line<'static>>>>> = RwLock::new(None);

/// Renders an image file into Ratatui `Line`s using TrueColor UTF-8 half-blocks (`▀`).
/// Each character represents two vertical pixels (top pixel as foreground, bottom pixel as background).
pub fn render_image_halfblocks(path: &Path, max_width: u16, max_height: u16) -> Option<Vec<Line<'static>>> {
    if !path.exists() || max_width == 0 || max_height == 0 {
        return None;
    }

    let cache_key = (path.to_path_buf(), max_width, max_height);

    // 1. Check in-memory cache
    {
        let read_guard = IMAGE_CACHE.read();
        if let Some(ref cache) = *read_guard {
            if let Some(lines) = cache.get(&cache_key) {
                return Some(lines.clone());
            }
        }
    }

    // 2. Load and decode image
    let img = match image::open(path) {
        Ok(img) => img,
        Err(_) => return None,
    };

    let (orig_w, orig_h) = img.dimensions();
    if orig_w == 0 || orig_h == 0 {
        return None;
    }

    // We have max_width columns and max_height rows.
    // Each row has 2 vertical pixels, so available pixel area is max_width x (max_height * 2).
    let target_pixel_h = (max_height as u32 * 2).max(2);
    let target_pixel_w = (max_width as u32).max(1);

    // Compute aspect-ratio scale
    let scale_w = target_pixel_w as f64 / orig_w as f64;
    let scale_h = target_pixel_h as f64 / orig_h as f64;
    let scale = scale_w.min(scale_h).min(1.0); // do not upscale tiny icons excessively

    let mut final_w = (orig_w as f64 * scale).round() as u32;
    let mut final_h = (orig_h as f64 * scale).round() as u32;

    final_w = final_w.max(1).min(target_pixel_w);
    final_h = final_h.max(2).min(target_pixel_h);
    // Ensure height in pixels is even
    if final_h % 2 != 0 {
        final_h = (final_h - 1).max(2);
    }

    let resized = img.resize_exact(final_w, final_h, FilterType::Triangle);
    let rgba = resized.to_rgba8();

    let mut lines = Vec::with_capacity((final_h / 2) as usize);

    for y in 0..(final_h / 2) {
        let top_y = y * 2;
        let bottom_y = top_y + 1;

        let mut spans = Vec::with_capacity(final_w as usize);
        for x in 0..final_w {
            let top_px = rgba.get_pixel(x, top_y);
            let bottom_px = rgba.get_pixel(x, bottom_y);

            let top_fg = Color::Rgb(top_px[0], top_px[1], top_px[2]);
            let bottom_bg = Color::Rgb(bottom_px[0], bottom_px[1], bottom_px[2]);

            spans.push(Span::styled(
                "▀",
                Style::default().fg(top_fg).bg(bottom_bg),
            ));
        }
        lines.push(Line::from(spans));
    }

    // 3. Store in cache
    {
        let mut write_guard = IMAGE_CACHE.write();
        let cache = write_guard.get_or_insert_with(HashMap::new);
        // Evict if cache exceeds 100 images
        if cache.len() > 100 {
            cache.clear();
        }
        cache.insert(cache_key, lines.clone());
    }

    Some(lines)
}

/// Open an image or media file in the user's default OS desktop application.
pub fn open_file_in_os(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Err(format!("File does not exist: {:?}", path));
    }

    #[cfg(target_os = "windows")]
    {
        let _status = std::process::Command::new("cmd")
            .args(["/C", "start", "", &path.to_string_lossy()])
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(path)
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}
