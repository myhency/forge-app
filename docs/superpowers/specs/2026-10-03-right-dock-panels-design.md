# Plan 21: the right-hand dock and its panels (PNL-01, PNL-06, PNL-02, PNL-03, PNL-04)

The README's order put the terminal panel in Plan 20 and Changes, Files and Preview in Plan 21. Plan 20 became archive, so the user asked on 2026-10-03 to do both in this plan. Evidence is in `docs/superpowers/roadmap/observations.md` (round 1: terminal; round 5: Changes, Preview, Files). Forge is the UI equivalent of the desktop app (spec §1), so the panels copy what was seen and mark what was not.

## Scope, decided with the user on 2026-10-03

| Id | Decision | Why |
| --- | --- | --- |
| Dock (columns right of the chat, toolbar toggles, expand, close) | build | Seen in round 5. Every panel needs it. |
| PNL-01 terminal | build | A shell in the session folder. pty in Rust, screen drawn by xterm.js. |
| PNL-06 prevent sleep | build | A per-session toggle in the title bar's `⋮` menu. |
| PNL-02 Changes | build, read only | File list and diff from `git`. No stage, revert or commit. |
| PNL-04 Files | build, read only | Tree, filter, `?` content search, viewer. The editor, auto save and tab size are not built. |
| PNL-03 Preview | build, small | Tabs, address bar, an `<iframe>`. Cookie import, element picker and the layout icon are not built. |
| PNL-05 iOS simulator | not built | macOS and Xcode only (backlog: won't). |

Two plans: **21A** the dock, terminal and prevent sleep; **21B** Changes, Files and Preview. 21B starts with a spike (below).

## Assumptions (not seen in the real app)

- Panel state (which panels are open, column widths) is kept per session. The real app was not checked on a session switch.
- The terminal gets its own column. Whether it stacks with Changes and Preview was not seen.
- The UI text is English like the rest of Forge. Strings that were seen are kept in the notes only.
- A panel with no folder (new-session screen) is not offered: the toolbar icons show only for a live session.

## The dock (Plan 21A)

State in `forge-features/dock.rs`, pure and unit tested:

```rust
pub enum Panel { Terminal, Changes, Preview, Files }
pub struct DockState { pub columns: Vec<Column>, pub expanded: Option<Panel> }
pub struct Column { pub panels: Vec<Panel>, pub width: f32 }   // Changes and Preview share one column
```

- `toggle(panel)`: opens it (a new column at the far right; Preview joins the Changes column when Changes is open, and the reverse) or closes it if open. Opening never closes another panel.
- `expand(panel)` fills the area right of the sidebar; `collapse()` returns. Closing the expanded panel collapses first.
- `resize(column, width)` clamps the width to 280–900 px; the chat column keeps a CSS `min-width`. Reuses the pointer-drag logic of the sidebar resize.
- One `DockState` per session id, held in the workspace state. It is not saved to disk (assumption above; YAGNI).

Widget `forge-widgets/dock.rs` draws the columns; each panel has a header with its own title/controls, `⋮`, expand and close. The toolbar icons (terminal, Changes, Preview) and the `⋮` menu (Files ⇧⌘F, Prevent sleep) live in the chat header (`chat_window.rs`) and show their open state. The chat column narrows as columns open.

The `pages` layer lays out sidebar, chat and dock; no logic there.

## Terminal and prevent sleep (Plan 21A)

Port `TerminalPort` in `forge-ports`: `open(id, cwd, cols, rows)`, `write(id, bytes)`, `resize(id, cols, rows)`, `close(id)`, plus an output stream of `(id, bytes)` and an exit event. Adapter `forge-adapter-terminal` uses `portable-pty` and starts the user's login shell (`$SHELL`, falling back to `/bin/zsh`) in the session folder, with `TERM=xterm-256color`.

- Tabs: `+` opens another shell in the same panel; closing the last tab closes the panel. Each tab is one pty, killed on tab close and on app exit.
- Screen: a vendored `xterm.js` and its fit addon, served from the app's assets (no network). Rust sends output chunks to it and receives keystrokes and size changes through the Dioxus JS bridge. The scrollback and selection are xterm's.
- Folder missing: no pty is started; the panel shows `Working directory "<path>" does not exist` (seen in round 1).
- A shell that exits shows "Process exited (<code>)" and a "Restart" button in the tab.

Prevent sleep: port `SleepPort` (`hold(id)`, `release(id)`), adapter runs `caffeinate -i` as a child and kills it on release or when the session is closed or the app exits. Off by default, per session, not saved. A failure to start shows a toast and leaves the toggle off.

## Changes (Plan 21B)

Port `GitPort`: `status(cwd)` and `diff(cwd, path)`. The adapter runs `git` with a timeout and `--no-optional-locks`, read only. `status` parses `--porcelain=v1 -z`; `diff` returns the unified diff of the working tree against `HEAD` (untracked files are shown as all added).

- Left: file list (path, status letter, +/-), toggled by the list icon. Right: the diff of the selected file, drawn with the existing diff view in `forge-entities` (`diff_view.rs`).
- Not a git repository, or no changes: "No changes to show". `git` missing or failing: the message with the first line of stderr.
- `⋮` has "Refresh". The panel also refreshes when a tool call in the session finishes (the session already emits events).
- Large diffs: stop at a fixed line limit with "Diff too large to show".

## Files (Plan 21B)

Port `FilesPort`: `list(dir)`, `read(path, limit)`, `search(root, query, limit)`. The adapter skips `.git` and `node_modules`, caps the read at 1 MB and treats a file with a NUL byte as binary ("Cannot show this file").

- Tree with lazy expanding folders (a pure `FileTree` model in `forge-features`, with the filter), a filter field focused on open, and `?query` for a content search whose results are file and line rows. ⇧⌘Y toggles the tree. Empty folder: "Folder is empty".
- The viewer shows the file with the existing highlighter (`highlight.rs`), read only, line numbers.
- Paths stay inside the session folder: a path that resolves outside it (`..`, symlink) is refused in the adapter.

## Preview (Plan 21B)

Tabs ("New tab", `+`), back, forward, reload, a URL field, drawn in `forge-widgets/preview_panel.rs`; the page itself is an `<iframe>`. A URL without a scheme gets `http://` for `localhost` and `https://` otherwise.

- **Spike first.** Dioxus 0.7 desktop cannot easily host a second webview, so `<iframe>` is the plan. The first task of 21B loads `http://localhost` and an external site in an `<iframe>` inside the real app. If `localhost` does not work, the preview is reduced to "Open in browser" and the spec is amended before more work.
- A site that refuses to be framed cannot be detected reliably from an `<iframe>`; the panel keeps an "Open in browser" button in the tab so the user can leave it.
- History (back, forward) is the tab's own list of URLs held in `forge-features` (pure).
- No cookie import, no element picker.

## Where the code goes

| Layer | New |
| --- | --- |
| `forge-features` | `dock.rs`, `file_tree.rs`, `git_status.rs` (parse), `preview_tabs.rs`, workspace wiring |
| `forge-ports` | `terminal_port.rs`, `sleep_port.rs`, `git_port.rs`, `files_port.rs` (with mocks) |
| adapters | `forge-adapter-terminal` (portable-pty), `forge-adapter-sleep`, `forge-adapter-git`, `forge-adapter-files` |
| `forge-widgets` | `dock.rs`, `terminal_panel.rs`, `changes_panel.rs`, `files_panel.rs`, `preview_panel.rs`; toolbar in `chat_window.rs` |
| `forge-app` | composition of the new adapters; the `xterm.js` assets |

## Tests and checks

- Unit: dock state (toggle, join/split a column, expand, clamp), `git status` parsing (renames, untracked, spaces), `FileTree` (expand, filter, `?` query), preview history.
- Adapters on a temp directory and a temp git repo: terminal runs `echo` and the output arrives, resize and close; sleep hold/release kills the child; git status and diff of a changed, added and untracked file; files list, binary and large file refusal, path escape refused.
- The visual harness (`scripts/harness`, `FORGE_CLAUDE_HOME` fixture): open each panel, expand, close, resize, a changed file in a temp repo shows in Changes, a file opens in Files, `localhost` loads in Preview, the terminal runs a command. Compare against the round 5 notes.
- Observe again after building: Changes with real changes, Files with real files, Preview with a loaded page, whether the terminal stacks. Record in `observations.md` and fix the assumptions above.

## Not built (to the backlog)

Files editor, auto save and its options; Changes stage/revert/commit; Preview cookie import, element picker, layout icon; the terminal's stacking rule; saving panel layout across restarts.

## As built (Plan 21A)

Built as designed: the dock (`forge-features/dock.rs`, `forge-widgets/dock.rs`), `TerminalPort` with `forge-adapter-terminal` (`portable-pty`, login shell), `SleepPort` with `forge-adapter-sleep` (`caffeinate -i -w <pid>`), xterm.js 5.5.0 and addon-fit 0.10.0 vendored under `crates/forge-shared/assets` and put in the window head with `Config::with_custom_head`, `SessionTools` in the chat header. Plan: `docs/superpowers/plans/2026-10-03-dock-terminal-sleep.md`.

Differences from the design:

- Width: a fixed 280 to 900 px clamp; the chat column keeps a CSS minimum.
- The dock is not drawn for an archived session, and its terminal icon is hidden there (the real app's archived toolbar has no terminal icon).
- The page sends the shell a new size only 150 ms after the last change. Without it a drag made zsh redraw its prompt at every step (found in the harness).
- Leaving a session ends its shells and returning opens a fresh one; the dock layout and the Prevent sleep switch are kept per session. This is a limit of the chat being unmounted when it is not active, not a decision.
- The `⋮` menu stays open after a click on the switch.
- Found by the final review and fixed: typed text goes to a writer thread (a write to a pty blocks while the shell's program reads nothing, and the caller is the UI thread); the reader thread gives the pty back when the shell is over; output that is already queued reaches the page as one batch (`TerminalSession::recv_batch`, up to 64 KB). The output channel is still unbounded. Nine minor findings are not fixed (zombie shell after a background job, keys typed in a new tab's first 150 ms, a drag stuck after a release outside the window, and others).

Checked in the harness (`scripts/harness`, `my-app` and the missing `my_app`): open and close, a command runs in the session folder, drag resize, tabs, expand and collapse, `exit 3` and Restart, the missing-folder message, Prevent sleep on and off with `pgrep`, switching sessions, closing a session and quitting leave no child process. Plan 21B (Changes, Files, Preview) is next.
