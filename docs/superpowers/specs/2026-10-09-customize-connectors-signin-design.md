# Plan 30D: Connectors sign-in through the dock terminal

Date: 2026-10-09. Branch `plan-30`. Evidence: the 30C spec `2026-10-09-customize-connectors-design.md`, the bundled `claude mcp --help` (no login command), the Agent SDK 0.3.283 typings (no MCP OAuth call; only the `Elicitation` hook), `TerminalPort` (`open`, `write`, `resize`, `close`), and the macOS Keychain entry `Claude Code-credentials` that the terminal's Claude Code also uses.

## Goal

A connector that "needs sign-in" (for example Figma, Sentry) can be signed in from its detail screen without the person typing anything. Forge opens a terminal tab in the dock, starts `claude`, opens `/mcp`, picks the server and its "Authenticate" item. The CLI opens the browser, runs its own local callback and stores the token. Forge then checks the connection again.

Success: on a connector whose check says "needs sign-in", the button "Sign in" opens a dock terminal that reaches the browser step with no key typed by the person; after the browser sign-in, the detail screen shows "connected" and the "needs sign-in" tag in Mine is gone. If any step does not show the text it waits for, Forge stops sending keys, leaves the terminal open and says to go on there.

Decided with the user (2026-10-09):
- Approach B: drive the CLI's own `/mcp` in a dock terminal. Not A (OAuth written in Forge: discovery, dynamic client registration, PKCE, callback server, Keychain format), because the token store belongs to the CLI and would break with it.
- Keys are sent by Forge, matched to the screen: each step waits for text in the terminal output, then sends its keys (not a fixed delay). On a timeout it stops and leaves the terminal to the person.

## Design

### 1. Entry and terminal

- Detail screen of a connector whose last check answered `NeedsSignIn` shows "Sign in" beside "Check connection". Not in the Mine list.
- The button opens a new terminal tab in the dock of the selected session and opens the dock. Working directory: the folder of the selected session; with no session, the home folder (project-scope servers depend on the folder).
- The tab stays until the person closes it. Forge never closes it, so the output can be read after the sign-in.

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
- `Signed`: the detail screen runs "Check connection" once by itself and updates the result line; Mine drops the "needs sign-in" tag. `Failed`: the detail screen says "Sign-in did not finish. See the terminal." The reason is already on the terminal.
- "Check connection" stays usable at all times; pressing it after a hand-finished sign-in gives the same result.
- If the tab is closed first, the driver stops and nothing is shown.

### 4. State and flow

- `customize_connectors.rs` gains `sign_in(name)`; a `SignInRun` per detail visit holds the terminal id and the driver. Output comes from the `TerminalSession` events already used by the terminal panel; the run takes a second reader through the same path the panel uses (to be settled in the plan: the panel must keep drawing the output).
- Leaving the detail screen does not stop the run; coming back shows its state. A second "Sign in" on the same connector while a run is going focuses its tab.

## Out of scope

Sign out ("Clear authentication"); servers that ask for a client id; OAuth written in Forge (approach A); Windows; remote servers behind a proxy.

## Testing

- **Observation round before the plan's code:** start the real bundled `claude` in a pty, open `/mcp` with the harness home, record the screen text of each step, the arrow key behaviour and the success and failure texts, and save them as test fixtures. Real window: sign in to a "needs sign-in" server (Sentry) to the browser step; Figma needs the person to finish the browser part.
- Driver (fake output only): each step advances; ANSI codes inside the text; text split between pieces; each step's timeout; name missing and name twice; success, failure and 5 minute stop.
- State with mock ports: order button → tab opened → keys written; tab closed first; automatic check after `Signed`; second press focuses the tab; no session → home folder.
- Secrets: nothing from the output is stored or logged.
- Not checked, to be written in "As built": Windows, a proxy, servers that need a client id.
