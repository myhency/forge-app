# Fork from a message (NEW-18, part 1)

Plan 63A. Branch `plan-67` carries the work. Observation: round 32 (`docs/superpowers/roadmap/observations.md`).

## What the desktop app does

Hover a user message: `✓✓ time · copy ↺ fork`. The fork icon makes a new session at once, with no menu or confirmation:

- Title `<old title> (fork)`, same folder.
- The conversation holds everything **before** the clicked message. The clicked message's text sits in the composer. The original session is left whole.
- A grey line at the top of the chat: `포크 원본 <old title, a link> <time>` (`지금`, `3분 전`). The link opens the original.
- In the sidebar the fork nests under the original (`└`, and `▾` on the original). **Not part of this plan** (63B).

## Scope

Built: the fork icon on the message row, the composer text coming back, the `포크 원본` line.
Left for 63B: the nested sidebar row and its fold.
The existing whole-session fork (row menu, key F) is unchanged, but its copy carries `forkedFrom` too, so it gets the `포크 원본` line.

## What already exists

- `Registry::fork_before` / `SessionRepositoryPort::fork_session_before` cut a transcript before a message uuid and write the copy with `forkedFrom: {sessionId, messageUuid}` on every line (the immediate parent, also for a fork of a fork).
- `Workspace::fork_session_before` wraps it and opens the copy; nothing calls it yet.
- `LiveSessionHandle::replace_session(session, Some((text, attachments)))` shows a session and puts a message back in the box (what `↺` uses).

## Design

### 1. The icon

`forge-entities/src/turn.rs`: next to `↺`, a second button `⑂` (title and aria-label `Fork from this message`, Korean `이 메시지에서 포크`) when the message has a `uuid`. A new prop `on_fork: Option<EventHandler<String>>` on `Turns` / `TurnItem`, like `on_rewind`. `chat_window.rs` passes `workspace.fork_from_message(id, uuid)`.

### 2. The flow

`Workspace::fork_from_message(id, message_uuid)`:

1. Read the open session's project, Claude session id, title and the message (its text with cards, attachments). Nothing to fork when the id or message is missing: a notice `This message cannot be forked yet.`
2. `registry.fork_before(project, session, uuid, title)` makes the copy and returns its id. A failure is a notice; nothing opens.
3. Reload the project's session list, open the copy (`open_past`), then load the copy's session and call `handle.replace_session(session, Some((text, attachments)))` on its handle so the message is in the box.
4. The original stays as it is. A running turn does not matter: the cut only reads the transcript file.

The unused `Workspace::fork_session_before` is replaced by this function (same steps, plus step 3).

### 3. The `포크 원본` line

- `forge-domain` `Session` gets `forked_from: Option<SessionId>`. The claude-fs reader (`sessions.rs`) sets it from `forkedFrom.sessionId` on the first line that has one. Every place that builds a `Session` sets `None`.
- New widget `ForkLine { id }` in `forge-widgets`, rendered in `chat_window.rs` after `StartLine`. It looks the origin up in the workspace's past session list (same project): the title and the project summary. Shown only when the origin is found. It reads `포크 원본 <title as a link> <time>`; the time is the open session's `created_at` through `relative_time` (omitted when empty). A click calls `open_past_clicked(project, origin_id)`.
- English text: `Forked from`. Korean: `포크 원본`.

## Out of scope

Sidebar nesting and fold; `✓✓`; keeping Claude Code CLI forks (`/branch`) in sync.

## Tests

- claude-fs: `forked_from` is read (none, one fork, a fork of a fork carries the immediate parent).
- features: a pure helper picks the origin title and project from the past list (found, other project, missing).
- application: unchanged (`fork_before` and `replace_session` are covered).
- Real window (harness): fork a message of the second turn, check the new session shows the first turn only, the message text is in the box, the `포크 원본` line shows and its link opens the original; fork the first message (the copy is empty, the text is in the box).
