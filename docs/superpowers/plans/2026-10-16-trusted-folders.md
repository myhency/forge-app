# Plan 43: Trusted folders Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Settings > General has a "Trusted folders" row with a `Manage` dialog; a folder added to a session is added at once when it is on the list (or below a listed folder) and otherwise asks first, with `Trust and add`, `Add this time` and `Cancel`.

**Architecture:** A pure rule (`is_trusted`, `add_trusted`) in `forge-domain` works on path text so the Windows rules are tested on every platform. The list is `AppConfig.trusted_folders`, edited through the existing `CodeChange` / `save_code_settings` path. `ComposerState::add_folder` reads the saved list, decides with a pure function (`classify_added_folder`), and either adds the chip or sets `pending_trust`, which a `TrustCard` above the chips shows.

**Tech Stack:** Rust workspace (Dioxus 0.7 desktop), crates `forge-domain` (rule, config), `forge-features` (state), `forge-widgets` (Settings row and dialog), `forge-shared` (Korean texts, CSS).

**Spec:** `docs/superpowers/specs/2026-10-16-trusted-folders-design.md`

## Global Constraints

- The list is app-wide and used only when a folder is added to a session (the `+` menu "Add folder" and the new-session screen share `ComposerState::add_folder`). The session's project folder is not checked; `choose_folder` is untouched.
- The default browser row is not built.
- Unknown folder: exactly three buttons, `Trust and add` / `Add this time` / `Cancel`. The check runs when the folder is picked, before the chip is added.
- Path comparison is by components: `/work/app-old` is not below `/work/app`. On Windows it ignores case and treats `/` and `\` alike; elsewhere it is exact.
- An old config file without `trusted_folders` loads as an empty list; a value of the wrong type reads as an empty list and the rest of the config still loads (`lenient`).
- Every `tr!` sentence needs a Korean text in `crates/forge-shared/src/ko.rs` (kept in alphabetical order by English text), and every entry there must be used (`i18n.rs` tests check both ways).
- Match surrounding code style and comment density; tests next to the code in `mod tests`. Commit messages end with `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.
- The worktree has no `sidecar/node_modules`; this plan does not need the sidecar.
- Known unrelated failures to ignore: `forge-adapter-claude-plugins --test round_trip`, and the duplicate `SessionUpdate::PromptSuggestion` arm warning in `live_session_actor.rs`.

## Review Focus

1. A sibling folder whose name starts with a trusted folder's name (`/work/app-old` with `/work/app` trusted) must ask, not pass. Task 1.
2. `..` inside a path must not walk out of a trusted folder (`/work/app/../secret`) or out of a Windows drive (`C:\..`). Task 1.
3. A macOS path through a symlink (`/tmp/x` is `/private/tmp/x`) must match a list entry written either way. Task 3 (the caller resolves with `canonicalize`; the real-window check covers it).
4. A hand-edited config with a number or a null inside `trusted_folders` must not stop Forge from starting. Task 1.
5. The same folder added twice, or a folder below an already trusted one, must not grow the list; adding a parent removes the entries below it. Task 1.
6. Picking a second unknown folder while the card is open replaces the first; sending with the card open sends without that folder. Task 4.
7. A failed save of the trust must not lose the folder the person just chose. Task 4.

---

## File Structure

- `crates/forge-domain/src/trusted.rs` (new): `is_trusted`, `add_trusted` and the path-text helpers.
- `crates/forge-domain/src/config.rs`: the `trusted_folders` field.
- `crates/forge-features/src/trust.rs` (new): `resolved`, `FolderAdd`, `classify_added_folder` (pure, no signals).
- `crates/forge-features/src/settings.rs`: `CodeChange::TrustFolder` / `UntrustFolder`, the folder picker, the dialog flag, Esc.
- `crates/forge-features/src/settings_pages.rs`: search titles.
- `crates/forge-features/src/composer_state.rs`: `pending_trust`, the three actions, `TrustCard`.
- `crates/forge-features/src/send_message.rs`, `new_session.rs`: render `TrustCard {}` under `ComposerChips {}`.
- `crates/forge-widgets/src/settings_overlay.rs`: the "Work" group row and `TrustedFoldersDialog`.
- `crates/forge-shared/src/ko.rs`, `theme.css`: texts and styles.
- Docs: `docs/superpowers/roadmap/backlog.md`, `README.md`.

---

### Task 1: The rule and the config field

**Files:**
- Create: `crates/forge-domain/src/trusted.rs`
- Modify: `crates/forge-domain/src/lib.rs` (add `mod trusted;` near `mod paths;`, and `pub use trusted::{add_trusted, is_trusted};`)
- Modify: `crates/forge-domain/src/config.rs` (field, `Default`, tests)

**Interfaces:**
- Produces: `pub fn is_trusted(list: &[PathBuf], path: &Path) -> bool`; `pub fn add_trusted(list: &mut Vec<PathBuf>, path: PathBuf) -> bool` (true when the list changed); `AppConfig.trusted_folders: Vec<PathBuf>`.

- [ ] **Step 1: Write the failing tests**

Create `crates/forge-domain/src/trusted.rs` with only the tests first (the functions come in Step 3; the file will not compile until then, which is the failure):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn list(paths: &[&str]) -> Vec<PathBuf> {
        paths.iter().map(PathBuf::from).collect()
    }

    fn unix(entries: &[&str], path: &str) -> bool {
        is_trusted_on(&list(entries), Path::new(path), false)
    }

    fn windows(entries: &[&str], path: &str) -> bool {
        is_trusted_on(&list(entries), Path::new(path), true)
    }

    #[test]
    fn a_listed_folder_and_the_folders_below_it_are_trusted() {
        assert!(unix(&["/work/app"], "/work/app"));
        assert!(unix(&["/work/app"], "/work/app/src/deep"));
        assert!(!unix(&["/work/app"], "/work"));
        assert!(!unix(&[], "/work/app"));
    }

    #[test]
    fn a_sibling_that_starts_with_the_same_name_is_not_trusted() {
        assert!(!unix(&["/work/app"], "/work/app-old"));
        assert!(!unix(&["/work/app"], "/work/application/src"));
        assert!(!windows(&["C:\\work\\app"], "C:\\work\\app-old"));
    }

    #[test]
    fn a_trailing_separator_and_dot_parts_change_nothing() {
        assert!(unix(&["/work/app/"], "/work/app"));
        assert!(unix(&["/work/app"], "/work/app/"));
        assert!(unix(&["/work/app"], "/work/./app//src"));
    }

    #[test]
    fn dot_dot_cannot_walk_out_of_a_trusted_folder() {
        assert!(!unix(&["/work/app"], "/work/app/../secret"));
        assert!(unix(&["/work/app"], "/work/app/src/../lib"));
        assert!(!unix(&["/work/app"], "/../../etc"));
        assert!(!windows(&["C:\\work"], "C:\\work\\..\\Windows"));
    }

    #[test]
    fn dot_dot_cannot_leave_a_windows_drive_or_share() {
        assert!(!windows(&["D:\\"], "C:\\..\\..\\x"), "C:\\.. stays on C:");
        assert!(!windows(&["\\\\srv\\share\\a"], "\\\\srv\\share\\..\\..\\b"));
        assert!(windows(&["C:\\"], "C:\\anything\\below"));
    }

    #[test]
    fn windows_ignores_case_and_the_kind_of_slash_but_other_systems_do_not() {
        assert!(windows(&["C:\\Work\\App"], "c:/work/app/src"));
        assert!(!unix(&["/Work/App"], "/work/app/src"));
        assert!(!unix(&["/work/app"], "/work\\app"), "a backslash is a name character outside Windows");
    }

    #[test]
    fn the_extended_windows_prefix_is_ignored() {
        assert!(windows(&["C:\\work"], "\\\\?\\C:\\work\\app"));
        assert!(windows(&["\\\\?\\C:\\work"], "C:\\work\\app"));
        assert!(windows(&["\\\\srv\\share\\a"], "\\\\?\\UNC\\srv\\share\\a\\b"));
    }

    #[test]
    fn a_relative_path_or_entry_is_never_trusted() {
        assert!(!unix(&["/work"], "work/app"));
        assert!(!unix(&["work"], "work/app"));
        assert!(!unix(&["work"], "/work/app"));
        assert!(!unix(&[""], "/work"));
        assert!(!windows(&["C:work"], "C:\\work"));
    }

    #[test]
    fn adding_a_folder_appends_it_once() {
        let mut folders = list(&["/a"]);
        assert!(add_trusted_on(&mut folders, "/b".into(), false));
        assert!(!add_trusted_on(&mut folders, "/b".into(), false), "the same folder twice");
        assert_eq!(folders, list(&["/a", "/b"]));
    }

    #[test]
    fn a_folder_below_a_trusted_one_is_not_added() {
        let mut folders = list(&["/work"]);
        assert!(!add_trusted_on(&mut folders, "/work/app".into(), false));
        assert_eq!(folders, list(&["/work"]));
    }

    #[test]
    fn adding_a_parent_removes_the_entries_below_it() {
        let mut folders = list(&["/work/app", "/other", "/work/lib/src"]);
        assert!(add_trusted_on(&mut folders, "/work".into(), false));
        assert_eq!(folders, list(&["/other", "/work"]));
    }

    #[test]
    fn a_relative_folder_is_refused() {
        let mut folders = Vec::new();
        assert!(!add_trusted_on(&mut folders, "work".into(), false));
        assert!(folders.is_empty());
    }
}
```

In `config.rs`'s `mod tests`, append:

```rust
    #[test]
    fn a_config_without_trusted_folders_has_an_empty_list() {
        let config: AppConfig = serde_json::from_str(r#"{"base_url":"https://x","api_key":null}"#).unwrap();
        assert!(config.trusted_folders.is_empty());
    }

    #[test]
    fn trusted_folders_are_stored_and_read_back() {
        let config = AppConfig { trusted_folders: vec!["/work/app".into(), "/other".into()], ..AppConfig::default() };
        let back: AppConfig = serde_json::from_str(&serde_json::to_string(&config).unwrap()).unwrap();
        assert_eq!(back.trusted_folders, config.trusted_folders);
    }

    #[test]
    fn a_bad_trusted_folders_value_reads_as_empty_and_the_rest_still_loads() {
        for text in [r#""trusted_folders": null"#, r#""trusted_folders": "/work""#, r#""trusted_folders": ["/work", 7]"#, r#""trusted_folders": 7"#] {
            let config: AppConfig = serde_json::from_str(&format!(r#"{{"base_url":"https://x",{text}}}"#)).unwrap();
            assert!(config.trusted_folders.is_empty(), "{text}");
            assert_eq!(config.base_url.as_deref(), Some("https://x"), "{text}");
        }
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p forge-domain trusted`
Expected: FAIL to compile (`is_trusted_on` not found, `trusted_folders` not a field).

- [ ] **Step 3: Write the implementation**

Put this above the `#[cfg(test)]` block in `trusted.rs`:

```rust
//! The folders the person trusts. A folder on the list, and every folder below it, may be added to
//! a session without a question. Paths are compared as text (`parts`), so the Windows rules
//! (case, `\`, `\\?\`) are tested on every platform; `windows` says which rules apply.

use std::path::{Path, PathBuf};

use crate::is_absolute_path;

/// Whether `path` is a listed folder or lies below one. A relative path or entry never matches.
pub fn is_trusted(list: &[PathBuf], path: &Path) -> bool {
    is_trusted_on(list, path, cfg!(windows))
}

/// Puts `path` on the list unless it is already trusted; entries below it go, so the list stays
/// short. Returns whether the list changed. A relative path is refused.
pub fn add_trusted(list: &mut Vec<PathBuf>, path: PathBuf) -> bool {
    add_trusted_on(list, path, cfg!(windows))
}

fn is_trusted_on(list: &[PathBuf], path: &Path, windows: bool) -> bool {
    let Some(child) = parts(path, windows) else {
        return false;
    };
    list.iter().filter_map(|entry| parts(entry, windows)).any(|parent| child.starts_with(&parent))
}

fn add_trusted_on(list: &mut Vec<PathBuf>, path: PathBuf, windows: bool) -> bool {
    let Some(new) = parts(&path, windows) else {
        return false;
    };
    if is_trusted_on(list, &path, windows) {
        return false;
    }
    list.retain(|entry| parts(entry, windows).map_or(true, |existing| !existing.starts_with(&new)));
    list.push(path);
    true
}

// The comparable parts of an absolute path, or `None` for a relative one. `.` and empty parts go;
// `..` removes the part before it but never a drive or a share. On Windows the parts are lower case.
fn parts(path: &Path, windows: bool) -> Option<Vec<String>> {
    let text = path.to_string_lossy();
    let text = if windows { strip_extended_prefix(&text) } else { text.to_string() };
    let absolute = if windows { is_absolute_path(&text) } else { text.starts_with('/') };
    if !absolute {
        return None;
    }
    let floor = if !windows {
        0
    } else if text.starts_with("\\\\") || text.starts_with("//") {
        2
    } else if text.as_bytes().get(1) == Some(&b':') {
        1
    } else {
        0
    };
    let mut parts: Vec<String> = Vec::new();
    for part in text.split(|c: char| c == '/' || (windows && c == '\\')) {
        match part {
            "" | "." => {}
            ".." => {
                if parts.len() > floor {
                    parts.pop();
                }
            }
            other => parts.push(if windows { other.to_lowercase() } else { other.to_string() }),
        }
    }
    Some(parts)
}

// `\\?\C:\a` is `C:\a` and `\\?\UNC\srv\share` is `\\srv\share`: the form `canonicalize` returns on Windows.
fn strip_extended_prefix(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("\\\\?\\UNC\\") {
        format!("\\\\{rest}")
    } else if let Some(rest) = path.strip_prefix("\\\\?\\") {
        rest.to_string()
    } else {
        path.to_string()
    }
}
```

In `lib.rs` add `mod trusted;` beside the other `mod` lines and `pub use trusted::{add_trusted, is_trusted};` beside the other `pub use` lines.

In `config.rs`, add to `AppConfig` after `language`:

```rust
    /// Folders the person trusts: a folder on the list, and every folder below it, is added to a
    /// session without a question. Absolute paths, as `forge_domain::add_trusted` keeps them.
    #[serde(default, deserialize_with = "lenient")]
    pub trusted_folders: Vec<std::path::PathBuf>,
```

and `trusted_folders: Vec::new(),` after `language: None,` in `Default for AppConfig`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p forge-domain`
Expected: PASS. If a test about `C:\..` fails, re-read `floor`: for `C:\..\..\x` the parts stay `["c:", "x"]`.

- [ ] **Step 5: Build the workspace**

Run: `cargo build --workspace`
Expected: builds. If a struct literal somewhere lists every `AppConfig` field, add `trusted_folders: Vec::new()` there.

- [ ] **Step 6: Commit**

```bash
git add crates/forge-domain
git commit -m "feat: trusted folders rule and config field (SET-15)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Settings state for the list

**Files:**
- Create: `crates/forge-features/src/trust.rs`
- Modify: `crates/forge-features/src/lib.rs` (`mod trust;`, and re-export what the widgets need in Task 3: nothing yet)
- Modify: `crates/forge-features/src/settings.rs`
- Modify: `crates/forge-features/src/settings_pages.rs`

**Interfaces:**
- Consumes: `forge_domain::{add_trusted, is_trusted}`, `AppConfig.trusted_folders`.
- Produces: `trust::resolved(&Path) -> PathBuf`; `trust::FolderAdd { AlreadyAdded, Trusted, NeedsAsking }`; `trust::classify_added_folder(trusted: &[PathBuf], added: &[PathBuf], picked: &Path, resolved: &Path) -> FolderAdd`; on `Settings`: `trusted_folders() -> Vec<PathBuf>`, `trusted_dialog_open() -> bool`, `open_trusted_dialog()`, `close_trusted_dialog()`, `add_trusted_folder()`, `remove_trusted_folder(PathBuf)`.

- [ ] **Step 1: Write the failing tests**

Create `crates/forge-features/src/trust.rs` with tests only:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn list(paths: &[&str]) -> Vec<PathBuf> {
        paths.iter().map(PathBuf::from).collect()
    }

    #[test]
    fn a_folder_that_is_already_a_chip_is_not_added_again() {
        let added = list(&["/work/app"]);
        assert_eq!(classify_added_folder(&[], &added, Path::new("/work/app"), Path::new("/work/app")), FolderAdd::AlreadyAdded);
    }

    #[test]
    fn a_trusted_folder_or_one_below_it_is_added_without_asking() {
        let trusted = list(&["/work"]);
        assert_eq!(classify_added_folder(&trusted, &[], Path::new("/work"), Path::new("/work")), FolderAdd::Trusted);
        assert_eq!(classify_added_folder(&trusted, &[], Path::new("/work/app"), Path::new("/work/app")), FolderAdd::Trusted);
    }

    #[test]
    fn any_other_folder_asks() {
        let trusted = list(&["/work/app"]);
        assert_eq!(classify_added_folder(&trusted, &[], Path::new("/work/app-old"), Path::new("/work/app-old")), FolderAdd::NeedsAsking);
        assert_eq!(classify_added_folder(&[], &[], Path::new("/tmp"), Path::new("/tmp")), FolderAdd::NeedsAsking);
    }

    #[test]
    fn the_trust_is_judged_on_the_resolved_path_but_the_chip_on_the_picked_one() {
        let trusted = list(&["/private/tmp/x"]);
        assert_eq!(classify_added_folder(&trusted, &[], Path::new("/tmp/x"), Path::new("/private/tmp/x")), FolderAdd::Trusted);
        let added = list(&["/tmp/x"]);
        assert_eq!(classify_added_folder(&trusted, &added, Path::new("/tmp/x"), Path::new("/private/tmp/x")), FolderAdd::AlreadyAdded);
    }

    #[test]
    fn a_path_that_does_not_exist_resolves_to_itself() {
        let gone = Path::new("/definitely/not/here/forge-43");
        assert_eq!(resolved(gone), gone);
    }
}
```

In `settings.rs`'s `mod tests` append:

```rust
    #[test]
    fn trusting_and_untrusting_a_folder_changes_only_the_list() {
        let mut config = AppConfig { model: Some("m".into()), ..AppConfig::default() };
        apply_code_change(&mut config, CodeChange::TrustFolder("/work/app".into()));
        apply_code_change(&mut config, CodeChange::TrustFolder("/other".into()));
        assert_eq!(config.trusted_folders, vec![PathBuf::from("/work/app"), PathBuf::from("/other")]);
        apply_code_change(&mut config, CodeChange::TrustFolder("/work/app/src".into()));
        assert_eq!(config.trusted_folders.len(), 2, "below a trusted folder: no new entry");
        apply_code_change(&mut config, CodeChange::UntrustFolder("/work/app".into()));
        assert_eq!(config.trusted_folders, vec![PathBuf::from("/other")]);
        apply_code_change(&mut config, CodeChange::UntrustFolder("/never/listed".into()));
        assert_eq!(config.trusted_folders, vec![PathBuf::from("/other")]);
        assert_eq!(config.model.as_deref(), Some("m"), "nothing else is touched");
    }
```

In `settings_pages.rs`'s `mod tests` append:

```rust
    #[test]
    fn a_search_finds_the_trusted_folders_row_on_the_general_page() {
        assert_eq!(filter_pages("trusted"), [SettingsSection::General]);
        assert_eq!(filter_pages("TRUSTED FOLDERS"), [SettingsSection::General]);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p forge-features trust`
Expected: FAIL to compile (`resolved`, `FolderAdd`, `CodeChange::TrustFolder` not found).

- [ ] **Step 3: Write `trust.rs`**

Above the tests in `crates/forge-features/src/trust.rs`:

```rust
//! What happens when a folder is added to a session: judged without any UI, so the rule is tested
//! apart from the composer's signals.

use std::path::{Path, PathBuf};

use forge_domain::is_trusted;

/// What the composer does with a folder the person picked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FolderAdd {
    /// It is a chip already: nothing to do.
    AlreadyAdded,
    /// On the trusted list (or below it): add the chip at once.
    Trusted,
    /// Not trusted: show the question first.
    NeedsAsking,
}

/// `picked` is what the dialog returned (the chip shows it); `resolved` is the same folder through
/// `resolved()`, which is what the trusted list is compared with.
pub fn classify_added_folder(trusted: &[PathBuf], added: &[PathBuf], picked: &Path, resolved: &Path) -> FolderAdd {
    if added.iter().any(|existing| existing == picked) {
        FolderAdd::AlreadyAdded
    } else if is_trusted(trusted, resolved) {
        FolderAdd::Trusted
    } else {
        FolderAdd::NeedsAsking
    }
}

/// The folder through symlinks (`/tmp` is `/private/tmp` on macOS); the folder as given when it
/// cannot be read. Windows `\\?\` prefixes are dealt with by the comparison.
pub fn resolved(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}
```

Add `mod trust;` to `lib.rs` next to the other `mod` lines.

- [ ] **Step 4: Change `CodeChange` and `Settings`**

In `settings.rs`:

1. `use forge_ports::FolderPickerPort;` with the other `use` lines.
2. `CodeChange` holds a `PathBuf` now, so it cannot be `Copy`: change its derive to `#[derive(Debug, Clone, PartialEq, Eq)]` and add the variants `TrustFolder(PathBuf)` and `UntrustFolder(PathBuf)`. (`change_code` takes it by value and the save closure consumes it, so nothing else needs `Copy`; fix any compile error that says otherwise by passing a clone.)
3. In `apply_code_change`:

```rust
        CodeChange::TrustFolder(path) => {
            forge_domain::add_trusted(&mut config.trusted_folders, path);
        }
        CodeChange::UntrustFolder(path) => config.trusted_folders.retain(|entry| *entry != path),
```

4. Two fields on `Settings`: `folder_picker: Signal<Arc<dyn FolderPickerPort>>` and `trusted_dialog: Signal<bool>`; in `use_settings_provider` set `folder_picker: Signal::new(services.folder_picker.clone())` and `trusted_dialog: Signal::new(false)`.
5. In `after_save_code` add `current.trusted_folders = config.trusted_folders;` after the `language` line.
6. `close()` also resets the dialog:

```rust
    pub fn close(&self) {
        let mut open = self.open;
        let mut dialog = self.trusted_dialog;
        open.set(false);
        dialog.set(false);
    }
```

7. New methods next to `set_attention_bounce`:

```rust
    pub fn trusted_folders(&self) -> Vec<PathBuf> {
        self.saved.read().trusted_folders.clone()
    }

    pub fn trusted_dialog_open(&self) -> bool {
        *self.trusted_dialog.read()
    }

    pub fn open_trusted_dialog(&self) {
        let mut dialog = self.trusted_dialog;
        dialog.set(true);
    }

    pub fn close_trusted_dialog(&self) {
        let mut dialog = self.trusted_dialog;
        dialog.set(false);
    }

    /// Asks for a folder with the system dialog and puts it on the list (a folder below a listed
    /// one is not added again).
    pub fn add_trusted_folder(&self) {
        let picker = (*self.folder_picker.peek()).clone();
        let this = *self;
        // Pinned to the root scope, like the other saves: the dialog may be closed while the system dialog is open.
        dioxus::core::spawn_forever(async move {
            if let Some(picked) = picker.pick_folder().await {
                this.change_code(CodeChange::TrustFolder(crate::trust::resolved(&picked)));
            }
        });
    }

    pub fn remove_trusted_folder(&self, path: PathBuf) {
        self.change_code(CodeChange::UntrustFolder(path));
    }
```

8. Esc closes the dialog before the overlay, in `on_key`:

```rust
        if self.is_open() && event.key() == Key::Escape {
            if self.trusted_dialog_open() {
                self.close_trusted_dialog();
            } else {
                self.close();
            }
            event.prevent_default();
            return true;
        }
```

- [ ] **Step 5: Search titles**

In `settings_pages.rs`, `row_titles` for `General`, add `tr!("Work"), tr!("Trusted folders"), tr!("Manage"),` after `tr!("Reduce motion")`. These three sentences also need Korean texts: do Task 3 Step 2 (the ko.rs entries) before running the full i18n tests; for this task run only the named tests.

- [ ] **Step 6: Run the tests**

Run: `cargo test -p forge-features trust && cargo test -p forge-features settings`
Expected: PASS (the `i18n` tests in `forge-shared` fail until Task 3 Step 2; that is expected).

- [ ] **Step 7: Commit**

```bash
git add crates/forge-features
git commit -m "feat: settings state for the trusted folders list (SET-15)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 3: The Settings row and dialog

**Files:**
- Modify: `crates/forge-widgets/src/settings_overlay.rs`
- Modify: `crates/forge-shared/src/ko.rs`
- Modify: `crates/forge-shared/src/theme.css`

**Interfaces:**
- Consumes: the `Settings` methods from Task 2; `forge_shared::{Button, ButtonKind, SettingRow}`.

- [ ] **Step 1: Write the row and the dialog**

In `settings_overlay.rs`, inside `GeneralSection`, between the "Reduce motion" row and `h3 ... Notifications`:

```rust
        h3 { class: "settings-group", {tr!("Work")} }
        SettingRow {
            title: tr!("Trusted folders"),
            description: tr!("Trusting a folder on this computer lets Claude use that folder and its subfolders. Trusting a folder does not add it to new sessions. When you add a folder to a session, you may be asked to allow it again."),
            Button { label: tr!("Manage"), onclick: move |_| settings.open_trusted_dialog() }
        }
```

In `SettingsOverlay`, right after the closing brace of the `div { class: "settings-panel", ... }` (still inside the `settings-backdrop` div), add `TrustedFoldersDialog {}`. Then add the component below `GeneralSection`:

```rust
// Over the settings panel, not inside it: the backdrop is as big as the whole window.
#[component]
fn TrustedFoldersDialog() -> Element {
    let settings = use_settings();
    if !settings.trusted_dialog_open() {
        return rsx! {};
    }
    let folders = settings.trusted_folders();
    rsx! {
        div {
            class: "settings-backdrop trusted-backdrop",
            onclick: move |event| {
                event.stop_propagation();
                settings.close_trusted_dialog();
            },
            div { class: "customize-dialog", onclick: move |event| event.stop_propagation(),
                h2 { {tr!("Trusted folders")} }
                p { class: "hint", {tr!("Trusting a folder on this computer lets Claude use that folder and its subfolders. Trusting a folder does not add it to new sessions. When you add a folder to a session, you may be asked to allow it again.")} }
                if let Some(SettingsStatus::Error(message)) = settings.status() {
                    div { class: "status-line error", "{message}" }
                }
                if folders.is_empty() {
                    div { class: "hint", {tr!("No trusted folders yet")} }
                }
                for path in folders {
                    div { key: "{path.display()}", class: "trusted-row",
                        span { "📁" }
                        span { class: "trusted-path", title: "{path.display()}", "{path.display()}" }
                        button {
                            class: "chip-remove",
                            r#type: "button",
                            title: tr!("Remove"),
                            onclick: {
                                let path = path.clone();
                                move |_| settings.remove_trusted_folder(path.clone())
                            },
                            "×"
                        }
                    }
                }
                div { class: "customize-dialog-buttons trusted-buttons",
                    Button { label: format!("+ {}", tr!("Add folder")), onclick: move |_| settings.add_trusted_folder() }
                    Button { label: tr!("Done"), kind: ButtonKind::Primary, onclick: move |_| settings.close_trusted_dialog() }
                }
            }
        }
    }
}
```

If `SettingsStatus::Error` has another shape, match what `StatusLine` in the same file does.

- [ ] **Step 2: Korean texts**

Add these entries to `KO` in `ko.rs`, each at its alphabetical place by English text (an entry with a wrong place does not fail a test, but keep the order):

```rust
    ("Allow Claude to use this folder and its subfolders?", "Claude가 이 폴더와 하위 폴더를 사용하도록 허용할까요?"),
    ("Add this time", "이번만 추가"),
    ("Manage", "관리"),
    ("No trusted folders yet", "신뢰한 폴더가 아직 없습니다"),
    ("Trust and add", "신뢰하고 추가"),
    ("Trusted folders", "신뢰할 수 있는 폴더"),
    ("Trusting a folder on this computer lets Claude use that folder and its subfolders. Trusting a folder does not add it to new sessions. When you add a folder to a session, you may be asked to allow it again.", "이 기기의 폴더를 신뢰하면 Claude가 해당 폴더와 그 하위 폴더를 사용하도록 허용하게 됩니다. 폴더를 신뢰해도 새 세션에 추가되지는 않습니다. 세션에 폴더를 추가할 때 다시 허용하라는 요청을 받을 수 있습니다."),
    ("Work", "작업"),
```

(Check first with `grep -n '"Manage"\|"Work"\|"Cancel"\|"Remove"\|"Done"\|"Add folder"' crates/forge-shared/src/ko.rs`; do not add one that exists. "Allow Claude…" and "Add this time" and "Trust and add" are used by Task 4; the used-entry test passes only after Task 4, so run the i18n test at the end of Task 4.)

- [ ] **Step 3: Styles**

In `theme.css`, after the `.customize-dialog-buttons` line add:

```css
.trusted-backdrop { z-index: 20; }
.trusted-row { display: flex; align-items: center; gap: 8px; padding: 8px 10px; margin-bottom: 6px; border: 1px solid var(--border); border-radius: 10px; }
.trusted-path { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 13px; }
.trusted-buttons { justify-content: space-between; }
```

- [ ] **Step 4: Build and run the tests**

Run: `cargo build --workspace && cargo test -p forge-features -p forge-widgets`
Expected: builds; tests PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-widgets crates/forge-shared
git commit -m "feat: Trusted folders row and dialog in Settings > General (SET-15)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 4: The question when a folder is added

**Files:**
- Modify: `crates/forge-features/src/composer_state.rs`
- Modify: `crates/forge-features/src/send_message.rs`, `crates/forge-features/src/new_session.rs`
- Modify: `crates/forge-features/src/lib.rs` (export `TrustCard` if the widgets need it; they do not, since both callers are in `forge-features`)
- Modify: `crates/forge-shared/src/theme.css`

**Interfaces:**
- Consumes: `crate::trust::{classify_added_folder, resolved, FolderAdd}`, `forge_domain::add_trusted`, `WorkspaceServices.settings.load()` / `.save_code_settings(..)`, `Workspace::report(String)`.
- Produces: `ComposerState::pending_trust() -> Option<PathBuf>`, `trust_and_add()`, `add_once()`, `cancel_trust()`; component `TrustCard`.

- [ ] **Step 1: Write the failing test for the pure part**

The classification is already tested in Task 2. The signal flow needs a Dioxus runtime, which this crate's tests do not use, so it is checked in Task 5. Add one test that pins the dedupe helper this task introduces, in `composer_state.rs`'s `mod tests`:

```rust
    #[test]
    fn a_folder_chip_is_added_once() {
        let mut folders = vec![(PathBuf::from("/a"), true)];
        push_folder(&mut folders, PathBuf::from("/a"));
        push_folder(&mut folders, PathBuf::from("/b"));
        assert_eq!(folders, vec![(PathBuf::from("/a"), true), (PathBuf::from("/b"), false)]);
    }
```

Run: `cargo test -p forge-features composer_state`
Expected: FAIL to compile (`push_folder` not found).

- [ ] **Step 2: Add the helper and the state**

In `composer_state.rs`, above `ComposerState`:

```rust
/// Adds a folder chip, once. A new chip has not been announced to the model yet.
fn push_folder(folders: &mut Vec<(PathBuf, bool)>, path: PathBuf) {
    if !folders.iter().any(|(existing, _)| *existing == path) {
        folders.push((path, false));
    }
}
```

Add to the imports: `use crate::trust::{classify_added_folder, resolved, FolderAdd};` and `use crate::workspace::{use_workspace, Workspace, WorkspaceServices};` (replace the existing `WorkspaceServices` import) and `use forge_shared::{tr, Button, ButtonKind};`.

Add the fields to `ComposerState`:

```rust
    /// A folder the person picked that is not trusted yet; the card under the chips asks about it.
    pending_trust: Signal<Option<PathBuf>>,
    workspace: Workspace,
```

and in `use_composer_state_provider`: `let workspace = use_workspace();` next to `let services = ...`, and `pending_trust: Signal::new(None), workspace,` in the struct literal.

- [ ] **Step 3: Change `add_folder` and add the actions**

Replace the body of `add_folder`:

```rust
    /// Opens the folder dialog. A trusted folder (or one below it) becomes a chip at once; any other
    /// is held in `pending_trust` until the person answers the card.
    pub fn add_folder(&self) {
        if *self.picking.peek() {
            return;
        }
        let services = (*self.services.peek()).clone();
        let mut picking = self.picking;
        let mut folders = self.folders;
        let mut pending = self.pending_trust;
        picking.set(true);
        spawn(async move {
            let picked = services.folder_picker.pick_folder().await;
            picking.set(false);
            let Some(path) = picked else { return };
            // A list that cannot be read counts as empty: the person is asked, never waved through.
            let trusted = services.settings.load().await.map(|config| config.trusted_folders).unwrap_or_default();
            let added: Vec<PathBuf> = folders.peek().iter().map(|(existing, _)| existing.clone()).collect();
            match classify_added_folder(&trusted, &added, &path, &resolved(&path)) {
                FolderAdd::AlreadyAdded => {}
                FolderAdd::Trusted => push_folder(&mut folders.write(), path),
                FolderAdd::NeedsAsking => pending.set(Some(path)),
            }
        });
    }

    pub fn pending_trust(&self) -> Option<PathBuf> {
        self.pending_trust.read().clone()
    }

    /// `Trust and add`: the chip now, and the folder on the trusted list. When the list cannot be
    /// saved the chip stays and a notice says so (the folder is then as for `Add this time`).
    pub fn trust_and_add(&self) {
        let Some(path) = self.take_pending() else { return };
        let mut folders = self.folders;
        push_folder(&mut folders.write(), path.clone());
        let services = (*self.services.peek()).clone();
        let workspace = self.workspace;
        // Root scope: the save must finish even if the composer goes away meanwhile.
        dioxus::core::spawn_forever(async move {
            let folder = resolved(&path);
            let saved = services.settings.save_code_settings(|config| {
                forge_domain::add_trusted(&mut config.trusted_folders, folder);
            });
            if let Err(error) = saved.await {
                workspace.report(tr!("Could not save the trusted folder: {error}", error = error));
            }
        });
        crate::refocus_composer();
    }

    /// `Add this time`: the chip only; nothing is saved.
    pub fn add_once(&self) {
        let Some(path) = self.take_pending() else { return };
        let mut folders = self.folders;
        push_folder(&mut folders.write(), path);
        crate::refocus_composer();
    }

    /// `Cancel` and Esc: the card goes, no chip.
    pub fn cancel_trust(&self) {
        self.take_pending();
        crate::refocus_composer();
    }

    fn take_pending(&self) -> Option<PathBuf> {
        let mut pending = self.pending_trust;
        pending.write().take()
    }
```

If `crate::refocus_composer` is not reachable by that path, import it with `use crate::refocus::refocus_composer;`. If `ConfigError` does not implement `Display`, use `error.to_string()` as `workspace.report` elsewhere does.

A second pick while the card is open replaces `pending_trust` (the `pending.set(Some(path))` line), and `take_for_send` never reads `pending_trust`, so a send with the card open goes without that folder: no further code is needed for either.

- [ ] **Step 4: The card**

Below `ComposerChips` in `composer_state.rs`:

```rust
/// The question for a folder that is not trusted yet, under the chips of the message box. Takes the
/// focus when it opens so Esc cancels.
#[component]
pub fn TrustCard() -> Element {
    let composer = use_composer_state();
    let Some(path) = composer.pending_trust() else {
        return rsx! {};
    };
    rsx! {
        div {
            class: "trust-card",
            tabindex: "0",
            onmounted: move |mounted| async move {
                let _ = mounted.set_focus(true).await;
            },
            onkeydown: move |event: KeyboardEvent| {
                if event.key() == Key::Escape {
                    event.stop_propagation();
                    composer.cancel_trust();
                }
            },
            div { class: "trust-question", {tr!("Allow Claude to use this folder and its subfolders?")} }
            div { class: "trust-path", title: "{path.display()}", "{path.display()}" }
            div { class: "trust-buttons",
                Button { label: tr!("Cancel"), onclick: move |_| composer.cancel_trust() }
                Button { label: tr!("Add this time"), onclick: move |_| composer.add_once() }
                Button { label: tr!("Trust and add"), kind: ButtonKind::Primary, onclick: move |_| composer.trust_and_add() }
            }
        }
    }
}
```

In `send_message.rs` (line ~100) and `new_session.rs` (line ~90), put `TrustCard {}` directly under `ComposerChips {}` and import it with the existing `use crate::composer_state::{use_composer_state_provider, ComposerChips};` line → add `TrustCard`.

- [ ] **Step 5: Texts and styles**

Add `("Could not save the trusted folder: {error}", "신뢰한 폴더를 저장하지 못했습니다: {error}"),` to `ko.rs` (alphabetical). The other three card texts were added in Task 3 Step 2.

In `theme.css` after the `.chip` rules:

```css
.trust-card { display: flex; flex-direction: column; gap: 6px; padding: 10px 12px; margin: 6px 0; border: 1px solid var(--border); border-radius: 10px; background: var(--bg-card); outline: none; }
.trust-question { font-size: 13px; font-weight: 600; }
.trust-path { font-size: 12px; color: var(--fg-muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.trust-buttons { display: flex; justify-content: flex-end; gap: 8px; }
```

- [ ] **Step 6: Run everything**

Run: `cargo test -p forge-features -p forge-domain -p forge-shared && cargo build --workspace`
Expected: PASS, including `every_sentence_in_the_sources_has_a_korean_text` and `every_korean_entry_is_used_in_the_sources`.

- [ ] **Step 7: Commit**

```bash
git add crates
git commit -m "feat: ask before an untrusted folder is added to a session (SET-15)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 5: Check in the real window and record

**Files:**
- Modify: `docs/superpowers/roadmap/backlog.md` (row SET-15)
- Modify: `docs/superpowers/roadmap/README.md` (add a row for Plan 43 after the Plan 31 row, before 24)

- [ ] **Step 1: Run the whole test suite**

Run: `cargo test --workspace`
Expected: PASS except the known unrelated failures.

- [ ] **Step 2: Drive the real window**

Use the harness (`scripts/harness/README.md`): `scripts/harness/launch.sh`, `python3 scripts/harness/capture.py`, `python3 scripts/harness/click.py X Y` (coordinates of the PNG, scaled by the factor the viewer reports), `scripts/harness/stop.sh`. Make three folders first: `mkdir -p /private/tmp/forge-trust/{a,a/sub,b}`. The system folder dialog takes ⇧⌘G, then the path, then Return, Return; if the harness cannot drive it, say so and ask the user to pick the folders, never mark the step done.

Check, with a screenshot each:
1. Settings (⌘,) > General: the "Work" group with "Trusted folders" and `Manage` sits between Appearance and Notifications; the search field finds it with "trusted".
2. `Manage`: empty text; `+ Add folder` with `/private/tmp/forge-trust/a` adds a row; `×` removes it; add it again; `Done` closes; Esc closes the dialog first, the second Esc closes Settings. Restart the app: the folder is still listed.
3. New session screen, `+` > Add folder:
   - `/private/tmp/forge-trust/a` (trusted): a chip at once, no card.
   - `/private/tmp/forge-trust/a/sub` (below it): a chip at once.
   - `/private/tmp/forge-trust/b` (unknown): the card appears with the path and three buttons. `Cancel` (and Esc): no chip. Pick `b` again: `Add this time` gives a chip; remove the chip, pick `b` again: the card asks again (nothing was saved). `Trust and add`: a chip, then remove it and pick `b` again: no card; `Manage` lists `b`.
   - Pick `/tmp/forge-trust/b` through a path typed with `/tmp` (a symlink to `/private/tmp`): no card once `b` is trusted.
4. The same in an open session: add an unknown folder, leave the card open, send a message: it goes without the folder.
5. Korean: switch the language to Korean (Settings > Claude Code or the account menu) and look at the row, dialog and card texts.
6. Stop the app (`scripts/harness/stop.sh`), then remove the test folders: `rm -r /private/tmp/forge-trust`.

Note anything not checked (Windows is covered by unit tests only; dark mode and IME are for the user).

- [ ] **Step 3: Update the roadmap**

In `backlog.md`, change the SET-15 row: Forge `partial`, Plan `Plan 43 (built; the default browser row is not built)`, and append to the Notes: "Plan 43 built the trusted folders list: Settings > General > Work > Manage dialog; a folder added to a session (the `+` menu and the new-session screen) is added at once when it is listed or below a listed folder, otherwise a card asks: Trust and add / Add this time / Cancel. The session's project folder is not checked. The default browser row is left out: Claude in Chrome needs SET-13." (match the row's existing column layout).

In `README.md`'s table add:
`| 43 | Trusted folders (built, checked in the real window) | SET-15 | Spec `2026-10-16-trusted-folders-design.md`; the list in Settings, a question when an unknown folder is added to a session; the default browser row is left out |`

- [ ] **Step 4: Commit**

```bash
git add docs
git commit -m "docs: Plan 43 trusted folders built, roadmap updated (SET-15)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```
