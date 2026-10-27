# TelegramTUI (tg-tui) v1.0.0

[![Rust](https://img.shields.io/badge/rust-2021_edition-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Version](https://img.shields.io/badge/version-1.0.0-green.svg)](Cargo.toml)
[![Tests](https://img.shields.io/badge/tests-14%2F14%20passed-brightgreen.svg)](tests/e2e_flows.rs)
[![Memory Footprint](https://img.shields.io/badge/memory-%3C25%20MB%20RAM-success.svg)](#features)

> **High-performance, distraction-free Telegram Terminal Client (TUI)** built in Rust using [Ratatui](https://github.com/ratatui/ratatui) and [Tokio](https://tokio.rs/).

Designed specifically for software engineers, sysadmins, and terminal power-users who want instant messaging without desktop bloat, high CPU temperatures, or mouse dependence.

---

## ⚡ Why TelegramTUI?

- **Ultra-Lightweight (<25 MB RAM)**: Replaces official 1.5 GB+ Qt/WebKit/Electron resource footprint. Zero CPU overhead when idle.
- **100% Keyboard-Driven**: Full Vim-style navigation (`j`/`k`, `/`, `Tab`, `Esc`, `?`). Never reach for your mouse.
- **Zero Bloat & No Distractions**: Purged of TON crypto wallets, WebGL mini-apps, casino bots, stories, and aggressive marketing popups.
- **Local-First & Fast**: Local SQLite persistence with WAL journaling mode, optimistic local UI rendering, and instant indexing.
- **WCAG 2.2 AA Compliant**: Terminal themes verified for accessibility (contrast ratio > 4.5:1 across ANSI and TrueColor palettes).
- **Tmux & SSH Ready**: Works seamlessly over remote SSH connections, mosh, and inside terminal multiplexers.

---

## 🚀 Quick Start

### Prerequisites
- [Rust toolchain](https://rustup.rs/) (version 1.80 or newer)
- SQLite3

### Installation & Run

```bash
# Clone the repository
git clone https://github.com/nolekek/TelegramTUI.git
cd TelegramTUI

# Run development client
cargo run --release

# Run design preview (visualize all UI widgets)
cargo run --bin design_preview

# Run automated headless snapshot generator
cargo run --bin capture_screens
```

### Running Tests

```bash
cargo test
```
*All 14 unit and end-to-end integration tests run in under 2 seconds.*

---

## ⌨️ Keybindings Cheatsheet

| Key | Context | Action |
|---|---|---|
| `j` / `↓` | Dialogs / Messages | Move cursor down |
| `k` / `↑` | Dialogs / Messages | Move cursor up |
| `Enter` | Dialogs | Open selected chat |
| `Tab` | Global | Cycle focus (Folders → Dialogs → Chat → Info) |
| `/` | Global | Open global / in-chat search modal |
| `i` | Chat | Enter typing / insert mode in message composer |
| `Esc` | Composer / Modals | Exit input mode / close overlay modal |
| `r` | Message | Reply to selected message |
| `e` | Message | Edit your sent message |
| `d` | Message | Delete message (modal confirmation) |
| `+` | Message | Open emoji reaction picker |
| `f` | Message | Open forward modal (with privacy author toggle) |
| `p` | Voice Message | Play voice message waveform |
| `a` | Chat | Open attachment dialog (photos, files, documents) |
| `o` | Media Message | Open media in fullscreen Lightbox |
| `?` | Global | Toggle interactive shortcuts cheatsheet overlay |
| `Ctrl+C` / `q` | Normal Mode | Quit TelegramTUI |

---

## 🏗️ Architecture

TelegramTUI uses an asynchronous Actor Model over Tokio channels:

```text
┌─────────────────┐       mpsc::channel      ┌──────────────────┐
│                 │ ───────────────────────> │                  │
│     UiActor     │                          │  AppCoordinator  │
│ (Ratatui/Input) │ <─────────────────────── │   (State & DB)   │
└─────────────────┘       broadcast/mpsc     └──────────────────┘
                                                       ▲
                                                       │ TDLib events
                                                       ▼
                                             ┌──────────────────┐
                                             │    TdlibActor    │
                                             │ (Network & Auth) │
                                             └──────────────────┘
```

- **`src/app.rs`**: Application state machine, dialog manager, message history model, and active modal overlays.
- **`src/ui.rs`**: Modular Ratatui rendering engine (dialog list, chat stream, composer, forwarder, reactions, lightbox, cheatsheet).
- **`src/theme.rs`**: Design tokens, semantic color roles, and border styles.
- **`src/config.rs`**: TOML configuration parser and validator.
- **`replica/schema.sql`**: Production SQLite schema with foreign keys, indexes, and WAL mode.

---

## ⚙️ Configuration

Copy `.env.example` or create `config.toml`:

```toml
[telegram]
api_id = 12345678
api_hash = "0123456789abcdef0123456789abcdef"

[ui]
theme = "dark" # "dark" | "light" | "ansi"
show_waveform = true
compact_mode = false

[storage]
database_path = "tgtui_cache.db"
cache_max_mb = 512
```

---

## 📄 License

MIT License. See [LICENSE](LICENSE) for details.
