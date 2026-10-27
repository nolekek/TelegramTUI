# TgTui Build Log

Record of screens, flows, and components built during `/replica-build`.

| Screen ID | Screen Name | Date | Status | What is Missing / What Was Harder Than Expected |
| --- | --- | --- | --- | --- |
| S01 | Auth & 2FA Gate | 2026-10-27 | Done | Phone input, SMS OTP code verification, and Cloud Password 2FA implemented. |
| S02 | Main Shell & Dialogs List | 2026-10-27 | Done | Folder tabs (All, Personal, Groups, Channels) with numeric hotkeys `1`..`4`, unread badges, pin icons `📌`, mute icons `🔇`, and last message preview. |
| S03 | Active Chat History | 2026-10-27 | Done | Message bubbles with sender names, timestamps, optimistic delivery ticks (`⏱` -> `✓` -> `✓✓`), reply quote previews, and ASCII audio waveform (` ▂▃▅▆▇▆▅▃▂ `). |
| S04 | Message Composer | 2026-10-27 | Done | Multi-line text input with typing cursor, reply banner, in-place message edit mode, optimistic local dispatch, markdown formatting, and draft autosave per chat. |
| S05 | Chat Profile Panel | 2026-10-27 | Done | Modal drawer showing avatar/bio, username, online status, and shared media breakdown (Photos, Files, Voice, Links). |
| S06 | Shared Media Drawer | 2026-10-27 | Done | Integrated within S05 drawer with item counts and quick file navigation. |
| S07 | Message Context Menu | 2026-10-27 | Done | Modal popup triggered via `m` or right-click: Reply, Edit, Forward, Delete, Play audio. |
| S07-R | Reaction Picker Modal | 2026-10-27 | Done | Popover triggered via `+` allowing instant emoji reaction attachment (`👍`, `❤️`, `🔥`, `🎉`, `👏`, `🤔`) with live badge counters. |
| S08 | Media Viewer Lightbox | 2026-10-27 | Done | Full-screen image/media inspector with zoom factor, dimension display, and external viewer launcher (`o`). |
| S10 | In-Chat Search Overlay | 2026-10-27 | Done | Interactive search bar overlay triggered via `/` with live query buffer and filter chips. |
| S11 | Forward Message Modal | 2026-10-27 | Done | Target dialog selector with `[X] Hide sender name` privacy toggle (Space). |
| S12 | Settings & Cheatsheet | 2026-10-27 | Done | Full keyboard shortcuts cheat sheet and application configuration modal triggered via `?`. |
| S04-A | Attachment Dialog | 2026-10-27 | Done | File picker input with `[X] Compress photo` vs uncompressed file toggle (`a`). |
