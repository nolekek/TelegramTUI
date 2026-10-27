# Recon map: Telegram Desktop (Desktop / TUI)

Scope: Core Desktop Messaging Client (Auth, Chat List, Message History, Composer, Media Viewer, Search, Shared Media, Folders)
For: High-performance Desktop / Terminal (TUI) Telegram client (`TgTui`)
Date: 2026-10-27

## Sources

| # | source | URL | notes |
| --- | --- | --- | --- |
| 1 | Telegram Core API Docs | https://core.telegram.org/api | MTProto wire protocol, encryption, data structures, authorization |
| 2 | TDLib Official Documentation | https://core.telegram.org/tdlib/docs/ | High-level client API, schema types (User, Chat, Message, Content) |
| 3 | Telegram Desktop Features & FAQ | https://telegram.org/faq | Official explanations of functional limits, 2FA, privacy, message rules |
| 4 | Telegram Desktop Official Site | https://desktop.telegram.org | Feature highlights, platform builds, release notes |
| 5 | Telegram Desktop Public Changelogs | https://desktop.telegram.org/changelog | History of UI additions (Topics, Reactions, Spoilers, Folders) |
| 6 | Active User Account & Desktop Client | Telegram Desktop v5.x on Windows/macOS/Linux | Visual layout, hotkeys, keyboard navigation, corner states, network reconnects |

## Core loop

Users connect securely to Telegram to instantly read, compose, search, and exchange formatted text, media, and reactions across personal chats, group communities, and broadcast channels.

## Screens

| ID | screen | route / how to reach | purpose | key components | states seen |
| --- | --- | --- | --- | --- | --- |
| S01 | Auth & 2FA Gate | App startup when unauthenticated | Authenticate user via phone + SMS/App code or QR scan | Phone input, QR code renderer, OTP code entry, Cloud Password input, error banner | empty (fresh install), waiting for code, resend cooldown timer, 2FA prompt, flood-wait error |
| S02 | Main Application Shell | Root view after authentication | Main workspace orchestrating folder bar, chat list, and active conversation | Left folder sidebar, Dialogs column, Active chat view, status/reconnect indicator | connected, updating/syncing, offline/reconnecting, proxy active |
| S03 | Active Chat History | S02 center column (select chat) | Read conversation history with continuous virtual scrolling | Message bubbles, date divider chips, pinned message header bar, unread banner, jump-to-bottom button | loading history, empty (no messages), active stream, pinned bar expanded |
| S04 | Message Composer | S02 bottom of active chat | Write and dispatch rich messages, attachments, or voice notes | Auto-expanding textarea, formatting bar, reply/edit indicator bar, attachment clip, emoji/sticker button, send/mic button | empty (placeholder), typing with draft, editing active, reply active, blocked/read-only (channel or muted group) |
| S05 | Chat Profile & Info Panel | S02 right sidebar (click chat header) | View contact/group metadata, mute toggles, member lists, actions | Avatar, title, username, bio, notification toggle, member list, action buttons (Leave, Clear, Block) | loading, normal, admin mode with edit controls |
| S06 | Shared Media Drawer | S05 -> "Shared Media" / Tabs | Browse media history sent in this chat by category | Tab bar (Photos, Files, Audio, Links, Voice), virtualized thumbnail grid / list, download progress | empty, loading page, populated gallery |
| S07 | Message Context Menu | S03 right-click or long-press on message | Quick contextual actions on an individual message | Floating menu: Reply, Copy, Edit, Pin, Forward, Delete, Select, View Reactions | own message vs other's message, admin permissions present |
| S08 | Media Viewer Lightbox | S03 click on photo or video | Distraction-free full-size media inspection | Black backdrop, pan/zoom canvas, video scrubber, toolbar (prev, next, download, copy, close) | loading high-res, playing video, paused, zoom 100%-400% |
| S09 | Sticker, Emoji & GIF Picker | S04 click smiley icon | Discover and pick visual expressions | Tabs: Emoji, Stickers, GIFs; search box, recent items grid, pack navigation strip | loading packs, searching Tenor GIFs, empty search results |
| S10 | Global & In-Chat Search Panel | `Ctrl+F` or click search bar in S02 | Search across all messages or within current chat | Search input, filter chips (media, links, files), search results list with match preview | empty query, searching, results found, zero results |
| S11 | Forward Message Modal | S07 -> "Forward" | Forward selected messages to one or multiple recipients | Chat list selector with search, quote options (hide sender name, hide caption), Send button | selecting target, confirmation |
| S12 | Settings & Preferences Modal | S02 hamburger/gear icon | Configure profile, themes, notifications, storage, and network | Navigation sidebar, profile editor, toggle switches, slider controls, session list, proxy form | normal, unsaved changes, cache calculation in progress |
| S13 | New Chat / Group / Channel Modal | S02 edit pencil / compose icon | Wizard to start 1-on-1 dialogue, create group or broadcast channel | Radio choice (Group/Channel), title input, avatar uploader, contact picker list | step 1 (info), step 2 (members), creating |
| S14 | Forum Topics List View | S02 for supergroups with topics enabled | Display sub-threads inside large community supergroups | Topic rows, topic icons/colors, last message snippet, unread counter, "Create Topic" button | loading topics, filtered, empty |

## Flows

```
F01 User Logs In via Phone & 2FA
    S01 (Enter phone) -> S01 (Receive & type OTP code) -> S01 (Enter 2FA Cloud Password) -> S02 (Main Shell loaded)
    happy path clicks: 3 (plus keyboard inputs)
    edge: Invalid OTP code, FloodWait error (cooldown timer), Forgot 2FA password (recovery email flow), Account deleted
```

```
F02 Compose and Send Rich Text Message
    S02 (Select chat) -> S04 (Focus composer) -> Type text with markdown syntax (**bold**, `code`) -> Press Enter -> S03 renders optimistic pending bubble (clock icon) -> Server ACK receives msg_id -> Status updates to sent (single tick) -> Recipient reads (double tick)
    happy path clicks: 1 (focus & enter)
    edge: Offline state (queued locally in draft/outbox), chat slow mode active (timer counts down), user restricted by admin
```

```
F03 Reply to an Existing Message
    S03 (Hover/Right-click message) -> S07 (Click "Reply" or double click) -> S04 displays Reply banner with sender snippet -> Type response -> Press Enter -> S03 renders new message referencing replied msg_id (clicking preview scrolls back to original)
    happy path clicks: 2
    edge: Replied message was deleted before response sent, replied message belongs to another topic
```

```
F04 Edit and Delete Messages
    S03 (Right-click own message) -> S07 (Click "Edit" or press Up arrow in empty composer) -> S04 loads text into composer with "Edit" banner -> Modify text -> Press Enter -> S03 message updates in place with "(edited)" label
    happy path clicks: 2
    edge: Edit window expired (>48h if applicable), delete dialog offers "Delete for me" vs "Delete for everyone"
```

```
F05 Send File / Photo Attachment
    S04 (Click attachment clip or drag file into S03) -> File dialog / Drop overlay -> Preview modal with toggle: "Compress image" vs "Send as uncompressed file" -> Add optional caption -> Click Send -> S03 displays message bubble with circular upload progress -> Upload completes
    happy path clicks: 3
    edge: File size exceeds 2 GB limit (4 GB for Premium), network interrupt resumes upload chunk
```

```
F06 Forward Message with Sender Privacy
    S03 (Right-click message) -> S07 (Click "Forward") -> S11 opens -> Select destination chat from list -> Toggle "Hide Sender Name" -> Click Send -> S02 switches to target chat with forwarded message
    happy path clicks: 3
    edge: Forwarding restricted by source chat (content protection enabled), target chat does not allow media
```

```
F07 Search and Navigate to Message
    S02 (Press Ctrl+F) -> S10 (Type search term) -> Instant list of matching messages rendered -> Click matching result -> S03 automatically scrolls to target message with a momentary highlight animation
    happy path clicks: 2
    edge: No matching results, search query within date range via calendar picker
```

```
F08 Create and Organize Chat Folders
    S02 (Right-click folder bar or open S12 Settings -> Folders) -> Click "Create New Folder" -> Enter folder name -> Add included filters (e.g. "Groups", "Channels", specific chats) -> Save -> S02 left sidebar updates with new folder icon & badge
    happy path clicks: 4
    edge: Maximum folder limit reached, folder reordering via drag-and-drop
```

## Components

| component | variants | states | used on |
| --- | --- | --- | --- |
| FolderSidebarItem | All, Personal, Work, Custom icon, Settings | default, active/selected, hover, unread-badge, muted-badge | S02 |
| DialogListItem | 1-on-1, Group, Channel, Bot, Secret | default, selected, hover, pinned, muted, typing-indicator | S02, S11 |
| MessageBubble | Incoming, Outgoing, Service/Date header | pending (clock), sent (1 tick), read (2 ticks), error, selected | S03 |
| ComposerInput | Textarea with auto-resize | empty, focused, filled, disabled (restricted/readonly) | S04 |
| ReplyEditBar | Reply preview, Edit preview | default, closing | S04 |
| ContextMenu | Message menu, Chat menu, Media menu | open, item-hover, item-disabled | S07, S02, S03 |
| LightboxViewer | Photo viewer, Video player | opening, loaded, zooming, buffering, paused, playing | S08 |
| StickerPickerTab | Emoji grid, Sticker packs, Tenor GIF | active tab, pack scrolling, search active, empty | S09 |
| ReactionPill | Single emoji + counter | default, chosen_by_me (highlighted), animating | S03 |
| SharedMediaTab | Photos, Files, Audio, Links, Voice | active, hover, loading skeleton | S06 |
| ModalDialog | Alert, Form, Confirmation, Settings | open, backdrop-click, submitting, error | S01, S11, S12, S13 |
| SearchBox | Header search, In-chat search, Modal search | unfocused, focused, typing, clear button visible | S02, S09, S10, S11 |

## Inferred data model

```
User
    id: int64
    first_name: string
    last_name: string (optional)
    username: string (optional)
    phone: string
    bio: string (optional)
    avatar_file_id: string (optional)
    is_bot: boolean
    is_premium: boolean
    is_verified: boolean
    status: UserStatus (online | offline | recently | within_week | hidden)
    evidence: S01 auth response, S05 profile panel, TDLib user object
    confidence: high
```

```
Chat (Dialog)
    id: int64
    type: ChatType (private | group | supergroup | channel)
    title: string
    photo_file_id: string (optional)
    unread_count: int32
    unread_mentions_count: int32
    unread_reactions_count: int32
    is_pinned: boolean
    is_muted: boolean
    folder_id: int32 (optional)
    draft_message: DraftMessage (optional: text, reply_to_id, date)
    last_message_id: int64
    has_scheduled_messages: boolean
    permissions: ChatPermissions (can_send_messages, can_send_media, can_pin, etc.)
    evidence: S02 chat list, S05 chat profile, TDLib chat object
    confidence: high
```

```
Message
    id: int64
    chat_id: int64
    sender_id: int64 (user_id or channel_id)
    date: timestamp (UTC)
    text: string
    entities: list<MessageEntity> (type: bold|italic|code|pre|spoiler|link|mention, offset, length)
    reply_to_message_id: int64 (optional)
    forward_info: MessageForwardInfo (from_id, from_name, date) (optional)
    media: MessageMedia (photo | video | document | audio | voice | sticker | contact) (optional)
    reactions: list<ReactionCount> (emoji, count, chosen_by_me)
    edit_date: timestamp (optional)
    is_outgoing: boolean
    delivery_status: DeliveryStatus (pending | sent | read | failed)
    is_pinned: boolean
    evidence: S03 message list, S04 composer, S07 context menu, TDLib message object
    confidence: high
```

```
MessageMedia
    id: string
    type: MediaType (photo | video | document | audio | voice | sticker)
    file_path: string (local cached path)
    remote_file_id: string
    file_size: int64
    mime_type: string
    width: int32 (optional)
    height: int32 (optional)
    duration: int32 (seconds, for audio/video/voice)
    waveform: bytes (for voice messages)
    thumbnail_path: string (optional)
    evidence: S03 media bubble, S06 shared media drawer, S08 lightbox
    confidence: high
```

```
ChatFolder (Filter)
    id: int32
    title: string
    icon_name: string
    included_chat_types: list<string> (contacts, non_contacts, groups, channels, bots)
    excluded_chat_types: list<string> (muted, read, archived)
    included_chat_ids: list<int64>
    excluded_chat_ids: list<int64>
    pinned_chat_ids: list<int64>
    evidence: S02 left folder bar, S12 folders settings, TDLib chatFilter object
    confidence: high
```

```
ActiveSession
    id: int64
    api_id: int32
    device_model: string
    platform: string
    system_version: string
    app_name: string
    app_version: string
    ip_address: string
    country: string
    created_at: timestamp
    last_active_at: timestamp
    is_current: boolean
    evidence: S12 settings -> Devices / Active Sessions
    confidence: high
```

Relationships:
- `User 1-n Chat` (one user participates in many chats; 1-1 or via group membership)
- `Chat 1-n Message` (a chat owns a chronological sequence of messages)
- `Message 0-1 Message` (self-referential relationship via `reply_to_message_id`)
- `Message 0-1 MessageMedia` (or 1-n for grouped albums)
- `Message 1-n ReactionCount`
- `User 1-n ChatFolder` (user customizes personal chat filter tabs)
- `ChatFolder n-m Chat` (chats belong to multiple folders matching filter criteria)
- `User 1-n ActiveSession` (user account authorized on multiple devices)

## Feature matrix

See `features.csv`.
- Must: 25
- Should: 18
- Could: 5
- Skip: 5
- Total items: 53

## Out of scope (cannot or should not be cloned)

- **Telegram Proprietary Server Infrastructure**: MTProto backend cluster, distributed datacenters, internal routing. (Replaced by connecting directly to Telegram's public MTProto endpoints via TDLib or custom client key).
- **Telegram Stories Desktop Studio**: Heavy video recording/editing mobile camera pipeline. (Desktop view is mostly read-only; skipped for desktop core).
- **Telegram Mini Apps (TMA / WebGL)**: Full embedded Chromium/CEF engine required to run HTML5 games and crypto web apps. (Heavyweight and outside the core desktop messaging scope).
- **TON Crypto Wallet & Fragment Auctions**: Web3 blockchain integration, seed phrase storage, custodial payments, and domain auctions. (External financial service).
- **Telegram Sponsored Ads Engine**: In-channel promotional ad injection platform managed server-side.

## Size

Screens: 14, Flows: 8, Entities: 6.
Hard parts:
1. **Real-time MTProto / TDLib synchronization**: Handling sequence gaps, local optimistic updates vs server message ACKs, reconnects, and flood-wait backoffs.
2. **Rich Text Formatting & Custom Spoilers/Entities**: Parsing, validating, and rendering overlapping markdown spans (bold, italic, code, blockquote, spoiler, custom emoji) across desktop rendering frameworks (or ANSI/terminal cells for TUI).
3. **Chunked Media Upload/Download & Audio Waveform Rendering**: Handling 2GB+ streaming transfers, resume tokens, caching strategies, and voice Opus decoding/waveform synthesis.

Size: **M (a few weeks)** for a dedicated TUI / lightweight desktop client powered by TDLib; **L (a quarter)** for a full C++/Qt graphical clone.
