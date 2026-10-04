# Files Panel as an Editor Implementation Plan (Plan 25A)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The Files panel edits files like the real Claude desktop app: a CodeMirror editor with auto save (about 2 s) and a disk-conflict banner, the `⋮` menu settings, tabs (italic preview tab, keep open, four closes, drag to reorder), and screens for images, binary and too large files.

**Architecture:** CodeMirror 6 is bundled once into `crates/forge-shared/assets/editor.js` (built from `tools/editor-bundle/`, committed, no Node needed to build Forge) and driven from Rust over `document::eval`, like the terminal. Rust owns the rules: `FilesPort` gains `stat`/`write` with a version check, `EditorTabs` and `TabDoc` are pure models with tests, `EditorPrefs` lives in `AppConfig`. The panel is split into small files (bridge, kinds, tabs, menu, state, panel).

**Tech Stack:** Rust 2021, Dioxus 0.7.10, tokio, serde, CodeMirror 6 (JavaScript, bundled with esbuild).

**Spec:** `docs/superpowers/specs/2026-10-04-files-editor-design.md` (observations: `docs/superpowers/roadmap/observations.md`, Round 9).

## Global Constraints

- UI text is English (the real app is Korean; Forge's UI is English).
- Editor settings are kept for the whole of Forge in `AppConfig.editor` (`EditorPrefs`), not per session. Defaults: `tree`, `preview_tabs`, `auto_save`, `tab_indent`, `wrap`, `indent_guides` on; `whitespace` and `scroll_past_end` off; `tab_size` 4. A stored `tab_size` that is not 2, 4 or 8 is used as 4.
- `READ_LIMIT` is `10 * 1024 * 1024`. A content search still reads no file over 1 MiB (`SEARCH_FILE_BYTES`).
- `FilesPort::write` writes a temporary file in the same folder and renames it over the target, keeps the file's permissions, and never leaves the temporary file behind. Every path stays inside the session folder (the existing `resolve`).
- Auto save writes 2 s after the last change (the page debounces; it sends the text once per pause, never per key, and never during an IME composition).
- A file that is not valid UTF-8, and a binary file shown with "Show as text anyway", open read only. They are never written.
- The editor bundle is inline in the window head, one `<script>` and no network; its licence file is `crates/forge-shared/assets/EDITOR-LICENSE`. Colours come from the existing CSS variables (`--bg`, `--fg`, `--hl-*`); no new `prefers-color-scheme` block.
- Image extensions: `png jpg jpeg gif webp bmp ico svg` (any case). Editor languages: JavaScript and TypeScript (`js mjs cjs jsx ts mts cts tsx`), Python (`py pyi`), Rust (`rs`), JSON (`json jsonc`), Markdown (`md markdown`), HTML (`html htm`), CSS, YAML (`yml yaml`), shell (`sh bash zsh`); anything else is plain text.
- Added beyond the spec: `⌘S` in the editor saves the tab (needed when auto save is off); the confirmation for a close is an inline row in the panel (the code base has no modal dialogs).
- Keep every existing test passing; counts must not drop. The workspace had 968 passing Rust tests when this plan was written (`cargo test --workspace`), the sidecar is untouched.
- Commit messages end with `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.
- Out of scope (Plans 25B and 25C): line selection bar, cards and comments, the `?` search toggles and in-file find bar, right click items that attach, copy or open elsewhere, ⇧⌘F and ⇧⌘Y, creating, renaming or deleting files.

## Review Focus

- A text file that is not valid UTF-8 (bytes `61 ff 62`) is opened, then someone types: the tab is read only and the file on disk is never rewritten (Tasks 2, 8, 10).
- "Show as text anyway" on a binary file: the tab is read only, the page refuses edits, nothing is ever written back (Tasks 8, 10).
- Text typed while a write is in flight, and text typed less than 2 s before a tab is closed: neither is lost, the close asks or saves the text the page really holds (Tasks 5, 10).
- A file deleted, or its folder made read only, while open: a "save failed" banner with Retry, the text stays in the editor, auto save pauses (Tasks 2, 5, 10).
- Two files with the same name in different folders (`a/x.py`, `b/x.py`): two tabs, the title shows the name and the tooltip the full path, closing one leaves the other (Tasks 4, 9).

## File Structure

- `tools/editor-bundle/` — `package.json`, `package-lock.json`, `build.mjs`, `README.md`, `src/main.js` (create).
- `crates/forge-shared/assets/editor.js`, `EDITOR-LICENSE` — the built bundle and licence (create).
- `crates/forge-shared/src/editor.rs`, `lib.rs`, `theme.css`, `theme.rs` — `editor_head()`, styles, class-list test (create / modify).
- `crates/forge-app/src/main.rs` — the head (modify).
- `crates/forge-domain/src/config.rs`, `lib.rs` — `EditorPrefs` (modify).
- `crates/forge-application/src/use_cases/settings.rs` — `save_editor` (modify).
- `crates/forge-ports/src/files_port.rs`, `browser_port.rs`, `lib.rs` — new port shapes (modify).
- `crates/forge-adapter-files/src/lib.rs`, `crates/forge-adapter-browser/src/lib.rs` — the adapters (modify).
- `crates/forge-features/src/editor_tabs.rs`, `editor_doc.rs`, `lib.rs`, `workspace.rs` — pure models and workspace accessors (create / modify).
- `crates/forge-widgets/src/files_editor.rs`, `files_kinds.rs`, `files_tabs.rs`, `files_menu.rs`, `files_state.rs`, `files_panel.rs`, `lib.rs` — the panel (create / rewrite).

---

### Task 1: `EditorPrefs`, saving it, and the workspace accessors

**Files:**
- Modify: `crates/forge-domain/src/config.rs`, `crates/forge-domain/src/lib.rs`
- Modify: `crates/forge-application/src/use_cases/settings.rs`
- Modify: `crates/forge-features/src/workspace.rs`
- Test: the same domain and use case files (their `tests` modules)

**Interfaces:**
- Produces:
  - `forge_domain::EditorPrefs { tree, preview_tabs, auto_save, tab_indent, wrap, indent_guides, whitespace, scroll_past_end: bool, tab_size: u8 }` with `Default`, `clamped_tab_size(&self) -> u8`; `forge_domain::DEFAULT_TAB_SIZE`.
  - `AppConfig.editor: EditorPrefs` (`#[serde(default)]`).
  - `SettingsUseCase::save_editor(&self, prefs: &EditorPrefs) -> Result<AppConfig, ConfigError>`.
  - `Workspace::editor_prefs(&self) -> EditorPrefs`, `Workspace::update_editor_prefs(&self, change: impl FnOnce(&mut EditorPrefs))`.

- [ ] **Step 1: Write the failing domain tests**

Add to the `tests` module at the end of `crates/forge-domain/src/config.rs`:

```rust
    #[test]
    fn an_old_config_without_editor_preferences_loads_with_the_defaults() {
        let config: AppConfig = serde_json::from_str(r#"{"base_url":null,"api_key":null}"#).unwrap();

        assert_eq!(config.editor, EditorPrefs::default());
        let editor = &config.editor;
        assert!(editor.tree && editor.preview_tabs && editor.auto_save && editor.tab_indent && editor.wrap && editor.indent_guides);
        assert!(!editor.whitespace && !editor.scroll_past_end);
        assert_eq!(editor.tab_size, DEFAULT_TAB_SIZE);
    }

    #[test]
    fn editor_preferences_survive_a_round_trip() {
        let prefs = EditorPrefs { tree: false, auto_save: false, wrap: false, whitespace: true, tab_size: 8, ..EditorPrefs::default() };
        let config = AppConfig { editor: prefs.clone(), ..AppConfig::default() };

        let back: AppConfig = serde_json::from_str(&serde_json::to_string(&config).unwrap()).unwrap();

        assert_eq!(back.editor, prefs);
    }

    #[test]
    fn a_config_that_names_only_some_editor_fields_keeps_the_defaults_for_the_rest() {
        let config: AppConfig = serde_json::from_str(r#"{"editor":{"wrap":false}}"#).unwrap();

        assert!(!config.editor.wrap);
        assert!(config.editor.tree);
        assert_eq!(config.editor.tab_size, 4);
    }

    #[test]
    fn a_tab_size_other_than_2_4_or_8_is_used_as_4() {
        for stored in [0u8, 1, 3, 5, 16, 255] {
            assert_eq!(EditorPrefs { tab_size: stored, ..EditorPrefs::default() }.clamped_tab_size(), 4, "stored {stored}");
        }
        for stored in [2u8, 4, 8] {
            assert_eq!(EditorPrefs { tab_size: stored, ..EditorPrefs::default() }.clamped_tab_size(), stored);
        }
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p forge-domain editor 2>&1 | tail -15`
Expected: compile error, `EditorPrefs` not found.

- [ ] **Step 3: Implement `EditorPrefs`**

In `crates/forge-domain/src/config.rs`, after `impl SidebarPrefs { ... }` and before `pub struct AppConfig`, add:

```rust
pub const DEFAULT_TAB_SIZE: u8 = 4;

/// How the Files panel's editor behaves: the settings of its `⋮` menu. They are kept for the whole
/// of Forge, not per session. A field missing from the stored config reads as its default.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct EditorPrefs {
    /// Whether the file tree is shown.
    pub tree: bool,
    /// Whether a file opens in an italic preview tab that the next file replaces.
    pub preview_tabs: bool,
    pub auto_save: bool,
    /// Whether the Tab key indents (else it moves focus).
    pub tab_indent: bool,
    pub wrap: bool,
    pub indent_guides: bool,
    pub whitespace: bool,
    pub scroll_past_end: bool,
    pub tab_size: u8,
}

impl Default for EditorPrefs {
    fn default() -> Self {
        Self {
            tree: true,
            preview_tabs: true,
            auto_save: true,
            tab_indent: true,
            wrap: true,
            indent_guides: true,
            whitespace: false,
            scroll_past_end: false,
            tab_size: DEFAULT_TAB_SIZE,
        }
    }
}

impl EditorPrefs {
    /// The tab size to use: a hand-edited value other than 2, 4 or 8 reads as 4.
    pub fn clamped_tab_size(&self) -> u8 {
        match self.tab_size {
            2 | 4 | 8 => self.tab_size,
            _ => DEFAULT_TAB_SIZE,
        }
    }
}
```

In the same file, in `pub struct AppConfig` add after the `sidebar` field:

```rust
    #[serde(default)]
    pub editor: EditorPrefs,
```

In `crates/forge-domain/src/lib.rs`, change the `config` re-export to:

```rust
pub use config::{
    normalize_setting, AppConfig, ArchivedRef, EditorPrefs, GroupBy, SidebarPrefs, SortBy, StatusFilter, DEFAULT_SIDEBAR_WIDTH, DEFAULT_TAB_SIZE,
    MAX_SIDEBAR_WIDTH, MIN_SIDEBAR_WIDTH,
};
```

- [ ] **Step 4: Fix the `AppConfig` literals that name every field**

Run: `cargo test --workspace --no-run 2>&1 | grep -B3 -A8 "missing field"`
Expected: errors for any `AppConfig { ... }` literal without `..AppConfig::default()`. For each, add `editor: forge_domain::EditorPrefs::default(),` (or `editor: EditorPrefs::default(),` where `EditorPrefs` is imported). Repeat until `cargo test --workspace --no-run` compiles.

- [ ] **Step 5: Run the domain tests**

Run: `cargo test -p forge-domain 2>&1 | tail -8`
Expected: all pass, including the four new tests.

- [ ] **Step 6: Write the failing use case test**

In `crates/forge-application/src/use_cases/settings.rs`, in the `tests` module after `save_sidebar_replaces_only_the_sidebar_preferences`, add:

```rust
    #[tokio::test]
    async fn save_editor_replaces_only_the_editor_preferences() {
        let mut mock = MockConfigRepositoryPort::new();
        mock.expect_load().times(1).returning(|| Ok(with_lists()));
        mock.expect_save()
            .withf(|c: &AppConfig| {
                !c.editor.wrap
                    && c.editor.tab_size == 2
                    && c.base_url.as_deref() == Some("https://old.test")
                    && c.enabled_skills == vec!["brainstorming".to_string()]
            })
            .times(1)
            .returning(|_| Ok(()));
        let usecase = SettingsUseCase::new(Arc::new(mock));
        let prefs = forge_domain::EditorPrefs { wrap: false, tab_size: 2, ..forge_domain::EditorPrefs::default() };

        let saved = usecase.save_editor(&prefs).await.unwrap();

        assert_eq!(saved.editor, prefs);
    }

    #[tokio::test]
    async fn save_editor_does_not_replace_a_config_it_cannot_read() {
        let mut mock = MockConfigRepositoryPort::new();
        mock.expect_load().times(1).returning(|| Err(ConfigError::ParseFailed("trailing comma".to_string())));
        // No `expect_save`: a save would panic.
        let usecase = SettingsUseCase::new(Arc::new(mock));

        let result = usecase.save_editor(&forge_domain::EditorPrefs::default()).await;

        assert!(matches!(result, Err(ConfigError::ParseFailed(_))));
    }
```

- [ ] **Step 7: Run to verify they fail**

Run: `cargo test -p forge-application save_editor 2>&1 | tail -10`
Expected: compile error, no method `save_editor`.

- [ ] **Step 8: Implement `save_editor`**

In `crates/forge-application/src/use_cases/settings.rs`, add `EditorPrefs` to the `forge_domain` import at the top, and after `save_sidebar` add:

```rust
    /// Saves how the Files panel's editor behaves. Like `save_sidebar`, a config that cannot be read
    /// is never replaced, and it shares the write lock.
    pub async fn save_editor(&self, prefs: &EditorPrefs) -> Result<AppConfig, ConfigError> {
        let _saving = self.write_lock.lock().await;
        let mut config = self.config.load().await?;
        config.editor = prefs.clone();
        self.config.save(&config).await?;
        Ok(config)
    }
```

- [ ] **Step 9: Run the use case tests**

Run: `cargo test -p forge-application save_editor 2>&1 | tail -8`
Expected: both pass.

- [ ] **Step 10: Add the workspace accessors**

In `crates/forge-features/src/workspace.rs`:

1. Add `EditorPrefs` to the `forge_domain::{...}` import list.
2. In `pub struct Workspace`, after `prefs: Signal<SidebarPrefs>,` add `editor: Signal<EditorPrefs>,`.
3. In `use_workspace_provider`, after `prefs: Signal::new(SidebarPrefs::default()),` add `editor: Signal::new(EditorPrefs::default()),`.
4. In `reload_prefs`, add `let mut editor = self.editor;` next to the other `let mut` lines and `editor.set(config.editor);` after `prefs.set(config.sidebar);`.
5. After `save_prefs` add:

```rust
    /// How the Files panel's editor behaves (its `⋮` menu).
    pub fn editor_prefs(&self) -> EditorPrefs {
        self.editor.read().clone()
    }

    /// Changes the editor preferences and saves them.
    pub fn update_editor_prefs(&self, change: impl FnOnce(&mut EditorPrefs)) {
        let mut editor = self.editor;
        change(&mut editor.write());
        let this = *self;
        let services = (*self.services.peek()).clone();
        let prefs = self.editor_prefs();
        // Root scope: the save must finish even if the menu that asked for it unmounts.
        dioxus::core::spawn_forever(async move {
            if let Err(error) = services.settings.save_editor(&prefs).await {
                this.report(format!("failed to save the editor settings: {error}"));
            }
        });
    }
```

- [ ] **Step 11: Build and run the whole suite**

Run: `cargo test --workspace 2>&1 | grep -E "^test result|FAILED|error" | head`
Expected: no failures, no errors.

- [ ] **Step 12: Commit**

```bash
git add -A crates
git commit -m "feat: editor preferences in the config, saved by the settings use case, read by the workspace

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 2: `FilesPort`: versions, content kinds, `stat`, `write`, `read_text_anyway`

**Files:**
- Modify: `crates/forge-ports/src/files_port.rs`, `crates/forge-ports/src/lib.rs`
- Modify: `crates/forge-adapter-files/src/lib.rs`
- Modify: `crates/forge-widgets/src/files_panel.rs` (only the three match arms, so it compiles)
- Test: `crates/forge-adapter-files/src/lib.rs` (its `tests` module)

**Interfaces:**
- Produces (Tasks 5, 8, 10 use these exact shapes):
  - `pub struct FileVersion { pub modified_ns: u128, pub len: u64 }` (`Debug, Clone, Copy, PartialEq, Eq`).
  - `pub enum FileContent { Text { text: String, version: FileVersion, lossy: bool }, Image { bytes: Vec<u8>, mime: String }, Binary, TooLarge }`.
  - `pub enum WriteError { Changed, Outside, Failed(String) }` (`thiserror`, `Debug, Clone, PartialEq, Eq`).
  - on `FilesPort`: `fn read_text_anyway(&self, root: &Path, path: &str) -> Result<FileContent, FilesError>`, `fn stat(&self, root: &Path, path: &str) -> Result<FileVersion, FilesError>`, `fn write(&self, root: &Path, path: &str, text: &str, expected: Option<FileVersion>) -> Result<FileVersion, WriteError>`.
  - `READ_LIMIT` becomes 10 MiB.

- [ ] **Step 1: Change the port**

Replace `crates/forge-ports/src/files_port.rs` from the top through the end of `pub enum FilesError { ... }` with:

```rust
use std::path::Path;

use thiserror::Error;

#[cfg(any(test, feature = "mocks"))]
use mockall::automock;

/// The most bytes of a file that `FilesPort::read` shows: 10 MiB (the real app opened a 10 MiB
/// text file and refused a 15 MiB one).
pub const READ_LIMIT: usize = 10 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
}

/// When a file was last changed and how long it is. Two reads that give the same version saw the
/// same file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileVersion {
    pub modified_ns: u128,
    pub len: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileContent {
    /// `lossy` is true when the bytes were not valid UTF-8 and some were replaced; such a file must
    /// not be written back.
    Text { text: String, version: FileVersion, lossy: bool },
    /// A picture, by its extension; `mime` is for a `data:` address.
    Image { bytes: Vec<u8>, mime: String },
    /// The file has a NUL byte near its start.
    Binary,
    /// The file is over `READ_LIMIT`.
    TooLarge,
}

/// A line of a file that holds the searched text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentHit {
    pub path: String,
    pub line: usize,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FilesError {
    #[error("That path is outside the session folder")]
    Outside,
    #[error("{0}")]
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WriteError {
    #[error("The file changed on disk since it was read")]
    Changed,
    #[error("That path is outside the session folder")]
    Outside,
    #[error("{0}")]
    Failed(String),
}
```

In the `FilesPort` trait, after `read` add:

```rust
    /// The file's bytes as text even when it looks binary (invalid UTF-8 is replaced). `TooLarge`
    /// over `READ_LIMIT`. For "Show as text anyway"; the result must be shown read only.
    fn read_text_anyway(&self, root: &Path, path: &str) -> Result<FileContent, FilesError>;
    /// The file's version now.
    fn stat(&self, root: &Path, path: &str) -> Result<FileVersion, FilesError>;
    /// Replaces the file's text. With `Some(version)` it first checks the file is still at that
    /// version and gives `WriteError::Changed` when it is not; with `None` it overwrites. The text
    /// goes to a temporary file in the same folder that is renamed over the target, so a crash
    /// leaves the old or the new file whole. Returns the version after the write.
    fn write(&self, root: &Path, path: &str, text: &str, expected: Option<FileVersion>) -> Result<FileVersion, WriteError>;
```

In the `tests` module at the bottom of `files_port.rs` add:

```rust
    #[test]
    fn write_errors_read_as_messages() {
        assert_eq!(WriteError::Changed.to_string(), "The file changed on disk since it was read");
        assert_eq!(WriteError::Failed("denied".to_string()).to_string(), "denied");
    }
```

In `crates/forge-ports/src/lib.rs` change the files re-export line to:

```rust
pub use files_port::{ContentHit, DirEntry, FileContent, FileVersion, FilesError, FilesPort, WriteError, READ_LIMIT};
```

- [ ] **Step 2: Write the failing adapter tests**

In `crates/forge-adapter-files/src/lib.rs`, in the `tests` module: change the import line to `use forge_ports::{ContentHit, DirEntry, FileContent, FileVersion, FilesError, FilesPort, WriteError, READ_LIMIT};`, and replace the three tests `a_text_file_is_read_with_its_content`, `a_file_with_a_nul_byte_is_binary_and_a_big_file_is_too_large` and `text_that_is_not_utf8_is_read_lossily` with:

```rust
    #[test]
    fn a_text_file_is_read_with_its_content_and_version() {
        let dir = tree();
        let files = LocalFiles::new();
        let FileContent::Text { text, version, lossy } = files.read(dir.path(), "README.md").unwrap() else { panic!("not text") };
        assert_eq!(text, "# Title\nhello World\n");
        assert!(!lossy);
        assert_eq!(version.len, 20);
        assert_eq!(version, files.stat(dir.path(), "README.md").unwrap());
    }

    #[test]
    fn a_file_with_a_nul_byte_is_binary_and_a_big_file_is_too_large() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("b.bin"), [1u8, 2, 0, 3]).unwrap();
        fs::write(dir.path().join("big.txt"), vec![b'a'; READ_LIMIT + 1]).unwrap();
        fs::write(dir.path().join("exact.txt"), vec![b'a'; READ_LIMIT]).unwrap();
        let files = LocalFiles::new();
        assert_eq!(files.read(dir.path(), "b.bin").unwrap(), FileContent::Binary);
        assert_eq!(files.read(dir.path(), "big.txt").unwrap(), FileContent::TooLarge);
        assert!(matches!(files.read(dir.path(), "exact.txt").unwrap(), FileContent::Text { .. }));
    }

    #[test]
    fn text_that_is_not_utf8_is_read_lossily_and_marked_so() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("l.txt"), [b'a', 0xff, b'b']).unwrap();
        let FileContent::Text { text, lossy, .. } = LocalFiles::new().read(dir.path(), "l.txt").unwrap() else { panic!("not text") };
        assert_eq!(text, "a\u{fffd}b");
        assert!(lossy, "a file that is not valid UTF-8 must be marked so it is never written back");
    }

    #[test]
    fn an_image_is_read_as_bytes_by_its_extension_in_any_case() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.PNG"), [0x89, b'P', b'N', b'G', 0]).unwrap();
        fs::write(dir.path().join("v.svg"), "<svg/>").unwrap();
        let files = LocalFiles::new();
        assert_eq!(
            files.read(dir.path(), "a.PNG").unwrap(),
            FileContent::Image { bytes: vec![0x89, b'P', b'N', b'G', 0], mime: "image/png".to_string() }
        );
        assert_eq!(files.read(dir.path(), "v.svg").unwrap(), FileContent::Image { bytes: b"<svg/>".to_vec(), mime: "image/svg+xml".to_string() });
    }

    #[test]
    fn an_image_over_the_limit_is_too_large() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("big.png"), vec![0u8; READ_LIMIT + 1]).unwrap();
        assert_eq!(LocalFiles::new().read(dir.path(), "big.png").unwrap(), FileContent::TooLarge);
    }

    #[test]
    fn text_anyway_reads_a_binary_file_as_text_and_still_refuses_a_big_one() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("b.bin"), [b'a', 0, b'b']).unwrap();
        fs::write(dir.path().join("big.bin"), vec![0u8; READ_LIMIT + 1]).unwrap();
        let files = LocalFiles::new();
        let FileContent::Text { text, .. } = files.read_text_anyway(dir.path(), "b.bin").unwrap() else { panic!("not text") };
        assert_eq!(text, "a\0b");
        assert_eq!(files.read_text_anyway(dir.path(), "big.bin").unwrap(), FileContent::TooLarge);
        assert_eq!(files.read_text_anyway(dir.path(), "../x"), Err(FilesError::Outside));
    }

    fn written(dir: &TempDir, name: &str, text: &str) -> FileVersion {
        fs::write(dir.path().join(name), text).unwrap();
        LocalFiles::new().stat(dir.path(), name).unwrap()
    }

    #[test]
    fn write_replaces_the_file_when_the_version_matches_and_returns_the_new_version() {
        let dir = tempfile::tempdir().unwrap();
        let before = written(&dir, "a.txt", "one");
        let files = LocalFiles::new();

        let after = files.write(dir.path(), "a.txt", "two two", Some(before)).unwrap();

        assert_eq!(fs::read_to_string(dir.path().join("a.txt")).unwrap(), "two two");
        assert_eq!(after.len, 7);
        assert_eq!(after, files.stat(dir.path(), "a.txt").unwrap());
    }

    #[test]
    fn write_refuses_when_the_file_changed_since_it_was_read() {
        let dir = tempfile::tempdir().unwrap();
        let before = written(&dir, "a.txt", "one");
        fs::write(dir.path().join("a.txt"), "changed by someone else").unwrap();

        let result = LocalFiles::new().write(dir.path(), "a.txt", "mine", Some(before));

        assert_eq!(result, Err(WriteError::Changed));
        assert_eq!(fs::read_to_string(dir.path().join("a.txt")).unwrap(), "changed by someone else", "the other change is untouched");
    }

    #[test]
    fn write_with_no_expected_version_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        written(&dir, "a.txt", "one");
        fs::write(dir.path().join("a.txt"), "changed by someone else").unwrap();

        LocalFiles::new().write(dir.path(), "a.txt", "mine", None).unwrap();

        assert_eq!(fs::read_to_string(dir.path().join("a.txt")).unwrap(), "mine");
    }

    #[test]
    fn write_leaves_no_temporary_file_and_keeps_the_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        written(&dir, "run.sh", "echo 1\n");
        fs::set_permissions(dir.path().join("run.sh"), fs::Permissions::from_mode(0o755)).unwrap();
        let before = LocalFiles::new().stat(dir.path(), "run.sh").unwrap();

        LocalFiles::new().write(dir.path(), "run.sh", "echo 2\n", Some(before)).unwrap();

        let mode = fs::metadata(dir.path().join("run.sh")).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o755);
        let names: Vec<String> = fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(names, vec!["run.sh".to_string()], "the temporary file is gone");
    }

    #[test]
    fn write_refuses_paths_that_leave_the_folder_and_says_why_a_missing_file_fails() {
        let dir = tree();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("secret.txt"), "secret").unwrap();
        symlink(outside.path().join("secret.txt"), dir.path().join("link.txt")).unwrap();
        let files = LocalFiles::new();
        assert_eq!(files.write(dir.path(), "../secret.txt", "x", None), Err(WriteError::Outside));
        assert_eq!(files.write(dir.path(), "link.txt", "x", None), Err(WriteError::Outside));
        assert_eq!(fs::read_to_string(outside.path().join("secret.txt")).unwrap(), "secret");
        assert!(matches!(files.write(dir.path(), "nope.txt", "x", None), Err(WriteError::Failed(_))));
    }

    #[test]
    fn write_into_a_folder_that_cannot_be_written_is_a_message_and_keeps_the_old_text() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let before = written(&dir, "a.txt", "one");
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o555)).unwrap();

        let result = LocalFiles::new().write(dir.path(), "a.txt", "two", Some(before));

        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o755)).unwrap();
        assert!(matches!(result, Err(WriteError::Failed(_))), "{result:?}");
        assert_eq!(fs::read_to_string(dir.path().join("a.txt")).unwrap(), "one");
    }

    #[test]
    fn a_file_over_a_mebibyte_is_not_searched_even_though_it_can_be_read() {
        let dir = tempfile::tempdir().unwrap();
        let mut text = "needle\n".to_string();
        text.push_str(&"x".repeat(2 * 1024 * 1024));
        fs::write(dir.path().join("big.txt"), text).unwrap();
        fs::write(dir.path().join("small.txt"), "needle\n").unwrap();

        let hits = LocalFiles::new().search(dir.path(), "needle", 10).unwrap();

        assert_eq!(hits.iter().map(|hit| hit.path.as_str()).collect::<Vec<_>>(), vec!["small.txt"]);
    }
```

- [ ] **Step 3: Run to verify they fail**

Run: `cargo test -p forge-adapter-files 2>&1 | tail -15`
Expected: compile errors (the trait has methods the adapter lacks).

- [ ] **Step 4: Implement the adapter**

In `crates/forge-adapter-files/src/lib.rs`:

1. Replace the import line and constants at the top with:

```rust
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;

use forge_ports::{ContentHit, DirEntry, FileContent, FileVersion, FilesError, FilesPort, WriteError, READ_LIMIT};

const SKIPPED: [&str; 2] = [".git", "node_modules"];
// A NUL byte in this many first bytes makes a file binary.
const BINARY_SNIFF: usize = 8000;
// A search looks at no more files than this, so a huge tree cannot keep it busy for long.
const SEARCH_FILE_LIMIT: usize = 20_000;
// A search reads no file larger than this, so the larger read limit does not slow it.
const SEARCH_FILE_BYTES: u64 = 1024 * 1024;
const HIT_TEXT_CHARS: usize = 200;
const IMAGE_TYPES: [(&str, &str); 8] = [
    ("png", "image/png"),
    ("jpg", "image/jpeg"),
    ("jpeg", "image/jpeg"),
    ("gif", "image/gif"),
    ("webp", "image/webp"),
    ("bmp", "image/bmp"),
    ("ico", "image/x-icon"),
    ("svg", "image/svg+xml"),
];
```

2. After `fn looks_binary`, add:

```rust
fn image_mime(path: &Path) -> Option<&'static str> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    IMAGE_TYPES.iter().find(|(known, _)| *known == extension).map(|(_, mime)| *mime)
}

fn version_of(meta: &fs::Metadata) -> FileVersion {
    let modified_ns = meta.modified().ok().and_then(|time| time.duration_since(UNIX_EPOCH).ok()).map_or(0, |since| since.as_nanos());
    FileVersion { modified_ns, len: meta.len() }
}

fn text_content(bytes: Vec<u8>, version: FileVersion) -> FileContent {
    match String::from_utf8(bytes) {
        Ok(text) => FileContent::Text { text, version, lossy: false },
        Err(error) => FileContent::Text { text: String::from_utf8_lossy(error.as_bytes()).into_owned(), version, lossy: true },
    }
}
```

3. Replace the body of `fn read` and add the three new methods in `impl FilesPort for LocalFiles`:

```rust
    fn read(&self, root: &Path, path: &str) -> Result<FileContent, FilesError> {
        let file = resolve(root, path)?;
        let meta = fs::metadata(&file).map_err(failed)?;
        if meta.len() > READ_LIMIT as u64 {
            return Ok(FileContent::TooLarge);
        }
        let bytes = fs::read(&file).map_err(failed)?;
        if let Some(mime) = image_mime(&file) {
            return Ok(FileContent::Image { bytes, mime: mime.to_string() });
        }
        if looks_binary(&bytes) {
            return Ok(FileContent::Binary);
        }
        Ok(text_content(bytes, version_of(&meta)))
    }

    fn read_text_anyway(&self, root: &Path, path: &str) -> Result<FileContent, FilesError> {
        let file = resolve(root, path)?;
        let meta = fs::metadata(&file).map_err(failed)?;
        if meta.len() > READ_LIMIT as u64 {
            return Ok(FileContent::TooLarge);
        }
        let bytes = fs::read(&file).map_err(failed)?;
        Ok(FileContent::Text { text: String::from_utf8_lossy(&bytes).into_owned(), version: version_of(&meta), lossy: true })
    }

    fn stat(&self, root: &Path, path: &str) -> Result<FileVersion, FilesError> {
        let file = resolve(root, path)?;
        Ok(version_of(&fs::metadata(&file).map_err(failed)?))
    }

    fn write(&self, root: &Path, path: &str, text: &str, expected: Option<FileVersion>) -> Result<FileVersion, WriteError> {
        let file = resolve(root, path).map_err(|error| match error {
            FilesError::Outside => WriteError::Outside,
            FilesError::Failed(message) => WriteError::Failed(message),
        })?;
        let failed_write = |error: std::io::Error| WriteError::Failed(error.to_string());
        let meta = fs::metadata(&file).map_err(failed_write)?;
        if expected.is_some_and(|version| version != version_of(&meta)) {
            return Err(WriteError::Changed);
        }
        let name = file.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
        let temp = file.with_file_name(format!(".{name}.forge-tmp"));
        let result = fs::write(&temp, text).and_then(|()| fs::set_permissions(&temp, meta.permissions())).and_then(|()| fs::rename(&temp, &file));
        if let Err(error) = result {
            let _ = fs::remove_file(&temp);
            return Err(failed_write(error));
        }
        Ok(version_of(&fs::metadata(&file).map_err(failed_write)?))
    }
```

4. In `search_dir`, replace `meta.len() > READ_LIMIT as u64` with `meta.len() > SEARCH_FILE_BYTES`.

- [ ] **Step 5: Keep the panel compiling**

In `crates/forge-widgets/src/files_panel.rs` make exactly these three edits (the panel is rewritten in Task 10):

- `(Some(path), Some(Ok(FileContent::Text(text)))) => {` becomes `(Some(path), Some(Ok(FileContent::Text { text, .. }))) => {`
- `(Some(_), Some(Ok(FileContent::Binary | FileContent::TooLarge))) =>` becomes `(Some(_), Some(Ok(FileContent::Binary | FileContent::TooLarge | FileContent::Image { .. }))) =>`
- `(Some(_), Some(Ok(FileContent::Text(_)))) => match shown() {` becomes `(Some(_), Some(Ok(FileContent::Text { .. }))) => match shown() {`

- [ ] **Step 6: Run the tests**

Run: `cargo test -p forge-ports -p forge-adapter-files 2>&1 | tail -12` then `cargo build --workspace 2>&1 | tail -5`
Expected: all adapter and port tests pass; the workspace builds.

- [ ] **Step 7: Commit**

```bash
git add -A crates
git commit -m "feat: FilesPort versions, images, text anyway, stat and a checked atomic write

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Reveal a file in Finder

**Files:**
- Modify: `crates/forge-ports/src/browser_port.rs`, `crates/forge-adapter-browser/src/lib.rs`, `crates/forge-features/src/workspace.rs`
- Test: the port and adapter `tests` modules

**Interfaces:**
- Produces: `BrowserPort::reveal(&self, path: &Path) -> Result<(), String>`; `Workspace::reveal(&self, path: &Path)` (shows a notice on failure).

- [ ] **Step 1: Write the failing tests**

In `crates/forge-ports/src/browser_port.rs` add to `tests`:

```rust
    #[test]
    fn the_mock_can_refuse_to_reveal() {
        let mut mock = MockBrowserPort::new();
        mock.expect_reveal().returning(|_| Err("no finder".to_string()));
        assert_eq!(mock.reveal(std::path::Path::new("/tmp/a.txt")), Err("no finder".to_string()));
    }
```

In `crates/forge-adapter-browser/src/lib.rs` add to `tests`:

```rust
    #[test]
    fn revealing_with_a_missing_program_says_it_could_not_run() {
        let browser = SystemBrowser::with_program("/nonexistent/forge-open");
        let error = browser.reveal(std::path::Path::new("/tmp")).unwrap_err();
        assert!(error.contains("Could not run"), "{error}");
    }

    #[test]
    fn revealing_a_relative_path_is_refused_before_any_program_runs() {
        let browser = SystemBrowser::with_program("/nonexistent/forge-open");
        let error = browser.reveal(std::path::Path::new("relative/a.txt")).unwrap_err();
        assert!(error.contains("absolute"), "{error}");
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p forge-ports -p forge-adapter-browser reveal 2>&1 | tail -10`
Expected: compile errors, no `reveal`.

- [ ] **Step 3: Implement**

In `browser_port.rs`, add `use std::path::Path;` at the top and inside the trait after `open`:

```rust
    /// Shows a file or folder in Finder (selected in its folder). The path must be absolute.
    fn reveal(&self, path: &Path) -> Result<(), String>;
```

In `crates/forge-adapter-browser/src/lib.rs` add `use std::path::Path;` and inside `impl BrowserPort for SystemBrowser` after `open`:

```rust
    fn reveal(&self, path: &Path) -> Result<(), String> {
        if !path.is_absolute() {
            return Err("Only an absolute path can be shown in Finder".to_string());
        }
        // `-R` reveals instead of opening; the path is one argument, never part of a command line.
        let status = Command::new(&self.program)
            .arg("-R")
            .arg(path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|error| format!("Could not run {}: {error}", self.program))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("{} could not show the file in Finder", self.program))
        }
    }
```

In `crates/forge-features/src/workspace.rs`, after `pub fn browser(&self)` add:

```rust
    /// Shows a file in Finder; a failure becomes a notice.
    pub fn reveal(&self, path: &Path) {
        if let Err(message) = self.browser().reveal(path) {
            self.report(message);
        }
    }
```

- [ ] **Step 4: Run and commit**

Run: `cargo test -p forge-ports -p forge-adapter-browser 2>&1 | tail -8` and `cargo build --workspace 2>&1 | tail -3`
Expected: pass; builds.

```bash
git add -A crates
git commit -m "feat: show a file in Finder through the browser port

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 4: `EditorTabs`, the tab model

**Files:**
- Create: `crates/forge-features/src/editor_tabs.rs`
- Modify: `crates/forge-features/src/lib.rs`
- Test: in the new file

**Interfaces:**
- Produces:
  - `pub struct EditorTab { pub path: String, pub preview: bool }`
  - `pub struct EditorTabs` (`Default`, `Clone`, `PartialEq`) with: `open(&mut self, path: &str, preview_tabs: bool) -> Option<String>` (the path of the preview tab it replaced, if any), `keep(&mut self, path: &str)`, `select(&mut self, path: &str)`, `close(&mut self, path: &str)`, `close_others(&mut self, keep: &str)`, `close_all(&mut self)`, `reorder(&mut self, from: &str, to: &str)`, `contains(&self, path: &str) -> bool`, `tabs(&self) -> &[EditorTab]`, `paths(&self) -> Vec<String>`, `selected(&self) -> Option<&str>`.
- Tabs are keyed by the full relative path, so `a/x.py` and `b/x.py` are two tabs.

- [ ] **Step 1: Write the failing tests**

Create `crates/forge-features/src/editor_tabs.rs` with only the tests module first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn paths(tabs: &EditorTabs) -> Vec<&str> {
        tabs.tabs().iter().map(|tab| tab.path.as_str()).collect()
    }

    #[test]
    fn a_new_file_opens_in_a_preview_tab_and_is_selected() {
        let mut tabs = EditorTabs::default();
        assert_eq!(tabs.open("a.py", true), None);
        assert_eq!(paths(&tabs), vec!["a.py"]);
        assert!(tabs.tabs()[0].preview);
        assert_eq!(tabs.selected(), Some("a.py"));
    }

    #[test]
    fn the_next_file_replaces_the_preview_tab_in_place_and_says_which_it_replaced() {
        let mut tabs = EditorTabs::default();
        tabs.open("kept.py", true);
        tabs.keep("kept.py");
        tabs.open("a.py", true);
        tabs.open("tail.py", false);

        assert_eq!(tabs.open("b.py", true), Some("a.py".to_string()));

        assert_eq!(paths(&tabs), vec!["kept.py", "b.py", "tail.py"]);
        assert_eq!(tabs.selected(), Some("b.py"));
    }

    #[test]
    fn opening_an_open_file_selects_it_and_changes_nothing_else() {
        let mut tabs = EditorTabs::default();
        tabs.open("a.py", true);
        tabs.keep("a.py");
        tabs.open("b.py", true);

        assert_eq!(tabs.open("a.py", true), None);

        assert_eq!(paths(&tabs), vec!["a.py", "b.py"]);
        assert_eq!(tabs.selected(), Some("a.py"));
        assert!(tabs.tabs()[1].preview, "b is still the preview tab");
    }

    #[test]
    fn with_preview_tabs_off_every_file_gets_a_normal_tab() {
        let mut tabs = EditorTabs::default();
        tabs.open("a.py", false);
        tabs.open("b.py", false);
        assert_eq!(paths(&tabs), vec!["a.py", "b.py"]);
        assert!(tabs.tabs().iter().all(|tab| !tab.preview));
    }

    #[test]
    fn keeping_a_preview_tab_makes_it_normal_so_the_next_file_gets_its_own_tab() {
        let mut tabs = EditorTabs::default();
        tabs.open("a.py", true);
        tabs.keep("a.py");
        assert_eq!(tabs.open("b.py", true), None);
        assert_eq!(paths(&tabs), vec!["a.py", "b.py"]);
    }

    #[test]
    fn closing_the_selected_tab_selects_its_right_neighbour_else_the_left() {
        let mut tabs = EditorTabs::default();
        for name in ["a", "b", "c"] {
            tabs.open(name, false);
        }
        tabs.select("b");
        tabs.close("b");
        assert_eq!(tabs.selected(), Some("c"));
        tabs.close("c");
        assert_eq!(tabs.selected(), Some("a"));
        tabs.close("a");
        assert_eq!(tabs.selected(), None);
        assert!(tabs.tabs().is_empty());
    }

    #[test]
    fn closing_another_tab_keeps_the_selection() {
        let mut tabs = EditorTabs::default();
        for name in ["a", "b", "c"] {
            tabs.open(name, false);
        }
        tabs.select("a");
        tabs.close("c");
        assert_eq!(tabs.selected(), Some("a"));
        tabs.close("missing");
        assert_eq!(paths(&tabs), vec!["a", "b"]);
    }

    #[test]
    fn close_others_keeps_one_tab_and_selects_it() {
        let mut tabs = EditorTabs::default();
        for name in ["a", "b", "c"] {
            tabs.open(name, false);
        }
        tabs.close_others("b");
        assert_eq!(paths(&tabs), vec!["b"]);
        assert_eq!(tabs.selected(), Some("b"));
    }

    #[test]
    fn close_all_leaves_nothing() {
        let mut tabs = EditorTabs::default();
        tabs.open("a", false);
        tabs.close_all();
        assert!(tabs.tabs().is_empty());
        assert_eq!(tabs.selected(), None);
    }

    #[test]
    fn dragging_a_tab_onto_another_puts_it_at_that_place_and_keeps_the_selection() {
        let mut tabs = EditorTabs::default();
        for name in ["a", "b", "c", "d"] {
            tabs.open(name, false);
        }
        tabs.select("c");
        tabs.reorder("a", "d");
        assert_eq!(paths(&tabs), vec!["b", "c", "d", "a"]);
        assert_eq!(tabs.selected(), Some("c"));
        tabs.reorder("a", "b");
        assert_eq!(paths(&tabs), vec!["a", "b", "c", "d"]);
        tabs.reorder("a", "a");
        tabs.reorder("a", "missing");
        assert_eq!(paths(&tabs), vec!["a", "b", "c", "d"]);
    }

    #[test]
    fn two_files_with_the_same_name_in_different_folders_are_two_tabs() {
        let mut tabs = EditorTabs::default();
        tabs.open("a/x.py", false);
        tabs.open("b/x.py", false);
        assert_eq!(paths(&tabs), vec!["a/x.py", "b/x.py"]);
        tabs.close("a/x.py");
        assert_eq!(paths(&tabs), vec!["b/x.py"]);
    }

    #[test]
    fn selecting_a_path_that_is_not_open_does_nothing() {
        let mut tabs = EditorTabs::default();
        tabs.open("a", false);
        tabs.select("nope");
        assert_eq!(tabs.selected(), Some("a"));
    }
}
```

- [ ] **Step 2: Wire the module and run to verify the failure**

In `crates/forge-features/src/lib.rs` add `mod editor_tabs;` (next to `mod effort;`) and `pub use editor_tabs::{EditorTab, EditorTabs};` (next to the other `pub use` lines).

Run: `cargo test -p forge-features editor_tabs 2>&1 | tail -10`
Expected: compile error, `EditorTabs` not found.

- [ ] **Step 3: Implement**

Put this above the `tests` module in `crates/forge-features/src/editor_tabs.rs`:

```rust
/// One tab of the Files panel. Tabs are keyed by the file's path inside the session folder, so two
/// files with the same name in different folders are two tabs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorTab {
    pub path: String,
    /// An italic tab that the next file replaces, until it is kept or edited.
    pub preview: bool,
}

/// The tabs of one Files panel and which one is selected.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EditorTabs {
    tabs: Vec<EditorTab>,
    selected: Option<String>,
}

impl EditorTabs {
    /// Opens a file: an open one is selected; a new one takes the place of the preview tab when
    /// `preview_tabs` is on (and there is one), else goes at the right. Returns the path of the
    /// preview tab it replaced, so the caller can forget that file's state.
    pub fn open(&mut self, path: &str, preview_tabs: bool) -> Option<String> {
        if self.contains(path) {
            self.selected = Some(path.to_string());
            return None;
        }
        let tab = EditorTab { path: path.to_string(), preview: preview_tabs };
        let mut replaced = None;
        match self.tabs.iter().position(|existing| existing.preview).filter(|_| preview_tabs) {
            Some(index) => replaced = Some(std::mem::replace(&mut self.tabs[index], tab).path),
            None => self.tabs.push(tab),
        }
        self.selected = Some(path.to_string());
        replaced
    }

    /// A preview tab becomes a normal one (`Keep open`, or its first edit).
    pub fn keep(&mut self, path: &str) {
        if let Some(tab) = self.tabs.iter_mut().find(|tab| tab.path == path) {
            tab.preview = false;
        }
    }

    pub fn select(&mut self, path: &str) {
        if self.contains(path) {
            self.selected = Some(path.to_string());
        }
    }

    /// Closing the selected tab selects its right neighbour, else the left one.
    pub fn close(&mut self, path: &str) {
        let Some(position) = self.tabs.iter().position(|tab| tab.path == path) else {
            return;
        };
        self.tabs.remove(position);
        if self.selected.as_deref() == Some(path) {
            self.selected = self.tabs.get(position).or_else(|| self.tabs.last()).map(|tab| tab.path.clone());
        }
    }

    pub fn close_others(&mut self, keep: &str) {
        self.tabs.retain(|tab| tab.path == keep);
        self.selected = self.tabs.first().map(|tab| tab.path.clone());
    }

    pub fn close_all(&mut self) {
        self.tabs.clear();
        self.selected = None;
    }

    /// Moves the `from` tab to the place of the `to` tab. The selection does not change.
    pub fn reorder(&mut self, from: &str, to: &str) {
        let position = |path: &str| self.tabs.iter().position(|tab| tab.path == path);
        let (Some(a), Some(b)) = (position(from), position(to)) else {
            return;
        };
        if a != b {
            let tab = self.tabs.remove(a);
            self.tabs.insert(b, tab);
        }
    }

    pub fn contains(&self, path: &str) -> bool {
        self.tabs.iter().any(|tab| tab.path == path)
    }

    pub fn tabs(&self) -> &[EditorTab] {
        &self.tabs
    }

    pub fn paths(&self) -> Vec<String> {
        self.tabs.iter().map(|tab| tab.path.clone()).collect()
    }

    pub fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }
}
```

- [ ] **Step 4: Run and commit**

Run: `cargo test -p forge-features editor_tabs 2>&1 | tail -8`
Expected: 12 tests pass.

```bash
git add -A crates
git commit -m "feat: EditorTabs, the model of the Files panel tabs

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 5: `TabDoc`, the save and conflict rules

**Files:**
- Create: `crates/forge-features/src/editor_doc.rs`
- Modify: `crates/forge-features/src/lib.rs`
- Test: in the new file

**Interfaces:**
- Consumes: `forge_ports::{FileVersion, WriteError}` (Task 2).
- Produces:
  - `pub fn text_hash(text: &str) -> u64`
  - `pub enum Banner { Conflict, SaveFailed(String) }` with `message(&self) -> String`
  - `pub struct TabDoc { pub loaded: FileVersion, pub clean: u64, pub banner: Option<Banner> }` with `new(loaded, text: &str)`, `on_dirty(&mut self, disk: Result<FileVersion, String>)`, `may_write(&self, auto_save: bool) -> bool`, `differs(&self, text: &str) -> bool`, `written(&mut self, version, text: &str)`, `write_failed(&mut self, error: &WriteError)`, `reloaded(&mut self, version, text: &str)`, `clear_banner(&mut self)`.
- Why a hash and not a "dirty" flag: the page reports the text it really holds; comparing with the last text that was read or written cannot go wrong when a keystroke lands while a write is in flight.

- [ ] **Step 1: Write the failing tests**

Create `crates/forge-features/src/editor_doc.rs` with the tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn version(ns: u128, len: u64) -> FileVersion {
        FileVersion { modified_ns: ns, len }
    }

    #[test]
    fn a_freshly_read_text_is_clean_and_a_different_text_differs() {
        let doc = TabDoc::new(version(1, 3), "abc");
        assert!(!doc.differs("abc"));
        assert!(doc.differs("abcd"));
        assert!(doc.differs(""));
    }

    #[test]
    fn the_first_edit_on_an_unchanged_disk_raises_no_banner() {
        let mut doc = TabDoc::new(version(1, 3), "abc");
        doc.on_dirty(Ok(version(1, 3)));
        assert_eq!(doc.banner, None);
        assert!(doc.may_write(true));
    }

    #[test]
    fn the_first_edit_after_the_disk_changed_raises_the_conflict_banner_and_pauses_auto_save() {
        let mut doc = TabDoc::new(version(1, 3), "abc");
        doc.on_dirty(Ok(version(2, 9)));
        assert_eq!(doc.banner, Some(Banner::Conflict));
        assert!(!doc.may_write(true));
    }

    #[test]
    fn a_file_that_cannot_be_looked_at_raises_a_save_failed_banner_with_the_reason() {
        let mut doc = TabDoc::new(version(1, 3), "abc");
        doc.on_dirty(Err("No such file or directory".to_string()));
        assert_eq!(doc.banner, Some(Banner::SaveFailed("No such file or directory".to_string())));
        assert!(!doc.may_write(true));
    }

    #[test]
    fn a_banner_already_shown_is_not_replaced_by_a_later_check() {
        let mut doc = TabDoc::new(version(1, 3), "abc");
        doc.on_dirty(Ok(version(2, 9)));
        doc.on_dirty(Ok(version(1, 3)));
        assert_eq!(doc.banner, Some(Banner::Conflict));
    }

    #[test]
    fn with_auto_save_off_nothing_may_be_written_by_the_timer() {
        let doc = TabDoc::new(version(1, 3), "abc");
        assert!(!doc.may_write(false));
    }

    #[test]
    fn a_successful_write_makes_that_text_the_clean_one_and_keeps_later_typing_dirty() {
        let mut doc = TabDoc::new(version(1, 3), "abc");
        doc.written(version(5, 4), "abcd");
        assert_eq!(doc.loaded, version(5, 4));
        assert!(!doc.differs("abcd"));
        // The person typed one more letter while the write was in flight.
        assert!(doc.differs("abcde"), "text typed during a write must still be saved by the next change");
    }

    #[test]
    fn a_write_that_found_the_file_changed_raises_the_conflict_banner() {
        let mut doc = TabDoc::new(version(1, 3), "abc");
        doc.write_failed(&WriteError::Changed);
        assert_eq!(doc.banner, Some(Banner::Conflict));
    }

    #[test]
    fn a_write_that_failed_keeps_the_reason_and_the_clean_text() {
        let mut doc = TabDoc::new(version(1, 3), "abc");
        doc.write_failed(&WriteError::Failed("Permission denied".to_string()));
        assert_eq!(doc.banner, Some(Banner::SaveFailed("Permission denied".to_string())));
        assert!(doc.differs("abcd"), "the clean text is still the one on disk");
        doc.write_failed(&WriteError::Outside);
        assert!(matches!(doc.banner, Some(Banner::SaveFailed(_))));
    }

    #[test]
    fn reloading_from_disk_clears_the_banner_and_takes_the_new_text() {
        let mut doc = TabDoc::new(version(1, 3), "abc");
        doc.on_dirty(Ok(version(2, 9)));
        doc.reloaded(version(2, 9), "disk text");
        assert_eq!(doc.banner, None);
        assert_eq!(doc.loaded, version(2, 9));
        assert!(!doc.differs("disk text"));
    }

    #[test]
    fn clearing_the_banner_lets_auto_save_go_on() {
        let mut doc = TabDoc::new(version(1, 3), "abc");
        doc.write_failed(&WriteError::Failed("x".to_string()));
        doc.clear_banner();
        assert!(doc.may_write(true));
    }

    #[test]
    fn the_banner_messages_are_plain_english() {
        assert_eq!(Banner::Conflict.message(), "This file has changed on disk since you started editing.");
        assert_eq!(Banner::SaveFailed("Permission denied".to_string()).message(), "Could not save: Permission denied");
    }
}
```

- [ ] **Step 2: Wire the module and run to verify the failure**

In `crates/forge-features/src/lib.rs` add `mod editor_doc;` and `pub use editor_doc::{text_hash, Banner, TabDoc};`.

Run: `cargo test -p forge-features editor_doc 2>&1 | tail -8`
Expected: compile error, `TabDoc` not found.

- [ ] **Step 3: Implement**

Put this above the `tests` module:

```rust
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use forge_ports::{FileVersion, WriteError};

/// A hash of a text, to tell whether the editor holds what is on disk without keeping a second copy.
pub fn text_hash(text: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

/// What the banner over the editor says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Banner {
    /// The file changed on disk since the editor read it.
    Conflict,
    SaveFailed(String),
}

impl Banner {
    pub fn message(&self) -> String {
        match self {
            Banner::Conflict => "This file has changed on disk since you started editing.".to_string(),
            Banner::SaveFailed(reason) => format!("Could not save: {reason}"),
        }
    }
}

/// What Rust knows about one open text tab: the file version it last read or wrote, a hash of the
/// text that is on disk, and the banner. The text itself lives in the editor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabDoc {
    pub loaded: FileVersion,
    pub clean: u64,
    pub banner: Option<Banner>,
}

impl TabDoc {
    pub fn new(loaded: FileVersion, text: &str) -> Self {
        Self { loaded, clean: text_hash(text), banner: None }
    }

    /// The editor reports its first change since a read or a write. `disk` is the file's version
    /// now: a different one is a conflict; a file that cannot be looked at is a failed save.
    pub fn on_dirty(&mut self, disk: Result<FileVersion, String>) {
        if self.banner.is_some() {
            return;
        }
        self.banner = match disk {
            Ok(version) if version == self.loaded => None,
            Ok(_) => Some(Banner::Conflict),
            Err(reason) => Some(Banner::SaveFailed(reason)),
        };
    }

    /// Whether the timer's save may write: auto save is on and no banner waits for an answer.
    pub fn may_write(&self, auto_save: bool) -> bool {
        auto_save && self.banner.is_none()
    }

    /// Whether `text` (what the editor holds now) is not what is on disk.
    pub fn differs(&self, text: &str) -> bool {
        text_hash(text) != self.clean
    }

    /// A write of `text` succeeded.
    pub fn written(&mut self, version: FileVersion, text: &str) {
        self.loaded = version;
        self.clean = text_hash(text);
        self.banner = None;
    }

    pub fn write_failed(&mut self, error: &WriteError) {
        self.banner = Some(match error {
            WriteError::Changed => Banner::Conflict,
            WriteError::Outside => Banner::SaveFailed(WriteError::Outside.to_string()),
            WriteError::Failed(reason) => Banner::SaveFailed(reason.clone()),
        });
    }

    /// The file was read again (Cancel on the conflict banner, or a quiet refresh).
    pub fn reloaded(&mut self, version: FileVersion, text: &str) {
        self.loaded = version;
        self.clean = text_hash(text);
        self.banner = None;
    }

    pub fn clear_banner(&mut self) {
        self.banner = None;
    }
}
```

- [ ] **Step 4: Run and commit**

Run: `cargo test -p forge-features editor_doc 2>&1 | tail -8`
Expected: 12 tests pass.

```bash
git add -A crates
git commit -m "feat: TabDoc, the save and conflict rules of an open file

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 6: The editor bundle and its head

**Files:**
- Create: `tools/editor-bundle/package.json`, `tools/editor-bundle/build.mjs`, `tools/editor-bundle/README.md`, `tools/editor-bundle/src/main.js`
- Create (built, committed): `crates/forge-shared/assets/editor.js`, `crates/forge-shared/assets/EDITOR-LICENSE`
- Create: `crates/forge-shared/src/editor.rs`
- Modify: `crates/forge-shared/src/lib.rs`, `crates/forge-app/src/main.rs`
- Test: `crates/forge-shared/src/editor.rs` (its `tests` module)

**Interfaces:**
- Produces:
  - `forge_shared::editor_head() -> String` (one `<script>`).
  - the page global `window.ForgeEditor.create(host: HTMLElement, send: (message) => void)` returning `{ load(key, doc, language, readonly), show(key), drop(key), settings(s), flush(key), focus(), destroy() }`.
  - page to Rust messages: `{t:"dirty",key}`, `{t:"change",key,s}`, `{t:"save",key,s}` (⌘S), `{t:"flush",key,s}` (`s` is `null` when the page has no state for `key`), `{t:"missing",key}`.
  - settings shape from Rust: `{ wrap, indent_guides, whitespace, scroll_past_end, tab_indent: bool, tab_size: 2|4|8 }`.

- [ ] **Step 1: Write the failing Rust contract tests**

Create `crates/forge-shared/src/editor.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_head_is_one_inline_script() {
        let head = editor_head();
        assert!(head.starts_with("<script>"));
        assert!(head.ends_with("</script>"));
        // A `</script>` inside the vendored code would end the script early.
        assert_eq!(head.matches("<script>").count(), 1);
        assert_eq!(head.matches("</script>").count(), 1);
    }

    #[test]
    fn the_script_defines_the_global_the_bridge_uses() {
        let head = editor_head();
        assert!(head.contains("ForgeEditor"));
    }

    #[test]
    fn the_licence_file_names_the_authors() {
        assert!(include_str!("../assets/EDITOR-LICENSE").contains("Marijn Haverbeke"));
    }
}
```

In `crates/forge-shared/src/lib.rs` add `mod editor;` and `pub use editor::editor_head;`.

- [ ] **Step 2: Run to verify the failure**

Run: `cargo test -p forge-shared editor 2>&1 | tail -8`
Expected: compile error (no `editor_head`, no asset files).

- [ ] **Step 3: Create the bundle project**

Create `tools/editor-bundle/package.json`:

```json
{
  "name": "forge-editor-bundle",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "description": "Builds crates/forge-shared/assets/editor.js (CodeMirror 6 for the Files panel). Run `npm run build` here after changing src/main.js or a dependency, and commit the output.",
  "scripts": {
    "build": "node build.mjs"
  },
  "dependencies": {
    "@codemirror/commands": "6.11.1",
    "@codemirror/lang-css": "6.3.1",
    "@codemirror/lang-html": "6.4.12",
    "@codemirror/lang-javascript": "6.2.5",
    "@codemirror/lang-json": "6.0.2",
    "@codemirror/lang-markdown": "6.5.2",
    "@codemirror/lang-python": "6.2.1",
    "@codemirror/lang-rust": "6.0.2",
    "@codemirror/lang-yaml": "6.1.3",
    "@codemirror/language": "6.12.4",
    "@codemirror/legacy-modes": "6.5.4",
    "@codemirror/state": "6.7.6",
    "@codemirror/view": "6.43.13",
    "@lezer/highlight": "1.2.5"
  },
  "devDependencies": {
    "esbuild": "0.28.2"
  }
}
```

Create `tools/editor-bundle/build.mjs`:

```js
// Bundles src/main.js into the one file Forge puts in the window head.
import { build } from "esbuild";
import { writeFileSync } from "node:fs";

const out = new URL("../../crates/forge-shared/assets/editor.js", import.meta.url);
const result = await build({
  entryPoints: ["src/main.js"],
  bundle: true,
  minify: true,
  format: "iife",
  target: "safari15",
  legalComments: "none",
  write: false,
});
// The code goes inline between <script> tags: a literal `</script` inside it would end the script.
const text = result.outputFiles[0].text.replaceAll("</script", "<\\/script");
writeFileSync(out, text);
console.log(`wrote ${out.pathname} (${(text.length / 1024).toFixed(0)} KiB)`);
```

Create `tools/editor-bundle/README.md`:

```markdown
# Editor bundle

CodeMirror 6 for the Files panel, bundled into `crates/forge-shared/assets/editor.js`.
Forge puts that file inline in the window head (`forge_shared::editor_head`), like xterm.js, so
building Forge needs no Node. Only rebuild when `src/main.js` or a dependency changes:

    cd tools/editor-bundle
    npm install
    npm run build

Commit `package-lock.json` and the rebuilt `editor.js`. The licences of CodeMirror and Lezer
(MIT) are in `crates/forge-shared/assets/EDITOR-LICENSE`.

The page API (`window.ForgeEditor`) is described at the top of `src/main.js`; the Rust side that
drives it is `crates/forge-widgets/src/files_editor.rs`.
```

- [ ] **Step 4: Write `src/main.js`**

Create `tools/editor-bundle/src/main.js`:

```js
// window.ForgeEditor.create(host, send) builds one CodeMirror editor in `host`.
//
// One editor serves every tab: each tab's EditorState (text, undo history) and scroll position are
// kept per key, so switching tabs keeps what the person did.
//
// Rust to page (methods): load(key, doc, language, readonly), show(key), drop(key),
//   settings({ wrap, indent_guides, whitespace, scroll_past_end, tab_indent, tab_size }),
//   flush(key), focus(), destroy().
// Page to Rust (send): { t: "dirty", key }, { t: "change", key, s }, { t: "save", key, s },
//   { t: "flush", key, s }, { t: "missing", key }.
//
// `change` is sent 2 s after the last edit, once per pause, never during an IME composition.
import { Compartment, EditorState } from "@codemirror/state";
import {
  Decoration,
  EditorView,
  ViewPlugin,
  drawSelection,
  highlightActiveLine,
  highlightActiveLineGutter,
  highlightWhitespace,
  keymap,
  lineNumbers,
  scrollPastEnd,
} from "@codemirror/view";
import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import {
  HighlightStyle,
  StreamLanguage,
  bracketMatching,
  foldGutter,
  foldKeymap,
  indentOnInput,
  indentUnit,
  syntaxHighlighting,
} from "@codemirror/language";
import { tags as t } from "@lezer/highlight";
import { css } from "@codemirror/lang-css";
import { html } from "@codemirror/lang-html";
import { javascript } from "@codemirror/lang-javascript";
import { json } from "@codemirror/lang-json";
import { markdown } from "@codemirror/lang-markdown";
import { python } from "@codemirror/lang-python";
import { rust } from "@codemirror/lang-rust";
import { yaml } from "@codemirror/lang-yaml";
import { shell } from "@codemirror/legacy-modes/mode/shell";

const SAVE_DELAY_MS = 2000;
const COMPOSING_RETRY_MS = 300;

const LANGUAGES = {
  javascript: () => javascript(),
  jsx: () => javascript({ jsx: true }),
  typescript: () => javascript({ typescript: true }),
  tsx: () => javascript({ jsx: true, typescript: true }),
  python: () => python(),
  rust: () => rust(),
  json: () => json(),
  markdown: () => markdown(),
  html: () => html(),
  css: () => css(),
  yaml: () => yaml(),
  shell: () => StreamLanguage.define(shell),
};

// Colours come from Forge's CSS variables, so the editor follows the light and dark theme.
const highlight = HighlightStyle.define([
  { tag: [t.comment, t.lineComment, t.blockComment], color: "var(--hl-comment)", fontStyle: "italic" },
  { tag: [t.keyword, t.controlKeyword, t.definitionKeyword, t.moduleKeyword, t.operatorKeyword], color: "var(--hl-keyword)" },
  { tag: [t.string, t.special(t.string), t.regexp], color: "var(--hl-string)" },
  { tag: [t.number, t.bool, t.null, t.atom], color: "var(--hl-number)" },
  { tag: [t.function(t.variableName), t.function(t.propertyName), t.definition(t.function(t.variableName))], color: "var(--hl-function)" },
  { tag: [t.typeName, t.className, t.namespace], color: "var(--hl-type)" },
  { tag: t.heading, color: "var(--hl-function)", fontWeight: "bold" },
  { tag: t.link, color: "var(--hl-function)", textDecoration: "underline" },
  { tag: t.emphasis, fontStyle: "italic" },
  { tag: t.strong, fontWeight: "bold" },
]);

const theme = EditorView.theme({
  "&": { height: "100%", fontSize: "12px", backgroundColor: "var(--bg)", color: "var(--fg)" },
  "&.cm-focused": { outline: "none" },
  ".cm-scroller": { fontFamily: "var(--mono)", lineHeight: "1.5", overflow: "auto" },
  ".cm-content": { caretColor: "var(--fg)" },
  ".cm-cursor": { borderLeftColor: "var(--fg)" },
  ".cm-gutters": { backgroundColor: "var(--bg)", color: "var(--fg-muted)", border: "none" },
  ".cm-activeLine": { backgroundColor: "var(--bg-hover)" },
  ".cm-activeLineGutter": { backgroundColor: "transparent", color: "var(--fg)" },
  ".cm-selectionBackground, &.cm-focused .cm-selectionBackground": { backgroundColor: "var(--accent-soft)" },
  ".cm-highlightSpace": {
    backgroundImage: "radial-gradient(circle at 50% 55%, var(--fg-muted) 20%, transparent 5%)",
  },
  ".cm-indent-guides": {
    backgroundImage:
      "repeating-linear-gradient(to right, var(--border) 0, var(--border) 1px, transparent 1px, transparent calc(var(--forge-tab, 4) * 1ch))",
  },
});

// Marks the leading whitespace of every visible line; the theme draws a thin line every tab size.
const guideMark = Decoration.mark({ class: "cm-indent-guides" });
function guideDecorations(view) {
  const ranges = [];
  for (const { from, to } of view.visibleRanges) {
    for (let pos = from; pos <= to; ) {
      const line = view.state.doc.lineAt(pos);
      const lead = /^[ \t]+/.exec(line.text);
      if (lead) ranges.push(guideMark.range(line.from, line.from + lead[0].length));
      pos = line.to + 1;
    }
  }
  return Decoration.set(ranges, true);
}
const indentGuides = ViewPlugin.fromClass(
  class {
    constructor(view) {
      this.decorations = guideDecorations(view);
    }
    update(update) {
      if (update.docChanged || update.viewportChanged) this.decorations = guideDecorations(update.view);
    }
  },
  { decorations: (plugin) => plugin.decorations },
);

window.ForgeEditor = {
  create(host, send) {
    const config = new Compartment();
    let settings = { wrap: true, indent_guides: true, whitespace: false, scroll_past_end: false, tab_indent: true, tab_size: 4 };
    let key = null;
    let timer = null;
    let dirtySent = false;
    // key -> { state, scroll, readonly }
    const states = new Map();

    // The extensions the settings control, kept in one compartment so a change reconfigures them all.
    function configExtensions(readonly) {
      return [
        settings.wrap ? EditorView.lineWrapping : [],
        settings.indent_guides ? indentGuides : [],
        settings.whitespace ? highlightWhitespace() : [],
        settings.scroll_past_end ? scrollPastEnd() : [],
        EditorState.tabSize.of(settings.tab_size),
        indentUnit.of(" ".repeat(settings.tab_size)),
        keymap.of([...(settings.tab_indent ? [indentWithTab] : [])]),
        EditorState.readOnly.of(readonly),
        EditorView.editable.of(!readonly),
      ];
    }

    function onUpdate(update) {
      if (!update.docChanged || key === null) return;
      if (!dirtySent) {
        dirtySent = true;
        send({ t: "dirty", key });
      }
      clearTimeout(timer);
      timer = setTimeout(fire, SAVE_DELAY_MS);
    }

    function fire() {
      if (view.composing) {
        timer = setTimeout(fire, COMPOSING_RETRY_MS);
        return;
      }
      timer = null;
      dirtySent = false;
      send({ t: "change", key, s: view.state.doc.toString() });
    }

    // Sends a change that is waiting for its timer, so a switch of tab never leaves one behind.
    function flushPending() {
      if (timer !== null && key !== null) {
        clearTimeout(timer);
        timer = null;
        dirtySent = false;
        send({ t: "change", key, s: view.state.doc.toString() });
      }
    }

    function makeState(doc, language, readonly) {
      const lang = LANGUAGES[language];
      return EditorState.create({
        doc,
        extensions: [
          lineNumbers(),
          foldGutter(),
          history(),
          drawSelection(),
          highlightActiveLine(),
          highlightActiveLineGutter(),
          bracketMatching(),
          indentOnInput(),
          syntaxHighlighting(highlight),
          theme,
          lang ? lang() : [],
          keymap.of([
            { key: "Mod-s", run: () => { send({ t: "save", key, s: view.state.doc.toString() }); return true; } },
            ...defaultKeymap,
            ...historyKeymap,
            ...foldKeymap,
          ]),
          config.of(configExtensions(readonly)),
          EditorView.updateListener.of(onUpdate),
        ],
      });
    }

    const view = new EditorView({ parent: host, state: EditorState.create({ doc: "", extensions: [theme] }) });
    view.dom.style.setProperty("--forge-tab", String(settings.tab_size));

    function saveCurrent() {
      const entry = key === null ? null : states.get(key);
      if (entry) {
        entry.state = view.state;
        entry.scroll = view.scrollDOM.scrollTop;
      }
    }

    // A tab switch or a load waits for an IME composition to end, so it never cuts one in half.
    function whenIdle(work) {
      if (view.composing) setTimeout(() => whenIdle(work), 100);
      else work();
    }

    function reapply(readonly) {
      view.dispatch({ effects: config.reconfigure(configExtensions(readonly)) });
    }

    return {
      load(newKey, doc, language, readonly) {
        whenIdle(() => {
          flushPending();
          saveCurrent();
          key = newKey;
          dirtySent = false;
          const state = makeState(doc, language, readonly);
          states.set(newKey, { state, scroll: 0, readonly });
          view.setState(state);
          reapply(readonly);
          view.scrollDOM.scrollTop = 0;
        });
      },
      show(newKey) {
        whenIdle(() => {
          const entry = states.get(newKey);
          if (!entry) {
            send({ t: "missing", key: newKey });
            return;
          }
          flushPending();
          saveCurrent();
          key = newKey;
          dirtySent = false;
          view.setState(entry.state);
          reapply(entry.readonly);
          view.scrollDOM.scrollTop = entry.scroll;
        });
      },
      drop(oldKey) {
        states.delete(oldKey);
        if (key === oldKey) {
          clearTimeout(timer);
          timer = null;
          key = null;
          view.setState(EditorState.create({ doc: "", extensions: [theme] }));
        }
      },
      settings(next) {
        settings = { ...settings, ...next };
        view.dom.style.setProperty("--forge-tab", String(settings.tab_size));
        const entry = key === null ? null : states.get(key);
        if (entry) reapply(entry.readonly);
      },
      flush(askedKey) {
        if (askedKey === key) {
          clearTimeout(timer);
          timer = null;
          dirtySent = false;
          send({ t: "flush", key: askedKey, s: view.state.doc.toString() });
        } else {
          const entry = states.get(askedKey);
          send({ t: "flush", key: askedKey, s: entry ? entry.state.doc.toString() : null });
        }
      },
      focus() {
        view.focus();
      },
      destroy() {
        clearTimeout(timer);
        view.destroy();
      },
    };
  },
};
```

- [ ] **Step 5: Install, build, and add the licence**

Run:

```bash
cd tools/editor-bundle && npm install && npm run build && cd ../..
ls -la crates/forge-shared/assets/editor.js
cat tools/editor-bundle/node_modules/@codemirror/view/LICENSE > crates/forge-shared/assets/EDITOR-LICENSE
printf '\n---\n\n' >> crates/forge-shared/assets/EDITOR-LICENSE
cat tools/editor-bundle/node_modules/@lezer/common/LICENSE >> crates/forge-shared/assets/EDITOR-LICENSE
```
Expected: `editor.js` exists and is a few hundred KiB; `EDITOR-LICENSE` contains "Marijn Haverbeke". If `npm install` reports a different resolved version, keep the lock file's versions and update `package.json` to match.

- [ ] **Step 6: Implement `editor_head` and wire it into the window**

Put this above the `tests` module in `crates/forge-shared/src/editor.rs`:

```rust
const EDITOR_JS: &str = include_str!("../assets/editor.js");

/// The HTML that goes into the window's `<head>` for the Files panel's editor: CodeMirror 6 and
/// Forge's glue (`window.ForgeEditor`), inline, so the editor needs no network (MIT licences:
/// `assets/EDITOR-LICENSE`). Built from `tools/editor-bundle`.
pub fn editor_head() -> String {
    format!("<script>{EDITOR_JS}</script>")
}
```

In `crates/forge-app/src/main.rs` replace `.with_custom_head(forge_shared::xterm_head())` with:

```rust
        .with_custom_head(format!("{}{}", forge_shared::xterm_head(), forge_shared::editor_head()))
```

- [ ] **Step 7: Run the tests, then smoke-test the bundle in Node**

Run: `cargo test -p forge-shared 2>&1 | tail -8` then `cargo build -p forge-app 2>&1 | tail -3`
Expected: pass; builds.

Run a quick syntax check of the built script: `node --check crates/forge-shared/assets/editor.js && echo SYNTAX_OK`
Expected: `SYNTAX_OK`.

- [ ] **Step 8: Commit**

```bash
git add tools/editor-bundle/package.json tools/editor-bundle/package-lock.json tools/editor-bundle/build.mjs tools/editor-bundle/README.md tools/editor-bundle/src crates/forge-shared crates/forge-app
git commit -m "feat: the CodeMirror editor bundle and its place in the window head

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 7: The Rust side of the editor bridge

**Files:**
- Create: `crates/forge-widgets/src/files_editor.rs`
- Modify: `crates/forge-widgets/src/lib.rs`
- Test: in the new file

**Interfaces:**
- Consumes: the page API of Task 6; `forge_domain::EditorPrefs` (Task 1).
- Produces:
  - `pub enum FromPage { Ready, Unavailable, Dirty { key }, Change { key, s }, Save { key, s }, Flush { key, s: Option<String> }, Missing { key } }` (`Deserialize`, `Debug, Clone, PartialEq`).
  - `pub struct EditorHandle` (`Clone`) with `load(&self, key, doc, language, readonly: bool)`, `show(&self, key)`, `forget(&self, key)`, `settings(&self, prefs: &EditorPrefs)`, `flush(&self, key)`.
  - `pub fn use_editor_channel() -> EditorHandle` and `pub fn use_editor_driver(host_id: String, handle: &EditorHandle, on_event: Callback<FromPage>)`.
  - `pub fn editor_language(path: &str) -> &'static str` (a CodeMirror language id or `""`).

- [ ] **Step 1: Write the failing tests**

Create `crates/forge-widgets/src/files_editor.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn parse(json: &str) -> FromPage {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn the_messages_from_the_page_parse() {
        assert_eq!(parse(r#"{"t":"ready"}"#), FromPage::Ready);
        assert_eq!(parse(r#"{"t":"unavailable"}"#), FromPage::Unavailable);
        assert_eq!(parse(r#"{"t":"dirty","key":"a.py"}"#), FromPage::Dirty { key: "a.py".to_string() });
        assert_eq!(parse(r#"{"t":"change","key":"a.py","s":"x = 1\n"}"#), FromPage::Change { key: "a.py".to_string(), s: "x = 1\n".to_string() });
        assert_eq!(parse(r#"{"t":"save","key":"a.py","s":""}"#), FromPage::Save { key: "a.py".to_string(), s: String::new() });
        assert_eq!(parse(r#"{"t":"flush","key":"a.py","s":"t"}"#), FromPage::Flush { key: "a.py".to_string(), s: Some("t".to_string()) });
        assert_eq!(parse(r#"{"t":"flush","key":"a.py","s":null}"#), FromPage::Flush { key: "a.py".to_string(), s: None });
        assert_eq!(parse(r#"{"t":"missing","key":"a.py"}"#), FromPage::Missing { key: "a.py".to_string() });
    }

    #[test]
    fn a_path_with_quotes_and_hangul_survives_the_json() {
        let key = "한글 \"폴더\"/a.py";
        let message = serde_json::to_string(&serde_json::json!({ "t": "dirty", "key": key })).unwrap();
        assert_eq!(parse(&message), FromPage::Dirty { key: key.to_string() });
    }

    #[test]
    fn the_commands_to_the_page_carry_what_the_bridge_reads() {
        assert_eq!(
            load_command("a.py", "x = 1\n", "python", true),
            serde_json::json!({ "t": "load", "key": "a.py", "doc": "x = 1\n", "language": "python", "readonly": true })
        );
        assert_eq!(show_command("a.py"), serde_json::json!({ "t": "show", "key": "a.py" }));
        assert_eq!(forget_command("a.py"), serde_json::json!({ "t": "drop", "key": "a.py" }));
        assert_eq!(flush_command("a.py"), serde_json::json!({ "t": "flush", "key": "a.py" }));
    }

    #[test]
    fn the_settings_message_uses_the_names_the_page_reads_and_a_usable_tab_size() {
        let prefs = EditorPrefs { wrap: false, whitespace: true, tab_size: 3, ..EditorPrefs::default() };
        assert_eq!(
            settings_command(&prefs),
            serde_json::json!({
                "t": "settings",
                "settings": {
                    "wrap": false,
                    "indent_guides": true,
                    "whitespace": true,
                    "scroll_past_end": false,
                    "tab_indent": true,
                    "tab_size": 4
                }
            })
        );
    }

    #[test]
    fn the_language_follows_the_extension() {
        for (path, expected) in [
            ("a.py", "python"),
            ("src/main.RS", "rust"),
            ("x.tsx", "tsx"),
            ("x.ts", "typescript"),
            ("x.jsx", "jsx"),
            ("x.mjs", "javascript"),
            ("x.json", "json"),
            ("README.md", "markdown"),
            ("index.html", "html"),
            ("a.css", "css"),
            ("a.yml", "yaml"),
            ("run.sh", "shell"),
            ("Makefile", ""),
            (".gitignore", ""),
            ("notes.txt", ""),
        ] {
            assert_eq!(editor_language(path), expected, "{path}");
        }
    }
}
```

In `crates/forge-widgets/src/lib.rs` add `mod files_editor;` (next to `mod files_panel;`).

- [ ] **Step 2: Run to verify the failure**

Run: `cargo test -p forge-widgets files_editor 2>&1 | tail -10`
Expected: compile errors (`FromPage` etc. not found).

- [ ] **Step 3: Implement**

Put this above the `tests` module in `crates/forge-widgets/src/files_editor.rs`:

```rust
use std::cell::RefCell;
use std::rc::Rc;

use dioxus::prelude::*;
use forge_domain::EditorPrefs;
use forge_entities::language_of;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

/// What the page tells Rust (see `tools/editor-bundle/src/main.js`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "t", rename_all = "lowercase")]
pub enum FromPage {
    /// The editor is built and takes commands.
    Ready,
    /// The bundle did not load (no `ForgeEditor`).
    Unavailable,
    /// The first edit of a tab since a load or a save.
    Dirty { key: String },
    /// The text, 2 s after the last edit.
    Change { key: String, s: String },
    /// `⌘S`.
    Save { key: String, s: String },
    /// The answer to a flush: the tab's text, or `None` when the page has no state for it.
    Flush { key: String, s: Option<String> },
    /// A tab was asked for that the page never loaded.
    Missing { key: String },
}

pub fn load_command(key: &str, doc: &str, language: &str, readonly: bool) -> Value {
    json!({ "t": "load", "key": key, "doc": doc, "language": language, "readonly": readonly })
}

pub fn show_command(key: &str) -> Value {
    json!({ "t": "show", "key": key })
}

pub fn forget_command(key: &str) -> Value {
    json!({ "t": "drop", "key": key })
}

pub fn flush_command(key: &str) -> Value {
    json!({ "t": "flush", "key": key })
}

pub fn settings_command(prefs: &EditorPrefs) -> Value {
    json!({
        "t": "settings",
        "settings": {
            "wrap": prefs.wrap,
            "indent_guides": prefs.indent_guides,
            "whitespace": prefs.whitespace,
            "scroll_past_end": prefs.scroll_past_end,
            "tab_indent": prefs.tab_indent,
            "tab_size": prefs.clamped_tab_size(),
        }
    })
}

/// The CodeMirror language id of a file, or `""` for plain text.
pub fn editor_language(path: &str) -> &'static str {
    match language_of(path).to_ascii_lowercase().as_str() {
        "js" | "mjs" | "cjs" => "javascript",
        "jsx" => "jsx",
        "ts" | "mts" | "cts" => "typescript",
        "tsx" => "tsx",
        "py" | "pyi" => "python",
        "rs" => "rust",
        "json" | "jsonc" => "json",
        "md" | "markdown" => "markdown",
        "html" | "htm" => "html",
        "css" => "css",
        "yml" | "yaml" => "yaml",
        "sh" | "bash" | "zsh" => "shell",
        _ => "",
    }
}

type Inbox = Rc<RefCell<Option<UnboundedReceiver<Value>>>>;

/// Sends commands to the editor in the page. Commands wait in a queue until the page says it is
/// ready, so a tab can be loaded before the editor exists.
#[derive(Clone)]
pub struct EditorHandle {
    tx: UnboundedSender<Value>,
    inbox: Inbox,
}

impl EditorHandle {
    fn send(&self, command: Value) {
        let _ = self.tx.send(command);
    }

    pub fn load(&self, key: &str, doc: &str, language: &str, readonly: bool) {
        self.send(load_command(key, doc, language, readonly));
    }

    pub fn show(&self, key: &str) {
        self.send(show_command(key));
    }

    /// The tab is gone: the page drops its state.
    pub fn forget(&self, key: &str) {
        self.send(forget_command(key));
    }

    pub fn settings(&self, prefs: &EditorPrefs) {
        self.send(settings_command(prefs));
    }

    /// Asks for the tab's text; the answer comes as `FromPage::Flush`.
    pub fn flush(&self, key: &str) {
        self.send(flush_command(key));
    }
}

/// The channel the panel talks to the page through. Call it first, then `use_editor_driver` once
/// the event handler exists (the handler needs the handle).
pub fn use_editor_channel() -> EditorHandle {
    use_hook(|| {
        let (tx, rx) = unbounded_channel();
        EditorHandle { tx, inbox: Rc::new(RefCell::new(Some(rx))) }
    })
}

// The page side: builds the editor in the element `__HOST__` and then relays messages both ways.
// It cleans itself up when its element leaves the page.
const BRIDGE_JS: &str = r#"
const host = document.getElementById("__HOST__");
if (!window.ForgeEditor || !host) {
    dioxus.send({ t: "unavailable" });
} else {
    const editor = window.ForgeEditor.create(host, (m) => dioxus.send(m));
    const watch = setInterval(() => {
        if (!host.isConnected) { clearInterval(watch); editor.destroy(); }
    }, 2000);
    dioxus.send({ t: "ready" });
    while (true) {
        const m = await dioxus.recv();
        if (m.t === "load") editor.load(m.key, m.doc, m.language, m.readonly);
        else if (m.t === "show") editor.show(m.key);
        else if (m.t === "drop") editor.drop(m.key);
        else if (m.t === "settings") editor.settings(m.settings);
        else if (m.t === "flush") editor.flush(m.key);
        else if (m.t === "focus") editor.focus();
    }
}
"#;

async fn drive(host_id: String, inbox: Inbox, on_event: Callback<FromPage>) {
    let Some(mut commands) = inbox.borrow_mut().take() else { return };
    let mut page = document::eval(&BRIDGE_JS.replace("__HOST__", &host_id));
    // Nothing is sent to the page before it says it is ready.
    loop {
        match page.recv::<FromPage>().await {
            Ok(FromPage::Ready) => {
                on_event.call(FromPage::Ready);
                break;
            }
            Ok(FromPage::Unavailable) => {
                on_event.call(FromPage::Unavailable);
                return;
            }
            Ok(other) => on_event.call(other),
            Err(_) => return,
        }
    }
    loop {
        tokio::select! {
            event = page.recv::<FromPage>() => match event {
                Ok(event) => on_event.call(event),
                Err(_) => return,
            },
            command = commands.recv() => match command {
                Some(command) => {
                    let _ = page.send(command);
                }
                None => return,
            },
        }
    }
}

/// Starts the bridge to the editor that lives in the element with id `host_id` (render the
/// element before this runs; it is built after the first render). `on_event` gets every message
/// from the page.
pub fn use_editor_driver(host_id: String, handle: &EditorHandle, on_event: Callback<FromPage>) {
    let inbox = handle.inbox.clone();
    use_future(move || {
        let (host_id, inbox) = (host_id.clone(), inbox.clone());
        async move { drive(host_id, inbox, on_event).await }
    });
}
```

- [ ] **Step 4: Run and commit**

Run: `cargo test -p forge-widgets files_editor 2>&1 | tail -8` then `cargo build -p forge-widgets 2>&1 | tail -5`
Expected: 5 tests pass; builds (warnings for unused items are fine until Task 10).

```bash
git add -A crates
git commit -m "feat: the Rust side of the editor bridge

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 8: File kinds: image, binary, too large, read only notices

**Files:**
- Create: `crates/forge-widgets/src/files_kinds.rs`
- Modify: `crates/forge-widgets/src/lib.rs`
- Test: in the new file

**Interfaces:**
- Consumes: `forge_ports::FileContent` (Task 2).
- Produces:
  - `pub enum TabKind { Loading, Text { lossy: bool, anyway: bool }, Image { data_url: String }, Binary, TooLarge, Failed(String) }` (`Debug, Clone, PartialEq`) with `read_only(&self) -> bool` and `notice(&self) -> Option<&'static str>`.
  - `pub fn kind_of(content: FileContent) -> TabKind`.
  - `FileKindView { kind: TabKind, path: String, on_anyway: EventHandler<()>, on_reveal: EventHandler<()> }`.

- [ ] **Step 1: Write the failing tests**

Create `crates/forge-widgets/src/files_kinds.rs`:

```rust
#[cfg(test)]
mod tests {
    use forge_ports::FileVersion;

    use super::*;

    fn version() -> FileVersion {
        FileVersion { modified_ns: 1, len: 1 }
    }

    #[test]
    fn text_is_editable_unless_it_was_read_lossily_or_forced() {
        let plain = kind_of(FileContent::Text { text: "a".to_string(), version: version(), lossy: false });
        assert_eq!(plain, TabKind::Text { lossy: false, anyway: false });
        assert!(!plain.read_only());
        assert_eq!(plain.notice(), None);

        let lossy = kind_of(FileContent::Text { text: "a\u{fffd}".to_string(), version: version(), lossy: true });
        assert!(lossy.read_only(), "a file that is not valid UTF-8 must never be written back");
        assert_eq!(lossy.notice(), Some("This file is not valid UTF-8, so it is shown read only."));

        let forced = TabKind::Text { lossy: false, anyway: true };
        assert!(forced.read_only(), "a binary file shown as text must never be written back");
        assert_eq!(forced.notice(), Some("Shown as text. This file cannot be edited."));
    }

    #[test]
    fn an_image_becomes_a_data_address_with_its_type() {
        let kind = kind_of(FileContent::Image { bytes: vec![1, 2, 3], mime: "image/png".to_string() });
        assert_eq!(kind, TabKind::Image { data_url: "data:image/png;base64,AQID".to_string() });
    }

    #[test]
    fn binary_and_too_large_map_straight_across() {
        assert_eq!(kind_of(FileContent::Binary), TabKind::Binary);
        assert_eq!(kind_of(FileContent::TooLarge), TabKind::TooLarge);
    }

    #[test]
    fn only_text_has_an_editor_and_only_other_kinds_are_not_read_only() {
        assert!(TabKind::Text { lossy: false, anyway: false }.shows_editor());
        for other in [TabKind::Loading, TabKind::Binary, TabKind::TooLarge, TabKind::Failed("x".to_string())] {
            assert!(!other.shows_editor(), "{other:?}");
        }
    }
}
```

In `crates/forge-widgets/src/lib.rs` add `mod files_kinds;`.

- [ ] **Step 2: Run to verify the failure**

Run: `cargo test -p forge-widgets files_kinds 2>&1 | tail -8`
Expected: compile error.

- [ ] **Step 3: Implement**

Put this above the `tests` module:

```rust
use base64::Engine;
use dioxus::prelude::*;
use forge_ports::FileContent;

/// What an open tab shows.
#[derive(Debug, Clone, PartialEq)]
pub enum TabKind {
    Loading,
    /// `lossy`: the bytes were not valid UTF-8. `anyway`: "Show as text anyway" on a binary file.
    Text { lossy: bool, anyway: bool },
    Image { data_url: String },
    Binary,
    TooLarge,
    Failed(String),
}

impl TabKind {
    /// Text that must never be written back: it was read lossily, or it is binary shown as text.
    pub fn read_only(&self) -> bool {
        matches!(self, TabKind::Text { lossy: true, .. } | TabKind::Text { anyway: true, .. })
    }

    /// Whether the editor (not a note or a picture) is what this tab shows.
    pub fn shows_editor(&self) -> bool {
        matches!(self, TabKind::Text { .. })
    }

    /// A line above the editor when the text cannot be edited.
    pub fn notice(&self) -> Option<&'static str> {
        match self {
            TabKind::Text { lossy: true, .. } => Some("This file is not valid UTF-8, so it is shown read only."),
            TabKind::Text { anyway: true, .. } => Some("Shown as text. This file cannot be edited."),
            _ => None,
        }
    }
}

pub fn kind_of(content: FileContent) -> TabKind {
    match content {
        FileContent::Text { lossy, .. } => TabKind::Text { lossy, anyway: false },
        FileContent::Image { bytes, mime } => {
            TabKind::Image { data_url: format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)) }
        }
        FileContent::Binary => TabKind::Binary,
        FileContent::TooLarge => TabKind::TooLarge,
    }
}

/// The body of a tab that has no editor: a picture, a binary file, a file that is too large.
#[component]
pub fn FileKindView(kind: TabKind, path: String, on_anyway: EventHandler<()>, on_reveal: EventHandler<()>) -> Element {
    match kind {
        TabKind::Image { data_url } => rsx! {
            div { class: "files-kind",
                img { src: "{data_url}", alt: "{path}" }
            }
        },
        TabKind::Binary => rsx! {
            div { class: "files-kind",
                p { "This file looks like binary data and cannot be previewed as text." }
                div { class: "files-kind-buttons",
                    button { class: "files-kind-button", r#type: "button", onclick: move |_| on_reveal.call(()), "Show in Finder" }
                    button { class: "files-kind-button", r#type: "button", onclick: move |_| on_anyway.call(()), "Show as text anyway" }
                }
            }
        },
        TabKind::TooLarge => rsx! {
            div { class: "files-kind",
                p { class: "files-kind-title", "This file cannot be previewed" }
                p { class: "files-kind-sub", "It is too large to preview here." }
                p { class: "files-kind-sub", "{path}" }
                div { class: "files-kind-buttons",
                    button { class: "files-kind-button", r#type: "button", onclick: move |_| on_reveal.call(()), "Show in Finder" }
                }
            }
        },
        TabKind::Loading | TabKind::Text { .. } | TabKind::Failed(_) => rsx! {},
    }
}
```

- [ ] **Step 4: Run and commit**

Run: `cargo test -p forge-widgets files_kinds 2>&1 | tail -8`
Expected: 4 tests pass.

```bash
git add -A crates
git commit -m "feat: tab kinds for the Files panel: image, binary, too large, read only

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 9: The tab strip, the `⋮` menu and the styles

**Files:**
- Create: `crates/forge-widgets/src/files_tabs.rs`, `crates/forge-widgets/src/files_menu.rs`
- Modify: `crates/forge-widgets/src/lib.rs`, `crates/forge-shared/src/theme.css`, `crates/forge-shared/src/theme.rs`
- Test: `files_menu.rs` (its `tests` module) and the class-list test in `theme.rs`

**Interfaces:**
- Consumes: `Workspace::editor_prefs`, `update_editor_prefs` (Task 1).
- Produces:
  - `pub struct TabView { pub path: String, pub preview: bool }`
  - `FilesTabs { tabs: Vec<TabView>, selected: Option<String>, on_select, on_close, on_reorder: EventHandler<(String, String)>, on_keep, on_close_others: EventHandler<String>, on_close_all: EventHandler<()> }`
  - `FilesMenu {}` (the header `⋮` menu).

- [ ] **Step 1: Write the failing tests**

Create `crates/forge-widgets/src/files_menu.rs` with the tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_menu_item_toggles_its_own_setting_and_nothing_else() {
        let items = [
            Item::Tree,
            Item::PreviewTabs,
            Item::AutoSave,
            Item::TabIndent,
            Item::Wrap,
            Item::Guides,
            Item::Whitespace,
            Item::ScrollPastEnd,
        ];
        for item in items {
            let before = EditorPrefs::default();
            let mut after = before.clone();
            let was = item.is_on(&before);
            item.toggle(&mut after);
            assert_eq!(item.is_on(&after), !was, "{}", item.label());
            for other in items.into_iter().filter(|other| *other != item) {
                assert_eq!(other.is_on(&after), other.is_on(&before), "{} changed {}", item.label(), other.label());
            }
            assert_eq!(after.tab_size, before.tab_size);
        }
    }

    #[test]
    fn the_defaults_show_the_ticks_the_real_app_shows() {
        let prefs = EditorPrefs::default();
        let on: Vec<&str> = [Item::Tree, Item::PreviewTabs, Item::AutoSave, Item::TabIndent, Item::Wrap, Item::Guides, Item::Whitespace, Item::ScrollPastEnd]
            .into_iter()
            .filter(|item| item.is_on(&prefs))
            .map(Item::label)
            .collect();
        assert_eq!(on, vec!["Show file tree", "Preview tabs", "Auto save", "Tab key indents", "Word wrap", "Indent guides"]);
    }
}
```

In `crates/forge-widgets/src/lib.rs` add `mod files_menu;` and `mod files_tabs;`.

In `crates/forge-shared/src/theme.rs`, add these class names to the list in `the_stylesheet_defines_the_layout_classes_the_components_use` (before the closing `]`): `".files-main"`, `".files-tabs"`, `".files-tab"`, `".files-tab-close"`, `".files-tab-menu"`, `".files-banner"`, `".files-editor"`, `".files-kind"`, `".files-kind-button"`, `".files-menu-sub"`.

- [ ] **Step 2: Run to verify the failures**

Run: `cargo test -p forge-widgets files_menu 2>&1 | tail -6` and `cargo test -p forge-shared the_stylesheet 2>&1 | tail -8`
Expected: compile error in the widget; the theme test fails naming the missing classes.

- [ ] **Step 3: Implement the menu**

Put this above the `tests` module in `crates/forge-widgets/src/files_menu.rs`:

```rust
use dioxus::prelude::*;
use forge_domain::EditorPrefs;
use forge_features::use_workspace;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Item {
    Tree,
    PreviewTabs,
    AutoSave,
    TabIndent,
    Wrap,
    Guides,
    Whitespace,
    ScrollPastEnd,
}

impl Item {
    fn label(self) -> &'static str {
        match self {
            Item::Tree => "Show file tree",
            Item::PreviewTabs => "Preview tabs",
            Item::AutoSave => "Auto save",
            Item::TabIndent => "Tab key indents",
            Item::Wrap => "Word wrap",
            Item::Guides => "Indent guides",
            Item::Whitespace => "Show whitespace",
            Item::ScrollPastEnd => "Scroll past end",
        }
    }

    fn is_on(self, prefs: &EditorPrefs) -> bool {
        match self {
            Item::Tree => prefs.tree,
            Item::PreviewTabs => prefs.preview_tabs,
            Item::AutoSave => prefs.auto_save,
            Item::TabIndent => prefs.tab_indent,
            Item::Wrap => prefs.wrap,
            Item::Guides => prefs.indent_guides,
            Item::Whitespace => prefs.whitespace,
            Item::ScrollPastEnd => prefs.scroll_past_end,
        }
    }

    fn toggle(self, prefs: &mut EditorPrefs) {
        let field = match self {
            Item::Tree => &mut prefs.tree,
            Item::PreviewTabs => &mut prefs.preview_tabs,
            Item::AutoSave => &mut prefs.auto_save,
            Item::TabIndent => &mut prefs.tab_indent,
            Item::Wrap => &mut prefs.wrap,
            Item::Guides => &mut prefs.indent_guides,
            Item::Whitespace => &mut prefs.whitespace,
            Item::ScrollPastEnd => &mut prefs.scroll_past_end,
        };
        *field = !*field;
    }
}

// The menu's items in groups; a divider sits between groups.
const GROUPS: [&[Item]; 3] = [
    &[Item::Tree],
    &[Item::PreviewTabs, Item::AutoSave, Item::TabIndent],
    &[Item::Wrap, Item::Guides, Item::Whitespace, Item::ScrollPastEnd],
];

/// The `⋮` menu in the Files panel header: the editor settings. A tick click keeps the menu open
/// and takes effect at once. `Tab size` opens a small menu to the left.
#[component]
pub fn FilesMenu() -> Element {
    let workspace = use_workspace();
    let mut open = use_signal(|| false);
    let mut sizes = use_signal(|| false);
    let prefs = workspace.editor_prefs();
    let tab_size = prefs.clamped_tab_size();
    rsx! {
        div { class: "picker",
            button {
                class: if open() { "picker-button open" } else { "picker-button" },
                r#type: "button",
                title: "Editor settings",
                "aria-label": "Editor settings",
                "aria-haspopup": "menu",
                "aria-expanded": "{open()}",
                onclick: move |_| {
                    sizes.set(false);
                    open.set(!open());
                },
                "⋮"
            }
            if open() {
                div { class: "picker-backdrop", onclick: move |_| open.set(false) }
                div {
                    class: "picker-menu down",
                    role: "menu",
                    tabindex: "0",
                    onmounted: move |mounted| async move {
                        let _ = mounted.set_focus(true).await;
                    },
                    onkeydown: move |event: KeyboardEvent| {
                        if event.key() == Key::Escape {
                            event.stop_propagation();
                            open.set(false);
                        }
                    },
                    for (index, group) in GROUPS.iter().enumerate() {
                        if index > 0 {
                            div { class: "picker-divider" }
                        }
                        for item in group.iter().copied() {
                            button {
                                class: "picker-item",
                                r#type: "button",
                                role: "menuitemcheckbox",
                                "aria-checked": "{item.is_on(&prefs)}",
                                onclick: move |_| workspace.update_editor_prefs(|prefs| item.toggle(prefs)),
                                span { class: "picker-check", if item.is_on(&prefs) { "✓" } }
                                span { class: "picker-text", "{item.label()}" }
                            }
                        }
                    }
                    div { class: "picker-divider" }
                    div { class: "files-menu-row",
                        button {
                            class: "picker-item",
                            r#type: "button",
                            "aria-haspopup": "menu",
                            "aria-expanded": "{sizes()}",
                            onclick: move |_| sizes.set(!sizes()),
                            span { class: "picker-check" }
                            span { class: "picker-text", "Tab size" }
                            span { class: "menu-key", "{tab_size} ›" }
                        }
                        if sizes() {
                            div { class: "picker-menu files-menu-sub", role: "menu",
                                for size in [2u8, 4, 8] {
                                    button {
                                        class: "picker-item",
                                        r#type: "button",
                                        role: "menuitemradio",
                                        "aria-checked": "{size == tab_size}",
                                        onclick: move |_| {
                                            workspace.update_editor_prefs(|prefs| prefs.tab_size = size);
                                            sizes.set(false);
                                        },
                                        span { class: "picker-check", if size == tab_size { "✓" } }
                                        span { class: "picker-text", "{size}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
```

- [ ] **Step 4: Implement the tab strip**

Create `crates/forge-widgets/src/files_tabs.rs`:

```rust
use dioxus::html::input_data::MouseButton;
use dioxus::prelude::*;

/// One tab as the strip draws it.
#[derive(Debug, Clone, PartialEq)]
pub struct TabView {
    pub path: String,
    pub preview: bool,
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// The tabs above the editor. A preview tab is italic. A press on a tab and a release on another
/// reorders; a press and release on the same tab selects it. A right click opens the tab menu.
/// The pointer is followed with mouse events (not the HTML drag and drop, which a web view may
/// take over for file drops).
#[component]
pub fn FilesTabs(
    tabs: Vec<TabView>,
    selected: Option<String>,
    on_select: EventHandler<String>,
    on_close: EventHandler<String>,
    on_reorder: EventHandler<(String, String)>,
    on_keep: EventHandler<String>,
    on_close_others: EventHandler<String>,
    on_close_all: EventHandler<()>,
) -> Element {
    // The tab a press started on.
    let mut pressed = use_signal(|| None::<String>);
    // The tab menu: the tab, whether it is a preview tab, and where it was asked for.
    let mut menu = use_signal(|| None::<(String, bool, f64, f64)>);
    rsx! {
        div { class: "files-tabs", role: "tablist", onmouseleave: move |_| pressed.set(None),
            for tab in tabs.iter().cloned() {
                {
                    let path = tab.path.clone();
                    let is_selected = selected.as_deref() == Some(path.as_str());
                    let class = match (is_selected, tab.preview) {
                        (true, true) => "files-tab selected preview",
                        (true, false) => "files-tab selected",
                        (false, true) => "files-tab preview",
                        (false, false) => "files-tab",
                    };
                    rsx! {
                        div {
                            key: "{path}",
                            class: "{class}",
                            role: "tab",
                            title: "{path}",
                            "aria-selected": "{is_selected}",
                            onmousedown: {
                                let path = path.clone();
                                move |event: MouseEvent| {
                                    if event.trigger_button() == Some(MouseButton::Primary) {
                                        pressed.set(Some(path.clone()));
                                    }
                                }
                            },
                            onmouseup: {
                                let path = path.clone();
                                move |_| {
                                    let from = pressed.write().take();
                                    match from {
                                        Some(from) if from != path => on_reorder.call((from, path.clone())),
                                        Some(_) => on_select.call(path.clone()),
                                        None => {}
                                    }
                                }
                            },
                            oncontextmenu: {
                                let (path, preview) = (path.clone(), tab.preview);
                                move |event: MouseEvent| {
                                    event.prevent_default();
                                    let point = event.client_coordinates();
                                    menu.set(Some((path.clone(), preview, point.x, point.y)));
                                }
                            },
                            span { class: "files-tab-name", "{file_name(&path)}" }
                            button {
                                class: "files-tab-close",
                                r#type: "button",
                                title: "Close",
                                "aria-label": "Close {file_name(&path)}",
                                onmousedown: move |event: MouseEvent| event.stop_propagation(),
                                onmouseup: move |event: MouseEvent| event.stop_propagation(),
                                onclick: {
                                    let path = path.clone();
                                    move |event: MouseEvent| {
                                        event.stop_propagation();
                                        on_close.call(path.clone());
                                    }
                                },
                                "×"
                            }
                        }
                    }
                }
            }
        }
        if let Some((path, preview, x, y)) = menu() {
            div {
                class: "picker-backdrop",
                onclick: move |_| menu.set(None),
                oncontextmenu: move |event: MouseEvent| {
                    event.prevent_default();
                    menu.set(None);
                },
            }
            div { class: "picker-menu files-tab-menu", role: "menu", style: "left: {x}px; top: {y}px;",
                button {
                    class: if preview { "picker-item" } else { "picker-item disabled" },
                    r#type: "button",
                    role: "menuitem",
                    onclick: {
                        let path = path.clone();
                        move |_| {
                            if preview {
                                on_keep.call(path.clone());
                            }
                            menu.set(None);
                        }
                    },
                    "Keep open"
                }
                button {
                    class: "picker-item",
                    r#type: "button",
                    role: "menuitem",
                    onclick: {
                        let path = path.clone();
                        move |_| {
                            on_close.call(path.clone());
                            menu.set(None);
                        }
                    },
                    "Close"
                }
                button {
                    class: "picker-item",
                    r#type: "button",
                    role: "menuitem",
                    onclick: {
                        let path = path.clone();
                        move |_| {
                            on_close_others.call(path.clone());
                            menu.set(None);
                        }
                    },
                    "Close others"
                }
                button {
                    class: "picker-item",
                    r#type: "button",
                    role: "menuitem",
                    onclick: move |_| {
                        on_close_all.call(());
                        menu.set(None);
                    },
                    "Close all"
                }
            }
        }
    }
}
```

If the compiler cannot find `MouseButton` at `dioxus::html::input_data::MouseButton`, use whichever path `cargo doc -p dioxus-html` shows for `MouseButton` (it is the type `MouseEvent::trigger_button` returns). If `Signal::write().take()` on an `Option` does not compile, use `let from = pressed(); pressed.set(None);`.

- [ ] **Step 5: Add the styles**

In `crates/forge-shared/src/theme.css`, after the `.files-source-toggle` line (near line 597) add:

```css
/* The Files panel as an editor: tabs, banners, the editor, and screens for other kinds of file. */
.files-main { flex: 1; min-width: 0; display: flex; flex-direction: column; }
.files-tabs { flex: none; display: flex; gap: 2px; padding: 4px; overflow-x: auto; border-bottom: 1px solid var(--border); }
.files-tab { flex: none; display: inline-flex; align-items: center; gap: 2px; padding: 2px 4px 2px 8px; border-radius: 6px; font-size: 12px; cursor: pointer; user-select: none; }
.files-tab:hover, .files-tab.selected { background: var(--bg-hover); }
.files-tab.preview .files-tab-name { font-style: italic; }
.files-tab-close { border: none; border-radius: 4px; background: transparent; color: var(--fg-muted); font: inherit; font-size: 12px; line-height: 1; padding: 2px 4px; cursor: pointer; }
.files-tab-close:hover { background: var(--border); color: var(--fg); }
.files-tab-menu { position: fixed; bottom: auto; right: auto; }
.files-banner { flex: none; display: flex; align-items: center; gap: 8px; padding: 6px 10px; background: var(--bg-hover); border-bottom: 1px solid var(--border); font-size: 12px; }
.files-banner span { flex: 1; min-width: 0; }
.files-banner button { flex: none; padding: 2px 8px; border: 1px solid var(--border); border-radius: 6px; background: var(--bg-card); color: var(--fg); font: inherit; font-size: 12px; cursor: pointer; }
.files-banner button:hover { background: var(--bg); }
.files-view { flex: 1; min-height: 0; display: flex; flex-direction: column; }
.files-editor { flex: 1; min-width: 0; min-height: 0; }
.files-editor.hidden { display: none; }
.files-kind { flex: 1; min-height: 0; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 8px; padding: 16px; text-align: center; font-size: 13px; }
.files-kind img { max-width: 100%; max-height: 100%; }
.files-kind-title { margin: 0; font-size: 14px; }
.files-kind-sub { margin: 0; color: var(--fg-muted); font-size: 12px; overflow-wrap: anywhere; }
.files-kind-buttons { display: flex; gap: 8px; }
.files-kind-button { padding: 3px 10px; border: 1px solid var(--border); border-radius: 6px; background: var(--bg-card); color: var(--fg); font: inherit; font-size: 12px; cursor: pointer; }
.files-kind-button:hover { background: var(--bg-hover); }
.files-menu-row { position: relative; }
.files-menu-sub { position: absolute; top: 0; right: calc(100% + 4px); left: auto; bottom: auto; min-width: 90px; }
```

- [ ] **Step 6: Run and commit**

Run: `cargo test -p forge-widgets files_menu 2>&1 | tail -6`, `cargo test -p forge-shared 2>&1 | tail -6`, and `cargo build -p forge-widgets 2>&1 | tail -5`
Expected: menu tests pass, the theme test passes, builds.

```bash
git add -A crates
git commit -m "feat: the Files panel tab strip, its settings menu and styles

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 10: The panel: state, events, and the new `FilesPanel`

**Files:**
- Create: `crates/forge-widgets/src/files_state.rs`
- Rewrite: `crates/forge-widgets/src/files_panel.rs`
- Modify: `crates/forge-widgets/src/lib.rs` (add `mod files_state;`)
- Modify: `crates/forge-features/src/lib.rs` (nothing new if Tasks 4 and 5 exported their types)
- Test: compile and the existing suite; behaviour is checked in Task 11

**Interfaces:**
- Consumes: everything above (`EditorTabs`, `TabDoc`, `Banner`, `FileContent`, `EditorHandle`, `FromPage`, `TabKind`, `kind_of`, `FilesTabs`, `FilesMenu`, `FileKindView`, `Workspace::{editor_prefs, update_editor_prefs, reveal}`).
- Produces: the `FilesPanel { id, cwd }` component with the same props as before, so `dock.rs` is unchanged.

- [ ] **Step 1: Write the state and its operations**

Create `crates/forge-widgets/src/files_state.rs`:

```rust
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use dioxus::prelude::*;
use forge_entities::{code_html, is_markdown, language_of, render_markdown};
use forge_features::{run_blocking, EditorTabs, TabDoc, Workspace};
use forge_ports::{FileContent, FilesError, FilesPort};

use crate::files_editor::{editor_language, EditorHandle, FromPage};
use crate::files_kinds::{kind_of, TabKind};

// Highlighting a file larger than this would keep the UI busy, so the fallback shows plain text.
const HIGHLIGHT_LIMIT: usize = 200_000;

/// What a `flush` answer from the page is for. One per tab at a time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pending {
    /// Write the text (`overwrite`: ignore the version check).
    Save { overwrite: bool },
    /// The tab is being closed: decide whether to ask first.
    Close,
    /// A markdown tab goes back to its rendered view.
    Render,
    /// A tool call finished: re-read the file when the tab is clean and the disk changed.
    Refresh,
}

/// Everything the panel does, in one place. It is `Clone` (signals are `Copy`), so event handlers
/// take a clone. The text of an open file lives in the editor in the page; Rust keeps a `TabDoc`
/// (the version and a hash of what is on disk) per tab.
#[derive(Clone)]
pub struct FilesCtx {
    pub workspace: Workspace,
    pub cwd: PathBuf,
    pub handle: EditorHandle,
    pub tabs: Signal<EditorTabs>,
    pub kinds: Signal<HashMap<String, TabKind>>,
    pub docs: Signal<HashMap<String, TabDoc>>,
    /// Rendered markdown of a markdown tab.
    pub markdown: Signal<HashMap<String, String>>,
    /// Markdown tabs shown as source (editable) instead of rendered.
    pub source: Signal<HashSet<String>>,
    pub pending: Signal<HashMap<String, Pending>>,
    /// A tab with edits that cannot be saved on its own, and the text it holds, waiting for an answer.
    pub confirm: Signal<Option<(String, String)>>,
    /// Tabs still to close one by one (close others, close all).
    pub queue: Signal<Vec<String>>,
    /// Highlighted html and line numbers, used only when the editor bundle did not load.
    pub fallback: Signal<HashMap<String, (String, String)>>,
    pub ready: Signal<bool>,
    pub unavailable: Signal<bool>,
}

impl FilesCtx {
    fn files(&self) -> Arc<dyn FilesPort> {
        self.workspace.files()
    }

    fn selected(&self) -> Option<String> {
        self.tabs.peek().selected().map(str::to_string)
    }

    // Whether the editor in the page is what the tab shows: a text tab, and for markdown only in source mode.
    fn editor_visible(&self, path: &str) -> bool {
        !*self.unavailable.peek()
            && matches!(self.kinds.peek().get(path), Some(TabKind::Text { .. }))
            && (!is_markdown(path) || self.source.peek().contains(path))
    }

    /// Opens a file from the tree, a search hit or a link in the chat.
    pub fn open(&self, path: String) {
        let preview_tabs = self.workspace.editor_prefs().preview_tabs;
        let mut tabs = self.tabs;
        let replaced = tabs.write().open(&path, preview_tabs);
        if let Some(old) = replaced {
            self.forget(&old);
        }
        self.show_selected();
    }

    pub fn select(&self, path: &str) {
        let mut tabs = self.tabs;
        tabs.write().select(path);
        self.show_selected();
    }

    /// Makes the selected tab show: reads it the first time, else puts its editor state back.
    pub fn show_selected(&self) {
        let Some(path) = self.selected() else { return };
        if self.kinds.peek().contains_key(&path) {
            if self.editor_visible(&path) {
                self.handle.show(&path);
            }
        } else {
            let mut kinds = self.kinds;
            kinds.write().insert(path.clone(), TabKind::Loading);
            self.read_and_apply(path);
        }
    }

    // Reads the file and applies the result when the tab is still open.
    fn read_and_apply(&self, path: String) {
        let ctx = self.clone();
        spawn(async move {
            let (files, root, relative) = (ctx.files(), ctx.cwd.clone(), path.clone());
            let Some(result) = run_blocking(move || files.read(&root, &relative)).await else { return };
            if ctx.tabs.peek().contains(&path) {
                ctx.apply(path, result);
            }
        });
    }

    fn apply(&self, path: String, result: Result<FileContent, FilesError>) {
        let (mut kinds, mut docs) = (self.kinds, self.docs);
        match result {
            Ok(FileContent::Text { text, version, lossy }) => {
                docs.write().insert(path.clone(), TabDoc::new(version, &text));
                kinds.write().insert(path.clone(), TabKind::Text { lossy, anyway: false });
                self.take_text(&path, &text, lossy);
            }
            Ok(other) => {
                docs.write().remove(&path);
                kinds.write().insert(path, kind_of(other));
            }
            Err(error) => {
                kinds.write().insert(path, TabKind::Failed(error.to_string()));
            }
        }
    }

    // The text of a file that was just read: markdown is rendered, and the editor takes it when it is
    // the selected tab's view.
    fn take_text(&self, path: &str, text: &str, read_only: bool) {
        if is_markdown(path) {
            let mut markdown = self.markdown;
            markdown.write().insert(path.to_string(), render_markdown(text));
        }
        if *self.unavailable.peek() {
            let language = if text.len() > HIGHLIGHT_LIMIT { "" } else { language_of(path) };
            let numbers = (1..=text.lines().count().max(1)).map(|n| n.to_string()).collect::<Vec<_>>().join("\n");
            let mut fallback = self.fallback;
            fallback.write().insert(path.to_string(), (code_html(language, text), numbers));
            return;
        }
        if self.selected().as_deref() == Some(path) && self.editor_visible(path) {
            self.handle.load(path, text, editor_language(path), read_only);
        }
    }

    /// "Show as text anyway" on a binary file: read only, never written back.
    pub fn show_anyway(&self, path: String) {
        let ctx = self.clone();
        spawn(async move {
            let (files, root, relative) = (ctx.files(), ctx.cwd.clone(), path.clone());
            let Some(result) = run_blocking(move || files.read_text_anyway(&root, &relative)).await else { return };
            let (mut kinds, mut docs) = (ctx.kinds, ctx.docs);
            match result {
                Ok(FileContent::Text { text, version, .. }) => {
                    docs.write().insert(path.clone(), TabDoc::new(version, &text));
                    kinds.write().insert(path.clone(), TabKind::Text { lossy: false, anyway: true });
                    ctx.take_text(&path, &text, true);
                }
                Ok(other) => {
                    kinds.write().insert(path, kind_of(other));
                }
                Err(error) => {
                    kinds.write().insert(path, TabKind::Failed(error.to_string()));
                }
            }
        });
    }

    pub fn reveal(&self, path: &str) {
        self.workspace.reveal(&self.cwd.join(path));
    }

    /// A message from the page.
    pub fn on_event(&self, event: FromPage) {
        match event {
            FromPage::Ready => {
                let mut ready = self.ready;
                ready.set(true);
            }
            FromPage::Unavailable => {
                let mut unavailable = self.unavailable;
                unavailable.set(true);
                // Tabs read before this was known get their plain viewer text now.
                for path in self.tabs.peek().paths() {
                    self.read_and_apply(path);
                }
            }
            FromPage::Dirty { key } => self.dirty(key),
            FromPage::Change { key, s } => self.change(key, s, false),
            FromPage::Save { key, s } => self.change(key, s, true),
            FromPage::Flush { key, s } => self.flushed(key, s),
            // The page never loaded this tab (it was not the visible one when its file was read).
            FromPage::Missing { key } => self.read_and_apply(key),
        }
    }

    // The first edit of a tab: it stops being a preview tab, and the disk is checked for a conflict.
    fn dirty(&self, key: String) {
        let mut tabs = self.tabs;
        tabs.write().keep(&key);
        let ctx = self.clone();
        spawn(async move {
            let (files, root, relative) = (ctx.files(), ctx.cwd.clone(), key.clone());
            let Some(result) = run_blocking(move || files.stat(&root, &relative)).await else { return };
            let mut docs = ctx.docs;
            if let Some(doc) = docs.write().get_mut(&key) {
                doc.on_dirty(result.map_err(|error| error.to_string()));
            }
        });
    }

    // The timer's text (or `⌘S` with `manual`): written unless auto save is off or a banner waits.
    fn change(&self, key: String, text: String, manual: bool) {
        let auto_save = self.workspace.editor_prefs().auto_save || manual;
        let allowed = self.docs.peek().get(&key).is_some_and(|doc| doc.may_write(auto_save) && doc.differs(&text));
        if allowed {
            self.write(key, text, false, false);
        }
    }

    // Writes `text`; with `close` the tab closes when the write worked (and the queue stops when it did not).
    fn write(&self, key: String, text: String, overwrite: bool, close: bool) {
        let expected = if overwrite { None } else { self.docs.peek().get(&key).map(|doc| doc.loaded) };
        let ctx = self.clone();
        spawn(async move {
            let (files, root, relative, body) = (ctx.files(), ctx.cwd.clone(), key.clone(), text.clone());
            let Some(result) = run_blocking(move || files.write(&root, &relative, &body, expected)).await else { return };
            let saved = result.is_ok();
            let mut docs = ctx.docs;
            if let Some(doc) = docs.write().get_mut(&key) {
                match result {
                    Ok(version) => doc.written(version, &text),
                    Err(error) => doc.write_failed(&error),
                }
            }
            if !close {
                return;
            }
            if saved {
                ctx.close_now(&key);
                ctx.pump();
            } else {
                // The tab stays, with its banner on screen.
                let mut queue = ctx.queue;
                queue.write().clear();
                ctx.select(&key);
            }
        });
    }

    fn flushed(&self, key: String, text: Option<String>) {
        let mut pending = self.pending;
        let Some(what) = pending.write().remove(&key) else { return };
        match what {
            Pending::Save { overwrite } => {
                if let Some(text) = text {
                    self.write(key, text, overwrite, false);
                }
            }
            Pending::Close => self.decide_close(key, text),
            Pending::Render => {
                if let Some(text) = text {
                    let mut markdown = self.markdown;
                    markdown.write().insert(key, render_markdown(&text));
                }
            }
            Pending::Refresh => self.refresh(key, text),
        }
    }

    fn ask(&self, key: &str, what: Pending) {
        let mut pending = self.pending;
        pending.write().insert(key.to_string(), what);
        self.handle.flush(key);
    }

    /// The banner's Overwrite, and Retry after a failed save: write what the editor holds now.
    pub fn save_from_editor(&self, key: &str, overwrite: bool) {
        if !overwrite {
            let mut docs = self.docs;
            if let Some(doc) = docs.write().get_mut(key) {
                doc.clear_banner();
            }
        }
        self.ask(key, Pending::Save { overwrite });
    }

    /// The banner's Cancel (and Dismiss): drop the edits and show the file on disk.
    pub fn reload(&self, key: String) {
        self.read_and_apply(key);
    }

    pub fn dismiss_banner(&self, key: &str) {
        let mut docs = self.docs;
        if let Some(doc) = docs.write().get_mut(key) {
            doc.clear_banner();
        }
    }

    /// The tab's `×`, the tab menu, and the queue of a close others or close all.
    pub fn close(&self, path: String) {
        let has_editor_state = self.docs.peek().contains_key(&path) && !*self.unavailable.peek();
        if has_editor_state {
            self.ask(&path, Pending::Close);
        } else {
            self.close_now(&path);
            self.pump();
        }
    }

    // The page's text decides: a tab whose text is what is on disk closes at once; otherwise it is
    // saved (auto save on, no banner) or the person is asked.
    fn decide_close(&self, key: String, text: Option<String>) {
        let differs = match (&text, self.docs.peek().get(&key)) {
            (Some(text), Some(doc)) => doc.differs(text),
            _ => false,
        };
        if !differs {
            self.close_now(&key);
            self.pump();
            return;
        }
        let text = text.unwrap_or_default();
        let auto_save = self.workspace.editor_prefs().auto_save;
        let may_write = self.docs.peek().get(&key).is_some_and(|doc| doc.may_write(auto_save));
        if may_write {
            self.write(key, text, false, true);
        } else {
            self.select(&key);
            let mut confirm = self.confirm;
            confirm.set(Some((key, text)));
        }
    }

    /// Save on the close question.
    pub fn confirm_save(&self) {
        let mut confirm = self.confirm;
        if let Some((key, text)) = confirm.write().take() {
            self.write(key, text, false, true);
        }
    }

    /// Don't save on the close question.
    pub fn confirm_discard(&self) {
        let mut confirm = self.confirm;
        if let Some((key, _)) = confirm.write().take() {
            self.close_now(&key);
            self.pump();
        }
    }

    /// Cancel on the close question: the tab stays and the rest of the queue is dropped.
    pub fn confirm_cancel(&self) {
        let (mut confirm, mut queue) = (self.confirm, self.queue);
        confirm.set(None);
        queue.write().clear();
    }

    fn close_now(&self, path: &str) {
        let mut tabs = self.tabs;
        tabs.write().close(path);
        self.forget(path);
        self.show_selected();
    }

    // Everything remembered about a tab, here and in the page.
    fn forget(&self, path: &str) {
        self.handle.forget(path);
        let (mut kinds, mut docs, mut markdown, mut source, mut pending, mut fallback) =
            (self.kinds, self.docs, self.markdown, self.source, self.pending, self.fallback);
        kinds.write().remove(path);
        docs.write().remove(path);
        markdown.write().remove(path);
        source.write().remove(path);
        pending.write().remove(path);
        fallback.write().remove(path);
    }

    pub fn close_others(&self, keep: String) {
        let mut tabs = self.tabs;
        tabs.write().select(&keep);
        let others: Vec<String> = self.tabs.peek().paths().into_iter().filter(|path| *path != keep).collect();
        self.close_many(others);
    }

    pub fn close_all(&self) {
        let all = self.tabs.peek().paths();
        self.close_many(all);
    }

    fn close_many(&self, paths: Vec<String>) {
        let mut queue = self.queue;
        queue.set(paths);
        self.pump();
    }

    // Closes the next tab of the queue; each close continues the queue when it finishes.
    fn pump(&self) {
        let mut queue = self.queue;
        let next = {
            let mut waiting = queue.write();
            if waiting.is_empty() {
                None
            } else {
                Some(waiting.remove(0))
            }
        };
        if let Some(path) = next {
            self.close(path);
        }
    }

    pub fn keep(&self, path: &str) {
        let mut tabs = self.tabs;
        tabs.write().keep(path);
    }

    pub fn reorder(&self, from: &str, to: &str) {
        let mut tabs = self.tabs;
        tabs.write().reorder(from, to);
    }

    /// The markdown `</>` button: source (editable) or rendered.
    pub fn toggle_source(&self, path: &str) {
        let mut source = self.source;
        if source.peek().contains(path) {
            source.write().remove(path);
            self.ask(path, Pending::Render);
        } else {
            source.write().insert(path.to_string());
            self.show_selected();
        }
    }

    /// A tool call of the session finished: tabs with no edits are read again when the disk changed.
    pub fn refresh_all(&self) {
        for path in self.tabs.peek().paths() {
            if self.docs.peek().contains_key(&path) {
                if *self.unavailable.peek() {
                    self.refresh(path, None);
                } else {
                    self.ask(&path, Pending::Refresh);
                }
            }
        }
    }

    fn refresh(&self, key: String, text: Option<String>) {
        let clean = match (&text, self.docs.peek().get(&key)) {
            (Some(text), Some(doc)) => doc.banner.is_none() && !doc.differs(text),
            (None, Some(doc)) => doc.banner.is_none(),
            _ => false,
        };
        if !clean {
            return;
        }
        let ctx = self.clone();
        spawn(async move {
            let (files, root, relative) = (ctx.files(), ctx.cwd.clone(), key.clone());
            let Some(Ok(version)) = run_blocking(move || files.stat(&root, &relative)).await else { return };
            let changed = ctx.docs.peek().get(&key).is_some_and(|doc| doc.loaded != version);
            if !changed {
                return;
            }
            if ctx.selected().as_deref() != Some(key.as_str()) {
                // The page still holds the old text of a tab that is not shown: let it go.
                ctx.handle.forget(&key);
            }
            ctx.read_and_apply(key);
        });
    }
}
```

- [ ] **Step 2: Rewrite the panel**

Replace the whole of `crates/forge-widgets/src/files_panel.rs` with:

```rust
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_entities::is_markdown;
use forge_features::{parse_files_query, run_blocking, use_workspace, Banner, EditorTabs, FileTree, FilesQuery, Panel};
use forge_ports::{ContentHit, FilesPort};

use crate::files_editor::{use_editor_channel, use_editor_driver, FromPage};
use crate::files_kinds::{FileKindView, TabKind};
use crate::files_menu::FilesMenu;
use crate::files_state::FilesCtx;
use crate::files_tabs::{FilesTabs, TabView};
use crate::panel_frame::PanelFrame;

// Typing waits this long before the files are searched.
const SEARCH_DEBOUNCE: Duration = Duration::from_millis(200);
const SEARCH_LIMIT: usize = 100;

async fn load_dir(files: Arc<dyn FilesPort>, root: PathBuf, dir: String, mut tree: Signal<FileTree>, mut problem: Signal<Option<String>>) {
    let key = dir.clone();
    match run_blocking(move || files.list(&root, &dir)).await {
        Some(Ok(entries)) => tree.write().set_children(&key, entries),
        // Only the root failing leaves nothing to show. A folder that cannot be read (a link out
        // of the session folder, no permission) just shows as empty.
        Some(Err(error)) if key.is_empty() => problem.set(Some(error.to_string())),
        Some(Err(_)) => tree.write().set_children(&key, Vec::new()),
        None => {}
    }
}

/// The Files panel: a file tree with a filter field (a filter that starts with `?` searches inside
/// the files), tabs, and an editor that saves by itself. See `files_state.rs` for what the panel does.
#[component]
pub fn FilesPanel(id: LiveSessionId, cwd: PathBuf) -> Element {
    let workspace = use_workspace();
    let mut tree = use_signal(FileTree::default);
    let mut problem = use_signal(|| None::<String>);
    let mut filter = use_signal(String::new);
    let mut hits = use_signal(|| None::<Vec<ContentHit>>);

    let handle = use_editor_channel();
    let ctx = FilesCtx {
        workspace,
        cwd: cwd.clone(),
        handle: handle.clone(),
        tabs: use_signal(EditorTabs::default),
        kinds: use_signal(HashMap::new),
        docs: use_signal(HashMap::new),
        markdown: use_signal(HashMap::new),
        source: use_signal(HashSet::new),
        pending: use_signal(HashMap::new),
        confirm: use_signal(|| None),
        queue: use_signal(Vec::new),
        fallback: use_signal(HashMap::new),
        ready: use_signal(|| false),
        unavailable: use_signal(|| false),
    };
    let host_id = format!("forge-editor-{}", id.0);
    let on_event = {
        let ctx = ctx.clone();
        use_callback(move |event: FromPage| ctx.on_event(event))
    };

    {
        let cwd = cwd.clone();
        use_future(move || {
            let cwd = cwd.clone();
            async move { load_dir(workspace.files(), cwd, String::new(), tree, problem).await }
        });
    }
    {
        let cwd = cwd.clone();
        use_effect(move || {
            let typed = filter();
            match parse_files_query(&typed) {
                FilesQuery::Content(query) if !query.is_empty() => {
                    let query = query.to_string();
                    let (files, root) = (workspace.files(), cwd.clone());
                    spawn(async move {
                        tokio::time::sleep(SEARCH_DEBOUNCE).await;
                        if filter.peek().as_str() != typed {
                            return;
                        }
                        if let Some(Ok(found)) = run_blocking(move || files.search(&root, &query, SEARCH_LIMIT)).await {
                            if filter.peek().as_str() == typed {
                                hits.set(Some(found));
                            }
                        }
                    });
                }
                _ => hits.set(None),
            }
        });
    }
    // The settings go to the page once it is ready and whenever they change.
    {
        let (handle, ready) = (handle.clone(), ctx.ready);
        use_effect(move || {
            let prefs = workspace.editor_prefs();
            if ready() {
                handle.settings(&prefs);
            }
        });
    }
    // A click in the chat asked for a file: show it (the request is read when the panel mounts, too).
    {
        let ctx = ctx.clone();
        use_effect(move || {
            if let Some(path) = workspace.file_request(id) {
                workspace.clear_file_request(id);
                ctx.open(path);
            }
        });
    }
    // A tool call of the session finished: tabs with no edits show the file as it is now. A memo, so
    // a streamed reply (which rewrites the view per chunk) does not rerun it.
    {
        let ctx = ctx.clone();
        let finished = use_memo(move || workspace.finished_tool_calls(id));
        let mut seen = use_signal(|| *finished.peek());
        use_effect(move || {
            let now = finished();
            if now != *seen.peek() {
                seen.set(now);
                ctx.refresh_all();
            }
        });
    }
    use_editor_driver(host_id.clone(), &handle, on_event);

    let prefs = workspace.editor_prefs();
    let selected = ctx.tabs.read().selected().map(str::to_string);
    let kind = selected.as_ref().and_then(|path| ctx.kinds.read().get(path).cloned());
    let banner = selected.as_ref().and_then(|path| ctx.docs.read().get(path).and_then(|doc| doc.banner.clone()));
    let tab_views: Vec<TabView> = ctx.tabs.read().tabs().iter().map(|tab| TabView { path: tab.path.clone(), preview: tab.preview }).collect();
    let unavailable = *ctx.unavailable.read();
    let is_text = matches!(kind, Some(TabKind::Text { .. }));
    let markdown_selected = is_text && selected.as_deref().is_some_and(is_markdown);
    let source_on = selected.as_ref().is_some_and(|path| ctx.source.read().contains(path));
    let shows_editor = is_text && !unavailable && (!markdown_selected || source_on);

    let tools = rsx! {
        button {
            class: if prefs.tree { "icon-button active" } else { "icon-button" },
            r#type: "button",
            title: "File tree",
            "aria-label": "File tree",
            "aria-pressed": "{prefs.tree}",
            onclick: move |_| workspace.update_editor_prefs(|prefs| prefs.tree = !prefs.tree),
            "☰"
        }
        if markdown_selected {
            button {
                class: "icon-button files-source-toggle",
                r#type: "button",
                title: if source_on { "Show rendered" } else { "Show source" },
                "aria-label": if source_on { "Show rendered" } else { "Show source" },
                onclick: {
                    let (ctx, path) = (ctx.clone(), selected.clone().unwrap_or_default());
                    move |_| ctx.toggle_source(&path)
                },
                if source_on { "👁" } else { "</>" }
            }
        }
        FilesMenu {}
    };

    let typed = filter();
    let query = parse_files_query(&typed);
    let list = if let FilesQuery::Content(text) = query {
        match hits() {
            _ if text.is_empty() => rsx! { div { class: "dock-note", "Type the text to search for after ?" } },
            None => rsx! { div { class: "dock-note", "Searching…" } },
            Some(found) if found.is_empty() => rsx! { div { class: "dock-note", "No matches" } },
            Some(found) => rsx! {
                ul { class: "files-list",
                    for (index, hit) in found.iter().enumerate() {
                        li { key: "{index}",
                            button {
                                class: "files-hit",
                                r#type: "button",
                                onclick: {
                                    let (path, ctx) = (hit.path.clone(), ctx.clone());
                                    move |_| ctx.open(path.clone())
                                },
                                span { class: "files-hit-path", "{hit.path}:{hit.line}" }
                                span { class: "files-hit-text", "{hit.text}" }
                            }
                        }
                    }
                }
            },
        }
    } else if let Some(message) = problem() {
        rsx! { div { class: "dock-note error", "{message}" } }
    } else if !tree.read().is_loaded("") {
        rsx! { div { class: "dock-note", "Loading…" } }
    } else {
        let rows = tree.read().rows(&typed);
        if rows.is_empty() {
            let note = if typed.trim().is_empty() { "Folder is empty" } else { "No files match" };
            rsx! { div { class: "dock-note", "{note}" } }
        } else {
            let current = selected.clone();
            rsx! {
                ul { class: "files-list",
                    for row in rows {
                        {
                            let indent = format!("padding-left: {}px", 6 + row.depth * 14);
                            let mark = if !row.is_dir { " " } else if row.expanded { "▾" } else { "▸" };
                            let is_selected = current.as_deref() == Some(row.path.as_str());
                            let path = row.path.clone();
                            let (is_dir, cwd, ctx) = (row.is_dir, cwd.clone(), ctx.clone());
                            rsx! {
                                li { key: "{row.path}",
                                    button {
                                        class: if is_selected { "files-row selected" } else { "files-row" },
                                        r#type: "button",
                                        style: "{indent}",
                                        onclick: move |_| {
                                            if is_dir {
                                                if tree.write().toggle(&path) {
                                                    spawn(load_dir(workspace.files(), cwd.clone(), path.clone(), tree, problem));
                                                }
                                            } else {
                                                ctx.open(path.clone());
                                            }
                                        },
                                        span { class: "files-mark", "{mark}" }
                                        span { "{row.name}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    };

    let banner_view = match banner {
        Some(Banner::Conflict) => {
            let (cancel_ctx, overwrite_ctx, key) = (ctx.clone(), ctx.clone(), selected.clone().unwrap_or_default());
            let cancel_key = key.clone();
            rsx! {
                div { class: "files-banner", role: "alert",
                    span { "{Banner::Conflict.message()}" }
                    button { r#type: "button", onclick: move |_| cancel_ctx.reload(cancel_key.clone()), "Cancel" }
                    button { r#type: "button", onclick: move |_| overwrite_ctx.save_from_editor(&key, true), "Overwrite" }
                }
            }
        }
        Some(Banner::SaveFailed(reason)) => {
            let (retry_ctx, dismiss_ctx, key) = (ctx.clone(), ctx.clone(), selected.clone().unwrap_or_default());
            let dismiss_key = key.clone();
            rsx! {
                div { class: "files-banner", role: "alert",
                    span { "Could not save: {reason}" }
                    button { r#type: "button", onclick: move |_| retry_ctx.save_from_editor(&key, false), "Retry" }
                    button { r#type: "button", onclick: move |_| dismiss_ctx.dismiss_banner(&dismiss_key), "Dismiss" }
                }
            }
        }
        None => rsx! {},
    };

    let notice = kind.as_ref().and_then(TabKind::notice);
    let confirm_view = match ctx.confirm.read().clone() {
        Some((path, _)) => {
            let name = path.rsplit('/').next().unwrap_or(&path).to_string();
            let (save, discard, cancel) = (ctx.clone(), ctx.clone(), ctx.clone());
            rsx! {
                div { class: "files-banner", role: "alertdialog",
                    span { "Save changes to {name}?" }
                    button { r#type: "button", onclick: move |_| save.confirm_save(), "Save" }
                    button { r#type: "button", onclick: move |_| discard.confirm_discard(), "Don't save" }
                    button { r#type: "button", onclick: move |_| cancel.confirm_cancel(), "Cancel" }
                }
            }
        }
        None => rsx! {},
    };

    let content = match (&selected, &kind) {
        (None, _) => rsx! { div { class: "dock-note", "Pick a file" } },
        (Some(_), None | Some(TabKind::Loading)) => rsx! { div { class: "dock-note", "Loading…" } },
        (Some(_), Some(TabKind::Failed(message))) => rsx! { div { class: "dock-note error", "{message}" } },
        (Some(path), Some(TabKind::Text { .. })) if unavailable => {
            match ctx.fallback.read().get(path).cloned() {
                Some((html, numbers)) => rsx! {
                    div { class: "dock-note", "The editor could not be loaded, so this file is shown read only." }
                    div { class: "changes-diff",
                        div { class: "file-viewer", "aria-label": "{path}",
                            pre { class: "file-gutter", "{numbers}" }
                            pre { class: "file-code", dangerous_inner_html: "{html}" }
                        }
                    }
                },
                None => rsx! { div { class: "dock-note", "Loading…" } },
            }
        }
        (Some(path), Some(TabKind::Text { .. })) if markdown_selected && !source_on => {
            let html = ctx.markdown.read().get(path).cloned().unwrap_or_default();
            rsx! { div { class: "changes-diff", div { class: "file-viewer file-markdown markdown", "aria-label": "{path}", dangerous_inner_html: "{html}" } } }
        }
        (Some(_), Some(TabKind::Text { .. })) => rsx! {},
        (Some(path), Some(other)) => {
            let (anyway_ctx, reveal_ctx) = (ctx.clone(), ctx.clone());
            let (anyway_path, reveal_path) = (path.clone(), path.clone());
            rsx! {
                FileKindView {
                    kind: other.clone(),
                    path: path.clone(),
                    on_anyway: move |_| anyway_ctx.show_anyway(anyway_path.clone()),
                    on_reveal: move |_| reveal_ctx.reveal(&reveal_path),
                }
            }
        }
    };

    rsx! {
        PanelFrame { id, panel: Panel::Files, tools,
            div { class: "files-filter",
                input {
                    class: "files-filter-input",
                    r#type: "text",
                    placeholder: "Filter, or start with ? to search inside files",
                    value: "{filter}",
                    oninput: move |event| filter.set(event.value()),
                    onmounted: move |mounted| async move {
                        let _ = mounted.set_focus(true).await;
                    },
                }
            }
            div { class: "changes-body",
                if prefs.tree {
                    div { class: "files-side", {list} }
                }
                div { class: "files-main",
                    FilesTabs {
                        tabs: tab_views,
                        selected: selected.clone(),
                        on_select: {
                            let ctx = ctx.clone();
                            move |path: String| ctx.select(&path)
                        },
                        on_close: {
                            let ctx = ctx.clone();
                            move |path: String| ctx.close(path)
                        },
                        on_reorder: {
                            let ctx = ctx.clone();
                            move |(from, to): (String, String)| ctx.reorder(&from, &to)
                        },
                        on_keep: {
                            let ctx = ctx.clone();
                            move |path: String| ctx.keep(&path)
                        },
                        on_close_others: {
                            let ctx = ctx.clone();
                            move |path: String| ctx.close_others(path)
                        },
                        on_close_all: {
                            let ctx = ctx.clone();
                            move |_| ctx.close_all()
                        },
                    }
                    {banner_view}
                    {confirm_view}
                    if let Some(text) = notice {
                        div { class: "dock-note", "{text}" }
                    }
                    div { class: "files-view",
                        div { id: "{host_id}", class: if shows_editor { "files-editor" } else { "files-editor hidden" } }
                        {content}
                    }
                }
            }
        }
    }
}
```

In `crates/forge-widgets/src/lib.rs` add `mod files_state;`.

- [ ] **Step 3: Build and fix compile errors**

Run: `cargo build --workspace 2>&1 | tail -40`
Expected: the first build of the new panel may report type or borrow errors (Dioxus 0.7.10 API details: `use_callback`, `Signal::write().take()`, `EventHandler` closures, `Memo::peek`). Fix each one without changing the behaviour described in `files_state.rs`; keep the public method names, which Task 11 and the spec rely on. Typical fixes: add `mut` to a captured signal, clone a `String` before a `move` closure, import `dioxus::html::input_data::MouseButton`. Repeat until the whole workspace builds with no new warnings about unused items in `files_*` files.

- [ ] **Step 4: Run the whole suite**

Run: `cargo test --workspace 2>&1 | grep -E "^test result|FAILED|error\[" | head`
Expected: no failures. The passing count is at least 968 plus the tests of Tasks 1 to 9 (about 80 more).

- [ ] **Step 5: Commit**

```bash
git add -A crates
git commit -m "feat: the Files panel edits files with tabs, auto save, conflict banners and file kinds

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 11: Check it in the real window, then write it down

**Files:**
- Modify: `docs/superpowers/specs/2026-10-04-files-editor-design.md` (add "As built")
- Modify: `docs/superpowers/roadmap/backlog.md` (PNL-04 row: Plan 25A built)
- Modify: `docs/superpowers/roadmap/README.md` (the plan table: add a 25A row)

**Interfaces:** none. This task proves the behaviour with the harness and records what was found.

- [ ] **Step 1: Start the app on the fixture**

Run: `scripts/harness/launch.sh` (see `scripts/harness/README.md`; stop first with `scripts/harness/stop.sh` if one is running). Then, because the launch script rebuilds the fixture, put the test files in the fixture's session folder (`/private/tmp/forge-harness/my-app`):

```bash
cd /private/tmp/forge-harness/my-app
printf 'for i in range(1, 16):\n    if i %% 15 == 0:\n        print("FizzBuzz")\n' > fizz.py
printf 'a\xffb\n' > latin.txt
printf 'a\0b\n' > blob.bin
python3 - <<'EOF'
import struct, zlib
def png(w, h):
    raw = b"".join(b"\x00" + b"".join(bytes([x * 255 // w, y * 255 // h, 128]) for x in range(w)) for y in range(h))
    def chunk(t, d):
        c = struct.pack(">I", len(d)) + t + d
        return c + struct.pack(">I", zlib.crc32(t + d) & 0xffffffff)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(raw)) + chunk(b"IEND", b"")
open("pic.png", "wb").write(png(64, 64))
open("m10.txt", "w").write(("z" * 79 + "\n") * (10 * 1024 * 1024 // 80))
open("m15.txt", "w").write(("z" * 79 + "\n") * (15 * 1024 * 1024 // 80))
EOF
mkdir -p a b && echo 'print("a")' > a/x.py && echo 'print("b")' > b/x.py
```

Open the fixture session (the harness README says how), then open Files from the header `⋮` menu.

- [ ] **Step 2: Walk through the checks**

Use `python3 scripts/harness/capture.py` and `click.py` after every step, and compare each screen with `docs/superpowers/roadmap/observations.md` Round 9. Use `python3 scripts/observe/claude_app.py` only for the real Claude app. Mark each line as seen or not seen in the "As built" section. The harness cannot type Korean (leave a Hangul word to the user) and its synthetic key events may not reach the page, so for typing use the clipboard route if available, or ask the user to type.

1. Click `fizz.py`: it opens in an italic tab, the code is highlighted, the caret works. Click `hello`-like other files: each replaces the italic tab.
2. Edit `fizz.py` (add a line); the tab turns upright; about 2 s later `cat fizz.py` shows the change; no dirty mark appears.
3. Change `fizz.py` from a shell (`echo x >> fizz.py`) with the editor open and unedited, then run a tool call in the session (or wait): the tab shows the new text without a banner. Then edit in the editor and change the file from the shell: the banner `This file has changed on disk since you started editing.` with `Cancel` and `Overwrite` appears and the disk is not written; `Cancel` shows the disk text; repeat and `Overwrite` writes the editor's text.
4. `⋮` menu: each of the 8 items changes the editor at once and the menu stays open; `Tab size` opens a submenu to the left with 2, 4 (ticked), 8; Escape closes the menu. Restart the app: the settings are remembered.
5. `latin.txt` opens read only with the notice `This file is not valid UTF-8, so it is shown read only.`; typing changes nothing; `cat -v latin.txt` is unchanged.
6. `blob.bin` shows the binary note with `Show in Finder` and `Show as text anyway`; the second shows the text read only with `Shown as text. This file cannot be edited.`; the file is unchanged.
7. `pic.png` shows the picture; `m10.txt` opens; `m15.txt` shows `This file cannot be previewed`.
8. Tabs: open `a/x.py` and `b/x.py` (both normal after `Keep open`); both tabs are named `x.py`, the tooltip shows the full path; closing one leaves the other. Right click a preview tab: `Keep open`, `Close`, `Close others`, `Close all`. Drag a tab with `python3 scripts/observe/claude_app.py drag` style events (use the harness click helper's drag if it has one, else a short Quartz script) and check the order changes and the selection stays.
9. Close with auto save off: turn `Auto save` off, edit `fizz.py`, click its `×`: the row `Save changes to fizz.py?` appears with `Save`, `Don't save`, `Cancel`; try all three (with `Cancel` the tab stays; with `Save` the file changes; with `Don't save` it does not). `⌘S` with auto save off saves the tab.
10. Close with auto save on, less than 2 s after typing: the typed text is on disk after the tab closes.
11. Markdown: `README.md` opens rendered; `</>` shows the source editor; edit, then `👁`: the rendered view shows the edit.
12. Delete the file under an open editor (`rm fizz.py`) and type: a `Could not save: …` banner with `Retry` and `Dismiss` appears and the text stays; recreate the file (`touch fizz.py`), `Retry`: it saves.
13. Dark mode and a Hangul word typed in the editor: not checked by the harness; ask the user to check them and say so in the report.

Fix anything that fails with a small commit each (`fix: …`), then re-check that line.

- [ ] **Step 3: Stop the app**

Run: `scripts/harness/stop.sh`

- [ ] **Step 4: Record what was built**

Append an `## As built (Plan 25A)` section to `docs/superpowers/specs/2026-10-04-files-editor-design.md`, in plain sentences written from what actually happened (look at the earlier sections of this file, such as `## As built (Plan 21B)` in `2026-10-03-right-dock-panels-design.md`, for the style). It must say: that it was built from `docs/superpowers/plans/2026-10-04-files-editor.md`; each way the result differs from the design (including the two additions the plan made, `⌘S` and the inline close question); for each numbered check above, whether it was seen working in the real window or not seen and why; what the user still has to check by hand (a Hangul word typed in the editor, dark mode); the minor things left for later; and the lessons found (for example how the tab drag, the bundle in the head and the IME behaved).

In `docs/superpowers/roadmap/backlog.md` change the `Plan 25A (planned: editor, tabs), 25B, 25C` part of the PNL-04 `Plan` cell to `Plan 21B (built); Plan 25A (built: editor, tabs); 25B and 25C planned`. In `docs/superpowers/roadmap/README.md` add a row to the plan table after the 24 row: `| 25A | Files panel as an editor (built) | PNL-04 | CodeMirror bundle, auto save and conflict banner, tabs; 25B line comments and 25C search next |`.

- [ ] **Step 5: Run everything once more and commit**

Run: `cargo test --workspace 2>&1 | grep -E "^test result|FAILED" | head` and `cd sidecar && npm test 2>&1 | tail -4; cd ..`
Expected: no failures; the sidecar suite still has its 102 tests.

```bash
git add -A docs crates
git commit -m "docs: Plan 25A as built, the Files panel editor

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```
