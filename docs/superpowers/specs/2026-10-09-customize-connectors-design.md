# Plan 30C: Customize page, Connectors tab

Date: 2026-10-09. Branch `plan-30`. Evidence: the 30A spec `2026-10-08-customize-plugins-design.md`, the 30B spec `2026-10-08-customize-skills-design.md`, and the bundled `claude mcp` help (read 2026-10-09). Backlog entry: PG-02. Connectors in Claude Code are MCP servers.

## Goal

The Customize page has a Connectors tab that only links to Settings. This plan builds it: see the connectors you have (Mine), open one to read its settings and check the connection, find connectors in plugins of the marketplaces you added (Explore), add a remote (http or sse) or local (stdio) connector from a form, and remove one.

Success: servers from `~/.claude.json`, the project and installed plugins show under Mine with their kind and source; a server opens to a detail screen; a user-scope server can be added from a form and then shows under Mine, and removed after a confirm; "Check connection" shows connected / failed / needs sign-in in one line; a plugin that bundles an MCP server of an added marketplace shows under Explore and its `+` installs that plugin; a bad form or a CLI error is shown with the reason.

Decided with the user (2026-10-09):
- Scope is option 2: remote servers (URL, headers) and local servers (command, args, env). OAuth sign-in inside the app is **not** built (a later plan, 30D). A server that needs sign-in shows "needs sign-in" and points to `/mcp` in the terminal.
- Add and remove touch **user scope only** (`-s user`). Servers of project scope, local scope, `.mcp.json` and plugins are listed read-only with a source badge.
- Approach A: writes go through the bundled `claude mcp`, reads come from the files (existing `ClaudeMcpAdapter`), connection status only on demand.
- As in 30A and 30B: no default catalogue, no allow list (in-house).

## Design

### 1. Back end

- `forge-domain`: `McpServerDefinition` gains a kind (`Stdio`, `Http`, `Sse`), a source (`User`, `Project`, `Local`, `Plugin(name)`), and the names (not values) of headers and env keys. Values stay out of the domain type; the detail screen shows them masked, and only the add form holds a value, until the CLI is called.
- `forge-ports`: `McpManagerPort` with `add(spec)`, `remove(name)`, `check(name) -> Connection` (`Connected`, `Failed(reason)`, `NeedsSignIn`). Same shape as `PluginManagerPort`; mockable.
- `forge-adapter-claude-plugins`: `mcp.rs` runs the bundled `claude` (the lookup and the `CLAUDE_CONFIG_DIR` rule from 30A are reused, no new crate). `add` runs `claude mcp add -s user -t <kind> [-H ..] [-e ..] <name> <url|command> [args]` with an argument array, never a shell; stdio args follow `--`. `remove` runs `claude mcp remove -s user <name>`. `check` runs `claude mcp get <name>` with a 15 s timeout and maps the output to `Connection`.
- `forge-adapter-claude-fs`: `ClaudeMcpAdapter` also returns kind and source, and reads the MCP servers a plugin bundles (its `.mcp.json` or `mcpServers` in its manifest). One server with an odd shape becomes an "unreadable" row; the rest still list.
- `forge-application`: `ConnectorCatalogUseCase`: Mine (registry + `enabled_mcp_servers`), Explore (bundled servers of plugins in added marketplaces that are not installed), add, remove, check. Remove also drops the name from `enabled_mcp_servers` under the shared `ConfigLock`, so adding the same name later does not bring back an old switch.
- Form validation lives in the use case: name not empty and not taken, name chars as the CLI accepts, URL parses as http or https, header as `Name: value`, env as `KEY=value`, command not empty.

### 2. Screen

- Tab `Connectors` replaces `SettingsNote`. Settings keeps its MCP servers tab.
- **Mine:** a list with name, kind, source badge, a Forge on/off switch (the one Settings has), and a "needs sign-in" or "unreadable" tag where it applies. Count in the group header.
- **Detail:** back link, name, kind and source; the URL or the command and args; header names and env keys with values masked; the switch; "Check connection" with its result line; Remove (user scope only, with a confirm dialog). Read-only servers show no Remove, and say where they come from.
- **Add:** `+ Add` menu with "Add connector". A dialog: name, kind (Remote http, Remote sse, Local). Remote: URL and a list of headers. Local: command, args, and a list of env rows. Errors show in the dialog.
- **Explore:** cards of bundled servers (name, plugin, marketplace, `+`); `+` installs the plugin as in 30B. With no marketplace added it says to add one in the Plugins tab.
- Long names, URLs and many rows follow the 30B lessons: own CSS classes, wrapped text, list rows `flex: none`.

### 3. State and flow

- `customize_connectors.rs` in `forge-features`, using `JobQueue` and `Latest` from `customize.rs`.
- Opening the tab reads the files and draws at once; no CLI is called.
- Add: validate, CLI, re-read, open the new connector's detail only if the person is still on the Connectors tab.
- Remove: confirm, CLI, drop from `enabled_mcp_servers`, re-read, back to the list.
- Check: runs only on the button, result kept only in the open detail.

### Secrets

Header and env values can hold tokens. They go only into the CLI argument array, are never logged, and are removed from any CLI error text shown or logged. The detail screen masks them.

## Out of scope

OAuth sign-in in the app; project / local / `.mcp.json` add or remove; editing an existing server (remove and add again); import from Claude Desktop (`add-from-claude-desktop`); add by JSON (`add-json`); a connector directory; Windows checking.

## Testing

- Adapter: a fake `claude` script records its arguments; check the exact array for each kind, and values with spaces and quotes. `check` output cases: connected, failed, needs sign-in, timeout.
- Registry: kind, source and bundled servers from fixture files; one bad entry does not hide the rest.
- Use case and state with mock ports: each validation rule, duplicate name, remove cleans `enabled_mcp_servers`, no jump to the new connector after leaving the tab, a missing bundled `claude` leaves Mine listing.
- Secrets: a test that a header value in a failing CLI's stderr does not reach the error shown.
- Real window with the real bundled `claude` and the harness home: add an http and a stdio connector, check connection, remove, Explore from the marketplace fixture; many rows and long text.
- Not checked, to be written in "As built": Windows, OAuth servers, a remote server behind a proxy.

## Risks

- `claude mcp get` output is text, not a stable format. The mapping is kept in one function with a fallback of "unknown", and tested with captured real output.
- A terminal Claude Code writing `~/.claude.json` at the same moment as our `claude mcp add`: both go through the CLI's own writer, so Forge never writes that file.
- A plugin-bundled server may use variables like `${CLAUDE_PLUGIN_ROOT}`; the detail shows them as written.

## As built (2026-10-09)

Built as designed, with these differences:

- **Reads.** `ClaudeMcpAdapter` (which starts sessions) is unchanged. The page reads through a new `ClaudeConnectorCatalogAdapter` (port `ConnectorCatalogPort`), as 30B did for skills. It lists user, project (`.mcp.json`) and local-scope servers by the scope that wins a name, plus the servers installed plugins bundle (`.mcp.json` and `mcpServers` of `plugin.json`, as an object or a path inside the plugin) and the ones plugins of added marketplaces offer. A bundled `.mcp.json` or a manifest `mcpServers` path that leaves its plugin folder through a link is skipped (the manifest file `plugin.json` itself is not link-checked).
- **Form.** Validation is a pure function in the domain (`validate_new`, `split_args`); the use case calls it. `Debug` of the draft and of the new connector prints counts, not values.
- **Secrets.** The CLI error text has the header and env values taken out (`redact`, longest first). The output of `claude mcp get` is reduced to its `Status:` and `Issue:` lines; the rest (which has the values in clear) is not kept.
- **`claude` file.** With `FORGE_CLAUDE_HOME` the bundled `claude` keeps `.claude.json` inside the config folder, so the connector read follows it (`cli_claude_json` in `main.rs`). With the real home it is `~/.claude.json`. Found by the real-window check. The session adapter still reads `<home>/.claude.json` in override mode, so servers added from the page do not reach sessions there (harness only).

Review Focus items, and the tests that pin them: values with spaces, quotes, Korean and a leading dash reach the CLI as one argument (adapter test `add_passes_every_value_as_exactly_one_argument`, `add_args` tests); an echoed value is not shown (`a_failure_that_echoes_a_value_does_not_show_it`); a taken name is refused before the CLI runs and a second add while one runs does nothing (`add_checks_the_form_first...`, `add_connector` busy guard); remove drops the Forge switch (`remove_runs_the_cli_and_drops_the_name...`); odd files do not hide the rest (`odd_files_do_not_hide_the_rest`, `a_broken_plugin_file_or_a_manifest_path_outside_the_plugin_is_skipped`).

### Checked in the real window (harness, bundled `claude`, macOS)

- Empty Connectors tab: pass. Add dialog (name, type switch, address, header rows with a password field): pass.
- Add an http connector with a header: it opens in its detail, the header shows `X-Key: ••••••`, the value is only in the CLI's file, the switch is on: pass.
- Check connection on `https://mcp.sentry.dev/mcp`: "Needs sign-in. Run /mcp in the terminal's Claude Code to sign in.": pass. Found and fixed: the result text collapsed into a one-character column and the button wrapped (CSS, `.customize-check`).
- Explore showed the connector of a plugin of the fixture marketplace; `+` installed the plugin and the connector moved to "From plugins" with a switch that cannot be pressed: pass.
- Remove (confirm dialog): the server left the CLI's file and the name left `enabled_mcp_servers`: pass.
- 30 connectors, a 50-character name, a 300-character address and an 80-character header name: rows keep their height, the long address is cut in the list and wraps in the detail: pass.
- Not checked in the window: a local connector with env typed in the form (checked by adapter tests and by hand with the CLI), a refused duplicate name or bad address in the dialog (unit tests), a project `.mcp.json` row (adapter test), leaving the tab while an add runs (unit test `a_new_connector_is_shown_only_to_a_person_who_is_still_on_the_connectors_tab`).
- Not checked at all: Windows, OAuth servers (sign-in itself), a remote server behind a proxy.

### After the final review (2026-10-09)

- "Check connection" is shown only for connectors added by you (`ConnectorSource::checkable`): `claude mcp get <name>` finds project and local servers only from inside the project, and knows a plugin's server as `plugin:<plugin>:<name>`, so from here it would answer about the wrong server or none.
- The Add button waits for the list (`taken_names_for_add`): without it a name used by a project or local connector could pass the duplicate check.
- Known and left as they are: removing a name switches it off in the shared, name-keyed `enabled_mcp_servers` for every project; a token inside a URL or in stdio args is shown as written (only header and env values are treated as secret); the list does not draw until the plugin catalogue has been read; "needs sign-in" shows in the detail after a check, not as a tag in the list.
