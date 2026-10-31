# Forge UI kit

Every interactive page starts with this kit already loaded, in light and dark. Write semantic markup with these
classes instead of writing your own CSS; add css only for what the kit lacks (a chart, a canvas, a custom shape).

## Design rules
1. One idea per page. A title (`fx-title`), one line of context (`fx-sub`), then the thing the person plays with.
2. Controls first, result big. Put the result in a `fx-stat` (large number) or a chart, close to the controls it changes, and update it on `input`, never behind a button.
3. Fill the width. Use `fx-grid` for side-by-side blocks; let the card height follow the content (never use `vh`).
4. Use the theme colours only: `var(--fg)`, `var(--muted)`, `var(--line)`, `var(--bg)`, `var(--surface)`, `var(--accent)`, and the chart colours `var(--c1)` to `var(--c6)`. Never hard-code white, black or grey.
5. Real data and sensible defaults: pre-fill every input so the first view is already meaningful; label units.
6. Keep the page short: at most about 40 lines of markup, 80 lines of script. Add a one-line note when numbers are illustrative.
7. A result worth acting on gets a button that calls `forge.send({label, values})` (only from a click).
8. Check the empty and extreme values (zero people, a blank field) so the page never shows NaN or Infinity.

## Layout
- `.fx-stack`: vertical stack with a gap. `.fx-row`: horizontal row that wraps; add `.fx-spread` to push ends apart.
- `.fx-grid`: responsive columns (min 150px); `.fx-grid.cols-2`, `.fx-grid.cols-3` fix the count.
- `.fx-card`: a bordered surface for a block of content.

## Text
- `.fx-title` (heading), `.fx-sub` (secondary line), `.fx-label` (small field caption), `.fx-muted` (grey text), `.fx-badge` (small pill).

## Controls
- `.fx-btn` (primary), `.fx-btn.ghost` (outline). `.fx-field` wraps a caption and a control; `.fx-value` shows its live value.
- `.fx-input`, `.fx-select`, `.fx-range` (a slider), `.fx-check` (a checkbox row: `<label class="fx-check"><input type="checkbox"><span>Text</span></label>`).

## Numbers and data
- `.fx-stat` with `.fx-stat-value` and `.fx-stat-label`: a big number tile.
- `.fx-table` (cells with class `num` align right), `.fx-bar` (progress: `<div class="fx-bar"><span style="width:40%"></span></div>`).
- `.fx-chart`: put on an inline `<svg viewBox>` so it scales. Colour series with `.fx-c1`, `.fx-c2`, `.fx-c3`, `.fx-c4`, `.fx-c5`, `.fx-c6` and `fill="currentColor"`/`stroke="currentColor"`.

## Guidance
- `.fx-callout` (a note; `.fx-callout.warn` for a caution), `.fx-steps` (an ordered list with numbered circles).

## Tabs (no script needed)
```html
<div class="fx-tabset">
  <div class="fx-tabs"><button class="fx-tab" data-tab="a">First</button><button class="fx-tab" data-tab="b">Second</button></div>
  <div class="fx-panel" data-panel="a">...</div>
  <div class="fx-panel" data-panel="b">...</div>
</div>
```
The first tab is open at the start.

## Libraries and media
Load three.js, MapLibre, Chart.js or D3 with a `<script src>` from cdnjs, jsdelivr or unpkg at the top of `html`. Maps: MapLibre with `https://tiles.openfreemap.org/styles/liberty`. Photos: Wikimedia Commons through `Special:FilePath`, always with `onerror` and a credit. Draw icons and illustrations as inline SVG.
