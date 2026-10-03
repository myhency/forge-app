# Plan 20: archive a session (SES-02, SES-06 key A, SES-08 status filter)

Plan 19 left archive out (the user put it off). Plan 20 builds it. Evidence is in `docs/superpowers/roadmap/observations.md` (round 4: archived sessions show under the status filter "보관됨" with an archive icon; opened, they are read-only with a bar "보관 해제"). The menu item "보관" itself was never chosen in the real app.

## Scope, decided with the user on 2026-10-03

| Id | Decision | Why |
| --- | --- | --- |
| SES-02 archive and restore | build | Menu item Archive / Unarchive, a status filter, a read-only view with an Unarchive bar. |
| SES-06 key A | build | Matches the physical key `KeyA`, like R, F and D, so it works with a Korean input method on. |
| SES-08 status filter | build: Active (default), Archived, All | Seen in round 4. |
| SES-08 group by status, environment filter, PR state | not built | No need for the first; no data for the others. |
| Bulk delete of archived sessions, auto-archive | not built | Not seen in the real app. |

Assumptions (not seen in the real app): archiving does not move or change any file; an open session cannot be archived (same rule as delete); the UI text is English like the rest of Forge.

## Storage: `AppConfig.archived`

The user chose Forge's own config file. `AppConfig` gets `archived: Vec<ArchivedRef>` with `#[serde(default)]`, so an old config loads unchanged.

```rust
pub struct ArchivedRef { pub project: PathBuf, pub session_id: String }
```

The Claude transcripts and the fs adapter are not touched, so Claude Code (CLI) is unaffected and sees nothing. The state is visible in Forge only. Saved on each change through `ConfigRepositoryPort`, as `SidebarPrefs` is.

An id in the list whose transcript no longer exists (deleted outside Forge) is ignored when the sidebar is built. The list is not pruned (YAGNI). Deleting an archived session in Forge removes its entry.

## Sidebar status filter

`SidebarPrefs` gets `status: StatusFilter { Active (default), Archived, All }`, `#[serde(default)]`.

`arrange` stays a pure function (no I/O, no clock). Its input gets the archived set and the status filter, and it drops entries before grouping and sorting:

- Active: entries not in the archived set.
- Archived: only entries in the set.
- All: everything.

`SidebarEntry` gets `archived: bool`, set from the archived set. A row with `archived` draws a small archive icon. Empty-group rules (`show_empty`) apply after the status filter.

The filter menu gets a "Status" row (current value at the right, radio submenu of Active / Archived / All), above "Sort by". It reuses the keyboard and focus handling of the existing filter menu.

## Session menu: Archive / Unarchive, key A

`RowActions` gets `archive: ArchiveAction { Archive, Unarchive, Unavailable }`:

- Past session with a Claude id, not archived: `Archive`.
- Past session, archived: `Unarchive`.
- Open (live) session, or one with no Claude id: `Unavailable`.

The menu shows the matching item with the letter A. `menu_key_action("KeyA", ...)` returns `MenuKeyAction::ToggleArchive` unless `Unavailable`. It matches the physical key, as the other keys do.

Archiving in the default (Active) view makes the row disappear. If the archived session was the selected one, the selection is cleared back to the new-session screen. Unarchiving from the Archived view makes the row leave that view the same way.

## Opening an archived session

An archived session opens through the existing view-only path (`open` of a past session). The composer area is replaced by a bar: "This session is archived." with an "Unarchive" button. The button restores the session (removes the entry) and the composer returns as for any past session. Nothing is sent while the bar shows.

The search palette (⌘K) searches archived sessions too (search is not filtered by status). A hit opens the same read-only view.

## Where the code goes

- `forge-domain`: `ArchivedRef`, `AppConfig.archived`, `StatusFilter`, `SidebarPrefs.status`.
- `forge-features`: the status step in `arrange`, `SidebarEntry.archived`, `ArchiveAction` in `RowActions`, `MenuKeyAction::ToggleArchive`, `Workspace::archive_session` / `unarchive_session` (update the config, reload the sidebar).
- `forge-widgets`: the menu item and A key, the Status submenu, the archive icon, the Unarchive bar.
- No change to `forge-ports` `SessionRepositoryPort` or the Claude fs adapter.

## Tests and checks

Unit tests: `arrange` with each status value, including an archived id with no transcript; `menu_key_action` for `KeyA` (also with the Korean-IME flag), `RowActions` for past, archived, live and fresh sessions; config round trip, and an old config file without the new fields.

Then run the app against a `FORGE_CLAUDE_HOME` fixture and check with screenshots: archive a session by the menu and by A, switch the filter to Archived and All, open an archived session, press Unarchive.

## As-built notes

To be filled in when the plan is done: what differed from this spec, and which parts are assumptions not seen in the real app (menu item wording, the A key toggling, the bar's wording).
