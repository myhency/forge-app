# Plan 32B: the shortcut list and new keys (SET-25)

Date: 2026-10-09. Roadmap row: SET-25. Seen in observation round 15 part 3 (`observations.md`): the `단축키` modal of the desktop app (⌘/), about 90 rows in four groups.

## Goal

A `⌘/` modal that lists the keys Forge really has, grouped like the real app, and the missing keys that are cheap and useful, bound for real.

Success: every row in the modal works, and every key Forge binds is in the modal. No dead rows.

## Decisions made with the user

- Scope is "list and bind": new keys are bound this round, but only those that map to features Forge has.
- Panel toggles are included; the composer pickers (⇧⌘M, ⇧⌘I, ⇧⌘E) are not.
- Left out because Forge has no such feature: iOS simulator, voice, split panes, side chat, fast mode, pin (⌥⌘P), read/unread (⌥⌘U), open PR (⌥⌘G), copy session link, queue a message (⌘Enter), browser-tab keys, element picking.

## One table

`shortcut.rs` gets a table of rows `{ group, label, keys, action }`. The same table decides which action a key press is and what the modal draws. A key that is not in the table is not matched and not shown.

- `keys` is a list of alternatives; each is a physical key (`Code`), the modifiers, and a platform rule (Cmd on macOS, Ctrl elsewhere).
- Matching uses the physical key (`event.code()`), not the typed character. On macOS ⌥ changes the character (⌥P gives π) and the Korean IME changes it too (Plan 15 lesson).
- Display text per platform: macOS shows ⌘ ⇧ ⌥ ⌃; other platforms show `Ctrl+Shift+Alt`.
- `shortcut_for` is rewritten over a `KeyPress { code, ctrl, meta, alt, shift }`; the existing tests keep their cases and move to the new input. `Shortcut` gets a variant per new action.
- `shortcut_allowed` stays: while Settings, the search palette or the shortcut modal is open, only the keys that belong to that overlay act (opening Settings, ⌘/ to close the modal).

## The rows

General:

| Keys | Action |
| --- | --- |
| ⌘K | search sessions (has) |
| ⌘N | new session (has) |
| ⇧⌘N | new session page with the active session's folder chosen |
| ⌘W | close the session (has) |
| ⌘, | settings (has) |
| ⌘/ | open or close this list |
| ⌘B | hide or show the sidebar (`prefs.hidden`, saved like the button) |
| ⌃Tab, ⇧⌘], ⌥↓ | next session (⌃Tab has) |
| ⌃⇧Tab, ⇧⌘[, ⌥↑ | previous session (⌃⇧Tab has) |
| ⌘1…9 | go to the n-th open session, in sidebar order; nothing if fewer are open |
| ⇧⌘T | reopen the last closed session |
| ⌥⌘A | archive or unarchive the active session |
| ⌥⌘O | fork the active session |
| ⌥⌘R | rename the active session |

Panels:

| Keys | Action |
| --- | --- |
| ⇧⌘D | Changes |
| ⇧⌘F | Files |
| ⌘J, ⌃\` | Terminal |
| ⇧⌘B | Preview (the real app's "browser") |

Each panel key calls `DockState::toggle` for the active session and does nothing on the new-session, Customize and Routines pages.

## Behaviour details

- **⇧⌘N** is a Forge guess: the real app's "start with the current settings" was not observed. It opens the new-session page with the active session's project already chosen. If a later observation shows otherwise, change this row.
- **⇧⌘T**: `Workspace::close` pushes `(project, claude session id)` of the closed session onto an in-memory stack (kept for the run of the app, at most 20). The key pops one and calls `open_past`. A session that never got an id (no message sent) is not pushed. An empty stack does nothing.
- **⌥⌘R**: `Workspace` gets a `rename_request` signal, the same pattern as `file_request`. The key sets it for the active session's row; the sidebar row consumes it by entering its rename mode and clears it. If the session has no row (no id yet), nothing happens.
- **⌥⌘A, ⌥⌘O**: call `archive_session` / `fork_session` for the active session, under the same conditions as the row menu (`archive` and `fork` actions available; the session has an id). The row menu's single keys (R, F, A, D) stay as they are.
- **⌥↓ / ⌥↑** inside a text field would move the caret in other apps. As the real app does, they cycle sessions; the key press is handled (and default-prevented) only when the active page has sessions to cycle.
- **⌘B** also works when the sidebar is the overlay of a narrow window (Plan 26): it opens and closes the overlay.
- Keys act only when the workspace root has the key event; the composer and menus that stop propagation keep their own keys.

## The modal

- `ShortcutsDialog` in `forge-widgets`; state `Settings`-style in `forge-features` (`shortcuts_open`), toggled by ⌘/ and closed by Escape or ×. Same dimmed background and frame as the Settings overlay.
- Title "Keyboard shortcuts"; groups "General" and "Panels"; a row is the name at the left and the keys at the right, `or` between alternatives. Built from the table, so it needs no list of its own.
- Entry points: the key, and a "Keyboard shortcuts" link at the bottom of the Settings overlay (Forge has no account menu yet, PG-08).
- The composer group is not drawn: it would be empty.

## Where it changes

- `forge-features`: `shortcut.rs` (table, matching, display), `workspace.rs` (`on_key`, closed stack, `rename_request`, go-to-n), `settings.rs` or a small `shortcuts_dialog.rs` state.
- `forge-widgets`: the dialog, the Settings link, the sidebar row's rename request.
- `forge-pages`: `workspace_page.rs` passes the new overlay state to `on_key`.
- `forge-app/src/main.rs`: check the native menu does not take ⌘B, ⌘/, ⌘J or ⌘1…9 before the page sees them (the ⌘W case, Amendment A).

## Tests

- Unit: the table has no two rows with the same key; each row has at least one key; display text on both platforms; matching by physical key with ⌥ held; the wrong platform modifier does nothing; ⌘1…9 map to indexes 0..8; the closed stack keeps at most 20 and skips id-less sessions; `shortcut_allowed` for every new action.
- Every action in the table is reachable: a test walks the table and checks `shortcut_for` gives back that row's action for each of its keys.
- Real window (Quartz harness, with the `claude_app.py key` limits noted in the observation lessons): press each key; open the modal with ⌘/ and compare it with the keys that work; check Korean IME input still gets the shortcuts.

## Not in this plan

The composer pickers' keys (⇧⌘M, ⇧⌘I, ⇧⌘E), queueing a message, split panes and side chat, voice, the iOS simulator group, pin and unread, and the window search of Settings.
