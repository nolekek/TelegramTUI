# ⚡ TelegramTUI

[![CI](https://github.com/nolekek/TelegramTUI/actions/workflows/ci.yml/badge.svg)](https://github.com/nolekek/TelegramTUI/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Platform: Cross-Platform](https://img.shields.io/badge/Platform-Linux%20%7C%20Windows%20%7C%20macOS-brightgreen.svg)]()

> Fast, beautiful, and distraction-free terminal Telegram client in Rust with a full-screen **Vim workflow**, native TDLib engine, and Tokyo Night aesthetic.

---

## 🌟 Key Highlights

- **⚡ Full Vim Workflow**: Two discrete full-screen views (no cramped sidebars). Fast chat search and instant chat focus with full Vim semantics (`Normal`, `Insert`, `Command`).
- **🇷🇺 Cyrillic Normalization**: All Vim shortcuts and command keys work transparently regardless of current keyboard layout (English or Russian).
- **🎨 Tokyo Night Palette**: Modern styled UI powered by [Ratatui](https://github.com/ratatui/ratatui) and [Crossterm](https://github.com/crossterm-rs/crossterm).
- **🚀 Native Cross-Platform**: Runs on Linux (glibc 2.28+), Windows 10/11, and macOS.
- **🔄 Zero-Setup TDLib**: Dynamic FFI loading of Telegram Database Library (`tdjson`). The build process automatically fetches the appropriate prebuilt binary for your platform.
- **🛡️ Hybrid Storage**: Uses standard OS directories (`~/.local/share/telegram-tui/`, `%APPDATA%\telegram-tui`) by default, with automatic portable mode when `./.data` is present or `-p` / `--portable` is used.
- **🌐 Network Resilience**: Integrated SOCKS5, HTTP, and MTProto proxy configuration directly inside the TUI (`F3`).

---

## ⌨️ Vim Navigation Cheatsheet

### 1. Global Navigation
| Key | Action |
| --- | --- |
| `Ctrl+C` | Instant quit from anywhere |
| `Ctrl+F` | Jump directly to Chat Search |
| `F2` | Open Settings modal |
| `F3` | Open Proxy Configuration modal |

### 2. Chat Search Screen
| Key | Action |
| --- | --- |
| `Typing...` | Filters chats in real-time |
| `↓` / `↑` / `Ctrl+j` / `Ctrl+k` | Navigate filtered chat list |
| `Tab` / `Shift+Tab` | Switch category tabs (*Все*, *Личные*, *Группы*, *Каналы*, *Боты*) |
| `Enter` | Open selected chat in full-screen view |
| `Esc` | Clear search query or return to active chat |

### 3. Chat View (Normal Mode)
| Key | Action |
| --- | --- |
| `j` / `k` | Scroll messages down / up |
| `G` | Jump to newest messages |
| `gg` | Jump to oldest loaded messages |
| `i` / `a` / `Enter` | Enter **Insert Mode** to type a message |
| `r` | Reply to highlighted message |
| `e` | Edit selected outgoing message |
| `dd` | Delete selected message |
| `Esc` / `/` / `s` | Return to Chat Search |
| `:` | Enter **Command Mode** |

### 4. Chat View (Insert Mode)
| Key | Action |
| --- | --- |
| `Enter` | Send message |
| `Esc` | Return to **Normal Mode** |
| `Ctrl+W` | Delete word backwards |
| `Ctrl+U` | Clear entire input buffer |

### 5. Command Mode (`:`)
| Command | Action |
| --- | --- |
| `:q`, `:quit` | Quit application |
| `:b`, `:find` | Open Chat Search |
| `:w`, `:settings` | Open Settings |
| `Esc` | Cancel command and return to Normal Mode |

---

## 📦 Installation & Prebuilt Binaries

Ready-to-use standalone archives are available on the [Releases](https://github.com/nolekek/TelegramTUI/releases) page:

### Linux (x86_64)
```bash
# Download and extract the release archive
tar -xzf tg-tui-linux-x86_64.tar.gz
cd tg-tui-linux-x86_64

# Run directly
./tg-tui
# Or using the launcher
./run.sh
```
*Note: All TDLib dependencies (`libtdjson.so`) are included directly in the archive.*

### Windows (x64)
1. Download `tg-tui-windows-x64.zip` from Releases.
2. Extract the folder and double-click `tg-tui.exe` (or run it from Windows Terminal / PowerShell).

---

## 🛠️ Building From Source

### Prerequisites
- [Rust toolchain](https://rustup.rs/) (1.75+)
- Git

### Build & Run
```bash
git clone https://github.com/nolekek/TelegramTUI.git
cd TelegramTUI

# Simply build and run — TDLib is fetched automatically!
cargo run -p tg-tui --release
```

---

## ⚙️ CLI Options & Configuration

```text
TelegramTUI v0.1.0
USAGE:
    tg-tui [OPTIONS]

OPTIONS:
    -h, --help               Print this help information and exit
    -V, --version            Print version information and exit
    -p, --portable           Force portable mode (stores all data in ./.data)
    -c, --config <PATH>      Use custom path for config.toml
    -d, --data-dir <PATH>    Use custom directory for session data and cache
    --mock                   Run with offline mock backend for testing
```

### Storage Paths:
- **Linux**: `~/.local/share/telegram-tui/` (data/cache) and `~/.config/telegram-tui/` (config)
- **macOS**: `~/Library/Application Support/telegram-tui/`
- **Windows**: `%APPDATA%\telegram-tui\`
- **Portable Mode**: `./.data/` (automatically selected if `./.data` exists or `-p` / `--portable` is provided)

---

## 📄 License

This project is licensed under the [MIT License](LICENSE).
