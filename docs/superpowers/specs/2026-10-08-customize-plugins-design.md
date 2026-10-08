# Plan 30: Customize page, plugins and marketplaces (30A)

Date: 2026-10-08. Branch `plan-30`. Evidence: `docs/superpowers/roadmap/observations.md`, Round 13, and a spike of the bundled `claude` binary (below). Backlog entry: PG-02.

## Goal

Claude Code desktop has a "사용자 지정" (Customize) page with tabs for skills, connectors and plugins, each with "내 항목" (mine) and "탐색" (explore). Forge has only Settings tabs that switch things on and off. This plan adds the page and builds the **Plugins** tab in full: see what is installed, add a marketplace, find a plugin in it, install it, uninstall it.

Forge is an in-house app, so the catalogue is not Anthropic's directory. It is whatever marketplaces the person adds: a GitHub repository, a Git URL or a local path (for example the company's Git server). There is no default catalogue and no allow list (decided with the user on 2026-10-08: "일단은 제한하지말자"; a later plan may add one).

Success: with a marketplace repository added, a person finds one of its plugins on the Explore switch, installs it with one click, sees it under Mine grouped by that marketplace, turns it off and on, and uninstalls it; the same plugin shows in the terminal's Claude Code (`claude plugin list`) while it is installed, and a failed Git clone shows the Git message.

This is plan 30A. The Skills tab (30B) and the Connectors tab (30C) are later plans; until then those two tabs only link to Settings.

## What the real app does (seen, not assumed)

Round 13, one page, read only (nothing installed or removed):

- Page: heading, tabs Skills / 커넥터 / 플러그인, a switch 내 항목 / 탐색, search, filter, sort, a black `+ 추가` button with a menu that differs by tab.
- Plugins, 내 항목: groups "이 프로젝트에서" (with a project picker), "내가 만듦", "Anthropic 및 파트너 제공", each with a count. A row: icon, name, a blue badge `새로 생성` on recent ones, "<author> 작성 · description", a date, `⋮` with `끄기` and a red `제거`.
- Plugins `추가` menu: 마켓플레이스 추가, 마켓플레이스 관리, 플러그인 업로드, Claude로 만들기.
- 마켓플레이스 관리: "이 컴퓨터에서 Claude Code로 추가한 마켓플레이스입니다. 마켓플레이스를 제거하면 해당 플러그인도 함께 제거됩니다." Cards with name, GitHub repo, last update, `⋮`.
- 마켓플레이스 추가: two choices, "Anthropic 소스 탐색" and "저장소에서 추가" (GitHub repo or Git URL).
- Plugins, 탐색: a banner, a section "새 플러그인", two-column cards (icon, name, two lines, source, `+`), "가장 많이 설치된 플러그인".
- Detail (seen for skills, not for plugins): back link, icon, name, "<author> 작성 · <date> 업데이트됨", tabs 개요 and 구성 파일, a side card.
- The app shares the plugin state with the Claude Code CLI (`~/.claude/plugins/known_marketplaces.json`).

## Deviations (chosen, not seen)

1. **Catalogue**: only marketplaces the person adds; no "Anthropic 소스 탐색", no banner, no "체험", no "가장 많이 설치된". Reason: in-house use.
2. **Mine is grouped by marketplace**, not by "내가 만듦" / "Anthropic 및 파트너 제공". No "이 프로젝트에서" group: Forge installs with scope `user` only.
3. **The plugin detail screen is a guess** copied from the skill detail (no 구성 파일 tab; that is 30B). The real plugin detail was not opened.
4. **Add menu**: only "Add marketplace" and "Manage marketplaces". No upload, no "Claude로 만들기".
5. **No filter and no sort menus** (search only). They were not observed either.
6. **Skills and Connectors tabs** show a note and a button to Settings until 30B and 30C.
7. Uninstall asks for a confirmation that says other Claude Code installs lose the plugin too (the install is shared).
8. The page text is English, as the rest of Forge.

## Spike (done, throwaway files in the scratchpad)

The bundled binary `node_modules/@anthropic-ai/claude-agent-sdk-<platform>/claude` (version 2.1.283, the one the SDK runs) has `plugin install | uninstall | update | list`, and `plugin marketplace add | list | remove | update`. It does not need a `claude` on the PATH. With `CLAUDE_CONFIG_DIR` set to a temp directory and a local directory marketplace:

| Command | Result |
|---|---|
| `plugin marketplace add <dir>` | "Successfully added marketplace"; writes `plugins/known_marketplaces.json` and `extraKnownMarketplaces` in `settings.json` |
| `plugin marketplace list --json` | array of `{name, source, path, installLocation}` |
| `plugin install hello@spike-market` | scope user; writes `enabledPlugins: {"hello@spike-market": true}` and copies the plugin to `plugins/cache/<market>/<plugin>/<version>/` |
| `plugin list --json` | array of `{id, version, scope, enabled, installPath, installedAt, lastUpdated}` |
| `plugin list --available --json` | `{installed: [...], available: [...]}` (a plugin that is installed is not in `available`) |
| `plugin uninstall hello@spike-market` | removes it from `enabledPlugins` |
| `plugin marketplace remove spike-market` | list is `[]` again |

`CLAUDE_CONFIG_DIR` is honoured and the real `~/.claude` was not changed. The `available` entries' fields were not seen (the marketplace had its only plugin installed at that moment): the first task of the plan lists them with a second, uninstalled plugin.

Not tried: a GitHub or Git URL marketplace (network and credentials), `plugin update`, a marketplace whose clone fails (the message), the packaged app (macOS bundle, Windows).

## Design

### 1. Back end

Port `PluginManagerPort` in `forge-ports` (the existing `PluginRepositoryPort` stays and keeps feeding session start):

- `snapshot() -> PluginSnapshot { installed, available, marketplaces }`: runs `plugin list --available --json` and `plugin marketplace list --json`.
- `add_marketplace(source)`, `remove_marketplace(name)`, `update_marketplace(name)`.
- `install(id)`, `uninstall(id)`, `update(id)`, where `id` is `name@marketplace`.

Domain types in `forge-domain`: `PluginEntry` (id, name, marketplace, version, description, author, installed, install path), `Marketplace` (name, source kind, repo or path, last update). `PluginDefinition` (the on/off row) is kept; both are joined by the key `name@marketplace`.

Adapter `ClaudePluginCliAdapter`:

- runs the bundled `claude` with `CLAUDE_CONFIG_DIR=<claude_home>` (so `FORGE_CLAUDE_HOME` works for the harness);
- finds the binary in the sidecar's `node_modules` (`@anthropic-ai/claude-agent-sdk-<os>-<arch>[ -musl ]/claude[.exe]`); a missing file is `PluginManagerError::BinaryMissing`;
- runs write commands one at a time (a mutex in the adapter), so two commands never write the settings file together;
- time limits: 120 s for install, update and marketplace add or update; 20 s for the rest;
- a non-zero exit is `PluginManagerError::Cli(stderr)`, shown as it is. A timeout is its own kind.

Use case `PluginCatalogUseCase` in `forge-application` wraps the port. After a successful install it sets the plugin on in Forge's own config (the on/off state stays separate from Claude Code's `enabledPlugins`: the user accepted that the two may differ); after an uninstall it drops the plugin from that config.

### 2. Screen

- **Entry**: a "Customize" item in the sidebar under "+ New session" and Search. The main pane gets a third state, `Active::Customize`. Choosing a session or "+ New session" leaves the page.
- **Frame**: title, tabs `Skills` / `Connectors` / `Plugins`, the `Mine` / `Explore` switch, a search icon (filters by name and description), and `+ Add` (Add marketplace, Manage marketplaces). Skills and Connectors show a note and an "Open in Settings" button.
- **Mine**: one group per marketplace with a count. Row: icon, name, marketplace tag, one line of description, version, `⋮` (Turn off or Turn on; red Uninstall). A plugin that is off has an "Off" badge. Uninstall asks first ("Other Claude Code installs on this computer lose it too").
- **Detail** (click a row or card): back link, name, "<marketplace> · v<version>", description, author, install path; `Install` or `Uninstall` at the top right.
- **Explore**: per marketplace, a two-column card grid (icon, name, two lines, marketplace, `+`). Installed cards show a check. While one install runs its card shows a spinner and the other installs wait. No marketplace at all: "Add a marketplace to find plugins" and an `Add marketplace` button.
- **Add marketplace** dialog: one field ("owner/repo, a Git URL or a local path"), `Add`, `Cancel`; a spinner while it runs; a failure shows the CLI message under the field.
- **Manage marketplaces** dialog: cards (name, source, last update, `⋮` with Update and Remove), `Done`. Remove warns that the plugins of that marketplace go too.
- **Errors**: a dismissible bar at the top of the page with the CLI message.
- **Narrow window**: cards fall to one column by the Plan 26 width rules.

### 3. State and flow

- State in `forge-features` (`customize.rs`): tab, switch, search text, open detail, open dialog, the queue of writes, the error. Pure logic (grouping, filtering, transitions, the "Off" test) lives there and is tested without Dioxus.
- Opening the page and every finished write call `snapshot()` again; the list is never patched by hand.
- Writes run one at a time through a queue; the running id drives the spinner. A failure shows the bar and the snapshot is read again.

## Out of scope

Skills tab (30B), Connectors tab (30C), project-scope or local-scope installs, plugin upload, "Claude로 만들기", filter and sort menus, the "Anthropic 소스 탐색" choice, automatic update checks, an allow list of marketplace sources, asking for Git credentials inside the app, the plugin 구성 파일 tab.

## Testing

- Unit: the pure logic in `forge-features`; the use case with `MockPluginManagerPort` (install turns the plugin on in Forge's config, a failure leaves the config alone, writes are queued).
- Adapter: JSON parsing from real output captured in the spike, plus fixed strings for a non-zero exit, a timeout and a missing binary.
- Integration (needs the bundled `claude`; skipped when it is absent): temp `CLAUDE_CONFIG_DIR`, a local directory marketplace with two plugins; add, list available, install, list, uninstall, remove.
- Real window with `scripts/harness/` (fixture home): entry, the four dialogs and menus, install and uninstall of a plugin from a local marketplace, a failing add (bad path), a narrow window. IME and dark mode are left to the user.

## Risks

- The packaged app must contain the platform `claude` file. Task 1 of the plan checks the Plan 29 bundle on macOS; Windows needs a check on a real machine (the `.exe` path, `CLAUDE_CONFIG_DIR`).
- A marketplace on a private Git server depends on the person's own Git setup (SSH keys, credential helper). Forge only shows the error.
- The `available` JSON fields are not yet seen (see Spike).
- Uninstalling here removes the plugin from the terminal's Claude Code as well; the confirmation says so.
- The plugin detail screen is a guess (Deviation 3).
