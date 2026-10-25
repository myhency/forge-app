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

## Left (not in 57C)

Fonts, sizes, alignment, borders; theme colours and table styles; pptx colours, shapes, charts and pictures; images in docx; pages and slide images (57D).
