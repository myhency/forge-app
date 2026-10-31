# docx and pptx previews (Plan 57B)

Date: 2026-10-25. Continues `docs/superpowers/specs/2026-10-25-office-files-design.md` (Plan 57A, merged): links, the `결과물` card, the xlsx grid, the outside-folder dialog and `기본 프로그램으로 열기` already exist. Backlog row `NEW-02` (the last part).

## Why

The person who gets a report or a deck from Claude wants to look at it in Forge first and then fix it in Word or PowerPoint (57A's decision: no editing in Forge). Today a `.docx` or `.pptx` shows the binary notice ("이 파일은 바이너리 데이터로 보여 …") with only the open buttons, so the person cannot tell whether Claude made the right file without leaving Forge.

## Goals

1. A `.docx` opens in the Files panel as a read-only document: headings, paragraphs with bold/italic, bullet and numbered list items, simple tables.
2. A `.pptx` opens as a read-only list of slides: one card per slide, in slide order, with the slide title and the text of the other shapes (bullets keep their indent level).
3. Both have the `기본 프로그램으로 열기` button (same rule as the sheet: hidden for programs and scripts, which these are not), and a failed read shows the failure text plus the button (like a failed xlsx).
4. Hostile or broken files never crash or freeze the app (same guarantees as 57A's xlsx reader).

## Non-goals and anti-goals

- **No editing, no saving, no comments, no tracked-change view.** Text inside tracked deletions (`w:del`) is left out, tracked insertions (`w:ins`) show as normal text.
- **Not drawn**: images, charts, shapes' geometry, SmartArt, headers and footers, footnotes, text boxes inside drawings of docx, slide masters and layouts, animations, speaker notes. A slide that has only a picture shows its title (if any) and a line "텍스트가 없는 슬라이드입니다".
- **No page layout fidelity**: no page breaks, margins, fonts, colours or sizes. It is a reading view of the text and its structure, not a copy of the page.
- **Legacy `.doc` and `.ppt`** (binary Office 97) and OpenDocument files: no preview; they keep the binary notice plus the open button.
- Anti-goal: parsing must not resolve external entities or fetch anything (no network, no file access outside the file's own zip). Cell and paragraph text is rendered as text, never as HTML.
- Anti-goal: a huge document must not freeze the window (caps below, reading off the UI thread like the xlsx).

## The design

### Reading (files adapter)

- The adapter already depends on `zip` and `quick-xml` (57A). `FileContent` gets two variants: `Document(DocumentView)` and `Slides(SlideDeck)`, chosen by extension (`docx`, `pptx`, any case) before the binary test, like `xlsx`.
- Port types (names are the builder's to adjust, shape is fixed):
  - `DocumentView { blocks: Vec<DocBlock>, truncated: bool }`; `DocBlock` is `Heading { level: u8 (1–6), text }`, `Paragraph { runs: Vec<TextRun> }`, `ListItem { level: u8, numbered: bool, runs }`, `Table { rows: Vec<Vec<String>> }`; `TextRun { text, bold, italic }`.
  - `SlideDeck { slides: Vec<Slide>, truncated: bool }`; `Slide { number: usize, title: Option<String>, items: Vec<SlideItem> }`; `SlideItem { level: u8, text }`.
- docx: read `word/document.xml`. Heading = paragraph style id `Heading1`…`Heading9` (levels above 6 become 6) or `Title` (level 1). List item = a paragraph with `w:numPr`; `numbered` is true when the numbering definition's format for that level is not `bullet` (look at `word/numbering.xml`; if it cannot be resolved, treat as bullet). Hyperlinks show their text. A tab is a space, a line break (`w:br`) splits into a new run line break shown as a space. Tables: each `w:tc` text joined, nested tables flattened to text.
- pptx: slide order from `ppt/presentation.xml` (`p:sldIdLst`) and its relationships; read each `ppt/slides/slideN.xml`. Title = the shape whose placeholder type is `title` or `ctrTitle`; other shapes' paragraphs become `SlideItem`s with the paragraph's `lvl`. Group shapes and table cells inside slides: their text is included as items.
- Caps (builder may tune, keep the idea): file under `READ_LIMIT` (existing `TooLarge`); each zip part's declared size at most 100 MB and its real unpacked size bounded while reading (the same guard style as 57A's `check_xlsx`); at most 2,000 blocks for a docx and 200 slides for a pptx; a table keeps at most 100 rows and 20 columns; text of one paragraph/item is cut at 10,000 characters; `truncated` is true when something was cut and the view shows a line "일부만 표시합니다" under the content.
- A password-protected file (an encrypted Office file is a compound file, not a zip) fails with a readable text like the xlsx's ("password"); a corrupt or non-Office zip fails with the library's text.

### View (widgets)

- `PreviewKind::Document` for `.docx` and `PreviewKind::Slides` for `.pptx`; icons: `FileIcon::Doc` for both (the builder may add a slide icon if one exists).
- `DocumentView` component: a reading column (max width about 720 px) with headings in descending sizes, paragraphs, runs with bold/italic, list items indented by level with `•` or `1.`-style markers (numbers counted per list in the view), tables with light borders. The open button is in a slim toolbar above the content, right-aligned. No selection state is needed; text is selectable.
- `SlideView` component: slide cards stacked vertically, each with a small header `슬라이드 N`, the title in bold, the items indented by level. Same toolbar.
- `failed_offers_open` (57A) also covers `Document` and `Slides` kinds; the binary notice's open button stays for other files.
- Korean strings go through `tr!`, entries in `ko.rs` keeping the table sorted (an unsorted entry silently falls back to English; `cargo test -p forge-shared` catches it). New: `Only part of the file is shown.` → `일부만 표시합니다`, `This slide has no text.` → `텍스트가 없는 슬라이드입니다`, `Slide` → `슬라이드`.

## Decisions I made (not the person's)

- Reading view, not layout fidelity; images and charts are not drawn.
- docx tracked deletions hidden, insertions shown.
- A slide card list rather than a slide-by-slide pager.
- Caps: 2,000 blocks, 200 slides, tables 100×20, paragraph 10,000 characters.
- The sample files for the real-window check come from a script (`scripts/harness/add_office_samples.py` gets a real docx and pptx written with python-docx / python-pptx if installed, else a minimal hand-built zip).

## Left to the builder

Port type names and fields beyond the shape above, how numbering is resolved, the CSS, where the view components live (new files next to `files_sheet.rs`), tests with fixtures built in test code (`zip` writer with hand-written minimal XML, so no new dev-dependency is needed unless it is much simpler with one).

## As built

Types (forge-ports `files_port.rs`): `DocumentView`, `DocBlock`, `TextRun`, `SlideDeck`, `Slide`, `SlideItem` as in the spec. Readers are `docx.rs` and `pptx.rs` in forge-adapter-files; the guards they share are in `office_zip.rs`. Views are in `files_office.rs` (forge-widgets).

Guards (`office_zip.rs`, shared by both readers):
- at most 10,000 zip parts;
- each part declares at most 100 MB and is read with a bounded real read;
- one XML event budget of 5,000,000 per file, shared by all the parts read (docx styles, numbering and body; pptx list of slides, rels and slides). Running out marks the file cut;
- the small parts (docx `styles.xml` and `numbering.xml`, pptx `presentation.xml` and its rels) at most 5 MB each, checked against the declared size and the real read. One over this fails with the "too much data" text;
- 200 MB of slide XML in total across slides.

Caps on content:
- per item: 2,000 blocks (docx), 200 slides, 2,000 items per slide (not in the original spec), tables of 100 rows x 20 columns, 10,000 characters per paragraph or item;
- totals per file (added after the final review, which found that a 215 KB docx gave 4,000,000 cells): at most 50,000 table cells across all tables of a docx, 10,000 slide items across all slides of a pptx, and 8 MB of kept text in either reader (`TextBudget`). A budget that runs out stops adding and marks the file cut. The xlsx check (`check_xlsx`) now also refuses a zip with more than 10,000 parts.

docx details beyond the spec:
- A heading is found by style NAME in `styles.xml` (`heading 1`...), not only by id, because Korean Word uses ids such as `1` and `2`.
- A list comes from the paragraph's style too (python-docx `List Bullet`, `List Number`), following `basedOn` up to 10 steps. `numId 0` means not a list.
- `*PrChange` elements (tracked formatting changes) are ignored.
- XML that is not UTF-8 fails with a text error.

Views: a reading column of 720 px, real `h1`...`h6`, list markers counted per list, a table scrolls horizontally inside its own wrapper, slide cards stacked. The open button sits in a slim toolbar, top right.

Strings: `Slide {number}` stays an interpolated string (`슬라이드 {number}`) instead of the spec's plain `Slide` (controller ruling).

## Checked in the real window (2026-10-25)

With the samples from `scripts/harness/add_office_samples.py`:
- report.docx shows the heading, a paragraph with bold and italic, bullets and numbers (after the style-list fix found in this very check), the 2x2 table, and the 14-column table clipped at the right edge and scrollable.
- deck.pptx shows 3 slide cards: title with subtitle; a title with a nested bullet indented; a blank slide with `텍스트가 없는 슬라이드입니다.`
- `기본 프로그램으로 열기` sits in a slim toolbar, top right. The Korean strings are correct.

Not checked: dark mode, the horizontal scrolling by actually dragging, the `일부만 표시합니다` line, a real Word or PowerPoint file (only python-docx and python-pptx samples), password-protected or corrupt files in the window, Windows, pressing the open button.

## Left (not in 57B)

- Images in docx and pptx, headers/footers, footnotes, comments, speaker notes, slide thumbnails, copy of a table, search inside the preview, `.doc`/`.ppt`/OpenDocument.
- Heading detection by style name does not follow `basedOn`.
- A docx style chain longer than 10 `basedOn` steps loses its list.
- No tests for deep nesting or a duplicate `sldId`. The event budget and a DOCTYPE entity (not expanded) are now tested in both readers.
- The message "unpacks to too much data" is also used for the part-count refusal.
