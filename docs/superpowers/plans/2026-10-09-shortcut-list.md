# Plan 32B: the shortcut list and new keys Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A `⌘/` dialog that lists the keys Forge really has, grouped General and Panels, and the missing keys that are cheap and useful, bound for real.

**Architecture:** `shortcut.rs` gets one table of rows (`group`, `label`, `action`, `keys`). The same table decides which action a key press is (matching on the physical key, `event.code()`) and what the dialog draws, so a key that is not in the table is neither matched nor shown. `Workspace::on_key` runs the actions; the page handles the one action that needs the layout (sidebar toggle). A small in-memory stack remembers closed sessions for ⇧⌘T. The dialog is a widget over a `shortcuts_open` signal on `Workspace`.

**Tech Stack:** Rust workspace (Dioxus desktop, `dioxus::events::KeyboardEvent`), crates `forge-features` (logic), `forge-widgets` (dialog, rows), `forge-pages` (page key handler), `forge-shared` (CSS).

**Spec:** `docs/superpowers/specs/2026-10-09-shortcut-list-design.md`

## Global Constraints

- Every row in the dialog works, and every key Forge binds is in the dialog. No dead rows.
- Match on the physical key (`event.code()`), never on the typed character (⌥ changes the character on macOS; the Korean IME changes it too).
- Modifiers must match exactly: a press with an extra modifier (⌥⌘N) is not ⌘N. "Cmd" is Ctrl on platforms other than macOS; `ctrl` bindings mean the literal Control key on every platform.
- Display: macOS shows ⌃⌥⇧⌘ (no separator, that order); other platforms show `Ctrl+Alt+Shift+Key`.
- Left out on purpose (Forge has no such feature): iOS simulator, voice, split panes, side chat, fast mode, pin (⌥⌘P), read/unread (⌥⌘U), open PR (⌥⌘G), copy session link, queue a message (⌘Enter), browser-tab keys, element picking, and the composer pickers' keys (⇧⌘M, ⇧⌘I, ⇧⌘E).
- While Settings, the search palette or the shortcut dialog is open, only keys that belong to the overlay act (⌘, and ⌘/); the session keys are swallowed (Plan 6 found ⌘W closing the session hidden behind the overlay).
- Match surrounding code style and comment density; tests next to the code in `mod tests`. Commit messages end with `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.
- The worktree has no `sidecar/node_modules`; this plan does not need the sidecar.
- Known unrelated failures to ignore: `forge-adapter-claude-plugins --test round_trip`, and the duplicate `SessionUpdate::PromptSuggestion` arm warning in `live_session_actor.rs`.

## Review Focus

1. A ⌥ combination on macOS (⌥⌘A types "å", ⌥⌘R "®") must still be the shortcut, and with the Korean IME on. Matching by `code` is pinned by a test per row. Task 1.
2. ⌥↓ / ⌥↑ in a text box move the caret in other apps; they must be taken only when there is a session to go to, and left alone (not default-prevented) otherwise. Task 2.
3. ⌘1…9 with fewer open sessions than the digit, and ⌃Tab with none open: nothing happens and nothing breaks. Task 2.
4. ⇧⌘T with an empty stack, with a session whose folder is gone, and with a session that is already open again: no panic, a notice for a failed open, the open one is focused. Task 3.
5. ⌥⌘R with the sidebar hidden, the overlay sidebar closed, or the session's group collapsed or filtered out: the row must really enter rename mode or nothing must happen; a rename must not start later by surprise. Task 4.
6. A key press while the shortcut dialog, Settings or the palette is open must not reach the session behind it, for every action in the table (not only the old ones). Task 1 pins it by walking the table.

---

## File Structure

- `crates/forge-features/src/shortcut.rs`: the table, matching, display, the dialog's data (`shortcut_sections`). Rewritten around `KeyPress` and `Binding`.
- `crates/forge-features/src/sidebar.rs`: `nth_session`, `ArchiveKey` and `archive_key_plan` (pure helpers next to `cycle_target`).
- `crates/forge-features/src/closed_sessions.rs` (new): the in-memory stack of closed sessions.
- `crates/forge-features/src/workspace.rs`: `on_key` arms, `go_to_session`, `new_session_here`, `toggle_panel`, closed stack, active-session actions, rename request, `shortcuts_open`.
- `crates/forge-features/src/lib.rs`: module and exports.
- `crates/forge-widgets/src/shortcuts_dialog.rs` (new), `settings_overlay.rs` (link), `session_row.rs` (rename request), `lib.rs`.
- `crates/forge-pages/src/workspace_page.rs`: sidebar toggle, rename reveal, dialog, overlay gating.
- `crates/forge-shared/src/theme.css`: dialog styles.

---

### Task 1: The key table and matching

**Files:**
- Modify: `crates/forge-features/src/shortcut.rs` (rewrite the top part and the tests), `crates/forge-features/src/workspace.rs` (`shortcut_from_event` at ~line 1115, the `match` in `on_key` at ~line 1061), `crates/forge-features/src/lib.rs` (exports)

**Interfaces:**
- Consumes: `crate::dock::Panel` (existing).
- Produces:
  - `pub enum Shortcut { NewSession, NewSessionHere, NextSession, PreviousSession, CloseSession, OpenSettings, OpenSearch, ToggleSidebar, ShowShortcuts, GoToSession(usize), ReopenClosed, ArchiveActive, ForkActive, RenameActive, TogglePanel(Panel) }` (derives `Debug, Clone, Copy, PartialEq, Eq`; `GoToSession` counts from 0).
  - `pub struct KeyPress<'a> { pub code: &'a str, pub ctrl: bool, pub meta: bool, pub alt: bool, pub shift: bool }`.
  - `pub fn shortcut_for(press: &KeyPress, mac: bool) -> Option<Shortcut>`.
  - `pub fn shortcut_allowed(shortcut: Shortcut, overlay_open: bool) -> bool`.
  - `pub struct ShortcutSection { pub title: &'static str, pub rows: Vec<(String, String)> }` and `pub fn shortcut_sections(mac: bool) -> Vec<ShortcutSection>` (label, keys text) for the dialog.
  - `pub fn shortcut_from_event(event: &KeyboardEvent) -> Option<Shortcut>` in `workspace.rs`, now `pub` and re-exported from the crate root together with `Shortcut`, `KeyPress`, `shortcut_for`, `ShortcutSection`, `shortcut_sections`.
  - Unchanged: `command_pressed`, `command_held`.

- [ ] **Step 1: Write the failing tests**

Replace `mod tests` in `shortcut.rs` with the following (the old cases are ported to the new input; the shifted-⌘N case changes meaning: it is now a shortcut):

```rust
#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    fn press(code: &str, ctrl: bool, meta: bool, alt: bool, shift: bool) -> KeyPress<'_> {
        KeyPress { code, ctrl, meta, alt, shift }
    }

    /// The press a binding stands for on `mac`.
    fn press_of(binding: &Binding, code: &'static str, mac: bool) -> KeyPress<'static> {
        let (meta, ctrl) = if binding.command { if mac { (true, false) } else { (false, true) } } else { (false, binding.ctrl) };
        KeyPress { code, ctrl, meta, alt: binding.alt, shift: binding.shift }
    }

    #[test]
    fn the_command_key_is_meta_on_a_mac_and_ctrl_elsewhere() {
        assert!(command_pressed(false, true, true));
        assert!(!command_pressed(true, false, true));
        assert!(command_pressed(true, false, false));
        assert!(!command_pressed(false, true, false), "the Windows key is not the command key");
    }

    #[test]
    fn cmd_n_and_cmd_w_on_a_mac() {
        assert_eq!(shortcut_for(&press("KeyN", false, true, false, false), true), Some(Shortcut::NewSession));
        assert_eq!(shortcut_for(&press("KeyW", false, true, false, false), true), Some(Shortcut::CloseSession));
    }

    #[test]
    fn ctrl_n_and_ctrl_w_elsewhere() {
        assert_eq!(shortcut_for(&press("KeyN", true, false, false, false), false), Some(Shortcut::NewSession));
        assert_eq!(shortcut_for(&press("KeyW", true, false, false, false), false), Some(Shortcut::CloseSession));
    }

    #[test]
    fn the_wrong_modifier_for_the_platform_does_nothing() {
        assert_eq!(shortcut_for(&press("KeyN", true, false, false, false), true), None, "Ctrl+N on a Mac");
        assert_eq!(shortcut_for(&press("KeyW", false, true, false, false), false), None, "Meta+W elsewhere");
    }

    #[test]
    fn plain_keys_and_unbound_commands_do_nothing() {
        assert_eq!(shortcut_for(&press("KeyN", false, false, false, false), true), None);
        assert_eq!(shortcut_for(&press("KeyX", false, true, false, false), true), None);
        assert_eq!(shortcut_for(&press("KeyN", false, true, true, false), true), None, "an extra Option turns Cmd+N into something else");
    }

    #[test]
    fn shift_cmd_n_starts_a_session_in_this_folder() {
        assert_eq!(shortcut_for(&press("KeyN", false, true, false, true), true), Some(Shortcut::NewSessionHere));
    }

    #[test]
    fn ctrl_tab_cycles_on_every_platform_and_shift_reverses() {
        for mac in [true, false] {
            assert_eq!(shortcut_for(&press("Tab", true, false, false, false), mac), Some(Shortcut::NextSession));
            assert_eq!(shortcut_for(&press("Tab", true, false, false, true), mac), Some(Shortcut::PreviousSession));
        }
    }

    #[test]
    fn tab_without_ctrl_is_left_alone() {
        assert_eq!(shortcut_for(&press("Tab", false, false, false, false), true), None);
        assert_eq!(shortcut_for(&press("Tab", false, true, false, false), true), None);
    }

    #[test]
    fn the_other_keys_for_next_and_previous_session() {
        for mac in [true, false] {
            assert_eq!(shortcut_for(&press("ArrowDown", false, false, true, false), mac), Some(Shortcut::NextSession), "Option+Down");
            assert_eq!(shortcut_for(&press("ArrowUp", false, false, true, false), mac), Some(Shortcut::PreviousSession), "Option+Up");
        }
        assert_eq!(shortcut_for(&press("BracketRight", false, true, false, true), true), Some(Shortcut::NextSession));
        assert_eq!(shortcut_for(&press("BracketLeft", false, true, false, true), true), Some(Shortcut::PreviousSession));
        assert_eq!(shortcut_for(&press("ArrowDown", false, false, false, false), true), None, "plain Down is the caret's");
    }

    #[test]
    fn cmd_comma_opens_settings_and_cmd_k_the_search() {
        assert_eq!(shortcut_for(&press("Comma", false, true, false, false), true), Some(Shortcut::OpenSettings));
        assert_eq!(shortcut_for(&press("Comma", true, false, false, false), false), Some(Shortcut::OpenSettings));
        assert_eq!(shortcut_for(&press("Comma", true, false, false, false), true), None, "Ctrl+, on a Mac");
        assert_eq!(shortcut_for(&press("KeyK", false, true, false, false), true), Some(Shortcut::OpenSearch));
        assert_eq!(shortcut_for(&press("KeyK", true, false, false, false), false), Some(Shortcut::OpenSearch));
    }

    #[test]
    fn the_digits_go_to_the_session_at_that_place() {
        assert_eq!(shortcut_for(&press("Digit1", false, true, false, false), true), Some(Shortcut::GoToSession(0)));
        assert_eq!(shortcut_for(&press("Digit9", false, true, false, false), true), Some(Shortcut::GoToSession(8)));
        assert_eq!(shortcut_for(&press("Digit0", false, true, false, false), true), None);
        assert_eq!(shortcut_for(&press("Digit1", false, false, false, false), true), None, "a typed digit");
    }

    #[test]
    fn option_combinations_match_by_the_physical_key() {
        // On a Mac Option+Cmd+A types "å"; the press carries the key's code, not the character.
        assert_eq!(shortcut_for(&press("KeyA", false, true, true, false), true), Some(Shortcut::ArchiveActive));
        assert_eq!(shortcut_for(&press("KeyO", false, true, true, false), true), Some(Shortcut::ForkActive));
        assert_eq!(shortcut_for(&press("KeyR", false, true, true, false), true), Some(Shortcut::RenameActive));
        assert_eq!(shortcut_for(&press("KeyA", true, false, true, false), false), Some(Shortcut::ArchiveActive), "Ctrl+Alt+A elsewhere");
    }

    #[test]
    fn the_panel_keys() {
        use crate::dock::Panel;
        assert_eq!(shortcut_for(&press("KeyD", false, true, false, true), true), Some(Shortcut::TogglePanel(Panel::Changes)));
        assert_eq!(shortcut_for(&press("KeyF", false, true, false, true), true), Some(Shortcut::TogglePanel(Panel::Files)));
        assert_eq!(shortcut_for(&press("KeyJ", false, true, false, false), true), Some(Shortcut::TogglePanel(Panel::Terminal)));
        assert_eq!(shortcut_for(&press("Backquote", true, false, false, false), true), Some(Shortcut::TogglePanel(Panel::Terminal)));
        assert_eq!(shortcut_for(&press("KeyB", false, true, false, true), true), Some(Shortcut::TogglePanel(Panel::Preview)));
        assert_eq!(shortcut_for(&press("KeyB", false, true, false, false), true), Some(Shortcut::ToggleSidebar));
    }

    #[test]
    fn the_rest_of_the_general_keys() {
        assert_eq!(shortcut_for(&press("Slash", false, true, false, false), true), Some(Shortcut::ShowShortcuts));
        assert_eq!(shortcut_for(&press("KeyT", false, true, false, true), true), Some(Shortcut::ReopenClosed));
    }

    #[test]
    fn every_binding_of_every_row_gives_that_row_back_on_both_platforms() {
        for mac in [true, false] {
            for row in SHORTCUTS {
                for binding in row.keys {
                    if binding.code == DIGITS {
                        continue;
                    }
                    let found = shortcut_for(&press_of(binding, binding.code, mac), mac);
                    assert_eq!(found, Some(row.action), "{} on mac={mac}", row.label);
                }
            }
        }
    }

    #[test]
    fn no_two_bindings_share_a_key() {
        for mac in [true, false] {
            let mut seen = HashSet::new();
            for row in SHORTCUTS {
                for binding in row.keys {
                    let key = (binding.code, binding.command, binding.ctrl, binding.alt, binding.shift);
                    assert!(seen.insert(key), "{} repeats a key (mac={mac})", row.label);
                }
            }
        }
    }

    #[test]
    fn every_row_has_a_label_and_at_least_one_key() {
        for row in SHORTCUTS {
            assert!(!row.label.is_empty());
            assert!(!row.keys.is_empty(), "{}", row.label);
        }
    }

    #[test]
    fn while_an_overlay_is_open_only_opening_settings_and_this_list_are_allowed() {
        use crate::dock::Panel;
        let mut actions: Vec<Shortcut> = SHORTCUTS.iter().map(|row| row.action).collect();
        actions.push(Shortcut::GoToSession(3));
        actions.push(Shortcut::TogglePanel(Panel::Files));
        for action in actions {
            assert!(shortcut_allowed(action, false), "{action:?} with no overlay");
            let allowed = matches!(action, Shortcut::OpenSettings | Shortcut::ShowShortcuts);
            assert_eq!(shortcut_allowed(action, true), allowed, "{action:?} behind an overlay");
        }
    }

    #[test]
    fn keys_are_shown_the_platform_way() {
        let find = |label: &str| SHORTCUTS.iter().find(|row| row.label == label).unwrap();
        assert_eq!(find("Archive or unarchive the session").keys[0].display(true), "⌥⌘A");
        assert_eq!(find("Archive or unarchive the session").keys[0].display(false), "Ctrl+Alt+A");
        assert_eq!(find("New session in this folder").keys[0].display(true), "⇧⌘N");
        assert_eq!(find("New session in this folder").keys[0].display(false), "Ctrl+Shift+N");
        assert_eq!(find("Go to session 1…9").keys[0].display(true), "⌘1…9");
        assert_eq!(find("Next session").keys[0].display(true), "⌃Tab");
        assert_eq!(find("Next session").keys[0].display(false), "Ctrl+Tab");
        assert_eq!(find("Next session").keys[2].display(true), "⌥↓");
        assert_eq!(find("Terminal").keys[1].display(true), "⌃`");
    }

    #[test]
    fn the_dialog_lists_every_row_under_its_group_with_alternatives_joined() {
        let sections = shortcut_sections(true);
        assert_eq!(sections.iter().map(|section| section.title).collect::<Vec<_>>(), vec!["General", "Panels"]);
        let total: usize = sections.iter().map(|section| section.rows.len()).sum();
        assert_eq!(total, SHORTCUTS.len());
        let next = sections[0].rows.iter().find(|(label, _)| label == "Next session").unwrap();
        assert_eq!(next.1, "⌃Tab or ⇧⌘] or ⌥↓");
    }
}
```

- [ ] **Step 2: Run to see failure**

Run: `cargo test -p forge-features shortcut`
Expected: FAIL to compile (`KeyPress`, `Binding`, `SHORTCUTS` missing).

- [ ] **Step 3: Implement the table and matching**

Replace everything above `mod tests` in `shortcut.rs` (keep `command_pressed` and `command_held` exactly as they are) with:

```rust
use crate::dock::Panel;

/// What a shortcut does. `GoToSession` counts from 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shortcut {
    NewSession,
    NewSessionHere,
    NextSession,
    PreviousSession,
    CloseSession,
    OpenSettings,
    OpenSearch,
    ToggleSidebar,
    ShowShortcuts,
    GoToSession(usize),
    ReopenClosed,
    ArchiveActive,
    ForkActive,
    RenameActive,
    TogglePanel(Panel),
}

/// A key press: the physical key (`event.code()`, `"KeyN"`) and the modifiers held. Never the typed
/// character: Option changes it on a Mac and an input method (Korean) changes it too.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyPress<'a> {
    pub code: &'a str,
    pub ctrl: bool,
    pub meta: bool,
    pub alt: bool,
    pub shift: bool,
}

/// One key of a shortcut. `command` is Cmd on macOS and Ctrl elsewhere; `ctrl` is the Control key
/// itself on every platform. `code` `DIGITS` stands for the keys 1 to 9.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Binding {
    pub code: &'static str,
    pub command: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

pub const DIGITS: &str = "Digit*";

const fn binding(code: &'static str, command: bool, ctrl: bool, alt: bool, shift: bool) -> Binding {
    Binding { code, command, ctrl, alt, shift }
}
const fn cmd(code: &'static str) -> Binding {
    binding(code, true, false, false, false)
}
const fn cmd_shift(code: &'static str) -> Binding {
    binding(code, true, false, false, true)
}
const fn cmd_alt(code: &'static str) -> Binding {
    binding(code, true, false, true, false)
}
const fn ctrl(code: &'static str) -> Binding {
    binding(code, false, true, false, false)
}
const fn ctrl_shift(code: &'static str) -> Binding {
    binding(code, false, true, false, true)
}
const fn alt(code: &'static str) -> Binding {
    binding(code, false, false, true, false)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    General,
    Panels,
}

impl Group {
    pub fn title(self) -> &'static str {
        match self {
            Group::General => "General",
            Group::Panels => "Panels",
        }
    }
}

/// One row of the shortcut list and the action it stands for.
#[derive(Debug, Clone, Copy)]
pub struct ShortcutRow {
    pub group: Group,
    pub label: &'static str,
    pub action: Shortcut,
    pub keys: &'static [Binding],
}

/// Every shortcut Forge has. The key handler and the shortcut list both read this table, so a key
/// that is not here is not bound and not shown.
pub static SHORTCUTS: &[ShortcutRow] = &[
    ShortcutRow { group: Group::General, label: "Search sessions", action: Shortcut::OpenSearch, keys: &[cmd("KeyK")] },
    ShortcutRow { group: Group::General, label: "New session", action: Shortcut::NewSession, keys: &[cmd("KeyN")] },
    ShortcutRow { group: Group::General, label: "New session in this folder", action: Shortcut::NewSessionHere, keys: &[cmd_shift("KeyN")] },
    ShortcutRow { group: Group::General, label: "Close the session", action: Shortcut::CloseSession, keys: &[cmd("KeyW")] },
    ShortcutRow { group: Group::General, label: "Settings", action: Shortcut::OpenSettings, keys: &[cmd("Comma")] },
    ShortcutRow { group: Group::General, label: "Keyboard shortcuts", action: Shortcut::ShowShortcuts, keys: &[cmd("Slash")] },
    ShortcutRow { group: Group::General, label: "Show or hide the sidebar", action: Shortcut::ToggleSidebar, keys: &[cmd("KeyB")] },
    ShortcutRow {
        group: Group::General,
        label: "Next session",
        action: Shortcut::NextSession,
        keys: &[ctrl("Tab"), cmd_shift("BracketRight"), alt("ArrowDown")],
    },
    ShortcutRow {
        group: Group::General,
        label: "Previous session",
        action: Shortcut::PreviousSession,
        keys: &[ctrl_shift("Tab"), cmd_shift("BracketLeft"), alt("ArrowUp")],
    },
    ShortcutRow { group: Group::General, label: "Go to session 1…9", action: Shortcut::GoToSession(0), keys: &[cmd(DIGITS)] },
    ShortcutRow { group: Group::General, label: "Reopen the last closed session", action: Shortcut::ReopenClosed, keys: &[cmd_shift("KeyT")] },
    ShortcutRow { group: Group::General, label: "Archive or unarchive the session", action: Shortcut::ArchiveActive, keys: &[cmd_alt("KeyA")] },
    ShortcutRow { group: Group::General, label: "Fork the session", action: Shortcut::ForkActive, keys: &[cmd_alt("KeyO")] },
    ShortcutRow { group: Group::General, label: "Rename the session", action: Shortcut::RenameActive, keys: &[cmd_alt("KeyR")] },
    ShortcutRow { group: Group::Panels, label: "Changes", action: Shortcut::TogglePanel(Panel::Changes), keys: &[cmd_shift("KeyD")] },
    ShortcutRow { group: Group::Panels, label: "Files", action: Shortcut::TogglePanel(Panel::Files), keys: &[cmd_shift("KeyF")] },
    ShortcutRow { group: Group::Panels, label: "Terminal", action: Shortcut::TogglePanel(Panel::Terminal), keys: &[cmd("KeyJ"), ctrl("Backquote")] },
    ShortcutRow { group: Group::Panels, label: "Preview", action: Shortcut::TogglePanel(Panel::Preview), keys: &[cmd_shift("KeyB")] },
];

impl Binding {
    // The Meta and Ctrl keys this binding asks for on the platform.
    fn wanted(&self, mac: bool) -> (bool, bool) {
        if self.command {
            if mac { (true, false) } else { (false, true) }
        } else {
            (false, self.ctrl)
        }
    }

    fn modifiers_match(&self, press: &KeyPress, mac: bool) -> bool {
        let (meta, ctrl) = self.wanted(mac);
        press.meta == meta && press.ctrl == ctrl && press.alt == self.alt && press.shift == self.shift
    }

    /// The key as the list shows it: `⌥⌘A` on a Mac, `Ctrl+Alt+A` elsewhere.
    pub fn display(&self, mac: bool) -> String {
        let (meta, ctrl) = self.wanted(mac);
        let key = key_label(self.code);
        if mac {
            let mut text = String::new();
            if ctrl { text.push('⌃'); }
            if self.alt { text.push('⌥'); }
            if self.shift { text.push('⇧'); }
            if meta { text.push('⌘'); }
            text.push_str(&key);
            text
        } else {
            let mut parts = Vec::new();
            if ctrl { parts.push("Ctrl".to_string()); }
            if self.alt { parts.push("Alt".to_string()); }
            if self.shift { parts.push("Shift".to_string()); }
            parts.push(key);
            parts.join("+")
        }
    }
}

fn key_label(code: &str) -> String {
    match code {
        DIGITS => "1…9".to_string(),
        "ArrowDown" => "↓".to_string(),
        "ArrowUp" => "↑".to_string(),
        "Comma" => ",".to_string(),
        "Slash" => "/".to_string(),
        "BracketLeft" => "[".to_string(),
        "BracketRight" => "]".to_string(),
        "Backquote" => "`".to_string(),
        other => other.strip_prefix("Key").unwrap_or(other).to_string(),
    }
}

// The 0-based place a `DigitN` code stands for (`Digit1` is 0); 0 and anything else is none.
fn digit_index(code: &str) -> Option<usize> {
    let number: usize = code.strip_prefix("Digit")?.parse().ok()?;
    (1..=9).contains(&number).then(|| number - 1)
}

/// The shortcut a key press is, if any. "Cmd" is Ctrl on platforms other than macOS.
pub fn shortcut_for(press: &KeyPress, mac: bool) -> Option<Shortcut> {
    for row in SHORTCUTS {
        for binding in row.keys {
            if !binding.modifiers_match(press, mac) {
                continue;
            }
            if binding.code == DIGITS {
                if let Some(index) = digit_index(press.code) {
                    return Some(Shortcut::GoToSession(index));
                }
            } else if binding.code == press.code {
                return Some(row.action);
            }
        }
    }
    None
}

/// Whether `shortcut` may act while an overlay (Settings, the search palette, the shortcut list) is
/// open. Only opening settings and the list do: the session shortcuts would change the session
/// hidden behind the overlay (Plan 6 found ⌘W closing it).
pub fn shortcut_allowed(shortcut: Shortcut, overlay_open: bool) -> bool {
    !overlay_open || matches!(shortcut, Shortcut::OpenSettings | Shortcut::ShowShortcuts)
}

/// One group of the shortcut list: rows of (label, keys text).
#[derive(Debug, Clone, PartialEq)]
pub struct ShortcutSection {
    pub title: &'static str,
    pub rows: Vec<(String, String)>,
}

/// The shortcut list as the dialog draws it, groups in order, alternatives joined with " or ".
pub fn shortcut_sections(mac: bool) -> Vec<ShortcutSection> {
    [Group::General, Group::Panels]
        .into_iter()
        .map(|group| ShortcutSection {
            title: group.title(),
            rows: SHORTCUTS
                .iter()
                .filter(|row| row.group == group)
                .map(|row| (row.label.to_string(), row.keys.iter().map(|binding| binding.display(mac)).collect::<Vec<_>>().join(" or ")))
                .collect(),
        })
        .collect()
}
```

(`Panel` must derive `Copy, PartialEq, Eq, Debug`; it does. `command_pressed` and `command_held` stay below this block, unchanged.)

In `workspace.rs` replace `shortcut_from_event`:

```rust
/// The shortcut a key press is, if any (platform-aware: Cmd on macOS, Ctrl elsewhere).
pub fn shortcut_from_event(event: &KeyboardEvent) -> Option<Shortcut> {
    let modifiers = event.modifiers();
    let code = event.code().to_string();
    shortcut_for(
        &KeyPress { code: &code, ctrl: modifiers.ctrl(), meta: modifiers.meta(), alt: modifiers.alt(), shift: modifiers.shift() },
        cfg!(target_os = "macos"),
    )
}
```

and fix its imports (`use crate::shortcut::{shortcut_allowed, shortcut_for, KeyPress, Shortcut};`). The `match shortcut` in `on_key` must stay exhaustive; give it the old arms and one temporary arm for the new ones:

```rust
            Shortcut::OpenSettings => return false,
            Shortcut::NewSession => self.show_new_session(),
            Shortcut::OpenSearch => self.open_search(),
            Shortcut::NextSession => self.cycle(true),
            Shortcut::PreviousSession => self.cycle(false),
            Shortcut::CloseSession => self.close_active(),
            // Temporary: Tasks 2 to 5 give each of these its action.
            Shortcut::NewSessionHere
            | Shortcut::ToggleSidebar
            | Shortcut::ShowShortcuts
            | Shortcut::GoToSession(_)
            | Shortcut::ReopenClosed
            | Shortcut::ArchiveActive
            | Shortcut::ForkActive
            | Shortcut::RenameActive
            | Shortcut::TogglePanel(_) => return false,
```

In `forge-features/src/lib.rs` export: `pub use shortcut::{shortcut_for, shortcut_sections, KeyPress, Shortcut, ShortcutSection, ...}` (keep what is exported today, add these) and `pub use workspace::shortcut_from_event`.

- [ ] **Step 4: Run to see pass**

Run: `cargo test -p forge-features shortcut && cargo build --workspace`
Expected: PASS and a clean build.

- [ ] **Step 5: Commit**

```bash
git add -A crates/forge-features
git commit -m "feat: one table of shortcuts matched by physical key, with the platform's display of each

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Sessions by number, new session here, panel keys, sidebar key

**Files:**
- Modify: `crates/forge-features/src/sidebar.rs` (next to `cycle_target`, line ~323), `crates/forge-features/src/workspace.rs` (`cycle` ~line 1034, `on_key` ~line 1050, new methods), `crates/forge-pages/src/workspace_page.rs` (key handler ~line 69)

**Interfaces:**
- Consumes: `Shortcut`, `shortcut_from_event` (Task 1); `open_session_order`, `Workspace::show_new_session_in(folder: PathBuf)` (existing), `Workspace::change_dock(id, FnOnce(&mut DockState))` (existing), `DockState::toggle(panel)` (existing).
- Produces:
  - `pub fn nth_session(order: &[LiveSessionId], index: usize) -> Option<LiveSessionId>` in `sidebar.rs`.
  - `Workspace::go_to_session(&self, index: usize) -> bool`, `Workspace::new_session_here(&self)`, `Workspace::toggle_panel(&self, panel: Panel)`, and `Workspace::cycle(&self, forward: bool) -> bool` (now says whether there was a session to go to).
  - `on_key` returns `false` (event untouched) for `NextSession`/`PreviousSession` when no session is open, and for `ToggleSidebar` (the page does it).

- [ ] **Step 1: Write the failing tests**

In `sidebar.rs` `mod tests` add:

```rust
    #[test]
    fn nth_session_counts_from_zero_and_gives_nothing_past_the_end() {
        let order = [LiveSessionId(7), LiveSessionId(3), LiveSessionId(9)];
        assert_eq!(nth_session(&order, 0), Some(LiveSessionId(7)));
        assert_eq!(nth_session(&order, 2), Some(LiveSessionId(9)));
        assert_eq!(nth_session(&order, 3), None);
        assert_eq!(nth_session(&[], 0), None);
    }
```

- [ ] **Step 2: Run to see failure**

Run: `cargo test -p forge-features nth_session`
Expected: FAIL to compile.

- [ ] **Step 3: Implement**

`sidebar.rs`, after `cycle_target`:

```rust
/// The open session at `index` (from 0) in the order the sidebar draws them; none when fewer are open.
pub fn nth_session(order: &[LiveSessionId], index: usize) -> Option<LiveSessionId> {
    order.get(index).copied()
}
```

`workspace.rs`: change `cycle` to report whether it moved, and add the three methods (import `nth_session` and `Panel` where the file imports its siblings):

```rust
    /// Focuses the next (or previous) open session in the order the sidebar draws them; `false` when
    /// no session is open, so the key press can be left to the page.
    pub fn cycle(&self, forward: bool) -> bool {
        let order = open_session_order(&self.sidebar(), &self.prefs.read(), crate::arrange::now_local());
        let current = match self.active() {
            Active::Live(id) => Some(id),
            Active::NewSession | Active::Customize | Active::Routines => None,
        };
        match cycle_target(&order, current, forward) {
            Some(target) => {
                self.select(target);
                true
            }
            None => false,
        }
    }

    /// Focuses the n-th open session (from 0), in the order the sidebar draws them. Nothing when
    /// fewer are open.
    pub fn go_to_session(&self, index: usize) {
        let order = open_session_order(&self.sidebar(), &self.prefs.read(), crate::arrange::now_local());
        if let Some(id) = nth_session(&order, index) {
            self.select(id);
        }
    }

    /// The new-session page with the active session's folder chosen (just the page when no session is active).
    pub fn new_session_here(&self) {
        if let Active::Live(id) = self.active() {
            if let Some(view) = self.live_view(id) {
                let folder = view.read().project_path.clone();
                return self.show_new_session_in(folder);
            }
        }
        self.show_new_session();
    }

    /// Opens or closes `panel` in the active session's dock; nothing on the other pages.
    pub fn toggle_panel(&self, panel: Panel) {
        if let Active::Live(id) = self.active() {
            self.change_dock(id, |dock| dock.toggle(panel));
        }
    }
```

(If `cycle` has other callers, keep them compiling: `let _ = workspace.cycle(true);`.)

`on_key` arms (replace the temporary arm's relevant lines):

```rust
            Shortcut::NextSession => {
                if !self.cycle(true) {
                    return false;
                }
            }
            Shortcut::PreviousSession => {
                if !self.cycle(false) {
                    return false;
                }
            }
            Shortcut::NewSessionHere => self.new_session_here(),
            Shortcut::GoToSession(index) => self.go_to_session(index),
            Shortcut::TogglePanel(panel) => self.toggle_panel(panel),
            // The sidebar belongs to the page (it knows the layout); it offers the event next.
            Shortcut::ToggleSidebar => return false,
```

`workspace_page.rs`: in the root `onkeydown`, after `workspace.on_key(...)` returns false, handle the sidebar key before `settings.on_key`. The existing handler is:

```rust
                if !workspace.on_key(&event, settings.is_open() || workspace.search_open()) {
                    settings.on_key(&event);
                }
```

Make it:

```rust
                if !workspace.on_key(&event, settings.is_open() || workspace.search_open()) {
                    if forge_features::shortcut_from_event(&event) == Some(forge_features::Shortcut::ToggleSidebar) {
                        // An overlay swallowed the key before this point (`shortcut_allowed`).
                        match layout.sidebar {
                            SidebarMode::Overlay => peek.set(!peek()),
                            SidebarMode::Docked => workspace.update_prefs(|prefs| prefs.hidden = !prefs.hidden),
                        }
                        event.prevent_default();
                    } else {
                        settings.on_key(&event);
                    }
                }
```

(`layout` is computed above the `rsx!` in this component; if it is not `Copy` for a `move` closure, copy `layout.sidebar` into a local `let sidebar_mode = layout.sidebar;` before the `rsx!` and use that.)

- [ ] **Step 4: Run to see pass**

Run: `cargo test -p forge-features && cargo build --workspace`
Expected: PASS and a clean build.

- [ ] **Step 5: Commit**

```bash
git add -A crates
git commit -m "feat: keys for the n-th session, a new session in this folder, the panels and the sidebar; Option+Up/Down leave the caret alone when no session is open

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Reopen the last closed session

**Files:**
- Create: `crates/forge-features/src/closed_sessions.rs`
- Modify: `crates/forge-features/src/lib.rs` (`mod closed_sessions;`), `crates/forge-features/src/workspace.rs` (struct, provider, `close` at ~line 1007, new `reopen_closed`, `on_key`)

**Interfaces:**
- Consumes: `Workspace::open_past(ProjectSummary, SessionId)` and `project_for_open(&[(ProjectSummary, Vec<SessionSummary>)], &Path) -> ProjectSummary` (existing, used by `fork_session`).
- Produces: `ClosedSessions { push(PathBuf, SessionId), pop() -> Option<(PathBuf, SessionId)> }` with `ClosedSessions::MAX = 20`; `Workspace::reopen_closed(&self)`.

- [ ] **Step 1: Write the failing tests**

Create `closed_sessions.rs`:

```rust
use std::path::PathBuf;

use forge_domain::SessionId;

/// The sessions closed during this run of the app, newest last, for "reopen the last closed
/// session". Kept in memory only.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ClosedSessions {
    items: Vec<(PathBuf, SessionId)>,
}

impl ClosedSessions {
    pub const MAX: usize = 20;

    /// Remembers a closed session. One that is already remembered moves to the newest place; past
    /// `MAX` the oldest is forgotten.
    pub fn push(&mut self, project: PathBuf, session: SessionId) {
        self.items.retain(|(path, id)| !(*path == project && *id == session));
        self.items.push((project, session));
        if self.items.len() > Self::MAX {
            self.items.remove(0);
        }
    }

    /// The most recently closed session, forgotten from now on.
    pub fn pop(&mut self) -> Option<(PathBuf, SessionId)> {
        self.items.pop()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(text: &str) -> SessionId {
        SessionId(text.to_string())
    }

    #[test]
    fn the_newest_comes_back_first() {
        let mut closed = ClosedSessions::default();
        closed.push(PathBuf::from("/a"), id("one"));
        closed.push(PathBuf::from("/a"), id("two"));
        assert_eq!(closed.pop(), Some((PathBuf::from("/a"), id("two"))));
        assert_eq!(closed.pop(), Some((PathBuf::from("/a"), id("one"))));
        assert_eq!(closed.pop(), None);
    }

    #[test]
    fn nothing_closed_gives_nothing() {
        assert_eq!(ClosedSessions::default().pop(), None);
    }

    #[test]
    fn closing_the_same_session_twice_keeps_it_once_at_the_newest_place() {
        let mut closed = ClosedSessions::default();
        closed.push(PathBuf::from("/a"), id("one"));
        closed.push(PathBuf::from("/a"), id("two"));
        closed.push(PathBuf::from("/a"), id("one"));
        assert_eq!(closed.pop(), Some((PathBuf::from("/a"), id("one"))));
        assert_eq!(closed.pop(), Some((PathBuf::from("/a"), id("two"))));
        assert_eq!(closed.pop(), None);
    }

    #[test]
    fn the_same_id_in_another_folder_is_another_session() {
        let mut closed = ClosedSessions::default();
        closed.push(PathBuf::from("/a"), id("one"));
        closed.push(PathBuf::from("/b"), id("one"));
        assert_eq!(closed.pop(), Some((PathBuf::from("/b"), id("one"))));
        assert_eq!(closed.pop(), Some((PathBuf::from("/a"), id("one"))));
    }

    #[test]
    fn only_the_newest_twenty_are_kept() {
        let mut closed = ClosedSessions::default();
        for n in 0..25 {
            closed.push(PathBuf::from("/a"), id(&format!("s{n}")));
        }
        let mut popped = Vec::new();
        while let Some((_, session)) = closed.pop() {
            popped.push(session.0);
        }
        assert_eq!(popped.len(), ClosedSessions::MAX);
        assert_eq!(popped.first().map(String::as_str), Some("s24"));
        assert_eq!(popped.last().map(String::as_str), Some("s5"));
    }
}
```

(Check that `SessionId` is `pub struct SessionId(pub String)` with `Clone, PartialEq`; the sidebar code builds `SessionId("...".to_string())`, so it is.)

- [ ] **Step 2: Run to see failure**

Run: `cargo test -p forge-features closed_sessions`
Expected: FAIL (the module is not registered yet, so the tests are not found: `0 tests` or a compile error). Registering `mod closed_sessions;` in Step 3 is what makes them run.

- [ ] **Step 3: Implement**

`lib.rs`: add `mod closed_sessions;`. `workspace.rs`: add the field `closed: Signal<ClosedSessions>` to `Workspace` (after `search_task`), `closed: Signal::new(ClosedSessions::default()),` in `use_workspace_provider`, `use crate::closed_sessions::ClosedSessions;`. In `close(&self, id)`, first thing, before `services.registry.close(id)`:

```rust
        // Remembered for "reopen the last closed session"; a session that never got an id has no transcript to come back to.
        if let Some(handle) = self.handle(id) {
            let view = handle.snapshot();
            if let Some(session) = view.claude_session_id {
                let mut closed = self.closed;
                closed.write().push(view.project_path, session);
            }
        }
```

and the method:

```rust
    /// Opens the session closed last again (or focuses it when it is open once more). Does
    /// nothing when none was closed; a failed open shows as a notice, like any open.
    pub fn reopen_closed(&self) {
        let mut closed = self.closed;
        let Some((project, session)) = closed.write().pop() else {
            return;
        };
        let summary = project_for_open(&self.past.peek(), &project);
        self.open_past(summary, session);
    }
```

`on_key`: `Shortcut::ReopenClosed => self.reopen_closed(),`.

- [ ] **Step 4: Run to see pass**

Run: `cargo test -p forge-features && cargo build --workspace`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A crates/forge-features
git commit -m "feat: Shift+Cmd+T reopens the session closed last

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Archive, fork and rename the active session

**Files:**
- Modify: `crates/forge-features/src/sidebar.rs` (`ArchiveKey`, `archive_key_plan` after `menu_key_action`), `crates/forge-features/src/workspace.rs` (new field and methods, `on_key`, `select` and `show_new_session` clear the request), `crates/forge-widgets/src/session_row.rs` (consume the request), `crates/forge-pages/src/workspace_page.rs` (reveal the sidebar)

**Interfaces:**
- Consumes: `SidebarEntry::actions()` (existing, `can_fork`, `can_rename`), `Workspace::archive_session / unarchive_session / fork_session / close` (existing), `arranged_sidebar(now)` and `ArrangedEntry.entry` (existing), `Phase` from `forge_application`.
- Produces:
  - `pub enum ArchiveKey { Nothing, Unarchive, ArchiveAndClose, StopFirst }` and `pub fn archive_key_plan(archived: bool, has_id: bool, running: bool) -> ArchiveKey`.
  - `Workspace::archive_active()`, `fork_active()`, `rename_active()`, `rename_request() -> Option<EntryKey>`, `clear_rename_request()`.

**Ruling (the spec said "under the same conditions as the row menu"):** the row menu archives only sessions that are not open ("close it first"), and the active session is always open, so that rule would make ⌥⌘A inert for almost every active session. The key therefore archives AND closes the session (an archived session is hidden from the default list, so leaving it open would be confusing), unarchives an archived open one, and asks the person to stop a running session first. This is a deliberate extension of the spec for the key only; the row menu is unchanged.

- [ ] **Step 1: Write the failing tests**

`sidebar.rs` `mod tests`:

```rust
    #[test]
    fn the_archive_key_archives_and_closes_an_idle_session_with_an_id() {
        assert_eq!(archive_key_plan(false, true, false), ArchiveKey::ArchiveAndClose);
    }

    #[test]
    fn the_archive_key_unarchives_an_archived_session_even_while_it_runs() {
        assert_eq!(archive_key_plan(true, true, false), ArchiveKey::Unarchive);
        assert_eq!(archive_key_plan(true, true, true), ArchiveKey::Unarchive);
    }

    #[test]
    fn a_running_session_is_not_closed_by_the_archive_key() {
        assert_eq!(archive_key_plan(false, true, true), ArchiveKey::StopFirst);
    }

    #[test]
    fn a_session_without_an_id_cannot_be_archived() {
        assert_eq!(archive_key_plan(false, false, false), ArchiveKey::Nothing);
        assert_eq!(archive_key_plan(true, false, false), ArchiveKey::Nothing);
    }
```

- [ ] **Step 2: Run to see failure**

Run: `cargo test -p forge-features archive_key`
Expected: FAIL to compile.

- [ ] **Step 3: Implement the plan function and the Workspace side**

`sidebar.rs`, after `menu_key_action`:

```rust
/// What ⌥⌘A does to the active (open) session. The row menu only archives sessions that are not
/// open; the key closes the session as part of archiving it, and does not close one that is
/// still working.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveKey {
    Nothing,
    Unarchive,
    ArchiveAndClose,
    /// A turn is running: tell the person to stop it first.
    StopFirst,
}

pub fn archive_key_plan(archived: bool, has_id: bool, running: bool) -> ArchiveKey {
    match (has_id, archived, running) {
        (false, _, _) => ArchiveKey::Nothing,
        (true, true, _) => ArchiveKey::Unarchive,
        (true, false, true) => ArchiveKey::StopFirst,
        (true, false, false) => ArchiveKey::ArchiveAndClose,
    }
}
```

`workspace.rs`: add `rename_request: Signal<Option<EntryKey>>` (init `Signal::new(None)`), import `EntryKey`, `SidebarEntry`, `archive_key_plan`, `ArchiveKey`, `Phase`. Methods:

```rust
    // The active session's folder and sidebar entry (its title, its actions).
    fn active_entry(&self) -> Option<(PathBuf, SidebarEntry)> {
        let Active::Live(id) = self.active() else {
            return None;
        };
        self.sidebar().into_iter().find_map(|group| {
            let path = group.project.path.clone();
            group.entries.into_iter().find(|entry| entry.key == EntryKey::Live(id)).map(|entry| (path, entry))
        })
    }

    /// Forks the active session, as the row menu's Fork does.
    pub fn fork_active(&self) {
        if let Some((project, entry)) = self.active_entry() {
            if let (true, Some(session)) = (entry.actions().can_fork, entry.session_id.clone()) {
                self.fork_session(project, session, entry.title);
            }
        }
    }

    /// Archives the active session and closes it, or unarchives it when it is archived (see
    /// `ArchiveKey`).
    pub fn archive_active(&self) {
        let (Active::Live(id), Some((project, entry))) = (self.active(), self.active_entry()) else {
            return;
        };
        let running = self.live_view(id).is_some_and(|view| view.read().phase == Phase::Running);
        match (archive_key_plan(entry.archived, entry.session_id.is_some(), running), entry.session_id) {
            (ArchiveKey::Unarchive, Some(session)) => self.unarchive_session(project, session),
            (ArchiveKey::ArchiveAndClose, Some(session)) => {
                self.archive_session(project, session);
                self.close(id);
            }
            (ArchiveKey::StopFirst, _) => self.report("Stop the session before archiving it.".to_string()),
            _ => {}
        }
    }

    /// Puts the active session's sidebar row into rename mode. Nothing when the session cannot be
    /// renamed or its row is not in the list (its group is collapsed, or the status filter hides it).
    pub fn rename_active(&self) {
        let Some((_, entry)) = self.active_entry() else {
            return;
        };
        let shown = self
            .arranged_sidebar(crate::arrange::now_local())
            .iter()
            .any(|group| !group.collapsed && group.entries.iter().any(|item| item.entry.key == entry.key));
        if entry.actions().can_rename && shown {
            let mut request = self.rename_request;
            request.set(Some(entry.key));
        }
    }

    /// The row that is asked to start renaming, until it takes the request.
    pub fn rename_request(&self) -> Option<EntryKey> {
        self.rename_request.read().clone()
    }

    pub fn clear_rename_request(&self) {
        let mut request = self.rename_request;
        request.set(None);
    }
```

In `select(&self, id)` and `show_new_session(&self)` add `self.clear_rename_request();` as the first line (a request must not outlive the page it was made on). In `on_key`: `Shortcut::ArchiveActive => self.archive_active(), Shortcut::ForkActive => self.fork_active(), Shortcut::RenameActive => self.rename_active(),`.

Check the field names used above against the code: `SidebarGroup` has `project` and `entries`; `ArrangedGroup` has `collapsed` and `entries` of items with `.entry`; adjust if the compiler says otherwise.

`session_row.rs`: after `let session_id = entry.session_id.clone();` (before the `match mode()`), add:

```rust
    // ⌥⌘R asks the active session's row to start renaming.
    let own_key = entry.key.clone();
    let own_title = entry.title.clone();
    use_effect(move || {
        if workspace.rename_request().as_ref() == Some(&own_key) {
            workspace.clear_rename_request();
            mode.set(RowMode::Renaming(own_title.clone()));
        }
    });
```

`workspace_page.rs`: the row exists only while the sidebar is shown. After the `layout` is computed, add an effect that shows it when a rename is asked for (`use_effect` keeps the closure from its first render, so read signals inside it, not the local `layout`):

```rust
    // ⌥⌘R needs the row on screen: bring the sidebar out when it is hidden or an overlay.
    use_effect(move || {
        if workspace.rename_request().is_none() {
            return;
        }
        match layout_for(window_width(), SIDEBAR_RAIL_WIDTH, 0).sidebar {
            SidebarMode::Overlay => peek.set(true),
            SidebarMode::Docked => {
                if workspace.prefs().hidden {
                    workspace.update_prefs(|prefs| prefs.hidden = false);
                }
            }
        }
    });
```

Before relying on `layout_for(window_width(), SIDEBAR_RAIL_WIDTH, 0).sidebar`, read `crates/forge-domain/src/layout.rs` and confirm the sidebar mode depends on the window width only; if it also depends on the dock width, pass the active session's dock width the way the page computes `dock_width`.

- [ ] **Step 4: Run to see pass**

Run: `cargo test -p forge-features && cargo build --workspace`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A crates
git commit -m "feat: Option+Cmd+A, O and R archive, fork and rename the active session

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 5: The shortcut dialog

**Files:**
- Create: `crates/forge-widgets/src/shortcuts_dialog.rs`
- Modify: `crates/forge-features/src/workspace.rs` (`shortcuts_open` signal, methods, `on_key`), `crates/forge-widgets/src/lib.rs`, `crates/forge-widgets/src/settings_overlay.rs` (link), `crates/forge-pages/src/workspace_page.rs` (dialog, gating), `crates/forge-shared/src/theme.css` (styles; check `theme.rs` for how the CSS is shipped)

**Interfaces:**
- Consumes: `shortcut_sections(mac) -> Vec<ShortcutSection>` (Task 1).
- Produces: `Workspace::shortcuts_open() -> bool`, `open_shortcuts()`, `close_shortcuts()`, `toggle_shortcuts()`; `ShortcutsDialog` component.

- [ ] **Step 1: Implement the state**

`workspace.rs`: add `shortcuts_open: Signal<bool>` (init `Signal::new(false)`) and

```rust
    pub fn shortcuts_open(&self) -> bool {
        *self.shortcuts_open.read()
    }

    pub fn open_shortcuts(&self) {
        let mut open = self.shortcuts_open;
        open.set(true);
    }

    pub fn close_shortcuts(&self) {
        let mut open = self.shortcuts_open;
        open.set(false);
    }

    pub fn toggle_shortcuts(&self) {
        if self.shortcuts_open() {
            self.close_shortcuts();
        } else {
            self.open_shortcuts();
        }
    }
```

In `on_key`, first lines (before `shortcut_from_event`): 

```rust
        // Escape puts the shortcut list away.
        if self.shortcuts_open() && event.key() == Key::Escape {
            self.close_shortcuts();
            event.prevent_default();
            return true;
        }
```

and the arm `Shortcut::ShowShortcuts => self.toggle_shortcuts(),`. Now no wildcard or temporary arm should be left in `on_key`: every `Shortcut` variant has its own arm (`OpenSettings` and `ToggleSidebar` return `false`).

- [ ] **Step 2: The widget, the styles, the wiring**

`shortcuts_dialog.rs`:

```rust
use dioxus::prelude::*;
use forge_features::{refocus_workspace, shortcut_sections, use_workspace};

/// The list of keyboard shortcuts (⌘/): every key Forge binds, from the same table the key handler
/// reads. Renders nothing while closed; Escape or the close button puts it away.
#[component]
pub fn ShortcutsDialog() -> Element {
    let workspace = use_workspace();
    if !workspace.shortcuts_open() {
        return rsx! {};
    }
    let sections = shortcut_sections(cfg!(target_os = "macos"));
    rsx! {
        div {
            class: "settings-backdrop",
            onclick: move |_| {
                workspace.close_shortcuts();
                refocus_workspace();
            },
            div {
                class: "shortcuts-panel",
                role: "dialog",
                "aria-label": "Keyboard shortcuts",
                // A click inside the panel must not reach the backdrop and close it.
                onclick: move |event| event.stop_propagation(),
                button {
                    class: "settings-close",
                    title: "Close (Esc)",
                    onclick: move |_| {
                        workspace.close_shortcuts();
                        refocus_workspace();
                    },
                    "✕"
                }
                h2 { "Keyboard shortcuts" }
                for section in sections {
                    h3 { key: "{section.title}", "{section.title}" }
                    for (label, keys) in section.rows {
                        div { class: "shortcut-row", key: "{label}",
                            span { "{label}" }
                            span { class: "shortcut-keys", "{keys}" }
                        }
                    }
                }
            }
        }
    }
}
```

(The two `key:` attributes on elements inside different `for` loops are fine; if Dioxus complains about a key on `h3` inside a `for` that yields a fragment, wrap each section in `div { key: "...", ... }`.)

`theme.css` (next to the `.settings-*` rules at line ~205):

```css
.shortcuts-panel { position: relative; width: min(560px, calc(100% - 64px)); max-height: min(640px, calc(100% - 64px)); overflow-y: auto; padding: 24px 28px; background: var(--bg); border: 1px solid var(--border); border-radius: var(--radius-lg); box-shadow: var(--shadow); box-sizing: border-box; }
.shortcuts-panel h2 { margin: 0 0 12px; font-size: 16px; }
.shortcuts-panel h3 { margin: 16px 0 4px; font-size: 12px; font-weight: 600; color: var(--fg-muted); }
.shortcut-row { display: flex; justify-content: space-between; gap: 24px; padding: 7px 0; border-bottom: 1px solid var(--border); }
.shortcut-keys { color: var(--fg-muted); white-space: nowrap; font-variant-numeric: tabular-nums; }
```

`forge-widgets/src/lib.rs`: `mod shortcuts_dialog; pub use shortcuts_dialog::ShortcutsDialog;`.

`settings_overlay.rs`: in the nav, after the MCP servers button, add (and `use_workspace` in the imports and `let workspace = use_workspace();` at the top of `SettingsOverlay`, after the `is_open` early return is fine as long as it is not conditional on a hook order issue — put it before the early return):

```rust
                    button {
                        class: "settings-nav-item",
                        style: "margin-top: 16px;",
                        onclick: move |_| {
                            settings.close();
                            workspace.open_shortcuts();
                        },
                        "Keyboard shortcuts"
                    }
```

`workspace_page.rs`: import `ShortcutsDialog`; render `ShortcutsDialog {}` after `SearchPalette {}`; add `workspace.shortcuts_open()` to the refocus effect's tuple (`let _ = (settings.is_open(), workspace.search_open(), workspace.shortcuts_open(), workspace.active(), ...)`); in the key handler use `let overlay_open = settings.is_open() || workspace.search_open() || workspace.shortcuts_open();` for both the Escape-puts-the-overlay-sidebar-away condition (add `&& !workspace.shortcuts_open()`) and the `workspace.on_key(&event, overlay_open)` call; add `workspace.shortcuts_open()` to the `overlay_open` of the effect that puts the overlay sidebar away.

- [ ] **Step 3: Build and run**

Run: `cargo build --workspace && cargo test --workspace --no-fail-fast`
Expected: build clean; tests PASS except the known `round_trip`.

- [ ] **Step 4: Commit**

```bash
git add -A crates
git commit -m "feat: Cmd+/ opens the list of keyboard shortcuts, drawn from the same table the keys use

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 6: Check the keys in the real window

Use the harness from the memory notes "Driving the Forge app visually" (screencapture + Quartz clicks + `FORGE_CLAUDE_HOME` fixture with a few past sessions in two folders) and the lessons of Plans 15 (match keys by physical key, Korean IME), 26 and 28 (click scale), 30. Nothing here may touch the user's real `~/.claude` or Forge config. Report only; do not change product code.

- [ ] **Step 1: Native menu**: confirm the app's native menu (crates/forge-app/src/main.rs `app_menu`) takes none of ⌘B, ⌘/, ⌘J, ⌘1…9, ⇧⌘T, ⇧⌘D/F/B, ⌥⌘A/O/R before the page sees them (the ⌘W case, Amendment A): press each and see the app react, not the menu.
- [ ] **Step 2: Walk the table**: open the dialog with ⌘/ and press every row's key in turn, comparing what happens with the label (screenshots for each): search, new session, new session in this folder (the folder of the active session is chosen), close, settings, sidebar hide/show (docked, then in a narrow window where it is an overlay), next/previous session by all three keys, ⌘1…9 including a digit larger than the number of open sessions, reopen closed (and with an empty stack), archive (idle session: archived and closed; running: the notice; archived and open: unarchived), fork, rename (sidebar shown, hidden, overlay closed, group collapsed), the four panel keys.
- [ ] **Step 3: Overlays and input**: with Settings, the search palette and the dialog open, press ⌘N, ⌘W, ⌘1, ⌥⌘A: nothing may change behind them; ⌘, and ⌘/ still work. With the Korean input method on, repeat three of the keys (⌥⌘R, ⇧⌘T, ⌘1). In the composer with text, ⌥↓ with no other session open must still move the caret.
- [ ] **Step 4: Report**: PASS / FAIL / NOT CHECKED per step with screenshot paths; for each FAIL the observed against the expected behaviour. Remove any fixture state.

---

### Task 7: Docs and finish

**Files:**
- Modify: `docs/superpowers/roadmap/backlog.md` (SET-25 row), `docs/superpowers/plans/2026-10-09-shortcut-list.md` (a "Checked in the real window" note from Task 6)

- [ ] **Step 1: Update the backlog**

SET-25: Forge column `partial` (the list and the keys that map to Forge's features exist; iOS simulator, voice, split panes, side chat, fast mode, queueing, pin/unread/PR/link and the composer pickers' keys are not built), Plan column `Plan 32B (built)`, and a note that the dialog and the key handler read one table (`forge-features/src/shortcut.rs`), keys match by physical key, ⇧⌘N is a guess (new-session page in the active session's folder), and ⌥⌘A archives and closes the active session (the row menu only archives closed ones). Update the SET-01 row if it describes how Settings is opened.

- [ ] **Step 2: Run everything once more**

Run: `cargo test --workspace --no-fail-fast && cargo clippy --workspace --all-targets`
Expected: PASS except the known `round_trip`; no new clippy warning in the files this plan touched. `grep -n "return false" crates/forge-features/src/workspace.rs` near `on_key` must show only the `OpenSettings`, `ToggleSidebar` and empty-cycle returns, and `on_key` must have no wildcard `_ =>` arm.

- [ ] **Step 3: Commit, then finish**

```bash
git add -A docs
git commit -m "docs: Plan 32B is built; SET-25 updated in the backlog

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

Then use superpowers:finishing-a-development-branch (the base branch is `master`).
