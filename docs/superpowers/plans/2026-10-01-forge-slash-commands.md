# Forge Slash Command Autocomplete Implementation Plan (Plan 11)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Typing `/` in the message box lists the slash commands the SDK reports, even before the first message of a session.

**Architecture:** The sidecar asks the SDK for its commands right after the query starts (`supportedCommands()`) and again on `system/init`, and sends a new `commands_available` event. Rust keeps the list in `LiveSessionView.commands`. A session now starts its sidecar when it is opened (a `WarmUp` command sent by the registry), so the list exists before the first send. The message box shows a popup built from pure functions in `forge-entities`.

**Tech Stack:** TypeScript sidecar (vitest, Agent SDK 0.3.283), Rust workspace (tokio, Dioxus 0.7.10).

**Spec:** `docs/superpowers/specs/2026-09-27-forge-design.md` §21 (and §20 for the pattern this follows).

## Global Constraints

- Commit messages end with the line `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.
- Commit subject style: `feat: ...`, `fix: ...`, `docs: ...`, lower case, as in `git log`.
- A command name is stored without the leading `/` (spec §21.2).
- The popup is open only while the input starts with `/` and has no whitespace after it (spec §21.4).
- Keys: Up/Down move and wrap, Enter or Tab insert `/name `, Esc closes until the input changes (spec §21.1, §21.4). These are Forge's own choices; the desktop docs do not specify them.
- A failing or slow `supportedCommands()` is never an error and never blocks a turn (spec §21.3).
- Do not add the **+** button, argument completion, or a cross-session cache (spec §21.1 out of scope).
- Run commands from the worktree root `/Users/james/orca/workspaces/forge/plan-11`.

## Review Focus

1. `supportedCommands()` that never answers or throws: no event, no error, the turn is unaffected (Task 2 tests).
2. A command name that arrives with a leading `/`, empty, non-string or duplicated: shown once, without the slash (Task 2 tests).
3. The warm-up sidecar fails to start: no error banner, and the first send starts it again and reports its own error (Task 4 test).
4. Closing a session right after opening it, while the warm-up is starting: the session ends cleanly (Task 4 test).
5. Enter that confirms an IME composition (Korean) must not pick a command (Task 5 test `menu_action`).

Known cost, accepted in the spec's choice of "start the SDK early": every opened session now holds a sidecar process, and the extensions are resolved when the session opens, not at its first send.

---

### Task 1: Spike — what the real SDK returns before any message

**Files:**
- Create (throwaway, deleted at the end of the task): `sidecar/spike_commands.mjs`
- Modify: `docs/superpowers/specs/2026-09-27-forge-design.md` (append §21.7)

**Interfaces:**
- Produces: the answers to spec §21.6 (1)–(4), which Tasks 2 and 4 rely on.

- [ ] **Step 1: Install the sidecar dependencies**

Run: `cd sidecar && npm ci`
Expected: installs without error (Node 22; `node --version` should print `v22.x`).

- [ ] **Step 2: Write the probe**

```js
// sidecar/spike_commands.mjs — throwaway.
import { query } from "@anthropic-ai/claude-agent-sdk";

const withMessage = process.argv.includes("--message");
const queue = [];
let wake = null;
const prompt = (async function* () {
  while (true) {
    if (queue.length === 0) await new Promise((resolve) => (wake = resolve));
    yield queue.shift();
  }
})();

const q = query({ prompt, options: { cwd: process.cwd(), settingSources: ["project"], includePartialMessages: true } });
const started = Date.now();

const answer = await Promise.race([
  q.supportedCommands(),
  new Promise((resolve) => setTimeout(() => resolve("TIMEOUT after 30 s"), 30000)),
]);
console.log("supportedCommands() after", Date.now() - started, "ms:");
console.log(JSON.stringify(answer, null, 1).slice(0, 1500));
console.log("count:", Array.isArray(answer) ? answer.length : "n/a");

if (withMessage) {
  queue.push({ type: "user", message: { role: "user", content: "say ok" }, parent_tool_use_id: null });
  wake?.();
  for await (const message of q) {
    if (message.type === "system" && message.subtype === "init") {
      console.log("init.slash_commands:", JSON.stringify(message.slash_commands));
      break;
    }
  }
}
await q.interrupt?.().catch(() => {});
process.exit(0);
```

- [ ] **Step 3: Run it without a message**

Run: `cd sidecar && node spike_commands.mjs`
Expected: it prints a time in ms and a list of objects (note the exact field names), or `TIMEOUT`. Write the numbers down.

- [ ] **Step 4: Run it with a message**

Run: `cd sidecar && node spike_commands.mjs --message`
Expected: the same list, then `init.slash_commands: [...]`. Compare the names with the first list: are they the same, with or without a leading `/`, and do they include the skills and plugins that are enabled?

- [ ] **Step 5: Record the findings in the spec**

Append to the spec a `### 21.7 Spike result` section with, for items (1)–(4) of §21.6, what was measured (the time, the fields, whether a leading `/` is present, whether the init list differs). If `supportedCommands()` did **not** answer before a message, write `Fallback: Tasks 2 and 4 keep only the init path; no warm-up` and continue: Task 4 is skipped, and the ledger gets a `Ruling:` line for it.

- [ ] **Step 6: Delete the probe and commit**

```bash
rm sidecar/spike_commands.mjs
git add docs/superpowers/specs/2026-09-27-forge-design.md
git commit -m "docs: record the Plan 11 spike findings (commands before the first message)" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Sidecar sends `commands_available`

**Files:**
- Modify: `sidecar/src/protocol.ts`
- Modify: `sidecar/src/session.ts`
- Test: `sidecar/test/session.test.ts`

**Interfaces:**
- Consumes: Task 1's findings (field names of `supportedCommands()`; this plan assumes `{ name, description, argumentHint }` as in the SDK's `SlashCommand` type; if Task 1 found otherwise, change `toWireCommands` only).
- Produces: wire event `{ type: "commands_available", commands: Array<{ name: string; description?: string; argument_hint?: string }> }`. Names have no leading `/`, are non-empty and unique.

- [ ] **Step 1: Write the failing tests**

Add to `sidecar/test/session.test.ts`, after the `contextQuery` tests:

```ts
  function commandsQuery(
    supportedCommands: () => Promise<unknown>,
    initCommands?: string[],
    waitMs = 30,
  ): QueryFn {
    return () =>
      toQueryHandle(
        (async function* () {
          await new Promise((resolve) => setTimeout(resolve, waitMs));
          if (initCommands) yield { type: "system", subtype: "init", session_id: "s-1", slash_commands: initCommands };
          await new Promise((resolve) => setTimeout(resolve, waitMs));
        })(),
        { supportedCommands },
      );
  }

  function commandEvents(events: SidecarEvent[]) {
    return events.filter((e) => e.type === "commands_available");
  }

  it("sends commands_available from supportedCommands() before any message", async () => {
    const events: SidecarEvent[] = [];
    const session = new SidecarSession(
      commandsQuery(async () => [
        { name: "compact", description: "Free up context", argumentHint: "[instructions]" },
        { name: "review", description: "" },
      ]),
      (e) => events.push(e),
    );
    await session.handle(startCommand);

    expect(commandEvents(events)).toEqual([
      {
        type: "commands_available",
        commands: [
          { name: "compact", description: "Free up context", argument_hint: "[instructions]" },
          { name: "review" },
        ],
      },
    ]);
  });

  it("drops a leading slash, empty and non-string names and duplicates", async () => {
    const events: SidecarEvent[] = [];
    const session = new SidecarSession(
      commandsQuery(async () => [{ name: "/compact" }, { name: "compact" }, { name: "" }, { name: 3 }, null, { name: "/" }]),
      (e) => events.push(e),
    );
    await session.handle(startCommand);

    expect(commandEvents(events)).toEqual([{ type: "commands_available", commands: [{ name: "compact" }] }]);
  });

  it("does not send the init list again when it has the same names", async () => {
    const events: SidecarEvent[] = [];
    const session = new SidecarSession(
      commandsQuery(async () => [{ name: "compact", description: "Free up context" }], ["compact"]),
      (e) => events.push(e),
    );
    await session.handle(startCommand);

    expect(commandEvents(events)).toHaveLength(1);
  });

  it("sends the init list when its names differ, and when supportedCommands() gave nothing", async () => {
    const events: SidecarEvent[] = [];
    const session = new SidecarSession(
      commandsQuery(async () => [{ name: "compact" }], ["compact", "review"]),
      (e) => events.push(e),
    );
    await session.handle(startCommand);
    expect(commandEvents(events)).toEqual([
      { type: "commands_available", commands: [{ name: "compact" }] },
      { type: "commands_available", commands: [{ name: "compact" }, { name: "review" }] },
    ]);

    const later: SidecarEvent[] = [];
    const second = new SidecarSession(
      commandsQuery(async () => { throw new Error("nope"); }, ["compact"]),
      (e) => later.push(e),
    );
    await second.handle(startCommand);
    expect(commandEvents(later)).toEqual([{ type: "commands_available", commands: [{ name: "compact" }] }]);
  });

  it("sends nothing and no error when supportedCommands() throws, and the session carries on", async () => {
    const events: SidecarEvent[] = [];
    const session = new SidecarSession(
      commandsQuery(async () => { throw new Error("nope"); }),
      (e) => events.push(e),
    );
    await session.handle(startCommand);

    expect(commandEvents(events)).toEqual([]);
    expect(events.map((e) => e.type)).toEqual(["session_ended"]);
  });

  it("gives up on a supportedCommands() that never answers", async () => {
    const events: SidecarEvent[] = [];
    const session = new SidecarSession(
      commandsQuery(() => new Promise(() => {}), undefined, 60),
      (e) => events.push(e),
      { commandsTimeoutMs: 10 },
    );
    await session.handle(startCommand);

    expect(commandEvents(events)).toEqual([]);
    expect(events.map((e) => e.type)).toEqual(["session_ended"]);
  });
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cd sidecar && npx vitest run test/session.test.ts -t "commands"`
Expected: FAIL (a type error on `commands_available` or `commandsTimeoutMs`, or empty event lists).

- [ ] **Step 3: Add the event to the protocol**

In `sidecar/src/protocol.ts`, add above `SidecarCommand`:

```ts
export interface WireSlashCommand {
  name: string;
  description?: string;
  argument_hint?: string;
}
```

and in `SidecarEvent`, after `context_usage`:

```ts
  | { type: "commands_available"; commands: WireSlashCommand[] }
```

- [ ] **Step 4: Implement it in the session**

In `sidecar/src/session.ts`:

1. Import: `import type { SidecarCommand, SidecarEvent, WirePermissionDecision, WireSlashCommand } from "./protocol.js";`
2. Extend `QueryHandle`:
```ts
  supportedCommands?: () => Promise<unknown>;
```
3. Constants and helper, next to `CONTEXT_USAGE_TIMEOUT_MS`:
```ts
const COMMANDS_TIMEOUT_MS = 10000;

// The SDK may list a name with or without its slash; Forge keeps it without.
function toWireCommands(raw: unknown): WireSlashCommand[] {
  if (!Array.isArray(raw)) return [];
  const seen = new Set<string>();
  const commands: WireSlashCommand[] = [];
  for (const item of raw) {
    const entry = typeof item === "string" ? { name: item } : (item as { name?: unknown } | null);
    const name = typeof entry?.name === "string" ? entry.name.replace(/^\//, "") : "";
    if (name === "" || seen.has(name)) continue;
    seen.add(name);
    const command: WireSlashCommand = { name };
    const { description, argumentHint } = entry as { description?: unknown; argumentHint?: unknown };
    if (typeof description === "string" && description !== "") command.description = description;
    if (typeof argumentHint === "string" && argumentHint !== "") command.argument_hint = argumentHint;
    commands.push(command);
  }
  return commands;
}
```
4. Fields and constructor option:
```ts
  private lastCommandNames: string | null = null;
  private readonly commandsTimeoutMs: number;
```
Constructor options type becomes `{ contextUsageTimeoutMs?: number; commandsTimeoutMs?: number }` and add `this.commandsTimeoutMs = options.commandsTimeoutMs ?? COMMANDS_TIMEOUT_MS;`.
5. Methods, next to `emitContextUsage`:
```ts
  // Sent only when the names changed, so the poorer init list (names only) never replaces the richer
  // supportedCommands() list that has the same names.
  private emitCommands(raw: unknown): void {
    const commands = toWireCommands(raw);
    const key = commands.map((c) => c.name).join("\n");
    if (this.shuttingDown || commands.length === 0 || key === this.lastCommandNames) return;
    this.lastCommandNames = key;
    this.emit({ type: "commands_available", commands });
  }

  // A failure or a slow answer only means no list yet; it never touches the session.
  private async emitSupportedCommands(): Promise<void> {
    const query = this.activeQuery;
    if (!query?.supportedCommands) return;
    try {
      this.emitCommands(await withTimeout(query.supportedCommands(), this.commandsTimeoutMs));
    } catch {
      // No list from this call; `init` may still bring one.
    }
  }
```
6. In `run`, right after `this.activeQuery = query;` add `void this.emitSupportedCommands();`.
7. In `forwardMessage`, in the `init` branch, after the `session_started` emit add `this.emitCommands(message.slash_commands);`.

- [ ] **Step 5: Run the tests to see them pass**

Run: `cd sidecar && npx vitest run`
Expected: PASS, the whole sidecar suite (the earlier count was 60 tests; six new ones are added). The existing `forwards a session init...` test still passes because its query has no `supportedCommands` and its `init` has no `slash_commands`.

- [ ] **Step 6: Type-check and commit**

Run: `cd sidecar && npm run typecheck`
Expected: no output.

```bash
git add sidecar/src/protocol.ts sidecar/src/session.ts sidecar/test/session.test.ts
git commit -m "feat: the sidecar reports the SDK's slash commands as commands_available" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Rust keeps the command list in the session view

**Files:**
- Create: `crates/forge-domain/src/command.rs`
- Modify: `crates/forge-domain/src/lib.rs`
- Modify: `crates/forge-ports/src/sidecar_port.rs`
- Modify: `crates/forge-application/src/event.rs`
- Modify: `crates/forge-application/src/live_session.rs` (view field, constructor, test)
- Modify: `crates/forge-application/src/live_session_actor.rs`
- Modify: `crates/forge-features/src/sidebar.rs:384` (test constructor)

**Interfaces:**
- Produces: `forge_domain::SlashCommand { name: String, description: Option<String>, argument_hint: Option<String> }`; `SidecarEvent::CommandsAvailable { commands: Vec<SlashCommand> }`; `SessionUpdate::CommandsAvailable(Vec<SlashCommand>)`; `LiveSessionView.commands: Vec<SlashCommand>` (empty until the first event; replaced, not merged).

- [ ] **Step 1: Write the failing tests**

In `crates/forge-ports/src/sidecar_port.rs` tests:
```rust
    #[test]
    fn commands_available_parses_with_and_without_the_optional_fields() {
        let event: SidecarEvent = serde_json::from_str(
            r#"{"type":"commands_available","commands":[{"name":"compact","description":"Free up context","argument_hint":"[text]"},{"name":"review"}]}"#,
        )
        .unwrap();
        assert_eq!(
            event,
            SidecarEvent::CommandsAvailable {
                commands: vec![
                    forge_domain::SlashCommand {
                        name: "compact".to_string(),
                        description: Some("Free up context".to_string()),
                        argument_hint: Some("[text]".to_string()),
                    },
                    forge_domain::SlashCommand { name: "review".to_string(), description: None, argument_hint: None },
                ]
            }
        );
    }
```
In `crates/forge-application/src/event.rs` tests:
```rust
    #[test]
    fn commands_available_is_reported_and_leaves_the_session_alone() {
        let mut session = empty_session();
        let commands = vec![forge_domain::SlashCommand { name: "compact".to_string(), description: None, argument_hint: None }];
        let update = apply_sidecar_event(&mut session, &SidecarEvent::CommandsAvailable { commands: commands.clone() });
        assert_eq!(update, SessionUpdate::CommandsAvailable(commands));
        assert!(session.messages.is_empty());
    }
```
In `crates/forge-application/src/live_session.rs` tests:
```rust
    #[tokio::test]
    async fn a_commands_update_replaces_the_last_list() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        assert!(handle.snapshot().commands.is_empty());

        handle.send("go".to_string());
        let named = |name: &str| forge_domain::SlashCommand { name: name.to_string(), description: None, argument_hint: None };
        control.push(SidecarEvent::CommandsAvailable { commands: vec![named("a"), named("b")] });
        control.push(SidecarEvent::CommandsAvailable { commands: vec![named("c")] });
        let seen = wait_for(&mut view, "the second list", |v| v.commands == vec![named("c")]).await;

        assert_eq!(seen.commands.len(), 1);
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p forge-ports -p forge-application commands 2>&1 | tail -20`
Expected: compile error, `SlashCommand` / `CommandsAvailable` / `commands` not found.

- [ ] **Step 3: Implement**

`crates/forge-domain/src/command.rs`:
```rust
use serde::{Deserialize, Serialize};

/// A slash command the SDK reports for a session. `name` has no leading `/`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlashCommand {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub argument_hint: Option<String>,
}
```
`lib.rs`: add `mod command;` (alphabetical, before `config`) and `pub use command::SlashCommand;`.

`sidecar_port.rs`: import `SlashCommand` with the other `forge_domain` names and add to `SidecarEvent`, after `ContextUsage`:
```rust
    /// The slash commands the SDK reports; each event replaces the earlier list.
    CommandsAvailable { commands: Vec<SlashCommand> },
```
`event.rs`: import `SlashCommand`; add `CommandsAvailable(Vec<SlashCommand>),` to `SessionUpdate` after `ContextUsage`; add the arm in `apply_sidecar_event`:
```rust
        SidecarEvent::CommandsAvailable { commands } => SessionUpdate::CommandsAvailable(commands.clone()),
```
`live_session.rs`: import `SlashCommand`; add to `LiveSessionView` after `context`:
```rust
    /// The slash commands the SDK reported; empty until the first report.
    pub commands: Vec<SlashCommand>,
```
and `commands: Vec::new(),` after `context: None,` in `spawn_live_session`. Same line in `crates/forge-features/src/sidebar.rs` test constructor (after `context: None,`).
`live_session_actor.rs`, in `apply_event`, after the `ContextUsage` arm:
```rust
            SessionUpdate::CommandsAvailable(commands) => self.view.commands = commands,
```

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test --workspace 2>&1 | tail -15`
Expected: all pass (454 before this plan plus the three new ones).

- [ ] **Step 5: Commit**

```bash
git add crates
git commit -m "feat: sessions keep the slash commands the sidecar reports" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Open sessions warm up their sidecar

Skip this task (and write `Ruling:` in the ledger) only if Task 1 recorded the fallback.

**Files:**
- Modify: `crates/forge-application/src/live_session.rs` (`LiveCommand`, handle method, tests)
- Modify: `crates/forge-application/src/live_session_actor.rs`
- Modify: `crates/forge-application/src/registry.rs` (`insert`, and any test it breaks)

**Interfaces:**
- Consumes: `Actor::start_sidecar` (exists), `LiveSessionHandle`.
- Produces: `LiveCommand::WarmUp`; `LiveSessionHandle::warm_up(&self)`. The registry calls it for every session it opens whose project exists.

- [ ] **Step 1: Write the failing tests**

In `live_session.rs` tests:
```rust
    #[tokio::test]
    async fn warming_up_starts_the_sidecar_before_any_send_and_the_commands_arrive() {
        let (sidecar, control) = fake_sidecar();
        let (factory, spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();

        handle.warm_up();
        let command = forge_domain::SlashCommand { name: "compact".to_string(), description: None, argument_hint: None };
        control.push(SidecarEvent::CommandsAvailable { commands: vec![command.clone()] });
        let seen = wait_for(&mut view, "the commands", |v| !v.commands.is_empty()).await;

        assert_eq!(spawns.load(Ordering::SeqCst), 1);
        assert_eq!(control.calls(), vec![Call::Start(start_params(None))]);
        assert_eq!(seen.commands, vec![command]);
        assert_eq!(seen.phase, Phase::Idle);
        assert_eq!(seen.last_error, None);
    }

    #[tokio::test]
    async fn the_first_send_after_a_warm_up_reuses_the_sidecar() {
        let (sidecar, control) = fake_sidecar();
        let (factory, spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();

        handle.warm_up();
        handle.send("hello".to_string());
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;

        assert_eq!(spawns.load(Ordering::SeqCst), 1);
        assert_eq!(control.calls(), vec![Call::Start(start_params(None)), Call::Send("hello".to_string())]);
    }

    #[tokio::test]
    async fn a_failed_warm_up_shows_no_error_and_the_first_send_starts_the_sidecar_again() {
        let (sidecar, control) = fake_sidecar();
        let (factory, spawns) = factory_returning(vec![Err(SidecarError::SpawnFailed("no node".to_string())), Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();

        handle.warm_up();
        handle.send("hello".to_string());
        let seen = wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;

        assert_eq!(spawns.load(Ordering::SeqCst), 2);
        assert_eq!(seen.last_error, None);
        assert_eq!(control.calls(), vec![Call::Start(start_params(None)), Call::Send("hello".to_string())]);
    }

    #[tokio::test]
    async fn a_session_that_cannot_send_does_not_warm_up() {
        let (factory, spawns) = factory_returning(vec![]);
        let handle = spawn_live_session(LiveSessionInit { can_send: false, ..new_init(factory) });
        let mut view = handle.subscribe();

        handle.warm_up();
        handle.close();
        wait_closed(&mut view).await;

        assert_eq!(spawns.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn closing_right_after_a_warm_up_ends_the_session() {
        let (sidecar, _control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();

        handle.warm_up();
        handle.close();

        wait_closed(&mut view).await;
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p forge-application warm 2>&1 | tail -15`
Expected: compile error, `warm_up` not found.

- [ ] **Step 3: Implement**

`live_session.rs`: add `WarmUp,` to `LiveCommand` (after `Send`), and on the handle:
```rust
    /// Starts the session's sidecar now instead of at the first send, so its slash commands are known
    /// before the person types. A failure is not shown: the first send starts the sidecar again.
    pub fn warm_up(&self) {
        let _ = self.commands.send(LiveCommand::WarmUp);
    }
```
`live_session_actor.rs`: in `handle_command` add `LiveCommand::WarmUp => self.warm_up().await,` and the method (next to `send`):
```rust
    async fn warm_up(&mut self) {
        if !self.view.can_send || self.sidecar.is_some() {
            return;
        }
        // Not `fail`: nothing was asked of the session yet, so there is nothing to report. The first
        // send resolves and spawns again and shows its own error if it fails too.
        let _ = self.start_sidecar().await;
    }
```
`registry.rs`, in `insert`, after `handles.insert(id, handle.clone());`:
```rust
        if project.exists {
            handle.warm_up();
        }
```

- [ ] **Step 4: Run the whole application suite**

Run: `cargo test -p forge-application 2>&1 | tail -30`
Expected: the five new tests pass. Registry tests that script a factory for the first send may now fail because the warm-up spawned first (message `the test scripted fewer sidecars than were spawned`, or a changed spawn count). For each one: if the test is about something other than spawning, give it a sidecar for the warm-up (`factory_returning` with the extra `Ok(...)`) or assert the new count; if it asserts "opening spawns nothing" at the registry level, change the assertion to one spawn and say why. Ledger every such change as `Ruling:`. Do not weaken `opening_a_session_spawns_no_sidecar` in `live_session.rs`: it tests `spawn_live_session`, which does not warm up.

- [ ] **Step 5: Run the workspace and commit**

Run: `cargo test --workspace 2>&1 | tail -8`
Expected: all pass.

```bash
git add crates
git commit -m "feat: an opened session starts its sidecar at once so its slash commands are known" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 5: Pure command menu logic

**Files:**
- Create: `crates/forge-entities/src/command_menu.rs`
- Modify: `crates/forge-entities/src/lib.rs`

**Interfaces:**
- Consumes: `forge_domain::SlashCommand`.
- Produces (all in `forge_entities`):
  - `command_query(input: &str) -> Option<&str>`
  - `filter_commands<'a>(commands: &'a [SlashCommand], query: &str) -> Vec<&'a SlashCommand>`
  - `move_selection(current: usize, len: usize, delta: isize) -> usize`
  - `insert_text(command: &SlashCommand) -> String`
  - `enum MenuKey { Up, Down, Enter, Tab, Escape, Other }`, `enum MenuAction { Move(isize), Pick, Close }`
  - `menu_action(key: MenuKey, composing: bool) -> Option<MenuAction>`

- [ ] **Step 1: Write the failing tests**

`crates/forge-entities/src/command_menu.rs`, tests first (with the empty `use` lines the tests need), then the functions in Step 3:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn cmd(name: &str) -> SlashCommand {
        SlashCommand { name: name.to_string(), description: None, argument_hint: None }
    }

    #[test]
    fn the_query_is_the_text_after_the_slash_while_it_is_one_word() {
        assert_eq!(command_query("/"), Some(""));
        assert_eq!(command_query("/com"), Some("com"));
        assert_eq!(command_query("/compact now"), None, "a space ends the command name");
        assert_eq!(command_query("/a\nb"), None);
        assert_eq!(command_query("hello /x"), None);
        assert_eq!(command_query(""), None);
        assert_eq!(command_query("/가나"), Some("가나"));
    }

    #[test]
    fn prefix_matches_come_before_substring_matches_in_the_given_order() {
        let commands = vec![cmd("recompact"), cmd("compact"), cmd("context"), cmd("review")];
        let names: Vec<_> = filter_commands(&commands, "co").iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["compact", "context", "recompact"]);
    }

    #[test]
    fn filtering_ignores_case_and_an_empty_query_lists_everything() {
        let commands = vec![cmd("Compact"), cmd("review")];
        assert_eq!(filter_commands(&commands, "COM").len(), 1);
        assert_eq!(filter_commands(&commands, "").len(), 2);
        assert!(filter_commands(&commands, "zzz").is_empty());
    }

    #[test]
    fn the_selection_wraps_in_both_directions_and_survives_an_empty_list() {
        assert_eq!(move_selection(0, 3, 1), 1);
        assert_eq!(move_selection(2, 3, 1), 0);
        assert_eq!(move_selection(0, 3, -1), 2);
        assert_eq!(move_selection(5, 3, 0), 2, "a stale index is brought back into range");
        assert_eq!(move_selection(0, 0, 1), 0);
    }

    #[test]
    fn a_picked_command_becomes_a_slash_name_and_a_space() {
        assert_eq!(insert_text(&cmd("compact")), "/compact ");
    }

    #[test]
    fn keys_map_to_menu_actions_and_enter_during_an_ime_composition_does_nothing() {
        assert_eq!(menu_action(MenuKey::Up, false), Some(MenuAction::Move(-1)));
        assert_eq!(menu_action(MenuKey::Down, false), Some(MenuAction::Move(1)));
        assert_eq!(menu_action(MenuKey::Enter, false), Some(MenuAction::Pick));
        assert_eq!(menu_action(MenuKey::Tab, false), Some(MenuAction::Pick));
        assert_eq!(menu_action(MenuKey::Escape, false), Some(MenuAction::Close));
        assert_eq!(menu_action(MenuKey::Other, false), None);
        assert_eq!(menu_action(MenuKey::Enter, true), None);
        assert_eq!(menu_action(MenuKey::Tab, true), None);
    }
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p forge-entities command_menu 2>&1 | tail -10`
Expected: compile error (functions not defined).

- [ ] **Step 3: Implement**

Top of `command_menu.rs`:
```rust
use forge_domain::SlashCommand;

/// The text after the `/` while the input is still just a command name: it starts with `/` and has no
/// whitespace. `None` means the menu is closed.
pub fn command_query(input: &str) -> Option<&str> {
    let rest = input.strip_prefix('/')?;
    if rest.chars().any(char::is_whitespace) {
        None
    } else {
        Some(rest)
    }
}

/// Commands whose name starts with `query`, then those that contain it; case-insensitive, each group
/// in the order the SDK gave.
pub fn filter_commands<'a>(commands: &'a [SlashCommand], query: &str) -> Vec<&'a SlashCommand> {
    let query = query.to_lowercase();
    let mut prefix = Vec::new();
    let mut inner = Vec::new();
    for command in commands {
        let name = command.name.to_lowercase();
        if name.starts_with(&query) {
            prefix.push(command);
        } else if name.contains(&query) {
            inner.push(command);
        }
    }
    prefix.extend(inner);
    prefix
}

/// The selected row after moving `delta` rows, wrapping around; always inside `0..len` (0 for no rows).
pub fn move_selection(current: usize, len: usize, delta: isize) -> usize {
    if len == 0 {
        return 0;
    }
    let current = current.min(len - 1) as isize;
    (current + delta).rem_euclid(len as isize) as usize
}

/// What the message box holds after a command is picked.
pub fn insert_text(command: &SlashCommand) -> String {
    format!("/{} ", command.name)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuKey {
    Up,
    Down,
    Enter,
    Tab,
    Escape,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    Move(isize),
    Pick,
    Close,
}

/// What an open menu does with a key. Enter or Tab that confirms an IME composition (Korean, Japanese,
/// Chinese) belongs to the composition, so it does nothing here.
pub fn menu_action(key: MenuKey, composing: bool) -> Option<MenuAction> {
    match key {
        MenuKey::Up => Some(MenuAction::Move(-1)),
        MenuKey::Down => Some(MenuAction::Move(1)),
        MenuKey::Enter | MenuKey::Tab if !composing => Some(MenuAction::Pick),
        MenuKey::Escape => Some(MenuAction::Close),
        _ => None,
    }
}
```
`lib.rs`: add `mod command_menu;` and
`pub use command_menu::{command_query, filter_commands, insert_text, menu_action, move_selection, MenuAction, MenuKey};`

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test -p forge-entities 2>&1 | tail -8`
Expected: all pass, six new.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-entities
git commit -m "feat: pure filtering, selection and key rules for the slash command menu" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 6: The popup in the message box

**Files:**
- Modify: `crates/forge-features/src/send_message.rs`
- Modify: `crates/forge-shared/src/theme.css` (and `theme.rs` only if its tests list the CSS rules)
- Modify: `docs/superpowers/specs/2026-09-27-forge-design.md` (§21.8 As built)

**Interfaces:**
- Consumes: Task 5's functions, `LiveSessionView.commands` (Task 3).

- [ ] **Step 1: Wire the popup into `SendMessageBox`**

In `send_message.rs`:

1. Imports: `use forge_entities::{command_query, filter_commands, insert_text, menu_action, move_selection, MenuAction, MenuKey};` and `use forge_domain::SlashCommand;`
2. Signals after `seen_failures`: 
```rust
    let mut selected = use_signal(|| 0usize);
    let mut dismissed = use_signal(|| false);
```
3. After `view` is read and before `rsx!`, compute the rows (owned, so the closures can keep them):
```rust
    let matches: Vec<SlashCommand> = match view {
        Some(view) if !dismissed() && can_send => {
            let text = draft.read();
            match command_query(&text) {
                Some(query) => filter_commands(&view.read().commands, query).into_iter().cloned().collect(),
                None => Vec::new(),
            }
        }
        _ => Vec::new(),
    };
    let current = move_selection(selected(), matches.len(), 0);
```
4. A closure that picks a row:
```rust
    let pick = {
        let matches = matches.clone();
        move |index: usize| {
            if let Some(command) = matches.get(index) {
                draft.set(insert_text(command));
                selected.set(0);
            }
        }
    };
```
5. Replace the `oninput` with:
```rust
                oninput: move |text| {
                    draft.set(text);
                    selected.set(0);
                    dismissed.set(false);
                },
```
6. Replace the `onkeydown` with:
```rust
                onkeydown: {
                    let mut pick = pick.clone();
                    let count = matches.len();
                    move |event: KeyboardEvent| {
                        if count > 0 {
                            let key = match event.key() {
                                Key::ArrowUp => MenuKey::Up,
                                Key::ArrowDown => MenuKey::Down,
                                Key::Enter => MenuKey::Enter,
                                Key::Tab => MenuKey::Tab,
                                Key::Escape => MenuKey::Escape,
                                _ => MenuKey::Other,
                            };
                            if let Some(action) = menu_action(key, event.is_composing()) {
                                event.prevent_default();
                                match action {
                                    MenuAction::Move(delta) => selected.set(move_selection(selected(), count, delta)),
                                    MenuAction::Pick => pick(selected()),
                                    MenuAction::Close => dismissed.set(true),
                                }
                                return;
                            }
                        }
                        // Enter that confirms an IME composition (Korean, Japanese, Chinese) must not send.
                        if event.key() == Key::Enter && !event.modifiers().shift() && !event.is_composing() {
                            event.prevent_default();
                            submit();
                        }
                    }
                },
```
(`pick` and `submit` are `FnMut` closures over signals; if the borrow checker objects, make them `let mut` and clone as needed. The behaviour above is what matters.)

7. Inside `div { class: "composer", ... }`, before `TextArea`:
```rust
            if !matches.is_empty() {
                div { class: "command-menu", role: "listbox",
                    for (index, command) in matches.iter().enumerate() {
                        div {
                            key: "{command.name}",
                            role: "option",
                            class: if index == current { "command-item selected" } else { "command-item" },
                            // mousedown, not click: a click would first blur the text box.
                            onmousedown: {
                                let mut pick = pick.clone();
                                move |event: MouseEvent| {
                                    event.prevent_default();
                                    pick(index);
                                }
                            },
                            span { class: "command-name", "/{command.name}" }
                            if let Some(hint) = command.argument_hint.as_ref() {
                                span { class: "command-hint", "{hint}" }
                            }
                            if let Some(description) = command.description.as_ref() {
                                span { class: "command-desc", "{description}" }
                            }
                        }
                    }
                }
            }
```

- [ ] **Step 2: Add the styles**

In `crates/forge-shared/src/theme.css`, next to the `.usage-popover` rules (use the tokens already used there):
```css
.command-menu { position: absolute; left: 0; right: 0; bottom: calc(100% + 8px); z-index: 10; max-height: 240px; overflow-y: auto; padding: 4px; box-sizing: border-box; border: 1px solid var(--border); border-radius: var(--radius); background: var(--bg-card); box-shadow: var(--shadow); font-size: 13px; }
.command-item { display: flex; align-items: baseline; gap: 8px; padding: 5px 8px; border-radius: 6px; cursor: pointer; }
.command-item.selected, .command-item:hover { background: var(--bg-hover); }
.command-name { flex: none; font-weight: 600; }
.command-hint { flex: none; color: var(--fg-muted); }
.command-desc { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--fg-muted); font-size: 12px; }
```
If `.composer` has no `position: relative`, add it to that rule (the menu is positioned against it).

- [ ] **Step 3: Build and run the whole suite**

Run: `cargo build --workspace 2>&1 | tail -20 && cargo test --workspace 2>&1 | tail -8`
Expected: builds; all tests pass. `theme.rs` has tests for the stylesheet: if one lists the rules or checks both palettes, keep it green.

- [ ] **Step 4: Check it in the running app**

Use the `run` skill to start the app. Open a new session without sending anything and type `/`: the list should appear (this is the warm-up). Check: typing narrows it; Up/Down move and wrap; Enter and Tab insert `/name `; Esc closes it and typing reopens it; clicking a row inserts it; typing `/x ` (with a space) closes it; Korean IME composition with Enter does not pick; Enter with the menu closed still sends; the popup works in light and dark. Note what could not be checked.

- [ ] **Step 5: Record "As built" in the spec**

Append `### 21.8 As built` to §21: the branch, the check date, test counts (Rust workspace and sidecar before/after), what was checked in the live window and what was not, differences from §21 and any `Ruling:` that changed behaviour (the registry tests changed in Task 4).

- [ ] **Step 6: Commit**

```bash
git add crates docs
git commit -m "feat: slash command popup above the message box; record Plan 11 as built" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```
