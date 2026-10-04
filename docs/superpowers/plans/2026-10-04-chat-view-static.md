# Chat View, Static Look Implementation Plan (Plan 23A)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make a finished conversation in Forge look like the Claude desktop app: right-aligned user bubble, plain answer text, tool activity folded into one summary line, an end-of-turn file card, a reply footer, and the observed markdown styles.

**Architecture:** `Message` becomes an ordered list of blocks (text, thinking, tool call) so the order of events survives. A pure view-model function in `forge-entities` turns messages into render items (user bubble, text, folded activity, file card, footer); a new `Turns` widget draws them and replaces `MessageBubble`. CSS and the markdown renderer are then brought to the observed look.

**Tech Stack:** Rust 2021, Dioxus 0.7.10 (webview), pulldown-cmark 0.13, `similar` (diffs), `time` 0.3. Tests: `cargo test`. Visual check: `scripts/harness/`.

**Spec:** `docs/superpowers/specs/2026-10-04-chat-view-static-design.md` (observations: `docs/superpowers/roadmap/observations.md`, Round 7).

## Global Constraints

- UI text is English (Forge's UI language); the Korean words in the observations are only the real app's wording.
- The Normal / Thinking / Detailed switch stays; Detailed opens the activity and every row.
- Keep every existing test passing; tests may change shape (field to accessor) but their count must not drop.
- No new crates. Colours come from existing CSS variables; new CSS must not add a `prefers-color-scheme: dark` block (the theme test reads the last dark block, which must stay the code-colours one).
- Commit messages end with the line `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.
- `at` on a message is an RFC 3339 string (what `relative_time` takes); a live-built message has `at: None`.
- Out of scope: undo on the end card, branch/pin/speaker icons, opening a file from a row (23C), anything shown only while a turn runs (23B).

## Review Focus

- A reply that starts with a tool call and has no text: the activity line comes first and no empty text item or bubble appears (Task 4).
- An assistant message holding only thinking, in Normal view: nothing is drawn for it, no empty footer (Task 4).
- A turn still running (last tool call has no result): no file card, no footer, no failed mark on the pending row (Task 4).
- A session that begins with an assistant message (resumed or forked): the first turn has no user item and still builds (Task 4).
- A Write with no `file_path`, a NotebookEdit (no diff), and a Bash with no `description`: a row with a sensible label, no panic, no card entry without a diff (Task 4).

## File Structure

- `crates/forge-domain/src/session.rs` — `Block`, new `Message` (modify).
- `crates/forge-domain/src/lib.rs` — export `Block` (modify).
- `crates/forge-adapter-claude-fs/src/sessions.rs` — parser keeps order, reads `timestamp` (modify).
- `crates/forge-application/src/event.rs` — live events use the new accessors (modify).
- `crates/forge-entities/src/turn_view.rs` — pure view model, `build_turns` (create).
- `crates/forge-entities/src/turn.rs` — `Turns` widget and its parts (create).
- `crates/forge-entities/src/message.rs` — keeps `attachment_label` and `ToolDetail`; `MessageBubble` and `ToolCallLine` removed (modify).
- `crates/forge-entities/src/transcript_view.rs` — `shows` removed (modify).
- `crates/forge-entities/src/markdown.rs` — task lists, code copy button (modify).
- `crates/forge-widgets/src/chat_window.rs` — draws `Turns` (modify).
- `crates/forge-shared/src/theme.css` — chat shapes and markdown styles (modify).
- Callers of the old fields in other crates (sidebar title, tool activity count, tests): switched to accessors in Task 1.

---

### Task 1: `Message` as ordered blocks, and every caller moved over

**Files:**
- Modify: `crates/forge-domain/src/session.rs`, `crates/forge-domain/src/lib.rs`
- Modify (compiler-driven): `crates/forge-adapter-claude-fs/src/sessions.rs`, `crates/forge-application/src/event.rs`, `crates/forge-application/src/live_session.rs`, `crates/forge-application/src/registry.rs`, `crates/forge-features/src/sidebar.rs`, `crates/forge-features/src/tool_activity.rs`, `crates/forge-entities/src/message.rs`, `crates/forge-entities/src/transcript_view.rs`, and any other file `cargo build --workspace --tests` reports.

**Interfaces:**
- Produces (used by every later task):
  ```rust
  pub enum Block { Text(String), Thinking(String), Tool(ToolCall) }
  pub struct Message { pub role: MessageRole, pub blocks: Vec<Block>, pub attachments: Vec<Attachment>, pub at: Option<String> }
  impl Message {
      pub fn new(role: MessageRole) -> Self;
      pub fn user(text: impl Into<String>) -> Self;          // no block when text is empty
      pub fn assistant_text(text: impl Into<String>) -> Self; // an assistant message with one Text block (tests)
      pub fn text(&self) -> String;       // Text blocks joined by "\n\n"
      pub fn thinking(&self) -> String;   // Thinking blocks joined by "\n\n"
      pub fn tool_calls(&self) -> impl Iterator<Item = &ToolCall>;
      pub fn tool_calls_mut(&mut self) -> impl Iterator<Item = &mut ToolCall>;
      pub fn push_text(&mut self, text: &str);        // extends the last block if it is Text, else a new block
      pub fn push_thinking(&mut self, text: &str);    // same for Thinking
      pub fn push_text_block(&mut self, text: String);     // always a new block
      pub fn push_thinking_block(&mut self, text: String); // always a new block
      pub fn push_tool(&mut self, call: ToolCall);
  }
  ```

- [ ] **Step 1: Write the failing domain tests**

In `crates/forge-domain/src/session.rs`, replace the test `message_new_starts_empty` with these tests (keep the other tests in the module):

```rust
    fn call(id: &str) -> ToolCall {
        ToolCall { id: id.to_string(), name: "Bash".to_string(), input: serde_json::json!({}), result: None }
    }

    #[test]
    fn message_new_starts_empty() {
        let message = Message::new(MessageRole::Assistant);
        assert_eq!(message.role, MessageRole::Assistant);
        assert!(message.blocks.is_empty());
        assert_eq!(message.text(), "");
        assert_eq!(message.thinking(), "");
        assert_eq!(message.tool_calls().count(), 0);
        assert_eq!(message.at, None);
    }

    #[test]
    fn a_user_message_has_one_text_block_unless_it_is_empty() {
        assert_eq!(Message::user("hi").blocks, vec![Block::Text("hi".to_string())]);
        assert!(Message::user("").blocks.is_empty());
        assert_eq!(Message::user("hi").role, MessageRole::User);
    }

    #[test]
    fn push_text_extends_the_last_text_block() {
        let mut message = Message::new(MessageRole::Assistant);
        message.push_text("Hel");
        message.push_text("lo");
        assert_eq!(message.blocks, vec![Block::Text("Hello".to_string())]);
    }

    #[test]
    fn push_text_after_a_tool_call_starts_a_new_block() {
        let mut message = Message::new(MessageRole::Assistant);
        message.push_text("before");
        message.push_tool(call("t-1"));
        message.push_text("after");
        assert_eq!(
            message.blocks,
            vec![Block::Text("before".to_string()), Block::Tool(call("t-1")), Block::Text("after".to_string())]
        );
    }

    #[test]
    fn push_thinking_extends_thinking_but_not_text() {
        let mut message = Message::new(MessageRole::Assistant);
        message.push_thinking("a");
        message.push_thinking("b");
        message.push_text("answer");
        message.push_thinking("c");
        assert_eq!(
            message.blocks,
            vec![Block::Thinking("ab".to_string()), Block::Text("answer".to_string()), Block::Thinking("c".to_string())]
        );
    }

    #[test]
    fn block_pushes_always_start_a_new_block_and_text_joins_with_a_blank_line() {
        let mut message = Message::new(MessageRole::Assistant);
        message.push_text_block("one".to_string());
        message.push_text_block("two".to_string());
        message.push_thinking_block("x".to_string());
        message.push_thinking_block("y".to_string());
        assert_eq!(message.blocks.len(), 4);
        assert_eq!(message.text(), "one\n\ntwo");
        assert_eq!(message.thinking(), "x\n\ny");
    }

    #[test]
    fn tool_calls_mut_lets_a_result_be_attached() {
        let mut message = Message::new(MessageRole::Assistant);
        message.push_tool(call("t-1"));
        for tool_call in message.tool_calls_mut() {
            tool_call.result = Some(ToolResult { output: "ok".to_string(), is_error: false });
        }
        assert_eq!(message.tool_calls().next().unwrap().result.as_ref().unwrap().output, "ok");
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p forge-domain 2>&1 | tail -15`
Expected: compile error, `Block` / `Message::user` not found.

- [ ] **Step 3: Implement the domain change**

In `crates/forge-domain/src/session.rs`, replace the `Message` struct and its `impl` (lines 35-56) with:

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    Text(String),
    /// What the model thought before it answered.
    Thinking(String),
    Tool(ToolCall),
}

/// One message, its parts in the order they happened: an assistant reply can be text, a tool call,
/// more text, another tool call.
#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    pub role: MessageRole,
    pub blocks: Vec<Block>,
    /// Files and images the person attached to this message (user messages only).
    pub attachments: Vec<crate::Attachment>,
    /// RFC 3339 time of the last transcript line of this message; `None` for a message built live.
    pub at: Option<String>,
}

impl Message {
    pub fn new(role: MessageRole) -> Self {
        Self { role, blocks: Vec::new(), attachments: Vec::new(), at: None }
    }

    pub fn user(text: impl Into<String>) -> Self {
        let mut message = Self::new(MessageRole::User);
        let text = text.into();
        if !text.is_empty() {
            message.blocks.push(Block::Text(text));
        }
        message
    }

    pub fn assistant_text(text: impl Into<String>) -> Self {
        let mut message = Self::new(MessageRole::Assistant);
        message.blocks.push(Block::Text(text.into()));
        message
    }

    /// All text blocks, joined by a blank line.
    pub fn text(&self) -> String {
        self.joined(|block| match block {
            Block::Text(text) => Some(text.as_str()),
            _ => None,
        })
    }

    /// All thinking blocks, joined by a blank line.
    pub fn thinking(&self) -> String {
        self.joined(|block| match block {
            Block::Thinking(text) => Some(text.as_str()),
            _ => None,
        })
    }

    fn joined<'a>(&'a self, pick: impl Fn(&'a Block) -> Option<&'a str>) -> String {
        self.blocks.iter().filter_map(pick).collect::<Vec<_>>().join("\n\n")
    }

    pub fn tool_calls(&self) -> impl Iterator<Item = &ToolCall> {
        self.blocks.iter().filter_map(|block| match block {
            Block::Tool(call) => Some(call),
            _ => None,
        })
    }

    pub fn tool_calls_mut(&mut self) -> impl Iterator<Item = &mut ToolCall> {
        self.blocks.iter_mut().filter_map(|block| match block {
            Block::Tool(call) => Some(call),
            _ => None,
        })
    }

    /// Streamed text: extends the last block when it is text, otherwise starts a block.
    pub fn push_text(&mut self, text: &str) {
        match self.blocks.last_mut() {
            Some(Block::Text(existing)) => existing.push_str(text),
            _ => self.blocks.push(Block::Text(text.to_string())),
        }
    }

    pub fn push_thinking(&mut self, text: &str) {
        match self.blocks.last_mut() {
            Some(Block::Thinking(existing)) => existing.push_str(text),
            _ => self.blocks.push(Block::Thinking(text.to_string())),
        }
    }

    pub fn push_text_block(&mut self, text: String) {
        self.blocks.push(Block::Text(text));
    }

    pub fn push_thinking_block(&mut self, text: String) {
        self.blocks.push(Block::Thinking(text));
    }

    pub fn push_tool(&mut self, call: ToolCall) {
        self.blocks.push(Block::Tool(call));
    }
}
```

In `crates/forge-domain/src/lib.rs`, add `Block` to the `pub use session::{...}` list (open the file and add it next to `Message`).

- [ ] **Step 4: Run the domain tests**

Run: `cargo test -p forge-domain 2>&1 | tail -8`
Expected: all pass.

- [ ] **Step 5: Move every caller over, behaviour unchanged**

Run `cargo build --workspace --tests 2>&1 | grep -E "^error" -A6 | head -80` and fix file by file until it builds. The mapping is mechanical:

| Old | New |
| --- | --- |
| `message.content` (read) | `message.text()` |
| `message.thinking` (read) | `message.thinking()` |
| `message.tool_calls.iter()` / `.len()` / `[i]` | `message.tool_calls()` / `.count()` / `.nth(i).unwrap()` |
| `for t in message.tool_calls.iter_mut()` | `for t in message.tool_calls_mut()` |
| `Message { role: User, content: X, thinking: String::new(), tool_calls: Vec::new(), attachments: A }` | `{ let mut m = Message::user(X); m.attachments = A; m }` (just `Message::user(X)` when `A` is `Vec::new()`) |
| test literal of an assistant `Message { content: X, .. }` | `Message::assistant_text(X)` |
| `message.content.push_str(t)` on a streamed message | `message.push_text(t)` |
| `message.tool_calls.push(c)` | `message.push_tool(c)` |

The non-test sites, with their final code:

`crates/forge-application/src/event.rs`, replace `push_user_message` and the four event arms:

```rust
pub fn push_user_message(session: &mut Session, text: String, attachments: Vec<Attachment>) {
    let mut message = Message::user(text);
    message.attachments = attachments;
    session.messages.push(message);
}
```
```rust
        SidecarEvent::AssistantDelta { text } => {
            match session.messages.last_mut() {
                Some(message) if message.role == MessageRole::Assistant => message.push_text(text),
                _ => {
                    let mut message = Message::new(MessageRole::Assistant);
                    message.push_text(text);
                    session.messages.push(message);
                }
            }
            SessionUpdate::MessageDelta { text: text.clone() }
        }
        SidecarEvent::ThinkingDelta { text } => {
            match session.messages.last_mut() {
                Some(message) if message.role == MessageRole::Assistant => message.push_thinking(text),
                _ => {
                    let mut message = Message::new(MessageRole::Assistant);
                    message.push_thinking(text);
                    session.messages.push(message);
                }
            }
            SessionUpdate::ThinkingDelta { text: text.clone() }
        }
```
In the `ToolUse` arm use `Some(message) => message.push_tool(tool_call.clone())` and `message.push_tool(tool_call.clone())` in the `None` arm. In the `ToolResult` arm use `for tool_call in message.tool_calls_mut()`.

`crates/forge-adapter-claude-fs/src/sessions.rs`: `push_user_message` becomes `session.messages.push(Message::user(text));`; `attach_tool_result` uses `message.tool_calls_mut()`; the tail of `apply_assistant_line` (from `let text = texts.join` to the end) becomes:

```rust
    let text = texts.join("\n\n");
    let thinking = thoughts.join("\n\n");
    if text.is_empty() && thinking.is_empty() && tool_calls.is_empty() {
        return;
    }
    if !matches!(session.messages.last(), Some(m) if m.role == MessageRole::Assistant) {
        session.messages.push(Message::new(MessageRole::Assistant));
    }
    let message = session
        .messages
        .last_mut()
        .expect("an assistant message was just ensured");
    if !text.is_empty() {
        message.push_text_block(text);
    }
    if !thinking.is_empty() {
        message.push_thinking_block(thinking);
    }
    for tool_call in tool_calls {
        message.push_tool(tool_call);
    }
```
(This keeps today's order-losing behaviour on purpose; Task 2 fixes it test-first.)

`crates/forge-features/src/sidebar.rs:186`: `truncate_chars(&message.text(), MAX_TITLE_CHARS)`.
`crates/forge-features/src/tool_activity.rs:6`: `session.messages.iter().flat_map(|message| message.tool_calls()).filter(|call| call.result.is_some()).count()`; its test builds the message with `for call in [call(true), call(false), call(true)] { message.push_tool(call); }`.
`crates/forge-entities/src/message.rs` (temporary, deleted in Task 5): in `MessageBubble` use `message.text()`, `message.thinking()` and `message.tool_calls()`; the `for tool_call in message.tool_calls.iter()` loop becomes `for tool_call in message.tool_calls()` (clone as before).
`crates/forge-entities/src/transcript_view.rs`: in `shows`, use `!message.text().is_empty()`, `message.tool_calls().next().is_some()`, `!message.thinking().is_empty()`; rewrite its test helper:

```rust
    fn assistant(content: &str, thinking: &str, tools: usize) -> Message {
        let mut message = Message::new(MessageRole::Assistant);
        if !thinking.is_empty() {
            message.push_thinking_block(thinking.to_string());
        }
        if !content.is_empty() {
            message.push_text_block(content.to_string());
        }
        for i in 0..tools {
            message.push_tool(ToolCall { id: format!("t{i}"), name: "Read".to_string(), input: json!({}), result: None });
        }
        message
    }
```

- [ ] **Step 6: Run the whole workspace**

Run: `cargo test --workspace 2>&1 | grep -E "^test result|FAILED|failed|error" | head -30`
Expected: every `test result: ok`, no failures. Compare the total test count with `git stash`-free baseline: before editing, `git log -1` is the baseline; the count must be at least as large as before (the domain module gained tests).

- [ ] **Step 7: Commit**

```bash
git add -A crates
git commit -m "refactor: a message is an ordered list of blocks

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 2: The transcript parser keeps order and reads the time

**Files:**
- Modify: `crates/forge-adapter-claude-fs/src/sessions.rs` (`apply_assistant_line` and its tests)

**Interfaces:**
- Consumes: `Message::push_*`, `Message.at`, `Block` (Task 1).
- Produces: assistant `Message.blocks` in transcript order; `Message.at` = RFC 3339 `timestamp` of the last assistant line merged into the message.

- [ ] **Step 1: Write the failing tests**

Add next to `load_session_merges_an_assistant_response_split_one_block_per_line` in the `tests` module:

```rust
    #[tokio::test]
    async fn load_session_keeps_text_and_tool_calls_in_the_order_they_happened() {
        let home = tempfile::tempdir().unwrap();
        let tool = |id: &str| {
            assistant_block(json!({"type": "tool_use", "id": id, "name": "Bash", "input": {"command": "ls"}}))
        };
        let result = |id: &str| user_blocks(json!([{"type": "tool_result", "tool_use_id": id, "content": "ok"}]));
        write_session(
            home.path(),
            &project(),
            "s1",
            &[
                user_text("go"),
                assistant_block(json!({"type": "text", "text": "first"})),
                tool("t-1"),
                result("t-1"),
                assistant_block(json!({"type": "text", "text": "between"})),
                tool("t-2"),
                result("t-2"),
                assistant_block(json!({"type": "text", "text": "last"})),
            ],
        );

        let session = load(home.path(), "s1").await.unwrap();

        assert_eq!(session.messages.len(), 2);
        let shape: Vec<String> = session.messages[1]
            .blocks
            .iter()
            .map(|block| match block {
                Block::Text(text) => format!("text:{text}"),
                Block::Thinking(text) => format!("thinking:{text}"),
                Block::Tool(call) => format!("tool:{}", call.id),
            })
            .collect();
        assert_eq!(shape, vec!["text:first", "tool:t-1", "text:between", "tool:t-2", "text:last"]);
        assert!(session.messages[1].tool_calls().all(|call| call.result.is_some()));
    }

    #[tokio::test]
    async fn load_session_keeps_the_order_inside_one_line() {
        let home = tempfile::tempdir().unwrap();
        write_session(
            home.path(),
            &project(),
            "s1",
            &[
                user_text("go"),
                json!({"type": "assistant", "isSidechain": false, "message": {"role": "assistant", "content": [
                    {"type": "thinking", "thinking": "hm"},
                    {"type": "text", "text": "a"},
                    {"type": "tool_use", "id": "t-1", "name": "Read", "input": {"file_path": "x"}},
                    {"type": "text", "text": "b"}
                ]}}),
            ],
        );

        let session = load(home.path(), "s1").await.unwrap();

        assert_eq!(
            session.messages[1].blocks,
            vec![
                Block::Thinking("hm".to_string()),
                Block::Text("a".to_string()),
                Block::Tool(ToolCall {
                    id: "t-1".to_string(),
                    name: "Read".to_string(),
                    input: json!({"file_path": "x"}),
                    result: None
                }),
                Block::Text("b".to_string()),
            ]
        );
    }

    #[tokio::test]
    async fn load_session_reads_the_time_of_an_assistant_message_from_its_last_line() {
        let home = tempfile::tempdir().unwrap();
        let at = |stamp: &str, text: &str| {
            json!({"type": "assistant", "isSidechain": false, "timestamp": stamp,
                   "message": {"role": "assistant", "content": [{"type": "text", "text": text}]}})
        };
        write_session(
            home.path(),
            &project(),
            "s1",
            &[
                user_text("go"),
                at("2026-10-04T01:00:00.000Z", "one"),
                at("2026-10-04T01:00:05.000Z", "two"),
            ],
        );

        let session = load(home.path(), "s1").await.unwrap();

        assert_eq!(session.messages[1].at.as_deref(), Some("2026-10-04T01:00:05.000Z"));
        assert_eq!(session.messages[0].at, None);
    }
```
At the top of the `tests` module's imports make sure `Block` and `ToolCall` are in scope (`use forge_domain::{Block, ToolCall, ...}` — extend the existing `use` line).

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p forge-adapter-claude-fs load_session_keeps -- --nocapture 2>&1 | tail -20` and `cargo test -p forge-adapter-claude-fs reads_the_time 2>&1 | tail -8`
Expected: FAIL (order is `text, tool, tool, ...`; `at` is `None`).

- [ ] **Step 3: Implement**

Replace `apply_assistant_line` entirely with:

```rust
fn apply_assistant_line(session: &mut Session, line: &Value) {
    let Some(blocks) = line.pointer("/message/content").and_then(Value::as_array) else {
        return;
    };
    let mut parsed: Vec<Block> = Vec::new();
    for block in blocks {
        match block.get("type").and_then(Value::as_str) {
            Some("text") => {
                if let Some(text) = block.get("text").and_then(Value::as_str).filter(|t| !t.is_empty()) {
                    parsed.push(Block::Text(text.to_string()));
                }
            }
            Some("thinking") => {
                if let Some(thought) = block.get("thinking").and_then(Value::as_str).filter(|t| !t.is_empty()) {
                    parsed.push(Block::Thinking(thought.to_string()));
                }
            }
            Some("tool_use") => {
                if let (Some(id), Some(name)) = (
                    block.get("id").and_then(Value::as_str),
                    block.get("name").and_then(Value::as_str),
                ) {
                    parsed.push(Block::Tool(ToolCall {
                        id: id.to_string(),
                        name: name.to_string(),
                        input: block.get("input").cloned().unwrap_or(Value::Null),
                        result: None,
                    }));
                }
            }
            _ => {}
        }
    }
    if parsed.is_empty() {
        return;
    }
    if !matches!(session.messages.last(), Some(m) if m.role == MessageRole::Assistant) {
        session.messages.push(Message::new(MessageRole::Assistant));
    }
    let message = session
        .messages
        .last_mut()
        .expect("an assistant message was just ensured");
    message.blocks.extend(parsed);
    if let Some(stamp) = line.get("timestamp").and_then(Value::as_str) {
        message.at = Some(stamp.to_string());
    }
}
```
Add `Block` to the file's `use forge_domain::{...}` line.

- [ ] **Step 4: Run the crate's tests**

Run: `cargo test -p forge-adapter-claude-fs 2>&1 | grep -E "^test result|FAILED|failed"`
Expected: all pass, including the older `merges_an_assistant_response_split_one_block_per_line` (its `text()` is still `"Let me look.\n\nDone."`).

- [ ] **Step 5: Commit**

```bash
git add crates/forge-adapter-claude-fs
git commit -m "feat: the transcript parser keeps the order of text and tool calls

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Live events keep the order

**Files:**
- Modify: `crates/forge-application/src/event.rs` (tests only; the code was moved in Task 1)

**Interfaces:**
- Consumes: `Message::push_text`, `push_tool`, `Block` (Task 1).

- [ ] **Step 1: Write the failing-or-pinning tests**

Add to the `tests` module in `event.rs` (it has `empty_session()`); also add `Block` to its `use forge_domain::{...}` line:

```rust
    fn tool_use(id: &str) -> SidecarEvent {
        SidecarEvent::ToolUse { tool_call_id: id.to_string(), name: "Bash".to_string(), input: serde_json::json!({"command": "ls"}) }
    }

    #[test]
    fn text_after_a_tool_call_starts_a_new_text_block() {
        let mut session = empty_session();
        apply_sidecar_event(&mut session, &SidecarEvent::AssistantDelta { text: "one ".to_string() });
        apply_sidecar_event(&mut session, &SidecarEvent::AssistantDelta { text: "two".to_string() });
        apply_sidecar_event(&mut session, &tool_use("t-1"));
        apply_sidecar_event(&mut session, &SidecarEvent::AssistantDelta { text: "three".to_string() });

        let kinds: Vec<String> = session.messages[0]
            .blocks
            .iter()
            .map(|block| match block {
                Block::Text(text) => format!("text:{text}"),
                Block::Thinking(_) => "thinking".to_string(),
                Block::Tool(call) => format!("tool:{}", call.id),
            })
            .collect();
        assert_eq!(kinds, vec!["text:one two", "tool:t-1", "text:three"]);
    }

    #[test]
    fn a_tool_result_lands_on_its_call_even_with_text_after_it() {
        let mut session = empty_session();
        apply_sidecar_event(&mut session, &tool_use("t-1"));
        apply_sidecar_event(&mut session, &SidecarEvent::AssistantDelta { text: "later".to_string() });
        apply_sidecar_event(
            &mut session,
            &SidecarEvent::ToolResult { tool_call_id: "t-1".to_string(), output: "out".to_string(), is_error: false },
        );
        let call = session.messages[0].tool_calls().next().unwrap();
        assert_eq!(call.result.as_ref().unwrap().output, "out");
        assert_eq!(session.messages[0].text(), "later");
    }
```

- [ ] **Step 2: Run**

Run: `cargo test -p forge-application text_after_a_tool_call a_tool_result_lands 2>&1 | tail -10` (run each name separately if cargo rejects two filters: `cargo test -p forge-application event::tests 2>&1 | tail -15`)
Expected: PASS (Task 1 already moved the code onto `push_text`/`push_tool`); if either fails, fix `apply_sidecar_event` until it passes.

- [ ] **Step 3: Commit**

```bash
git add crates/forge-application
git commit -m "test: live events keep the order of text and tool calls

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 4: The turn view model

**Files:**
- Create: `crates/forge-entities/src/turn_view.rs`
- Modify: `crates/forge-entities/src/lib.rs`

**Interfaces:**
- Consumes: `forge_domain::{Block, Message, MessageRole, ToolCall}`; `crate::diff::diff_for_tool_call`; `crate::tool_summary::{tool_call_status, tool_input_summary, ToolCallStatus}`; `crate::transcript_view::TranscriptView` (`shows_thinking()`).
- Produces (Tasks 5 and 6 use these exact names):
  ```rust
  pub enum ToolKind { Created, Edited, Read, Command, Other }
  pub struct ToolRow { pub call: ToolCall, pub kind: ToolKind, pub verb: String, pub subject: String,
                       pub path: String, pub added: usize, pub removed: usize, pub failed: bool }
  pub enum Row { Tool(ToolRow), Narration(String), Thinking(String) }
  pub struct Activity { pub rows: Vec<Row>, pub summary: String, pub added: usize, pub removed: usize, pub failed: usize }
  pub struct CardFile { pub name: String, pub path: String, pub added: usize, pub removed: usize }
  pub struct FileCard { pub files: Vec<CardFile>, pub added: usize, pub removed: usize }
  pub struct Footer { pub text: String, pub at: Option<String> }
  pub enum Item { User(Message), Text(String), Thinking(String), Activity(Activity), FileCard(FileCard), Footer(Footer) }
  pub fn build_turns(messages: &[Message], view: TranscriptView, running: bool) -> Vec<Item>;
  ```
  All derive `Debug, Clone, PartialEq` (`ToolKind` also `Copy, Eq`).

- [ ] **Step 1: Write the failing tests**

Create `crates/forge-entities/src/turn_view.rs` containing only the test module first (the code comes in Step 3; put both in the same file, tests at the bottom):

```rust
#[cfg(test)]
mod tests {
    use forge_domain::{Block, Message, MessageRole, ToolCall, ToolResult};
    use serde_json::json;

    use super::*;
    use crate::diff::diff_for_tool;

    fn tool(id: &str, name: &str, input: serde_json::Value, failed: Option<bool>) -> Block {
        Block::Tool(ToolCall {
            id: id.to_string(),
            name: name.to_string(),
            input,
            result: failed.map(|is_error| ToolResult { output: "out".to_string(), is_error }),
        })
    }
    fn write(id: &str, path: &str) -> Block {
        tool(id, "Write", json!({"file_path": path, "content": "a\nb\nc\n"}), Some(false))
    }
    fn edit(id: &str, path: &str) -> Block {
        tool(id, "Edit", json!({"file_path": path, "old_string": "x", "new_string": "y"}), Some(false))
    }
    fn bash(id: &str, description: Option<&str>) -> Block {
        let input = match description {
            Some(d) => json!({"command": "ls -la", "description": d}),
            None => json!({"command": "ls -la"}),
        };
        tool(id, "Bash", input, Some(false))
    }
    fn assistant(blocks: Vec<Block>) -> Message {
        let mut message = Message::new(MessageRole::Assistant);
        message.blocks = blocks;
        message
    }
    fn text(t: &str) -> Block {
        Block::Text(t.to_string())
    }
    fn activity(items: &[Item]) -> &Activity {
        items.iter().find_map(|i| if let Item::Activity(a) = i { Some(a) } else { None }).expect("an activity")
    }
    fn kinds(items: &[Item]) -> Vec<&'static str> {
        items
            .iter()
            .map(|item| match item {
                Item::User(_) => "user",
                Item::Text(_) => "text",
                Item::Thinking(_) => "thinking",
                Item::Activity(_) => "activity",
                Item::FileCard(_) => "card",
                Item::Footer(_) => "footer",
            })
            .collect()
    }

    #[test]
    fn a_turn_without_tools_is_user_text_footer() {
        let messages = vec![Message::user("hi"), assistant(vec![text("hello")])];
        let items = build_turns(&messages, TranscriptView::Normal, false);
        assert_eq!(kinds(&items), vec!["user", "text", "footer"]);
        assert_eq!(items[1], Item::Text("hello".to_string()));
        assert_eq!(items[2], Item::Footer(Footer { text: "hello".to_string(), at: None }));
    }

    #[test]
    fn one_tool_gives_a_one_step_summary_a_card_and_a_footer() {
        let input = json!({"file_path": "/p/hello.txt", "content": "a\nb\nc\n"});
        let messages = vec![
            Message::user("make it"),
            assistant(vec![write("t-1", "/p/hello.txt"), text("Done.")]),
        ];
        let items = build_turns(&messages, TranscriptView::Normal, false);
        assert_eq!(kinds(&items), vec!["user", "activity", "text", "card", "footer"]);
        let a = activity(&items);
        assert_eq!(a.summary, "Created hello.txt");
        let expected = diff_for_tool("Write", &input).unwrap();
        assert_eq!((a.added, a.removed), (expected.added, 0));
        assert!(a.added > 0);
        let Item::FileCard(card) = &items[3] else { panic!("card") };
        assert_eq!(card.files.len(), 1);
        assert_eq!(card.files[0].name, "hello.txt");
        assert_eq!(card.files[0].path, "/p/hello.txt");
    }

    #[test]
    fn text_before_and_after_the_tools_stays_plain_and_text_between_is_narration() {
        let messages = vec![
            Message::user("go"),
            assistant(vec![
                text("plan"),
                bash("t-1", Some("Run it")),
                text("step done"),
                bash("t-2", Some("Run it again")),
                text("all done"),
            ]),
        ];
        let items = build_turns(&messages, TranscriptView::Normal, false);
        assert_eq!(kinds(&items), vec!["user", "text", "activity", "text", "footer"]);
        assert_eq!(items[1], Item::Text("plan".to_string()));
        assert_eq!(items[3], Item::Text("all done".to_string()));
        let rows = &activity(&items).rows;
        assert_eq!(rows.len(), 3);
        assert!(matches!(&rows[1], Row::Narration(t) if t == "step done"));
        assert_eq!(items[4], Item::Footer(Footer { text: "all done".to_string(), at: None }));
    }

    #[test]
    fn a_reply_that_starts_with_a_tool_has_the_activity_first_and_no_empty_text() {
        let messages = vec![Message::user("go"), assistant(vec![bash("t-1", None), text("")])];
        let items = build_turns(&messages, TranscriptView::Normal, false);
        assert_eq!(kinds(&items), vec!["user", "activity", "footer"]);
        assert_eq!(activity(&items).summary, "ls -la");
    }

    #[test]
    fn several_tools_are_counted_by_kind() {
        let messages = vec![
            Message::user("go"),
            assistant(vec![
                write("t-1", "/p/fizz.py"),
                bash("t-2", Some("Run fizz.py")),
                edit("t-3", "/p/fizz.py"),
                write("t-4", "/p/README.md"),
                bash("t-5", Some("Re-run fizz.py")),
                tool("t-6", "Read", json!({"file_path": "/p/fizz.py"}), Some(false)),
                tool("t-7", "Grep", json!({"pattern": "x"}), Some(false)),
            ]),
        ];
        let items = build_turns(&messages, TranscriptView::Normal, false);
        let a = activity(&items);
        assert_eq!(a.summary, "Created 2 files, Edited 1 file, Ran 2 commands, Read 1 file, Used 1 other tool");
        assert_eq!(a.failed, 0);
        let Item::FileCard(card) = items.iter().find(|i| matches!(i, Item::FileCard(_))).unwrap() else { panic!() };
        let names: Vec<_> = card.files.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["fizz.py", "README.md"]);
    }

    #[test]
    fn a_file_edited_twice_counts_once_but_both_edits_are_rows_and_the_card_sums_them() {
        let messages = vec![
            Message::user("go"),
            assistant(vec![edit("t-1", "/p/a.py"), edit("t-2", "/p/a.py")]),
        ];
        let items = build_turns(&messages, TranscriptView::Normal, false);
        let a = activity(&items);
        assert_eq!(a.summary, "Edited 1 file");
        assert_eq!(a.rows.len(), 2);
        assert_eq!((a.added, a.removed), (2, 2));
        let Item::FileCard(card) = items.iter().find(|i| matches!(i, Item::FileCard(_))).unwrap() else { panic!() };
        assert_eq!(card.files.len(), 1);
        assert_eq!((card.files[0].added, card.files[0].removed), (2, 2));
        assert_eq!((card.added, card.removed), (2, 2));
    }

    #[test]
    fn a_failed_tool_is_marked_and_counted() {
        let messages = vec![
            Message::user("go"),
            assistant(vec![bash("t-1", Some("Run it")), tool("t-2", "Bash", json!({"command": "false"}), Some(true))]),
        ];
        let items = build_turns(&messages, TranscriptView::Normal, false);
        let a = activity(&items);
        assert_eq!(a.failed, 1);
        assert_eq!(a.summary, "Ran 2 commands (1 failed)");
        let Row::Tool(row) = &a.rows[1] else { panic!() };
        assert!(row.failed);
    }

    #[test]
    fn thinking_shows_only_in_thinking_and_detailed_views() {
        let messages = vec![
            Message::user("go"),
            assistant(vec![
                Block::Thinking("before".to_string()),
                text("lead"),
                bash("t-1", None),
                Block::Thinking("inside".to_string()),
                bash("t-2", None),
                Block::Thinking("after".to_string()),
                text("end"),
            ]),
        ];
        let normal = build_turns(&messages, TranscriptView::Normal, false);
        assert_eq!(kinds(&normal), vec!["user", "text", "activity", "text", "footer"]);
        assert_eq!(activity(&normal).rows.len(), 2);
        let thinking = build_turns(&messages, TranscriptView::Thinking, false);
        assert_eq!(kinds(&thinking), vec!["user", "thinking", "text", "activity", "thinking", "text", "footer"]);
        assert_eq!(activity(&thinking).rows.len(), 3);
        assert!(matches!(&activity(&thinking).rows[1], Row::Thinking(t) if t == "inside"));
    }

    #[test]
    fn a_message_of_only_thinking_draws_nothing_in_normal_view() {
        let messages = vec![Message::user("go"), assistant(vec![Block::Thinking("hmm".to_string())])];
        let items = build_turns(&messages, TranscriptView::Normal, false);
        assert_eq!(kinds(&items), vec!["user"]);
        let shown = build_turns(&messages, TranscriptView::Thinking, false);
        assert_eq!(kinds(&shown), vec!["user", "thinking", "footer"]);
    }

    #[test]
    fn a_running_turn_has_no_card_and_no_footer_and_a_pending_row_is_not_failed() {
        let messages = vec![
            Message::user("go"),
            assistant(vec![write("t-1", "/p/a.py"), tool("t-2", "Bash", json!({"command": "ls"}), None)]),
        ];
        let items = build_turns(&messages, TranscriptView::Normal, true);
        assert_eq!(kinds(&items), vec!["user", "activity"]);
        let Row::Tool(pending) = &activity(&items).rows[1] else { panic!() };
        assert!(!pending.failed);
        let done = build_turns(&messages, TranscriptView::Normal, false);
        assert_eq!(kinds(&done), vec!["user", "activity", "card", "footer"]);
    }

    #[test]
    fn an_earlier_turn_has_its_footer_even_while_a_later_turn_runs() {
        let messages = vec![
            Message::user("one"),
            assistant(vec![text("a")]),
            Message::user("two"),
            assistant(vec![text("b")]),
        ];
        let items = build_turns(&messages, TranscriptView::Normal, true);
        assert_eq!(kinds(&items), vec!["user", "text", "footer", "user", "text"]);
    }

    #[test]
    fn a_session_that_starts_with_an_assistant_message_builds() {
        let messages = vec![assistant(vec![text("welcome back")]), Message::user("hi")];
        let items = build_turns(&messages, TranscriptView::Normal, false);
        assert_eq!(kinds(&items), vec!["text", "footer", "user"]);
    }

    #[test]
    fn rows_without_a_path_a_diff_or_a_description_still_get_a_label() {
        let messages = vec![
            Message::user("go"),
            assistant(vec![
                tool("t-1", "Write", json!({"content": "x"}), Some(false)),
                tool("t-2", "NotebookEdit", json!({"notebook_path": "/p/n.ipynb", "new_source": "x"}), Some(false)),
                bash("t-3", None),
            ]),
        ];
        let items = build_turns(&messages, TranscriptView::Normal, false);
        let rows = &activity(&items).rows;
        let Row::Tool(write_row) = &rows[0] else { panic!() };
        assert_eq!(write_row.verb, "Created");
        let Row::Tool(notebook) = &rows[1] else { panic!() };
        assert_eq!((notebook.verb.as_str(), notebook.subject.as_str()), ("Edited", "n.ipynb"));
        let Row::Tool(command) = &rows[2] else { panic!() };
        assert_eq!((command.verb.as_str(), command.subject.as_str()), ("", "ls -la"));
        assert!(!items.iter().any(|i| matches!(i, Item::FileCard(_))), "no diff, no card entry");
    }

    #[test]
    fn the_footer_time_is_the_time_of_the_last_assistant_message() {
        let mut first = assistant(vec![text("a")]);
        first.at = Some("2026-10-04T01:00:00Z".to_string());
        let mut second = assistant(vec![text("b")]);
        second.at = Some("2026-10-04T01:00:09Z".to_string());
        let items = build_turns(&[Message::user("go"), first, second], TranscriptView::Normal, false);
        let Item::Footer(footer) = items.last().unwrap() else { panic!() };
        assert_eq!(footer.at.as_deref(), Some("2026-10-04T01:00:09Z"));
        assert_eq!(footer.text, "a\n\nb");
    }

    #[test]
    fn three_hundred_commands_still_summarize() {
        let blocks: Vec<Block> = (0..300).map(|i| bash(&format!("t-{i}"), Some("Run"))).collect();
        let items = build_turns(&[Message::user("go"), assistant(blocks)], TranscriptView::Normal, false);
        assert_eq!(activity(&items).summary, "Ran 300 commands");
        assert_eq!(activity(&items).rows.len(), 300);
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Add `mod turn_view;` and `pub use turn_view::{build_turns, Activity, CardFile, FileCard, Footer, Item, Row, ToolKind, ToolRow};` to `crates/forge-entities/src/lib.rs`, then run:
`cargo test -p forge-entities turn_view 2>&1 | tail -15`
Expected: compile errors (`build_turns` and the types are missing).

- [ ] **Step 3: Implement**

Put this above the `#[cfg(test)]` module in `turn_view.rs`:

```rust
use forge_domain::{Block, Message, MessageRole, ToolCall};
use serde_json::Value;

use crate::diff::diff_for_tool_call;
use crate::tool_summary::{tool_call_status, tool_input_summary, ToolCallStatus};
use crate::transcript_view::TranscriptView;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolKind {
    Created,
    Edited,
    Read,
    Command,
    Other,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolRow {
    pub call: ToolCall,
    pub kind: ToolKind,
    /// "Created", "Edited", "Read", empty for a command, the tool name otherwise.
    pub verb: String,
    /// The file name, the command's description (else the command), or the input summary.
    pub subject: String,
    /// The full path for a file tool; empty for the others.
    pub path: String,
    pub added: usize,
    pub removed: usize,
    pub failed: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Row {
    Tool(ToolRow),
    Narration(String),
    Thinking(String),
}

/// The folded part of a turn: from the first tool call to the last.
#[derive(Debug, Clone, PartialEq)]
pub struct Activity {
    pub rows: Vec<Row>,
    pub summary: String,
    pub added: usize,
    pub removed: usize,
    pub failed: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CardFile {
    pub name: String,
    pub path: String,
    pub added: usize,
    pub removed: usize,
}

/// The files a finished turn wrote or edited, each once.
#[derive(Debug, Clone, PartialEq)]
pub struct FileCard {
    pub files: Vec<CardFile>,
    pub added: usize,
    pub removed: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Footer {
    /// What the copy button puts on the clipboard: the turn's closing text.
    pub text: String,
    pub at: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    User(Message),
    Text(String),
    Thinking(String),
    Activity(Activity),
    FileCard(FileCard),
    Footer(Footer),
}

pub fn tool_kind(name: &str) -> ToolKind {
    match name {
        "Write" => ToolKind::Created,
        "Edit" | "MultiEdit" | "NotebookEdit" => ToolKind::Edited,
        "Read" => ToolKind::Read,
        "Bash" => ToolKind::Command,
        _ => ToolKind::Other,
    }
}

fn file_name(path: &str) -> String {
    path.rsplit('/').next().filter(|name| !name.is_empty()).unwrap_or(path).to_string()
}

fn tool_row(call: &ToolCall) -> ToolRow {
    let kind = tool_kind(&call.name);
    let (added, removed) = diff_for_tool_call(call).map_or((0, 0), |diff| (diff.added, diff.removed));
    let summary = tool_input_summary(&call.input);
    let (verb, subject, path) = match kind {
        ToolKind::Created | ToolKind::Edited | ToolKind::Read => {
            let path = ["file_path", "notebook_path"]
                .iter()
                .find_map(|key| call.input.get(*key).and_then(Value::as_str))
                .map(str::to_string)
                .unwrap_or_else(|| summary.clone());
            let verb = match kind {
                ToolKind::Created => "Created",
                ToolKind::Edited => "Edited",
                _ => "Read",
            };
            (verb.to_string(), file_name(&path), path)
        }
        ToolKind::Command => {
            let label = call
                .input
                .get("description")
                .and_then(Value::as_str)
                .filter(|text| !text.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| summary.clone());
            (String::new(), label, String::new())
        }
        ToolKind::Other => (call.name.clone(), summary.clone(), String::new()),
    };
    ToolRow { call: call.clone(), kind, verb, subject, path, added, removed, failed: tool_call_status(call) == ToolCallStatus::Failed }
}

fn summarize(tools: &[&ToolRow], failed: usize) -> String {
    let mut text = if tools.len() == 1 {
        let row = tools[0];
        if row.verb.is_empty() { row.subject.clone() } else { format!("{} {}", row.verb, row.subject) }
    } else {
        let distinct = |kind: ToolKind| {
            let mut seen: Vec<&str> = Vec::new();
            for row in tools.iter().filter(|row| row.kind == kind) {
                if !seen.contains(&row.path.as_str()) {
                    seen.push(&row.path);
                }
            }
            seen.len()
        };
        let commands = tools.iter().filter(|row| row.kind == ToolKind::Command).count();
        let others = tools.iter().filter(|row| row.kind == ToolKind::Other).count();
        let parts = [
            ("Created", distinct(ToolKind::Created), "file"),
            ("Edited", distinct(ToolKind::Edited), "file"),
            ("Ran", commands, "command"),
            ("Read", distinct(ToolKind::Read), "file"),
            ("Used", others, "other tool"),
        ];
        parts
            .iter()
            .filter(|(_, count, _)| *count > 0)
            .map(|(verb, count, noun)| format!("{verb} {count} {noun}{}", if *count == 1 { "" } else { "s" }))
            .collect::<Vec<_>>()
            .join(", ")
    };
    if failed > 0 {
        text.push_str(&format!(" ({failed} failed)"));
    }
    text
}

fn build_activity(blocks: &[Block], view: TranscriptView) -> Activity {
    let mut rows = Vec::new();
    for block in blocks {
        match block {
            Block::Tool(call) => rows.push(Row::Tool(tool_row(call))),
            Block::Text(text) if !text.trim().is_empty() => rows.push(Row::Narration(text.clone())),
            Block::Thinking(text) if view.shows_thinking() && !text.trim().is_empty() => rows.push(Row::Thinking(text.clone())),
            _ => {}
        }
    }
    let tools: Vec<&ToolRow> = rows.iter().filter_map(|row| if let Row::Tool(tool) = row { Some(tool) } else { None }).collect();
    let failed = tools.iter().filter(|row| row.failed).count();
    let (added, removed) = tools.iter().fold((0, 0), |(a, r), row| (a + row.added, r + row.removed));
    let summary = summarize(&tools, failed);
    Activity { rows, summary, added, removed, failed }
}

fn build_card(blocks: &[Block]) -> Option<FileCard> {
    let mut files: Vec<CardFile> = Vec::new();
    for block in blocks {
        let Block::Tool(call) = block else { continue };
        let row = tool_row(call);
        if !matches!(row.kind, ToolKind::Created | ToolKind::Edited) || diff_for_tool_call(call).is_none() {
            continue;
        }
        match files.iter_mut().find(|file| file.path == row.path) {
            Some(file) => {
                file.added += row.added;
                file.removed += row.removed;
            }
            None => files.push(CardFile { name: row.subject, path: row.path, added: row.added, removed: row.removed }),
        }
    }
    if files.is_empty() {
        return None;
    }
    let (added, removed) = files.iter().fold((0, 0), |(a, r), file| (a + file.added, r + file.removed));
    Some(FileCard { files, added, removed })
}

fn finish_turn(items: &mut Vec<Item>, blocks: Vec<Block>, at: Option<String>, view: TranscriptView, ended: bool) {
    if blocks.is_empty() {
        return;
    }
    let start = items.len();
    let first = blocks.iter().position(|block| matches!(block, Block::Tool(_)));
    let last = blocks.iter().rposition(|block| matches!(block, Block::Tool(_)));
    let mut closing: Vec<&str> = Vec::new();
    for (index, block) in blocks.iter().enumerate() {
        if let (Some(first), Some(last)) = (first, last) {
            if index >= first && index <= last {
                if index == first {
                    items.push(Item::Activity(build_activity(&blocks[first..=last], view)));
                }
                continue;
            }
        }
        match block {
            Block::Text(text) if !text.trim().is_empty() => {
                items.push(Item::Text(text.clone()));
                if last.map_or(true, |last| index > last) {
                    closing.push(text);
                }
            }
            Block::Thinking(text) if view.shows_thinking() && !text.trim().is_empty() => items.push(Item::Thinking(text.clone())),
            _ => {}
        }
    }
    if items.len() == start {
        return;
    }
    if ended {
        if let Some(card) = build_card(&blocks) {
            items.push(Item::FileCard(card));
        }
        items.push(Item::Footer(Footer { text: closing.join("\n\n"), at }));
    }
}

/// The conversation as things to draw. A turn runs from a user message to the next one; the
/// assistant messages in between are read as one run of blocks. `running` is whether a turn is in
/// progress now: the last turn then has no file card and no footer.
pub fn build_turns(messages: &[Message], view: TranscriptView, running: bool) -> Vec<Item> {
    let mut items = Vec::new();
    let mut blocks: Vec<Block> = Vec::new();
    let mut at: Option<String> = None;
    for message in messages {
        match message.role {
            MessageRole::User => {
                finish_turn(&mut items, std::mem::take(&mut blocks), at.take(), view, true);
                items.push(Item::User(message.clone()));
            }
            MessageRole::Assistant => {
                blocks.extend(message.blocks.iter().cloned());
                if message.at.is_some() {
                    at = message.at.clone();
                }
            }
        }
    }
    finish_turn(&mut items, blocks, at, view, !running);
    items
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p forge-entities turn_view 2>&1 | tail -25`
Expected: all 14 tests pass. If `a_file_edited_twice...` fails on the `(2, 2)` counts, print `diff_for_tool` for one edit (`x`→`y` should be 1 added, 1 removed) and correct the test's expectation to what `diff.rs` produces for a one-line change — do not change `diff.rs`.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-entities
git commit -m "feat: a turn view model, folded activity, file card and footer

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 5: The `Turns` widget replaces `MessageBubble`

**Files:**
- Create: `crates/forge-entities/src/turn.rs`
- Modify: `crates/forge-entities/src/message.rs`, `crates/forge-entities/src/transcript_view.rs`, `crates/forge-entities/src/lib.rs`, `crates/forge-widgets/src/chat_window.rs`

**Interfaces:**
- Consumes: everything produced in Task 4; `crate::diff::diff_for_tool_call`; `crate::diff_view::DiffView`; `crate::markdown::render_markdown`; `crate::relative_time::relative_time`; `message::{attachment_label, ToolDetail}`.
- Produces: `#[component] pub fn Turns(messages: Vec<Message>, view: TranscriptView, running: bool) -> Element`.

- [ ] **Step 1: Slim down `message.rs`**

In `crates/forge-entities/src/message.rs` delete `MessageBubble` and `ToolCallLine` and their imports, keep `attachment_label` (with its test) and make `ToolDetail` crate-visible:

```rust
use dioxus::prelude::*;
use forge_domain::ToolCall;

use crate::tool_summary::tool_detail_text;

/// The text of a chip on a sent message: an icon and the file name.
pub fn attachment_label(attachment: &forge_domain::Attachment) -> String {
    let icon = match attachment.kind {
        forge_domain::AttachmentKind::Image => "🖼",
        forge_domain::AttachmentKind::File => "📄",
    };
    format!("{icon} {}", attachment.file_name())
}

/// The input of a tool call and, once it has one, its result.
#[component]
pub(crate) fn ToolDetail(tool_call: ToolCall) -> Element {
    let input = tool_detail_text(&serde_json::to_string_pretty(&tool_call.input).unwrap_or_default());
    let result = tool_call.result.as_ref().map(|result| {
        (
            if result.is_error { "tool-output failed" } else { "tool-output" },
            tool_detail_text(&result.output),
        )
    });
    rsx! {
        div { class: "tool-detail",
            pre { class: "tool-input", "{input}" }
            if let Some((class, output)) = result {
                pre { class: "{class}", "{output}" }
            }
        }
    }
}
```
(keep the existing `#[cfg(test)] mod attachment_tests` block unchanged below it). In `transcript_view.rs` delete the `shows` method and the four tests that call it (`a_message_of_only_thinking_is_hidden...`, `an_answer_or_a_tool_call_is_shown...`, `an_empty_assistant_message_is_never_shown`, `a_user_message_is_always_shown`) and the now-unused `assistant` helper, `Message`/`MessageRole` imports. In `lib.rs` change `pub use message::{MessageBubble, ToolCallLine};` to `pub use message::attachment_label;` and add `mod turn;` and `pub use turn::Turns;`.

- [ ] **Step 2: Write `turn.rs`**

```rust
use dioxus::prelude::*;
use forge_domain::Message;
use time::OffsetDateTime;

use crate::diff::diff_for_tool_call;
use crate::diff_view::DiffView;
use crate::markdown::render_markdown;
use crate::message::{attachment_label, ToolDetail};
use crate::relative_time::relative_time;
use crate::transcript_view::TranscriptView;
use crate::turn_view::{build_turns, Activity, FileCard, Footer, Item, Row, ToolRow};

fn counts(added: usize, removed: usize) -> Option<(String, String)> {
    (added > 0 || removed > 0).then(|| (format!("+{added}"), format!("−{removed}")))
}

#[component]
fn CountChips(added: usize, removed: usize) -> Element {
    let Some((plus, minus)) = counts(added, removed) else { return rsx! {} };
    rsx! {
        span { class: "count-chips",
            span { class: "count-added", "{plus}" }
            span { class: "count-removed", "{minus}" }
        }
    }
}

/// The conversation: user bubbles, answer text, one folded activity line per turn, a card of the
/// files the turn changed, and a footer under a finished turn.
#[component]
pub fn Turns(messages: Vec<Message>, view: TranscriptView, running: bool) -> Element {
    let items = build_turns(&messages, view, running);
    let detailed = view.expands_tool_calls();
    rsx! {
        div { class: "turns",
            for (index, item) in items.into_iter().enumerate() {
                TurnItem { key: "{index}", item, detailed }
            }
        }
    }
}

#[component]
fn TurnItem(item: Item, detailed: bool) -> Element {
    match item {
        Item::User(message) => {
            let text = message.text();
            rsx! {
                div { class: "turn-user",
                    if !text.is_empty() {
                        div { class: "user-bubble", "{text}" }
                    }
                    if !message.attachments.is_empty() {
                        div { class: "chips",
                            for attachment in message.attachments.iter() {
                                span { class: "chip", title: "{attachment.path.display()}", "{attachment_label(attachment)}" }
                            }
                        }
                    }
                }
            }
        }
        Item::Text(text) => {
            let html = render_markdown(&text);
            rsx! { div { class: "turn-text markdown", dangerous_inner_html: "{html}" } }
        }
        Item::Thinking(text) => rsx! { div { class: "thinking", "{text}" } },
        Item::Activity(activity) => rsx! { ActivityBlock { activity, detailed } },
        Item::FileCard(card) => rsx! { FileCardView { card } },
        Item::Footer(footer) => rsx! { FooterView { footer } },
    }
}

#[component]
fn ActivityBlock(activity: Activity, detailed: bool) -> Element {
    let mut open = use_signal(|| false);
    let shown = open() || detailed;
    let chevron = if shown { "⌄" } else { "›" };
    rsx! {
        div { class: "activity",
            button {
                class: "activity-head",
                r#type: "button",
                "aria-expanded": "{shown}",
                onclick: move |_| open.set(!shown),
                span { class: "activity-summary", "{activity.summary}" }
                CountChips { added: activity.added, removed: activity.removed }
                span { class: "chevron", "{chevron}" }
            }
            if shown {
                div { class: "activity-box",
                    for (index, row) in activity.rows.iter().enumerate() {
                        ActivityRow { key: "{index}", row: row.clone(), detailed }
                    }
                }
            }
        }
    }
}

#[component]
fn ActivityRow(row: Row, detailed: bool) -> Element {
    match row {
        Row::Narration(text) => rsx! { div { class: "activity-row narration", "{text}" } },
        Row::Thinking(text) => rsx! { div { class: "activity-row narration thinking", "{text}" } },
        Row::Tool(tool) => rsx! { ToolRowView { tool, detailed } },
    }
}

#[component]
fn ToolRowView(tool: ToolRow, detailed: bool) -> Element {
    let mut expanded = use_signal(|| false);
    let open = expanded() || detailed;
    let chevron = if open { "⌄" } else { "›" };
    let class = if tool.failed { "activity-row-head failed" } else { "activity-row-head" };
    let diff = diff_for_tool_call(&tool.call);
    rsx! {
        div { class: "activity-row",
            button {
                class: "{class}",
                r#type: "button",
                "aria-expanded": "{open}",
                onclick: move |_| expanded.set(!open),
                if !tool.verb.is_empty() {
                    span { class: "row-verb", "{tool.verb}" }
                }
                span { class: "row-subject", "{tool.subject}" }
                if tool.failed {
                    span { class: "row-failed", "failed" }
                }
                CountChips { added: tool.added, removed: tool.removed }
                span { class: "chevron", "{chevron}" }
            }
            if open {
                if let Some(diff) = diff {
                    DiffView { diff }
                } else {
                    ToolDetail { tool_call: tool.call.clone() }
                }
            }
        }
    }
}

#[component]
fn FileCardView(card: FileCard) -> Element {
    let heading = if card.files.len() == 1 { None } else { Some(format!("{} files edited", card.files.len())) };
    rsx! {
        div { class: "file-card",
            if let Some(heading) = heading {
                div { class: "file-card-row file-card-head",
                    span { class: "file-name", "{heading}" }
                    CountChips { added: card.added, removed: card.removed }
                }
            }
            for file in card.files.iter() {
                div { key: "{file.path}", class: "file-card-row", title: "{file.path}",
                    span { class: "file-name", "{file.name}" }
                    CountChips { added: file.added, removed: file.removed }
                }
            }
        }
    }
}

#[component]
fn FooterView(footer: Footer) -> Element {
    let time = footer.at.as_deref().map(|at| relative_time(at, OffsetDateTime::now_utc())).unwrap_or_default();
    let text = footer.text.clone();
    rsx! {
        div { class: "turn-footer",
            button {
                class: "icon-button",
                r#type: "button",
                title: "Copy",
                onclick: move |_| {
                    let literal = serde_json::to_string(&text).unwrap_or_default();
                    document::eval(&format!("navigator.clipboard.writeText({literal});"));
                },
                "⧉"
            }
            if !time.is_empty() {
                span { class: "footer-time", "{time}" }
            }
        }
    }
}
```

- [ ] **Step 3: Wire `ChatWindow`**

In `crates/forge-widgets/src/chat_window.rs` change the import to `use forge_entities::{PermissionCard, TranscriptView, Turns};` and replace the `div { class: "messages", for ... }` block with:

```rust
            div { class: "messages",
                Turns { messages: view.session.messages.clone(), view: mode, running: view.phase == Phase::Running }
            }
```

- [ ] **Step 4: Build and run all tests**

Run: `cargo build --workspace 2>&1 | grep -E "^(error|warning: unused)" -A6 | head -40; cargo test --workspace 2>&1 | grep -E "^test result|FAILED|failed"`
Expected: builds; all tests pass. Fix unused-import warnings you introduced.

- [ ] **Step 5: Commit**

```bash
git add -A crates
git commit -m "feat: the Turns widget draws bubbles, folded activity, file card and footer

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 6: Chat CSS

**Files:**
- Modify: `crates/forge-shared/src/theme.css`

**Interfaces:**
- Consumes: the class names used in `turn.rs`: `turns`, `turn-user`, `user-bubble`, `chips`, `chip`, `turn-text`, `thinking`, `activity`, `activity-head`, `activity-summary`, `chevron`, `activity-box`, `activity-row`, `narration`, `activity-row-head` (+`failed`), `row-verb`, `row-subject`, `row-failed`, `count-chips`, `count-added`, `count-removed`, `file-card`, `file-card-row`, `file-card-head`, `file-name`, `turn-footer`, `icon-button`, `footer-time`.

- [ ] **Step 1: Replace the old message rules**

In `theme.css` delete the rules `.message`, `.message .role`, `.message .content`, `.message.user .content`, `.tool-call`, `.tool-call.failed`, `button.tool-call`, `.tool-call.expandable:hover`, `.message .content.markdown` (they have no users left; `grep -n "\.message\b\|tool-call" crates/forge-shared/src/theme.css crates/forge-*/src --include=*.rs --include=*.css` first and keep any selector still used elsewhere, such as `.permission-card`). Keep `.thinking`, `.tool-detail*` and `.diff*`. Then add, next to the removed block (not inside a media query):

```css
:root { --chat-column: 720px; }
.messages { padding: 8px 28px 16px; }
.turns { max-width: var(--chat-column); margin: 0 auto; display: flex; flex-direction: column; gap: 14px; }
.composer { max-width: var(--chat-column); }

.turn-user { display: flex; flex-direction: column; align-items: flex-end; gap: 6px; margin-top: 18px; }
.user-bubble { max-width: 100%; box-sizing: border-box; padding: 9px 14px; background: var(--bg-hover); border-radius: 14px; white-space: pre-wrap; overflow-wrap: anywhere; }
.turn-text { overflow-wrap: anywhere; }
.turn-text.markdown { white-space: normal; }

.activity-head { appearance: none; display: flex; align-items: center; gap: 8px; width: 100%; padding: 2px 0; border: none; background: none; color: var(--fg-muted); font: inherit; text-align: left; cursor: pointer; }
.activity-head:hover .activity-summary { color: var(--fg); }
.activity-summary { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.chevron { color: var(--fg-muted); }
.count-chips { display: inline-flex; flex: none; gap: 1px; font-family: var(--mono); font-size: 11px; }
.count-added, .count-removed { padding: 0 5px; line-height: 1.6; }
.count-added { background: var(--diff-added-bg); border-radius: 5px 0 0 5px; }
.count-removed { background: var(--diff-removed-bg); border-radius: 0 5px 5px 0; }
.activity-box { margin-top: 6px; border: 1px solid var(--border); border-radius: var(--radius); overflow: hidden; }
.activity-row + .activity-row { border-top: 1px solid var(--border); }
.activity-row-head { appearance: none; display: flex; align-items: center; gap: 8px; width: 100%; box-sizing: border-box; padding: 8px 12px; border: none; background: none; color: var(--fg-muted); font: inherit; text-align: left; cursor: pointer; }
.activity-row-head:hover { background: var(--bg-hover); }
.activity-row-head.failed .row-failed { color: var(--danger); }
.row-subject { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--fg); }
.activity-row-head:not(:has(.row-verb)) .row-subject { color: var(--fg-muted); }
.row-failed { font-size: 12px; }
.activity-row .chevron { margin-left: auto; }
.activity-row .count-chips + .chevron { margin-left: 0; }
.activity-row-head .count-chips { margin-left: 4px; }
.activity-row.narration { padding: 8px 12px; color: var(--fg-muted); white-space: pre-wrap; overflow-wrap: anywhere; }
.activity-row .diff, .activity-row .tool-detail { max-width: none; margin: 0; border-radius: 0; border-left: none; border-right: none; border-bottom: none; }

.file-card { border: 1px solid var(--border); border-radius: var(--radius); overflow: hidden; background: var(--bg-card); }
.file-card-row { display: flex; align-items: center; gap: 8px; padding: 8px 12px; }
.file-card-row + .file-card-row { border-top: 1px solid var(--border); }
.file-card-head { color: var(--fg-muted); }
.file-name { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

.turn-footer { display: flex; align-items: center; gap: 8px; color: var(--fg-muted); font-size: 12px; margin-top: -6px; }
.icon-button { appearance: none; border: none; background: none; color: var(--fg-muted); font-size: 14px; padding: 2px 4px; border-radius: 6px; cursor: pointer; }
.icon-button:hover { background: var(--bg-hover); color: var(--fg); }
```
If `.icon-button` already exists, delete the duplicate rule rather than keeping two.

- [ ] **Step 2: Run the theme and workspace tests**

Run: `cargo test --workspace 2>&1 | grep -E "^test result|FAILED|failed"`
Expected: pass (the theme test reads the last dark `@media` block; none was added or moved).

- [ ] **Step 3: Commit**

```bash
git add crates/forge-shared
git commit -m "style: chat bubbles, folded activity, file card and footer

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 7: Markdown parity

**Files:**
- Modify: `crates/forge-entities/src/markdown.rs`, `crates/forge-shared/src/theme.css`

**Interfaces:**
- Consumes: `render_markdown(&str) -> String`.
- Produces: task-list items render a checkbox; a fenced or indented code block is wrapped as `<div class="code-wrap"><button class="code-copy" ...>⧉</button><pre class="code-block">…</pre></div>`.

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `markdown.rs`:

```rust
    #[test]
    fn task_list_items_get_a_checkbox() {
        let html = render_markdown("- [x] done\n- [ ] todo");
        assert!(html.contains("type=\"checkbox\""), "{html}");
        assert_eq!(html.matches("checked").count(), 1, "{html}");
    }

    #[test]
    fn a_plain_list_has_no_checkbox() {
        assert!(!render_markdown("- a\n- b").contains("checkbox"));
    }

    #[test]
    fn a_code_block_carries_a_copy_button_outside_the_pre() {
        let html = render_markdown("```rust\nfn main() {}\n```");
        assert!(html.starts_with("<div class=\"code-wrap\"><button"), "{html}");
        assert!(html.contains("class=\"code-copy\""), "{html}");
        assert!(html.contains("<pre class=\"code-block\">"), "{html}");
        assert!(html.contains("</pre></div>"), "{html}");
    }

    #[test]
    fn the_copy_button_script_is_fixed_text_not_from_the_message() {
        let html = render_markdown("```\n\"); alert(1); (\"\n```");
        let button = html.split("</button>").next().unwrap();
        assert!(!button.contains("alert"), "{button}");
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p forge-entities markdown 2>&1 | tail -15`
Expected: the four new tests FAIL.

- [ ] **Step 3: Implement**

In `render_markdown` change the options to `Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS`. Replace `code_block_html`'s `match` with:

```rust
    const COPY: &str = "<button class=\"code-copy\" type=\"button\" title=\"Copy\" onclick=\"navigator.clipboard.writeText(this.nextElementSibling.innerText)\">⧉</button>";
    match language {
        Some(language) => format!("<div class=\"code-wrap\">{COPY}<pre class=\"code-block\"><code class=\"language-{language}\">{body}</code></pre></div>\n"),
        None => format!("<div class=\"code-wrap\">{COPY}<pre class=\"code-block\"><code>{body}</code></pre></div>\n"),
    }
```
Existing tests that assert on the exact `<pre class="code-block">` start (`grep -n 'code-block' crates/forge-entities/src/markdown.rs crates/forge-entities/src/highlight.rs`) may need their expected string wrapped; adjust the expectation only, keeping what each test checks.

- [ ] **Step 4: Markdown CSS**

In `theme.css`, in the markdown block replace the rules for `.markdown ul, .markdown ol`, `.markdown blockquote`, `.markdown table`, `.markdown th`, `.code-block` and add the new ones (keep `.markdown code`, headings and links):

```css
.markdown ul { list-style: disc; }
.markdown ul ul { list-style: circle; }
.markdown ul ul ul { list-style: square; }
.markdown ol { list-style: decimal; }
.markdown ol ol { list-style: lower-alpha; }
.markdown ol ol ol { list-style: lower-roman; }
.markdown li > ul, .markdown li > ol { margin: 4px 0 0; }
.markdown li { margin: 2px 0; }
.markdown li:has(> input[type="checkbox"]) { list-style: none; margin-left: -1.2em; }
.markdown li > input[type="checkbox"] { margin-right: 8px; accent-color: var(--fg-muted); }
.markdown li:has(> input:checked) { color: var(--fg-muted); text-decoration: line-through; }
.markdown blockquote { padding: 2px 0 2px 14px; border-left: 3px solid var(--border); color: var(--fg-muted); }
.markdown blockquote > :last-child { margin-bottom: 0; }
.markdown table { display: table; width: 100%; border-collapse: collapse; overflow: hidden; border: 1px solid var(--border); border-radius: var(--radius); }
.markdown th, .markdown td { padding: 8px 12px; border: none; border-top: 1px solid var(--border); text-align: left; }
.markdown thead th { background: var(--bg-hover); border-top: none; font-weight: 600; }
.markdown hr { border: none; border-top: 1px solid var(--border); margin: 16px 0; }
.code-wrap { position: relative; width: fit-content; max-width: 100%; margin: 0 0 10px; }
.code-wrap .code-block { margin: 0; }
.code-copy { appearance: none; position: absolute; top: 6px; right: 6px; border: none; background: var(--bg-card); color: var(--fg-muted); border-radius: 6px; padding: 2px 6px; cursor: pointer; opacity: 0; transition: opacity 0.12s; }
.code-wrap:hover .code-copy { opacity: 1; }
```
and change the `.code-block` rule's `margin: 0 0 10px;` to `margin: 0;` (the wrapper now owns the margin). Centred table cells already follow `style="text-align: center"` that pulldown-cmark emits for `:---:`; the `text-align: left` default must not override it: write the th/td rule without `text-align` and add `.markdown th, .markdown td { text-align: left; }` only if an unaligned cell looks centred in the harness check (Task 8).

- [ ] **Step 5: Run the tests**

Run: `cargo test --workspace 2>&1 | grep -E "^test result|FAILED|failed"`
Expected: pass.

- [ ] **Step 6: Commit**

```bash
git add crates/forge-entities crates/forge-shared
git commit -m "feat: markdown task lists, nested list markers, full-width tables, code copy button

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 8: Look at it, compare, record

**Files:**
- Modify: `docs/superpowers/specs/2026-10-04-chat-view-static-design.md` (an "As built" section), `docs/superpowers/roadmap/README.md`, `docs/superpowers/roadmap/backlog.md` if rows need a status.

- [ ] **Step 1: Start Forge on the real `forge-observe` session, read-only**

```bash
mkdir -p /private/tmp/forge-cmp-config
cargo build -p forge-app 2>&1 | tail -2
(FORGE_CONFIG_PATH=/private/tmp/forge-cmp-config/config.json nohup target/debug/forge-app >/private/tmp/forge-cmp.log 2>&1 & echo $! >/private/tmp/forge-harness.pid)
sleep 6
python3 scripts/harness/capture.py
```
Read `/private/tmp/forge-harness-shot.png`, click the `hello.txt` session in the sidebar (`python3 scripts/harness/click.py X Y` with pixels of that PNG, multiplied by the factor the viewer prints), capture again.

- [ ] **Step 2: Compare with Round 7**

Check each point against `observations.md` Round 7 and the screenshots taken then, and write down what differs: user bubble right-aligned and grey, no role labels, answer text plain, `Created hello.txt +1 −0` line, the `Created 2 files, Edited 1 file, Ran 2 commands` line and its opened box with narration rows between, the end card, the footer with copy and time, markdown sample (scroll down: nested lists, checklist, table full width, quote bar, code block fitting its content, rule, links). Click a file row and a Bash row (they open in place), click the `Detailed` switch (everything opens). Resize the window narrower and check nothing overflows horizontally.

- [ ] **Step 3: Fix what is clearly wrong**

Fix CSS or view-model issues found (each as its own small commit with a test where the cause is in the view model). Anything that is a taste difference or needs the running state goes to the "left" list.

- [ ] **Step 4: Stop the app and clean up**

```bash
bash scripts/harness/stop.sh; pkill -f "plan-23/target/debug/forge-app"; rm -rf /private/tmp/forge-cmp-config
```

- [ ] **Step 5: Record**

Append an "As built" section to the spec: what was built, differences from the spec (the first-to-last-tool fold rule and `at: Option<String>` were corrected before building), what was left, and the lessons. Mark Plan 23A built in `roadmap/README.md`. Run the whole suite once more:

Run: `cargo test --workspace 2>&1 | grep -E "^test result|FAILED|failed"`
Expected: all ok.

- [ ] **Step 6: Commit**

```bash
git add docs
git commit -m "docs: Plan 23A as built

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

## Self-Review

**Spec coverage:** §2 data model → Task 1 (type, accessors, `at`), parser → Task 2, live events → Task 3. §3 view model rules 1-9 → Task 4 (turn = rule 1; first-to-last fold = rule 2; summary text = rule 4; diff counts = rule 5; Detailed opens everything = widget `detailed` in Task 5 (rule 6); FileCard = rule 7; Footer = rule 8; failure = rule 9). §4 widgets and CSS → Tasks 5, 6, 7 (user bubble, no labels, column, activity, file card, footer, markdown list). §5 testing → inside each task plus Task 8 visual check. §1 non-goals respected (no undo, no branch/pin icons, no file-open).
**Placeholder scan:** no TBD; the only deliberate discretion is Task 1 Step 5's compiler-driven mapping (table given) and Task 4 Step 4's note on a diff-count expectation.
**Type consistency:** `Block`, `Message` accessors, `ToolRow`/`Row`/`Activity`/`FileCard`/`Footer`/`Item` and `build_turns(&[Message], TranscriptView, bool)` are named identically in Tasks 1, 2, 3, 4, 5. `Turns { messages, view, running }` matches the `ChatWindow` call. CSS classes in Task 6 list the ones `turn.rs` emits.
**Review focus:** the five lines each have a test in Task 4 (`a_reply_that_starts_with_a_tool...`, `a_message_of_only_thinking...`, `a_running_turn...`, `a_session_that_starts_with_an_assistant_message_builds`, `rows_without_a_path...`).
