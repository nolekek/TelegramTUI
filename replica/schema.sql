-- Architecture Schema for TgTui (Telegram Terminal Client)
-- Represents the relational data model for caching, offline indexing, and local state

-- 1. Users table
CREATE TABLE IF NOT EXISTS users (
    id BIGINT PRIMARY KEY,
    first_name TEXT NOT NULL,
    last_name TEXT,
    username TEXT,
    phone TEXT,
    bio TEXT,
    avatar_local_path TEXT,
    is_bot BOOLEAN NOT NULL DEFAULT FALSE,
    is_premium BOOLEAN NOT NULL DEFAULT FALSE,
    is_verified BOOLEAN NOT NULL DEFAULT FALSE,
    status TEXT NOT NULL DEFAULT 'offline'
        CHECK (status IN ('online', 'offline', 'recently', 'within_week', 'hidden')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_users_username ON users (username);

-- 2. Chat Folders (Filters)
CREATE TABLE IF NOT EXISTS chat_folders (
    id INTEGER PRIMARY KEY,
    title TEXT NOT NULL,
    icon_name TEXT NOT NULL DEFAULT 'folder',
    included_types TEXT[] NOT NULL DEFAULT '{}',
    excluded_types TEXT[] NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- 3. Chats (Dialogs)
CREATE TABLE IF NOT EXISTS chats (
    id BIGINT PRIMARY KEY,
    type TEXT NOT NULL
        CHECK (type IN ('private', 'group', 'supergroup', 'channel')),
    title TEXT NOT NULL,
    photo_local_path TEXT,
    unread_count INTEGER NOT NULL DEFAULT 0,
    unread_mentions_count INTEGER NOT NULL DEFAULT 0,
    unread_reactions_count INTEGER NOT NULL DEFAULT 0,
    is_pinned BOOLEAN NOT NULL DEFAULT FALSE,
    is_muted BOOLEAN NOT NULL DEFAULT FALSE,
    pinned_order INTEGER NOT NULL DEFAULT 0,
    folder_id INTEGER REFERENCES chat_folders(id) ON DELETE SET NULL,
    last_message_id BIGINT,
    draft_text TEXT,
    permissions_json JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_chats_folder_id ON chats (folder_id);
CREATE INDEX IF NOT EXISTS idx_chats_pinned ON chats (is_pinned, pinned_order DESC);

-- 4. Media Attachments
CREATE TABLE IF NOT EXISTS media_attachments (
    id TEXT PRIMARY KEY,
    type TEXT NOT NULL
        CHECK (type IN ('photo', 'video', 'document', 'audio', 'voice', 'sticker', 'animation')),
    remote_file_id TEXT NOT NULL,
    local_path TEXT,
    file_size BIGINT NOT NULL DEFAULT 0,
    mime_type TEXT NOT NULL DEFAULT 'application/octet-stream',
    width INTEGER,
    height INTEGER,
    duration INTEGER,
    waveform_bytes BYTEA,
    thumbnail_path TEXT,
    download_status TEXT NOT NULL DEFAULT 'idle'
        CHECK (download_status IN ('idle', 'downloading', 'completed', 'failed')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_media_remote_file_id ON media_attachments (remote_file_id);

-- 5. Messages
CREATE TABLE IF NOT EXISTS messages (
    id BIGINT NOT NULL,
    chat_id BIGINT NOT NULL REFERENCES chats(id) ON DELETE CASCADE,
    sender_id BIGINT NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    date TIMESTAMPTZ NOT NULL,
    text TEXT NOT NULL DEFAULT '',
    reply_to_message_id BIGINT,
    forward_from_id BIGINT REFERENCES users(id) ON DELETE SET NULL,
    forward_date TIMESTAMPTZ,
    media_id TEXT REFERENCES media_attachments(id) ON DELETE SET NULL,
    is_outgoing BOOLEAN NOT NULL DEFAULT FALSE,
    delivery_status TEXT NOT NULL DEFAULT 'sent'
        CHECK (delivery_status IN ('pending', 'sent', 'read', 'failed')),
    edit_date TIMESTAMPTZ,
    is_pinned BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (chat_id, id)
);

CREATE INDEX IF NOT EXISTS idx_messages_chat_date ON messages (chat_id, date DESC);
CREATE INDEX IF NOT EXISTS idx_messages_reply_to ON messages (chat_id, reply_to_message_id);

-- 6. Message Entities (Rich text spans)
CREATE TABLE IF NOT EXISTS message_entities (
    id BIGSERIAL PRIMARY KEY,
    chat_id BIGINT NOT NULL,
    message_id BIGINT NOT NULL,
    type TEXT NOT NULL
        CHECK (type IN ('bold', 'italic', 'code', 'pre', 'spoiler', 'blockquote', 'text_link', 'mention')),
    offset INTEGER NOT NULL,
    length INTEGER NOT NULL,
    extra_data TEXT,
    FOREIGN KEY (chat_id, message_id) REFERENCES messages(chat_id, id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_entities_message ON message_entities (chat_id, message_id);

-- 7. Message Reactions
CREATE TABLE IF NOT EXISTS message_reactions (
    chat_id BIGINT NOT NULL,
    message_id BIGINT NOT NULL,
    emoji TEXT NOT NULL,
    count INTEGER NOT NULL DEFAULT 0,
    chosen_by_me BOOLEAN NOT NULL DEFAULT FALSE,
    PRIMARY KEY (chat_id, message_id, emoji),
    FOREIGN KEY (chat_id, message_id) REFERENCES messages(chat_id, id) ON DELETE CASCADE
);

-- 8. Chat-Folder Many-to-Many mapping
CREATE TABLE IF NOT EXISTS folder_chats (
    folder_id INTEGER NOT NULL REFERENCES chat_folders(id) ON DELETE CASCADE,
    chat_id BIGINT NOT NULL REFERENCES chats(id) ON DELETE CASCADE,
    pinned_in_folder BOOLEAN NOT NULL DEFAULT FALSE,
    PRIMARY KEY (folder_id, chat_id)
);

-- 9. Local Key-Value Preferences & Cache
CREATE TABLE IF NOT EXISTS local_preferences (
    key TEXT PRIMARY KEY,
    value JSONB NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
