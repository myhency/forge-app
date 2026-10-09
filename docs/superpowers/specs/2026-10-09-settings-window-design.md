# Plan 33: the Settings window, appearance and notifications (SET-01, SET-02, SET-03)

Date: 2026-10-09. Roadmap rows: SET-01, SET-02, SET-03 (`docs/superpowers/roadmap/backlog.md`). Seen in observation round 15 (`observations.md`).

## Goal

The Settings overlay looks and works like the desktop app's Settings window: a search field, grouped pages, an Appearance block with a theme, one font, text size, chat width and reduced motion, and switches for the Code notifications.

Success: each control changes the screen or the next notification at once, the choice survives a restart, and an old config file without the new fields loads with today's look and behaviour.

## Decisions made with the user

- Code themes are not a choice. Code highlighting follows the theme: one light and one dark.
- One font choice for the whole interface (interface, chat and code). Choices: System, OpenDyslexic (used only if installed; not bundled).
- Everything else follows the Claude Code desktop app as closely as Forge's features allow.

## SET-01: the window

- Left list: a search field on top, then groups. `Settings`: General (new), Claude Code (the existing Code tab). `This computer`: Connection, Model. `Customize`: Skills, Plugins, MCP servers. Existing tab content moves as is.
- Search filters the list by page name and by the titles of the rows on each page (a static table of row titles per page). An empty result says so.
- Opened by ⌘, and the sidebar footer as today. The window reopens on the last page shown (kept in memory for the run, not saved).
- The shortcut list button stays. Account menu (PG-08) is not part of this plan.

## SET-02: Appearance (General page, group "Appearance")

`AppConfig` gets an `appearance` struct, `#[serde(default, deserialize_with = "lenient")]`:

| Field | Values | Default |
| --- | --- | --- |
| `theme` | system, light, dark | system |
| `font` | system, opendyslexic | system |
| `text_size` | small, normal, large | normal |
| `chat_width` | narrow, normal, wide | normal |
| `reduce_motion` | system, reduced | system |

- The app root carries `data-theme`, `data-font`, `data-text-size`, `data-width`, `data-motion`. `theme.css` keys its variables on them: the dark variables apply for `data-theme="dark"` and, for `system`, under `prefers-color-scheme: dark`; `--chat-column` is 560 / 720 / 880px; chat text is 13 / 14 / 16px; `--font` and `--mono` follow the font choice (OpenDyslexic falls back to System when missing); `reduced` sets animation and transition durations to zero.
- Syntax highlighting (syntect, `forge-entities`) renders with a light and a dark theme; the dark one applies when the effective theme is dark. Highlight colours are classes or two class sets switched by the root attribute so no re-render is needed on a theme change.
- The attributes are set as soon as the config loads (a moment after the first paint), and updated by the `Settings` methods that also save the config.

## SET-03: Notifications

`AppConfig.notifications`, same lenient/default rules, all on by default: `reply_done`, `scheduled_task`, `code_notice`, `permission_request`. `AppConfig.attention_bounce` (bool, default off).

- General page, group "Notifications": four switches. Claude Code page, group "Sessions": "Bounce the Dock icon when attention is needed" (macOS only; hidden elsewhere).
- Rules (a pure function, tested): a notification is sent only when its switch is on and the person is not looking at that session (the window is unfocused, or another session is active).
  - reply_done: a session finished a turn.
  - permission_request: a session is waiting for approval (any session, not only routine runs).
  - scheduled_task: the existing routine notices (Plan 31) obey this switch.
  - code_notice: sent for notices Forge already raises for a session (for example a failed start); if the plan finds none beyond the routine ones, the switch is left out and the backlog row notes it.
- `registry_host` already watches session views for the routine case; the same watcher gains the rules above. A new method on `NotifyPort`, `request_attention()`, bounces the Dock icon (macOS: the window's user-attention request; elsewhere a no-op returning an error that is ignored).
- Not built: email, phone, cloud session, Dispatch and artifact notifications.

## Where it changes

`forge-domain` (config), `forge-features` (`settings.rs`, search table, a small appearance applier), `forge-widgets` (`settings_overlay.rs`, split by page), `forge-shared` (`theme.css`), `forge-entities` (highlight themes), `forge-ports`/`forge-adapter-notify` (attention), `forge-application` (`registry_host.rs`), `forge-app` (apply on start).

## Tests

- Unit: old config loads with defaults; fields round-trip; a bad value reads as the default; the search filter; the notification rules (switch off, session in view, window unfocused); the CSS has a rule for every attribute value.
- Real window (Quartz harness, `FORGE_CLAUDE_HOME` fixture): change each control and look; restart; trigger a reply in a background session and see the notification; the Dock bounce.

## Not in this plan

The other General rows (trusted folders, default browser, voice), the System and Developer pages, code theme choice, bundled fonts, the account menu.
