# Plan 15: session menu (SES-03, SES-06; SES-09 and the rest left out)

Plan 14 listed the session menu of Claude Code desktop. Plan 15 builds the parts that fit Forge. Evidence is in `docs/superpowers/roadmap/observations.md` (rounds 1 and 2).

## Scope, decided with the user on 2026-10-03

| Id | Decision | Why |
| --- | --- | --- |
| SES-03 transcript view (일반 / 사고 과정 / 자세히) | build | Seen: three radio choices, normal is the default. |
| SES-06 single-key shortcuts | build R (rename) and D (delete) only | F and A belong to fork and archive, which are not in this plan. |
| SES-09 recovery screen | not built | See "SES-09" below. |
| SES-04 output style | won't | The menu holds only "새 스타일...", no built-in styles. |
| SES-05 open in | won't | The items are "new window" and "cloud", not editors. Cloud runs and multi-window are out of scope. |
| SES-01 fork, SES-02 archive | later plan | Need a data model decision. |

## SES-03: transcript view modes

Forge has no session header menu; rename and delete live in the sidebar row's `⋯` menu. The mode is a three-button control in the pane header of the chat window, because that is where the view it changes is.

Modes (what each shows is inferred from the names; the real app's "자세히" was not seen with content, see OBS-01):

- **Normal** (default, today's view): the assistant's text, tool calls as one collapsed line each.
- **Thinking**: Normal, plus the assistant's thinking text above its answer, in a muted block.
- **Detailed**: Thinking, plus every tool call expanded with its input and its result.

The mode belongs to an open session and lives in memory only (`Workspace`, keyed by live session id). A new session starts in Normal. It is not saved to disk.

Data path. Thinking text is dropped today in two places, and both change:

1. `Message` gets `thinking: String`.
2. The transcript loader (`forge-adapter-claude-fs`) joins `thinking` blocks into `Message.thinking`. Claude Code writes each content block as its own line, so a thinking-only line attaches to the assistant message that the next line continues.
3. The sidecar forwards `thinking_delta` stream events as a new event `thinking_delta { text }`; Rust parses it as `SidecarEvent::ThinkingDelta` and `apply_sidecar_event` appends it to the current assistant message.

An assistant message with no text, no tool calls and no visible thinking renders nothing in the current mode (no empty bubble).

Not verified: whether the SDK emits thinking for the session's model and options. If it does not, live sessions show no thinking and only loaded transcripts do. This is stated in the as-built notes after a check in the real app.

## SES-06: keys in the row menu

With the row menu open, `R` starts the rename and `D` asks to delete, exactly as clicking the items does. `D` does nothing when the item is disabled (a session that is open). The menu shows the letter at the right of each item, as the desktop app does. Keys are ignored while any modifier is held and while the rename input has focus.

## SES-09: why it is not built

The desktop app shows "디스크에서 세션을 찾을 수 없음" for a session in its own list whose transcript is gone. Forge builds the sidebar from the transcripts on disk, so a listed session always has one: the screen cannot occur. A missing folder already opens the session view-only with a banner (spec §23). "폴더 선택" would resume a session under another folder, and Claude finds transcripts by folder, so it is not safe to promise without checking. Both stay in the backlog.

## Testing

Unit tests for the loader, the event, the mode filter and the key mapping. Then a harness run (`scripts/harness/`) with a fixture session that has thinking, to see the three modes and the keys in the real window.
