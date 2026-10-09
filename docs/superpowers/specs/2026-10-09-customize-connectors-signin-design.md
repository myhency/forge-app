# Plan 30D: Connectors sign-in through a terminal dialog

Date: 2026-10-09. Branch `plan-30`. Evidence: the 30C spec `2026-10-09-customize-connectors-design.md`, the bundled `claude mcp --help` (no login command), the Agent SDK 0.3.283 typings (no MCP OAuth call; only the `Elicitation` hook), `TerminalPort` (`open`, `write`, `resize`, `close`), and the macOS Keychain entry `Claude Code-credentials` that the terminal's Claude Code also uses.

## Goal

A connector that "needs sign-in" (for example Figma, Sentry) can be signed in from its detail screen without the person typing anything. Forge opens a dialog with a terminal in it, starts `claude`, opens `/mcp`, picks the server and its "Authenticate" item. The CLI opens the browser, runs its own local callback and stores the token. Forge then checks the connection again.

Success: on a connector whose check says "needs sign-in", the button "Sign in" opens a dialog whose terminal reaches the browser step with no key typed by the person; after the browser sign-in, the detail screen shows "connected" and the "needs sign-in" tag in Mine is gone. If any step does not show the text it waits for, Forge stops sending keys, leaves the terminal in the dialog and says to go on there.

Decided with the user (2026-10-09):
- Approach B: drive the CLI's own `/mcp` in a terminal inside a dialog. Not A (OAuth written in Forge: discovery, dynamic client registration, PKCE, callback server, Keychain format), because the token store belongs to the CLI and would break with it.
- Keys are sent by Forge, matched to the screen: each step waits for text in the terminal output, then sends its keys (not a fixed delay). On a timeout it stops and leaves the terminal to the person.

## Design

### 1. Entry and terminal

- Detail screen of a connector whose last check answered `NeedsSignIn` shows "Sign in" beside "Check connection". Not in the Mine list.
- The Customize page has no dock (it replaces the chat view), and a terminal tab lives only as long as the `TerminalPanel` that owns it. So the terminal is shown in a **dialog**: "Sign in to <name>". The dialog holds a terminal view (the `TerminalTab` of the dock, reused), a status line, and a Close button.
- Working directory: the folder of the selected session; with no session, the home folder (project-scope servers depend on the folder).
- Closing the dialog ends the shell. No tab is left behind. While the dialog is open the rest of the page is behind its backdrop; the backdrop does nothing.

### 2. Sign-in driver (`forge-features`, pure)

- `SignInDriver` knows no terminal. `feed(output_bytes) -> Vec<Action>` where `Action` is `Send(bytes)`, `Done(Outcome)` or `Stop(reason)`. It strips ANSI colour and cursor codes, keeps a tail of the visible text, and joins output that arrives in pieces.
- Steps (the waited-for texts are fixed by the observation round, see Testing):
  1. shell prompt seen → `claude\r`
  2. Claude input box seen → `/mcp\r`
  3. server list with the connector's name seen → arrow keys down to its row → `\r`
  4. "Authenticate" item seen → `\r`
  5. no more keys; only watching the output.
- Each step has a time limit (15 s). On a timeout, a name not found, or a name found twice, the driver answers `Stop(reason)`. Forge shows "Go on in the terminal" in the detail screen and sends nothing more.
- Keys sent are only: `claude`, `/mcp`, arrow keys and Enter. The connector name and URL are never put into a shell line.

### 3. Done

- After step 4 the driver only reads. Success or failure text in the output ends it with `Done(Signed)` or `Done(Failed)` (texts from the observation round); a 5 minute limit ends it quietly with `Stop(TimedOut)`.
- `Signed`: the dialog's status line says "Signed in. Checking…", runs "Check connection" once by itself, and then shows its result; the detail screen and the "needs sign-in" tag in Mine follow the same result. `Failed`: the status line says "Sign-in did not finish. See the terminal above."
- `Stop(reason)`: the status line says "Go on in the terminal above." The terminal is a real shell session, so the person can type.
- "Check connection" stays usable on the detail screen at all times; pressing it after a hand-finished sign-in gives the same result.
- Closing the dialog first stops the driver and ends the shell; nothing is shown.

### 4. State and flow

- `customize_connectors.rs` gains `sign_in(name)`, `close_sign_in()` and a `SignInState` signal (`Idle`, `Running(step text)`, `Signed`, `NeedsHand(reason)`, `Failed`). The dialog `Dialog::SignInConnector(name)` reads it.
- The terminal view's `drive` loop (`terminal_panel.rs`) gets an optional *tap*: output bytes are given to the driver, and the `Send` actions it returns are written to the same shell. The dock's terminal passes no tap and is unchanged. The tap is a plain trait object of `forge-features`, so the widget needs no knowledge of connectors.
- A second "Sign in" press while the dialog is open does nothing.

## Out of scope

Sign out ("Clear authentication"); servers that ask for a client id; OAuth written in Forge (approach A); Windows; remote servers behind a proxy.

## Testing

- **Observation round before the plan's code:** start the real bundled `claude` in a pty, open `/mcp` with the harness home, record the screen text of each step, the arrow key behaviour and the success and failure texts, and save them as test fixtures. Real window: sign in to a "needs sign-in" server (Sentry) to the browser step; Figma needs the person to finish the browser part.
- Driver (fake output only): each step advances; ANSI codes inside the text; text split between pieces; each step's timeout; name missing and name twice; success, failure and 5 minute stop.
- State with mock ports: order button → dialog open → keys written; dialog closed first; automatic check after `Signed`; second press does nothing; no session → home folder.
- Secrets: nothing from the output is stored or logged.
- Not checked, to be written in "As built": Windows, a proxy, servers that need a client id.

## As built (2026-10-09)

Built as in the design, with these differences and findings. Plan: `docs/superpowers/plans/2026-10-09-customize-connectors-signin.md`.

- **Real screens differ from what the plan guessed** (captures in `crates/forge-features/tests/fixtures/sign_in/`, notes in `NOTES.md`, Claude Code 2.1.283):
  - The input box has no `? for shortcuts` in auto mode; it shows `(shift+tab to cycle)`. Both are marks.
  - After a Down key the CLI draws only the rows that changed, so the driver keeps the text between Down presses and reads the last line with the pointer; it acts only after 150 ms without output, and presses nothing more until something is drawn after a Down.
  - The CLI spaces the parts of a row with moves to a column (`CSI n G`), not spaces. `PlainText` reads `CSI n G` as a space. Found in the real window (the driver walked past the connector and handed over; fixed with a test of the exact bytes).
- **The shell starts `claude` by its absolute path** (`McpManagerPort::launch_line`, with `CLAUDE_CONFIG_DIR` when Forge runs against another Claude home), so a person with no `claude` on PATH is served too.
- **A throwaway `CLAUDE_CONFIG_DIR` cannot be used to observe or test the happy path**: it has no login, and `claude` shows its theme and login screens. The harness's fake Claude home has the same limit: in it the dialog hands over (the step for the input box times out). The real window check used the real Claude home with a throwaway Forge config.
- The sign-in button shows only for connectors that "Check connection" can check (user scope). Plugin, project and local connectors have no check, so no button (30C decision).
- The status line while running also says what to do when Forge does not see the end of the sign-in: press Close, then Check connection.

### Checked in the real window

Real Claude home, a temporary user connector `forge-signin-test` (`https://mcp.sentry.dev/mcp`, added and removed with `claude mcp`, nothing else touched):
- Check connection → "Needs sign-in…" and the "Sign in" button appears: pass.
- Sign in → the dialog opens, `claude` starts, `/mcp` opens, the pointer goes to the connector, its menu opens, Authenticate is pressed with no key typed by hand; the terminal shows "Authenticating with forge-signin-test… A browser window will open for authentication": pass.
- The first run failed (column jumps, see above): the dialog said "The connector was not found on Claude's list. Go on in the terminal above." and left the terminal alone, which is the designed hand-over: pass; the cause was fixed.
- Close ends the shell (no `claude` process of the dialog left) and the detail screen is usable: pass.

### Not checked

- A person who presses Esc in the CLI's "Authenticating" screen (goes back): the item screen returns with no error line, so Forge does not see an end and the dialog stays on "Signing in…" (the status line says to press Close, then Check connection).
- Folder trust prompt in a new folder (covered by a driver test, not by the real window), a double press of "Sign in" (covered by the guard in `sign_in_connector`, not pressed twice in the window), Windows, a proxy, servers that need a client id.

### Checked with a real sign-in (Figma, 2026-10-09)

Real Claude home, a user connector `figma` (`https://mcp.figma.com/mcp`), the person finished the Figma sign-in in the browser:
- Sign in → the dialog reached "Authenticating with figma… A browser window will open for authentication", with "Return here after authenticating in your browser. Press Esc to go back." below the URL: pass.
- After the browser sign-in the terminal printed `Authentication successful. Connected to figma.`: this is what `mark::SIGNED` (`Authentication successful`) matches. The dialog's status line went to "Connected" (the automatic check), and after Close the detail screen said "Connected", the "Sign in" button was gone: pass.
- The shell of the dialog ended with Close (the remaining `claude` process has no terminal and belongs to the app's own sidecar).

### Added after the real sign-in: the connector list follows changes made outside Forge

The list was read only when the Customize page opened, so a connector added, removed or signed in with the terminal's Claude Code did not show until the person left the page and came back (found when a connector added with `claude mcp add` was missing from an open Connectors tab). Now `Customize::refresh_connectors` reads the list from the files again (no `claude` call), when the person arrives on the Connectors tab (`refreshes_on_tab`) and when the Forge window gets focus back (a `focus` listener of the page). It skips while the first read is still on its way or an add or remove runs, and the newest refresh wins.
Checked in the real window: a connector added with the CLI while the page was open showed after Plugins → Connectors; another showed after going to Finder and back to Forge, with no click.

### Checked after the merge (2026-10-09): failure, proxy, client-id server, packaged app

- **A refused sign-in** does not print `Authentication failed`. The CLI brings the item screen back with the line `OAuth error: access_denied - User denied` and the Authenticate item again; a server without dynamic client registration brings it back with `SDK auth failed: Incompatible auth server: does not support dynamic client registration`. `mark::FAILED` first held guesses, so Forge stayed on "Signing in…" (found by driving the CLI and sending the callback of a refused consent with `curl`). `mark::FAILED` now starts with `OAuth error:` and `SDK auth failed:` (a driver test with both real lines).
- **Packaged app** (`node scripts/package/build.mjs`, `dist/Forge.app`, macOS arm64, real Claude home): the package holds the bundled `claude` under `Contents/Resources/runtime/sidecar/node_modules/@anthropic-ai/claude-agent-sdk-darwin-arm64/`. Check connection → Sign in → the dialog reached "Authenticating with …" with no key typed, and the callback of a refused consent (sent with `curl`) gave "Sign-in did not finish. See the terminal above." with `OAuth error: access_denied - User denied` in the terminal: pass. The harness finds the window of the dev build by the owner name `forge-app`; the packaged app's owner name is `ForgeApp`.
- **Proxy** (a local CONNECT proxy through `HTTPS_PROXY`, CLI level): `claude mcp get` through it said "Needs authentication" (the proxy log shows `CONNECT mcp.sentry.dev:443`), and `/mcp` Authenticate reached "Authenticating with …"; through a dead proxy `claude mcp get` says "Failed to connect", so Forge shows "Could not connect…" and no Sign in button. Not checked: a Forge launched from Finder, which does not get `HTTPS_PROXY` from the shell's rc files: the dialog's shell (the person's own shell) may have it while the checks Forge starts itself do not.
- **A server that needs a client id** (GitHub's MCP, `https://api.githubcopilot.com/mcp/`): `claude mcp get` says "Failed to connect", so Forge offers no Sign in button for it; in `/mcp` it shows as needing authentication and Authenticate gives the `SDK auth failed` line above. Such servers are added with the CLI's client id option and are out of scope.
