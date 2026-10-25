# Plan 58B: queued messages, slash list, /compact line, /btw, /rewind (as built)

Built on `plan-58` after 58A. Written after the fact; the user asked to start without questions.

## What was built

- **Queue (NEW-03).** While a turn runs, ⌘↵ (Ctrl on Windows) keeps the message instead of sending it. The composer hint while running is `⏎ 키로 보내기, ⌘⏎ 키로 대기열에 추가하여 나중에 보내기`. A card above the box says `메시지 N개 대기 중`, one row per message with send now (↑), edit (✎, puts the text and files back in the box) and remove (×). The session task (`live_session_actor.rs`) owns the list (`LiveSessionView.queued`) and sends the front one when a turn completes, so it works with the session off screen. With no turn running ⌘↵ sends at once. Stopping a turn also ends it, so a queued message is sent after a stop (a choice; the desktop app's behaviour was not observed).
- **/compact line.** The sidecar forwards the SDK's `compact_boundary` as a `compacted` event; the transcript reader turns a `compact_boundary` line into the same. `Block::Compacted { before, after }` shows as a divider `대화를 압축했습니다 · 21.7k → 4.3k 토큰`.
- **Slash list.** `arrange_commands` puts Forge's own commands first, then the desktop app's usual names in its order (`btw ultrareview rewind fork subtask mcp plugin feedback model effort fast plan permissions`), then the rest as the SDK gave them. Forge's own: `/btw`, `/rewind`.
- **/btw.** `/btw question` is not sent. The sidecar calls the SDK's `Query.askSideQuestion` (present in 0.3.295, missing from `sdk.d.ts`, so typed locally in `session.ts`); it answers from the conversation and writes nothing to it. The answer shows in a closable card above the box. Typing `/btw` with no question does nothing. In the SDK, `/btw` as plain text answers "isn't available in this environment".
- **/rewind.** `/rewind` opens a dialog: pick an earlier user message, see `파일 N개가 바뀝니다 (+x −y)` (a dry run), then `파일 되돌리기` (files back as they were before that message), `대화 열기` (a new session holding the conversation before it; this session stays) or `둘 다`. The sidecar starts the SDK with `enableFileCheckpointing: true` and gives every user message it sends a uuid (`user_message` event), which is the id `Query.rewindFiles` takes. Loaded sessions take the uuid from the transcript line. `fork_session_before` cuts a copy of the transcript before the message (and its file snapshot). The first message has nothing before it, so the conversation buttons are off there. Buttons are off while a turn runs.

## Not done / left

- `/compact` line, queue edit and send-now, `대화 열기`/`둘 다`, dark mode, Windows: not checked in the real window (the `/compact` line and queue logic have unit tests; the rest of the checks below passed).
- The slash menu shows eight rows and does not scroll, so commands past the eighth are reached only by typing their name (already so before 58B).
- `/rewind` does not put the message text back in the box; the desktop app's flow was not observed.
- Desktop commands Forge does not run itself (`fork`, `subtask`, `plugin`, `feedback`, `plan`, `permissions`) are listed only if the SDK reports them.
- `rewind-N` and `side-N` request ids share one counter; a restarted sidecar forgets file checkpoints it did not write itself (not tried).

## Checked in the real window (2026-10-26, light theme, real SDK)

Slash list order; `/btw` question answered in a card and absent from the transcript; ⌘↵ queue card while a permission card waited, auto-sent after allowing; `/rewind` list, preview (`파일 1개가 바뀝니다 (+0 −1)`), `파일 되돌리기` deleted `hello.txt`; first message disables the conversation buttons. Found and fixed: a bare `/btw` was sent as a message; rewinding to the first message made an empty session.

Harness lesson: two `forge-app` processes can run; `set frontmost of process "forge-app"` picks either, so key events can reach the user's own app. Target by pid (`first process whose unix id is <harness pid>`). Under the Korean IME letters become jamo: type Hangul by jamo key codes (shift for ㅆ ㄲ…); the first Enter after typing only commits the composition.
