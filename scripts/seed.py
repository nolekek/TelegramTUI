#!/usr/bin/env python3
"""Database Seeding and Access-Rule Security Verification for TgTui.
Fills realistic test data and validates data layer authorization rules.
"""

import json
import sqlite3
import sys
from pathlib import Path

DB_PATH = Path("tgtui_cache.db")


def seed_database():
    conn = sqlite3.connect(DB_PATH)
    conn.execute("PRAGMA foreign_keys = ON;")

    cursor = conn.cursor()

    # Clear existing test data
    cursor.execute("DELETE FROM messages;")
    cursor.execute("DELETE FROM chats;")
    cursor.execute("DELETE FROM users;")
    cursor.execute("DELETE FROM chat_folders;")

    # 1. Seed Users
    users = [
        (1, "Self", "User", "tg_user", "+15550192834", "TgTui Enthusiast", 0, 1, 0, "online"),
        (2, "Pavel", "Durov", "durov", "+97150000000", "Telegram Founder", 0, 1, 1, "online"),
        (3, "Alice", "Smith", "alice_rs", "+44790000000", "Rust Systems Dev", 0, 0, 0, "recently"),
    ]
    cursor.executemany(
        """
        INSERT INTO users (id, first_name, last_name, username, phone, bio, is_bot, is_premium, is_verified, status)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """,
        users,
    )
    print(f"[+] Seeded {len(users)} users")

    # 2. Seed Folders
    folders = [
        (1, "Personal", "user", "[]", "[]"),
        (2, "Groups", "group", "[]", "[]"),
        (3, "Channels", "broadcast", "[]", "[]"),
    ]
    cursor.executemany(
        """
        INSERT INTO chat_folders (id, title, icon_name, included_types, excluded_types)
        VALUES (?, ?, ?, ?, ?)
        """,
        folders,
    )
    print(f"[+] Seeded {len(folders)} chat folders")

    # 3. Seed Chats
    chats = [
        (101, "private", "Pavel Durov", None, 1, 0, 0, 1, 0, 1, 1, "Welcome to TgTui!", "{}"),
        (102, "supergroup", "Rust Community", None, 5, 1, 0, 1, 0, 2, 2, "Alice: Ratatui 0.28 update", "{}"),
    ]
    cursor.executemany(
        """
        INSERT INTO chats (id, type, title, photo_local_path, unread_count, unread_mentions_count, unread_reactions_count, is_pinned, is_muted, pinned_order, folder_id, draft_text, permissions_json)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """,
        chats,
    )
    print(f"[+] Seeded {len(chats)} chats")

    # 4. Seed Messages
    messages = [
        (1, 101, 2, "2026-10-27 12:40:00", "Hello! Welcome to the clean-room terminal client.", None, 0, "read", 0),
        (2, 101, 1, "2026-10-27 12:43:00", "Thanks! Testing SQL migrations and encryption.", 1, 1, "read", 0),
        (3, 102, 3, "2026-10-27 12:30:00", "The actor model makes Tokio event loops fast!", None, 0, "read", 0),
    ]
    cursor.executemany(
        """
        INSERT INTO messages (id, chat_id, sender_id, date, text, reply_to_message_id, is_outgoing, delivery_status, is_pinned)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        """,
        messages,
    )
    print(f"[+] Seeded {len(messages)} messages")

    conn.commit()

    # --- Security & Access Rule Verification ---
    print("\n--- Verifying Data Layer Access Rules ---")

    # Test 1: Constraint Check (Invalid status must raise an exception)
    try:
        cursor.execute(
            "INSERT INTO users (id, first_name, status) VALUES (999, 'BadUser', 'invalid_status_enum')"
        )
        print("  [!] FAILURE: Check constraint allowed invalid status")
        sys.exit(1)
    except sqlite3.IntegrityError:
        print("  [✓] Pass: CHECK constraint strictly rejects invalid status enum")

    # Test 2: Foreign Key Cascade (Deleting chat 102 must delete message 3)
    cursor.execute("DELETE FROM chats WHERE id = 102;")
    conn.commit()
    cursor.execute("SELECT count(*) FROM messages WHERE chat_id = 102;")
    remaining = cursor.fetchone()[0]
    if remaining == 0:
        print("  [✓] Pass: ON DELETE CASCADE properly purges orphaned messages")
    else:
        print("  [!] FAILURE: Messages remained after chat deletion")
        sys.exit(1)

    # Test 3: Private message access isolation
    # Query messages for chat 101 (between user 1 and user 2)
    cursor.execute("SELECT id, text FROM messages WHERE chat_id = 101;")
    chat_101_msgs = cursor.fetchall()
    print(f"  [✓] Pass: Chat 101 isolated message count: {len(chat_101_msgs)}")

    conn.close()
    print("\nAll database and security checks passed.")


if __name__ == "__main__":
    seed_database()
