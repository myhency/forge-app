# docx and pptx previews (Plan 57B) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A `.docx` opens in the Files panel as a read-only document and a `.pptx` as a read-only list of slides, each with `기본 프로그램으로 열기`.

**Architecture:** Port types and preview kinds first (Task 1), then two readers in the files adapter on the existing `zip` + `quick-xml` dependencies (Tasks 2, 3), then the two views (Task 4), then samples, docs and the whole-workspace test (Task 5).

**Tech Stack:** Rust, Dioxus, `zip` 9, `quick-xml` 0.42 (both already dependencies of `forge-adapter-files`).

**Spec:** `docs/superpowers/specs/2026-10-25-docx-pptx-previews-design.md` (continues `docs/superpowers/specs/2026-10-25-office-files-design.md`, Plan 57A, merged; read its "As built" for the xlsx reader's guards, `check_xlsx`, `read_xlsx`, `FileContent::Sheet`, `failed_offers_open`, `files_sheet.rs`).

## Global Constraints

- Korean UI text goes through `tr!("English")` with an entry `("English", "한국어")` in `crates/forge-shared/src/ko.rs`; **the table must stay sorted by English key (byte order)** — an unsorted entry silently falls back to English; `cargo test -p forge-shared` catches it. New: `Only part of the file is shown.`→`일부만 표시합니다`, `This slide has no text.`→`텍스트가 없는 슬라이드입니다`, `Slide`→`슬라이드`.
- Read only: no editing, no writing of docx/pptx in product code.
- Caps: file under `READ_LIMIT` (existing `TooLarge`); each zip part's declared size ≤ 100 MB and its real unpacked size bounded while reading (same guard style as `check_xlsx`); docx ≤ 2,000 blocks; pptx ≤ 200 slides; a table keeps ≤ 100 rows × 20 columns; one paragraph/item text cut at 10,000 characters; `truncated` true when anything was cut.
- Parsing never resolves external entities, never opens anything outside the file's own zip, and never panics on malformed XML or zip (return `FilesError::Failed(<text>)`). Password-protected Office files (compound files) fail with a text containing "password" like the xlsx reader.
- Text is rendered as text (Dioxus escapes), never as raw HTML.
- Never change the rule that the Files port refuses paths outside the folder.
- Match surrounding comment density and naming. Commit after each task with a `feat:`/`fix:`/`docs:` message. Run the focused crate tests per task; the whole workspace once in Task 5 (the `forge-adapter-claude-plugins` round_trip test is a known machine-state failure and unrelated).

## Review Focus

- A docx with only a table, an empty body, or a paragraph inside a table cell inside a table (nested) must not panic or loop.
- A pptx whose `presentation.xml` lists a slide whose part is missing, or whose relationships are broken, must skip that slide or fail readably, not panic.
- A docx/pptx zip bomb or a part claiming a huge size is refused before reading (the 57A xlsx incident: calamine aborted the whole process on a hostile file).
- Tracked deletions (`w:del`) never show; list numbering restarts per list in the view.
- Heading levels above 6 become 6; a slide with no text shows the "no text" line.

---

### Task 1: Port types, preview kinds, and the match arms

**Files:**
- Modify: `crates/forge-ports/src/files_port.rs`, `crates/forge-ports/src/lib.rs` (exports), `crates/forge-entities/src/preview.rs`, `crates/forge-widgets/src/files_kinds.rs`, `crates/forge-widgets/src/files_state.rs`, and every other `match` on `FileContent`/`PreviewKind` the compiler reports.

**Interfaces:**
- Produces in `forge-ports` (all `Debug, Clone, PartialEq, Eq`): `FileContent::Document(DocumentView)`, `FileContent::Slides(SlideDeck)`; `DocumentView { blocks: Vec<DocBlock>, truncated: bool }`; `enum DocBlock { Heading { level: u8, text: String }, Paragraph { runs: Vec<TextRun> }, ListItem { level: u8, numbered: bool, runs: Vec<TextRun> }, Table { rows: Vec<Vec<String>> } }`; `TextRun { text: String, bold: bool, italic: bool }`; `SlideDeck { slides: Vec<Slide>, truncated: bool }`; `Slide { number: usize, title: Option<String>, items: Vec<SlideItem> }`; `SlideItem { level: u8, text: String }`.
- Produces in `forge-entities`: `PreviewKind::Document` (`docx`, any case), `PreviewKind::Slides` (`pptx`); `icon_of` → `FileIcon::Doc` for both; `has_rendered_view` false; `open_target` stays `Files`.

- [ ] **Step 1: Write failing tests** in `preview.rs`: `docx_is_a_document_and_pptx_are_slides_in_any_case`, `documents_and_slides_have_the_doc_icon`, `open_target_of_docx_and_pptx_is_files`; in `files_kinds.rs`: `kind_of_keeps_a_document_and_a_deck` (until Task 4, `kind_of` maps them to `TabKind::Binary`; the test then asserts the Task 4 shape, so write it in Task 4 instead and here only test `contents_to_copy` returns the "Only a text file can be copied" error for both variants).
- [ ] **Step 2: Run** `cargo test -p forge-entities -p forge-widgets` — fail.
- [ ] **Step 3: Implement** the port types and variants, the preview kinds, and the stopgap arms (`kind_of`: `Document`/`Slides` → `TabKind::Binary`; say so in a comment like 57A did). `cargo test --workspace --no-run` must build.
- [ ] **Step 4: Run** `cargo test -p forge-ports -p forge-entities -p forge-widgets` — pass.
- [ ] **Step 5: Commit** `feat: document and slide types in the files port, and their preview kinds`.

### Task 2: The docx reader

**Files:**
- Modify: `crates/forge-adapter-files/src/lib.rs` (the `read` branch before the NUL test, next to `xlsx`); Create: `crates/forge-adapter-files/src/docx.rs` (parsing) and, if it removes duplication, `crates/forge-adapter-files/src/office_zip.rs` holding the zip guards shared with `check_xlsx` (declared part size ≤ 100 MB, bounded real read through `take(limit + 1)`, compound-file/password detection). Move, do not copy, anything shared; keep xlsx behaviour and tests unchanged.

**Interfaces:**
- Consumes: Task 1's types.
- Produces: `fn read_docx(bytes: Vec<u8>) -> Result<FileContent, FilesError>` returning `FileContent::Document`. Rules: Heading = paragraph style id `Heading1`…`Heading9` (levels above 6 → 6) or `Title` (level 1); list item = paragraph with `w:numPr` (`numbered` true when `word/numbering.xml` gives that level a format other than `bullet`; unresolved → bullet); runs keep `w:b`/`w:i` (a run property `w:val="0"`/`false` turns it off); `w:hyperlink` text shows; `w:tab` and `w:br` become a space; text inside `w:del` is skipped, `w:ins` kept; tables: each `w:tc` text joined with a space, nested tables flattened into the cell text, caps per Global Constraints.

- [ ] **Step 1: Write failing tests** with fixtures built in test code (`zip::ZipWriter` and hand-written minimal XML): `a_docx_gives_headings_paragraphs_and_runs_with_bold_and_italic`; `heading_levels_above_six_become_six`; `a_list_paragraph_is_a_list_item_numbered_by_numbering_xml`; `a_table_becomes_rows_of_cell_text`; `a_nested_table_is_flattened_into_the_cell`; `deleted_text_is_hidden_and_inserted_text_is_shown`; `an_empty_body_gives_no_blocks`; `more_than_2000_blocks_are_cut_and_marked`; `a_table_over_100_rows_or_20_columns_is_cut_and_marked`; `a_paragraph_over_10000_characters_is_cut`; `a_part_declaring_over_100_mb_is_refused`; `a_non_zip_file_named_docx_fails_with_text`; `a_compound_file_named_docx_fails_with_password_text` (see how 57A's password test builds a compound file with `cfb`); `a_docx_without_document_xml_fails_with_text`; `malformed_xml_fails_with_text_and_does_not_panic`; `an_uppercase_docx_extension_is_read`.
- [ ] **Step 2: Run** `cargo test -p forge-adapter-files` — fail.
- [ ] **Step 3: Implement** `read_docx` with `quick_xml::Reader` events (no DTD/entity resolution beyond the five predefined entities), the `read` branch, and any shared-guard extraction.
- [ ] **Step 4: Run** `cargo test -p forge-adapter-files -p forge-ports` and `cargo test --workspace --no-run` — pass.
- [ ] **Step 5: Commit** `feat: read a docx file as headings, paragraphs, lists and tables`.

### Task 3: The pptx reader

**Files:**
- Create: `crates/forge-adapter-files/src/pptx.rs`; Modify: `crates/forge-adapter-files/src/lib.rs` (the `read` branch).

**Interfaces:**
- Consumes: Task 1's types, Task 2's shared zip guards.
- Produces: `fn read_pptx(bytes: Vec<u8>) -> Result<FileContent, FilesError>` returning `FileContent::Slides`. Rules: slide order from `p:sldIdLst` in `ppt/presentation.xml` resolved through `ppt/_rels/presentation.xml.rels` (a slide whose part is missing is skipped, numbering stays by position in the list); title = the shape whose placeholder type is `title` or `ctrTitle`; every other paragraph in shapes, group shapes and table cells becomes a `SlideItem` with the paragraph's `lvl` (0 default); empty paragraphs are dropped; speaker notes, layouts, masters ignored; ≤ 200 slides.

- [ ] **Step 1: Write failing tests** (fixtures built in test code): `slides_come_in_presentation_order_not_file_name_order`; `a_title_placeholder_becomes_the_title_and_other_text_becomes_items_with_levels`; `text_in_a_group_shape_and_a_table_cell_is_included`; `a_slide_with_only_a_picture_has_no_items`; `a_listed_slide_whose_part_is_missing_is_skipped`; `broken_relationships_fail_with_text_or_give_no_slides_without_panicking`; `more_than_200_slides_are_cut_and_marked`; `an_item_over_10000_characters_is_cut`; `speaker_notes_are_not_shown`; `a_non_zip_pptx_fails_with_text`; `a_compound_file_named_pptx_fails_with_password_text`; `an_uppercase_pptx_extension_is_read`; `a_part_declaring_over_100_mb_is_refused`.
- [ ] **Step 2: Run** `cargo test -p forge-adapter-files` — fail.
- [ ] **Step 3: Implement** `read_pptx` and the `read` branch.
- [ ] **Step 4: Run** `cargo test -p forge-adapter-files` and `cargo test --workspace --no-run` — pass.
- [ ] **Step 5: Commit** `feat: read a pptx file as slides with a title and indented text`.

### Task 4: The document view and the slide view

**Files:**
- Create: `crates/forge-widgets/src/files_office.rs` (both components and their pure helpers); export in `lib.rs`.
- Modify: `crates/forge-widgets/src/files_kinds.rs` (`TabKind::Document(DocumentView)`, `TabKind::Slides(SlideDeck)`, `kind_of`, `FileKindView` arms), `crates/forge-widgets/src/files_sheet.rs` (`failed_offers_open` also true for `PreviewKind::Document`/`Slides`), `crates/forge-widgets/src/files_panel.rs` if the failed-read view needs it, `crates/forge-shared/src/theme.css`, `crates/forge-shared/src/ko.rs`.

**Interfaces:**
- Consumes: Task 1's types; `offers_default_program` / `FileAction::OpenDefault` through the `on_open` handler as in `SheetView`; the `Rc`-in-a-memo and child-component pattern of `files_sheet.rs` only where a large list needs it (a document of 2,000 blocks should not be cloned per render: take the value by reference or `Rc`).
- Produces: `DocumentViewer(doc: DocumentView, path: String, on_open: EventHandler<()>)` and `SlideViewer(deck: SlideDeck, path: String, on_open: EventHandler<()>)`; pure helpers: `list_markers(blocks: &[DocBlock]) -> Vec<Option<String>>` (a marker per block: `•` for a bullet item, `1.`, `2.` … for numbered items counted per consecutive run of numbered items at the same or deeper level, restarting after a non-list block; `None` for other blocks), `heading_class(level) -> &'static str`, `slide_is_empty(&Slide) -> bool`.
- Layout: a slim toolbar above the content with `기본 프로그램으로 열기` right-aligned; reading column max width about 720 px; headings in descending sizes; list items indented by level; tables with light borders (use existing CSS variables so dark mode works); slide cards with header `슬라이드 N`, the title in bold, items indented by level, `텍스트가 없는 슬라이드입니다` when `slide_is_empty`; a line `일부만 표시합니다` below the content when `truncated`.

- [ ] **Step 1: Write failing tests**: `list_markers` (bullets; `1.`, `2.` counting; restart after a paragraph; nested numbered items count per level), `heading_class` for 1..=6 and clamping above 6, `slide_is_empty` (title only, no items → empty of body text: define it as "no items"; a slide with a title and no items shows the title and the no-text line), `kind_of_gives_a_document_tab_and_a_slides_tab`, `failed_read_of_docx_or_pptx_offers_the_open_button`.
- [ ] **Step 2: Run** `cargo test -p forge-widgets` — fail.
- [ ] **Step 3: Implement** the components, tab kinds, CSS (`office-*` classes) and strings (sorted in `ko.rs`; run `cargo test -p forge-shared`).
- [ ] **Step 4: Run** `cargo test -p forge-widgets -p forge-shared -p forge-features -p forge-entities` and `cargo build -p forge-app` — pass.
- [ ] **Step 5: Commit** `feat: the document view and the slide view`.

### Task 5: Samples, docs and the whole-workspace test

**Files:**
- Modify: `scripts/harness/add_office_samples.py` (a real `report.docx` and `deck.pptx` in `my-app/office/` named in the reply's inline-code list; use `python-docx`/`python-pptx` if importable, else minimal hand-built zips with a heading, a bullet, a numbered item, a bold run, a 2×2 table; a deck with a title slide, a bullet slide with a nested bullet, and a picture-only slide), `scripts/harness/README.md` (one line), `docs/superpowers/specs/2026-10-25-docx-pptx-previews-design.md` (`## As built`, `## Left`), `docs/superpowers/roadmap/README.md`, `docs/superpowers/roadmap/backlog.md` (`NEW-02` built, Plan 57A + 57B).

- [ ] **Step 1: Run** `cargo test --workspace --no-fail-fast` and `cd sidecar && npx vitest run`; record failures other than the known plugin-crate round_trip test (there should be none).
- [ ] **Step 2: Write** the sample script changes and the docs from the code (As built: what differs from the spec, caps, shared zip guards, files). The real-window check is done by the controller after this task and its result is added to the spec by a follow-up commit.
- [ ] **Step 3: Commit** `docs: docx and pptx previews as built`.
