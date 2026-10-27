# Component Specifications: TgTui

Component design specifications and state requirements for TgTui (Telegram Desktop Rebuild). Every repeated UI part is documented with its variants, dimensions, states, token bindings, keyboard accessibility, and screen usage.

---

### FolderSidebarItem
* **variants**: Default, Personal, Work, Channels, Groups, Custom, Settings
* **sizes**: Height: 1-2 lines (TUI cell height); Width: 12-16 cells (collapsed icon mode: 3 cells)
* **states**: default, hover, active/selected, focused, badge-unread (accent badge), badge-muted (grey badge)
* **tokens**: bg surface, text text, active-bg surface-hover, active-indicator accent, badge-bg accent, badge-text on-accent
* **a11y**: Accessible label reading folder name and unread count; keyboard navigation via `Alt+1`..`Alt+9` or vertical arrow keys (`j`/`k`); screen reader role: `tab`
* **used on**: S02

---

### DialogListItem
* **variants**: Direct (1-on-1), Group, Channel, Bot, Forum Supergroup
* **sizes**: 2-3 lines height (Name + Last Message preview + Time/Badge)
* **states**: default, selected/focused, hover, pinned (pinned glyph `📌`), muted (mute glyph `🔇`), typing-indicator, draft-saved
* **tokens**: bg bg, selected-bg surface-hover, title text, snippet text-muted, time text-muted, badge-bg accent, badge-text on-accent, muted-badge-bg border
* **a11y**: Arrow keys `Up`/`Down` (`k`/`j`) to traverse dialog list; `Enter` to open chat; announce "Chat with {title}, {unread_count} unread messages"; screen reader role: `listitem`
* **used on**: S02, S11

---

### MessageBubble
* **variants**: Incoming, Outgoing, System/Date Header, Forwarded
* **sizes**: Dynamic width (min 20 cols, max 70% of pane width); multi-line wrapping
* **states**: pending (clock `⏱`), sent (single check `✓`), read (double check `✓✓`), edited (`(edited)` suffix), selected, focused, replying, error-failed
* **tokens**: incoming-bg surface, outgoing-bg border, text text, text-muted text-muted, checkmarks accent, date-header text-muted
* **a11y**: Screen reader reads sender name, timestamp, message body, and delivery state; `r` to reply, `e` to edit own message, `d` to delete, `Space` to select; screen reader role: `article`
* **used on**: S03

---

### ComposerInput
* **variants**: Normal, Replying, Editing, ReadOnly (Channel / Restricted)
* **sizes**: Dynamic auto-growing height: 1 to 5 lines; full chat width
* **states**: empty/placeholder, focused, typing, with-draft, with-reply-banner, with-edit-banner, disabled (slow-mode countdown / muted)
* **tokens**: bg surface, border border, border-focused border-input, text text, placeholder text-muted, indicator accent
* **a11y**: Standard text editing cursor with UTF-8 / grapheme navigation; `Enter` to send, `Shift+Enter` for newline; `Ctrl+B` for bold, `Ctrl+I` for italic, `Ctrl+K` for code; screen reader role: `textbox`
* **used on**: S04

---

### ReplyEditBar
* **variants**: Reply preview, Edit preview, Forward preview
* **sizes**: 1 line height placed directly above ComposerInput
* **states**: default, hover, closing (`Esc` or `x` button)
* **tokens**: bg surface-hover, border-left accent, title accent, preview-text text-muted, close-button danger
* **a11y**: Pressing `Esc` clears reply/edit mode; screen reader announces "Replying to {author}: {snippet}"
* **used on**: S04

---

### ContextMenu
* **variants**: Message actions, Chat actions, Media actions
* **sizes**: Floating modal window (width: 24-32 cols; height: based on item count)
* **states**: open, item-hover, item-selected, item-disabled
* **tokens**: bg surface, border border-input, text text, selected-item-bg accent, selected-item-text on-accent, danger-item danger
* **a11y**: Opened via `m` or right-click; `Up`/`Down` navigation; `Enter` to trigger; `Esc` to dismiss; trap focus inside menu; screen reader role: `menu`
* **used on**: S07, S02, S03

---

### LightboxViewer
* **variants**: Photo inspector, Video player launcher
* **sizes**: Full-screen overlay or maximum terminal viewport
* **states**: loading-fullres, loaded, error-load, external-playing
* **tokens**: backdrop bg (opaque), border border-input, info-text text, progress-bar accent
* **a11y**: `o` to launch system viewer/mpv; `s` to save to disk; `Esc` or `q` to close overlay; screen reader announces file dimensions and name
* **used on**: S08

---

### AudioWaveformPlayer
* **variants**: Voice note, Audio message, Podcast
* **sizes**: 1 line height embedded in message bubble
* **states**: idle, downloading, playing, paused, completed
* **tokens**: text text, waveform-bar accent, waveform-empty text-muted, time-label text-muted
* **a11y**: ASCII bars ` ▂▃▅▆▇` indicating voice amplitude; `p` to play/pause via audio daemon; `[` / `]` to seek 5s back/forward
* **used on**: S03, S06

---

### StickerPickerTab
* **variants**: Emoji grid, Custom sticker packs, Tenor GIF search
* **sizes**: 12-16 lines height popover pane
* **states**: active-tab, pack-scrolling, search-filtering, empty-results
* **tokens**: bg surface, border border, active-tab accent, active-tab-text on-accent, search-box border-input
* **a11y**: Arrow key grid navigation; `Tab` to switch categories; `Enter` to insert emoji/send sticker; `Esc` to close
* **used on**: S09

---

### ReactionPill
* **variants**: Single reaction, Multi-reaction bar
* **sizes**: Compact inline pill (width: emoji + 2-3 cols for count)
* **states**: default, chosen-by-me (highlighted border and accent background), incrementing
* **tokens**: bg surface, border border, chosen-bg border, chosen-text accent, count-text text-muted
* **a11y**: Quick reaction via `+` key; screen reader announces "{emoji} count {n}, chosen by you"
* **used on**: S03

---

### SharedMediaTab
* **variants**: Photos grid, Files list, Audio list, Shared links list, Voice list
* **sizes**: Full height of right info drawer (width: 30-40 cols)
* **states**: loading-skeleton, populated, empty-category, downloading-item
* **tokens**: bg surface, tab-active accent, tab-inactive text-muted, filename text, filesize text-muted
* **a11y**: Switch tabs with `h`/`l` or Left/Right; `Enter` to download/open selected item; screen reader role: `tabpanel`
* **used on**: S06

---

### ModalDialog
* **variants**: Auth & 2FA Form, Confirmation (Delete message), Settings Hub, New Chat Wizard
* **sizes**: Centered box (width: 40-60 cols; height: 10-20 lines)
* **states**: open, submitting, field-error, success-closing
* **tokens**: backdrop bg, window-bg surface, border border-input, title text, input-border border-input, submit-btn accent, cancel-btn border
* **a11y**: Focus is trapped within modal; `Tab` cycles inputs; `Enter` submits; `Esc` cancels/closes; screen reader role: `dialog`
* **used on**: S01, S11, S12, S13

---

### SearchBox
* **variants**: Global search, In-chat message search, Picker filter
* **sizes**: 1 line height with leading icon and trailing match count (`1/14`)
* **states**: unfocused, focused, typing, matches-found, no-matches
* **tokens**: bg surface, border border-input, text text, match-highlight warning, match-counter text-muted
* **a11y**: Activated via `Ctrl+F` or `/`; `Enter` / `n` for next match; `Shift+Enter` / `N` for previous match; `Esc` closes search bar
* **used on**: S02, S09, S10, S11

---

### StatusBadge
* **variants**: Unread count, Mention `@`, Pin `📌`, Mute `🔇`, Online bullet `●`
* **sizes**: 1-4 character glyph pill
* **states**: active, muted, zero/hidden
* **tokens**: unread-active-bg accent, unread-active-text on-accent, unread-muted-bg border, mention-bg danger, online-fg success
* **a11y**: Screen reader text: "Unread count: {n}", "You were mentioned"; high-contrast visibility against background
* **used on**: S02, S03, S05
