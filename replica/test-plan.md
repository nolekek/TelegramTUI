# Test Plan: TgTui (Telegram Desktop Rebuild)

Build: v0.1.0-alpha  Date: 2026-10-27  Env: local, seed data & SQLite cache

| case | flow | type | steps | expected | auto | result |
| --- | --- | --- | --- | --- | --- | --- |
| F01-H1 | F01 Auth & Login | happy | Enter valid phone +15550192834 -> submit code 12345 -> submit 2FA secret | Authenticated state, chat list loaded, dialog view rendered | e2e | PASS |
| F01-E1 | F01 Auth & Login | edge: whitespace in code | Enter code with leading/trailing spaces ` 12345 ` | Code trimmed and accepted, moves to 2FA gate | e2e | PASS |
| F01-N1 | F01 Auth & Login | negative: short phone | Enter `123` in phone input and press Enter | Error banner shown: "Phone number too short", stays on S01 | e2e | PASS |
| F01-N2 | F01 Auth & Login | negative: invalid OTP | Enter `99999` in code input and press Enter | Error banner shown: "Invalid code", prompt preserved | e2e | PASS |
| F02-H1 | F02 Send Message | happy | Focus composer, type "Hello **bold**", press Enter | Local message rendered with `Pending` (⏱), then `Sent` (✓), then `Read` (✓✓) | e2e | PASS |
| F02-E1 | F02 Send Message | edge: empty input | Press Enter on empty composer | No empty message sent, composer stays clean | e2e | PASS |
| F02-E2 | F02 Send Message | edge: very long text | Paste 1000 character string and send | Auto-wraps correctly across terminal cells without clipping | e2e | PASS |
| F03-H1 | F03 Reply to Message | happy | Select msg 0 -> press `r` -> compose text -> Enter | New message references replied msg_id with snippet header | e2e | PASS |
| F03-E1 | F03 Reply to Message | edge: cancel reply | Press `r` -> press `Esc` | Reply banner cleared, focus returns to composer/history | e2e | PASS |
| F04-H1 | F04 Edit Message | happy | Select outgoing msg -> press `e` -> edit text -> Enter | Message body updated in place with `(edited)` suffix | e2e | PASS |
| F04-H2 | F04 Delete Message | happy | Select msg -> press `d` | Message removed from conversation history immediately | e2e | PASS |
| F04-N1 | F04 Edit Message | negative: edit incoming | Select other user's message -> press `e` | Status notification: "Can only edit your own messages" | e2e | PASS |
| F05-H1 | F05 Voice Playback | happy | Select voice message bubble -> press `p` | Audio playback launched via external player / status notification | manual | PASS |
| F06-H1 | F06 Forward Message | happy | Select message -> open context menu `m` -> choose forward | Forward dialog opens target selector | manual | PASS |
| F07-H1 | F07 Search in Chat | happy | Press `/` -> type query "Pavel" | In-chat search modal renders with matching query buffer | e2e | PASS |
| F07-E1 | F07 Search in Chat | edge: dismiss search | Press `Esc` in search modal | Modal dismissed cleanly, returns to previous focused pane | e2e | PASS |
| F08-H1 | F08 Folder Switching | happy | Press `1`..`4` to switch folders | Filtered chats list instantly updates matching folder criteria | e2e | PASS |
| F08-E1 | F08 Folder Switching | edge: folder 0 (All) | Select folder 0 | All dialogs displayed without filtering | e2e | PASS |
| SEC-01 | Data Layer Security | negative: invalid enum | Attempt SQL insert with invalid user status | Rejected by SQLite CHECK constraint | script | PASS |
| SEC-02 | Data Layer Security | happy: cascade delete | Delete chat record from database | All associated messages purged via ON DELETE CASCADE | script | PASS |
