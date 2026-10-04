# Files panel as an editor (Plan 25A)

The Files panel (Plan 21B) is a read only viewer. The real Claude desktop app edits files in it, with tabs, auto save and a conflict banner. Observation round 9 (`docs/superpowers/roadmap/observations.md`) recorded how it behaves. This spec is the first of three pieces:

- **A (this spec):** the editor, auto save and conflict banner, the `⋮` settings, file kinds, tabs.
- **B (later):** line selection bar, `컨텍스트로 첨부` and `댓글 추가`, the cards in the message box and the chat, the editor's right click menu.
- **C (later):** the `?` search toggles and the in-file find bar, the tree and tab right click items that need B (attach, copy, reveal, open in).

## Scope, decided with the user on 2026-10-04

- Real editing in the Files panel: a CodeMirror 6 editor in the webview (approach 1 of two; the other, a textarea under a highlighted layer, was rejected because line selection and a comment box between lines, needed by B, cannot be built on it).
- Tabs are in A: italic preview tab, keep open, close four ways, drag to reorder. The right click items that need B or C are not.
- Closing a tab with unsaved changes, with auto save off, asks "Save / Don't save / Cancel" (option a).
- Settings are kept for the whole of Forge, not per session.
- A tab that has no edits is refreshed quietly when a tool call of the session finishes (the real app kept showing the old text until the next edit; this is a chosen difference).

## What was seen (round 9)

- Typing changes the file on disk about 2 s later; the tab shows no dirty mark and no "saved" text.
- If the file changed on disk and the next edit is made, a grey banner appears over the editor: "편집을 시작한 이후 디스크에서 이 파일이 변경되었습니다." with `취소` and `재정의`. Auto save is paused meanwhile. `취소` shows the disk text again and drops the edits.
- `⋮` menu: `파일 트리 표시` (⇧⌘Y), `미리보기 탭`, `자동 저장`, `Tab 키로 들여쓰기`, `자동 줄 바꿈`, `들여쓰기 가이드`, `공백 표시` (a dot per space), `끝 이후까지 스크롤`, `탭 크기` 2 / 4 / 8. Defaults: all on except `공백 표시` and `끝 이후까지 스크롤`; tab size 4. A tick click keeps the menu open and acts at once.
- Tabs: a file opens in an italic tab that the next file replaces; `열어 두기` makes it a normal tab; close, close others, close all; a drag reorders; each tab has its own `×`; the selection stays on the tab when another is dragged.
- File kinds: markdown opens rendered with a `</>` / eye switch; a binary file shows "이 파일은 바이너리 데이터로 보이며 텍스트로 미리 볼 수 없습니다." with `Finder에서 보기` and `그래도 텍스트로 보기`; an image is drawn centred at its real size; a text file of 10 MiB opens, one of 15 MiB shows "이 파일은 미리 보기를 사용할 수 없습니다 / 여기서 미리 보기를 하기에는 파일이 너무 큽니다." and its path.

## Units

### `editor.js` (in `crates/forge-shared/assets`)

CodeMirror 6, bundled once with esbuild from `tools/editor-bundle/` (a `package.json`, a source file and a README with the build command; `node_modules` is ignored). Only the built `editor.js` and its licence file (`EDITOR-LICENSE`, MIT) are committed, so building Forge needs no Node. `crates/forge-shared/src/editor.rs` has `editor_head()` that returns `<script>…</script>` for the window head, as `xterm_head()` does for the terminal.

Global `ForgeEditor.create(host, options)` returns a handle with `open(doc, language)`, `setSettings(settings)`, `getDoc()`, `focus()`, `destroy()`. The page reports to Rust through the channel of `document::eval`:

- `{t: "ready"}` once the editor is built;
- `{t: "dirty"}` on the first change after an `open` or a save;
- `{t: "change", s: <text>}` 2 s after the last change (the page debounces; the text is sent once per pause, not per key);
- `{t: "flush", s: <text>}` when Rust asks for the text at once (a close, a switch of tab).

Rust sends `{t: "open", doc, language, settings}`, `{t: "settings", settings}` and `{t: "flush?"}`. While an IME composition is open (`compositionstart` until `compositionend`) the page sends nothing, and an `open` waits for the end of the composition.

Features turned on by settings: line numbers, fold gutter, history, indent guides, whitespace dots, line wrapping, scroll past end, tab size, indent with Tab (`indentWithTab`). Languages: JavaScript and TypeScript, Python, Rust, JSON, Markdown, HTML, CSS, YAML, and a shell mode; any other extension is plain text. The theme follows Forge's light and dark theme tokens.

### Port: `FilesPort` (in `forge-ports`)

```rust
pub struct FileVersion { pub modified_ns: u128, pub len: u64 }

pub enum FileContent {
    Text { text: String, version: FileVersion },
    Image { bytes: Vec<u8>, mime: String },
    Binary,
    TooLarge,
}
```

- `read(root, path)` returns the new `FileContent`. `READ_LIMIT` becomes 10 MiB. A file with an image extension (`png jpg jpeg gif webp bmp ico svg`) is read as bytes up to the limit.
- `read_text_anyway(root, path)`: the bytes as text (invalid UTF-8 replaced), for `그래도 텍스트로 보기`, at the same size limit.
- `stat(root, path) -> Result<FileVersion, FilesError>`.
- `write(root, path, text, expected: Option<FileVersion>) -> Result<FileVersion, WriteError>`: with `Some(version)` it first compares the file's current version and returns `WriteError::Changed` when it differs; with `None` it overwrites (`재정의`). It writes a temporary file in the same folder and renames it over the target, so a crash leaves the old or the new file whole. It keeps the file's permissions. The path check is the existing `resolve` (inside the session folder, symlinks included).

`WriteError` is `Changed`, `Outside` or `Failed(String)`.

### `EditorSettings` (in `forge-domain`, inside `AppConfig`)

`EditorPrefs { tree: bool, preview_tabs: bool, auto_save: bool, tab_indent: bool, wrap: bool, indent_guides: bool, whitespace: bool, scroll_past_end: bool, tab_size: u8 }`, `#[serde(default)]`, defaults as seen. `tab_size` outside 2, 4, 8 reads as 4. It is saved through the existing `ConfigRepositoryPort` (as `SidebarPrefs` is), and read by the panel through the workspace.

### `EditorTabs` (in `forge-features`, a pure model)

State: an ordered list of tabs `{ path, preview: bool }`, the selected path.

- `open(path, preview_tabs)`: an open path is selected. A new path becomes a preview tab and replaces the existing preview tab (kept in the same place); with `preview_tabs` off it is a normal tab.
- `keep(path)`: a preview tab becomes normal. `edited(path)` does the same (the first edit of a preview tab).
- `close(path)`, `close_others(path)`, `close_all()`: the selection moves to the neighbour on the right, then the left; none left means no selection.
- `reorder(from, to)`: moves a tab; the selected path does not change.
- Closing returns the paths that have unsaved edits so the caller can ask.

### The panel (in `forge-widgets/src/files_panel.rs`, split into small files)

The panel keeps what it has (tree, filter, `?` search, the markdown render with the source switch) and replaces the viewer with:

- a tab strip (italic for a preview tab, `×`, drag to reorder, the selected tab as a grey pill; the `⋮`-menu check and the tab right click menu `열어 두기`, `파일 닫기`, `다른 파일 닫기`, `모든 파일 닫기` only);
- `EditorHost` (the editor in one element; one editor instance serves the selected tab and keeps a per tab undo history and scroll position in the page);
- `EditorBanner` (conflict, save failed);
- `FileKindView` (image, binary, too large, loading, error);
- the `⋮` menu with the 8 settings and the tab size submenu (opens to the left when it would leave the window).

## Data flow

1. **Open.** A tree click or a chat file link calls `EditorTabs::open`. The panel reads the file (`FilesPort::read`) and shows the result by kind. A `Text` result is remembered per tab: `loaded` (the version it was read at) and `dirty`.
2. **Edit.** On `dirty` the tab becomes a normal tab and the panel calls `stat`; a version different from `loaded` raises the conflict banner and pauses auto save. On `change` (and with `auto_save` on) the panel calls `write(.., Some(loaded))`; on success `loaded` takes the new version and `dirty` clears; `Changed` raises the banner; `Failed` raises the save failed banner (`<reason>  다시 시도 | 닫기`).
3. **Conflict banner.** `재정의` calls `write(.., None)`. `취소` re-reads the file, sends `open` to the page and clears `dirty`.
4. **Tool call finished.** The panel re-reads each open tab with no edits and sends `open` if the text changed (the cursor and scroll stay when the line still exists, else the top). A tab with edits is left; the banner rule above applies at its next save.
5. **Close.** For a tab with edits and `auto_save` on, the panel asks the page for `flush`, writes, and closes; a conflict or a failure keeps the tab open with its banner. With `auto_save` off a dialog asks `저장 / 저장 안 함 / 취소`. Close others and close all ask once per tab with edits, in order, and stop at a Cancel.
6. **Settings.** A change in the `⋮` menu updates `EditorPrefs`, saves the config and sends `settings` to the page. `tree` shows or hides the file tree. `preview_tabs` off makes new tabs normal ones.

## Errors

- Save failure, a deleted file, permissions: the banner above; the text stays in the editor; auto save pauses until `다시 시도` succeeds.
- Read failure or a path outside the session folder: the existing red note in the panel.
- If the bundle did not load (`ForgeEditor` is undefined) the panel falls back to the old read only viewer (`code_html`) and says nothing is editable.
- A file written by Claude or another program while the editor is clean is shown without any banner (step 4).

## Tests and checks

- Unit: `EditorTabs` (every rule above), `EditorPrefs` (defaults, an unknown tab size, a config written without the field), the version compare, `FileContent` kinds (text, image by extension, NUL byte, over the limit), the adapter's `write` (the expected version matches or not, `None` overwrites, a path outside the folder, a symlink out of the folder, the temporary file is gone, permissions kept), `read_text_anyway`.
- Bundle contract: `editor_head()` has exactly one `<script>` and one `</script>`, and mentions `ForgeEditor`.
- Checked in the real window with the harness: typing reaches the disk in about 2 s; the conflict banner with both buttons; a Hangul word typed in the editor; the 8 settings and the tab sizes; the italic tab, keep open, the four closes, dragging a tab; a binary file, an image, a file over the limit; the close dialog with auto save off. Compared side by side with the real app, using the same files as round 9 (`fizz.py`, `README.md`, a binary, a PNG, a 10 MiB and a 15 MiB text file).

## Not built here (to the backlog)

The line selection bar, cards and comments (B); the `?` search toggles and the in-file find bar, the tree, tab and editor right click items that attach, copy, reveal or open elsewhere (B and C); the shortcuts ⇧⌘F and ⇧⌘Y (the shortcut table cannot take Shift); creating, renaming or deleting files; what the real app does when the file is deleted under an open editor (not seen); the `자동 저장 켜짐` one time tip (its trigger was not seen).

## Where the code goes

`tools/editor-bundle/`, `crates/forge-shared/assets/editor.js` and `EDITOR-LICENSE`, `crates/forge-shared/src/editor.rs`, `crates/forge-ports/src/files_port.rs`, `crates/forge-adapter-files/src/lib.rs`, `crates/forge-domain/src/config.rs`, `crates/forge-features/src/editor_tabs.rs`, `crates/forge-widgets/src/files_panel.rs` and new `files_editor.rs`, `files_tabs.rs`, `files_kinds.rs`, `files_menu.rs`, `crates/forge-shared/src/theme.css`.
