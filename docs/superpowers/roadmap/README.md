# Forge roadmap: features taken from Claude Code desktop

Plan 14 wrote this roadmap. It builds nothing. Later plans (15 and on) each take some entries from [backlog.md](backlog.md), build them, and mark them built.

Forge is the UI equivalent of the Claude Code desktop app (spec §1). Every entry here comes from looking at the installed app on 2026-10-03 ([observations.md](observations.md)), not from guessing. Parity is the goal, so an entry that was not seen is marked so, and its behaviour is looked at before it is planned.

## Files

| File | What it holds | Who edits it |
| --- | --- | --- |
| `backlog.md` | One row per feature: id, what, evidence, Forge status, size, plan | every plan, when it starts and when it finishes |
| `observations.md` | What the desktop app showed, step by step, with what is still unseen | every observation round |
| `README.md` (this file) | How the roadmap works; the tentative plan order | when the order changes |

Screenshots are not committed: they show the user's session titles, name and usage. Observations are text only.

## Status values

Evidence (column `Seen`):

- `seen`: opened in the installed app and the result was seen.
- `partly`: the control was seen but its contents or effect were not (a menu item with a submenu that was not opened).
- `inferred`: guessed from an icon or a name. Must be looked at before planning.

Forge status (column `Forge`):

- `none`: nothing in the code. Checked by reading `crates/` on 2026-10-03.
- `partial`: some of it exists; the cell says what.
- `has`: Forge already does it; listed only so the list is complete.

Work status (column `Plan`):

- `-`: not scheduled.
- `Plan N (planned)`: a plan document exists, nothing built.
- `Plan N (built)`: built and checked; the plan's "As built" section in the spec says how.
- `won't`: left out on purpose, with the reason in the Notes.

## How a later plan uses this

1. Pick entries from `backlog.md` (the user decides which; the tentative order below is only a suggestion).
2. **Observe first** for every pick that is `partly` or `inferred`, and for any behaviour the plan depends on (what a menu item does, what is shown after a click, which shortcut). Use `scripts/observe/claude_app.py` (see below). Add what was seen to `observations.md`, and change `Seen` to `seen`. Do not design from an icon.
3. Check the Forge status again in the code; the code may have moved since this was written.
4. Brainstorm, write the spec section and the plan as in plans 5 to 13 (`docs/superpowers/specs/`, `docs/superpowers/plans/`).
5. Set `Plan` to `Plan N (planned)` when the plan is written and `Plan N (built)` when it is done.
6. A feature found while observing goes into `backlog.md` as a new row with the next free id in its area. Ids are never reused or renumbered.

Sending a message in the desktop app uses the user's real account and changes their data. Ask before doing it, and do it only in a throwaway folder.

## Observing the desktop app

`scripts/observe/claude_app.py` captures and clicks the installed Claude window (`shot OUT.png`, `click X Y`). It is separate from `scripts/harness/`, which only drives Forge.

Things that cost time on 2026-10-03:

- The window is 1200 x 800 points and the capture is 2400 x 1600. The image viewer may show it smaller (it says by what factor); pass that factor to `click`.
- Layout moves after a click. Opening a panel shifts the toolbar icons left, and the "Dispatch" sidebar item switches the whole app to chat mode (a different sidebar). Take a capture after every click and click from that capture, never from an earlier one.
- Menus close with Escape.
- Never click Delete, Archive or send. Opening a menu is fine; choosing an item that changes data is not.

## Tentative plan order

The user may reorder. Sized as the smallest useful slice of each area.

| Plan | Theme | Entries | Why this place |
| --- | --- | --- | --- |
| 15 | Session menu | SES-01..06, SES-09 | Highest value, small; Forge already has rename and delete |
| 16 | Composer controls | CMP-03, CMP-04, CMP-05 | Model, effort and mode are changed often; needs sidecar and SDK facts first |
| 17 | Sidebar search and groups (built) | SES-07, SES-08, SES-11 | Grows with the number of sessions |
| 18 | Composer `+` menu (built, CMP-02 voice left out) | CMP-01, CMP-06 | Attachments are the largest piece; split if needed |
| 19 | Fork and effort (built) | SES-01, CMP-05, OTH-07 | The small items the user chose first |
| 20 | Archive (built) | SES-02, SES-06 key A, SES-08 status | Took Plan 20's place; the terminal moved to 21A |
| 21A | Dock, terminal, prevent sleep (built) | PNL-01, PNL-06 | First right-hand dock; later panels reuse it |
| 21B | Changes, files, preview panels | PNL-02, PNL-03, PNL-04 | Observation round 5 is done; read-only Files and a small Preview (spec `2026-10-03-right-dock-panels-design.md`) |
| 22 | Routines | PG-01 | Needs a scheduler that runs only while the app is open |
| 23 | Customize: explore | PG-02 | Needs a catalogue source |
| 24 | Home dashboard | PG-05 | Reads transcripts; no new port if usage already covers it |

Plan 13's earlier candidates (packaging the app, chat view extras, trash restore, split panes) and Plan 12's deferred minors (spec §23.8) are not in this table; they are in the backlog under `OTH`.
