# Pre-Mortem Red-Team Report: TelegramTUI (v1.0.0)

> **Premise**: *It is 12 months post-release (October 2027). The v1.0.0 release of TelegramTUI, despite initial excitement, experienced significant user churn, issue complaints, and abandonment. We work backward to understand exactly why and how it failed, attack the underlying assumptions, and establish monitoring and mitigations.*

---

## 1. Intake: Plan and Decision

- **Title**: TelegramTUI (tg-tui) v1.0.0 Production Deployment and User Adoption
- **Decision**: Ship v1.0.0 publicly on GitHub as a daily-driver terminal client, claim production stability, and promote it as an ultra-fast, lightweight alternative to official Telegram Desktop.
- **Horizon**: 12 months post-release.

---

## 2. Load-Bearing Assumptions & Red-Team Attack

| ID | Assumption Statement | Load-Bearing? | The Red-Team Attack |
|---|---|:---:|---|
| **A1** | *Crossterm and Ratatui will reliably handle all terminal resize events, panics, signal interrupts (SIGINT/SIGWINCH), and multi-byte UTF-8 without corrupting the user's terminal or crashing.* | **YES** | **Inversion**: In `src/actors/ui.rs`, there is no custom panic hook (`std::panic::set_hook`). Any unexpected panic terminates the binary before restoring raw terminal mode and leaving the alternate screen. The user's terminal is left completely bricked (invisible cursor, no key echo, broken Enter key). |
| **A2** | *The client will stay strictly under 25MB RAM in continuous long-running daily usage without memory leaks or unbounded growth.* | **YES** | **Inversion**: In `src/actors/coordinator.rs`, `self.messages.entry(chat_id).or_default().push(msg)` appends every single incoming channel and group message into an in-memory vector without bounds, eviction, or SQLite offload. Over 2 weeks in active channels, memory usage balloons past 800MB, triggering the Linux OOM killer. |
| **A3** | *The Tokio async event loop and Crossterm input polling will maintain snappy, latency-free (<16ms) typing without keystroke drops under load.* | **YES** | **Inversion**: In `src/actors/ui.rs`, keyboard polling is throttled by `sleep(50ms)` in the `select!` branch. During high-throughput network bursts, network events starve the keyboard poll, causing input lag and dropped double-keystrokes. |
| **A4** | *Message history viewport and scrolling logic will naturally present conversations so incoming updates and navigation are immediately visible.* | **YES** | **Inversion**: In `src/ui/messages.rs`, `Paragraph::new(lines)` renders message bubbles starting from the top (oldest messages) with zero auto-scroll to the bottom. In chats with >20 messages, new messages arrive off-screen and are completely invisible. |
| **A5** | *The client will gracefully handle network dropouts, sleep/wake cycles, and MTProto reconnection without silent message loss.* | **YES** | **Inversion**: The coordinator lacks connection state tracking (`Connecting`, `Updating`, `WaitingForNetwork`). When WiFi drops, messages sent by the user remain stuck in pending state indefinitely with no retry queue or offline banner. |
| **A6** | *Notification bells and alerts will not cause visual tearing or acoustic spam during high-volume channel activity.* | NO | **Inversion**: `print!("\x07")` fires on every single incoming message directly to stdout while Crossterm double-buffers the Ratatui frame, creating visual frame tearing and continuous buzzer screeching in busy channels. |
| **A7** | *The single-line composer widget is sufficient for real-world messages without clipping, text overflow, or editing frustration.* | **YES** | **Inversion**: The composer widget does not implement horizontal scrolling offset (`scroll_x`). Typing more than ~50 characters clips the text past the right border, hiding what the user is typing. |

---

## 3. Ranked Failure Modes

Priority is calculated as `Likelihood (1–5) × Impact (1–5)`:

| # | Priority | Failure Mode | Breaks | L | I | Summary of Failure Story |
|---|:---:|---|:---:|:---:|:---:|---|
| **1** | **20** (Critical) | **FM-01: Terminal State Corruption on Panic** | **A1** | 4 | 5 | An unexpected panic (e.g. terminal resized to 0 lines or malformed Unicode) kills the app while in raw mode, completely locking up the user's terminal session. |
| **2** | **20** (Critical) | **FM-02: Message History Autoscroll Blindness** | **A4** | 5 | 4 | History view is pinned to the top of the chat history; users cannot see new incoming messages at the bottom of the screen. |
| **3** | **16** (Critical) | **FM-03: Long-Session Memory Leak from Unbounded Vectors** | **A2** | 4 | 4 | Running in a detached tmux session for days accumulates thousands of messages in memory, causing RAM to surge to hundreds of megabytes. |
| **4** | **15** (Critical) | **FM-04: Composer Text Clipping & Overflow on Long Messages** | **A7** | 5 | 3 | Long messages, code snippets, or URLs exceed terminal width and disappear into the right border without horizontal scrolling. |
| **5** | **12** (Moderate) | **FM-05: Input Stutter Under Bulk Network Sync** | **A3** | 3 | 4 | High-volume message synchronization starves the 50ms Crossterm polling tick, introducing noticeable typing stutter. |
| **6** | **12** (Moderate) | **FM-06: Terminal Redraw Tearing from Raw Bell Character** | **A6** | 4 | 3 | Raw `\x07` prints desynchronize the alternate screen double-buffer during active channel bursts. |
| **7** | **12** (Moderate) | **FM-07: Silent Message Drop on Network Disconnect** | **A5** | 3 | 4 | Outgoing messages get stuck in `Pending` with no offline status banner or persistent outbox retry queue. |

---

## 4. Monitoring Thresholds & Mitigation Packages

| Failure Mode | Leading Indicator | Concrete Threshold | Immediate Response | Mitigation Package | Owner |
|---|---|---|---|---|---|
| **FM-01** | Issues reporting "terminal frozen" or "cursor gone" | $\ge 1$ report in first 30 days | Deploy patch with global panic handler | Install `std::panic::set_hook` in `src/main.rs` that explicitly executes `disable_raw_mode()` and `LeaveAlternateScreen` before printing the backtrace. | Core Engine |
| **FM-02** | Reports stating "cannot see new messages" | $\ge 2$ complaints within 7 days | Push viewport auto-scroll fix | Implement auto-scroll pinning to bottom (`scroll_y = total_lines.saturating_sub(height)`), and synchronize cursor selection to visible area. | UI Layer |
| **FM-03** | Resident Set Size (RSS) memory after 72h idle run | Process RSS $> 50$ MB | Enforce sliding window memory cap | Cap in-memory history to 100 messages per chat; offload older history to `schema.sql` SQLite database and paginate dynamically on scroll-up. | Coordinator |
| **FM-04** | User reports of truncated input in composer | $\ge 1$ complaint on GitHub issues | Add horizontal scroll offset | Calculate `scroll_offset_x` based on cursor position and integrate `unicode-segmentation` grapheme clusters for safe editing. | Composer |
| **FM-05** | Input-to-render latency during benchmark sync | Latency $> 32$ ms | Move input reader to dedicated task | Run `crossterm::event::read` on a dedicated background thread with `tokio::select! biased;` prioritizing user input. | Architecture |
| **FM-06** | Frame flickering or audio bell complaints | $\ge 1$ report in issue tracker | Throttle bell & route via config | Add `notification_bell` toggle to `AppConfig`, throttle bell to $\le 1$ per 5s, and avoid raw unbuffered stdout writes. | Coordinator |
| **FM-07** | Messages stuck in `Pending` $> 30$s | Pending duration $> 15$s on degraded net | Show offline banner & outbox queue | Add `ConnectionState` enum (`Connected`, `Connecting`, `Offline`) with header banner and persistent outbox retry loop. | Network |

---

## 5. Residual Risk

- **Platform Escape Codes**: Minor variations in ANSI escape sequences between modern terminals (Alacritty, Kitty, WezTerm) and legacy Windows conhost.
- **TDLib Native Dynamic Linkage**: Distribution-specific glibc/musl variations when connecting to live TDLib shared libraries in advanced deployments.
