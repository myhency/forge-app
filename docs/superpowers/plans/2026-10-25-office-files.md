# Office files (Plan 57A) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A file path Claude names becomes a link, the details panel lists the session's `결과물`, an `.xlsx` opens as a read-only grid, and office or binary files can be opened in their default program.

**Architecture:** Pure rules go in `forge-entities` (links, output list, program exclusion, preview kind). The files adapter parses xlsx with `calamine` into a new `FileContent::Sheet`. `forge-features::Workspace` holds the output list and the "outside the folder" state; `forge-widgets` draws the card, the grid and the note.

**Tech Stack:** Rust, Dioxus, `calamine` (new), `rust_xlsxwriter` (new, dev-dependency of the files adapter, to make fixtures).

**Spec:** `docs/superpowers/specs/2026-10-25-office-files-design.md`

## Global Constraints

- Korean UI text goes through `tr!("English")` with an entry `("English", "한국어")` in `crates/forge-shared/src/ko.rs`; new strings: `Outputs`→`결과물`, `Open in the default program`→`기본 프로그램으로 열기`, `This file is outside the session folder.`→`작업 폴더 밖의 파일입니다.`, `Only the first 500 rows and 50 columns are shown.`→`처음 500행, 50열만 표시합니다.`
- Xlsx caps: file under `READ_LIMIT` (existing), at most 500 rows and 50 columns per sheet.
- Read only: no cell editing, no xlsx writing in product code.
- No existence-unchecked "run": opening goes through `Workspace::open_with_app(path, None)` only on a click of the button.
- Never change the rule that the Files port refuses paths outside the folder.
- Match the surrounding code's comment density and naming. Commit after each task with a `feat:`/`fix:`/`docs:` message.
- Test with `cargo test -p <crate>`; before the last task `cargo test --workspace --no-fail-fast` (two `forge-adapter-claude-plugins` tests are known flaky/machine-state failures and are unrelated).

## Review Focus

- A path in inline code inside a fenced or indented block, or inside an existing link, must stay unlinked.
- A path with a quote, `<`, `>` or `&` must never reach an attribute.
- An `.xlsx` with an empty sheet, a sheet with only formulas, a corrupt file and a password-protected file must not panic.
- A sheet over the caps shows the first rows and the truncation line.
- A script or program extension never gets the `기본 프로그램으로 열기` button.

---

### Task 1: Inline-code absolute paths become links

**Files:**
- Modify: `crates/forge-entities/src/links.rs`

**Interfaces:**
- Consumes: existing `link_known_code`, `has_file_extension`, `is_absolute_path`.
- Produces: `render_markdown_linked(source, known)` also links an inline code span that is an absolute path with a file extension even when not in `known`.

- [ ] **Step 1: Write failing tests** in `links.rs`'s test module: `an_absolute_path_in_inline_code_links_without_being_known` (`` `/p/out/보고서.xlsx` `` gives `<span class="file-link" data-path="/p/out/보고서.xlsx" …><code>…</code></span>` with empty `known`); `a_relative_name_in_inline_code_stays_plain_unless_known`; `an_absolute_path_in_a_fenced_block_stays_plain`; `an_inline_code_path_with_an_ampersand_is_not_linked`; `a_windows_path_in_inline_code_links` (`` `C:\work\a.xlsx` ``).
- [ ] **Step 2: Run** `cargo test -p forge-entities links` — new tests fail.
- [ ] **Step 3: Implement** in `link_known_code`: when `resolve_escaped(text, known)` gives `None`, accept `text` itself if it is an absolute path (`is_absolute_path`) with `has_file_extension`; keep the existing quote/`<`/`>`/`&` filter and the block check. Remove the early `known.is_empty()` return only as far as needed so the absolute-path rule still runs.
- [ ] **Step 4: Run** `cargo test -p forge-entities` — pass.
- [ ] **Step 5: Commit** `feat: inline code that names an absolute file path is a link`.

### Task 2: The output list, program exclusion, preview kind (pure rules)

**Files:**
- Create: `crates/forge-entities/src/outputs.rs`; export from `lib.rs`.
- Modify: `crates/forge-entities/src/preview.rs`

**Interfaces:**
- Produces:
  - `pub fn output_paths(messages: &[Message]) -> Vec<String>`: absolute paths from `Write`/`Edit`/`MultiEdit`/`NotebookEdit` tool calls (not `Read`), plus absolute paths named (prose or inline code, found with the same path rules as `links.rs`: expose a `pub(crate) fn absolute_paths_in(text: &str) -> Vec<String>` there) in the **last assistant text message of each turn** whose extension is in `OUTPUT_EXTENSIONS` (`xlsx xls csv docx doc pptx ppt pdf md txt html png jpg jpeg svg`, any case). Deduplicated; newest first (the path whose last mention is latest comes first).
  - `pub fn is_program(path: &str) -> bool`: true for extensions `app exe command sh bat cmd msi scpt jar` (any case).
  - `PreviewKind::Sheet` for `.xlsx`; `icon_of` gives `FileIcon::Table` for it; `has_rendered_view` stays false for it; `open_target` stays `Files`.
- Consumes: `Message::tool_calls()`, the message text accessors (read `known_paths` in `links.rs` and the `Message` type in `forge-domain` for how to get a message's role and text and how a turn ends).

- [ ] **Step 1: Write failing tests** `outputs.rs`: `write_tools_count_and_read_does_not`; `a_path_in_the_last_message_of_a_turn_counts_if_its_extension_is_a_document_type`; `a_path_in_an_earlier_message_of_the_turn_does_not_count`; `etc_hosts_and_a_path_without_a_document_extension_do_not_count`; `the_newest_mention_comes_first_and_duplicates_collapse`; `is_program_knows_scripts_in_any_case`. `preview.rs`: `xlsx_is_a_sheet_in_any_case`, `a_sheet_has_the_table_icon`.
- [ ] **Step 2: Run** `cargo test -p forge-entities` — fail.
- [ ] **Step 3: Implement** as in Interfaces. A "turn" is a run of messages starting at a user message; its last assistant text message is the last message with assistant text before the next user message.
- [ ] **Step 4: Run** `cargo test -p forge-entities` — pass; `cargo test --workspace --no-run` still builds (match arms on `PreviewKind` elsewhere get a `Sheet` arm that behaves like `Text` until Task 6).
- [ ] **Step 5: Commit** `feat: output paths, program extensions and the sheet preview kind`.

### Task 3: The `결과물` card and the outside-folder note

**Files:**
- Modify: `crates/forge-features/src/details.rs` (`DetailsCard::Outputs`), `crates/forge-features/src/workspace.rs`, `crates/forge-widgets/src/details_panel.rs`, `crates/forge-shared/src/ko.rs`, `crates/forge-shared/src/theme.css`
- Create: `crates/forge-widgets/src/outside_file_dialog.rs`; export in `lib.rs`

**Interfaces:**
- Consumes: `output_paths`, `is_program`, `Workspace::open_file`, `Workspace::open_with_app`, the files port for a stat (read how `refresh_change_counts` is wired for a background refresh and reuse that pattern).
- Produces:
  - `Workspace::session_outputs(id) -> Vec<String>` (absolute paths that exist; refreshed by `Workspace::refresh_outputs(id)`), refreshed when a turn ends and when the card mounts, never on every render.
  - `Workspace::outside_file() -> Option<String>` and `Workspace::close_outside_file()`. `open_file` for a path outside the session folder now sets this instead of the old notice.
  - `OutsideFileDialog` component: text `작업 폴더 밖의 파일입니다`, the file name, buttons `기본 프로그램으로 열기` (hidden when `is_program`) , `Finder에서 보기` (existing reveal call), and `닫기`.
  - `OutputsCard` in the details panel between `ConnectorsCard` and `ChangesCard`: header `결과물` with the fold arrow (`DetailsCard::Outputs`), a row per path (`icon_of` icon via the existing files icon component, file name, `title` attribute with the full path), click calls `workspace.open_file(id, path)`. The card renders nothing when the list is empty.

- [ ] **Step 1: Write failing tests** in `forge-features` (follow the existing `workspace.rs` test style): `a_path_outside_the_folder_sets_the_outside_file_not_a_notice`; `a_path_inside_the_folder_still_opens_in_the_files_panel`; `outputs_drop_paths_that_do_not_exist`. A widget test where the existing widgets have them (ssr render): `the_outputs_card_is_absent_when_empty`, `an_output_row_shows_the_name_and_the_full_path_as_title`.
- [ ] **Step 2: Run** `cargo test -p forge-features -p forge-widgets` — fail.
- [ ] **Step 3: Implement** per Interfaces; CSS reuses `env-card` / `env-card-row` classes; add strings to `ko.rs`.
- [ ] **Step 4: Run** the same tests — pass; `cargo build -p forge-app` compiles.
- [ ] **Step 5: Commit** `feat: the 결과물 card and a note for files outside the folder`.

### Task 4: xlsx reading in the files adapter

**Files:**
- Modify: `crates/forge-ports/src/files_port.rs`, `crates/forge-adapter-files/src/lib.rs`, `crates/forge-adapter-files/Cargo.toml`, `Cargo.toml` if the workspace lists shared dependencies, and every `match` on `FileContent` that the compiler reports (add the `Sheet` arm: `files_state.rs` `contents_to_copy` returns the "Only a text file can be copied" error).

**Interfaces:**
- Produces in `forge-ports`: `FileContent::Sheet(SheetBook)`; `pub struct SheetBook { pub sheets: Vec<Sheet> }`, `pub struct Sheet { pub name: String, pub rows: Vec<Vec<SheetCell>>, pub truncated: bool }`, `pub struct SheetCell { pub text: String, pub formula: Option<String> }` (all `Debug, Clone, PartialEq`). `rows` holds at most 500 rows of at most 50 cells; `truncated` is true when either cap cut something.
- `FilesPort::read` returns `FileContent::Sheet` for a file with extension `xlsx` (any case) under `READ_LIMIT`; an unreadable or password-protected file returns `Err(FilesError::Failed(<calamine error text>))`.
- Cell text: strings as they are, numbers without a trailing `.0` when whole, booleans `TRUE`/`FALSE`, dates `YYYY-MM-DD`, errors as their code (`#DIV/0!`), empty cells `""`. A formula cell's `formula` is its formula text with a leading `=`; a formula without a stored value has `text == ""`.

- [ ] **Step 1: Add dependencies** `calamine` (latest stable) to the adapter; `rust_xlsxwriter` as a dev-dependency. If the network is unavailable, report BLOCKED.
- [ ] **Step 2: Write failing tests** in the adapter, building fixtures with `rust_xlsxwriter` into a temp dir: `an_xlsx_is_read_as_sheets_with_names_and_cell_text`; `a_whole_number_has_no_decimal_point`; `a_date_cell_shows_as_iso`; `a_formula_cell_keeps_its_formula_and_stored_value` (write with `write_formula_with_result`); `a_formula_without_a_stored_value_has_empty_text_and_the_formula`; `a_sheet_over_500_rows_or_50_columns_is_cut_and_marked`; `an_empty_sheet_has_no_rows`; `a_file_named_xlsx_that_is_not_a_zip_fails_with_text`; `an_uppercase_xlsx_extension_is_read`.
- [ ] **Step 3: Run** `cargo test -p forge-adapter-files` — fail.
- [ ] **Step 4: Implement** the port types and the adapter branch (check the extension before the NUL-byte test, as `is_pdf` is checked). Use calamine's `Xlsx` reader on the bytes and its range plus formula range.
- [ ] **Step 5: Run** `cargo test --workspace --no-run` and `cargo test -p forge-adapter-files -p forge-ports` — pass.
- [ ] **Step 6: Commit** `feat: read an xlsx file as sheets`.

### Task 5: The sheet view and the open button

**Files:**
- Create: `crates/forge-widgets/src/files_sheet.rs`; export in `lib.rs`
- Modify: `crates/forge-widgets/src/files_kinds.rs` (`TabKind::Sheet(SheetBook)`, `kind_of`), `crates/forge-widgets/src/files_panel.rs` (render it; the `Binary` notice gets the open button), `crates/forge-widgets/src/files_state.rs` (an `open_default(path)` action if `FileAction::OpenDefault` is not directly reachable from the view), `crates/forge-shared/src/theme.css`, `crates/forge-shared/src/ko.rs`

**Interfaces:**
- Consumes: `FileContent::Sheet`, `is_program`, `FileAction::OpenDefault` / `Workspace::open_with_app`.
- Produces:
  - `pub fn column_letters(index: usize) -> String` (0→`A`, 25→`Z`, 26→`AA`) in `files_sheet.rs` (or `forge-entities` if the widget crate has no pure-function home; follow where `parse_csv` lives).
  - `SheetView(book: SheetBook, path: String)` component: formula bar (selected cell address like `B3` and its `formula` or `text`), a grid (corner cell, column letters, row numbers, cells; numbers right-aligned by a simple "text parses as a number" test), sheet tabs at the bottom (first selected), a click selects a cell, the truncation line `처음 500행, 50열만 표시합니다` when `truncated`, and a toolbar button `기본 프로그램으로 열기`.
  - The `Binary` tab view shows `기본 프로그램으로 열기` next to `Finder에서 보기` unless `is_program(path)`.
  - A failed read of an `.xlsx` shows the failure text plus the same button.

- [ ] **Step 1: Write failing tests**: `column_letters` cases (0, 25, 26, 51, 701, 702); widget ssr tests like the existing csv ones: `a_sheet_shows_tabs_and_the_first_sheet_cells`, `a_truncated_sheet_shows_the_line`, `the_open_button_is_in_the_sheet_toolbar`, `a_binary_file_has_the_open_button_unless_it_is_a_program`.
- [ ] **Step 2: Run** `cargo test -p forge-widgets` — fail.
- [ ] **Step 3: Implement** per Interfaces; reuse the CSV table styling classes where they fit and add `sheet-*` classes in `theme.css` for the formula bar, the tabs and the row/column headers (light and dark through the existing CSS variables).
- [ ] **Step 4: Run** `cargo test -p forge-widgets -p forge-features -p forge-entities` and `cargo build -p forge-app` — pass.
- [ ] **Step 5: Commit** `feat: the xlsx grid and the open-in-default-program button`.

### Task 6: Docs and the whole-workspace test

**Files:**
- Modify: `docs/superpowers/specs/2026-10-25-office-files-design.md` (add `## As built` and update `## Left`), `docs/superpowers/roadmap/README.md`, `docs/superpowers/roadmap/backlog.md` (`NEW-02`, `OTH-14`: built, Plan 57A; 57B left).

- [ ] **Step 1: Run** `cargo test --workspace --no-fail-fast` and the sidecar tests; record failures other than the two known plugin-crate tests (there should be none).
- [ ] **Step 2: Write** the As-built section from the code (what differs from the spec, files, caps).
- [ ] **Step 3: Commit** `docs: office files as built`.

The real-window check (a throwaway folder with a made xlsx and a docx, a turn that names them, the card, the grid, the outside-folder note) is done by the controller after Task 6, not by a subagent.
