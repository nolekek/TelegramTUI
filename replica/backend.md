# Backend Architecture & Security Checklist: TgTui

This document records the backend integration, MTProto communication layer, SQLite cache migrations, and security compliance for TgTui.

---

## 1. Authentication & Session Management

* **MTProto 2.0 Auth Protocol**:
  - User submits international phone number (E.164 format).
  - Telegram sends OTP verification code via app or SMS (`td_send("checkAuthenticationCode")`).
  - Two-Step Verification (Cloud Password) gate handles SRP (Secure Remote Password) protocol when 2FA is active (`td_send("checkAuthenticationPassword")`).
* **Session Persistence**:
  - Encrypted auth keys and session states persist inside `tdlib_data/` directory.
  - Optional local encryption with `TG_DATABASE_ENCRYPTION_KEY` using AES-256.
  - User can terminate active sessions remotely via Security settings (S12).

---

## 2. Database Migrations & Data Access Rules

* **SQLite Cache Engine** (`tgtui_cache.db`):
  - Migrations defined in [`replica/schema.sql`](file:///d:/TgTui/replica/schema.sql) and executed via [`scripts/migrate.py`](file:///d:/TgTui/scripts/migrate.py).
  - 9 relational tables: `users`, `chats`, `chat_folders`, `messages`, `message_entities`, `media_attachments`, `message_reactions`, `folder_chats`, `local_preferences`.
  - WAL journal mode enabled (`PRAGMA journal_mode = WAL;`) for high concurrency between Tokio async reader/writer tasks.
  - Foreign key constraints active (`PRAGMA foreign_keys = ON;`).
* **Access Rules & Isolation**:
  - Cascade deletion (`ON DELETE CASCADE`) ensures deleting a chat purges associated messages, reactions, and entities without orphaned records.
  - Data access queries filter strictly by `chat_id` and user ownership.
  - Enforced `CHECK` constraints on `status`, `delivery_status`, and entity types.
  - Verified with automated test script [`scripts/seed.py`](file:///d:/TgTui/scripts/seed.py).

---

## 3. Third-Party Integrations & Public APIs

* **Official Telegram API**:
  - Connected using user-owned credentials (`api_id` and `api_hash`) obtained from [my.telegram.org](https://my.telegram.org).
  - No reverse-engineered private endpoints used.
  - Configuration parsed via [`src/config.rs`](file:///d:/TgTui/src/config.rs) from `config.toml` and environment variables.
  - Template provided in [`.env.example`](file:///d:/TgTui/.env.example).
* **Audio / External Media Launchers**:
  - Opus voice notes and media files dispatched to system default applications or `mpv` via hotkey (`p` / `o`).

---

## 4. Security Checklist

- [x] **Secrets Isolation**: `TG_API_ID`, `TG_API_HASH`, and `TG_DATABASE_ENCRYPTION_KEY` are read exclusively from environment variables or local `config.toml`.
- [x] **Git Ignore Rules**: `.gitignore` strictly ignores `target/`, `.env*`, `config.local.toml`, `*.binlog`, and `tdlib_data/`.
- [x] **Input Validation**: `AppConfig::validate()` validates numeric format for `api_id` and 32-character hexadecimal length for `api_hash`.
- [x] **Data Integrity**: SQLite schema enforces `CHECK` constraints and `FOREIGN KEY` cascades.
- [x] **Zero Plaintext Logs**: Passwords and OTP codes are masked in memory and terminal buffers.
- [x] **FloodWait Throttling**: MTProto error 420 (`FLOOD_WAIT_X`) is intercepted with an automatic cooldown timer.
- [x] **Upload Limits**: Standard MTProto 2 GB payload size enforced.
- [x] **Dependencies Audit**: Rust dependencies pinned and compiled cleanly.

---

## 5. Verification Commands

```powershell
# 1. Run database migrations
python scripts/migrate.py

# 2. Seed test data and verify access rules
python scripts/seed.py

# 3. Compile and check backend
cargo check
```
