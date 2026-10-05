# Right click menus for files, and Go to line (Plan 25C-2)

Last piece of the Files panel work (25A editor, 25B excerpts and comments, 25C-1 search and find bar). Source: observation round 9 part 5 (`docs/superpowers/roadmap/observations.md`): the tree row menu, the tab menu, the editor menu's `Go to line`. Branch `plan-25C2` (from `master`, which has 25B and 25C-1).

## Scope, decided with the user on 2026-10-05

- In: the tree row and tab menus with their items and submenus, `Open in`, `Show in file tree`, `Go to line`.
- Out: moving the panel's state (tabs, unsaved edits) into `Workspace` (a later plan, 25D); keyboard navigation inside the menus; items that need a Forge feature that does not exist.
- Labels are English like the rest of Forge.

## Menus (forge-features, forge-widgets)

- `MenuEntry` = `Action { label, shortcut, action, enabled }` | `Submenu { label, entries }` | `Divider`. `file_menu(target)` builds the list; `MenuTarget` = `Tree` | `Tab { preview: bool }`.
- Tree row, top to bottom: `Attach as context`; `Copy >` (`Absolute path`, `Relative path`, `File name`); divider; `Open in >` (`VS Code`, divider, `Default app`); `Show in Finder`; divider; `Show in file tree`; divider; `Find in file` (`⌘F`), `Copy file contents`, `Reload file`; divider; `Close file`.
- Tab: `Attach as context`; `Copy >`; divider; `Show in file tree`; divider; `Keep open` (grey unless the tab is a preview tab), `Close file`, `Close others`, `Close all`.
- `FileAction` names each action; the panel runs it. One widget, `ContextMenu`, draws any entry list at a point: backdrop click or Escape closes, a submenu opens to the right and flips to the left near the window edge, the menu is kept inside the window (`place(...)` is a pure function with tests). It replaces the tab menu and the tree row menu written in 25B.

## Actions

- **Copy** `Absolute path`, `Relative path`, `File name`: to the clipboard through the existing `navigator.clipboard.writeText` call (moved to a shared function that `turn.rs` also uses). `Copy file contents`: the file on disk (not unsaved edits), refused above the read limit with a notice.
- **Open in:** `BrowserPort::open_with(path, app: Option<&str>)`; the adapter runs `open -a "Visual Studio Code" PATH` or `open PATH`; the path must be absolute and is one argument, never a command line; failure (no VS Code, no program) becomes a notice.
- **Show in Finder:** the existing `BrowserPort::reveal`.
- **Show in file tree:** `FileTree::reveal_dirs(path)` gives the ancestor folders to open; the panel reads the ones not loaded, opens them, clears the filter, turns the tree on if it was off, selects the row and scrolls it into view.
- **Find in file:** the find bar of the tab with an empty query (`find` with `query: ""`).
- **Reload file:** the `Refresh` question (a tab with no edits reads the disk again; a tab with edits gets the existing conflict banner with Cancel and Overwrite), never a silent discard.
- **Close file / Keep open / Close others / Close all:** the existing tab actions.

## Go to line (tools/editor-bundle)

- `Go to line` (shortcut label `⌃G`) in the editor right click menu, and the `Ctrl-g` key. A one field panel above the editor (like the find bar): Enter jumps to that line (a number past the end goes to the last line, a non number or 0 is ignored and keeps the panel), Escape closes, the field has focus on opening.

## Tests

`file_menu` (order, dividers, submenus, `Keep open` enabled only for a preview tab); `place`; `reveal_dirs`; `open_with` in the adapter (program `true` and `false`, a relative path refused before any program runs, the path passed as one argument); jsdom for `Go to line` (jump, past the end, non number, Escape, `Ctrl-g`); then a real window check with the harness.
