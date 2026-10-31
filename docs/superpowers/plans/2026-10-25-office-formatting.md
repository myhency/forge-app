# docx formatting and Office tab refresh (Plan 57C) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** docx runs keep colour, highlight, underline and strike; headings keep their style colour; table cells keep their fill; an open Office tab reloads when its file changes.

**Architecture:** Port types and the docx reader first (Task 1), then the view (Task 2), then the tab refresh (Task 3), then samples, docs and tests (Task 4).

**Tech Stack:** Rust, Dioxus, `quick-xml` 0.42, `zip` 9 (existing).

**Spec:** `docs/superpowers/specs/2026-10-25-office-formatting-design.md` (read it and its "Answers to the builder check"); background: `docs/superpowers/specs/2026-10-25-docx-pptx-previews-design.md` (57B, merged: `docx.rs`, `office_zip.rs`, `files_office.rs`, the budgets).

## Global Constraints

- A colour from a file reaches the page only as a validated 6-hex-digit `Rgb` (three bytes) turned into `#RRGGBB` by the view; never a free-form string.
- Stay inside 57B's budgets (event budget, 8 MB text, 50,000 cells, 2,000 blocks, 5 MB styles/numbering). No new unbounded allocation; the style lookups (`basedOn`) are bounded to 10 steps.
- Korean UI text goes through `tr!` with an entry in `crates/forge-shared/src/ko.rs`, **sorted by English key (byte order)**; `cargo test -p forge-shared` catches an unsorted table. This plan adds no new user strings.
- Read only; render text as text (Dioxus escapes); inline styles are built only from `Rgb` values.
- Match surrounding comment density and naming. Commit after each task (`feat:`/`fix:`/`docs:`); run focused crate tests per task, the whole workspace once in Task 4 (the `forge-adapter-claude-plugins` round_trip test is a known machine-state failure, unrelated).

## Review Focus

- A hostile `w:color w:val` (`"FF0000;background:url(x)"`, 7 digits, `auto`, empty) never becomes anything but None or a valid colour.
- A run with `w:u w:val="none"`, `w:strike w:val="0"`, `w:highlight w:val="none"` is NOT underlined/struck/highlighted.
- Unreadable colours (near-black in dark, near-white in light) fall back to the theme text colour; text on a fill gets black/white by the fill's luminance.
- An Office tab refresh never races an in-flight read and never loses the old view on a failed re-read.

---

### Task 1: Port types and the docx reader (colour, highlight, underline, strike, heading colour, cell fill)

**Files:**
- Modify: `crates/forge-ports/src/files_port.rs`, `crates/forge-ports/src/lib.rs`, `crates/forge-adapter-files/src/docx.rs`, every consumer the compiler reports (`crates/forge-widgets/src/files_office.rs` minimal compile fix only: draw the new fields in Task 2; `files_state.rs` tests).

**Interfaces:**
- Produces in `forge-ports`: `pub struct Rgb(pub u8, pub u8, pub u8)` (`Debug, Clone, Copy, PartialEq, Eq, Default`; `Rgb::from_hex(&str) -> Option<Rgb>` accepting exactly six ASCII hex digits, any case; `fn css(self) -> String` giving `#rrggbb` lower case); `TextRun { text, bold, italic, underline: bool, strike: bool, color: Option<Rgb>, highlight: Option<Rgb> }`; `TableCell { runs: Vec<TextRun>, fill: Option<Rgb> }` with `fn text(&self) -> String` (runs joined); `DocBlock::Table { rows: Vec<Vec<TableCell>> }`; `DocBlock::Heading { level: u8, text: String, color: Option<Rgb> }`. All new types derive what the old ones did (`Debug, Clone, PartialEq, Eq`).
- Produces in the reader (`docx.rs`): per spec "Reading": run `w:color w:val` (`auto` or non-hex → None), `w:highlight w:val` (named table: yellow→FFFF00, green→00FF00, cyan→00FFFF, magenta→FF00FF, blue→0000FF, red→FF0000, darkBlue→000080, darkCyan→008080, darkGreen→008000, darkMagenta→800080, darkRed→800000, darkYellow→808000, darkGray→808080, lightGray→C0C0C0, black→000000, white→FFFFFF, `none`→None), `w:shd w:fill` on a run as highlight when no `w:highlight`, `w:u` (val other than `none`/`0`/`false`), `w:strike`/`w:dstrike` (val `0`/`false` off); heading colour from the heading style's `w:rPr/w:color` following `basedOn` (bounded 10, cycle-safe, in the same single styles pass); table cells keep their runs and `w:tcPr/w:shd w:fill`.

- [ ] **Step 1: Write failing tests** in `files_port.rs`: `rgb_from_hex_takes_exactly_six_hex_digits_in_any_case` (rejects `FF00`, `FF00000`, `GG0000`, `#FF0000`, `FF0000;x`, ``), `rgb_css_is_lowercase_hash_hex`. In `docx.rs`: `a_run_colour_is_kept_and_auto_is_none`, `a_hostile_colour_value_is_none`, `highlight_names_map_to_colours_and_none_means_none`, `a_run_shading_fill_counts_as_highlight_when_there_is_no_highlight`, `underline_and_strike_are_on_unless_val_is_none_zero_or_false`, `a_heading_style_colour_is_the_headings_colour_following_basedon`, `a_heading_style_with_themecolor_and_a_hex_val_uses_the_hex`, `a_table_cell_keeps_its_runs_with_their_formatting_and_its_fill`, `a_cell_fill_auto_is_none`, `a_basedon_cycle_in_heading_colour_terminates`; keep all existing 57B docx tests green (adjust them to the new shapes).
- [ ] **Step 2: Run** `cargo test -p forge-ports -p forge-adapter-files` — fail.
- [ ] **Step 3: Implement** the types, the reader changes, and the minimal consumer fixes so `cargo test --workspace --no-run` builds (the view draws plain text for now; Task 2 draws formatting).
- [ ] **Step 4: Run** `cargo test -p forge-ports -p forge-adapter-files -p forge-widgets` and `cargo test --workspace --no-run` — pass.
- [ ] **Step 5: Commit** `feat: docx runs, headings and table cells keep their colours and decorations`.

### Task 2: The view draws them

**Files:**
- Modify: `crates/forge-widgets/src/files_office.rs`, `crates/forge-shared/src/theme.css`.

**Interfaces:**
- Consumes: Task 1's `Rgb`, `TextRun`, `TableCell`, `DocBlock`.
- Produces: pure helpers `luminance(Rgb) -> f32` (relative luminance 0..1, sRGB-linearised), `luminance_class(Rgb) -> LumClass` (`Dark` below 0.12, `Light` above 0.88, else `Mid`), `text_on(fill: Rgb) -> Rgb` (black for luminance ≥ 0.5, else white), `run_style(&TextRun) -> Option<String>` (the inline style or None for a plain run; built only from `Rgb::css` and fixed keywords: `color`, `background-color`, `text-decoration`), `cell_style(&TableCell) -> Option<String>`.
- Behaviour: a coloured run is a `span` with `style` and `data-lum="dark|mid|light"`; `theme.css` resets the colour (`color: inherit !important`-style rule) for `data-lum="dark"` under the dark theme and for `data-lum="light"` under the light theme (follow how `theme.css` switches themes: `@media (prefers-color-scheme: dark)` plus `:root[data-theme]`); a run with a highlight also gets the `text_on(highlight)` colour unless it has its own colour that has contrast against the highlight (luminance difference ≥ 0.35); a cell with `fill` gets `background-color` and `text_on(fill)` for its runs unless they carry their own readable colour; plain runs produce no extra span; heading colour applies to the heading element with the same `data-lum` rule.

- [ ] **Step 1: Write failing tests**: `luminance_of_black_white_and_red`, `luminance_class_boundaries`, `text_on_a_light_fill_is_black_and_on_a_dark_fill_is_white`, `run_style_is_none_for_a_plain_run`, `run_style_builds_only_from_validated_values` (assert the exact string for red+underline+strike; assert no `;` or `url(` can appear from any `Rgb`), `a_highlighted_run_gets_a_readable_text_colour`, `a_cell_fill_style_and_its_text_colour`.
- [ ] **Step 2: Run** `cargo test -p forge-widgets` — fail.
- [ ] **Step 3: Implement** the helpers, the span/style rendering in `DocumentViewer` (paragraphs, list items, headings, table cells), and the CSS.
- [ ] **Step 4: Run** `cargo test -p forge-widgets -p forge-shared` and `cargo build -p forge-app` — pass.
- [ ] **Step 5: Commit** `feat: the document view draws colours, highlights, decorations and cell fills`.

### Task 3: Office tabs refresh when the file changes

**Files:**
- Modify: `crates/forge-widgets/src/files_state.rs` (and `files_kinds.rs` / `files_panel.rs` only if needed).

**Interfaces:**
- Consumes: the existing text-tab refresh (`refresh`, `refresh_all`, `reload`, `read_and_apply`, `Pending::Refresh`, the ticket/`replaces` rules, `FileVersion`, `stat`). First read how it works and whether `Sheet`/`Document`/`Slides`/`Pdf`/`Image` tabs have a stored version; the spec says: if Office tabs already refresh, add the proving test and change nothing.
- Produces: an open `Sheet`, `Document` or `Slides` tab is re-read when `stat` shows a different version than the one it was loaded at (called at the same moments the text tabs use: after a turn, on focus); the tab's kind is replaced while shown; a failed re-read keeps the old kind; no refresh while a read is in flight for that tab; the version is stored at load time for these kinds.

- [ ] **Step 1: Write failing tests** with the existing state-test style (pure logic of "should this tab refresh": version differs and no pending read; failure keeps the old view): `an_office_tab_with_a_changed_version_is_refreshed`, `an_unchanged_office_tab_is_left_alone`, `a_tab_with_a_read_in_flight_is_not_refreshed_again`, `a_failed_reread_keeps_the_old_view`.
- [ ] **Step 2: Run** `cargo test -p forge-widgets` — fail (or pass if it already works: then keep the tests and report).
- [ ] **Step 3: Implement** by extending the existing mechanism, not by adding a second one.
- [ ] **Step 4: Run** `cargo test -p forge-widgets -p forge-features` and `cargo build -p forge-app` — pass.
- [ ] **Step 5: Commit** `feat: an open Office tab reloads when its file changes`.

### Task 4: Samples, docs and the whole-workspace test

**Files:**
- Modify: `scripts/harness/add_office_samples.py` (the real `report.docx` gets: a red run, a yellow-highlighted run, an underlined run, a struck run, a blue heading colour, a table whose header cells have a dark-blue fill with white text and a light-yellow cell), `docs/superpowers/specs/2026-10-25-office-formatting-design.md` (`## As built`, `## Left`), `docs/superpowers/roadmap/README.md` (rows 57C), `docs/superpowers/roadmap/backlog.md` (`NEW-09` built).

- [ ] **Step 1: Run** `cargo test --workspace --no-fail-fast` and `cd sidecar && npx vitest run`; record failures other than the known round_trip test (there should be none).
- [ ] **Step 2: Write** the sample script changes and the docs from the code. The real-window check is done by the controller after this task and recorded in a follow-up commit.
- [ ] **Step 3: Commit** `docs: docx formatting and Office tab refresh as built`.
