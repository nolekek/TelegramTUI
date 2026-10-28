# TelegramTUI Test Infrastructure Specification (TEST_INFRA.md)

**Document Version:** 1.0.0  
**Project:** TelegramTUI (tg-tui)  
**Scope:** E2E Testing Track & 4-Tier Automated Verification Architecture  
**Target:** Requirements R1 through R5 (Milestones M1–M5)  

---

## 1. Architecture Overview

TelegramTUI's test infrastructure is designed for **deterministic, opaque-box, requirement-driven verification** of the entire terminal client. Because terminal emulators and MTProto networks exhibit non-deterministic external behaviors, the testing harness models the client's internal Actor architecture asynchronously while verifying observable side effects.

### 1.1 Architecture Stack
```
+------------------------------------------------------------------------+
|                     4-Tier E2E Test Suite                              |
|   Tier 1: Feature | Tier 2: Boundary | Tier 3: Pairwise | Tier 4: Real |
+------------------------------------------------------------------------+
                                    |
            +-----------------------+-----------------------+
            |                                               |
            v                                               v
+------------------------+                      +------------------------+
|    AppCoordinator      | <==================> |      TdlibActor        |
|  - Auth State Machine  |     TdlibCommand     |  - Simulation Mode     |
|  - Focus & Navigation  |                      |  - State Broadcasting  |
|  - Composer Buffer     | <==================> |  - Message Lifecycle   |
|  - Modal Intercepts    |      TdlibEvent      |  - Latency / ACK Sim   |
+------------------------+                      +------------------------+
            |
            v
+------------------------+
|   Ratatui TestBackend  |
|  - In-Memory Cell Grid |
|  - Headless Rendering  |
|  - Layout & Wrapping   |
|  - Zero-Panic Checks   |
+------------------------+
```

### 1.2 Core Verification Components
1. **Asynchronous Actor Test Harness (`setup_test_app`, `setup_unauth_app`)**:
   - Spawns isolated Tokio MPSC communication channels between `AppCoordinator` and `TdlibActor`.
   - Supports both instant seed mode (starts authenticated) and clean unauthenticated mode (starts at S01 PhoneInput).
   - Simulates MTProto delivery acknowledgments (`Pending` -> `Sent` -> `Read`).
2. **Headless Terminal Pipeline (`ratatui::backend::TestBackend`)**:
   - Renders frames to in-memory buffers without requiring a physical TTY or ConHost instance.
   - Evaluates terminal geometry stresses (1x1, 0x0, 200x60) without corrupting the developer's console.
3. **Configuration & Persistence Isolation**:
   - Exercises `AppConfig` validation, serialization, and environment variable overrides safely without mutating user disk state.

---

## 2. 4-Tier Test Taxonomy

The test suite follows a structured 4-tier methodology to ensure 100% requirement coverage with progressive depth:

| Tier | Name | Target Scope | Criteria | Minimum Test Count |
|---|---|---|---|---|
| **Tier 1** | **Feature Coverage** | Requirements R1–R5 isolation | Happy-path isolation tests for every feature | >= 5 tests per feature (>= 25 total) |
| **Tier 2** | **Boundary & Corner Cases** | Edge conditions, limits, overflows | Empty inputs, max sizes, invalid codes, rapid resizes, zero dimensions | >= 5 tests per feature (>= 25 total) |
| **Tier 3** | **Cross-Feature Combinations** | Pairwise subsystem interactions | Concurrency, modal interleaving, navigation during streaming, state sync | >= 5 tests |
| **Tier 4** | **Real-World Application Scenarios** | End-to-end user journeys | Multi-step interactive workflows (first launch -> login -> chat -> compose -> send) | >= 5 tests |

---

## 3. Feature Inventory to Test Mapping Matrix

This matrix maps every architectural feature from `PROJECT.md` and `ORIGINAL_REQUEST.md` to specific automated test cases in `tests/e2e_requirements_tests.rs`:

| Feature ID | Requirement | Feature Description | Primary Test Case | Tier |
|---|---|---|---|---|
| **F1.1** | R1 | `KeyEventKind::Press` Filter | `test_tier1_r1_single_press_event_processed` | Tier 1 |
| **F1.1** | R1 | Discard Release / Repeat | `test_tier1_r1_release_and_repeat_discard_contract` | Tier 1 |
| **F1.2** | R1 | Input Reader Dispatch Latency | `test_tier1_r1_navigation_keys_dispatch_once` | Tier 1 |
| **F1.3** | R1 | Biased Select / Keystroke Priority | `test_tier1_r1_input_prioritization_over_network` | Tier 1 |
| **F1.4** | R1 | Navigation Key Mapping (`j`/`k`, Tab) | `test_tier1_r1_cycle_focus_tab_dispatch_once` | Tier 1 |
| **F2.1** | R2 | S00 First-Launch Interactive Setup | `test_tier1_r2_config_validation_valid_credentials` | Tier 1 |
| **F2.2** | R2 | Config Validation & .env Support | `test_tier1_r2_config_save_and_load_roundtrip` | Tier 1 |
| **F2.4** | R2 | E.164 Phone Number Input & Format | `test_tier1_r2_phone_number_submission_and_step_advancement` | Tier 1 |
| **F2.5** | R2 | Verification Code Input (SMS/App) | `test_tier1_r2_verification_code_submission` | Tier 1 |
| **F2.6** | R2 | Conditional 2FA Cloud Password | `test_tier1_r2_cloud_password_2fa_submission` | Tier 1 |
| **F3.4** | R3 | Chat & Message Initial Sync | `test_tier1_r3_chat_and_message_initial_sync` | Tier 1 |
| **F3.4** | R3 | Optimistic Message Delivery Lifecycle | `test_tier1_r3_optimistic_message_delivery_lifecycle` | Tier 1 |
| **F3.4** | R3 | Reaction Update Propagation | `test_tier1_r3_reaction_update_propagation` | Tier 1 |
| **F3.4** | R3 | Message Edit Propagation | `test_tier1_r3_message_edit_propagation` | Tier 1 |
| **F3.4** | R3 | Message Delete Propagation | `test_tier1_r3_message_delete_propagation` | Tier 1 |
| **F4.1** | R4 | Message History Navigation Bounds | `test_tier1_r4_message_navigation_next_prev_bounds` | Tier 1 |
| **F4.3** | R4 | Composer Character Accumulation | `test_tier1_r4_composer_character_accumulation` | Tier 1 |
| **F4.3** | R4 | Composer Backspace Handling | `test_tier1_r4_composer_backspace_handling` | Tier 1 |
| **F4.3** | R4 | Composer Long Text Rendering (80+ chars) | `test_tier1_r4_composer_long_text_rendering` | Tier 1 |
| **F4.1** | R4 | History Multi-Message Bubble Rendering | `test_tier1_r4_history_rendering_with_many_messages` | Tier 1 |
| **F5.1** | R5 | Clean Loop Termination on Quit | `test_tier1_r5_quit_action_terminates_loop` | Tier 1 |
| **F5.2** | R5 | Zero-Dimension Resize Resilience (0x0) | `test_tier1_r5_render_zero_dimension_resilience` | Tier 1 |
| **F5.2** | R5 | Rapid Dynamic Resize Stability | `test_tier1_r5_rapid_resize_render_stability` | Tier 1 |
| **F5.1** | R5 | Terminal Panic Safety & Catch Unwind | `test_tier1_r5_panic_safety_hook_contract` | Tier 1 |
| **F5.3** | R5 | Unauthenticated Clean Teardown | `test_tier1_r5_clean_teardown_unauthenticated_quit` | Tier 1 |
| **F1.1** | R1 (B) | Rapid Successive Keystrokes (100 keys) | `test_tier2_r1_b1_rapid_successive_keypresses` | Tier 2 |
| **F1.1** | R1 (B) | Multi-Byte UTF-8 Unicode Input | `test_tier2_r1_b2_non_ascii_unicode_input` | Tier 2 |
| **F1.4** | R1 (B) | Unmapped Key Graceful Ignore | `test_tier2_r1_b3_unmapped_keys_ignored_safely` | Tier 2 |
| **F1.4** | R1 (B) | Ctrl+C Global Intercept Under Modal | `test_tier2_r1_b4_modifier_combinations_ctrl_c` | Tier 2 |
| **F1.4** | R1 (B) | Rapid Focus Cycling Loop Wrap | `test_tier2_r1_b5_focus_cycling_full_loop` | Tier 2 |
| **F2.4** | R2 (B) | Empty Phone Number Rejection | `test_tier2_r2_b1_empty_phone_input_rejection` | Tier 2 |
| **F2.5** | R2 (B) | Whitespace Trimming in OTP Code | `test_tier2_r2_b2_whitespace_in_verification_code` | Tier 2 |
| **F2.6** | R2 (B) | Empty 2FA Password Rejection | `test_tier2_r2_b3_empty_2fa_password_rejection` | Tier 2 |
| **F2.2** | R2 (B) | Non-Positive `api_id` Rejection | `test_tier2_r2_b4_invalid_api_id_negative_or_zero` | Tier 2 |
| **F2.2** | R2 (B) | Invalid Hex `api_hash` Rejection | `test_tier2_r2_b5_invalid_api_hash_hex_length` | Tier 2 |
| **F3.4** | R3 (B) | Whitespace-Only Message Ignored | `test_tier2_r3_b1_send_empty_or_whitespace_message` | Tier 2 |
| **F3.4** | R3 (B) | Non-Existent Chat ID Message Lookup | `test_tier2_r3_b2_unknown_chat_message_lookup` | Tier 2 |
| **F3.4** | R3 (B) | Edit Non-Existent Message ID | `test_tier2_r3_b3_message_edit_nonexistent_id` | Tier 2 |
| **F3.4** | R3 (B) | Delete Non-Existent Message ID | `test_tier2_r3_b4_message_delete_nonexistent_id` | Tier 2 |
| **F3.4** | R3 (B) | Repeated Emoji Reaction Toggle | `test_tier2_r3_b5_duplicate_reaction_toggle` | Tier 2 |
| **F4.1** | R4 (B) | Empty Chat Navigation Safety | `test_tier2_r4_b1_navigate_empty_chat_history` | Tier 2 |
| **F4.3** | R4 (B) | Backspace on Empty Composer Buffer | `test_tier2_r4_b2_composer_backspace_on_empty_buffer` | Tier 2 |
| **F4.3** | R4 (B) | Extreme Message Length (1000+ Chars) | `test_tier2_r4_b3_composer_extreme_length_1000_chars` | Tier 2 |
| **F4.2** | R4 (B) | Single Message Chat Bounds Clamp | `test_tier2_r4_b4_navigate_single_message_chat` | Tier 2 |
| **F4.3** | R4 (B) | Per-Chat Draft Preservation | `test_tier2_r4_b5_draft_retention_across_chat_switching` | Tier 2 |
| **F5.2** | R5 (B) | Extreme Narrow Terminal Width (1 col) | `test_tier2_r5_b1_render_extreme_narrow_width_1` | Tier 2 |
| **F5.2** | R5 (B) | Extreme Short Terminal Height (1 row) | `test_tier2_r5_b2_render_extreme_short_height_1` | Tier 2 |
| **F5.2** | R5 (B) | Huge Terminal Geometry (500x500) | `test_tier2_r5_b3_render_huge_viewport_500x500` | Tier 2 |
| **F5.1** | R5 (B) | Catch Unwind Panic Boundary Isolation | `test_tier2_r5_b4_catch_unwind_around_render` | Tier 2 |
| **F5.3** | R5 (B) | Clean Teardown With Active Modal | `test_tier2_r5_b5_modal_open_during_quit` | Tier 2 |
| **Pairwise** | R1×R3 | Input Navigation During Streaming Messages | `test_tier3_pairwise_input_navigation_while_receiving_messages` | Tier 3 |
| **Pairwise** | R2×R3 | Auth Failure, Backspace Retry, and Success | `test_tier3_pairwise_auth_failure_with_retry_and_event_interleaving` | Tier 3 |
| **Pairwise** | R1×R4 | Typing While Status Notification Active | `test_tier3_pairwise_composer_typing_during_status_notification` | Tier 3 |
| **Pairwise** | R3×R4 | Reply Context Preserved Under Background Msg | `test_tier3_pairwise_reply_to_message_then_receive_new_message` | Tier 3 |
| **Pairwise** | R4×R1 | Folder Switch With Active Draft & Modal | `test_tier3_pairwise_folder_switch_with_composer_draft_and_modal` | Tier 3 |
| **Scenario** | E2E | Scenario 1: First Launch to Authenticated Shell | `test_tier4_scenario_first_launch_to_authenticated_shell` | Tier 4 |
| **Scenario** | E2E | Scenario 2: Chat Navigation & Lightbox Modal | `test_tier4_scenario_chat_navigation_and_message_reading` | Tier 4 |
| **Scenario** | E2E | Scenario 3: 80+ Character Compose & Server ACK | `test_tier4_scenario_compose_long_message_80_chars_and_send_ack` | Tier 4 |
| **Scenario** | E2E | Scenario 4: Reply, Edit, and Reaction Lifecycle | `test_tier4_scenario_reply_edit_reaction_workflow` | Tier 4 |
| **Scenario** | E2E | Scenario 5: Forward With Privacy & Media Attachment | `test_tier4_scenario_forward_with_privacy_and_attachment` | Tier 4 |

---

## 4. Pass / Fail Semantics

Every test in this suite adheres strictly to deterministic invariants:

1. **Explicit Assertions**:
   - Status codes and enums (`AuthStep::Authenticated`, `DeliveryStatus::Sent`, `DeliveryStatus::Read`).
   - Collection invariants: chat list count, message indices, reactions counts.
   - Text exactness: buffer equality, snippet formatting, error messages.
2. **Deterministic Timeouts**:
   - Simulated async operations are bound by maximum timeout windows (`tokio::time::sleep(20ms..800ms)`).
   - Any deadlocked channel or starved loop will fail within 5 seconds.
3. **No Facade Tests**:
   - Tests do not assert trivial tautologies (`assert!(true)`). Every test exercises actual coordinator and actor message handling logic and inspects resultant state.
4. **Panic Isolation**:
   - Rendering tests must never panic or unwrap on unexpected terminal dimensions (e.g. 0x0 or 1x1).

---

## 5. Test Runner Instructions

### 5.1 Run the Full Test Suite
To compile and execute all unit tests, existing e2e flows, and the full 4-tier requirement suite:
```powershell
cargo test
```

### 5.2 Run Only the 4-Tier E2E Requirements Suite
To execute only the comprehensive 60-test E2E requirements suite:
```powershell
cargo test --test e2e_requirements_tests
```

### 5.3 Run by Specific Tier
```powershell
# Tier 1: Feature Coverage (25 tests)
cargo test --test e2e_requirements_tests test_tier1

# Tier 2: Boundary & Corner Cases (25 tests)
cargo test --test e2e_requirements_tests test_tier2

# Tier 3: Cross-Feature Combinations (5 tests)
cargo test --test e2e_requirements_tests test_tier3

# Tier 4: Real-World Scenarios (5 tests)
cargo test --test e2e_requirements_tests test_tier4
```

### 5.4 Run by Specific Requirement
```powershell
# R1: Crossterm Input & Event Loop
cargo test --test e2e_requirements_tests r1

# R2: Authentication Gate & Config Setup
cargo test --test e2e_requirements_tests r2

# R3: TDLib Backend Integration
cargo test --test e2e_requirements_tests r3

# R4: History Viewport & Composer
cargo test --test e2e_requirements_tests r4

# R5: Terminal Safety & Teardown
cargo test --test e2e_requirements_tests r5
```

### 5.5 Run With Output Logging
To observe live status banners, transitions, and test output:
```powershell
cargo test --test e2e_requirements_tests -- --nocapture
```
