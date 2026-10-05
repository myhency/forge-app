# Keeping the Files panel's tabs and unsaved edits (Plan 25D)

25A left a known problem: the Files panel keeps its state in the panel, so closing the panel, switching to another session or expanding another panel drops the open tabs, and an edit made with auto save off is lost without a question. Branch `plan-25D` (from `master`, which has 25A to 25C-2).

## Why tabs alone are not enough

The text of an open tab lives in the editor page (CodeMirror), which is built with the panel and destroyed with it. Rust only knows a `TabDoc` per tab (file version, hash of the disk text, banner). The page already sends `change` (2 s after an edit and when the editor loses focus), `save` and `flush` with the full text whether or not auto save is on.

## Decision (with the user, 2026-10-05)

Approach A: mirror the state into `Workspace` per session and keep unsaved text as a draft. Approach B (keeping one editor page alive per session) was rejected: it needs several pages and a different mounting scheme for the dock.

## Store (forge-features, Workspace)

- `FilesStore { tabs: EditorTabs, source: HashSet<String>, drafts: HashMap<String, Draft> }` per `LiveSessionId`, held in `Workspace` (like `cards`); dropped when the session closes.
- `Draft { text: String, base: FileVersion }`: the editor's text when it differs from disk, and the disk version the text was made against.
- Pure rules (tested): `FilesStore::note_text(path, text, disk_hash, disk_version)` sets the draft when `text` differs from the disk text (by hash) and clears it when they match; `restore_plan(path, fresh_version) -> Restore` is `FromDisk` (no draft), `Draft` (draft and `base == fresh_version`), `DraftWithConflict` (draft and the disk changed since); closing a tab drops its draft.

## Panel (forge-widgets)

- The panel keeps its signals but starts from the store and writes back: when `tabs` or `source` change it copies them into the store (an effect); the `change`, `save`, `flush` and `missing` handlers call `note_text`; a tab closed for good, discarded (`Don't save`, the banner's Cancel) or written to disk clears its draft.
- On mount, for each tab in the store (same order, same selection, same preview flags) the panel reads the file (`read_and_apply`). When the read finishes and the tab has a draft: `Draft` loads the draft text into the editor as an edited tab (the `TabDoc` is built from the disk text, so it differs and auto save may write it again); `DraftWithConflict` does the same and raises the conflict banner; a draft for a file that cannot be read any more is kept and the tab shows the read error (nothing is dropped silently).
- The editor page gets a new load option `dirty: true` (text differs from disk: the page sends `dirty` so the existing first-edit logic runs once). Auto save, banners, close question work as before.

## What does not come back

Undo history and scroll position of a restored tab; an edit typed less than 2 s before the editor was destroyed and not flushed by a blur (the click that closes the panel or switches the session blurs the editor first).

## Tests

Store rules (set, clear, same text, base version equal / different, close drops the draft, session drop); a jsdom test for `load(..., dirty)` (sends `dirty` once, keeps the text); then a real window check: type with auto save off, close and reopen the panel, switch session and back, expand another panel; a tab with a draft for a file changed on disk shows the conflict banner.

## Out of scope

Keeping tabs across an app restart; undo history; scroll position.
