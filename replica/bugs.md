# Bug Reports: TgTui

Bug tracking log according to replica-test specifications.

---

### BUG-001: Rust borrow collision in AppCoordinator during message reply and edit

- Severity: S2
- Flow / case: F03 / F03-H1 & F04 / F04-H1
- Screen: S04 / S07
- Build: v0.1.0-alpha  Browser / device: Windows x64 Terminal

Steps:
1. Call `app.handle_user_action(UserAction::StartReply)` or `UserAction::StartEdit`.
2. Access `self.current_messages()` which borrows `&self`.
3. Simultaneously attempt to assign `self.composer_reply_to = Some(...)`.

Expected: Action updates coordinator state without compiler borrow conflict.
Actual: `cannot assign to self.composer_reply_to because it is borrowed` (E0506).
Evidence: Compiler log during `cargo check`.
Suspected cause: Retaining `&Message` reference into `current_messages()` while mutating coordinator fields.
Status: Fixed in commit `build: fix coordinator borrow collisions by cloning reply/edit parameters`.

---

### BUG-002: Race condition in capture_screens headless snapshot generation

- Severity: S3
- Flow / case: F02 / S02
- Screen: S02 / S03
- Build: v0.1.0-alpha  Browser / device: Headless TestBackend

Steps:
1. Spawn `TdlibActor` in separate Tokio task.
2. Immediately call `event_rx.try_recv()` before background actor yields its first loop tick.
3. Draw terminal frame to snapshot file.

Expected: Screen snapshot renders populated seed chats and folders.
Actual: Snapshot rendered empty ("No chats in this folder") because the spawned task had not dispatched initial events.
Evidence: `replica/clone-screens/S02_S03_S04_main_shell.txt` initial empty run.
Suspected cause: Missing await/yield on async event initialization.
Status: Fixed in commit `test: add async yield and drain barrier in capture_screens`.
