# Composer: attachment cards and the `@` file mention (Plan 58A) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Attached files are cards with a type tag (images get a thumbnail), the sent message shows the same cards, and typing `@` lists the session folder's files so a pick attaches one.

**Architecture:** Pure helpers first (Task 1), the folder walk in the files port and adapter (Task 2), the cards (Task 3), the mention list and its Workspace plumbing (Task 4), then samples, docs and tests (Task 5).

**Tech Stack:** Rust, Dioxus. No new dependencies.

**Spec:** `docs/superpowers/specs/2026-10-25-composer-attachments-mentions-design.md` (read it and its "Answers to the builder check").

## Global Constraints

- Korean UI text goes through `tr!("English")` with an entry in `crates/forge-shared/src/ko.rs`, **sorted by English key (byte order)** (an unsorted entry silently falls back to English; `cargo test -p forge-shared` catches it). New strings: `이 폴더에 파일이 없습니다` (key `No files in this folder`), `불러오는 중…` (key `Loading…`; check it may already exist), `일부만 표시합니다` for the file list (key `Only some files are listed.`).
- The mention list never leaves the session folder: no `..`, no absolute paths, symlinks are not followed, hidden entries and `.git`, `node_modules`, `target`, `__pycache__` are skipped. Never list the home directory: the root is the session's `project_path`.
- Caps: the walk visits at most 20,000 entries, depth ≤ 8, keeps the 5,000 newest files; at most 50 rows are ranked; thumbnails only for image files ≤ 1 MB. Reads and walks run off the UI thread; a walk in flight is not started twice.
- Attachments stay paths the model reads itself; only images are sent as image data (existing rules). No new sending behaviour.
- Match surrounding comment density and naming. Commit after each task (`feat:`/`fix:`/`docs:`); run focused crate tests per task; the whole workspace once in Task 5 (the `forge-adapter-claude-plugins` round_trip test is a known machine-state failure, unrelated).

## Review Focus

- `@` in the middle of a word or an e-mail address (`a@b.com`, `foo@bar`) never opens the list; `@` at the start of text or after whitespace does.
- While the list is open `Enter` picks and never sends the message; with the list closed `Enter` still sends.
- A symlink loop or a huge folder cannot hang or exhaust memory; the list never contains a path outside the folder.
- A file attached from outside the folder never ticks a row; an already attached file is not attached twice.
- A card for a file that no longer exists or cannot be read shows without a thumbnail and does not break the composer.

---

### Task 1: Pure helpers

**Files:**
- Create: `crates/forge-entities/src/mention.rs`; export from `crates/forge-entities/src/lib.rs`.

**Interfaces:**
- Produces: `pub fn type_tag(path: &str) -> String` (extension upper-cased, `FILE` when none or empty, at most 5 characters, any case; `a.tar.gz` → `GZ`; a leading-dot name like `.env` → `FILE`); `pub fn shorten_middle(name: &str, max_chars: usize) -> String` (names within the limit unchanged; otherwise keeps the start and the extension with `…` between: `분기_영업_실적_보고서_최종.docx` at 16 → `분기_영업_실…종.docx` style; counts characters not bytes; never panics on multi-byte text or `max_chars` smaller than the extension); `pub fn mention_query(text: &str, cursor: usize) -> Option<MentionRange>` with `MentionRange { start: usize, end: usize, query: String }` (byte offsets of the `@…` span up to the cursor; the `@` must be at index 0 or follow whitespace; the query has no whitespace; `cursor` is a byte offset on a char boundary, else `None`); `pub struct MentionRow { pub path: String, pub name: String, pub folder: String, pub attached: bool }`; `pub fn rank_mentions(files: &[String], query: &str, attached: &dyn Fn(&str) -> bool) -> Vec<MentionRow>` (case-insensitive substring match on the path; rows whose file NAME matches come first, each group in the input order; at most 50; `folder` is the path without the name).

- [ ] **Step 1: Write failing tests**: `type_tag` (`a.xlsx`→`XLSX`, `A.DocX`→`DOCX`, `noext`→`FILE`, `.env`→`FILE`, `a.tar.gz`→`GZ`, `x.verylongext`→ first 5 chars `VERYL`), `shorten_middle` (short unchanged; long Korean name; extension kept; `max_chars` 3; empty string), `mention_query` (`@`, `@보`, `hello @보고`, `a@b.com`→None, `foo@bar`→None, `@a b` with cursor after `b`→None, cursor in the middle, multi-byte cursor off boundary→None), `rank_mentions` (name matches first, case-insensitive, Korean, limit 50, attached flag from the closure, empty query lists the first 50).
- [ ] **Step 2: Run** `cargo test -p forge-entities` — fail.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test -p forge-entities` — pass.
- [ ] **Step 5: Commit** `feat: pure helpers for attachment cards and the @ mention`.

### Task 2: The folder walk in the files port

**Files:**
- Modify: `crates/forge-ports/src/files_port.rs`, `crates/forge-adapter-files/src/lib.rs`, every other implementer of `FilesPort` the compiler reports (mocks/fakes).

**Interfaces:**
- Produces: `fn list_files(&self, root: &Path, limit: usize) -> Result<FileList, FilesError>` on `FilesPort`; `pub struct FileList { pub files: Vec<String>, pub cut: bool }` (relative paths with `/` separators; `cut` true when the 20,000-entry visit cap was hit). Rules in the adapter: files only; depth ≤ 8; skip names starting with `.` and `node_modules`, `target`, `__pycache__`; do not follow symlinks (a symlink entry is skipped whether it points to a file or a folder); visit at most 20,000 entries; sort by modified time newest first (ties by path) and keep `limit` files (the caller passes 5,000); an unreadable subfolder is skipped, not an error; an unreadable or missing root is `FilesError`.

- [ ] **Step 1: Write failing tests** (tempdir fixtures; unix-only symlink tests `#[cfg(unix)]`): `files_are_relative_with_forward_slashes`, `hidden_entries_and_the_skipped_folders_are_left_out`, `symlinks_are_not_followed_or_listed` (a symlink to a folder, to a file, and a loop), `depth_over_8_is_not_walked`, `the_newest_files_come_first`, `limit_keeps_the_newest`, `more_than_20000_visited_entries_marks_the_list_cut` (use a small injected visit cap through an internal function `walk(root, limit, visit_cap, depth_cap)` so the test does not create 20,000 files), `an_unreadable_folder_is_skipped`, `a_missing_root_is_an_error`.
- [ ] **Step 2: Run** `cargo test -p forge-adapter-files -p forge-ports` — fail.
- [ ] **Step 3: Implement** the trait method, the adapter walk (iterative, no recursion), and fix every implementer so `cargo test --workspace --no-run` builds.
- [ ] **Step 4: Run** `cargo test -p forge-adapter-files -p forge-ports` and `cargo test --workspace --no-run` — pass.
- [ ] **Step 5: Commit** `feat: list the files of a folder for the @ mention`.

### Task 3: Attachment cards in the composer and in the sent message

**Files:**
- Modify: `crates/forge-features/src/composer_state.rs` (`ComposerChips`), `crates/forge-entities/src/turn.rs` (the bubble's attachments), `crates/forge-shared/src/theme.css`, `crates/forge-shared/src/ko.rs` if a string is needed; a small helper for thumbnails (where the Files panel loads image bytes and builds `data:` URLs is the pattern: `files_kinds.rs` `kind_of`).

**Interfaces:**
- Consumes: Task 1's `type_tag`, `shorten_middle`; `Attachment`/`AttachmentKind`.
- Produces: `AttachmentCard(attachment, removable: bool, thumbnail: Option<String>)`-style component(s) used by the composer (removable, with the existing remove action) and by the bubble (read only); a card shows the name (≤ 3 lines, `shorten_middle` to about 24 characters per line budget), a tag `type_tag`, and for images a thumbnail `img` from a `data:` URL when the file is ≤ 1 MB (read off the UI thread, e.g. `use_resource`/`run_blocking` as neighbours do; failure → no thumbnail); the full path stays in `title`. Folder chips (added folders) are unchanged.

- [ ] **Step 1: Write failing tests** for the pure parts (a helper deciding "thumbnail allowed": image kind and size ≤ 1 MB, `None` size → no; the label/tag composition) and keep existing composer/turn tests green; update the turn test that asserts the old chip markup.
- [ ] **Step 2: Run** `cargo test -p forge-features -p forge-entities` — fail.
- [ ] **Step 3: Implement** the cards and CSS (existing CSS variables; light and dark; a card about 110 px wide, tag as a small rounded pill; thumbnail `object-fit: cover`).
- [ ] **Step 4: Run** `cargo test -p forge-features -p forge-entities -p forge-widgets -p forge-shared` and `cargo build -p forge-app` — pass.
- [ ] **Step 5: Commit** `feat: attached files are cards with a type tag and image thumbnails`.

### Task 4: The mention list

**Files:**
- Create: `crates/forge-widgets/src/mention_list.rs`; export in `lib.rs`.
- Modify: `crates/forge-features/src/composer_state.rs` and/or `crates/forge-features/src/workspace.rs` (the per-session file list cache), the composer's text area key handling (find where `Enter` sends: `composer_controls.rs` / `chat_window.rs`), `ko.rs`, `theme.css`.

**Interfaces:**
- Consumes: Task 1's `mention_query`, `rank_mentions`; Task 2's `list_files`; `ComposerState::add_attachments`/the same path the `+` menu uses; how the existing popups (plus menu, slash) anchor and close.
- Produces: `Workspace::session_files(id) -> FileListState` (`Idle`, `Loading(old: Option<Vec<String>>)`, `Ready { files, cut }`) and `Workspace::refresh_session_files(id)` (starts the walk off the UI thread if none is in flight for that session; keeps the old list while a refresh runs; the first call is made when the person types a valid `@`; a refresh also after a turn ended or 30 s after the last walk; dropped when the session closes); `MentionList` popup: rows (`MentionRow`: icon via the Files panel's icon component for the extension, name, folder in grey, a tick for `attached`), ≤ 8 visible rows with scrolling, states `Loading…`, `No files in this folder`, and a trailing `Only some files are listed.` row when `cut`; keyboard: `↑`/`↓` move the highlight (wrapping off), `Enter`/`Tab` pick, `Esc` closes, click picks; while open `Enter` never sends; picking removes the `@query` text range and attaches the file (absolute path = session folder joined with the relative path), an already attached row does nothing; the list closes when the text no longer has a valid `mention_query` at the cursor.
- A pure function `attached_in(folder, relative, attachments) -> bool` (lexical comparison of the joined absolute path) tested.

- [ ] **Step 1: Write failing tests**: `attached_in` (inside match, other folder no tick, relative vs absolute), the state machine of the popup as pure logic (highlight moves, pick returns the range and path, Esc closes, a changed query resets the highlight to the first row, a row that becomes attached is skipped on pick), the Workspace cache decisions as pure logic (no second walk while one is in flight; refresh keeps the old list visible; the cache is dropped on close).
- [ ] **Step 2: Run** `cargo test -p forge-features -p forge-widgets` — fail.
- [ ] **Step 3: Implement** the cache, the popup, and the key integration so that while the list is open `Enter` picks and does not send; with it closed `Enter` sends as before (add a test around the key-decision function).
- [ ] **Step 4: Run** `cargo test -p forge-features -p forge-widgets -p forge-shared` and `cargo build -p forge-app` — pass.
- [ ] **Step 5: Commit** `feat: typing @ lists the folder's files and a pick attaches one`.

### Task 5: Samples, docs and the whole-workspace test

**Files:**
- Modify: `docs/superpowers/specs/2026-10-25-composer-attachments-mentions-design.md` (`## As built`, `## Left`), `docs/superpowers/roadmap/README.md` (row 58 → 58A built, 58B left), `docs/superpowers/roadmap/backlog.md` (`NEW-03` `@` half and `NEW-11` built; the queue half of `NEW-03` and `NEW-04` stay 58B).

- [ ] **Step 1: Run** `cargo test --workspace --no-fail-fast` and `cd sidecar && npx vitest run`; record failures other than the known round_trip test (there should be none).
- [ ] **Step 2: Write** the docs from the code (As built; the real-window check is done by the controller after this task and recorded in a follow-up commit).
- [ ] **Step 3: Commit** `docs: composer attachment cards and @ mention as built`.
