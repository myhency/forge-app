# docx formatting in the preview, and refresh of an open Office tab (Plan 57C)

Date: 2026-10-25. Continues `docs/superpowers/specs/2026-10-25-docx-pptx-previews-design.md` (Plan 57B, merged). What the desktop app does is in `docs/superpowers/roadmap/observations.md`, Round 30. Backlog row `NEW-09`.

## Why

In Round 30 an office worker asked Claude to update a report and "mark the changed numbers in red". The desktop app drew the document with the red numbers, blue section titles and shaded or bordered tables, so the person could check at a glance that the change was made. Forge's reading view (57B) drops every colour: the changed numbers look like the rest of the text. A person cannot tell whether the request was done without opening Word. The second gap: after a follow-up turn edited the same `.docx`, the desktop's open tab redrew by itself; Forge's behaviour for an open Office tab is not known to refresh.

## Goals

1. docx text runs keep their **colour**, **highlight**, **underline** and **strike-through** in the preview, in paragraphs, list items, headings and table cells.
2. Headings show the colour their style gives them (Word's blue section titles).
3. Table cells show a **background colour** when the cell has one (shaded header rows).
4. An open `.docx`, `.pptx` or `.xlsx` tab reloads when the file changes on disk (after a turn ends, and when the window gets focus), the way an open text tab already does; the selected cell / scroll position may reset.

## Non-goals and anti-goals

- **Fonts, font sizes, paragraph alignment, spacing, page layout, borders other than the light default**: not drawn (57D, if at all).
- **pptx colours and shapes**: not in this plan (57D).
- **Theme colours** (`w:themeColor` without a `w:val` hex), table styles' conditional shading, and character styles' colours are not resolved; a run without a resolvable colour keeps the theme text colour. Direct run formatting and the paragraph style's run properties (headings) are what is read.
- Anti-goal: a colour value from the file must never reach the page as anything but a validated `#RRGGBB`. The adapter keeps only six hex digits; the view builds the style attribute from that and nothing else.
- Anti-goal: unreadable text. A colour that is near the page background in the current theme (almost black in the dark theme, almost white in the light theme) is ignored and the theme text colour is used. A highlight or a cell fill is drawn as given, and the text on it gets black or white by the fill's luminance (black on a light fill, white on a dark one) unless the run has its own colour that is readable on that fill.
- Anti-goal: the new data must stay inside 57B's budgets (total text bytes, event budget, 50,000 table cells). Colour strings are fixed-size; a cell's runs count against the text budget like any text.

## The design

### Reading (files adapter, `docx.rs`)

- `TextRun` gets `color: Option<Rgb>`, `highlight: Option<Rgb>`, `underline: bool`, `strike: bool` where `Rgb` is a three-byte value (no free-form string reaches the view). A run's properties: `w:color w:val="RRGGBB"` (`auto` and non-hex values mean none); `w:highlight w:val` named colours (`yellow`, `green`, `cyan`, `magenta`, `blue`, `red`, `darkBlue`, `darkCyan`, `darkGreen`, `darkMagenta`, `darkRed`, `darkYellow`, `darkGray`, `lightGray`, `black`, `white`; `none` means none) mapped to a fixed table; `w:shd w:fill` on a run as highlight when no `w:highlight`; `w:u` with `w:val` other than `none`; `w:strike` / `w:dstrike` (val `0`/`false` turns off, as 57B does for bold and italic). `*PrChange` elements stay ignored.
- `DocBlock::Heading` gets `color: Option<Rgb>` from the heading style's run properties (`w:style/w:rPr/w:color`, following `basedOn` like the list lookup, bounded to 10 steps); a heading's own runs may still carry direct colours, which win per run.
- Table cells: `Table { rows }` becomes rows of `TableCell { runs: Vec<TextRun>, fill: Option<Rgb> }` (the cell text is its runs joined as today: nested tables flattened, a space between paragraphs). `fill` from `w:tcPr/w:shd w:fill` (`auto` or non-hex: none). Existing caps unchanged (100 rows × 20 columns, 50,000 cells, 8 MB of text).
- The shared event budget and the 5 MB limits for styles/numbering from 57B apply unchanged; the styles pass already reads `w:style`, it also collects each style's run colour.

### View (`files_office.rs`, `theme.css`)

- A run with colour/highlight/underline/strike is drawn as a `span` with an inline style built only from validated values: `color:#RRGGBB`, `background-color:#RRGGBB`, `text-decoration: underline|line-through|underline line-through`. Runs with none of these keep today's plain output (no extra span nodes for plain text).
- Contrast: `colour_is_readable(color, dark_theme)` (pure, tested): ignore a colour whose relative luminance is below 0.12 in the dark theme or above 0.88 in the light theme. The view asks the current theme the way other widgets do (find how `theme.css` / the theme setting is read in widgets; a CSS-only approach is also fine: draw coloured runs with a `data-lum="dark|light|mid"` attribute and let `theme.css` reset the colour for the unreadable class per theme).
- Cells with `fill` get `background-color` and keep the theme text colour.
- Heading colour: the heading element gets the style colour unless unreadable.

### Refresh of an open Office tab

- Find how an open text tab detects a change (`files_state.rs` `refresh`/`refresh_all` use `FileVersion` and `stat`; they are called after a turn and on focus). Make a `Sheet`, `Document` or `Slides` tab take part: when its file's version changed, read it again and replace the tab's kind. A failed re-read keeps the old view and shows nothing new. If Office tabs already refresh, add the test that proves it and change nothing. Never reload while a read is in flight (use the same ticket idea as the text tabs).

## Answers to the builder check (my calls)

- Office tabs have no version tracking today as far as the builder could tell: store a `FileVersion` and use the same ticket/`Pending::Refresh` rules the text tabs use; the refresh replaces the tab's kind while it is shown (the selected cell / scroll may reset). Check `files_state.rs` first; if Office tabs already refresh, add the proving test and change nothing.
- The theme has no Rust-side value: the CSS route is fine. `colour_is_readable` (pure, tested) computes the `data-lum` class that the CSS uses per theme; it need not take a theme argument then (`luminance_class(color) -> Dark|Mid|Light`).
- `Rgb` and `TableCell` derive `Debug, Clone, Copy (Rgb only), PartialEq, Eq, Default`; every consumer of `TextRun`/`Table` (view, `contents_to_copy` tests, adapter tests) is updated.
- A heading style with `w:themeColor` plus a hex `w:val`: read the hex `w:val` (Word writes both).

## Decisions I made (not the person's)

- Theme-colour-only runs are left uncoloured; headings take only a hex colour from the style.
- Unreadable colours are ignored per theme instead of drawing the document on a white page.
- Fonts and sizes are not drawn.
- A document's selection/scroll may reset on refresh.
- pptx stays text-only here.

## Left to the builder

Field names beyond the shape above, how `Rgb` is stored, where the highlight name table lives, how the theme is read for the contrast rule, the CSS, tests (fixtures built in test code, as in 57B), and how the refresh hooks into the existing tab refresh.

## As built

Types (forge-ports `files_port.rs`): `Rgb(u8, u8, u8)` with `from_hex` (exactly six ASCII hex digits in any case; `auto`, `#`, other lengths are `None`) and `css()` (`#rrggbb`, lower case). `TextRun` gained `underline`, `strike`, `color: Option<Rgb>` and `highlight: Option<Rgb>` (a named highlight or the run's shading) and derives `Default`. `TableCell { runs: Vec<TextRun>, fill: Option<Rgb> }` with `text()` replaces the plain string cell, so `DocBlock::Table` holds `Vec<Vec<TableCell>>`. `DocBlock::Heading` gained `color: Option<Rgb>`.

Reader (`docx.rs`), the rules:
- In a run's `w:rPr`: `w:u` is on unless `val` is `none` or off; `w:strike` and `w:dstrike` both give `strike`; `w:color`, `w:highlight` and `w:shd` (its `fill`) are read. A `w:themeColor` is not resolved. A colour on the paragraph mark is not the text's colour.
- Colours are validated in one place, `Rgb::from_hex`; the highlight names go through the table in `highlight_of` (the 16 Word names; `none` and unknown names give no highlight). The run's `highlight` is the named highlight, else its `w:shd` fill.
- A cell's fill is the `w:shd` fill in its own `w:tcPr` (table depth 1 only). A cell keeps its runs; paragraphs are joined by a plain space run, text from a nested table is part of the cell. The 10,000 character cap per cell now counts across runs.
- A heading's colour is the `w:color` in its paragraph style's own `w:rPr` (not the style's paragraph properties or a `w:tblStylePr`), followed up `basedOn` at most 10 steps. A style's `w:color auto` ends the lookup with no colour.
- Neighbouring runs with the same format are one run (`same_format`, `push_run`).
- Run budget: a document keeps at most 100,000 runs (`MAX_RUNS`) across paragraphs, list items and table cells, counted after neighbouring runs are joined. A paragraph's runs go back to the budget when a cell joins them (the cell takes them again) or when it becomes a heading (only its text is kept). When the budget runs out, further runs and their text are dropped and the document is marked cut. All 57B budgets are unchanged. Measured: a view of 100,000 alternating runs rebuilds in 0.78 s in a debug build and 0.17 s in a release build (300k edits).
- The `w:rPr` rule: a `w:t` inside `w:rPr` is not text, and run properties opened inside a `w:t` stop its text and change no formatting. A small file can no longer turn format toggles inside one text element into millions of runs.

Views (`files_office.rs`):
- `luminance` (relative luminance of linearised sRGB), `luminance_class` (`Dark` below 0.12, `Light` above 0.88, else `Mid`), `text_on` (black on a fill of luminance 0.5 or more, else white).
- `run_style_on` / `run_style` build the inline style only from `Rgb::css` and fixed keywords: the run's colour if it differs by at least 0.35 in luminance from its backdrop (highlight, else cell fill), the readable `text_on` colour on a highlight otherwise, the theme colour on a cell fill; `background-color` for a highlight; `text-decoration` for underline and strike.
- A colour that sits straight on the page gets `data-lum="dark|mid|light"` (`lum_attr_on`). `theme.css` has `.office-page [data-lum="dark"]` and `"light"` rules with `color: inherit !important` per theme (dark media, `data-theme="dark"`, light media, `data-theme="light"`), so a black run is not drawn black on the dark page and a white run not white on the light page. A run with a highlight or in a filled cell has no `data-lum`. The theme tests count variable blocks by `VARIABLE_BLOCK` and check that there is exactly one override block besides them.
- Runs are rendered as keyed fragments; a VirtualDom test renders a paragraph mixing plain and styled runs twice to catch a key mismatch.

Refresh: `FilesCtx` has `viewed: Signal<HashMap<String, FileVersion>>` (the version an Office tab was read at) and `reading: Signal<HashSet<String>>` (reads in flight). `read_and_apply` takes the `stat` before the `read`, so a file that changes in between is read again next time; `apply` records the version for Office tabs only. `refresh_all` has an Office branch (`refresh_office`): stat now, and read again quietly when `office_needs_reread(viewed, now, reading)`. A quiet re-read that fails keeps the old view (`keeps_view_on_failure`); an opening read that fails shows the error. Closing a tab drops its version.

Differs from this spec: the refresh runs after a tool call finishes (the one caller of `refresh_all`) and NOT on window focus. The text tabs do not refresh on focus either; there is no focus caller to hook into. Everything else follows the spec.

## Checked in the real window

With `scripts/harness/add_office_samples.py` (a real `report.docx` from python-docx), light theme: the sample shows the level-1 and level-2 headings in the style's blue, bold and italic runs, a red run (`빨간 글씨`), a yellow-highlighted run, an underlined run and a struck run in one paragraph, bullets and numbers, a 2×2 table whose header cells have a dark-blue fill with white bold text and a body cell with a light-yellow fill, and the 14-column table clipped at the right edge and scrollable. The pptx still shows text-only slide cards. NOT checked: dark mode (the `data-lum` resets and the text colour on fills), the refresh of an open Office tab after a turn (it needs a real tool call to finish), a real Word-made file with theme colours, Windows.

## Left (not in 57C)

Fonts, sizes, alignment, borders; theme colours and table styles; pptx colours, shapes, charts and pictures; images in docx; pages and slide images (57D).

Found while building:
- Refresh on window focus for all tabs (text and Office).
- A `stat` that fails while the `read` succeeds leaves an Office tab without a version, so it never refreshes.
- Before the run budget, the event budget was the only bound on runs. Format toggles inside a `w:t` inside `w:rPr` cost 2 events per run, so a 651 KB file gave about 2.5 million runs and an 8.5 s view rebuild. The 100,000-run budget and the `w:rPr` rule fix this.
- A quiet re-read that fails keeps the old version, so every later tool call tries the read again (intended).
- An Office tab whose opening read failed (`TabKind::Failed`) never refreshes.
- `reading` is a set, not a count, so two overlapping reads of one path clear the flag early.
- Every text-tab read now does one extra `stat`.
- The async refresh glue (`refresh_office`) has no unit test; only `office_needs_reread` and `keeps_view_on_failure` do.
- `highlight val=none` together with a `w:shd` fill shows the shading.
- A style's explicit `w:color auto` stops the `basedOn` lookup instead of continuing up.
