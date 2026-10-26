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
| 21A | Dock, terminal (built; its prevent sleep was removed 2026-10-09) | PNL-01 | First right-hand dock; later panels reuse it |
| 21B | Changes, files, preview panels | PNL-02, PNL-03, PNL-04 | Observation round 5 is done; read-only Files and a small Preview (spec `2026-10-03-right-dock-panels-design.md`) |
| 22 | Routines | PG-01 | Needs a scheduler that runs only while the app is open |
| 23A | Chat view, static look (built) | new (observation round 7) | Spec `2026-10-04-chat-view-static-design.md`: ordered message blocks, folded tool activity, end card, markdown |
| 23B | Chat view, running look (built) | new | Spinner, live tool labels, stop button, permission and question cards |
| 23C | Chat view, side panels and suggestions (built) | new | Spec `2026-10-04-chat-view-panels-design.md`: open files from the chat, markdown render and source switch, background-task pill and panel, suggested next prompt |
| later | Customize: explore | PG-02 | Needs a catalogue source; moved back at the user's request |
| 30A | Customize page, Plugins tab and marketplaces (built) | PG-02 | Observation round 13; catalogue is the marketplaces the person adds (in-house); spec `2026-10-08-customize-plugins-design.md`; 30B Skills and 30C Connectors follow |
| 30B | Customize Skills tab: Mine, detail with files, Explore, upload folder or zip, create (built) | PG-02 | Spec `2026-10-08-customize-skills-design.md`; Explore is the skills of plugins in added marketplaces; 30C Connectors follows |
| 30C | Customize Connectors tab: Mine, detail with connection check, Explore, add remote or local, remove (built, checked in the real window) | PG-02 | Spec `2026-10-09-customize-connectors-design.md`; add and remove are user scope only, through the bundled `claude mcp`; OAuth sign-in in the app is 30D (built) |
| 30D | Connector sign-in: a dialog with a terminal that Forge types `/mcp` keys into, then checks the connection (built, checked in the real window up to the browser step) | PG-02 | Spec `2026-10-09-customize-connectors-signin-design.md`; approach B (the CLI owns the token); success and failure texts of the CLI are not captured, see "As built" |
| 31 | Routines (built) | PG-01 | Spec `2026-10-09-routines-design.md`; observation round 14; a timer inside Forge, run history, approval wait, Forge's own storage |
| 43 | Trusted folders (built, checked in the real window) | SET-15 | Spec `2026-10-16-trusted-folders-design.md`; the list in Settings, a question when an unknown folder is added to a session; the default browser row is left out |
| 45 | Finished tasks, tokens and failed workflows (built, checked in the real window) | OTH-10 | Spec `2026-10-17-finished-tasks-design.md`; observation round 18; `Finished N` fold from the session record, tokens, red dots, stopped and failed lines, a pill that reopens the panel; amber dots left out |
| 48 | Changes panel parity (built) | PNL-02 | Spec 2026-10-17-changes-panel-design.md |
| 24 | Home dashboard and new-session composer (built) | PG-05, CMP-06 | Needed a new port `UsageStatsPort`; observation round 6 |
| 25A | Files panel as an editor (built, checked in the real window) | PNL-04 | CodeMirror bundle, auto save and conflict banner, tabs; 25B line comments and 25C search and right click items next |
| 26 | Responsive layout (built) | UI-01 | The user asked for it before any new feature; observation round 10 |
| 28 | File previews (built) | PNL-04 | Spec `2026-10-08-file-previews-design.md`: csv, html, svg and pdf in the Files panel, tree icons, absolute path links, picture lightbox; observation round 12 |
| 56 | Plan mode (56A built, checked in the real window; 56B comments and sidebar tooltip left) | NEW-01 | Observation round 27: plan panel with the rendered plan, `수락 / 수정 / 수락 및 자동 모드` card, plan file row; today Forge shows only the generic ExitPlanMode permission card |
| 56B | Plan mode, step 2 (built 2026-10-26: comments on selected plan text, sidebar tooltip) | NEW-01 | The desktop app's comment-on-selection in the plan panel and the sidebar `플랜 준비됨` tooltip |
| 57 | Files for office work (Plan 57A and 57B built, checked in the real window) | NEW-02 | Round 28: xlsx viewer (grid, formula bar, sheet tabs), result paths in chat as links, `결과물` card (OTH-14) |
| 57C | Office formatting (built, window check pending) | NEW-09 | Round 30: docx runs keep colour, highlight, underline, strike (the red changed numbers), heading colours, table cell formatting; refresh an open Office tab when the file changes |
| 57D | Office rendering (built 2026-10-26: docx pages and pptx slide pictures, window checked) | NEW-10 | Round 30: pages and slide images like the desktop app (a bundled renderer; spike first: docx-preview style HTML for docx, a pptx renderer with charts) |
| 58 | Composer | NEW-03, NEW-04, NEW-11 | 58A built (cards and the `@` file mention), window check pending; 58B built 2026-10-26 (queue card, slash list, `/compact` line, `/btw`, `/rewind`; spec `2026-10-26-queue-slash-compact-btw-rewind-design.md`) |
| 59 | Status look (built 2026-10-26 except pin, read aloud, end card buttons, task live output) | NEW-05, NEW-06, NEW-07 | Rounds 27 and 28: stop shown as stopped (not failed or done), generated session title, details panel opens on first send with branch row and fuller context rows, background task row, message action row, end card buttons |
| 60 | Small fixes (built 2026-10-26 except the narrow-window panels) | NEW-08 | pycache and binary entries in Changes, repeated path in the permission card, file card look, the Dioxus Copy value warning, panels covering the chat in a narrow window |

Plan 13's earlier candidates (packaging the app, chat view extras, trash restore, split panes) and Plan 12's deferred minors (spec §23.8) are not in this table; they are in the backlog under `OTH`.
