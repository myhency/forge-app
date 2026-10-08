# Plan 28: file previews in the right panel

Date: 2026-10-08. Branch `plan-28`. Evidence: `docs/superpowers/roadmap/observations.md`, Round 12 (parts 1 to 4), and a spike of the web view (below).

## Goal

The Files panel of Claude Code desktop shows more than text and pictures. Forge shows text (an editor), markdown (rendered, `</>` for source), pictures, and a note for binary and too large files. This plan adds what the real app shows for csv, html, svg and pdf, gives the tree an icon per kind, and makes a path in the chat open the right place: any absolute path in a reply becomes a link, and a picture path opens a lightbox.

Success: the eight sample files of Round 12 look the way the real app shows them (the table under "What the real app does"), a path pasted in a reply is clickable, and a picture path opens the lightbox.

## What the real app does (seen, not assumed)

Opening a file from the Files tree (Round 12 part 1 and 2):

| File | Body | `</>` toggle |
|---|---|---|
| csv | table (grey header and cells, white gaps); only the first 500 rows, with the note "처음 500개 행만 표시합니다. 더 보려면 소스로 전환하세요."; wide tables scroll sideways | yes |
| html | rendered in the panel; inline scripts run; files next to it (images, css) do not load; links do nothing in the panel (the real app asks "외부 링크 열기") | yes |
| svg | drawn on a dark backdrop with zoom in, zoom out and fit buttons | yes |
| pdf | pages stacked on a dark backdrop | no |
| png, jpg, ... | the picture, centred | no |
| json, css, ipynb, other text | source with colours (an editor) | no |
| binary (aiff) | the "looks like binary" note with `Finder에서 보기` and `그래도 텍스트로 보기` | no |

The tree row icon follows the kind: a table for csv, a globe for html, a picture for png and svg, `</>` for code and json, a plain file for the rest.

A path in the chat (Round 12 part 3 and 4): any absolute path in a reply text is a blue underlined link, whether or not a tool named it. A click opens: csv and md in the Files panel; html, pdf and svg in the Browser pane; png in a lightbox (a dark overlay, the picture, the path as a caption, `×` at the top right, a click on the picture closes it, no zoom).

## Deviations (chosen, not seen)

1. **html, pdf and svg paths in the chat open in the Files panel**, not in the Browser pane, because Forge's Browser panel cannot show a page yet (Plan 21B: the page opens in the system browser). Agreed with the user on 2026-10-08. A later plan that can show pages inside the Browser panel moves these three.
2. PDF in the Files panel uses the web view's own PDF viewer. It brings its own floating zoom, fit and download buttons, which the real app's Files panel does not show. Kept: it costs nothing and is the only way to scroll and zoom a PDF without a PDF library.
3. HTML links do nothing (no "외부 링크 열기" dialog). A later plan can add the dialog.
4. The lightbox also closes on Esc (the real app was not tried with Esc).

## Spike (done, throwaway code removed)

In the real Forge window (macOS web view, the same engine as the app): a PDF as a `data:application/pdf;base64` URL draws in both `<embed type="application/pdf">` and `<iframe>`, with the viewer's buttons. An HTML file in `<iframe sandbox="allow-scripts" srcdoc>` and as a `data:text/html` iframe both run an inline script; a relative image does not load; a click on a link in it does nothing in the panel. A 10 MB limit (`READ_LIMIT`) keeps data URLs small enough. Not tried: a PDF with many pages scrolled to the end, and whether a link click opens the system browser behind the window.

## Design

### 1. File kind by extension

`forge-entities` gets one function, `preview_kind(path) -> PreviewKind`, over the file name: `Csv`, `Html`, `Svg`, `Pdf`, `Image` (png, jpg, jpeg, gif, webp, bmp, ico), `Markdown` (as `is_markdown` now), `Text` (everything else). It also feeds the tree icon (`icon_of(path)`).

Reading (`FilesPort::read`, adapter `LocalFiles`):

- `Pdf` is read as bytes: a new `FileContent::Pdf { bytes }` (and `TabKind::Pdf { data_url }` in the widget). Over `READ_LIMIT` it is `TooLarge` as now.
- `Svg` stops being an `Image`. It is read as text like `Csv` and `Html`, so its source can be shown and edited. The rendered view builds a `data:image/svg+xml;base64` URL from the current text.
- `Image` keeps `FileContent::Image` and no longer lists svg.

### 2. Rendered and source view

Markdown already has a "rendered unless in `source` set" rule (`files_state.rs`: `markdown`, `source`; `files_panel.rs`: `markdown_selected`, `source_on`). That rule is generalised: any `Csv`, `Html`, `Svg` or `Markdown` tab opens rendered, shows the `</>` button, and the editor only when the tab is in the source set. Search ("a markdown file opens rendered, which has no lines to find in: show its source") applies to all four.

New widget `FileRendered { kind, text, path }` in `files_kinds.rs` chooses the body:

- `Csv`: `forge-entities::csv::parse(text, 500) -> CsvTable { rows, total_rows, truncated }`, an RFC 4180 reader (quoted fields, doubled quotes, newlines inside quotes, CRLF, a BOM skipped). The first row is the header. A row shorter than the header is padded. Over 500 data rows the note shows above the table. The table uses the markdown table's colours.
- `Html`: `<iframe srcdoc sandbox="allow-scripts">`. No `allow-same-origin`, `allow-popups`, `allow-top-navigation`, `allow-forms`. White background, fills the panel.
- `Svg`: `<img src="data:image/svg+xml;base64,…">` in a scroll box on a dark backdrop, with three buttons (zoom in, zoom out, fit) that change a scale signal (step 1.25, range 0.1 to 8, fit = 1 and centred).
- `Pdf`: `<embed type="application/pdf" src="data:…">` filling the panel on a dark backdrop (no `</>`).
- `Markdown`: unchanged.

Edits in source mode re-render the rendered view from the tab text as now (markdown) and for the new kinds.

### 3. Tree icons

The tree rows (`files_panel.rs`) get a small leading icon from `icon_of`: `table` (csv), `globe` (html), `picture` (png, svg, other images), `code` (code files and json: any `Text` whose `language_of` is known, plus css), `doc` (markdown), `file` (the rest, pdf included, as in the real app). Plain inline SVG, 14 px, the muted text colour.

### 4. Chat links

`forge-entities/src/links.rs`. Today only an inline code span that names a file a tool touched becomes a link (`file-link` span with `data-path`). New rule, additional to that:

- A bare absolute path in prose (a text node, not inside `code`, `pre` or an existing link) becomes a `file-link`. A path starts with `/`, is not preceded by a letter, digit, `/`, `:` or `.`, has no space, and ends in a file extension of 1 to 8 letters or digits. Trailing `.`, `,`, `;`, `:`, `)`, `]`, `"`, `'` are cut off the link and stay as text. A path with `"`, `<`, `>` or `&` is never linked.
- No check that the file exists and no check against the tool list. A click goes through the existing `open_file` rule: a path outside the session folder, or a missing file, only sets the notice.
- Inline code spans keep the current known-path rule.

The click handler (`chat_window.rs`, `WATCH_FILE_LINKS`) is unchanged: it sends `data-path`.

### 5. Click routing and the lightbox

`Workspace::open_file(id, path)` first asks `preview_kind(path)`. For `Image` it sets a per-window `lightbox: Signal<Option<LightboxImage { path, data_url }>>` (reading the file through the same `FilesPort`) instead of requesting the Files panel; for everything else it is as now. The lightbox is not per session: it closes when the session changes.

New widget `Lightbox` mounted at the app root: a fixed, full-window dark overlay (0.6 black), the picture centred at most 80% of the window, the path as a white caption under it, a white `×` at the top right of the picture. Esc, a click on the overlay or the picture, and `×` close it. A file that cannot be read or is over the limit closes it and sets the notice.

## Out of scope

Browser-pane routing of html, pdf and svg paths; video; ipynb as a notebook; audio; the "외부 링크 열기" dialog; zoom in the lightbox; pdf annotation; opening several pictures with arrows; csv sorting, filtering, editing the table.

## Testing

- Unit: `preview_kind` and `icon_of` over file names (case, dotfiles, no extension); the csv parser (quotes, doubled quotes, newline in a field, CRLF, BOM, short rows, 500 row cut and `truncated`, empty file); link detection (a path in prose, trailing punctuation, `https://x/a.b`, inside `code` and `pre`, an existing link, a path with `&`); the adapter returns `Pdf` for `.pdf` and text for `.svg`.
- Widget tests as in 25A: `TabKind` for each kind; the source set toggles for csv, html and svg.
- Real window with `scripts/harness`: the Round 12 samples (copied into the fixture folder) for each kind; a reply that holds paths (a fixture transcript) for the link click and the lightbox. IME, dark mode and a long PDF scrolled to the end are left to the user.

## Risks

- The PDF viewer inside an `<embed>` may swallow wheel events from the panel on some sizes; checked in the real window.
- A large csv still parses all rows to count them. Counting stops at the limit of `READ_LIMIT` bytes already read, so the cost is bounded.
- Linking every absolute path in prose makes paths in explanations clickable that the user did not mean; the click only opens or shows a notice, and the real app does the same.
