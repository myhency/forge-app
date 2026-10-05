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

## As built (2026-10-05)

Built on branch `plan-25D` (from `master`), executed inline. 1097 Rust tests and 37 jsdom bundle tests pass. As designed; the session's closing also drops its unsent cards (a leak 25B left).

**Checked in the real window (2026-10-05, harness, auto save turned off from the Files `⋮` menu).** A typed character (Korean input method: `ㅏ` after `alpha`) in `b.txt` with the disk text unchanged: closing the Files panel with `×` and opening it again from the session `⋮` menu brings the tab back (upright, a kept tab) with the typed text; the same after switching to another session and back; with the file changed from a shell meanwhile, the reopened tab keeps the typed text and shows the conflict banner (`Cancel`, `Overwrite`); `Cancel` drops the text and its draft (the disk text shows, and shows again after closing and reopening the panel); with the file deleted from a shell the reopened tab shows "No such file or directory (os error 2)" and, after the file is created again and the panel reopened, the draft comes back with the conflict banner.

Not checked in the real window: expanding another panel and returning (the same unmount and mount as closing the panel), a markdown file shown as source across a reopen, auto save on with a stale draft (covered by the store test of note then clear), several tabs with drafts at once, a CRLF file, closing a tab with `×` and `Don't save`.

**Fixed after the whole-branch review (store rules with tests that failed first; the close question checked in the real window):** a draft typed while the tab showed the conflict banner kept its conflict only until the panel was reopened a second time (the base version moved to the fresh one, the banner was not shown, and auto save then wrote over the external change): `Draft` has a sticky `conflict` flag and the editor's conflict state is passed with every note; `restore` also compares the content hash of the disk text (a change that kept the version, for example `cp -p`, is a conflict) and treats a draft equal to the disk text as no draft (a write that finished on disk while the panel went away no longer shows a false conflict banner); a successful write clears only the draft of the text that was written (`FilesStore::written`), text typed while the write ran stays against the new version; closing a restored tab the page does not hold yet (`×`, close others, close all) or a markdown tab shown rendered used to drop its draft silently: the page's "no state" answer falls back to the draft text, so the close question appears (checked: "Save changes to a.txt?" with the draft shown); `note_draft` no longer recreates the store of a session that was closed.

Deferred minors: closing a session silently discards its drafts (a product decision: no question is asked); a restored draft is written 2 s after the panel returns when auto save is on (also for text drafted with auto save off earlier); each `Change` and `Flush` hashes the whole text and copies it when it differs (10 MiB files, many tabs); a draft for a tab that turned read only on disk stays in the store unseen; a page `Change` arriving between the banner's Cancel and its read can re-note the dropped text (the blur makes it very unlikely); a `Failed` tab with a draft cannot save through the close question (nothing is lost, the tab stays).
