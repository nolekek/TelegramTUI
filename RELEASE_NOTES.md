# TelegramTUI v0.1.0 — Initial Release 🚀

A modern, lightning-fast terminal client for Telegram written in Rust, built with Ratatui and TDLib, designed from the ground up for a seamless **Vim workflow**.

---

## ✨ Features

- **⚡ Full Vim Workflow**:
  - **Normal Mode (`Esc`)**: `j` / `k` (or `Ctrl+n` / `Ctrl+p`) line-by-line navigation, `gg` (jump to top) / `G` (jump to newest), `/` or `s` for chat fuzzy search, `i` to enter Insert Mode, `v` for visual selection, `y` to yank text, `:` for command palette.
  - **Insert Mode (`i`)**: Native input field for composing messages, `Enter` to send, `Esc` to return to Normal Mode.
  - **Russian Layout Normalization**: Commands work seamlessly even if your keyboard is in RU layout (`о` acts as `j`, `л` as `k`, `ш` as `i`, etc.).
- **🔍 Instant Chat Finder**:
  - Fuzzy chat search dialog. Start typing any part of a chat name to filter conversations instantly. Press `Enter` to open.
- **📁 Chat Folders / Tabs**:
  - Switch tabs between `All`, `Personal`, `Groups`, `Channels`, and `Bots` using `[` and `]`.
- **🎨 Tokyo Night Theme**:
  - Sleek, polished terminal UI with status badges, connection indicator, unread counters, and custom modals.
- **📦 Zero-Config Bundling**:
  - TDLib dynamic libraries (`libtdjson.so` / `tdjson.dll`) and OpenSSL are bundled right inside the release archive. No system package installations needed!
- **💾 Hybrid Data Storage**:
  - Standard OS locations:
    - **Linux**: `~/.local/share/telegram-tui/` (data) & `~/.config/telegram-tui/` (config)
    - **Windows**: `%APPDATA%\telegram-tui\`
    - **macOS**: `~/Library/Application Support/telegram-tui/`
  - **Portable mode**: Store everything in `./.data/` next to the executable by passing `-p` / `--portable` or creating a `./.data` directory.

---

## 🛠️ CLI Options

```text
Usage: tg-tui [OPTIONS]

Options:
  -h, --help               Print help information and exit
  -V, --version            Print version information and exit
  -p, --portable           Force portable mode (stores all data in ./.data)
  -c, --config <PATH>      Use custom path for config.toml
  -d, --data-dir <PATH>    Use custom directory for session data and cache
  --mock                   Run with offline mock backend for testing
```

---

## 🚀 Quick Start

### Linux (x86_64)

Compatible with Fedora, Ubuntu, Debian, Arch Linux, CentOS, openSUSE, and other modern distributions:

```bash
# 1. Download and extract
tar -xzf tg-tui-linux-x86_64.tar.gz
cd tg-tui-linux-x86_64

# 2. Run the application
./run.sh
# Or directly:
./tg-tui
```

### Windows (x64)

1. Download and extract `tg-tui-windows-x64.zip`.
2. Double-click `tg-tui.exe` or run from PowerShell:
   ```powershell
   .\tg-tui.exe
   ```

---

## 📦 Release Assets & Checksums

| Asset | Platform | SHA256 Checksum |
| :--- | :--- | :--- |
| `tg-tui-linux-x86_64.tar.gz` | Linux x86_64 | `632054761c42d482aee9453dac74b4d16b9e9fc1de6c5842bb6bd2a10439d09a` |
| `tg-tui-windows-x64.zip` | Windows x64 | `001e8bfe7ee1bec1b91e282a3e96648e3c08599418ba8f3cc73a3b41643f44ad` |

---

## 🔨 Building from Source

```bash
# Clone the repository
git clone https://github.com/nolekek/TelegramTUI.git
cd TelegramTUI

# Build and run (TDLib will be downloaded automatically during build)
cargo run --release
```
