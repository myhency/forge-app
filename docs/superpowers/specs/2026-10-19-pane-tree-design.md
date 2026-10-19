# Pane tree: up to four panes, side by side and stacked (OTH-04, second step)

Date: 2026-10-19. Branch `plan-52`. Evidence: observation round 20, its addendum, and "Pane limit" (`docs/superpowers/roadmap/observations.md`, OTH-04). Builds on Plan 51 (`2026-10-19-split-panes-design.md`, "As built").

## Goal

Forge shows up to four sessions in the main area, laid out the way the desktop app does: side by side and stacked, in a tree of splits. Plan 51 stopped at two panes side by side.

Success, checked in the real window:
- `⌃⌘-` opens a New-session pane below the focused pane; `⌃⌘\` opens one to its right (as today).
- ⌥+click on a row, and a plain click while split, add a pane by one rule (below), up to four. The fourth gives a 2×2 grid.
- At four panes, ⌥+click and a plain click replace the session of the pane that was opened last.
- The `⋮` menu moves a pane (side by side to stacked and back, swap with the neighbour).
- Vertical and horizontal dividers drag. × closes a pane and its space goes to the neighbour.
- With one pane, and with two side by side, nothing looks or behaves differently from Plan 51.

## Decided with the user (2026-10-19)

- A plain click while split adds a pane like the desktop app (it replaced the focused pane in Plan 51). With one pane it still replaces that pane.
- The desktop app's limit is four panes (observed: a fifth ⌥+click replaced the last-opened pane). Forge uses the same limit and the same replacement.
- Still not in this step: saved split, keys that move focus between panes, side chat (`⌘;`), drag of a row into a pane.

## What the desktop app does (round 20 and "Pane limit")

- ⌥+click adds a pane. With two side by side and the focus in the right one, a plain click on a third row put the new pane **under the left pane**. Four ⌥+clicks gave a 2×2 grid: top left, top right, bottom left, bottom right.
- `⌃⌘\` opens a New-session pane to the right of the focused pane, `⌃⌘-` below it. A second `⌃⌘\` with the New-session pane already focused does nothing.
- The `⋮` menu labels follow the layout: side by side → "move split view left", "move split view down"; stacked → "move split view up", "move split view to the right"; both have "close split view".
- Closing a pane gives its space to the neighbour. A narrow pane header shows the title, `⋮`, `×`.

## Design

### 1. State (`forge-features`, `panes.rs`)

`Panes` stays a pure type with no Dioxus, tested first. Its inside changes from a list of targets plus one ratio to a binary tree:

```rust
enum Node { Leaf(Active), Split { dir: Dir, ratio: f32, first: Box<Node>, second: Box<Node> } }
enum Dir { Beside /* first left, second right */, Below /* first top, second bottom */ }
pub struct Panes { root: Node, focused: usize, opened: Vec<Active> /* oldest first */ }
```

- At most `MAX_PANES = 4` leaves. Leaves are numbered in reading order (a leaf in `first` before one in `second`); `focused` is such an index, as today.
- The existing reads keep their meaning: `len`, `is_split`, `focused`, `focused_target`, `target(i)`, `targets()`, `pane_of`. `targets()` returns the leaves in reading order, so `Workspace::pane_targets`, the sidebar highlight and `Active` readers need no change.
- `show(target)`: unchanged rule (replace the focused leaf; a target shown elsewhere takes the focus instead), **except** that while split a plain click goes through `add` below. The caller picks: `Workspace::select` and the sidebar call `show_or_add`; Home, Customize, Routines, New session, Back and Forward keep `show` (they change what the focused pane shows, the way the desktop app does).
- `add(target, area)`: a target already shown takes focus. Otherwise: at `MAX_PANES` leaves, replace the leaf in `opened.last()` and focus it. Else pick the leaf with the largest area (ties: the first in reading order), and split it along its longer side (wider than tall: `Beside`; else `Below`), the old content first, `target` second; the new leaf gets focus. If that leaf would drop under the minimum size, try the other leaves in the same order; if none fits, replace as at the limit. This is the observed order: 2 side by side → under the left pane → under the right pane = 2×2; 2 stacked → beside the first.
- `split_focused(dir, target, area)`: `⌃⌘\` is `split_focused(Beside, NewSession)`, `⌃⌘-` is `split_focused(Below, NewSession)`. The new leaf is the second child and takes focus. A target already shown takes focus instead. When the focused leaf has no room, or four panes are open, nothing happens (the desktop app refuses the fifth).
- `close(i)`: remove the leaf; its sibling takes the parent's place and keeps its own subtree. Focus goes to the nearest leaf in reading order (as Plan 51: pane 0 when two were open).
- `move_pane(i, dir)`: the `⋮` items. `Beside`/`Below` on a pane: if it is half of a split of the other direction, turn that split to `dir`; if it already has `dir`, swap it with its sibling ("move split view left/up" is a swap). Focus stays on the same content. Unit tests pin each label's effect on a two-pane layout, the case the desktop app shows; deeper trees use the same rule on the pane's parent split.
- `opened`: the order panes were opened in. `add`, `split_focused` and `replace` keep it; closing removes the target; `show` does not change it.
- `replace(from, to)`, `remove_target`, `focus(i)`: as today, on leaves.
- `rects(area_w, area_h) -> Vec<Rect>` (one per leaf, reading order, pixel `x, y, w, h`) and `dividers(area_w, area_h) -> Vec<Divider>` (`{ path, dir, x, y, length }`): the whole geometry in one pure place. Splits keep their ratio; the minimum sizes are applied here, so every pane gets at least `MIN_PANE_WIDTH` (420) and `MIN_PANE_HEIGHT` (260, the compact header plus a composer; a constant, adjustable after the window check).
- `set_ratio(path, ratio, area_w, area_h)`: drag of the divider at `path` (the sequence of first/second steps from the root). The ratio is clamped so both sides of that split keep their minimum.
- `can_hold(area_w, area_h) -> usize`: how many panes fit at the minimum sizes. Used to refuse a split and to fold (section 4).

`Workspace` replaces its width-only methods: `set_pane_ratio(path, r)`, `pane_rects()` (derived from `main_width` and a new `main_height` signal, set from the window height the way `main_width` is set from the window width), `open_session_beside` and `open_new_session_beside` become `add_session` and `split_new_session(dir)`; `swap_panes` becomes `move_pane(index, dir)`. `main_width`, `pane_targets`, `focused_pane`, `focus_pane`, `close_pane`, `show_started` and `open_past_beside` keep their names.

### 2. Opening

- Sidebar row: ⌥+click and, while split, a plain click call `add_session(id)` (a past session, `add_past`). With one pane a plain click is `select` as before. Search palette results and "Open" paths keep using `select`.
- `⌃⌘-` joins the shortcut table and the shortcut list as "New session below" next to `⌃⌘\`.
- A session closed (archive and close, ⌥⌘A) while shown: its pane closes if split, as today.

### 3. Drawing (`forge-pages/src/workspace_page.rs`, `forge-widgets`)

- `pane_row` draws a flat list: one `div.pane` per shown leaf and one `div.pane-divider` per divider, **absolutely positioned** from `rects` and `dividers` inside a `position: relative` `.panes` box. A flat keyed list keeps a pane mounted when the tree around it changes (a split turning stacked, a sibling closing), which a nested flex tree would remount. Panes are keyed by their screen, as now.
- Each pane carries its width and height; the dock decision uses the pane's own width (`pane_layout(pane_width, dock_width)` as today).
- Dividers: a vertical bar between side-by-side panes and a horizontal bar between stacked ones. The existing drag state (`pane_dragging`, move and up on the workspace root) keeps one dragging divider path; the move handler turns the pointer position into a ratio for that split's own rectangle.
- A pane shows the compact header under the existing compact threshold, by width. A pane that is short but wide keeps its toolbar.
- The pane `×` and `⋮` items stay in `SessionTools` and the page-level `pane_close`; the `⋮` menu's two move items follow the layout as in the desktop app (labels from `Panes`), "Close split view" stays.
- The focused pane keeps its 1 px outline when split (any count above one).

### 4. Narrow areas

- The main area shows as many panes as fit at the minimum sizes (`can_hold`). When fewer fit than are open, only the focused pane shows and the rest keep their targets and return when the window grows (Plan 51's rule, now by count: under 840 px wide Forge shows one pane, as before). A layout of three or four panes that does not fit shows the focused one only; no partial grids.
- `add` and `split_focused` use `can_hold` with the room that would remain, so a split that would not fit is refused or turns into a replace.

### 5. Preview and web selectors

Unchanged from Plan 51: the native Preview web view is drawn only in the focused pane, other panes show their tab bar and the line "Click to show the page". Selectors that find `.pane.focused` keep working. Scripts that name a pane use its index, and pane indexes now run 0..4 in reading order.

### 6. Out of scope

Saved split, focus keys between panes, side chat (`⌘;`), dragging a row into a pane, double click to reset a divider, more than four panes, a layout other than a grid of splits.

## Testing

- `Panes` unit tests, written first: the add order for 1 to 4 panes (2×2 from the observed path), a stacked pair adds beside, the limit replaces the last-opened pane and focuses it, a shown target only moves focus, `split_focused` for both directions and its refusals (no room, four panes, target already shown), `close` for every leaf of a 2×2 and of a 3-pane layout, `move_pane` labels on a pair, `rects` and `dividers` for 1, 2, 3 and 4 panes (sums equal the area, no overlap), minimum sizes, `set_ratio` clamps, `opened` bookkeeping, `can_hold`.
- Sidebar highlight test for three and four panes; `Workspace` history with panes (Back/Forward keep `show`).
- Real window by the harness (`scripts/harness/{click.py --option,resize.py}`): ⌥+click to a 2×2, `⌃⌘-`, a horizontal divider drag, close from a grid, the fifth click, a narrow window folding to one pane, light and dark. Korean typing is left to the user.

## Risks

- Remounting: a pane that remounts loses its unsent text and scroll. The flat keyed list is the guard; the window check closes a pane next to another and splits again, and watches the composer text.
- The web view: the focused pane's surface rect must follow every geometry change (a divider drag, a move, a close). Checked in the real window as in Plan 51.
- A vertical size now matters. `main_height` is new state; if the window height is wrong at first render the minimum-height refusals would fire early. The first render uses the measured height of the window (the same source as the width) and the window check resizes the window.
- The `move_pane` rule on deeper trees is an extension of what the desktop app shows (two panes). It is unit-tested; a surprise in a three-pane layout would be a note in the plan's "left" list, not a blocker.
