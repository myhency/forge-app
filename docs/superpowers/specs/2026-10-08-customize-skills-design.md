# Plan 30B: Customize page, Skills tab

Date: 2026-10-08. Branch `plan-30`. Evidence: `docs/superpowers/roadmap/observations.md`, Round 13 (Skills lines), and the 30A spec `2026-10-08-customize-plugins-design.md`. Backlog entry: PG-02.

## Goal

The Customize page (30A) has a Skills tab that only links to Settings. This plan builds it like Claude Code desktop: see the skills you have (Mine), open one to read its files, find skills in the marketplaces you added (Explore), add a skill by uploading a folder or zip, or create an empty one.

Success: skills from `~/.claude/skills`, the project's `.claude/skills` and installed plugins show under Mine in two groups; a skill opens to a detail screen with its files; a skill of an uninstalled marketplace plugin shows under Explore and its `+` installs that plugin; a folder or zip with a `SKILL.md` is added from the `+ Add` menu and then shows under Mine; a bad folder or zip is refused with the reason.

Decided with the user (2026-10-08): scope is "Mine + detail + create/upload + Explore" (option 3); upload takes a folder or a zip; Explore is built from the marketplaces the person added, not from Anthropic's directory (in-house, as in 30A: no default catalogue, no allow list).

## What the real app does (seen, not assumed)

From Round 13:

- Mine has two groups with a count: made by me, and from Anthropic and partners. A row: icon, name, a tag (the plugin it came from), the source, the description cut to one line, the date; a row of the second group has a `⋮` (menu not opened).
- Detail: back link, icon, name, a line "made by · from plugin · updated date"; tabs Overview and Files. Overview: description, and a side card (creator, plugin, connectors and tools with a warning). Files: a version menu, a download icon, a file tree, and the selected file with a rendered / source switch.
- Explore: a banner, a section of new skills with cards (icon, name, two lines of text, source line, `+`), more sections below.
- `+ Add` menu on Skills: upload skill, create skill, create with Claude, screen recording.

## Deviations (chosen, not seen)

1. **Groups** are "Made by me" (user and project folders) and "From plugins". There is no Anthropic group.
2. **Explore** has no banner and no "most installed" section: one list of cards of skills in uninstalled plugins of added marketplaces. `+` installs the skill's plugin (a skill cannot be installed alone). With no marketplace added it says to add one in the Plugins tab.
3. **Detail** has no "connectors and tools" card (Forge cannot tell what a skill talks to), no version menu and no download icon.
4. **Switch on a plugin skill** is disabled with the note "turned on and off with its plugin"; a user or project skill uses `enabled_skills`.
5. Not built: create with Claude, screen recording, the row `⋮` menu, filter and sort, the dates.
6. **Create** makes a `SKILL.md` template and opens the detail on the Files tab with a "Show in folder" button (`Workspace::reveal`). The Files panel (25A) cannot be used: it only opens files inside a session's folder, and this page has no session. There is no editor inside this page.

## Design

### 1. Back end

- **Domain** (`forge-domain`, new `skill_catalog.rs`): `SkillEntry { id, name, description, origin, installed, path }`; `SkillOrigin` is `User`, `Project` or `Plugin { plugin_id }`. `id` is the origin plus the folder name, so two skills of the same name in different places stay apart. `SkillSnapshot { skills }`.
- **Port** (`forge-ports`, new `SkillCatalogPort`): `snapshot(project_path, plugins)` read-only; `add_from_folder(path)`, `add_from_zip(path)`, `create(name, description)` write to `<claude_home>/skills/<name>/`; `list_files(skill_path)` and `read_file(skill_path, relative)` for the detail.
- **Adapter** (`forge-adapter-claude-fs`, new `skill_catalog.rs`) walks three places and reuses the front-matter parser of `skills.rs` (moved to a shared function, not copied): `<claude_home>/skills` and `<project>/.claude/skills`; `install_path/skills/*/SKILL.md` of installed plugins; `<claude_home>/plugins/marketplaces/<name>/` for the plugins a marketplace offers but are not installed (the plugin's folder comes from the marketplace's `marketplace.json` `source`, only when it is a relative path; a plugin that points at another Git repo has no readable skills and is skipped). An unreadable folder is skipped, the rest continue. `SkillRepositoryPort` and `ActiveExtensions::select` are not touched: a session starts exactly as before.
- **Add**: refuse when the source has no `SKILL.md` with a `name`, when the folder `<claude_home>/skills/<name>` exists (never overwrite), or when a name has `/`, `\` or `..`. A zip is read with the `zip` crate; refuse an entry whose path leaves the folder (zip-slip), a symlink entry, more than 500 entries or more than 20 MB unpacked. A folder copy skips symlinks. A half-written folder is removed on failure.
- **Application** (`forge-application`, new use case `skill_catalog`): joins the snapshot with the plugin list and the config's `enabled_skills`; the install from Explore calls the existing plugin install use case.

### 2. Screen

New `crates/forge-widgets/src/customize_skills.rs`, shaped like `customize_plugins.rs`; the Skills tab of `customize_page.rs` stops showing the Settings note.

- **Mine**: the two groups with counts, rows with name, origin tag, one-line description, the switch; search field filters by name and description.
- **Explore**: two-column cards, `+` per card, the empty state above.
- **Detail**: back link "Skills", name, origin line; tabs Overview (description, side card with origin and plugin) and Files (tree, file view, rendered / source switch for `.md` using the 28 preview code). Opening a skill of an uninstalled plugin shows the same detail with an Install button instead of the switch.
- **+ Add menu**: Upload skill (system picker for a folder, or for a zip), Create skill (dialog: name, description).

### 3. State and flow

`customize_state.rs` gets a skills section: snapshot, search text, selected skill, selected file, view (rendered or source), busy flag, last error. Load on opening the tab and after every add, create or plugin install. Errors show in the page banner the way 30A shows them.

## Out of scope

Create with Claude, screen recording, row `⋮` menu, filter and sort menus, dates, Anthropic's directory, banner and "most installed" sections, connector card, version menu, download, an editor inside the page, skills of plugins that live in another Git repo, deleting a skill.

## Testing

- Adapter, with temp folders: skills found in all three places with the right origin; same name in two places kept apart; broken front matter skipped; a marketplace plugin with a Git source skipped; an unreadable folder skipped.
- Add: a good folder and a good zip; no `SKILL.md`; existing name refused; zip-slip, symlink entry, too many entries and too large refused; nothing left behind after a failure.
- State: grouping, search, the disabled switch for plugin skills, Explore `+` calls the plugin install, errors reach the banner.
- Real window, as in 30A: `FORGE_CLAUDE_HOME` fixture plus `scripts/harness/add_market_fixture.py` (extend it so a fixture plugin carries a skill), then upload a folder and a zip, create, open the detail, install from Explore. Not checked: Windows paths for zip, a marketplace that needs credentials.

## Risks

- `marketplace.json` `source` has several forms; only the relative-path form is read. Skills of the others do not show under Explore until the plugin is installed. Say so in the empty text of a plugin card, not silently.
- A plugin's skill folder may sit outside `skills/` (a plugin can set its own path in `plugin.json`); the first version reads only `skills/`.
- Same skill name in user and project folders: the list shows both, `enabled_skills` is by name, so one switch turns both on, as today.

## As built (2026-10-08)

Built as the plan says, in 11 tasks (commits `4391814..` on branch `plan-30`): domain types, `SkillCatalogPort`, `ClaudeSkillCatalogAdapter` (scan, add from folder or zip, create, list and read files), `SkillCatalogUseCase`, pure page logic, the `Customize` state, the Skills tab (`customize_skills.rs`), the Create dialog, a harness fixture for uploads.

Changes made while building:
- **Show in folder** replaces "Edit in Files panel" (deviation 6): the Files panel only opens files inside a session folder.
- The page asks for the skills right after the plugins, in one read, from the plugin list it has just got. When the bundled `claude` fails, the skills are still read against an empty plugin list (found in review: user and project skills would have been hidden).
- A skill switch shows its new state at once and is confirmed by the next read.
- Added skills open on their Files tab once the lists are fresh; a visit counter drops the request if the person left the page and came back.
- The Create fields turn auto-capitalisation off; a first letter is no longer upper-cased by the system.

Checked in the real window (harness fixture home, macOS): Mine groups and counts; switch writes `enabled_skills`; the detail Overview and Files with Rendered (no front matter) and Source; Explore cards from a local marketplace and `+` installing the plugin (card leaves Explore, skill appears under "From plugins" with a disabled switch); Create refusing `../x` and creating a skill; folder upload through the system picker, a second upload of the same folder refused with files unchanged; zip upload with one top folder, no temp folder left; a 640 pt wide window (the tree stays beside the file view; it fits).

Not checked: cancelling a picker and the "Choose a .zip file." message in the window (unit-level only), Windows (zip picker, paths, device names), a marketplace that needs credentials.

Long content (checked in the window): a skill with 400 sections, a file of 20,000 lines, a file of one 200,000-character line, a file over 512 KB and a binary file. The file box scrolls inside itself (420 px high). The first version showed rendered Markdown through the Files panel's `.file-viewer` class, whose `min-width: max-content` kept paragraphs from wrapping, and plain text clipped long lines; both now use their own classes (`skill-rendered`, `skill-code`) that wrap, with code blocks and tables scrolling sideways inside the page. A file over 512 KB shows "This file is too large to show." and a binary file "This is not a text file."

Minors from the final review, all done after it (with tests where a test can show them):
- Names: a trailing `.` and Windows device names (`con`, `nul`, `aux`, `prn`, `com1`-`com9`, `lpt1`-`lpt9`, also with an extension) are refused.
- A folder whose `SKILL.md` is a link, or is over 512 KB, is refused.
- `.forge-upload-*` folders left by a crashed zip add are swept when the adapter starts.
- `list_files` and `read_file` work only on a folder that holds a `SKILL.md` of its own.
- One plugin entry without a name (or not an object) no longer hides the rest of its marketplace.
- A skill folder inside a marketplace that is a link to somewhere outside its plugin folder is not listed.
- A successful upload or create no longer pulls the person back to the Skills tab if they went elsewhere meanwhile (no unit test: window state).
- If a later read of the skills fails, the old list is dropped and the tab says so, with Try again.

Left as is, with the reason: no guard that resets the busy flag if the add task panics (a panic in a Dioxus task ends the app, so there is nothing left to reset; not verified by making one happen).
