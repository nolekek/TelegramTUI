//! Design System Primitives and Theme Tokens for TgTui
//! Generated according to replica-design specifications and WCAG 2.2 AA validation.

use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::BorderType;

/// Color palette roles matching tokens.json
#[derive(Debug, Clone, Copy)]
pub struct ThemePalette {
    pub bg: Color,
    pub surface: Color,
    pub surface_hover: Color,
    pub border: Color,
    pub border_input: Color,
    pub text: Color,
    pub text_muted: Color,
    pub accent: Color,
    pub on_accent: Color,
    pub danger: Color,
    pub success: Color,
    pub warning: Color,
}

impl ThemePalette {
    /// TrueColor (24-bit RGB) Dark Palette matching Telegram Desktop
    pub const fn dark_truecolor() -> Self {
        Self {
            bg: Color::Rgb(14, 22, 33),             // #0e1621
            surface: Color::Rgb(24, 37, 51),         // #182533
            surface_hover: Color::Rgb(32, 47, 61),   // #202f3d
            border: Color::Rgb(40, 58, 79),          // #283a4f
            border_input: Color::Rgb(91, 162, 244),  // #5ba2f4
            text: Color::Rgb(248, 250, 252),        // #f8fafc
            text_muted: Color::Rgb(184, 203, 219),  // #b8cbdb
            accent: Color::Rgb(91, 162, 244),       // #5ba2f4
            on_accent: Color::Rgb(14, 22, 33),      // #0e1621
            danger: Color::Rgb(248, 113, 113),      // #f87171
            success: Color::Rgb(74, 222, 128),      // #4ade80
            warning: Color::Rgb(251, 191, 36),      // #fbbf24
        }
    }

    /// ANSI-16 Fallback Palette for limited terminal environments
    pub const fn ansi16_fallback() -> Self {
        Self {
            bg: Color::Black,
            surface: Color::DarkGray,
            surface_hover: Color::DarkGray,
            border: Color::Gray,
            border_input: Color::Cyan,
            text: Color::White,
            text_muted: Color::Gray,
            accent: Color::Cyan,
            on_accent: Color::Black,
            danger: Color::Red,
            success: Color::Green,
            warning: Color::Yellow,
        }
    }
}

/// Central Theme Engine providing styles for UI components
#[derive(Debug, Clone)]
pub struct Theme {
    pub palette: ThemePalette,
    pub border_type: BorderType,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            palette: ThemePalette::dark_truecolor(),
            border_type: BorderType::Rounded,
        }
    }
}

impl Theme {
    pub fn new(palette: ThemePalette) -> Self {
        Self {
            palette,
            border_type: BorderType::Rounded,
        }
    }

    // --- Base Layout Styles ---

    pub fn base(&self) -> Style {
        Style::default().bg(self.palette.bg).fg(self.palette.text)
    }

    pub fn surface(&self) -> Style {
        Style::default().bg(self.palette.surface).fg(self.palette.text)
    }

    pub fn border(&self) -> Style {
        Style::default().fg(self.palette.border)
    }

    pub fn border_focused(&self) -> Style {
        Style::default().fg(self.palette.border_input)
    }

    // --- Dialogs & Chat List ---

    pub fn dialog_item(&self, selected: bool) -> Style {
        if selected {
            Style::default()
                .bg(self.palette.surface_hover)
                .fg(self.palette.text)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().bg(self.palette.bg).fg(self.palette.text)
        }
    }

    pub fn dialog_snippet(&self) -> Style {
        Style::default().fg(self.palette.text_muted)
    }

    pub fn badge_unread(&self, muted: bool) -> Style {
        if muted {
            Style::default()
                .bg(self.palette.border)
                .fg(self.palette.text_muted)
        } else {
            Style::default()
                .bg(self.palette.accent)
                .fg(self.palette.on_accent)
                .add_modifier(Modifier::BOLD)
        }
    }

    // --- Messages & Bubbles ---

    pub fn message_incoming(&self) -> Style {
        Style::default().bg(self.palette.surface).fg(self.palette.text)
    }

    pub fn message_outgoing(&self) -> Style {
        Style::default().bg(self.palette.border).fg(self.palette.text)
    }

    pub fn message_author(&self) -> Style {
        Style::default()
            .fg(self.palette.accent)
            .add_modifier(Modifier::BOLD)
    }

    pub fn message_meta(&self) -> Style {
        Style::default().fg(self.palette.text_muted)
    }

    pub fn checkmarks(&self, read: bool) -> Style {
        if read {
            Style::default().fg(self.palette.accent)
        } else {
            Style::default().fg(self.palette.text_muted)
        }
    }

    // --- Text Formatting Styles ---

    pub fn text_bold(&self) -> Style {
        Style::default().add_modifier(Modifier::BOLD)
    }

    pub fn text_italic(&self) -> Style {
        Style::default().add_modifier(Modifier::ITALIC)
    }

    pub fn text_code(&self) -> Style {
        Style::default()
            .bg(self.palette.surface_hover)
            .fg(self.palette.accent)
    }

    pub fn text_spoiler(&self) -> Style {
        Style::default()
            .bg(self.palette.text_muted)
            .fg(self.palette.text_muted)
    }

    pub fn text_quote(&self) -> Style {
        Style::default()
            .fg(self.palette.text_muted)
            .add_modifier(Modifier::ITALIC)
    }

    // --- Composer & Input ---

    pub fn composer_input(&self, focused: bool) -> Style {
        if focused {
            Style::default().bg(self.palette.surface).fg(self.palette.text)
        } else {
            Style::default().bg(self.palette.surface).fg(self.palette.text_muted)
        }
    }

    pub fn reply_bar(&self) -> Style {
        Style::default()
            .bg(self.palette.surface_hover)
            .fg(self.palette.accent)
    }

    // --- Media & Waveforms ---

    pub fn waveform_active(&self) -> Style {
        Style::default()
            .fg(self.palette.accent)
            .add_modifier(Modifier::BOLD)
    }

    pub fn waveform_inactive(&self) -> Style {
        Style::default().fg(self.palette.text_muted)
    }

    // --- Notifications & Alerts ---

    pub fn danger(&self) -> Style {
        Style::default().fg(self.palette.danger)
    }

    pub fn success(&self) -> Style {
        Style::default().fg(self.palette.success)
    }

    pub fn warning(&self) -> Style {
        Style::default().fg(self.palette.warning)
    }
}
