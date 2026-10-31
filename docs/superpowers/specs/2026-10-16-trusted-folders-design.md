# Plan 43: Trusted folders (SET-15, without the default browser)

Date: 2026-10-16. Roadmap row: SET-15 (`docs/superpowers/roadmap/backlog.md`). Seen in observation rounds 15 and 16 (`observations.md`): General page, group "작업", and the `관리` dialog.

## Goal

Settings > General has a "Trusted folders" row with a `Manage` button. A folder on that list, and every folder below it, is added to a session without a question. Any other folder is added only after the person confirms, and can be put on the list at that moment.

Success: the list survives a restart; a trusted folder or one below it is added with no question; any other folder asks first, with three choices; an old config file without the field loads with an empty list; path comparison is right on macOS and on Windows.

## Decisions made with the user

- Scope A: the list is app-wide and is used only when a folder is added to a session. The project folder a session starts in is not checked (no change to `choose_folder`).
- The default browser row is not built: its second choice, Claude in Chrome, needs SET-13. It stays in the backlog under SET-15's note and SET-13.
- Unknown folder, three buttons: `Trust and add`, `Add this time`, `Cancel`.
- The check happens when the person picks the folder, before the chip is added (not at send time).

## Data and rule

`AppConfig.trusted_folders: Vec<PathBuf>`, `#[serde(default)]`. An old file reads as an empty list. A bad entry (not a string) makes the whole field read as empty (`lenient`), like the other prefs.

A pure function `is_trusted(list: &[PathBuf], path: &Path) -> bool` (in `forge-domain`, next to the config): true when `path` equals a listed folder or lies below one. Rules:

- Both sides are compared after `normalize`: drop `.` parts, resolve `..` by text, drop a trailing separator. Where the folder exists, `std::fs::canonicalize` is tried first so a symlink does not hide the real place; on failure the text form is used. On Windows the `\\?\` prefix that `canonicalize` adds is removed.
- Comparison is by path components (`Path::starts_with`), so `/work/app-old` is not below `/work/app`.
- On Windows the comparison ignores case and treats `/` as `\`. On macOS and Linux it is exact. (A macOS volume can be case-insensitive; the exact rule is safe: it asks once too often, never once too little.)
- `add_trusted(list, path)`: does nothing if `is_trusted` already holds, otherwise appends; entries below the new one are removed so the list stays short. Order of the list is the order of adding.

## Settings: the row and the dialog

- General page gets a group "Work" (`작업`) above "Notifications" with one row: title "Trusted folders", the explanation, and a `Manage` button. The explanation, translated from the desktop app: trusting a folder on this computer lets Claude use that folder and its subfolders; trusting a folder does not add it to new sessions; when a folder is added to a session, you may be asked to allow it again.
- `Manage` opens a dialog (the pattern of `customize_dialogs.rs`): the same text, one row per folder (folder icon, the path, `×` that removes it), `+ Add folder` (the OS folder dialog through `FolderPickerPort`; the chosen folder goes through `add_trusted`), and a black `Done`. An empty list shows the line "No trusted folders yet".
- Remove and add save the config at once (the `change_code` path used by the other General rows, a new `CodeChange::TrustedFolders`), and `Settings::trusted_folders()` reads the saved list.
- The settings search table (`settings_pages.rs`) gets the titles "Work", "Trusted folders", "Manage".
- Not built: the default browser row.

## Adding a folder to a session

`+` menu "Add folder" and the new-session composer share `ComposerState::add_folder` (`composer_state.rs`). After the OS dialog returns a folder:

1. Already a chip: nothing (as today).
2. `is_trusted` with the saved list: the chip is added at once (as today).
3. Otherwise `pending_trust: Signal<Option<PathBuf>>` is set and a card shows above the message box: the path, the question "Allow Claude to use this folder and its subfolders?", and three buttons.
   - `Trust and add`: `add_trusted` + save, then the chip.
   - `Add this time`: the chip only; nothing is saved.
   - `Cancel` (and Esc): the card closes, no chip.
4. A second pick while the card is open replaces the pending path. Sending a message while the card is open sends without that folder.

If saving fails for `Trust and add`, the folder is still added as for `Add this time` and the usual notice reports the failed save.

The card is part of the message box, so it moves with the composer in an open session and on the new-session screen. The card does not use the permission card of the chat (that one belongs to the SDK's requests).

## Where it changes

`forge-domain` (`config.rs`: field, `is_trusted`, `add_trusted`, `normalize`), `forge-features` (`settings.rs` + `CodeChange`, `settings_pages.rs`, `composer_state.rs`), `forge-widgets` (`settings_overlay.rs` row and dialog, a trust card next to the message box), `forge-shared` (`ko.rs` texts, CSS for the row and card).

## Tests

- Unit (`is_trusted`, `add_trusted`): same folder, a folder below, a sibling with the same name prefix, a trailing separator, `..` inside, an empty list, a deeper entry removed when its parent is added, a duplicate; Windows case and `\` cases written as pure-text tests that run on every platform through the comparison helper.
- Config: round trip; an old file without the field; a wrong type reads as empty.
- `ComposerState`: trusted folder gives a chip and no card; unknown folder gives a card and no chip; each of the three buttons; a second pick replaces the pending path; a failed save still adds the chip.
- Settings: add and remove through the dialog state save the config; the search table finds "Trusted folders".
- Real window (Quartz harness, `FORGE_CLAUDE_HOME` fixture): open Manage, add and remove a folder, restart and see the list; in a session add a trusted folder, a folder below it, and an unknown one with each of the three buttons; check that `Trust and add` stops the question next time. Windows is checked by the unit tests only.

## Not in this plan

The default browser row and Claude in Chrome (SET-13), trust for the session's project folder, a trust question when a routine runs in a folder (PG-01), asking for confirmation again after a folder is removed from the list, sandbox (SET-11).
