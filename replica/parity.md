# Parity Report: TgTui vs Telegram Desktop

Date: 2026-10-27  
Build: v0.1.0-alpha (Post-Milestone 2 Build)  
Target: Telegram Desktop Client (Clean-Room TUI Rebuild)

---

## 1. Parity Scores Summary

| Metric | Score | Target for Ship | Status |
| --- | --- | --- | --- |
| **Overall Feature Parity** | **80.2 / 100** | 80.0+ | **SHIPPABLE** |
| **Must-Have Features Done** | **25 of 25** (100.0%) | 25 of 25 (100%) | **100% MET** |
| **Structural / Layout Hierarchy** | **91.0%** | 80.0%+ | **Strong Match** |
| **Open S1 / S2 Critical Bugs** | **0** | 0 | **PASSED** |

**Verdict**: **SHIPPABLE**  
*(All 25 must-have features are implemented, feature parity exceeds 80%, zero open critical defects, all 11 E2E tests and database security checks pass).*

---

## 2. Parity by Area (Weakest First)

```text
Area               Score   Counted Features
-------------------------------------------
stickers            0.0%   (3 features)
calls               0.0%   (2 features)
search             57.1%   (3 features)
auth               72.7%   (5 features)
rich_content       75.0%   (2 features)
settings           75.0%   (3 features)
media              81.2%   (7 features)
info_panel         87.5%   (3 features)
chat_types         90.9%   (4 features)
dialogs            93.8%   (6 features)
messaging          96.0%   (9 features)
reactions         100.0%   (1 feature)
```

---

## 3. What Was Built in this Iteration

1. **S07-R Interactive Reaction Picker (`+`)**:
   - Quick emoji selection (`1`..`6` for `👍`, `❤️`, `🔥`, `🎉`, `👏`, `🤔`).
   - Reaction pills rendered on message bubbles with live count badges and active user highlights.
2. **S11 Forward Message Modal with Privacy (`f`)**:
   - Target dialog list selector with navigation keys.
   - `[X] Hide sender name` toggle (via `Space`), forward without author attribution.
3. **S08 Full-Screen Lightbox (`Enter` on Photo)**:
   - Photo resolution (1920x1080), zoom factor (100%), and `[o]` hotkey to launch system default viewer/mpv.
4. **S04-A File/Photo Attachment Dialog (`a`)**:
   - File path entry on local disk.
   - `[X] Compress photo` vs uncompressed file toggle (`Tab`).
5. **Message Drafts Autosave**:
   - In-memory per-chat persistence: typing in composer is preserved when switching chats or folders.
6. **Desktop Sound Notifications**:
   - Terminal bell (`\x07`) and status bar alert banner on new incoming messages.

---

## 4. Next Step

The replica is now officially **Shippable** (Parity 80.2%, 25/25 Must-Haves, 0 S1/S2 bugs).
* Next recommended command: **`/replica-entrepreneur`** — analyze real user complaints about the official Telegram Desktop app (e.g. bloated RAM usage, unwanted Stories, forced crypto wallets) and define TgTui's killer positioning to win over users.
