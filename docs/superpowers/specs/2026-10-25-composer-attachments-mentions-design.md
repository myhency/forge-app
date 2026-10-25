# Composer: attachment cards and the `@` file mention (Plan 58A)

Date: 2026-10-25. What the desktop app does is in `docs/superpowers/roadmap/observations.md`, Round 28 (attached image, `@` list) and Round 30 (docx/xlsx attached, `@` list). Backlog rows `NEW-03` (the `@` half) and `NEW-11`. The queue card, the slash list, `/compact`, `/btw` and `/rewind` are Plan 58B.

The person asked for no questions; the choices below are my defaults.

## Why

Office staff hand Claude files: a draft `.docx`, a sheet, a picture. In the desktop app an attached file is a card with its name and a type tag (`XLSX`, `DOCX`), an image is a thumbnail, and typing `@` lists the folder's files so the person can point at one without the file dialog. Forge shows a small chip `📄 name ×` (no type, no thumbnail) and typing `@` does nothing, so a file already in the working folder takes a trip through the system dialog.

## Goals

1. An attached file is a **card** in the composer: the file name (up to three lines, long names cut in the middle with `…`), a small type tag with the upper-cased extension (`XLSX`, `DOCX`, `PPTX`, `PDF`, `CSV`, `TXT`, `PNG`; no extension → `FILE`), and a remove button `×`. An attached image shows a **thumbnail** instead of the name block (the name stays under it).
2. After sending, the user's message shows the same cards above the bubble (name and tag; image thumbnails), not chips under it.
3. Typing `@` (at the start of the text or after whitespace) opens a **mention list** above the composer with the files of the session's folder, filtered as the person types (`@보고` → files whose path contains `보고`, case-insensitive). `↑`/`↓` move, `Enter` or `Tab` pick, `Esc` closes, a click picks. Picking a file turns it into an attachment card and removes the `@…` text; a file that is already attached is shown ticked and picking it does nothing.
4. Each list row shows a type icon (the Files panel's icon for the extension), the file name, and the folder part in grey; at most 8 rows are visible, the list scrolls.

## Non-goals and anti-goals

- **Sessions and other apps' windows in the list**: the desktop lists them; Forge does not. Windows of other apps expose private titles (a strict-policy work PC would object) and a session mention has no behaviour defined yet. Not built.
- **Folders in the list**: files only.
- **Reading file contents**: unchanged. Attachments are paths the model reads with its own tools; only images are sent as image data (existing rules and the 5 MB limit).
- Anti-goal: the list never leaves the session folder (no `..`, no absolute paths, no symlinks followed) and never shows hidden entries, `.git`, `node_modules`, `target`, `__pycache__`.
- Anti-goal: a huge folder must not freeze the composer: the walk is capped (below) and runs off the UI thread; typing `@` shows `불러오는 중…` until the walk is done.
- Anti-goal: `@` in the middle of a word or an e-mail address (`a@b.com`) does not open the list.

## The design

### Cards (`composer_state.rs` `ComposerChips`, `forge-entities/src/turn.rs`)

- Pure helpers (tested): `type_tag(path) -> String` (extension upper-cased, `FILE` when none, at most 5 characters), `shorten_middle(name, max_chars) -> String` (keeps the start and the extension: `분기_영업_실적_보고서_최종.docx` → `분기_영업_실…종.docx`).
- Folder chips (added folders) stay chips as today.
- Thumbnails: for `AttachmentKind::Image` read the file off the UI thread, downscale is not needed: use a `data:` URL only for files up to 1 MB, otherwise show the card without a thumbnail (tag `PNG`). Failure to read → no thumbnail.
- In the chat bubble the cards are read-only (no ×).

### Mention list

- A pure parser `mention_query(text, cursor) -> Option<(start, query)>`: the `@` must be at the start or follow whitespace and the query (up to the cursor) has no whitespace; returns the text range to replace.
- A files source: a new `FilesPort` method `list_files(root, limit) -> Result<Vec<String>, FilesError>` returning relative paths of files (not folders), depth-first from the root, skipping the entries above and not following symlinks, at most 5,000 files and depth 8, sorted by modified time newest first (a person usually means a recent file). The Workspace caches the list per session and refreshes it when the composer gets focus after a turn ended or after 30 s; the cache is dropped when the session closes.
- A pure ranker `rank_mentions(files, query, attached) -> Vec<MentionRow>`: case-insensitive substring match on the path; rows whose file name (not folder) matches come first, then by the list's order; at most 50 rows.
- The widget `MentionList` is a popup anchored above the composer (like the existing slash/`+` popups: look at how `composer_controls.rs` / the plus menu position and close themselves); keyboard handling goes through the composer's existing key handler, and while the list is open `Enter` must NOT send the message.

## Decisions I made (not the person's)

- Files only, from the session folder; no sessions, no windows.
- Newest-modified-first order; 5,000 files / depth 8 caps.
- Picking adds an attachment card (the same thing the `+` menu does) rather than inserting a path into the text.
- Thumbnails up to 1 MB only.
- Plan 58B (queue card, slash list, `/compact` result, `/btw`, `/rewind`) is a separate plan.

## Left to the builder

CSS, the exact popup placement, how the cursor position is read from the textarea, tests (pure helpers first), Korean strings (`tr!`, `ko.rs` kept sorted: `ko.rs` must stay sorted by English key or lookups silently fall back to English).

## Left (not in 58A)

Session and window mentions, folder mentions, mentioning with a range or a line, drag-and-drop of files onto the composer (check whether it exists), thumbnails for pdf/docx, queue card and the rest of Plan 58.
