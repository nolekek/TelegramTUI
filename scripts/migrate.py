#!/usr/bin/env python3
"""Database Migration Runner for TgTui.
Applies schema definitions, foreign keys, indexes, and validates SQLite cache tables.
"""

import sqlite3
import sys
from pathlib import Path

DB_PATH = Path("tgtui_cache.db")

MIGRATIONS = [
    # 1. Users
    """
    CREATE TABLE IF NOT EXISTS users (
        id INTEGER PRIMARY KEY,
        first_name TEXT NOT NULL,
        last_name TEXT,
        username TEXT,
        phone TEXT,
        bio TEXT,
        avatar_local_path TEXT,
        is_bot INTEGER NOT NULL DEFAULT 0,
        is_premium INTEGER NOT NULL DEFAULT 0,
        is_verified INTEGER NOT NULL DEFAULT 0,
        status TEXT NOT NULL DEFAULT 'offline'
            CHECK (status IN ('online', 'offline', 'recently', 'within_week', 'hidden')),
        created_at TEXT NOT NULL DEFAULT (datetime('now')),
        updated_at TEXT NOT NULL DEFAULT (datetime('now'))
    );
    CREATE INDEX IF NOT EXISTS idx_users_username ON users (username);
    """,

    # 2. Chat Folders
    """
    CREATE TABLE IF NOT EXISTS chat_folders (
        id INTEGER PRIMARY KEY,
        title TEXT NOT NULL,
        icon_name TEXT NOT NULL DEFAULT 'folder',
        included_types TEXT NOT NULL DEFAULT '[]',
        excluded_types TEXT NOT NULL DEFAULT '[]',
        created_at TEXT NOT NULL DEFAULT (datetime('now'))
    );
    """,

    # 3. Chats (Dialogs)
    """
    CREATE TABLE IF NOT EXISTS chats (
        id INTEGER PRIMARY KEY,
        type TEXT NOT NULL
            CHECK (type IN ('private', 'group', 'supergroup', 'channel')),
        title TEXT NOT NULL,
        photo_local_path TEXT,
        unread_count INTEGER NOT NULL DEFAULT 0,
        unread_mentions_count INTEGER NOT NULL DEFAULT 0,
        unread_reactions_count INTEGER NOT NULL DEFAULT 0,
        is_pinned INTEGER NOT NULL DEFAULT 0,
        is_muted INTEGER NOT NULL DEFAULT 0,
        pinned_order INTEGER NOT NULL DEFAULT 0,
        folder_id INTEGER REFERENCES chat_folders(id) ON DELETE SET NULL,
        last_message_id INTEGER,
        draft_text TEXT,
        permissions_json TEXT NOT NULL DEFAULT '{}',
        created_at TEXT NOT NULL DEFAULT (datetime('now')),
        updated_at TEXT NOT NULL DEFAULT (datetime('now'))
    );
    CREATE INDEX IF NOT EXISTS idx_chats_folder_id ON chats (folder_id);
    CREATE INDEX IF NOT EXISTS idx_chats_pinned ON chats (is_pinned, pinned_order DESC);
    """,

    # 4. Media Attachments
    """
    CREATE TABLE IF NOT EXISTS media_attachments (
        id TEXT PRIMARY KEY,
        type TEXT NOT NULL
            CHECK (type IN ('photo', 'video', 'document', 'audio', 'voice', 'sticker', 'animation')),
        remote_file_id TEXT NOT NULL,
        local_path TEXT,
        file_size INTEGER NOT NULL DEFAULT 0,
        mime_type TEXT NOT NULL DEFAULT 'application/octet-stream',
        width INTEGER,
        height INTEGER,
        duration INTEGER,
        waveform_blob BLOB,
        thumbnail_path TEXT,
        download_status TEXT NOT NULL DEFAULT 'idle'
            CHECK (download_status IN ('idle', 'downloading', 'completed', 'failed')),
        created_at TEXT NOT NULL DEFAULT (datetime('now'))
    );
    CREATE INDEX IF NOT EXISTS idx_media_remote_file_id ON media_attachments (remote_file_id);
    """,

    # 5. Messages
    """
    CREATE TABLE IF NOT EXISTS messages (
        id INTEGER NOT NULL,
        chat_id INTEGER NOT NULL REFERENCES chats(id) ON DELETE CASCADE,
        sender_id INTEGER NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
        date TEXT NOT NULL,
        text TEXT NOT NULL DEFAULT '',
        reply_to_message_id INTEGER,
        forward_from_id INTEGER REFERENCES users(id) ON DELETE SET NULL,
        forward_date TEXT,
        media_id TEXT REFERENCES media_attachments(id) ON DELETE SET NULL,
        is_outgoing INTEGER NOT NULL DEFAULT 0,
        delivery_status TEXT NOT NULL DEFAULT 'sent'
            CHECK (delivery_status IN ('pending', 'sent', 'read', 'failed')),
        edit_date TEXT,
        is_pinned INTEGER NOT NULL DEFAULT 0,
        created_at TEXT NOT NULL DEFAULT (datetime('now')),
        PRIMARY KEY (chat_id, id)
    );
    CREATE INDEX IF NOT EXISTS idx_messages_chat_date ON messages (chat_id, date DESC);
    CREATE INDEX IF NOT EXISTS idx_messages_reply_to ON messages (chat_id, reply_to_message_id);
    """,

    # 6. Message Entities
    """
    CREATE TABLE IF NOT EXISTS message_entities (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        chat_id INTEGER NOT NULL,
        message_id INTEGER NOT NULL,
        type TEXT NOT NULL
            CHECK (type IN ('bold', 'italic', 'code', 'pre', 'spoiler', 'blockquote', 'text_link', 'mention')),
        offset_idx INTEGER NOT NULL,
        length_idx INTEGER NOT NULL,
        extra_data TEXT,
        FOREIGN KEY (chat_id, message_id) REFERENCES messages(chat_id, id) ON DELETE CASCADE
    );
    CREATE INDEX IF NOT EXISTS idx_entities_message ON message_entities (chat_id, message_id);
    """,

    # 7. Message Reactions
    """
    CREATE TABLE IF NOT EXISTS message_reactions (
        chat_id INTEGER NOT NULL,
        message_id INTEGER NOT NULL,
        emoji TEXT NOT NULL,
        count INTEGER NOT NULL DEFAULT 0,
        chosen_by_me INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY (chat_id, message_id, emoji),
        FOREIGN KEY (chat_id, message_id) REFERENCES messages(chat_id, id) ON DELETE CASCADE
    );
    """,

    # 8. Folder-Chats Link
    """
    CREATE TABLE IF NOT EXISTS folder_chats (
        folder_id INTEGER NOT NULL REFERENCES chat_folders(id) ON DELETE CASCADE,
        chat_id INTEGER NOT NULL REFERENCES chats(id) ON DELETE CASCADE,
        pinned_in_folder INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY (folder_id, chat_id)
    );
    """,

    # 9. Local Preferences
    """
    CREATE TABLE IF NOT EXISTS local_preferences (
        key TEXT PRIMARY KEY,
        value TEXT NOT NULL,
        updated_at TEXT NOT NULL DEFAULT (datetime('now'))
    );
    """
]


def run_migrations():
    print(f"Connecting to database: {DB_PATH.absolute()}")
    conn = sqlite3.connect(DB_PATH)
    conn.execute("PRAGMA foreign_keys = ON;")
    conn.execute("PRAGMA journal_mode = WAL;")

    with conn:
        for idx, statement in enumerate(MIGRATIONS, 1):
            conn.executescript(statement)
            print(f"  [+] Migration step {idx}/9 applied successfully")

    # Verify tables
    cursor = conn.cursor()
    cursor.execute("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name;")
    tables = [row[0] for row in cursor.fetchall() if not row[0].startswith("sqlite_")]
    print(f"\nMigration complete. Total active tables: {len(tables)}")
    for t in tables:
        print(f"   • {t}")

    conn.close()


if __name__ == "__main__":
    run_migrations()
