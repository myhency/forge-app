# Right click menus for files, and Go to line (Plan 25C-2) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The Files panel's tree row and tab right click menus get the observed items (copy, open in, Finder, show in tree, find in file, copy contents, reload, close ...) with submenus, and the editor gets `Go to line`.

**Architecture:** A pure, declarative menu (`MenuEntry`, `FileAction`, `file_menu(target)`) in forge-features; one `ContextMenu` widget draws any entry list; `FilesCtx::run_action` performs actions, the panel performs `Show in file tree` (it owns the tree). `BrowserPort` gets `open_with`. The editor bundle gets a one-field `Go to line` panel.

**Tech Stack:** Rust (Dioxus 0.7.10), CodeMirror 6 bundle (esbuild, jsdom tests).

**Spec:** `docs/superpowers/specs/2026-10-05-file-menus-design.md`

## Global Constraints

- Branch `plan-25C2` in `/Users/james/orca/workspaces/forge/plan-25B`; base `master`. Commit trailer: `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.
- English labels exactly: `Attach as context`, `Copy` (`Absolute path`, `Relative path`, `File name`), `Open in` (`VS Code`, `Default app`), `Show in Finder`, `Show in file tree`, `Find in file` (`⌘F`), `Copy file contents`, `Reload file`, `Close file`, `Keep open`, `Close others`, `Close all`, `Go to line` (`⌃G`).
- `open_with`: the path must be absolute and is one argument, never a command line; the app is a fixed name (`Visual Studio Code`) chosen by Forge, never typed by the person.
- A reload never discards edits silently; a failed action becomes a notice, never a panic.
- After a bundle change: `cd tools/editor-bundle && npm run build && npm test` (the built `crates/forge-shared/assets/editor.js` is committed).
- Out of scope: moving panel state into `Workspace` (25D), keyboard navigation inside menus.

## Review Focus

- `Reload file` on a tab with edits must give the conflict banner, never lose the edits (Task 4: test of the action mapping plus real window).
- `Copy file contents` of a binary, image or too large file must give a notice and leave the clipboard alone (Task 4).
- `open_with` must refuse a relative path and a path that looks like an option before any program runs (Task 1 test).
- `Show in file tree` for a file under folders that were never opened must read and open them in order and end on the row (Task 2 test, Task 4).
- A submenu near the right window edge must open to the left (Task 3 test of the pure rule).
- `Go to line` with 0, a negative, text, or a number past the end must not throw: ignore or clamp (Task 5 tests).
- A right click on a directory row must not open the file menu (Task 4).

---

### Task 1: `BrowserPort::open_with`

**Files:**
- Modify: `crates/forge-ports/src/browser_port.rs`, `crates/forge-adapter-browser/src/lib.rs`, `crates/forge-features/src/workspace.rs` (next to `reveal`, ~line 619)

**Interfaces:**
- Produces: `BrowserPort::open_with(&self, path: &Path, app: Option<&str>) -> Result<(), String>`; `Workspace::open_file(&self, path: &Path, app: Option<&str>)` (a failure becomes a notice); `Workspace::report` becomes `pub` if it is not (used by Task 4 for notices).

- [ ] **Step 1: Failing tests** in the adapter's tests module (follow the style of the existing `reveal` tests: program `true`, `false`, a missing program):

```rust
#[test]
fn opening_a_file_passes_the_path_as_one_argument() {
    assert_eq!(SystemBrowser::with_program("true").open_with(std::path::Path::new("/tmp/a b.txt"), None), Ok(()));
    assert_eq!(SystemBrowser::with_program("true").open_with(std::path::Path::new("/tmp/a.txt"), Some("Visual Studio Code")), Ok(()));
}

#[test]
fn a_relative_path_or_an_option_looking_path_is_refused_before_any_program_runs() {
    let browser = SystemBrowser::with_program("forge-no-such-program");
    for path in ["a.txt", "-a", "--help", ""] {
        let error = browser.open_with(std::path::Path::new(path), None).unwrap_err();
        assert!(!error.contains("forge-no-such-program"), "{path}: {error}");
    }
}

#[test]
fn a_program_that_fails_or_is_missing_is_reported_by_open_with() {
    assert!(SystemBrowser::with_program("false").open_with(std::path::Path::new("/tmp/a.txt"), Some("Nope")).is_err());
    let error = SystemBrowser::with_program("forge-no-such-program").open_with(std::path::Path::new("/tmp/a.txt"), None).unwrap_err();
    assert!(error.contains("forge-no-such-program"), "{error}");
}
```

and in `forge-ports/src/browser_port.rs` tests:

```rust
#[test]
fn the_mock_can_refuse_to_open_a_file() {
    let mut mock = MockBrowserPort::new();
    mock.expect_open_with().returning(|_, _| Err("no app".to_string()));
    assert_eq!(mock.open_with(std::path::Path::new("/tmp/a.txt"), None), Err("no app".to_string()));
}
```

- [ ] **Step 2: Run, expect FAIL** — `cargo test -p forge-ports -p forge-adapter-browser 2>&1 | grep -E "^error" | head -3` (no method `open_with`).

- [ ] **Step 3: Implement.** In the trait:

```rust
/// Opens a file with the default app, or with the named app (`open -a`). The path must be absolute.
fn open_with(&self, path: &Path, app: Option<&str>) -> Result<(), String>;
```

In the adapter:

```rust
fn open_with(&self, path: &Path, app: Option<&str>) -> Result<(), String> {
    if !path.is_absolute() {
        return Err("Only an absolute path can be opened".to_string());
    }
    let mut command = Command::new(&self.program);
    if let Some(app) = app {
        command.arg("-a").arg(app);
    }
    // The path is one argument, never part of a command line; it starts with `/`, so it cannot be an option.
    let status = command
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| format!("Could not run {}: {error}", self.program))?;
    if status.success() {
        Ok(())
    } else {
        Err(match app {
            Some(app) => format!("Could not open the file with {app}"),
            None => format!("{} could not open the file", self.program),
        })
    }
}
```

`Workspace`:

```rust
/// Opens a file with the default app or a named one; a failure becomes a notice.
pub fn open_file(&self, path: &Path, app: Option<&str>) {
    if let Err(message) = self.browser().open_with(path, app) {
        self.report(message);
    }
}
```

(Check `fn report` at ~line 906: make it `pub fn report`.) Any other `impl BrowserPort` (grep) gets the method.

- [ ] **Step 4: Run** — `cargo test -p forge-ports -p forge-adapter-browser` → pass; `cargo build` ok.
- [ ] **Step 5: Commit** — `feat: open a file with the default app or a named one (Plan 25C-2)`.

---

### Task 2: `FileTree::reveal_dirs` and `open_dir`

**Files:**
- Modify: `crates/forge-features/src/file_tree.rs`

**Interfaces:**
- Produces: `FileTree::reveal_dirs(path: &str) -> Vec<String>` (associated fn: the ancestor folders of `path`, outermost first, e.g. `"src/deep/a.txt"` gives `["src", "src/deep"]`, a root file gives `[]`); `FileTree::open_dir(&mut self, dir: &str) -> bool` (opens without closing; true when its entries must be read).

- [ ] **Step 1: Failing tests** (in the tests module; `sample()` exists):

```rust
#[test]
fn the_ancestors_of_a_path_come_outermost_first() {
    assert_eq!(FileTree::reveal_dirs("src/deep/a.txt"), vec!["src".to_string(), "src/deep".to_string()]);
    assert_eq!(FileTree::reveal_dirs("a.txt"), Vec::<String>::new());
    assert_eq!(FileTree::reveal_dirs(""), Vec::<String>::new());
}

#[test]
fn opening_a_folder_twice_keeps_it_open_and_asks_for_a_read_only_until_it_is_read() {
    let mut tree = FileTree::default();
    tree.set_children("", vec![dir("src")]);
    assert!(tree.open_dir("src"));
    assert!(tree.open_dir("src"));
    tree.set_children("src", vec![file("a.txt")]);
    assert!(!tree.open_dir("src"));
    // `toggle` would have closed it; `open_dir` never does.
    let rows = tree.rows("");
    assert!(rows[0].expanded);
    assert_eq!(rows[1].path, "src/a.txt");
}
```


- [ ] **Step 2: Run, expect FAIL** (`cargo test -p forge-features file_tree 2>&1 | grep -E "^error" | head -2`).
- [ ] **Step 3: Implement:**

```rust
/// The folders above `path`, outermost first (`src/deep/a.txt` gives `src`, `src/deep`).
pub fn reveal_dirs(path: &str) -> Vec<String> {
    let parts: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    (1..parts.len()).map(|n| parts[..n].join("/")).collect()
}

/// Opens a folder (never closes one). True when its entries have not been read yet.
pub fn open_dir(&mut self, dir: &str) -> bool {
    self.open.insert(dir.to_string());
    !self.is_loaded(dir)
}
```

- [ ] **Step 4: Run** — pass. **Step 5: Commit** — `feat: reveal_dirs and open_dir for showing a file in the tree (Plan 25C-2)`.

---

### Task 3: The declarative menu

**Files:**
- Create: `crates/forge-features/src/file_menu.rs`
- Modify: `crates/forge-features/src/lib.rs`

**Interfaces:**
- Produces: `FileAction` (`Debug, Clone, Copy, PartialEq, Eq`): `Attach, CopyAbsolute, CopyRelative, CopyName, OpenVsCode, OpenDefault, Reveal, ShowInTree, FindInFile, CopyContents, Reload, Close, KeepOpen, CloseOthers, CloseAll`; `MenuTarget { Tree, Tab { preview: bool } }`; `MenuEntry` (`Action { label: &'static str, shortcut: Option<&'static str>, action: FileAction, enabled: bool }`, `Submenu { label: &'static str, entries: Vec<MenuEntry> }`, `Divider`); `file_menu(MenuTarget) -> Vec<MenuEntry>`; `submenu_opens_left(menu_x: f64, window_width: f64, menu_width: f64) -> bool`.

- [ ] **Step 1: Failing tests** (`file_menu.rs` with only the tests module and a `labels` helper; add `mod file_menu;` and `pub use file_menu::{file_menu, submenu_opens_left, FileAction, MenuEntry, MenuTarget};` to `lib.rs`):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // One line per entry: the label, `—` for a divider, `Label >` followed by its items for a submenu.
    fn labels(entries: &[MenuEntry]) -> Vec<String> {
        let mut out = Vec::new();
        for entry in entries {
            match entry {
                MenuEntry::Divider => out.push("—".to_string()),
                MenuEntry::Action { label, shortcut, .. } => {
                    out.push(match shortcut {
                        Some(key) => format!("{label} [{key}]"),
                        None => label.to_string(),
                    })
                }
                MenuEntry::Submenu { label, entries } => {
                    out.push(format!("{label} >"));
                    out.extend(labels(entries).into_iter().map(|l| format!("  {l}")));
                }
            }
        }
        out
    }

    #[test]
    fn the_tree_row_menu_has_the_observed_items_in_order() {
        assert_eq!(
            labels(&file_menu(MenuTarget::Tree)),
            vec![
                "Attach as context", "Copy >", "  Absolute path", "  Relative path", "  File name", "—",
                "Open in >", "  VS Code", "  —", "  Default app", "Show in Finder", "—",
                "Show in file tree", "—",
                "Find in file [⌘F]", "Copy file contents", "Reload file", "—",
                "Close file",
            ]
        );
    }

    #[test]
    fn the_tab_menu_has_the_observed_items_in_order() {
        assert_eq!(
            labels(&file_menu(MenuTarget::Tab { preview: true })),
            vec![
                "Attach as context", "Copy >", "  Absolute path", "  Relative path", "  File name", "—",
                "Show in file tree", "—",
                "Keep open", "Close file", "Close others", "Close all",
            ]
        );
    }

    fn enabled(entries: &[MenuEntry], wanted: FileAction) -> Option<bool> {
        entries.iter().find_map(|entry| match entry {
            MenuEntry::Action { action, enabled, .. } if *action == wanted => Some(*enabled),
            _ => None,
        })
    }

    #[test]
    fn keep_open_is_enabled_only_for_a_preview_tab() {
        assert_eq!(enabled(&file_menu(MenuTarget::Tab { preview: true }), FileAction::KeepOpen), Some(true));
        assert_eq!(enabled(&file_menu(MenuTarget::Tab { preview: false }), FileAction::KeepOpen), Some(false));
    }

    #[test]
    fn the_submenu_flips_left_when_there_is_no_room_on_the_right() {
        assert!(!submenu_opens_left(100.0, 1200.0, 230.0));
        assert!(submenu_opens_left(900.0, 1200.0, 230.0));
        // Just enough room for the menu and the submenu: stays on the right.
        assert!(!submenu_opens_left(740.0, 1200.0, 230.0));
        assert!(submenu_opens_left(741.0, 1200.0, 230.0));
    }
}
```

- [ ] **Step 2: Run** — `cargo test -p forge-features file_menu 2>&1 | grep -E "^error" | head -2` → FAIL.
- [ ] **Step 3: Implement** (above the tests):

```rust
/// What a menu item does. The Files panel performs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileAction {
    Attach,
    CopyAbsolute,
    CopyRelative,
    CopyName,
    OpenVsCode,
    OpenDefault,
    Reveal,
    ShowInTree,
    FindInFile,
    CopyContents,
    Reload,
    Close,
    KeepOpen,
    CloseOthers,
    CloseAll,
}

/// Where the menu was asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuTarget {
    Tree,
    Tab { preview: bool },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuEntry {
    Action { label: &'static str, shortcut: Option<&'static str>, action: FileAction, enabled: bool },
    Submenu { label: &'static str, entries: Vec<MenuEntry> },
    Divider,
}

fn item(label: &'static str, action: FileAction) -> MenuEntry {
    MenuEntry::Action { label, shortcut: None, action, enabled: true }
}

fn copy_menu() -> MenuEntry {
    MenuEntry::Submenu {
        label: "Copy",
        entries: vec![
            item("Absolute path", FileAction::CopyAbsolute),
            item("Relative path", FileAction::CopyRelative),
            item("File name", FileAction::CopyName),
        ],
    }
}

pub fn file_menu(target: MenuTarget) -> Vec<MenuEntry> {
    match target {
        MenuTarget::Tree => vec![
            item("Attach as context", FileAction::Attach),
            copy_menu(),
            MenuEntry::Divider,
            MenuEntry::Submenu {
                label: "Open in",
                entries: vec![item("VS Code", FileAction::OpenVsCode), MenuEntry::Divider, item("Default app", FileAction::OpenDefault)],
            },
            item("Show in Finder", FileAction::Reveal),
            MenuEntry::Divider,
            item("Show in file tree", FileAction::ShowInTree),
            MenuEntry::Divider,
            MenuEntry::Action { label: "Find in file", shortcut: Some("⌘F"), action: FileAction::FindInFile, enabled: true },
            item("Copy file contents", FileAction::CopyContents),
            item("Reload file", FileAction::Reload),
            MenuEntry::Divider,
            item("Close file", FileAction::Close),
        ],
        MenuTarget::Tab { preview } => vec![
            item("Attach as context", FileAction::Attach),
            copy_menu(),
            MenuEntry::Divider,
            item("Show in file tree", FileAction::ShowInTree),
            MenuEntry::Divider,
            MenuEntry::Action { label: "Keep open", shortcut: None, action: FileAction::KeepOpen, enabled: preview },
            item("Close file", FileAction::Close),
            item("Close others", FileAction::CloseOthers),
            item("Close all", FileAction::CloseAll),
        ],
    }
}

/// Whether a submenu opens to the left of its menu: when the window has no room for the menu and a
/// submenu of the same width to the right of the menu's left edge.
pub fn submenu_opens_left(menu_x: f64, window_width: f64, menu_width: f64) -> bool {
    menu_x + 2.0 * menu_width > window_width
}
```

(Note the tree row menu has no `Close file` for a row that is not open: the action is then a no-op; Task 4 hides nothing.)

- [ ] **Step 4: Run** — pass. **Step 5: Commit** — `feat: the file menu as data (Plan 25C-2)`.

---

### Task 4: The widget, the actions and the panel wiring

**Files:**
- Create: `crates/forge-widgets/src/context_menu.rs`
- Modify: `crates/forge-widgets/src/lib.rs`, `crates/forge-widgets/src/files_tabs.rs`, `crates/forge-widgets/src/files_panel.rs`, `crates/forge-widgets/src/files_state.rs`, `crates/forge-entities/src/turn.rs` (+ `lib.rs`: shared `copy_text`), `crates/forge-shared/src/theme.css`

**Interfaces:**
- Consumes: Task 1 `Workspace::open_file`/`report`; Task 2 `FileTree::reveal_dirs`/`open_dir`; Task 3 menu types.
- Produces: `forge_entities::copy_text(text: &str)`; `ContextMenu { entries: Vec<MenuEntry>, x: f64, y: f64, on_select: EventHandler<FileAction>, on_close: EventHandler<()> }`; `FilesCtx::run_action(&self, action: FileAction, path: &str)` (everything except `ShowInTree`); `FilesTabs` loses `on_keep`, `on_close_others`, `on_close_all`, `on_attach` and gains `on_menu: EventHandler<(String, bool, f64, f64)>` (path, preview, x, y).

- [ ] **Step 1: Failing tests.** In `files_state.rs` tests, a pure mapping the panel uses to build the strings the actions need, and the action to behaviour table for the actions that need no UI:

```rust
#[test]
fn the_names_the_copy_actions_put_on_the_clipboard() {
    let cwd = std::path::Path::new("/p");
    assert_eq!(copy_text_for(FileAction::CopyAbsolute, cwd, "src/a b.py"), Some("/p/src/a b.py".to_string()));
    assert_eq!(copy_text_for(FileAction::CopyRelative, cwd, "src/a b.py"), Some("src/a b.py".to_string()));
    assert_eq!(copy_text_for(FileAction::CopyName, cwd, "src/a b.py"), Some("a b.py".to_string()));
    assert_eq!(copy_text_for(FileAction::Close, cwd, "src/a b.py"), None);
}

#[test]
fn only_a_text_file_can_be_copied_as_contents() {
    use forge_ports::{FileContent, FileVersion};
    let version = FileVersion { modified_ns: 1, len: 3 };
    assert_eq!(contents_to_copy(&FileContent::Text { text: "abc".into(), version, lossy: false }), Ok("abc".to_string()));
    assert!(contents_to_copy(&FileContent::Binary).is_err());
    assert!(contents_to_copy(&FileContent::TooLarge).is_err());
    assert!(contents_to_copy(&FileContent::Image { bytes: vec![1], mime: "image/png".into() }).is_err());
}
```

Run `cargo test -p forge-widgets files_state 2>&1 | grep -E "^error" | head -3` → FAIL.

- [ ] **Step 2: Implement the helpers** in `files_state.rs`:

```rust
/// The text a copy action puts on the clipboard, `None` for any other action.
pub fn copy_text_for(action: FileAction, cwd: &std::path::Path, path: &str) -> Option<String> {
    match action {
        FileAction::CopyAbsolute => Some(absolute(cwd, path).to_string_lossy().to_string()),
        FileAction::CopyRelative => Some(path.to_string()),
        FileAction::CopyName => Some(path.rsplit('/').next().unwrap_or(path).to_string()),
        _ => None,
    }
}

/// The text of a file for `Copy file contents`, or a message for the person.
pub fn contents_to_copy(content: &FileContent) -> Result<String, String> {
    match content {
        FileContent::Text { text, .. } => Ok(text.clone()),
        FileContent::TooLarge => Err("The file is too large to copy".to_string()),
        FileContent::Binary | FileContent::Image { .. } => Err("Only a text file can be copied".to_string()),
    }
}
```

(`use forge_features::FileAction;` and `forge_ports::FileContent` are already imported or added.)

- [ ] **Step 3: Shared clipboard function.** In `forge-entities` add (new small `clipboard.rs`, exported from `lib.rs`):

```rust
use dioxus::prelude::*;

/// Puts text on the clipboard (the page's `navigator.clipboard`).
pub fn copy_text(text: &str) {
    let literal = serde_json::to_string(text).unwrap_or_default();
    document::eval(&format!("navigator.clipboard.writeText({literal});"));
}
```

and make `turn.rs` call `copy_text(&text)` in the footer's copy button.

- [ ] **Step 4: `FilesCtx::run_action`:**

```rust
/// A menu item of the tree row or tab menu, for `path` (relative to the session folder).
/// `ShowInTree` is the panel's: it owns the tree.
pub fn run_action(&self, action: FileAction, path: &str) {
    if let Some(text) = copy_text_for(action, &self.cwd, path) {
        forge_entities::copy_text(&text);
        return;
    }
    match action {
        FileAction::Attach => self.attach_file(path),
        FileAction::OpenVsCode => self.workspace.open_file(&absolute(&self.cwd, path), Some("Visual Studio Code")),
        FileAction::OpenDefault => self.workspace.open_file(&absolute(&self.cwd, path), None),
        FileAction::Reveal => self.reveal(path),
        FileAction::FindInFile => self.find_in_file(path, "", forge_ports::SearchOptions::default(), 1),
        FileAction::CopyContents => {
            let (ctx, root, relative) = (self.clone(), self.cwd.clone(), path.to_string());
            let files = self.files();
            spawn(async move {
                match run_blocking(move || files.read(&root, &relative)).await {
                    Some(Ok(content)) => match contents_to_copy(&content) {
                        Ok(text) => forge_entities::copy_text(&text),
                        Err(message) => ctx.workspace.report(message),
                    },
                    Some(Err(error)) => ctx.workspace.report(error.to_string()),
                    None => {}
                }
            });
        }
        // A tab with no edits reads the disk again; one with edits gets the conflict banner (the
        // `Refresh` question), so nothing typed is lost. A file that is not open is simply opened.
        FileAction::Reload => {
            if self.docs.peek().contains_key(path) {
                self.ask(path, Pending::Refresh);
            } else {
                self.open(path.to_string());
            }
        }
        FileAction::Close => self.close(path.to_string()),
        FileAction::KeepOpen => self.keep(path),
        FileAction::CloseOthers => self.close_others(path.to_string()),
        FileAction::CloseAll => self.close_all(),
        FileAction::CopyAbsolute | FileAction::CopyRelative | FileAction::CopyName | FileAction::ShowInTree => {}
    }
}
```

(Remove the 25B `attach_file` caller only if unused; keep `attach_file`. `Workspace::report` must be `pub` from Task 1.)

- [ ] **Step 5: `ContextMenu` widget** (`context_menu.rs`; `pub mod`/`mod` in `lib.rs`):

```rust
use dioxus::prelude::*;
use forge_features::{submenu_opens_left, FileAction, MenuEntry};

const MENU_WIDTH: f64 = 230.0;

#[component]
fn Entry(entry: MenuEntry, on_select: EventHandler<FileAction>) -> Element {
    match entry {
        MenuEntry::Divider => rsx! { div { class: "picker-divider" } },
        MenuEntry::Action { label, shortcut, action, enabled } => rsx! {
            button {
                class: if enabled { "picker-item" } else { "picker-item disabled" },
                r#type: "button",
                role: "menuitem",
                onclick: move |_| {
                    if enabled {
                        on_select.call(action);
                    }
                },
                span { class: "picker-text", "{label}" }
                if let Some(key) = shortcut {
                    span { class: "picker-hint", "{key}" }
                }
            }
        },
        MenuEntry::Submenu { .. } => rsx! {},
    }
}

/// A context menu at a point. A backdrop click, a right click on it and Escape close it; a submenu
/// opens to the right of its item, or to the left near the window's right edge.
#[component]
pub fn ContextMenu(entries: Vec<MenuEntry>, x: f64, y: f64, on_select: EventHandler<FileAction>, on_close: EventHandler<()>) -> Element {
    let mut open_sub = use_signal(|| None::<usize>);
    let mut window_width = use_signal(|| 1200.0_f64);
    use_future(move || async move {
        if let Ok(width) = document::eval("dioxus.send(window.innerWidth)").recv::<f64>().await {
            window_width.set(width);
        }
    });
    let flip = submenu_opens_left(x, window_width(), MENU_WIDTH);
    rsx! {
        div {
            class: "picker-backdrop",
            onclick: move |_| on_close.call(()),
            oncontextmenu: move |event: MouseEvent| {
                event.prevent_default();
                on_close.call(());
            },
        }
        div {
            class: "picker-menu context-menu",
            role: "menu",
            tabindex: "0",
            style: "left: min({x}px, calc(100vw - {MENU_WIDTH}px)); top: min({y}px, calc(100vh - 360px));",
            onmounted: move |mounted| async move {
                let _ = mounted.set_focus(true).await;
            },
            onkeydown: move |event: KeyboardEvent| {
                if event.key() == Key::Escape {
                    event.stop_propagation();
                    on_close.call(());
                }
            },
            for (index, entry) in entries.iter().cloned().enumerate() {
                match entry {
                    MenuEntry::Submenu { label, entries } => rsx! {
                        div {
                            key: "{index}",
                            class: "context-sub",
                            onmouseenter: move |_| open_sub.set(Some(index)),
                            button { class: "picker-item", r#type: "button", role: "menuitem", "aria-haspopup": "menu",
                                span { class: "picker-text", "{label}" }
                                span { class: "picker-hint", "›" }
                            }
                            if open_sub() == Some(index) {
                                div { class: if flip { "picker-menu context-submenu left" } else { "picker-menu context-submenu" }, role: "menu",
                                    for (sub_index, sub) in entries.iter().cloned().enumerate() {
                                        Entry { key: "{sub_index}", entry: sub, on_select }
                                    }
                                }
                            }
                        }
                    },
                    other => rsx! {
                        div { key: "{index}", onmouseenter: move |_| open_sub.set(None),
                            Entry { entry: other, on_select }
                        }
                    },
                }
            }
        }
    }
}
```

CSS:

```css
.context-menu { min-width: 230px; }
.context-sub { position: relative; }
.context-submenu { position: absolute; top: -4px; left: 100%; min-width: 180px; }
.context-submenu.left { left: auto; right: 100%; }
.picker-item.disabled { opacity: .45; cursor: default; }
.picker-hint { margin-left: auto; padding-left: 16px; color: var(--fg-muted); }
```

(Check `.picker-menu`'s positioning: if it is `position: fixed`, the submenu needs `position: absolute` which the rule above sets; and `.picker-item` should be a flex row so the hint sits right: add `display: flex` if the existing rule lacks it.)

- [ ] **Step 6: Replace the two menus.**
  - `files_tabs.rs`: delete the local `menu` signal and its markup; the right click handler becomes `event.prevent_default(); let point = event.client_coordinates(); on_menu.call((path.clone(), preview, point.x, point.y));`; remove props `on_keep`, `on_close_others`, `on_close_all`, `on_attach`, add `on_menu: EventHandler<(String, bool, f64, f64)>`.
  - `files_panel.rs`: replace `row_menu` with `let mut menu = use_signal(|| None::<(MenuTarget, String, f64, f64)>);`; a file row's `oncontextmenu` sets `menu.set(Some((MenuTarget::Tree, path.clone(), x, y)))` only when `!is_dir` (a directory row gets `prevent_default` and nothing else); `FilesTabs { on_menu: move |(path, preview, x, y)| menu.set(Some((MenuTarget::Tab { preview }, path, x, y))) }`; render:

```rust
let menu_view = match menu() {
    Some((target, path, x, y)) => {
        let (ctx, run_path) = (ctx.clone(), path.clone());
        rsx! {
            ContextMenu {
                entries: file_menu(target),
                x,
                y,
                on_close: move |_| menu.set(None),
                on_select: move |action: FileAction| {
                    menu.set(None);
                    if action == FileAction::ShowInTree {
                        show_in_tree(run_path.clone());
                    } else {
                        ctx.run_action(action, &run_path);
                    }
                },
            }
        }
    }
    None => rsx! {},
};
```

  - `show_in_tree` is a closure in the panel (it owns `tree`, `problem`, `filter`, `workspace`, `ctx`):

```rust
let show_in_tree = {
    let (cwd, ctx) = (cwd.clone(), ctx.clone());
    move |path: String| {
        workspace.update_editor_prefs(|prefs| prefs.tree = true);
        filter.set(String::new());
        let (cwd, ctx) = (cwd.clone(), ctx.clone());
        spawn(async move {
            for dir in FileTree::reveal_dirs(&path) {
                if tree.write().open_dir(&dir) {
                    load_dir(workspace.files(), cwd.clone(), dir, tree, problem).await;
                }
            }
            // The row of the selected tab is the one with the grey band: select the file's tab.
            if ctx.tabs.peek().paths().contains(&path) {
                ctx.select(&path);
            }
            let literal = serde_json::to_string(&path).unwrap_or_default();
            document::eval(&format!("document.querySelector('[data-path=' + JSON.stringify({literal}) + ']')?.scrollIntoView({{ block: 'center' }});"));
        });
    }
};
```

  and each tree row button gets `"data-path": "{row.path}"`. (`document::eval` with a JSON literal inside a CSS attribute selector: use `CSS.escape` free approach — `[data-path="…"]` with the JSON string is valid CSS for strings without quotes/backslashes issues since JSON.stringify escapes `"` and `\`; verify with a file name containing a space and a quote in the real window.)

- [ ] **Step 7: Run** — `cargo test --workspace 2>&1 | grep "test result" | awk '{s+=$4; f+=$6} END {print s" passed, "f" failed"}'` → 0 failed; `cargo build` clean.
- [ ] **Step 8: Commit** — `feat: tree row and tab menus with copy, open in, show in tree, find, reload and close (Plan 25C-2)`.

---

### Task 5: Go to line

**Files:**
- Create: `tools/editor-bundle/src/goto.js`
- Modify: `tools/editor-bundle/src/main.js`, `tools/editor-bundle/src/lines.js`, `tools/editor-bundle/test/bundle.test.mjs`, rebuilt `crates/forge-shared/assets/editor.js`, `crates/forge-shared/src/theme.css`

**Interfaces:**
- `goto.js` exports `gotoTools()` returning `{ extensions, open(view) }`; `lineTools({ send, getKey, onGoto })`: the right click menu gets `Go to line` with the hint `⌃G` that calls `onGoto(view)`; the editor handle gets `openGoto()` (for tests).

- [ ] **Step 1: Failing tests** (append; reuse `boot`, `viewOf`, `mouse`):

```js
const gotoInput = (w) => w.document.querySelector(".cm-goto-bar input");
const enter = (w, input) => input.dispatchEvent(new w.KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
const head = (w) => viewOf(w).state.doc.lineAt(viewOf(w).state.selection.main.head).number;

test("Go to line opens a one field panel and Enter jumps to the line and closes it", () => {
  const { w, editor } = boot();
  editor.load("a.py", "a\nb\nc\nd\n", "python", false);
  editor.openGoto();
  const input = gotoInput(w);
  assert.ok(input);
  input.value = "3";
  enter(w, input);
  assert.equal(head(w), 3);
  assert.equal(gotoInput(w), null);
  editor.destroy();
});

test("a line past the end goes to the last line", () => {
  const { w, editor } = boot();
  editor.load("a.py", "a\nb\nc\n", "python", false);
  editor.openGoto();
  const input = gotoInput(w);
  input.value = "99";
  enter(w, input);
  assert.equal(head(w), viewOf(w).state.doc.lines);
  editor.destroy();
});

test("0, a negative and text are ignored and keep the panel open", () => {
  const { w, editor } = boot();
  editor.load("a.py", "a\nb\nc\n", "python", false);
  editor.openGoto();
  for (const bad of ["0", "-2", "abc", ""]) {
    const input = gotoInput(w);
    input.value = bad;
    enter(w, input);
    assert.ok(gotoInput(w), `panel stays for ${JSON.stringify(bad)}`);
    assert.equal(head(w), 1);
  }
  editor.destroy();
});

test("Escape closes the Go to line panel", () => {
  const { w, editor } = boot();
  editor.load("a.py", "a\n", "python", false);
  editor.openGoto();
  gotoInput(w).dispatchEvent(new w.KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
  assert.equal(gotoInput(w), null);
  editor.destroy();
});

test("the editor right click menu lists Go to line with its shortcut", () => {
  const { w, editor } = boot();
  editor.load("a.py", "a\nb\n", "python", false);
  const content = w.document.querySelector(".cm-content");
  content.dispatchEvent(new w.MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: 5, clientY: 5 }));
  const texts = [...w.document.querySelectorAll(".cm-ctx-menu button")].map((b) => b.textContent);
  assert.ok(texts.some((t) => t.startsWith("Go to line") && t.includes("⌃G")), texts.join("|"));
  editor.destroy();
});
```

Run `cd tools/editor-bundle && npm run build && npm test 2>&1 | grep -E "^# (pass|fail)|^not ok"` → the five FAIL.

- [ ] **Step 2: Implement `goto.js`:**

```js
// Go to line: a one field panel above the editor (like the find bar). Enter jumps (a number past the
// end goes to the last line), Escape closes; 0, a negative number and text are ignored.
import { StateEffect, StateField } from "@codemirror/state";
import { EditorView, keymap, showPanel } from "@codemirror/view";

const setGoto = StateEffect.define();

const gotoField = StateField.define({
  create: () => false,
  update(open, tr) {
    for (const e of tr.effects) if (e.is(setGoto)) return e.value;
    return open;
  },
});

export function gotoTools() {
  function open(view) {
    view.dispatch({ effects: setGoto.of(true) });
    view.dom.querySelector(".cm-goto-bar input")?.focus();
  }

  function close(view) {
    view.dispatch({ effects: setGoto.of(false) });
    view.focus();
  }

  function makePanel(view) {
    const dom = document.createElement("div");
    dom.className = "cm-goto-bar";
    const input = document.createElement("input");
    input.type = "text";
    input.inputMode = "numeric";
    input.placeholder = `Go to line (1-${view.state.doc.lines})`;
    input.addEventListener("keydown", (event) => {
      if (event.key === "Enter" && !event.isComposing && event.keyCode !== 229) {
        event.preventDefault();
        const n = Number.parseInt(input.value.trim(), 10);
        if (!Number.isFinite(n) || n < 1) return;
        const line = view.state.doc.line(Math.min(n, view.state.doc.lines));
        view.dispatch({ selection: { anchor: line.from }, effects: [setGoto.of(false), EditorView.scrollIntoView(line.from, { y: "center" })] });
        view.focus();
      } else if (event.key === "Escape") {
        event.preventDefault();
        close(view);
      }
    });
    dom.appendChild(input);
    return { dom, top: true, mount: () => input.focus() };
  }

  const panel = showPanel.compute([gotoField], (state) => (state.field(gotoField) ? makePanel : null));
  const keys = keymap.of([{ key: "Ctrl-g", run: (view) => { open(view); return true; } }]);
  return { extensions: [gotoField, panel, keys], open };
}
```

- [ ] **Step 3: Wire.** In `main.js`: `import { gotoTools } from "./goto.js";`, `const go = gotoTools();` before `const tools = lineTools(...)`, which becomes `lineTools({ send, getKey: () => key, onGoto: (view) => go.open(view) })`; add `...go.extensions` next to `...find.extensions` in `makeState`; add the handle method `openGoto() { go.open(view); },`. In `lines.js`: `lineTools({ send, getKey, onGoto })`; in `openMenu`, items become `[label, run, hint]` and the button renders the hint as a trailing span:

```js
["Select all", () => selectAll(view)],
null,
["Go to line", () => onGoto(view), "⌃G"],
```

and where the button is built: `button.textContent = item[0]; if (item[2]) { const hint = document.createElement("span"); hint.className = "cm-ctx-key"; hint.textContent = item[2]; button.appendChild(hint); }` (a test reads `textContent`, which then holds `Go to line⌃G`).

- [ ] **Step 4: CSS:**

```css
.cm-goto-bar { padding: 4px 8px; background: var(--bg); border-bottom: 1px solid var(--border); }
.cm-goto-bar input { width: 100%; box-sizing: border-box; padding: 3px 6px; border: 1px solid var(--border); border-radius: 6px; background: transparent; color: var(--fg); font-size: 12px; outline: none; }
.cm-ctx-menu button { display: flex; justify-content: space-between; gap: 16px; }
.cm-ctx-key { color: var(--fg-muted); }
```

- [ ] **Step 5: Run** — `npm run build && npm test` → all pass (the old ones too). If jsdom cannot dispatch the contextmenu through CodeMirror, build the menu through `editor.openMenuForTest`-free means: dispatch `contextmenu` on `.cm-content` as above; if that does not reach CodeMirror's handler in jsdom, drop that one test and check the menu in Task 6, saying so in the commit.
- [ ] **Step 6: Commit** (with the rebuilt `editor.js`) — `feat: Go to line (Plan 25C-2)`.

---

### Task 6: Check in the real window, fix, write down what was found

**Files:**
- Modify: `docs/superpowers/specs/2026-10-05-file-menus-design.md` (As built + results), `docs/superpowers/roadmap/*.md` if it tracks PNL-04

- [ ] **Step 1: Whole run** — `cargo test --workspace` and `cd tools/editor-bundle && npm test` green; record the counts.
- [ ] **Step 2: Drive the app** with the harness (memory `driving-the-forge-app-visually`; screen unlocked; `scripts/harness/launch.sh`; open a session; Files from the session `⋮` menu (it toggles); create files in `/private/tmp/forge-harness/my-app` including a nested `src/deep/a.txt` and a file name with a space; right click and drag need the small Quartz script (`ctl.py right X Y`, screen = window origin + pixels / 2; recreate it if the scratchpad copy is gone); `click.py` takes pixels of the 2400 wide capture, viewer coordinates times 1.2; a Korean input method is on: use key codes). Check, one line each:
  1. Right click a tree file row: the menu with the 9 items in the observed order, dividers right; `Copy >` opens its submenu to the right; near the right edge a submenu opens to the left.
  2. `Copy > Absolute path`, `Relative path`, `File name`: paste into the message box (⌘V may not reach the page: check with `pbpaste` in a shell).
  3. `Open in > Default app` opens the file; `Open in > VS Code` opens VS Code or shows a notice when it is not installed.
  4. `Show in Finder` reveals the file.
  5. `Show in file tree` on a tab of `src/deep/a.txt` with the tree collapsed: the folders open and the row is selected and in view; with the tree hidden it turns the tree on.
  6. `Find in file` opens the find bar with an empty field.
  7. `Copy file contents` of a text file (check with `pbpaste`); of a binary file: a notice and the clipboard unchanged.
  8. `Reload file` after changing the file from a shell: the editor shows the new text; with an unsaved edit pending: the conflict banner, nothing lost.
  9. `Close file`; the tab menu: `Keep open` is grey on an upright tab and works on an italic one; `Close others`, `Close all`.
  10. Right click a directory row: no menu.
  11. Editor right click menu: `Go to line ⌃G`; the panel opens, Enter jumps, a number past the end goes to the last line, Escape closes; `Ctrl-g` opens it.
  12. Dark mode if the harness can show it, else note not checked.
  Each defect found: failing test if one can exist, fix, commit `fix: …`. Clean up the files and the harness afterwards.
- [ ] **Step 3: Docs** — "As built" and "Checked in the real window" sections in the spec; the roadmap status for PNL-04 if tracked.
- [ ] **Step 4: Commit** — `docs: Plan 25C-2 checked in the real window`.
- [ ] **Step 5: Finish** — invoke `superpowers:finishing-a-development-branch`; the base is `master`. Do not merge without the user's word.
