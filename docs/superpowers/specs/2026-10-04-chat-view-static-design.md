# Chat view, static look (Plan 23A)

Date: 2026-10-04. Branch `plan-23`. Source of truth for what the real app does: `docs/superpowers/roadmap/observations.md`, Round 7.

## 1. Purpose

Forge's chat window looks unlike the Claude desktop app. The user asked to observe the real app in use and copy how the conversation looks and moves. Round 7 recorded it. The work is split in three plans, each with its own spec, plan and build:

- **23A (this spec): the static look.** What a finished conversation looks like: the message shapes, the folded tool activity, the end-of-turn file card, markdown, the reply footer. Checkable on saved sessions alone.
- 23B: the running look. Spinner with elapsed time and tokens, live tool labels, stop button, the permission and question cards in place of the composer.
- 23C: side panels. Opening a file in the right-hand dock, the background-task pill and panel, the suggested next prompt.

Success: open the `hello.txt` session of Round 7 in Forge and in the real app side by side; the message shapes, the folded summary line and its opened list, the end card, and the markdown sample read the same. The user judges by eye; tests cover the grouping rules and the summary text.

Not in 23A: "undo" on the end card (needs checkpoints), branch, pin and speaker icons (fork already lives in the session menu), opening a file from a row (23C), anything that only shows while a turn runs (23B), dark theme checks (the user does those).

## 2. Data model (`forge-domain`)

Today one assistant reply is one `Message` with `content: String`, `thinking: String` and `tool_calls: Vec<ToolCall>`. The order between text and tool calls is lost, but the real app shows text, tool, text, tool in the order they happened and puts the sentences between tools inside the folded activity.

New shape:

```rust
pub enum Block { Text(String), Thinking(String), Tool(ToolCall) }
pub struct Message {
    pub role: MessageRole,
    pub blocks: Vec<Block>,
    pub attachments: Vec<Attachment>,
    pub at_ms: Option<i64>,   // Unix milliseconds of the last transcript line of this message; live: when the last event arrived
}
```

Accessors keep the other crates simple: `Message::text()` (all Text blocks joined by a blank line), `thinking()` likewise, `tool_calls()` (iterator over `&ToolCall`), `tool_calls_mut()`, `Message::user(text)`, `Message::assistant()` and mutators `push_text(&str)`, `push_thinking(&str)`, `push_tool(ToolCall)`. `push_text` appends to the last block when it is Text, else adds a block; `push_thinking` likewise.

Callers that read `content`, `thinking` or `tool_calls` as fields (sidebar title, tool activity count, usage, tests) switch to the accessors. Struct-literal `Message { ... }` in tests become `Message::user("...")` and similar. This is a mechanical change; the test counts must not drop.

`at_ms` is the only addition not seen as a requirement of the app's shape: the reply footer shows a relative time ("2m ago") and a message has no time today.

### Producers

- Transcript parser (`forge-adapter-claude-fs/src/sessions.rs`, `apply_assistant_line`): walks the content blocks of each line in order and appends Text, Thinking (non-empty) and Tool blocks, with the existing merge of one reply split over lines (consecutive assistant lines extend the same message). Tool results still attach by id. `at_ms` from the line's `timestamp`.
- Live events (`forge-application/src/event.rs`, `apply_sidecar_event`): AssistantDelta calls `push_text`, ThinkingDelta `push_thinking`, ToolUse `push_tool`, ToolResult searches blocks for the id. Behaviour for the existing session updates is unchanged.

Hazard to check while building: a text delta that arrives after a tool block must start a new Text block (not append to the one before the tool).

## 3. Turn view model (`forge-entities`)

A pure function turns `&[Message]` and a `TranscriptView` into a list of render items; the widget only draws them. It is testable without Dioxus.

```
enum Item {
  User(&Message),                  // right-aligned bubble
  Text(markdown source),           // plain answer text
  Thinking(text),                  // only in Thinking and Detailed
  Activity { rows: Vec<Row>, summary: String, added: u32, removed: u32 },
  FileCard { files: Vec<FileRow>, added, removed },   // end of turn
  Footer { text: String, at_ms },  // copy button and time, last item of a finished turn
}
enum Row { File{verb,name,added,removed,call}, Command{label,call}, Other{label,call}, Narration(String) }
```

Rules, from the observed app:

1. A **turn** runs from a user message to the next user message (or the end). Assistant messages inside it are flattened into one block list, in order.
2. In a turn, everything up to and including the **last Tool block** is the activity span. Text and Thinking blocks inside that span become Narration rows (Thinking only in Thinking and Detailed). Text after the last tool is an ordinary Text item. A turn with no tool has no Activity.
3. A turn with several tool spans separated by user-visible text does not exist in the real app's format as far as observed; one span per turn is built. (If a later round shows otherwise, rows stay in order, so only the folding point changes.)
4. **Summary line.** One step: `<Verb> <name> +a −r` for a file tool (Write: Created, Edit and MultiEdit: Edited, Read: Read, NotebookEdit: Edited), or the command's `description` (else the command, 80 characters) for Bash. Several steps: counts by kind in this order: `Created N files`, `Edited N files`, `Ran N commands`, `Read N files`, `Used N other tools`, comma-separated, then `+a −r`; `(N failed)` when some failed. Counts and wording are Forge's English version of the Korean line the real app shows. A file edited twice counts once as "Edited" but both edits stay as rows.
5. `+a −r` come from the existing diff code (`diff_for_tool_call`); tools without a diff add nothing.
6. **Detailed** view opens the activity and every row; Normal and Thinking start it collapsed.
7. **FileCard** at the end of a turn lists each file written or edited in it once (icon by extension kind, name, summed `+a −r`), header `N files edited` (`1 file edited` is shown as a single row without the header). A turn with no file change has no card.
8. **Footer** after the last item of the last assistant block of each turn, only when the turn has ended (the session is not running, or a user message follows). It shows a copy icon (copies the turn's final text) and a relative time from `at_ms`; no time when `at_ms` is None.
9. Failed tool: the row carries a red "failed" mark and opens to show the error; the summary gets the failed count. Same data the Detailed view shows today.

## 4. Widgets and CSS

- `MessageBubble` is replaced by a `Turn` widget in `forge-entities` that draws the items. `ChatWindow` passes turns instead of messages. The Normal / Thinking / Detailed switch stays as is.
- No role labels. The user bubble is right-aligned, grey (`--bg-card` darker), hugs its text up to the column width, radius about 14 px. Claude's text has no box.
- Column: centered, text width about 53% of a 1200 px window (640 px), at least 480 px; the composer matches the column's width (it is wider today; adjust to the same column).
- Activity: summary line is grey, chevron right/down, the `+a −r` chips are small green and red pills. Opened: one bordered box, rows separated by hairlines; file rows `Created fizz.py +9 −0 ›` (verb grey, name dark); Bash rows grey; Narration rows grey text. A row opens in place to show the existing diff (`DiffView`) or input and result (`ToolDetail`), as now.
- FileCard: bordered, a header row and one row per file; no undo.
- Footer: small grey icon and time, shown only after the turn ends.
- Markdown (`forge-entities/src/markdown.rs` and `theme.css`, checked against Round 7 part 2): bullets by depth (disc, circle, square), ordered nesting (1, a, i), task-list boxes with strike-through when checked, block quote with a 3 px grey bar, tables full width with a grey header, code block fitting its content width with a copy button on hover, horizontal rule hairline, links blue underlined, inline code a pale-red chip.

## 5. Testing

- Domain: `Message` accessors and `push_*` (text after a tool starts a new block; text after text extends).
- Parser: a fixture line list with text, tool, text, tool, text keeps order; split replies merge; `at_ms` read; tool results attach.
- Live events: the same order from a delta/tool sequence.
- View model: each rule above as a table of cases (no tool, one tool, many tools with narration, Edit twice on one file, a failure, Thinking and Detailed visibility, running turn has no footer or card).
- Summary text: exact strings for one step and for mixed kinds, with and without failures.
- Visual: drive Forge with the harness against the real `forge-observe` session (read-only, config isolated) and compare with the screenshots of Round 7; list the differences left in an "As built" section here.

## 6. Risks

- The `Message` change touches about 90 test lines across five crates. Done first, alone, and committed with all tests green before any view work.
- The real app's folding point (up to the last tool) is inferred from one long task and one two-step task; if a later session shows text between two tool groups outside the fold, only rule 2 changes.
- Korean verbs in the real app ("생성됨") versus Forge's English UI: Forge keeps English.
