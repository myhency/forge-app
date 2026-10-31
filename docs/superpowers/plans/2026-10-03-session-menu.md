# Plan 15 implementation: session menu

Spec: `docs/superpowers/specs/2026-10-03-session-menu-design.md`. TDD, one commit per task.

1. **Domain**: `Message.thinking: String`; fix every `Message { .. }` literal.
2. **Loader**: `forge-adapter-claude-fs` collects thinking blocks; update the tests that said thinking is dropped.
3. **Live**: sidecar `thinking_delta` (TS), `SidecarEvent::ThinkingDelta`, `apply_sidecar_event`, `SessionUpdate::ThinkingDelta`.
4. **Modes**: `TranscriptView` enum with a function that says what a message shows; `MessageBubble` takes the mode.
5. **State and control**: per-session mode in `Workspace`; three-button control in the chat header.
6. **Keys**: R and D in the row menu, letters shown.
7. **Check and docs**: harness run with a thinking fixture; backlog rows (SES-03, SES-06 built; SES-04 won't; SES-09 note); as-built notes in the spec.
