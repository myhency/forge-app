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

## Checked later the same day (2026-10-26)

`/compact` line (`대화를 압축했습니다 · 11.4k → 1.3k 토큰`) live; queue edit and send-now; `대화 열기` (after a compaction, from a message that is not the first); the slash list scrolls to the selected row. Found and fixed: the compaction summary the SDK writes as a user line showed as a huge bubble after a reload (`isCompactSummary` lines are skipped); a rewound conversation was named `Session (fork)` when the session had no title (it now takes the first message). `둘 다` (files then conversation) was not clicked.

# Plans 59, 60, 56B, 57D (built the same day, on `plan-58`)

- **59.** A stopped turn: the sidecar forwards the SDK's `[Request interrupted by user…]` line as `interrupted`; the transcript shows a grey `중지됨` line and a refused tool call reads `중지됨`, not `실패` (the count of failed calls ignores it). A turn that is only a stop or a compaction has no copy footer. A new session is named by `Query.generateSessionTitle` from its first message (new sessions only, never over a title that exists). User messages get a row (copy, ↺) on hover; ↺ opens the `/rewind` dialog on that message (`Workspace::request_rewind`). A new session's first send opens the details panel, and the environment card shows the folder's branch (with a `워크트리` tag). Left: pin, read aloud, end card `Fix them / Not now`, background task live output.
- **60.** Every shared signal is owned by the root scope (`root_signal`, and the providers run in `App`), so the Dioxus Copy-value warning is gone (0 in a run). The permission card shows a file's path once. Changes leaves out `__pycache__` and `.pyc`. Left: panels covering the chat in a narrow window.
- **56B.** In the plan panel, selecting text offers `코멘트 추가`; comments show under the plan (`내 댓글`) and go with `수정` as `Comments on the plan: - On "<quote>": <text>`. Checked with the real SDK: the revised plan followed the comment. The sidebar row of a session with a waiting plan has the tooltip `플랜 준비됨 · 시작할까요?`, the plan's first lines and `승인하려면 세션을 여세요` (not looked at in the window). An automatic details panel gives way to the plan panel.
- **57D.** `docx-preview` 0.4.1 draws a docx as pages and `pptx-preview` 1.0.7 a pptx as slide pictures (charts included; jszip 3.10.2; `scripts/vendor_office.sh`, licences in `assets/OFFICE-LICENSES`). The file's bytes ride along in `DocumentView.source` / `SlideDeck.source`; the tab opens on `페이지` / `슬라이드` with `읽기` (the 57A–C reading view) beside it, and falls back to reading with a note when drawing fails. The docx library sits in the window head (75 KB + jszip); the 1.4 MB pptx library is loaded on the first deck (in the head it kept the window from opening at all). Word bullet symbols (private-use characters of Symbol/Wingdings) are rewritten to plain bullets. Links are stripped and scripts removed from what is drawn. Checked: python-docx and python-pptx files. Not checked: Word-made files, pptx charts and images, big decks, dark mode, a page-width resize.
