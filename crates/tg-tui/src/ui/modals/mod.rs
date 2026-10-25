pub mod auth;
pub mod help;
pub mod image_viewer;
pub mod proxy;
pub mod settings;

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use tg_core::ProxyConfig;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ActiveModal {
    #[default]
    None,
    Auth,
    Settings,
    Proxy,
    Help,
    ImageViewer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProxyTypeChoice {
    Socks5,
    Http,
    Mtproto,
}

#[derive(Debug, Clone)]
pub struct ProxyModalState {
    pub is_enabled: bool,
    pub proxy_type: ProxyTypeChoice,
    pub server: String,
    pub port: String,
    pub username: String,
    pub password: String,
    pub secret: String,
    pub selected_field: usize,
}

impl Default for ProxyModalState {
    fn default() -> Self {
        Self {
            is_enabled: false,
            proxy_type: ProxyTypeChoice::Socks5,
            server: "127.0.0.1".to_string(),
            port: "1080".to_string(),
            username: String::new(),
            password: String::new(),
            secret: String::new(),
            selected_field: 0,
        }
    }
}

impl ProxyModalState {
    pub fn from_config(config: Option<&ProxyConfig>) -> Self {
        if let Some(cfg) = config {
            let (choice, secret) = match &cfg.proxy_type {
                tg_core::ProxyType::Socks5 => (ProxyTypeChoice::Socks5, String::new()),
                tg_core::ProxyType::Http => (ProxyTypeChoice::Http, String::new()),
                tg_core::ProxyType::Mtproto { secret } => (ProxyTypeChoice::Mtproto, secret.clone()),
            };
            Self {
                is_enabled: cfg.is_enabled,
                proxy_type: choice,
                server: cfg.server.clone(),
                port: cfg.port.to_string(),
                username: cfg.username.clone().unwrap_or_default(),
                password: cfg.password.clone().unwrap_or_default(),
                secret,
                selected_field: 0,
            }
        } else {
            Self::default()
        }
    }

    pub fn to_proxy_config(&self) -> Option<ProxyConfig> {
        let port: u16 = self.port.parse().unwrap_or(1080);
        let proxy_type = match self.proxy_type {
            ProxyTypeChoice::Socks5 => tg_core::ProxyType::Socks5,
            ProxyTypeChoice::Http => tg_core::ProxyType::Http,
            ProxyTypeChoice::Mtproto => tg_core::ProxyType::Mtproto {
                secret: self.secret.clone(),
            },
        };
        Some(ProxyConfig {
            server: self.server.clone(),
            port,
            proxy_type,
            username: if self.username.is_empty() {
                None
            } else {
                Some(self.username.clone())
            },
            password: if self.password.is_empty() {
                None
            } else {
                Some(self.password.clone())
            },
            is_enabled: self.is_enabled,
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct AuthModalState {
    pub input_buffer: String,
    pub show_password_cleartext: bool,
    pub first_name: String,
    pub last_name: String,
    pub reg_field: usize,
}

#[derive(Debug, Clone)]
pub struct SettingsModalState {
    pub selected_index: usize,
    pub confirm_logout: bool,
    pub show_last_message_preview: bool,
    pub notifications_enabled: bool,
}

impl Default for SettingsModalState {
    fn default() -> Self {
        Self {
            selected_index: 0,
            confirm_logout: false,
            show_last_message_preview: true,
            notifications_enabled: true,
        }
    }
}

/// Compute a centered rectangle using percentage of screen dimensions.
pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

/// Compute a centered rectangle with fixed width and height (clamped to screen size).
pub fn centered_rect_fixed(width: u16, height: u16, r: Rect) -> Rect {
    let w = width.min(r.width);
    let h = height.min(r.height);

    let x = r.x + (r.width.saturating_sub(w)) / 2;
    let y = r.y + (r.height.saturating_sub(h)) / 2;

    Rect {
        x,
        y,
        width: w,
        height: h,
    }
}
