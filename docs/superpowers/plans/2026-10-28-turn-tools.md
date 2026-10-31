# Task List and Next-Step Card Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give Claude in Forge the two tools the desktop app gives it (`update_plan`, `wrap_up`) and draw their two cards: a live task list in the chat and an end-of-turn card with reply buttons.

**Architecture:** The sidecar offers an in-process SDK MCP server named `ccd_turn` with the two tools when a new setting (`turn_tools`, default on) is on. The tools only validate and answer; both cards are drawn from the recorded tool calls by pure functions in `forge-entities`, so a reopened session and a session made in the desktop app draw the same.

**Tech Stack:** TypeScript sidecar (`@anthropic-ai/claude-agent-sdk` `createSdkMcpServer` and `tool`, `zod` 4, `vitest`), Rust (Dioxus 0.6 components, `serde_json`), the harness in `scripts/harness/`.

**Spec:** `docs/superpowers/specs/2026-10-28-turn-tools-design.md` (read it first; Task 8 corrects five lines of it that this plan settles).

## Global Constraints

- Server name `ccd_turn`; tool names `mcp__ccd_turn__update_plan` and `mcp__ccd_turn__wrap_up` (the desktop app's names).
- `update_plan` input: `scope?`, `items?` (1-12 of `{content, status}`, `status` is `pending`, `in_progress` or `completed`), `advance?`, `done?` (1-based numbers), `start?` (1-based number). `scope` is accepted and ignored.
- `wrap_up` input: `recap` (required), `ask?`, `blocked?` (accepted, ignored), `actions?` (at most 2 of `{label, message}`; no `action` field). Label cut at 24 characters, message at 300.
- Tool results: `Plan shown (N steps).` for a plan, `Shown in the next-step box. Your closing message is your answer to the user: write it now, in full, as the last thing you say this turn.` for `wrap_up`, plus ` The box asks your question, so do not ask it again.` when there was an `ask`.
- The setting is `AppConfig.turn_tools`, default **true** (`#[serde(default = "default_true")]`); `SessionDefaults.turn_tools`; the `start_session` field `turn_tools`; a command without the field means **off** in the sidecar.
- The plan card is live only. The end card shows for the last turn only, as Live until the next message, Spent when the next user message equals one of its buttons' messages, otherwise gone.
- Every new sentence goes through `tr!` and has a Korean entry in `crates/forge-shared/src/ko.rs` (sorted table; `every_sentence_in_the_sources_has_a_korean_text` checks it).
- Windows is not checked (no Windows environment). Work-PC rule: nothing here needs OS features.
- Commit messages end with `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.

## Review Focus

- A `wrap_up` call with a `recap` but no `ask` (the model forgets it): no card at all and the call is not shown as a row. Pinned in Task 4 and Task 5.
- An `update_plan` call that fails (sidecar returned an error text): it must not change the plan. Pinned in Task 4.
- `advance` after the last step, `done` or `start` with a number beyond the list: nothing breaks and nothing moves. Pinned in Task 4.
- The next user message is the button's text with different spacing or case: spaces around it are ignored, case is not (an exact reply). Pinned in Task 4 and Task 5.
- A session whose transcript ends in a `wrap_up` and is then resumed and answered with something else: the card is gone, no leftover. Pinned in Task 5.
- The model calls `wrap_up` twice in one turn: only the last one draws. Pinned in Task 5.
- The desktop app's own sessions (the same tool names) opened in Forge, with a turn that is still running (no result yet): no raw `mcp__ccd_turn__…` row appears in the live line. Pinned in Task 5.
- The sidecar started by an older parent with no `turn_tools` field: no server, no allowed names. Pinned in Task 2.

## File Structure

- Create `sidecar/src/turn_tools.ts` (the two tool definitions and the server factory), `sidecar/test/turn_tools.test.ts`.
- Modify `sidecar/package.json`, `sidecar/package-lock.json` (zod as a direct dependency), `sidecar/src/protocol.ts`, `sidecar/src/session.ts`, `sidecar/test/session.test.ts`.
- Modify `crates/forge-domain/src/config.rs`, `crates/forge-domain/src/session_defaults.rs`, `crates/forge-adapter-sidecar/src/protocol.rs`, `crates/forge-features/src/settings.rs`, `crates/forge-widgets/src/settings_overlay.rs`, `crates/forge-shared/src/ko.rs`.
- Create `crates/forge-entities/src/turn_cards.rs` (pure: tool names, plan folding, end card reading).
- Modify `crates/forge-entities/src/turn_view.rs` (two new items, hiding the calls), `crates/forge-entities/src/lib.rs`.
- Create `crates/forge-entities/src/turn_card_views.rs` (the two components); modify `crates/forge-entities/src/turn.rs`, `crates/forge-shared/src/theme.css`, `crates/forge-widgets/src/chat_window.rs`.
- Modify `crates/forge-features/src/suggestion.rs`, `crates/forge-features/src/send_message.rs` (hide the grey suggestion and take `⌘⏎` while an end card is live).
- Create `scripts/harness/add_turn_cards_samples.py`; modify docs (spec, backlog, observations as needed).

---

### Task 1: The tool definitions (sidecar)

**Files:**
- Create: `sidecar/src/turn_tools.ts`
- Test: `sidecar/test/turn_tools.test.ts`
- Modify: `sidecar/package.json`, `sidecar/package-lock.json`

**Interfaces:**
- Produces: `TURN_SERVER_NAME = "ccd_turn"`, `UPDATE_PLAN_TOOL`, `WRAP_UP_TOOL` (full `mcp__…` names), `turnToolDefinitions(): SdkMcpToolDefinition<any>[]` (two definitions with `.name`, `.description`, `.handler(args, extra)`), `createTurnToolsServer(): McpSdkServerConfigWithInstance`.

- [ ] **Step 1: Make zod a direct dependency**

zod 4.6.5 is already installed as the SDK's dependency. In `sidecar/package.json` add to `dependencies`:

```json
    "zod": "4.6.5"
```

Run in `sidecar/`: `npm install --package-lock-only` (it only rewrites the lock file; if the worktree's `node_modules` is a symlink, that is fine, nothing is installed). Check `git diff sidecar/package-lock.json` shows only the root `dependencies` entry for zod.

- [ ] **Step 2: Write the failing test**

`sidecar/test/turn_tools.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { createTurnToolsServer, turnToolDefinitions, TURN_SERVER_NAME, UPDATE_PLAN_TOOL, WRAP_UP_TOOL } from "../src/turn_tools.js";

function tools() {
  const list = turnToolDefinitions();
  const byName = (name: string) => list.find((tool) => tool.name === name)!;
  return { updatePlan: byName("update_plan"), wrapUp: byName("wrap_up") };
}

async function text(result: Promise<{ content: Array<{ type: string; text?: string }>; isError?: boolean }>) {
  const done = await result;
  return { text: done.content.map((part) => part.text ?? "").join(""), isError: done.isError === true };
}

describe("turn tools", () => {
  it("names the server and the two tools like the desktop app", () => {
    expect(TURN_SERVER_NAME).toBe("ccd_turn");
    expect(UPDATE_PLAN_TOOL).toBe("mcp__ccd_turn__update_plan");
    expect(WRAP_UP_TOOL).toBe("mcp__ccd_turn__wrap_up");
    expect(turnToolDefinitions().map((tool) => tool.name).sort()).toEqual(["update_plan", "wrap_up"]);
    expect(createTurnToolsServer().type).toBe("sdk");
  });

  it("update_plan answers with the step count for a whole plan", async () => {
    const { updatePlan } = tools();
    const out = await text(
      updatePlan.handler(
        {
          scope: "first_task",
          items: [
            { content: "Read the code", status: "in_progress" },
            { content: "Fix it", status: "pending" },
          ],
        },
        {},
      ),
    );
    expect(out).toEqual({ text: "Plan shown (2 steps).", isError: false });
  });

  it("update_plan accepts advance, done and start once a plan exists", async () => {
    const { updatePlan } = tools();
    await updatePlan.handler({ items: [{ content: "a", status: "in_progress" }, { content: "b", status: "pending" }, { content: "c", status: "pending" }] }, {});
    expect((await text(updatePlan.handler({ advance: true }, {}))).isError).toBe(false);
    expect((await text(updatePlan.handler({ done: [1], start: 3 }, {}))).isError).toBe(false);
  });

  it("update_plan refuses an empty call, an update before any plan and a number outside the list", async () => {
    const { updatePlan } = tools();
    expect((await text(updatePlan.handler({}, {}))).isError).toBe(true);
    const early = await text(updatePlan.handler({ advance: true }, {}));
    expect(early.isError).toBe(true);
    expect(early.text).toContain("items");
    await updatePlan.handler({ items: [{ content: "a", status: "in_progress" }] }, {});
    const far = await text(updatePlan.handler({ done: [2] }, {}));
    expect(far.isError).toBe(true);
    expect(far.text).toContain("1");
  });

  it("each tools instance keeps its own plan", async () => {
    const first = tools().updatePlan;
    const second = tools().updatePlan;
    await first.handler({ items: [{ content: "a", status: "in_progress" }] }, {});
    expect((await text(second.handler({ advance: true }, {}))).isError).toBe(true);
  });

  it("wrap_up says the closing message is still needed, and that the box asks the question only when there is one", async () => {
    const { wrapUp } = tools();
    const plain = await text(wrapUp.handler({ recap: "Done." }, {}));
    expect(plain.isError).toBe(false);
    expect(plain.text).toContain("Your closing message is your answer to the user");
    expect(plain.text).not.toContain("The box asks your question");
    const asked = await text(wrapUp.handler({ recap: "Done.", ask: "Commit?", actions: [{ label: "Commit" }] }, {}));
    expect(asked.text).toContain("The box asks your question, so do not ask it again.");
  });
});
```

- [ ] **Step 3: Run it to see it fail**

Run in `sidecar/`: `npx vitest run test/turn_tools.test.ts`
Expected: FAIL, "Cannot find module '../src/turn_tools.js'".

- [ ] **Step 4: Write `sidecar/src/turn_tools.ts`**

```ts
import { createSdkMcpServer, tool } from "@anthropic-ai/claude-agent-sdk";
import type { McpSdkServerConfigWithInstance, SdkMcpToolDefinition } from "@anthropic-ai/claude-agent-sdk";
import { z } from "zod";

/** The desktop app's server name: the model sees the same tool names, and a session made there opens here. */
export const TURN_SERVER_NAME = "ccd_turn";
export const UPDATE_PLAN_TOOL = `mcp__${TURN_SERVER_NAME}__update_plan`;
export const WRAP_UP_TOOL = `mcp__${TURN_SERVER_NAME}__wrap_up`;

const MAX_STEPS = 12;
const MAX_ACTIONS = 2;

const UPDATE_PLAN_DESCRIPTION = [
  "Show the person a task list of your plan while you work. Use it for any request that edits files or runs more than one command; skip it for a single-command answer or plain conversation.",
  "Send it in the same message as another tool call, never alone, so it costs no extra round trip.",
  'First call: {items: [...]}, the whole plan in order, short imperative phrases (about 7 words, no numbering), the step you are starting "in_progress", the rest "pending".',
  "To move on: {advance: true} together with the call that begins the next step (it completes the step in progress and starts the next pending one).",
  "Out of order: {done: [2], start: 4} (1-based numbers).",
  "If the plan changes, send {items} again with the whole new list.",
  "Keep exactly one step in_progress while you work, and none when everything is done. Do not also write the plan as a numbered list in your message: the task list is the plan.",
].join("\n\n");

const WRAP_UP_DESCRIPTION = [
  "End a turn that changed files, ran commands or finished multi-step work with a box for the person: a two-or-three-sentence recap and, when the next move is theirs, one question with one or two reply buttons.",
  "Call it once, when the turn's work is done, before your closing message; it also checks off the task list. Skip it for short answers and plain conversation.",
  "Your closing message is still your full answer, written right after this call; the recap never replaces it.",
  "recap: what is now true or changed and where it stands, including what is not done or not verified (300 characters or fewer, said to the person, no recommendation).",
  "ask: only when the next move is the person's call. One short question (100 characters or fewer) that does not list the buttons' options. Without ask no box is drawn.",
  "actions: one or two reply buttons, your recommendation first. label is the button text (1-3 words, 24 characters or fewer); message is what a click sends (300 characters or fewer; leave it out when the label says it all).",
].join("\n\n");

const stepSchema = z.object({
  content: z.string(),
  status: z.enum(["pending", "in_progress", "completed"]),
});

function answer(text: string, isError = false) {
  return { content: [{ type: "text" as const, text }], ...(isError ? { isError: true } : {}) };
}

/** The two tools. Each call of this function has its own memory of the plan's length, which is all the
 *  tool keeps: the cards are drawn from the recorded calls, not from here. */
export function turnToolDefinitions(): SdkMcpToolDefinition<any>[] {
  let steps = 0;
  const updatePlan = tool(
    "update_plan",
    UPDATE_PLAN_DESCRIPTION,
    {
      scope: z.enum(["first_task", "continuation", "new_task"]).optional(),
      items: z.array(stepSchema).min(1).max(MAX_STEPS).optional(),
      advance: z.boolean().optional(),
      done: z.array(z.number()).optional(),
      start: z.number().optional(),
    },
    async (args) => {
      if (args.items) {
        steps = args.items.length;
        return answer(`Plan shown (${steps} steps).`);
      }
      if (!args.advance && !args.done && args.start === undefined) {
        return answer("Nothing to update: send {items} for a plan, or {advance}, {done} or {start} to change it.", true);
      }
      if (steps === 0) {
        return answer("There is no plan yet: send {items} first.", true);
      }
      const numbers = [...(args.done ?? []), ...(args.start === undefined ? [] : [args.start])];
      if (numbers.some((n) => !Number.isInteger(n) || n < 1 || n > steps)) {
        return answer(`Step numbers are 1 to ${steps}.`, true);
      }
      return answer(`Plan updated (${steps} steps).`);
    },
  );
  const wrapUp = tool(
    "wrap_up",
    WRAP_UP_DESCRIPTION,
    {
      recap: z.string().min(1),
      ask: z.string().optional(),
      blocked: z.boolean().optional(),
      actions: z
        .array(z.object({ label: z.string().optional(), message: z.string().optional() }))
        .max(MAX_ACTIONS)
        .optional(),
    },
    async (args) => {
      const base =
        "Shown in the next-step box. Your closing message is your answer to the user: write it now, in full, as the last thing you say this turn.";
      return answer(args.ask ? `${base} The box asks your question, so do not ask it again.` : base);
    },
  );
  return [updatePlan, wrapUp];
}

export function createTurnToolsServer(): McpSdkServerConfigWithInstance {
  return createSdkMcpServer({ name: TURN_SERVER_NAME, tools: turnToolDefinitions() });
}
```

- [ ] **Step 5: Run the tests and the typecheck**

Run in `sidecar/`: `npx vitest run test/turn_tools.test.ts && npm run typecheck`
Expected: all PASS, no type errors. If `McpSdkServerConfigWithInstance` is not exported by name from the package, import it with `import type { McpSdkServerConfigWithInstance } from "@anthropic-ai/claude-agent-sdk/sdk"` or declare the return type as `ReturnType<typeof createSdkMcpServer>`; use whichever the typecheck accepts.

- [ ] **Step 6: Commit**

```bash
git add sidecar/src/turn_tools.ts sidecar/test/turn_tools.test.ts sidecar/package.json sidecar/package-lock.json
git commit -m "feat: sidecar tools update_plan and wrap_up (server ccd_turn)"
```

---

### Task 2: Start the server in a session (sidecar)

**Files:**
- Modify: `sidecar/src/protocol.ts:44-60`, `sidecar/src/session.ts` (near the `query` options, `mcpServers` and `allowedTools`)
- Test: `sidecar/test/session.test.ts`

**Interfaces:**
- Consumes: `createTurnToolsServer`, `TURN_SERVER_NAME`, `UPDATE_PLAN_TOOL`, `WRAP_UP_TOOL` from Task 1.
- Produces: `start_session` accepts `turn_tools?: boolean` (missing means false).

- [ ] **Step 1: Write the failing tests** (add inside `describe("SidecarSession", ...)` in `sidecar/test/session.test.ts`, after the first test; `toQueryHandle`, `startCommand` and `QueryFn` are already in the file)

```ts
  it("adds the ccd_turn server and allows its two tools when turn_tools is on", async () => {
    let captured: Record<string, any> | undefined;
    const queryFn: QueryFn = ({ options }) => {
      captured = options;
      return toQueryHandle((async function* () {})());
    };
    const session = new SidecarSession(queryFn, () => {});
    await session.handle({
      ...startCommand,
      turn_tools: true,
      mcp_servers: { files: { command: "npx", args: ["files-mcp"] } },
      allowed_tools: ["Read"],
    });
    expect(Object.keys(captured!.mcpServers).sort()).toEqual(["ccd_turn", "files"]);
    expect(captured!.allowedTools).toEqual(["Read", "mcp__ccd_turn__update_plan", "mcp__ccd_turn__wrap_up"]);
  });

  it("adds nothing when turn_tools is off or missing (an older parent)", async () => {
    const seen: Array<Record<string, any>> = [];
    const queryFn: QueryFn = ({ options }) => {
      seen.push(options);
      return toQueryHandle((async function* () {})());
    };
    await new SidecarSession(queryFn, () => {}).handle({ ...startCommand, turn_tools: false });
    await new SidecarSession(queryFn, () => {}).handle({ ...startCommand });
    for (const options of seen) {
      expect(options.mcpServers).toEqual({});
      expect(options.allowedTools).toEqual([]);
    }
  });
```

- [ ] **Step 2: Run to see them fail**

Run in `sidecar/`: `npx vitest run test/session.test.ts -t "ccd_turn|turn_tools"`
Expected: FAIL (the first expects a `ccd_turn` key that is not there; TypeScript may also flag `turn_tools`).

- [ ] **Step 3: Implement**

`sidecar/src/protocol.ts`, in the `start_session` variant after `dynamic_workflows: boolean;` add:

```ts
      // Offers Claude the update_plan and wrap_up tools (a parent that omits it means off).
      turn_tools?: boolean;
```

`sidecar/src/session.ts`: add the import near the other imports

```ts
import { createTurnToolsServer, TURN_SERVER_NAME, UPDATE_PLAN_TOOL, WRAP_UP_TOOL } from "./turn_tools.js";
```

and in `run()`, just before `let query: QueryHandle;`, add

```ts
    // The task list and next-step tools: an in-process server, allowed up front so no question appears.
    const turnTools = command.turn_tools === true;
```

then replace the two option lines

```ts
          mcpServers: command.mcp_servers ?? {},
```
with
```ts
          mcpServers: { ...(command.mcp_servers ?? {}), ...(turnTools ? { [TURN_SERVER_NAME]: createTurnToolsServer() } : {}) },
```
and
```ts
          allowedTools: command.allowed_tools ?? [],
```
with
```ts
          allowedTools: [...(command.allowed_tools ?? []), ...(turnTools ? [UPDATE_PLAN_TOOL, WRAP_UP_TOOL] : [])],
```

- [ ] **Step 4: Run all sidecar tests and the typecheck**

Run in `sidecar/`: `npm test && npm run typecheck`
Expected: all PASS.

- [ ] **Step 5: Rebuild the sidecar bundle the app runs and commit**

Run in `sidecar/`: `npm run build` (the app starts `sidecar/dist/index.js`; `dist` is ignored by git if it is in `.gitignore`, check `git status` and add it only if the repo tracks it).

```bash
git add sidecar/src/protocol.ts sidecar/src/session.ts sidecar/test/session.test.ts
git commit -m "feat: the sidecar starts the ccd_turn server when turn_tools is on"
```

---

### Task 3: The setting and the start field (Rust)

**Files:**
- Modify: `crates/forge-domain/src/config.rs:334,413` (+ its tests near `:818,:829`), `crates/forge-domain/src/session_defaults.rs`, `crates/forge-adapter-sidecar/src/protocol.rs:29,138` (+ tests near `:229,:239,:355`), `crates/forge-features/src/settings.rs` (`:57,:104,:292,:312,:681`, tests near `:797`), `crates/forge-widgets/src/settings_overlay.rs:301,311`, `crates/forge-shared/src/ko.rs`
- Test: in the same files

**Interfaces:**
- Produces: `AppConfig.turn_tools: bool`; `SessionDefaults.turn_tools: bool`; `SidecarCommand::StartSession { turn_tools: bool, .. }` (JSON field `turn_tools`); `Settings::turn_tools()` and `Settings::set_turn_tools(bool)`; `CodeChange::TurnTools(bool)`.

- [ ] **Step 1: Write the failing tests**

In `crates/forge-domain/src/config.rs` tests, next to the `dynamic_workflows` assertions, add:

```rust
    #[test]
    fn turn_tools_are_on_for_a_config_written_before_the_switch_existed() {
        let config: AppConfig = serde_json::from_str("{}").unwrap();
        assert!(config.turn_tools);
        assert!(AppConfig::default().turn_tools);
        let off: AppConfig = serde_json::from_str(r#"{"turn_tools": false}"#).unwrap();
        assert!(!off.turn_tools);
    }
```

In `crates/forge-domain/src/session_defaults.rs` tests add:

```rust
    #[test]
    fn turn_tools_are_carried_and_default_on() {
        assert!(SessionDefaults::default().turn_tools);
        let config = AppConfig { turn_tools: false, ..AppConfig::default() };
        assert!(!SessionDefaults::from_config(&config).turn_tools);
    }
```

In `crates/forge-adapter-sidecar/src/protocol.rs` tests (next to the `dynamic_workflows` JSON assertions at about `:229` and `:245`) add an assertion in the same style as those tests build their command: the JSON of a start command built with `defaults.turn_tools = true` has `json["turn_tools"] == serde_json::json!(true)` and with `false` has `json["turn_tools"] == serde_json::json!(false)`. Copy the construction from the nearest existing `dynamic_workflows` test and change the field.

In `crates/forge-features/src/settings.rs` tests, next to the `DynamicWorkflows` test (about `:797`):

```rust
    #[test]
    fn the_turn_tools_switch_changes_only_that_value() {
        let mut config = AppConfig::default();
        assert!(config.turn_tools);
        apply_code_change(&mut config, CodeChange::TurnTools(false));
        assert!(!config.turn_tools);
        assert!(config.prompt_suggestions, "nothing else is touched");
    }
```

- [ ] **Step 2: Run to see them fail**

Run: `cargo test -p forge-domain -p forge-adapter-sidecar -p forge-features turn_tools`
Expected: FAIL to compile ("no field `turn_tools`").

- [ ] **Step 3: Implement**

`config.rs`: after the `dynamic_workflows` field add

```rust
    /// Whether Claude gets the `update_plan` and `wrap_up` tools: a task list while it works and a
    /// next-step card at the end of a turn.
    #[serde(default = "default_true")]
    pub turn_tools: bool,
```
and in `impl Default for AppConfig` (next to `dynamic_workflows: false,`) add `turn_tools: true,`. Fix the two test literals the compiler flags (`:829` area) with `turn_tools: true`.

`session_defaults.rs`: add the field `pub turn_tools: bool,` (doc: `/// Whether the session gets the task list and next-step tools.`), `turn_tools: true` in `Default`, `turn_tools: config.turn_tools` in `from_config`, and fix the literal in its own test (`turn_tools: true`).

`forge-adapter-sidecar/src/protocol.rs`: add `turn_tools: bool,` after `dynamic_workflows: bool,` in the `StartSession` variant, `turn_tools: params.defaults.turn_tools,` after the `dynamic_workflows` line at about `:138`, and `"turn_tools": ...` in the literal JSON test at about `:355` plus `turn_tools: true` in the test literal at about `:239`.

`settings.rs`: `CodeChange::TurnTools(bool),` in the enum (after `DynamicWorkflows(bool),`); in `apply_code_change`: `CodeChange::TurnTools(on) => config.turn_tools = on,`; getter and setter next to the workflow ones:

```rust
    pub fn turn_tools(&self) -> bool {
        self.saved.read().turn_tools
    }

    pub fn set_turn_tools(&self, on: bool) {
        self.change_code(CodeChange::TurnTools(on));
    }
```
and at about `:681` (where `current.dynamic_workflows = config.dynamic_workflows;` copies values) add `current.turn_tools = config.turn_tools;`.

`settings_overlay.rs`: after the `Dynamic workflows` `SettingRow` add

```rust
        SettingRow {
            title: tr!("Task list and next step"),
            description: tr!("Claude shows its plan as a list while it works and ends a turn with a summary and a question."),
            Switch { on: settings.turn_tools(), onchange: move |on| settings.set_turn_tools(on) }
        }
```
and change the note at about `:311` to mention it: `"Output style, prompt suggestions, the bypass switch, dynamic workflows and the task list apply to sessions that start after the change. The default view also applies to open sessions you have not changed."`

`ko.rs`: the old note key is replaced by the new one (keep the table sorted; insert each new key before the first existing key that sorts after it):

```rust
    ("Claude shows its plan as a list while it works and ends a turn with a summary and a question.", "Claude가 작업하는 동안 계획을 목록으로 보여 주고, 턴을 요약과 질문으로 마무리합니다."),
    ("Task list and next step", "작업 목록과 다음 단계"),
    ("Output style, prompt suggestions, the bypass switch, dynamic workflows and the task list apply to sessions that start after the change. The default view also applies to open sessions you have not changed.", "출력 스타일, 프롬프트 제안, 권한 무시, 동적 워크플로우, 작업 목록 스위치는 변경 후 시작하는 세션에 적용됩니다. 기본 보기는 변경하지 않은 열린 세션에도 적용됩니다."),
```
The third entry replaces the existing entry at about `ko.rs:503` (same place in the sorted table: delete the old key line, then insert this one where it sorts; "Output style, prompt suggestions, the bypass switch, dynamic workflows" sorts after "...the bypass switch and dynamic workflows" because "," (0x2C) comes before " " (0x20)? No: space is 0x20, comma is 0x2C, so the new key sorts **after** the old position; run `cargo test -p forge-shared the_table_is_sorted` and move the line if it fails).

- [ ] **Step 4: Fix every other literal the compiler flags**

Run: `cargo build --workspace 2>&1 | grep -E "^error" -A6` and `cargo test --workspace --no-run 2>&1 | grep -E "^error" -A6`. Add `turn_tools: true` (or `..Default::default()`) to each `AppConfig { .. }` and `SessionDefaults { .. }` literal it points to (known: `crates/forge-application/src/use_cases/extensions.rs:380`, `crates/forge-application/src/live_session.rs:469` and `:495`).

- [ ] **Step 5: Run the tests**

Run: `cargo test -p forge-domain -p forge-adapter-sidecar -p forge-features -p forge-shared -p forge-application -p forge-widgets`
Expected: all PASS (the Korean-table test included).

- [ ] **Step 6: Commit**

```bash
git add -A crates
git commit -m "feat: a turn_tools setting, on by default, sent to the sidecar at session start"
```

---

### Task 4: Reading the two tools' calls (pure logic)

**Files:**
- Create: `crates/forge-entities/src/turn_cards.rs`
- Modify: `crates/forge-entities/src/lib.rs`

**Interfaces:**
- Produces (all `pub`, re-exported from the crate root):
  - `enum TurnTool { UpdatePlan, WrapUp }`, `fn turn_tool(name: &str) -> Option<TurnTool>` (only the names with the `mcp__ccd_turn__` prefix).
  - `enum StepState { Pending, InProgress, Done }`, `struct Step { content: String, state: StepState }`, `fn apply(steps: &mut Vec<Step>, input: &serde_json::Value)`, `fn plan_of<'a>(calls: impl IntoIterator<Item = &'a ToolCall>) -> Option<Vec<Step>>`.
  - `struct CardAction { label: String, message: String }`, `struct EndCard { recap: String, ask: String, actions: Vec<CardAction> }`, `fn end_card_of(call: &ToolCall) -> Option<EndCard>`, `fn chosen_action<'a>(card: &'a EndCard, user_text: &str) -> Option<&'a CardAction>`.

- [ ] **Step 1: Write the failing tests** (create the file with only the test module first)

`crates/forge-entities/src/turn_cards.rs`:

```rust
#[cfg(test)]
mod tests {
    use forge_domain::{ToolCall, ToolResult};
    use serde_json::json;

    use super::*;

    fn call(name: &str, input: serde_json::Value, error: bool) -> ToolCall {
        ToolCall { id: "t".to_string(), name: name.to_string(), input, result: Some(ToolResult { output: "ok".to_string(), is_error: error }) }
    }
    fn plan(input: serde_json::Value) -> ToolCall {
        call("mcp__ccd_turn__update_plan", input, false)
    }
    fn wrap(input: serde_json::Value) -> ToolCall {
        call("mcp__ccd_turn__wrap_up", input, false)
    }
    fn states(steps: &[Step]) -> Vec<StepState> {
        steps.iter().map(|step| step.state).collect()
    }
    fn three() -> ToolCall {
        plan(json!({"items": [
            {"content": "Look", "status": "in_progress"},
            {"content": "Fix", "status": "pending"},
            {"content": "Test", "status": "pending"},
        ]}))
    }

    #[test]
    fn only_the_prefixed_names_are_the_two_tools() {
        assert_eq!(turn_tool("mcp__ccd_turn__update_plan"), Some(TurnTool::UpdatePlan));
        assert_eq!(turn_tool("mcp__ccd_turn__wrap_up"), Some(TurnTool::WrapUp));
        assert_eq!(turn_tool("update_plan"), None);
        assert_eq!(turn_tool("mcp__other__wrap_up"), None);
        assert_eq!(turn_tool("mcp__ccd_turn__other"), None);
    }

    #[test]
    fn items_make_the_plan_and_a_new_list_replaces_it() {
        let steps = plan_of([&three()]).unwrap();
        assert_eq!(steps.iter().map(|s| s.content.as_str()).collect::<Vec<_>>(), ["Look", "Fix", "Test"]);
        assert_eq!(states(&steps), [StepState::InProgress, StepState::Pending, StepState::Pending]);
        let again = plan(json!({"items": [{"content": "Only", "status": "completed"}]}));
        let steps = plan_of([&three(), &again]).unwrap();
        assert_eq!(steps, vec![Step { content: "Only".to_string(), state: StepState::Done }]);
    }

    #[test]
    fn advance_completes_the_current_step_and_starts_the_next() {
        let advance = plan(json!({"advance": true}));
        let steps = plan_of([&three(), &advance]).unwrap();
        assert_eq!(states(&steps), [StepState::Done, StepState::InProgress, StepState::Pending]);
        let steps = plan_of([&three(), &advance, &advance]).unwrap();
        assert_eq!(states(&steps), [StepState::Done, StepState::Done, StepState::InProgress]);
    }

    #[test]
    fn advance_after_the_last_step_leaves_everything_done() {
        let advance = plan(json!({"advance": true}));
        let steps = plan_of([&three(), &advance, &advance, &advance, &advance]).unwrap();
        assert_eq!(states(&steps), [StepState::Done, StepState::Done, StepState::Done]);
    }

    #[test]
    fn done_and_start_use_one_based_numbers_and_skip_numbers_outside_the_list() {
        let out_of_order = plan(json!({"done": [2], "start": 3}));
        let steps = plan_of([&three(), &out_of_order]).unwrap();
        assert_eq!(states(&steps), [StepState::Pending, StepState::Done, StepState::InProgress]);
        let outside = plan(json!({"done": [0, 9], "start": 7}));
        let steps = plan_of([&three(), &outside]).unwrap();
        assert_eq!(states(&steps), [StepState::InProgress, StepState::Pending, StepState::Pending], "nothing moved");
    }

    #[test]
    fn a_failed_call_does_not_change_the_plan_and_an_unknown_status_reads_as_pending() {
        let failed = call("mcp__ccd_turn__update_plan", json!({"advance": true}), true);
        let steps = plan_of([&three(), &failed]).unwrap();
        assert_eq!(states(&steps), [StepState::InProgress, StepState::Pending, StepState::Pending]);
        let odd = plan(json!({"items": [{"content": "A", "status": "weird"}, {"content": "  ", "status": "pending"}]}));
        let steps = plan_of([&odd]).unwrap();
        assert_eq!(steps, vec![Step { content: "A".to_string(), state: StepState::Pending }], "an empty step is dropped");
    }

    #[test]
    fn no_plan_call_or_an_empty_list_is_no_plan() {
        assert_eq!(plan_of([&call("Bash", json!({}), false)]), None);
        assert_eq!(plan_of([&plan(json!({"items": []}))]), None);
        assert_eq!(plan_of(std::iter::empty()), None);
    }

    #[test]
    fn at_most_twelve_steps_are_kept() {
        let items: Vec<_> = (0..15).map(|n| json!({"content": format!("s{n}"), "status": "pending"})).collect();
        assert_eq!(plan_of([&plan(json!({"items": items}))]).unwrap().len(), 12);
    }

    #[test]
    fn a_wrap_up_with_a_question_is_a_card_with_its_buttons() {
        let card = end_card_of(&wrap(json!({
            "recap": " Built it. ",
            "ask": "Commit?",
            "actions": [{"label": "Commit", "message": "commit it"}, {"label": "Not now"}],
        })))
        .unwrap();
        assert_eq!(card.recap, "Built it.");
        assert_eq!(card.ask, "Commit?");
        assert_eq!(
            card.actions,
            vec![
                CardAction { label: "Commit".to_string(), message: "commit it".to_string() },
                CardAction { label: "Not now".to_string(), message: "Not now".to_string() },
            ]
        );
    }

    #[test]
    fn a_wrap_up_without_a_question_or_a_recap_draws_no_card() {
        assert_eq!(end_card_of(&wrap(json!({"recap": "Done."}))), None, "no ask, no box");
        assert_eq!(end_card_of(&wrap(json!({"recap": "Done.", "ask": "  "}))), None);
        assert_eq!(end_card_of(&wrap(json!({"ask": "Commit?"}))), None);
        assert_eq!(end_card_of(&wrap(json!({"recap": "  ", "ask": "Commit?"}))), None);
        assert_eq!(end_card_of(&call("mcp__ccd_turn__wrap_up", json!({"recap": "x", "ask": "y"}), true)), None, "a refused call");
        assert_eq!(end_card_of(&call("Bash", json!({"recap": "x", "ask": "y"}), false)), None);
    }

    #[test]
    fn buttons_are_limited_and_cleaned() {
        let long_label = "L".repeat(40);
        let long_message = "M".repeat(400);
        let card = end_card_of(&wrap(json!({
            "recap": "r", "ask": "q",
            "actions": [{}, {"label": long_label, "message": long_message}, {"message": "only a message"}, {"label": "third"}],
        })))
        .unwrap();
        assert_eq!(card.actions.len(), 2, "the empty action is dropped, then at most two");
        assert_eq!(card.actions[0].label.chars().count(), 24);
        assert_eq!(card.actions[0].message.chars().count(), 300);
        assert_eq!(card.actions[1], CardAction { label: "only a message".to_string(), message: "only a message".to_string() });
    }

    #[test]
    fn a_button_is_chosen_by_an_exact_reply_ignoring_spaces_around_it() {
        let card = end_card_of(&wrap(json!({"recap": "r", "ask": "q", "actions": [{"label": "Yes", "message": "Yes, fix 1"}]}))).unwrap();
        assert_eq!(chosen_action(&card, "Yes, fix 1").map(|a| a.label.as_str()), Some("Yes"));
        assert_eq!(chosen_action(&card, "  Yes, fix 1\n").map(|a| a.label.as_str()), Some("Yes"));
        assert_eq!(chosen_action(&card, "yes, fix 1"), None);
        assert_eq!(chosen_action(&card, "something else"), None);
    }
}
```

Add `mod turn_cards;` to `crates/forge-entities/src/lib.rs` (next to `mod turn_view;`) and, below the other `pub use` lines, `pub use turn_cards::{apply, chosen_action, end_card_of, plan_of, turn_tool, CardAction, EndCard, Step, StepState, TurnTool};`

- [ ] **Step 2: Run to see it fail**

Run: `cargo test -p forge-entities turn_cards`
Expected: FAIL to compile (`apply`, `Step`, ... not found).

- [ ] **Step 3: Write the implementation** (put it above the test module in the same file)

```rust
//! The two tools the desktop app gives Claude, as Forge draws them: the task list folded from the
//! `update_plan` calls and the end-of-turn card read from a `wrap_up` call. No Dioxus in here.

use forge_domain::ToolCall;
use serde_json::Value;

const SERVER_PREFIX: &str = "mcp__ccd_turn__";
pub const MAX_STEPS: usize = 12;
pub const MAX_ACTIONS: usize = 2;
pub const MAX_LABEL_CHARS: usize = 24;
pub const MAX_MESSAGE_CHARS: usize = 300;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnTool {
    UpdatePlan,
    WrapUp,
}

/// Which of the two tools a call's name is. Only the desktop app's full name counts.
pub fn turn_tool(name: &str) -> Option<TurnTool> {
    match name.strip_prefix(SERVER_PREFIX)? {
        "update_plan" => Some(TurnTool::UpdatePlan),
        "wrap_up" => Some(TurnTool::WrapUp),
        _ => None,
    }
}

fn refused(call: &ToolCall) -> bool {
    call.result.as_ref().is_some_and(|result| result.is_error)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepState {
    Pending,
    InProgress,
    Done,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub content: String,
    pub state: StepState,
}

fn step_of(item: &Value) -> Option<Step> {
    let content = item.get("content")?.as_str()?.trim();
    if content.is_empty() {
        return None;
    }
    let state = match item.get("status").and_then(Value::as_str) {
        Some("in_progress") => StepState::InProgress,
        Some("completed") => StepState::Done,
        _ => StepState::Pending,
    };
    Some(Step { content: content.to_string(), state })
}

/// Applies one `update_plan` input to the list. `items` replaces it; otherwise `advance` completes the
/// step in progress and starts the first pending one, `done` completes the 1-based numbers, and `start`
/// makes one 1-based step the only one in progress. A number outside the list changes nothing.
pub fn apply(steps: &mut Vec<Step>, input: &Value) {
    if let Some(items) = input.get("items").and_then(Value::as_array) {
        *steps = items.iter().filter_map(step_of).take(MAX_STEPS).collect();
        return;
    }
    if input.get("advance").and_then(Value::as_bool) == Some(true) {
        if let Some(current) = steps.iter().position(|step| step.state == StepState::InProgress) {
            steps[current].state = StepState::Done;
        }
        if let Some(next) = steps.iter().position(|step| step.state == StepState::Pending) {
            steps[next].state = StepState::InProgress;
        }
    }
    if let Some(done) = input.get("done").and_then(Value::as_array) {
        for number in done.iter().filter_map(Value::as_u64) {
            if let Some(step) = number.checked_sub(1).and_then(|index| steps.get_mut(index as usize)) {
                step.state = StepState::Done;
            }
        }
    }
    if let Some(number) = input.get("start").and_then(Value::as_u64) {
        let target = number.checked_sub(1).map(|index| index as usize).filter(|index| *index < steps.len());
        if let Some(target) = target {
            for step in steps.iter_mut().filter(|step| step.state == StepState::InProgress) {
                step.state = StepState::Pending;
            }
            steps[target].state = StepState::InProgress;
        }
    }
}

/// The plan the calls build, in order; refused calls are skipped. `None` when there is no
/// `update_plan` call or the list ends up empty.
pub fn plan_of<'a>(calls: impl IntoIterator<Item = &'a ToolCall>) -> Option<Vec<Step>> {
    let mut steps = Vec::new();
    let mut seen = false;
    for call in calls {
        if turn_tool(&call.name) == Some(TurnTool::UpdatePlan) && !refused(call) {
            seen = true;
            apply(&mut steps, &call.input);
        }
    }
    (seen && !steps.is_empty()).then_some(steps)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardAction {
    pub label: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndCard {
    pub recap: String,
    pub ask: String,
    pub actions: Vec<CardAction>,
}

fn clip(text: &str, max: usize) -> String {
    text.trim().chars().take(max).collect()
}

fn text_of<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key)?.as_str().map(str::trim).filter(|text| !text.is_empty())
}

fn action_of(item: &Value) -> Option<CardAction> {
    let (label, message) = match (text_of(item, "label"), text_of(item, "message")) {
        (None, None) => return None,
        (Some(only), None) | (None, Some(only)) => (only, only),
        (Some(label), Some(message)) => (label, message),
    };
    Some(CardAction { label: clip(label, MAX_LABEL_CHARS), message: clip(message, MAX_MESSAGE_CHARS) })
}

/// The card a `wrap_up` call asks for. No card without a recap or without a question (the model's own
/// rule: a turn with nothing pending ends with no box), nor for a refused call.
pub fn end_card_of(call: &ToolCall) -> Option<EndCard> {
    if turn_tool(&call.name) != Some(TurnTool::WrapUp) || refused(call) {
        return None;
    }
    let recap = text_of(&call.input, "recap")?.to_string();
    let ask = text_of(&call.input, "ask")?.to_string();
    let actions = call
        .input
        .get("actions")
        .and_then(Value::as_array)
        .map(|list| list.iter().filter_map(action_of).take(MAX_ACTIONS).collect())
        .unwrap_or_default();
    Some(EndCard { recap, ask, actions })
}

/// The button a user message answers: its message equals the text (spaces around it ignored, case not).
pub fn chosen_action<'a>(card: &'a EndCard, user_text: &str) -> Option<&'a CardAction> {
    let sent = user_text.trim();
    card.actions.iter().find(|action| action.message == sent)
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p forge-entities turn_cards`
Expected: all PASS (12 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/forge-entities/src/turn_cards.rs crates/forge-entities/src/lib.rs
git commit -m "feat: read update_plan and wrap_up calls into a task list and an end card"
```

---

### Task 5: The two cards in the turn list

**Files:**
- Modify: `crates/forge-entities/src/turn_view.rs`, `crates/forge-entities/src/lib.rs`
- Test: `crates/forge-entities/src/turn_view.rs` (tests module)

**Interfaces:**
- Consumes: `turn_tool`, `TurnTool`, `plan_of`, `end_card_of`, `chosen_action`, `Step`, `EndCard` (Task 4).
- Produces: `Item::PlanList(Vec<Step>)`, `Item::EndCard(EndCardItem)`, `struct EndCardItem { card: EndCard, state: EndCardState }`, `enum EndCardState { Live, Spent { chosen: String } }`, `fn live_end_card(messages: &[Message]) -> Option<EndCard>` (the card a finished, unanswered last turn shows), all re-exported from the crate root.

- [ ] **Step 1: Write the failing tests** (in the tests module of `turn_view.rs`, with the other helpers; `tool`, `assistant`, `text`, `activity`, `kinds` exist; extend `kinds` first)

In `kinds()` add two arms:

```rust
                Item::PlanList(_) => "planlist",
                Item::EndCard(_) => "endcard",
```

Add the tests:

```rust
    fn update_plan(id: &str, input: serde_json::Value, done: bool) -> Block {
        tool(id, "mcp__ccd_turn__update_plan", input, done.then_some(false))
    }
    fn wrap_up(id: &str, ask: Option<&str>, actions: serde_json::Value) -> Block {
        let mut input = json!({"recap": "Built it.", "actions": actions});
        if let Some(ask) = ask {
            input["ask"] = json!(ask);
        }
        tool(id, "mcp__ccd_turn__wrap_up", input, Some(false))
    }
    fn two_steps() -> serde_json::Value {
        json!({"items": [{"content": "Look", "status": "in_progress"}, {"content": "Fix", "status": "pending"}]})
    }
    fn running() -> LiveState {
        LiveState::default()
    }

    #[test]
    fn the_task_list_shows_while_the_turn_runs_above_the_live_line_and_never_after() {
        let messages = vec![Message::user("go"), assistant(vec![text("Looking."), update_plan("p", two_steps(), false)])];
        let items = build_turns(&messages, TranscriptView::Normal, Some(&running()));
        assert_eq!(kinds(&items), vec!["user", "text", "planlist", "live"]);
        let Item::PlanList(steps) = &items[2] else { panic!() };
        assert_eq!(steps.len(), 2);
        let finished = build_turns(&messages, TranscriptView::Normal, None);
        assert!(!kinds(&finished).contains(&"planlist"));
    }

    #[test]
    fn the_two_tools_are_not_rows_and_not_counted_in_the_folded_line() {
        let messages = vec![
            Message::user("go"),
            assistant(vec![update_plan("p", two_steps(), true), bash("b", None), wrap_up("w", Some("Commit?"), json!([{"label": "Commit"}]))]),
        ];
        let items = build_turns(&messages, TranscriptView::Normal, None);
        let a = activity(&items);
        assert_eq!(a.rows.len(), 1);
        assert_eq!(a.summary, "Ran ls -la");
    }

    #[test]
    fn a_turn_of_only_the_two_tools_has_no_folded_line() {
        let messages = vec![Message::user("go"), assistant(vec![update_plan("p", two_steps(), true), text("done")])];
        let items = build_turns(&messages, TranscriptView::Normal, None);
        assert!(!kinds(&items).contains(&"activity"));
    }

    #[test]
    fn a_running_turn_tool_call_does_not_become_the_live_label() {
        let messages = vec![Message::user("go"), assistant(vec![update_plan("p", two_steps(), false)])];
        let items = build_turns(&messages, TranscriptView::Normal, Some(&running()));
        let Some(Item::Live(line)) = items.last() else { panic!() };
        assert_eq!(line.kind, LiveKind::Thinking, "no raw tool name");
    }

    #[test]
    fn a_finished_last_turn_with_a_wrap_up_question_shows_a_live_end_card_before_the_footer() {
        let messages = vec![Message::user("go"), assistant(vec![bash("b", None), wrap_up("w", Some("Commit?"), json!([{"label": "Commit", "message": "commit it"}])), text("All done.")])];
        let items = build_turns(&messages, TranscriptView::Normal, None);
        assert_eq!(kinds(&items), vec!["user", "activity", "text", "endcard", "footer"]);
        let Item::EndCard(card) = &items[3] else { panic!() };
        assert_eq!(card.state, EndCardState::Live);
        assert_eq!(card.card.ask, "Commit?");
        assert_eq!(live_end_card(&messages).map(|c| c.ask), Some("Commit?".to_string()));
        let running_items = build_turns(&messages, TranscriptView::Normal, Some(&running()));
        assert!(!kinds(&running_items).contains(&"endcard"), "not while the turn runs");
    }

    #[test]
    fn no_question_no_card_and_only_the_last_wrap_up_counts() {
        let none = vec![Message::user("go"), assistant(vec![wrap_up("w", None, json!([])), text("x")])];
        assert!(!kinds(&build_turns(&none, TranscriptView::Normal, None)).contains(&"endcard"));
        assert_eq!(live_end_card(&none), None);
        let twice = vec![Message::user("go"), assistant(vec![wrap_up("a", Some("First?"), json!([])), wrap_up("b", Some("Second?"), json!([])), text("x")])];
        let items = build_turns(&twice, TranscriptView::Normal, None);
        assert_eq!(items.iter().filter(|i| matches!(i, Item::EndCard(_))).count(), 1);
        let Some(Item::EndCard(card)) = items.iter().find(|i| matches!(i, Item::EndCard(_))) else { panic!() };
        assert_eq!(card.card.ask, "Second?");
    }

    #[test]
    fn a_reply_that_equals_a_button_leaves_a_spent_card_and_any_other_message_removes_it() {
        let first = vec![Message::user("go"), assistant(vec![wrap_up("w", Some("Commit?"), json!([{"label": "Commit", "message": "commit it"}])), text("x")])];
        let mut chosen = first.clone();
        chosen.push(Message::user("commit it"));
        chosen.push(assistant(vec![text("Committed.")]));
        let items = build_turns(&chosen, TranscriptView::Normal, None);
        let Some(Item::EndCard(card)) = items.iter().find(|i| matches!(i, Item::EndCard(_))) else { panic!("spent card kept") };
        assert_eq!(card.state, EndCardState::Spent { chosen: "Commit".to_string() });
        assert_eq!(live_end_card(&chosen), None, "no live card once the turn after it exists");
        let mut other = first.clone();
        other.push(Message::user("something else"));
        other.push(assistant(vec![text("ok")]));
        assert!(!kinds(&build_turns(&other, TranscriptView::Normal, None)).contains(&"endcard"));
    }

    #[test]
    fn a_resumed_session_answered_with_other_text_has_no_leftover_card() {
        let mut messages = vec![Message::user("go"), assistant(vec![wrap_up("w", Some("Commit?"), json!([{"label": "Yes"}])), text("x")])];
        messages.push(Message::user("Yes!"));
        let items = build_turns(&messages, TranscriptView::Normal, Some(&running()));
        assert!(!kinds(&items).contains(&"endcard"));
    }
```

- [ ] **Step 2: Run to see them fail**

Run: `cargo test -p forge-entities turn_view`
Expected: FAIL to compile (`Item::PlanList`, `EndCardState`, `live_end_card` not found).

- [ ] **Step 3: Implement**

In `turn_view.rs` imports add `use crate::turn_cards::{chosen_action, end_card_of, plan_of, turn_tool, EndCard, Step, TurnTool};`.

Add the types next to `PlanItem`:

```rust
/// The end-of-turn card Claude filled with `wrap_up`, and what became of it.
#[derive(Debug, Clone, PartialEq)]
pub struct EndCardItem {
    pub card: EndCard,
    pub state: EndCardState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EndCardState {
    /// The last turn, not answered yet: the buttons work.
    Live,
    /// The next message was one of its buttons: kept in grey with the button's label.
    Spent { chosen: String },
}
```

Add to `enum Item` (after `Plan(PlanItem),`):

```rust
    /// The task list Claude keeps with `update_plan`, shown only while the turn runs.
    PlanList(Vec<Step>),
    /// The next-step card Claude fills with `wrap_up`.
    EndCard(EndCardItem),
```

Add helpers (near `plan_proposal`):

```rust
fn is_turn_tool(call: &ToolCall) -> bool {
    turn_tool(&call.name).is_some()
}

/// What follows a turn: nothing (it is the last one) or the next user message.
#[derive(Clone, Copy)]
enum Next<'a> {
    End,
    Message(&'a str),
}

fn last_wrap_up(blocks: &[Block]) -> Option<&ToolCall> {
    blocks.iter().rev().find_map(|block| match block {
        Block::Tool(call) if turn_tool(&call.name) == Some(TurnTool::WrapUp) => Some(call),
        _ => None,
    })
}

fn end_card_item(blocks: &[Block], next: Next<'_>) -> Option<Item> {
    let card = end_card_of(last_wrap_up(blocks)?)?;
    let state = match next {
        Next::End => EndCardState::Live,
        Next::Message(text) => EndCardState::Spent { chosen: chosen_action(&card, text)?.label.clone() },
    };
    Some(Item::EndCard(EndCardItem { card, state }))
}

/// The card of the last turn when nothing has answered it yet; the composer uses it to give up its
/// grey suggestion and to take `⌘⏎` as the first button.
pub fn live_end_card(messages: &[Message]) -> Option<EndCard> {
    let start = messages.iter().rposition(|message| message.role == MessageRole::User).map_or(0, |index| index + 1);
    let blocks: Vec<Block> = messages[start..].iter().flat_map(|message| message.blocks.iter().cloned()).collect();
    end_card_of(last_wrap_up(&blocks)?)
}
```

In `live_line`, change the running-tool condition:

```rust
        if call.result.is_none() && !is_turn_tool(call) {
```

In `push_segment`:
- live branch: replace `items.push(Item::Live(live_line(blocks, tool_rows, live)));` with

```rust
        if let Some(steps) = plan_of(blocks.iter().filter_map(|block| if let Block::Tool(call) = block { Some(call) } else { None })) {
            items.push(Item::PlanList(steps));
        }
        items.push(Item::Live(live_line(blocks, tool_rows, live)));
```
- the `.filter(|(block, _)| !matches!(block, Block::Tool(call) if workflow_launch(call).is_some() || plan_proposal(call).is_some()))` becomes
`... if workflow_launch(call).is_some() || plan_proposal(call).is_some() || is_turn_tool(call)))`.

`finish_turn` gets a last parameter `next: Next<'_>`; after `items.push(Item::FileCard(card));` block and before the `Footer` push add:

```rust
        items.extend(end_card_item(&blocks, next));
```
Order inside that block: file card, end card, footer. Update both calls in `build_turns`: the user branch becomes

```rust
            MessageRole::User => {
                let text = message.text();
                finish_turn(&mut items, std::mem::take(&mut blocks), at.take(), view, None, Next::Message(&text));
                items.push(Item::User(message.clone()));
            }
```
and the last call `finish_turn(&mut items, blocks, at, view, live, Next::End);`.

`lib.rs`: extend the `turn_view` re-export with `live_end_card, EndCardItem, EndCardState`.

- [ ] **Step 4: Run the crate tests and fix compile errors in other crates**

Run: `cargo test -p forge-entities` then `cargo build --workspace`
Expected: PASS and a clean build; if another crate matches `Item` exhaustively, add the two new arms there (the compiler names them; Task 6 draws them, so for now a `_ => rsx! {}` arm in `TurnItem` is acceptable only until Task 6 replaces it).

- [ ] **Step 5: Commit**

```bash
git add -A crates/forge-entities
git commit -m "feat: the turn list holds the live task list and the end card; the two tools are hidden as rows"
```

---

### Task 6: Draw the cards

**Files:**
- Create: `crates/forge-entities/src/turn_card_views.rs`
- Modify: `crates/forge-entities/src/turn.rs`, `crates/forge-entities/src/lib.rs`, `crates/forge-shared/src/theme.css`, `crates/forge-widgets/src/chat_window.rs`, `crates/forge-shared/src/ko.rs`

**Interfaces:**
- Consumes: `Step`, `StepState`, `EndCardItem`, `EndCardState` (Tasks 4 and 5).
- Produces: components `PlanListView { steps: Vec<Step> }` and `EndCardView { item: EndCardItem, on_send: Option<EventHandler<String>> }`; a `Turns` prop `on_send: Option<EventHandler<String>>`.

- [ ] **Step 1: Write the failing test for the pure part**

The icon class is the only logic. In `turn_card_views.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_state_has_its_own_icon_class() {
        assert_eq!(step_class(StepState::Done), "plan-icon done");
        assert_eq!(step_class(StepState::InProgress), "plan-icon doing");
        assert_eq!(step_class(StepState::Pending), "plan-icon waiting");
    }
}
```

Run: `cargo test -p forge-entities turn_card_views` → FAIL (does not compile).

- [ ] **Step 2: Write the components** (above the test module)

```rust
//! The two cards of the task-list tools: the live task list and the end-of-turn card.

use dioxus::prelude::*;
use forge_shared::tr;

use crate::turn_cards::{Step, StepState};
use crate::turn_view::{EndCardItem, EndCardState};

fn step_class(state: StepState) -> &'static str {
    match state {
        StepState::Done => "plan-icon done",
        StepState::InProgress => "plan-icon doing",
        StepState::Pending => "plan-icon waiting",
    }
}

/// The task list of a running turn: a ring or a check and the step's words, one row each.
#[component]
pub fn PlanListView(steps: Vec<Step>) -> Element {
    rsx! {
        div { class: "plan-list",
            for (index, step) in steps.iter().enumerate() {
                div { key: "{index}", class: "plan-step",
                    span { class: step_class(step.state) }
                    span { class: "plan-step-text", "{step.content}" }
                }
            }
        }
    }
}

/// The next-step card. Live: a close button, the recap, the question and the buttons (the first is
/// the primary one and carries `⌘⏎`). Spent: narrower and grey, the chosen button's label under the
/// question, nothing to press.
#[component]
pub fn EndCardView(item: EndCardItem, on_send: Option<EventHandler<String>>) -> Element {
    let mut hidden = use_signal(|| false);
    if hidden() {
        return rsx! {};
    }
    let EndCardItem { card, state } = item;
    match state {
        EndCardState::Spent { chosen } => rsx! {
            div { class: "end-card spent",
                div { class: "end-card-recap", "{card.recap}" }
                div { class: "end-card-ask", "{card.ask}" }
                div { class: "end-card-chosen", "{chosen}" }
            }
        },
        EndCardState::Live => rsx! {
            div { class: "end-card",
                button {
                    class: "end-card-close",
                    r#type: "button",
                    title: tr!("Close"),
                    "aria-label": tr!("Close"),
                    onclick: move |_| hidden.set(true),
                    "×"
                }
                div { class: "end-card-recap", "{card.recap}" }
                div { class: "end-card-ask", "{card.ask}" }
                if !card.actions.is_empty() {
                    div { class: "end-card-actions",
                        for (index, action) in card.actions.iter().enumerate() {
                            button {
                                key: "{index}",
                                class: if index == 0 { "end-card-button primary" } else { "end-card-button" },
                                r#type: "button",
                                onclick: {
                                    let message = action.message.clone();
                                    move |_| {
                                        if let Some(send) = on_send {
                                            send.call(message.clone());
                                        }
                                    }
                                },
                                "{action.label}"
                                if index == 0 {
                                    span { class: "end-card-key", "⌘⏎" }
                                }
                            }
                        }
                    }
                }
            }
        },
    }
}
```

`lib.rs`: `mod turn_card_views;`.

- [ ] **Step 3: Wire them into `Turns`**

In `turn.rs`: `use crate::turn_card_views::{EndCardView, PlanListView};`. Add to `Turns` props `#[props(default)] on_send: Option<EventHandler<String>>,`; pass `on_send` into every `TurnItem { .. }` call and add `on_send: Option<EventHandler<String>>,` to `TurnItem`'s parameters. Add the two arms to the `match item` (replace any placeholder arm from Task 5):

```rust
        Item::PlanList(steps) => rsx! { PlanListView { steps } },
        Item::EndCard(item) => rsx! { EndCardView { item, on_send } },
```

In `crates/forge-widgets/src/chat_window.rs`, in the `Turns { .. }` call after `on_fork:` add

```rust
                    on_send: move |text: String| {
                        if let Some(handle) = workspace.handle(id) {
                            handle.send_with(text, Vec::new());
                        }
                    },
```

- [ ] **Step 4: Style it** (append to `crates/forge-shared/src/theme.css`; the variables exist in `:root` and the dark block)

```css
.plan-list { margin: 8px 0; padding: 10px 14px; border: 1px solid var(--border); border-radius: var(--radius); background: var(--bg-card); display: flex; flex-direction: column; gap: 8px; }
.plan-step { display: flex; align-items: center; gap: 10px; font-size: 14px; color: var(--fg); }
.plan-icon { flex: none; width: 16px; height: 16px; border-radius: 999px; box-sizing: border-box; }
.plan-icon.waiting { border: 1.5px solid var(--fg-muted); }
.plan-icon.doing { border: 1.5px dashed var(--fg-muted); animation: plan-spin 3s linear infinite; }
.plan-icon.done { background: var(--fg-muted); position: relative; }
.plan-icon.done::after { content: ""; position: absolute; left: 5px; top: 2px; width: 4px; height: 8px; border: solid var(--bg-card); border-width: 0 2px 2px 0; transform: rotate(45deg); }
.plan-step:has(.done) .plan-step-text { color: var(--fg-muted); }
@keyframes plan-spin { to { transform: rotate(360deg); } }
@media (prefers-reduced-motion: reduce) { .plan-icon.doing { animation: none; } }
.end-card { position: relative; margin: 10px 0; padding: 14px 16px; border: 1px solid var(--border); border-radius: var(--radius); background: var(--bg-card); display: flex; flex-direction: column; gap: 10px; }
.end-card-close { position: absolute; top: 8px; right: 10px; border: none; background: none; color: var(--fg-muted); font: inherit; font-size: 16px; cursor: pointer; }
.end-card-close:hover { color: var(--fg); }
.end-card-recap { padding-right: 24px; color: var(--fg-muted); font-size: 14px; line-height: 1.5; }
.end-card-ask { color: var(--fg); font-size: 14px; font-weight: 500; }
.end-card-actions { display: flex; gap: 8px; flex-wrap: wrap; }
.end-card-button { display: inline-flex; align-items: center; gap: 8px; padding: 6px 12px; border: 0; border-radius: 8px; background: var(--bg-hover); color: var(--fg); font: inherit; font-size: 13px; cursor: pointer; }
.end-card-button.primary { background: var(--fg); color: var(--bg-card); }
.end-card-key { padding: 0 5px; border-radius: 4px; background: rgba(255, 255, 255, 0.2); font-size: 11px; }
.end-card.spent { width: 70%; min-width: 260px; }
.end-card.spent .end-card-ask { color: var(--fg-muted); font-weight: 400; }
.end-card-chosen { color: var(--fg-muted); font-size: 13px; }
```

- [ ] **Step 5: Korean text**

`tr!("Close")` already exists in `ko.rs`. No new sentence is added in this task; run `cargo test -p forge-shared` to confirm.

- [ ] **Step 6: Run tests and build**

Run: `cargo test -p forge-entities -p forge-widgets -p forge-shared && cargo build --workspace`
Expected: PASS, clean build.

- [ ] **Step 7: Commit**

```bash
git add -A crates
git commit -m "feat: draw the task list and the next-step card; a button sends its message"
```

---

### Task 7: The composer follows a live end card

**Files:**
- Modify: `crates/forge-features/src/suggestion.rs`, `crates/forge-features/src/send_message.rs`, `crates/forge-features/src/lib.rs` (only if the helper must be exported)
- Test: `crates/forge-features/src/suggestion.rs`

**Interfaces:**
- Consumes: `forge_entities::live_end_card`, `forge_entities::EndCard`.
- Produces: `fn end_card_reply(card: Option<&EndCard>, draft_empty: bool, running: bool) -> Option<String>` (the message `⌘⏎` sends, when it applies).

- [ ] **Step 1: Write the failing tests** (in `suggestion.rs` tests module)

```rust
    fn card(actions: &[(&str, &str)]) -> forge_entities::EndCard {
        forge_entities::EndCard {
            recap: "r".to_string(),
            ask: "q".to_string(),
            actions: actions.iter().map(|(label, message)| forge_entities::CardAction { label: label.to_string(), message: message.to_string() }).collect(),
        }
    }

    #[test]
    fn command_enter_in_an_empty_idle_box_sends_the_first_button_of_a_live_card() {
        let with = card(&[("Commit", "commit it"), ("Not now", "not now")]);
        assert_eq!(end_card_reply(Some(&with), true, false).as_deref(), Some("commit it"));
    }

    #[test]
    fn it_does_nothing_with_text_in_the_box_a_running_turn_no_card_or_no_buttons() {
        let with = card(&[("Commit", "commit it")]);
        assert_eq!(end_card_reply(Some(&with), false, false), None);
        assert_eq!(end_card_reply(Some(&with), true, true), None);
        assert_eq!(end_card_reply(None, true, false), None);
        assert_eq!(end_card_reply(Some(&card(&[])), true, false), None);
    }
```

Run: `cargo test -p forge-features end_card_reply` → FAIL (no such function).

- [ ] **Step 2: Implement the helper** (above the tests in `suggestion.rs`)

```rust
/// The message `⌘⏎` sends while a next-step card is live: its first button's, when the box is empty and
/// no turn runs. With text in the box `⌘⏎` keeps queueing it.
pub fn end_card_reply(card: Option<&forge_entities::EndCard>, draft_empty: bool, running: bool) -> Option<String> {
    if !draft_empty || running {
        return None;
    }
    card?.actions.first().map(|action| action.message.clone())
}
```

- [ ] **Step 3: Use it in the composer** (`send_message.rs`)

Import: `use crate::suggestion::{composer_placeholder, end_card_reply, suggestion_to_accept};` (extend the existing `use`).

After `let suggestion = view.and_then(|view| view.read().suggestion.clone());` replace the suggestion lines with

```rust
    // A live next-step card is the suggestion: the grey text gives way to it.
    let end_card = view.and_then(|view| forge_entities::live_end_card(&view.read().session.messages)).filter(|_| !running);
    let suggestion = if end_card.is_some() { None } else { view.and_then(|view| view.read().suggestion.clone()) };
    let accept = suggestion.clone();
```
(delete the original two lines `let suggestion = …; let accept = suggestion.clone();`).

Change the `onqueue` handler on `CommandTextArea` to

```rust
                onqueue: {
                    let end_card = end_card.clone();
                    move |_| {
                        // ⌘⏎ on an empty box with a live card presses the card's first button.
                        if let Some(message) = end_card_reply(end_card.as_ref(), draft.peek().is_empty(), running) {
                            if let Some(handle) = workspace.handle(id) {
                                handle.send_with(message, Vec::new());
                            }
                            return;
                        }
                        queue()
                    }
                },
```
(if the closure moves `queue` and the compiler complains that `queue` is `FnMut`, declare `let mut queue = queue;` first, or keep the closure `move |_| { …; queue() }` with `queue` already `mut` as it is above.)

- [ ] **Step 4: Run tests and build**

Run: `cargo test -p forge-features && cargo build --workspace`
Expected: PASS, clean build.

- [ ] **Step 5: Commit**

```bash
git add -A crates/forge-features
git commit -m "feat: a live next-step card replaces the grey suggestion and ⌘⏎ presses its first button"
```

---

### Task 8: Look at it, run a real turn, and write it down

**Files:**
- Create: `scripts/harness/add_turn_cards_samples.py`
- Modify: `docs/superpowers/specs/2026-10-28-turn-tools-design.md`, `docs/superpowers/roadmap/backlog.md`, the memory file `plan-70-turn-tools.md` and `MEMORY.md`

- [ ] **Step 1: A harness sample with both cards**

`scripts/harness/add_turn_cards_samples.py` (same shape as `add_activity_samples.py`):

```python
#!/usr/bin/env python3
"""Adds two sessions to the harness fixture, for the task-list tools: "Next step card" ends in a wrap_up question
(a live end card) and "Answered card" has the same card answered with its first button (a spent card).

Run it through launch.sh:  FORGE_HARNESS_EXTRA=scripts/harness/add_turn_cards_samples.py scripts/harness/launch.sh
"""
import json
import os
from pathlib import Path

import build_fixture as bf

OPEN_ID = "00000000-0000-4000-8000-000000000301"
SPENT_ID = "00000000-0000-4000-8000-000000000302"
PLAN = {"items": [{"content": "Read the code", "status": "completed"}, {"content": "Fix the bug", "status": "in_progress"}, {"content": "Run the tests", "status": "pending"}], "scope": "first_task"}
WRAP = {
    "recap": "The login bug is fixed and the tests pass. Nothing is committed yet.",
    "ask": "Commit the fix?",
    "actions": [{"label": "Commit", "message": "Commit the fix"}, {"label": "Not now", "message": "Not now"}],
}


def lines_for(session_id, cwd, answered):
    def user(text):
        return {"type": "user", "cwd": str(cwd), "sessionId": session_id, "message": {"role": "user", "content": text}}

    def assistant(block):
        return {"type": "assistant", "sessionId": session_id, "message": {"role": "assistant", "content": [block]}}

    def call(number, name, tool_input, output):
        tool_id = f"toolu_{session_id[-3:]}_{number}"
        return [
            assistant({"type": "tool_use", "id": tool_id, "name": name, "input": tool_input}),
            {"type": "user", "sessionId": session_id, "message": {"role": "user", "content": [
                {"type": "tool_result", "tool_use_id": tool_id, "content": output}]}},
        ]

    out = [user("Fix the login bug")]
    out += call(1, "mcp__ccd_turn__update_plan", PLAN, "Plan shown (3 steps).")
    out += call(2, "Bash", {"command": "cargo test", "description": "Run the tests"}, "ok")
    out += call(3, "mcp__ccd_turn__wrap_up", WRAP, "Shown in the next-step box. Your closing message is your answer to the user: write it now, in full, as the last thing you say this turn. The box asks your question, so do not ask it again.")
    out.append(assistant({"type": "text", "text": "I fixed the login bug and ran the tests; they pass."}))
    if answered:
        out.append(user("Commit the fix"))
        out.append(assistant({"type": "text", "text": "Committed."}))
    return out


def add(claude_home, projects_root):
    cwd = Path(projects_root) / "my-app"
    project_dir = Path(claude_home) / "projects" / bf.escape_project_path(cwd)
    for session_id, title, answered, hour in ((OPEN_ID, "Next step card", False, 19), (SPENT_ID, "Answered card", True, 18)):
        transcript = project_dir / f"{session_id}.jsonl"
        lines = lines_for(session_id, cwd, answered)
        lines.insert(1, {"type": "custom-title", "customTitle": title, "sessionId": session_id})
        transcript.write_text("".join(json.dumps(line) + "\n" for line in lines))
        mtime = bf._epoch(30, hour)
        os.utime(transcript, (mtime, mtime))


if __name__ == "__main__":
    run_dir = Path(os.environ.get("FORGE_HARNESS_DIR", "/private/tmp/forge-harness-home"))
    add(run_dir / ".claude", Path(os.path.realpath(bf.DEFAULT_PROJECTS_ROOT)))
```

- [ ] **Step 2: Look at it in the real window**

A fresh worktree needs the sidecar's modules: `ln -s /Users/james/Developer/forge/sidecar/node_modules sidecar/node_modules` if it is missing. Then:

```bash
FORGE_HARNESS_EXTRA=scripts/harness/add_turn_cards_samples.py scripts/harness/launch.sh
sleep 6 && python3 scripts/harness/capture.py
```
Read `/private/tmp/forge-harness-shot.png`; click (capture pixels, shown × 1.20) the session `Next step card`. Expected: the folded line `명령 1개 실행` only (no plan or wrap row), the closing text, then the card: recap in grey, `Commit the fix?` in black, a black `Commit ⌘⏎` button and a grey `Not now`, `×` at the top right; the composer shows its normal placeholder, not a suggestion. Click `×`: the card hides. Open `Answered card`: the grey narrow card with `Commit` under the question, no buttons. Press the first button in `Next step card` after reopening it: the message `Commit the fix` is sent (the real sidecar answers or fails; a failure banner is fine, the bubble must appear). `scripts/harness/stop.sh` when done. Note what was not checked (dark mode, Windows, the plan card needs a running turn, see Step 3).

- [ ] **Step 3: A real turn with the real SDK** (the only proof that the model uses the tools)

In a throwaway folder (`/private/tmp/forge-turn-check`, `git init` it), with the real Claude home (not the fixture), start the built app (`cargo run -p forge-app` with Settings left at the default, turn tools on), open that folder, send: `Add a file notes.txt with three lines, then show me what you did.` Watch: the task list card appears while it works and is gone at the end; the turn ends with a next-step card or not; no permission question for the two tools. Record what happened, including how often the model called each tool over three different requests. If the model calls neither tool, adjust the two descriptions in `sidecar/src/turn_tools.ts` (the rules in `UPDATE_PLAN_DESCRIPTION` and `WRAP_UP_DESCRIPTION`), rebuild with `npm run build` in `sidecar/`, and repeat; this adjusting is part of this task. If the SDK refuses the in-process server because of `strictMcpConfig: true`, set it to `!turnTools` in `session.ts` (only when `turn_tools` is on the flag is dropped), add a test line in `session.test.ts` for it, and note it in the report.

- [ ] **Step 4: Correct the spec's five lines, update the backlog, write the memory**

Edit `docs/superpowers/specs/2026-10-28-turn-tools-design.md`:
- In section 1 remove the sentence that the sidecar cuts long labels and messages; add: "The label and message limits (24 and 300 characters) are applied by the app when it reads the call, since the recorded call keeps what the model sent."
- In section 3 replace "Both tool names are matched with and without the `mcp__ccd_turn__` prefix of the desktop app's server, plus Forge's own server name if it differs (it does not)." with "Only the full name `mcp__ccd_turn__…` counts (Forge uses the desktop app's server name)."
- In section 3, `end_card_of`: replace "no `ask` -> no actions are shown, the box would have nothing to answer" with "no `ask` -> no card at all (the tool's own rule: a turn with nothing pending ends with no box)".
- In section 3, `Item::PlanList`: replace "placed at the position of the **last** `update_plan` call of the turn" with "placed just above the live line, after the text that came before the first tool call".
- In section 4, replace "kept in the session's view state, not saved" with "kept only while the card is on screen (a component-local flag)".

In `docs/superpowers/roadmap/backlog.md` set NEW-25's Forge cell to `has` and its Plan cell to `Plan 70 (built on plan-70, window and real SDK checked <date>; see the report for what was not checked)`; set NEW-07's Notes to say the end card is NEW-25's.

Write `/Users/james/.claude/projects/-Users-james-Developer-forge/memory/plan-70-turn-tools.md` (what was built, the real-SDK result and any wording changes, what is left: `blocked`, the sidebar status, app-action buttons) and add its line to `MEMORY.md`.

- [ ] **Step 5: Full test run and commit**

Run: `cargo test --workspace --no-fail-fast` (the plugin `round_trip` test is the known flaky one), `cd sidecar && npm test && npm run typecheck`.
Expected: everything else passes.

```bash
git add -A
git commit -m "docs: harness samples for the turn cards, spec corrections, backlog and notes for Plan 70"
```
