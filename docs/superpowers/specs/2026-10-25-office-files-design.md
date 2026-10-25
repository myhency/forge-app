# Files for office work: result links, the 결과물 card and the xlsx preview (Plan 57A)

Date: 2026-10-25. What the desktop app does is in `docs/superpowers/roadmap/observations.md`, Round 26 part 2 (`결과물`) and Round 28 (the Excel result). Backlog rows `NEW-02` and `OTH-14`.

## Why

The person who asks Forge for a spreadsheet or a report is often not a developer. When Claude is done they want to look at the file and then fix it in the program they know (Excel, Word). In Round 28 Forge failed at the first step: the chat printed the result path as inline code that did nothing when clicked, there was no list of what was made, and the xlsx opened as "이 파일은 바이너리 데이터로 보여 텍스트로 미리 볼 수 없습니다". The desktop app shows the file name as a link, lists it in a `결과물` card, and opens the xlsx as a grid.

The person decided: look at it in Forge first, then open it in the real program. Editing cells inside Forge is not wanted.

## Goals

1. A file path that Claude names in its answer is a link, whether it is written as plain text or as inline code, and whether or not a file tool of this session touched it. A click opens the file in the Files panel (the existing `.file-link` path).
2. The details panel gets a `결과물` card: the files this session made or changed, newest first. A click on a row opens the file in the Files panel.
3. An `.xlsx` file opens in the Files panel as a read-only grid: column letters, row numbers, sheet tabs, a formula bar showing the selected cell.
4. The preview of an office file (xlsx now; docx and pptx in 57B) has a button `기본 프로그램으로 열기` that opens the file in the program the system has for it.
5. A file that is still binary and has no preview (a `.docx` before 57B, a `.zip`) also gets that button next to `Finder에서 보기`.

## Non-goals and anti-goals

- **Editing in Forge**: no cell editing, no saving of xlsx. The grid is read-only.
- **docx and pptx previews**: Plan 57B (text, headings, simple tables for docx; one text block per slide for pptx). Until then they show the binary notice plus `기본 프로그램으로 열기`.
- **Formulas are not calculated.** A cell with a formula shows its stored value; a formula that was never calculated shows empty, like the desktop app. The formula bar shows the formula text (`=SUM(B2:B9)`) for such a cell.
- **Charts, images, merged-cell borders, number formats beyond date/number/percent, conditional formats, comments, hidden rows/sheets' styling**: not drawn.
- Anti-goal: opening a link must never run a file. Only the system's "open with the default program" is used, and only when the person clicks the button. A path the person clicks that does not exist shows the Files panel's existing failure note.
- Anti-goal: a huge sheet must not freeze the window. Reading and parsing run off the UI thread and are capped (below).

## What the desktop app does (observed)

- The result file name in the chat is a link; a click opens the Files panel on the file.
- An `.xlsx` opens as a spreadsheet: grid with column letters, a formula bar `fx`, sheet tabs at the bottom (`요약`, `할일`, `데이터`), the folder's files at the right with a filter box. Formula cells never calculated show empty.
- `결과물 ▼` card (only once a file was made): one row per file, newest first, edited files included; hover shows the file name and full path; click opens the Files panel on it. Card order in the details panel: `환경`, `컨텍스트`, `커넥터`, `결과물`, `변경 사항`; each folds with `▼`/`▶`.

## The design

### Links (goal 1)

`forge_entities::links` already links inline code that names a *known* path (a path a file tool of the session named) and bare absolute paths in prose. Two changes:

- An inline code span (not in a fenced or indented block, not inside an existing link) that is an absolute path with a file extension (same test as prose paths: 1–8 letters or digits) becomes a link even when it is not in the known list. My call: no existence check, matching how prose paths already work; a missing file shows the failure note.
- A relative name in inline code still links only when it resolves against the known list (today's behaviour). Not changed.

### `결과물` card (goal 2)

- Source: the paths of the session's file tools that write (`Write`, `Edit`, `MultiEdit`, `NotebookEdit`, not `Read`), plus the absolute paths the assistant's text names **in the last message of a turn** whose extension is a document type (`xlsx xls csv docx doc pptx ppt pdf md txt html png jpg jpeg svg`), deduplicated, newest first. My call: files a script made (python `openpyxl`, a shell command) are only found through the path Claude names in its answer, which is also how Round 28's xlsx would be found; the last-message and extension limits keep `/etc/hosts` and paths from error text out of the card.
- Refresh: the list is rebuilt when a turn ends and when the card mounts; the existence stat runs then (not on every render).
- Rows that no longer exist on disk are left out when the card is built (a stat per row, cheap; done on the UI side with the existing files port, not in `forge_entities`).
- Row: file icon (`icon_of`), file name; hover shows the full path (title). Click opens the Files panel on the file (the same call a `.file-link` click makes). Fold arrow like the other cards; the card is absent when the list is empty. Position: after `커넥터`, before `변경 사항`.
- Korean label `결과물`; English `Outputs`.

### xlsx preview (goal 3)

- New `PreviewKind::Sheet` for `.xlsx` (case-insensitive), icon `FileIcon::Table`.
- Parsing in the files adapter, with `calamine` (pure Rust). A new `FileContent::Sheet(SheetBook)` variant of the files port: `SheetBook { sheets: Vec<Sheet> }`, `Sheet { name, rows: Vec<Vec<SheetCell>>, truncated: bool }`, `SheetCell { text: String, formula: Option<String> }`. Dates/numbers are turned into display text in the adapter.
- Caps (builder may adjust, keep the idea): the file stays under `READ_LIMIT` (10 MB, existing `TooLarge`); per sheet at most 500 rows and 50 columns are kept; `truncated` shows a line under the grid, "처음 500행, 50열만 표시합니다".
- View: header row of column letters (A, B, … AA), a row-number column, the cells as text (left for text, right for numbers), the first sheet selected, sheet tabs at the bottom, a formula bar above the grid showing the selected cell's address and its formula if any, otherwise its text. Click selects a cell; arrow keys move the selection (nice to have, not required).
- A corrupt or password-protected xlsx shows the existing failure note with the error text, plus `기본 프로그램으로 열기`.

### Open in the default program (goals 4 and 5)

- A button `기본 프로그램으로 열기` in the preview's toolbar for `Sheet`, and in the `Binary` notice row next to `Finder에서 보기`. It calls the existing `FileAction::OpenDefault` (`Workspace::open_with_app(path, None)`), the same as the file menu's `파일 열기`.
- The button is not shown for programs and scripts (`app exe command sh bat cmd msi scpt jar`); the person can still open those from Finder. My call, for the strict-policy work PCs.
- No new platform code. On a system with no program for the file, the existing open error is shown the way the file menu shows it.

## Files outside the session folder (the person's decision: A)

The Files panel keeps refusing paths outside the session folder (`FilesError::Outside`); no new read path is opened. A link or `결과물` row whose file is outside the folder does not try to preview: it shows a note "작업 폴더 밖의 파일입니다" with `기본 프로그램으로 열기` and `Finder에서 보기`, which open the absolute path. The `결과물` card keeps such a row (it exists on disk) and a click gives the same note. Opening goes through the existing `Workspace::open_with_app` with the absolute path; the script/program extension exclusion above applies to it too.

## Decisions I made (not the person's)

- Links for every absolute-path inline code, no existence check.
- `결과물` is built from write tool paths plus paths named in assistant messages, filtered by existence.
- Caps of 500 rows and 50 columns; `calamine` as the xlsx reader.
- The button label is `기본 프로그램으로 열기` (the desktop app's file menu says `파일 열기`; the person's goal is the real program, so the label says it).
- 57B (docx, pptx previews) is a separate plan; the person agreed to make them, I split them to keep this plan to one reviewable size.

- Paths: `~/x` and `file://` are not absolute paths and are not linked; `C:\\` and `C:/` paths are (existing rule). Dates in the grid show as `YYYY-MM-DD`; a password-protected or corrupt file shows the raw error text.

## Left to the builder

Where the sheet view lives (a new widget file next to `files_kinds.rs`), how the cells are laid out (table or grid of divs), how the card gets its list (a `Workspace` method), tests, and the English/Korean string table entries.

## As built

Goal 1 (links). `links.rs` in forge-entities also turns inline-code absolute paths into file links. There is no existence check for them, so a made-up path is a link that opens nothing.

Goal 2 (`결과물` card). `output_files.rs` (forge-features) builds the list from the session record, newest first; `OutputsStore` in forge-entities holds it. The card (`details_panel.rs`) refreshes on mount, when a tool call finishes, and at turn start and end. A row click opens the file in the Files panel.

Goal 3 (xlsx grid). `forge-adapter-files` reads sheets with calamine's streaming cell reader. The caps (500 rows x 50 columns) are enforced while streaming, not after loading. Empty styled cells do not extend a sheet. `truncated` means non-empty data was cut. A date with a non-midnight time shows `YYYY-MM-DD HH:MM:SS`. The view is `files_sheet.rs`: column letters, row numbers, right-aligned numbers, sheet tabs and a formula bar. A formula that was never calculated shows an empty cell, and the formula bar shows the formula text.

Goal 4 and 5 (open button). `기본 프로그램으로 열기` sits in the bottom bar beside the sheet tabs (so the formula bar has room). A binary file with no preview shows a notice with that button, `Finder에서 보기` and a third button.

Files outside the session folder (decision A). `open_file` has an outside-file branch; `outside_file_dialog.rs` shows the dialog `작업 폴더 밖의 파일입니다` with `기본 프로그램으로 열기`, `Finder에서 보기` and `닫기`.

Differences from the spec and lessons:
- The ko.rs string table must stay sorted. Lookups are by binary search, so an unsorted entry silently fails. This showed up in the real window as an English label.
- Caps and `truncated` are as above; the spec did not say where they are enforced.

## Checked in the real window (2026-10-25)

Via `scripts/harness` with the new `scripts/harness/add_office_samples.py` (an xlsx with two sheets, a formula never calculated, a fake docx, and an xlsx outside the folder):
- The inline-code paths in the reply are links.
- A click opens report.xlsx as a grid: columns A..C, row numbers, sheet tabs 요약/데이터. The formula bar shows `=SUM(B2:B3)` for the uncalculated B4 cell, which shows empty.
- The 700-row sheet shows `처음 500행, 50열만 표시합니다.`
- Numbers are right-aligned.
- The 결과물 card lists three files, newest first, with the xlsx and docx icons.
- A row outside the folder opens the dialog `작업 폴더 밖의 파일입니다` with `기본 프로그램으로 열기`, `Finder에서 보기`, `닫기`.
- memo.docx shows the binary notice with the three buttons.

Not checked: dark mode, split panes with two sheets, pressing `기본 프로그램으로 열기` (it would launch Excel), Windows.

Minor cosmetics found and left: the truncation line touches the panel's left edge; the three binary-notice buttons wrap to three lines at about 460 px.

## Left (not in 57A)

- docx and pptx previews (57B).
- Keyboard navigation and copy of a cell or range in the grid; charts.
- The selected-cell style rule is not scoped per view: two sheet views in split panes outline the same cell in both.
- The selected cell is not clamped when a reloaded sheet shrinks.
- `OutputsStore.begin()` wakes the card readers on every refresh.
- No Workspace-level tests for the outside-file branch of `open_file` (no harness for it).
- Paths in fenced code blocks do not count as outputs.
- The two cosmetics above.
- The `결과물` card's hover tooltip styling if the title attribute is too plain.
