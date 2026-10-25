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

## Answers to the builder check (my calls)

- **Root**: the session's own folder (`project_path`: the scratch folder for a no-folder session, the worktree path for a worktree session). Never the home directory. A session with no usable folder, or an empty one, shows one grey row `이 폴더에 파일이 없습니다` (`tr!`).
- **Cap vs newest-first**: the walk visits at most 20,000 entries, collects the files it meets, then keeps the 5,000 newest by modified time; past 20,000 visited entries the list is marked cut and the popup ends with a grey row `일부만 표시합니다` (reuse the existing string `Only part of the file is shown.`? no: add `Only some files are listed.`).
- **Sent cards and reopened sessions**: a message already carries `attachments` (path and kind; the bubble draws them as chips today via `attachment_label`). Draw cards from that data; whatever the transcript loader keeps today is what a reopened session shows (do not extend the loader in this plan); a card whose file is gone or unreadable shows without a thumbnail.
- **Already attached**: compare the absolute path (root joined with the relative path, lexical, no canonicalisation) with the attachments' paths; a file attached from outside the folder does not tick any row.
- **When the walk runs**: the first `@` of a session starts it (no prefetch); a refresh (focus after a turn, or 30 s) keeps showing the old list until the new one arrives; a walk already in flight is not started twice (same ticket idea as the Office tab refresh).
- A new `FilesPort` method also needs the mock/fake implementations that already implement the trait (find them with the compiler).

## Decisions I made (not the person's)

- Files only, from the session folder; no sessions, no windows.
- Newest-modified-first order; 5,000 files / depth 8 caps.
- Picking adds an attachment card (the same thing the `+` menu does) rather than inserting a path into the text.
- Thumbnails up to 1 MB only.
- Plan 58B (queue card, slash list, `/compact` result, `/btw`, `/rewind`) is a separate plan.

## Left to the builder

CSS, the exact popup placement, how the cursor position is read from the textarea, tests (pure helpers first), Korean strings (`tr!`, `ko.rs` kept sorted: `ko.rs` must stay sorted by English key or lookups silently fall back to English).

## As built

Pure helpers (`forge_entities::mention`, tested): `type_tag` (upper-cased extension, `FILE` when none, at most 5 characters), `shorten_middle` (keeps the start and the extension), `mention_query(text, cursor)` (the `@` at the start or after whitespace, no whitespace in the query, returns the range to replace), `rank_mentions(files, query, attached)` (case-insensitive substring on the path, file-name matches first, then list order, at most 50 rows).

Files source: `FilesPort::list_files(root, limit) -> FileList { files, cut }` (forge-ports `files_port.rs`). The adapter `walk` (forge-adapter-files) is iterative with an explicit stack. Caps: 20,000 entries visited (skipped entries and read errors count), depth 8, 5,000 files kept (newest modified first, ties by path, an unreadable time sorts last). Hidden entries (a leading `.`), `node_modules`, `target` and `__pycache__` are left out. Symlinks are neither followed nor listed. A name that is not UTF-8 is skipped. An unreadable subfolder is skipped; only the root must be readable. Hitting the visit cap with entries left sets `cut`.

Cards: `AttachmentCard` (forge-entities `attachment_card.rs`) is presentational and takes `thumbnail: Option<String>`. Thumbnails load in `forge-features/src/thumbnail.rs` (`thumbnail_data_url`, images up to 1 MB, read through `run_blocking`). The composer cards use `ComposerAttachment`. The sent bubble reads a `ThumbnailLoader` context defined in entities and provided by the workspace provider (`provide_thumbnail_loader`). Cards are keyed by path. Folder chips stay chips.

Mention list: `FileListStore` (`file_lists.rs`) is a ticketed per-session cache; `Workspace::refresh_session_files` begins a walk only when the store says one is needed, keeps the old list visible meanwhile, and a walk that finishes after the session closed or was superseded writes nothing. `mention_popup.rs` is a pure state machine (`MentionPopup`, `mention_rows`, `MentionStatus`) with `key_decision` (while the list is open Enter picks and does not send; composing IME keys are left alone). `absolute_in` joins the root and a relative path and is safe by construction (it refuses `..` and absolute parts); `attached_in` compares absolute paths lexically. The widget is `MentionList` (forge-widgets `mention_list.rs`). The cursor is read from the textarea through `document::eval`; `provisional_cursor` guesses the new cursor at once from the old text, old cursor and new text so the list does not flicker, and a latest-read counter drops a read that was overtaken. No read is made when the draft has no `@`. A pick adds an attachment card and removes the text from the `@` to the cursor, leaving no double space.

Where it differs from the spec:
- The refresh starts only when an `@` is typed. It is not started on composer focus, and not after a turn ends or after 30 s unless an `@` is typed (the store still decides a walk is needed by age 30 s or a new turn mark).
- The grey row `Only some files are listed.` is hidden while a refresh runs.
- The new-session screen has no mention list.
- A pick removes the text from `@` to the cursor (not only the query) and leaves no double space.

## Checked in the real window

(to be added by the controller after the real-window check)

## Left

- Session and window mentions, folder mentions.
- Drag and drop of files onto the composer.
- Thumbnails for pdf and docx.
- `Loading(old)` (a refresh over an old list) hides the cut row.
- No mention list on the new-session screen.
- No refresh on composer focus.
- Mouse-click cursor moves are not tracked (the list follows the keyboard and typing only).
- `provisional_cursor` is wrong on backspace inside a run of equal characters until the page read corrects it.
- An external draft change whose text equals the stale snapshot can reopen the list.
- IME Enter commit was not checked in a window.
- The non-UTF-8 name skip and the None-mtime-last order are untested on macOS.
- The unreadable-folder walk test fails when run as root.
- `@@` and `@a@b` open the list with a query that contains `@`.
- Plan 58B: queue card, slash list, `/compact` result, `/btw`, `/rewind`.
