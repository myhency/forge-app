# Real `/mcp` screens (Claude Code 2.1.283, captured 2026-10-09)

Captured by `scripts/observe/claude_mcp_screen.py` with the real config folder (so it is logged in), in a trusted
folder, targeting the plugin server `plugin:context7:context7` that "needs authentication". Authenticate is never pressed.
A throwaway `CLAUDE_CONFIG_DIR` does not work: it has no login, so `claude` shows its theme and login screens.

- `01_input.bin` — start-up. The input box shows `❯  Try "…"`. There is **no** `? for shortcuts` in auto mode; the
  status line says `⏵⏵ auto mode on  (shift+tab to cycle)`. Other modes: `? for shortcuts` (default),
  `(shift+tab to cycle)` (accept edits, plan). Marks used: `? for shortcuts`, `shift+tab to cycle`.
- `02_list.bin` — after `/mcp` + Enter. `Manage MCP servers`, `10 servers`, groups, then rows such as
  `❯  ✔  claude.ai Claude Docs  8 tools` and `⚠  plugin:context7:context7  needs authentication`. The pointer `❯` is on the
  first row. A row `→ Show unused connectors` is part of the cycle. Names with spaces exist (`claude.ai Claude Docs`); the
  servers Forge lists have none. A plugin server shows as `plugin:<plugin>:<name>`.
- `02b_down.bin` — the output after one Down (`\x1b[B`): only the **changed rows** are drawn again
  (`claude.ai Claude Docs` without the pointer, `❯ claude.ai Google Drive`). So the screen must not be cleared between
  Down presses, and the last line with `❯` is the selection. Down wraps around at the end of the list.
- `02c_downs.bin` — all output of the Down presses from the first row to the `context7` row.
- `03_item.bin` — Enter on that row: `Plugin:context7:context7 MCP Server`, `Status: ⚠ needs authentication`,
  `Auth: ✘ not authenticated`, then the items `❯  1.  Authenticate` and `2.  Disable`. The lower-case word
  `authenticated` is on the screen too, so the mark is the capital `Authenticate`. A server that is signed in shows
  `Re-authenticate` instead (not captured).
- After Authenticate (waiting for the browser): `Authenticating with <name>…`, `A browser window will open for authentication`, the URL, `If the redirect page shows a connection error, paste the URL…`, `URL >`, and `Return here after authenticating in your browser. Press Esc to go back.`
- After a successful browser sign-in (Figma): the screen returns to the input box and shows `/mcp` with the line `Authentication successful. Connected to figma.` under it. The failure text is **not** captured.
- Output is cut anywhere, and Ink draws a frame in several writes: act on the screen only after it has been quiet
  for a moment (150 ms), or a half-drawn frame can make Forge press Down twice.
