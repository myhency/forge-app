# Forge Foundation: Domain, Ports & Sidecar Protocol — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stand up the hexagonal core's foundation — `forge-domain`, `forge-ports`, and a working `forge-adapter-sidecar` that drives a bundled Node.js sidecar (using `@anthropic-ai/claude-agent-sdk`) over a stdio JSON protocol — and empirically resolve the one open risk in the design spec (whether the Agent SDK's session transcripts are format-compatible with the `claude` CLI's).

This is **Plan 1 of several** for the Forge project (see spec §Scope). Deliberately out of scope here, deferred to later plans: `forge-application` (use-cases), `forge-adapter-config` / `forge-adapter-claude-fs` (config and shared-state adapters), the FSD UI crates, and packaging/bundling. What this plan delivers is independently testable: a Rust process can spawn the sidecar, drive a full session lifecycle, and receive typed events back — with no UI yet.

**Architecture:** Two-process model. `forge-adapter-sidecar` (Rust, `tokio::process`) spawns one Node.js child process per session and exchanges newline-delimited JSON commands/events over its stdin/stdout. The Node sidecar wraps `@anthropic-ai/claude-agent-sdk`'s `query()` with an internal async-generator message queue so a single long-lived `query()` call can receive multiple `send_message` commands over time (multi-turn), and bridges the SDK's `canUseTool` callback to the wire protocol's `permission_request`/`permission_response` commands.

**Tech Stack:** Rust (`tokio`, `async-trait`, `serde`/`serde_json`, `thiserror`, `mockall` for tests), Node.js 22 + TypeScript (`@anthropic-ai/claude-agent-sdk`, `vitest`).

**Spec:** `docs/superpowers/specs/2026-09-27-forge-design.md`

## Global Constraints

- Every bundled component (Node runtime, Agent SDK, sidecar npm deps) is pinned to an exact version — no floating semver ranges (spec §3, §9).
- Sidecar communication is stdio, newline-delimited JSON — no local HTTP/WebSocket server (spec §3).
- One sidecar child process per open chat session, not a shared multiplexing sidecar (spec §3).
- Forge's own config (base_url, api_key, enabled_skills/plugins/mcp_servers) lives in a Forge-private plaintext `config.json`, never inside `~/.claude/` (spec §4.1) — not touched by this plan, listed for consistency.
- Skill/plugin/MCP-server *definitions* are read from Claude Code's shared files; only *activation state* is Forge-private (spec §4.3) — not touched by this plan.
- `forge-application` depends only on `forge-domain` + `forge-ports`, never on a concrete adapter crate (spec §6).
- FSD frontend layering (`shared → entities → features → widgets → pages → app`) is enforced by the Cargo dependency graph (spec §6) — not touched by this plan.
- App name is "Forge" — no "Claude" in the name (spec §11).

## Review Focus

- Sidecar process crashes or exits mid-session → `next_event()` must resolve to `None` rather than hang forever (Task 7).
- A malformed/non-JSON line on the sidecar's stdout must not crash the Rust reader or silently drop the next valid line (Task 7).
- Sending further commands while events are actively streaming back must not deadlock the stdin writer or stdout reader (Task 7).
- A very large single event payload (e.g. a big tool_result) must be read completely, not truncated (Task 7).
- A `permission_response` for an unknown or already-resolved `permission_request_id` must be ignored, not crash the sidecar (Task 5).

---

## Prerequisites (human partner, before Task 1)

This machine currently has no Rust toolchain installed. Before starting:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
# restart the shell, then:
rustup default stable
cargo install dioxus-cli --locked
```

Node.js v22.22.1 is already installed and is what this plan pins against (`.nvmrc`, `engines` field).

---

### Task 1: Workspace scaffold + `forge-domain`

**Files:**
- Create: `Cargo.toml` (workspace root)
- Create: `crates/forge-domain/Cargo.toml`
- Create: `crates/forge-domain/src/lib.rs`
- Create: `crates/forge-domain/src/permission.rs`
- Test: unit tests inline in `crates/forge-domain/src/permission.rs`

**Interfaces:**
- Consumes: nothing (first crate)
- Produces: `forge_domain::{PermissionMode, PermissionDecision}` — both `Copy + Clone + Debug + PartialEq + Eq + Serialize + Deserialize`, serializing to `snake_case` strings (`"default"`, `"accept_edits"`, `"bypass_permissions"`, `"plan"`; `"allow_once"`, `"allow_always"`, `"deny"`)

- [ ] **Step 1: Create the workspace root**

`Cargo.toml`:
```toml
[workspace]
resolver = "2"
members = ["crates/*"]

[workspace.package]
version = "0.1.0"
edition = "2021"
```

- [ ] **Step 2: Write the failing test for `forge-domain`**

`crates/forge-domain/Cargo.toml`:
```toml
[package]
name = "forge-domain"
version.workspace = true
edition.workspace = true

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

`crates/forge-domain/src/lib.rs`:
```rust
mod permission;

pub use permission::{PermissionDecision, PermissionMode};
```

`crates/forge-domain/src/permission.rs`:
```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionMode {
    Default,
    AcceptEdits,
    BypassPermissions,
    Plan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionDecision {
    AllowOnce,
    AllowAlways,
    Deny,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_mode_serializes_to_snake_case() {
        assert_eq!(serde_json::to_string(&PermissionMode::Default).unwrap(), "\"default\"");
        assert_eq!(serde_json::to_string(&PermissionMode::AcceptEdits).unwrap(), "\"accept_edits\"");
        assert_eq!(serde_json::to_string(&PermissionMode::BypassPermissions).unwrap(), "\"bypass_permissions\"");
        assert_eq!(serde_json::to_string(&PermissionMode::Plan).unwrap(), "\"plan\"");
    }

    #[test]
    fn permission_decision_serializes_to_snake_case() {
        assert_eq!(serde_json::to_string(&PermissionDecision::AllowOnce).unwrap(), "\"allow_once\"");
        assert_eq!(serde_json::to_string(&PermissionDecision::AllowAlways).unwrap(), "\"allow_always\"");
        assert_eq!(serde_json::to_string(&PermissionDecision::Deny).unwrap(), "\"deny\"");
    }

    #[test]
    fn permission_mode_round_trips_through_json() {
        for mode in [PermissionMode::Default, PermissionMode::AcceptEdits, PermissionMode::BypassPermissions, PermissionMode::Plan] {
            let json = serde_json::to_string(&mode).unwrap();
            let back: PermissionMode = serde_json::from_str(&json).unwrap();
            assert_eq!(mode, back);
        }
    }
}
```

- [ ] **Step 3: Run the tests to verify they pass**

Run: `cargo test -p forge-domain`
Expected: 3 tests pass (this is a from-scratch crate, so there's no "fails first" step — the test and implementation are written together; verify green here).

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml crates/forge-domain
git commit -m "feat: add forge-domain with PermissionMode/PermissionDecision"
```

---

### Task 2: `forge-ports` — `SidecarPort` trait

**Files:**
- Create: `crates/forge-ports/Cargo.toml`
- Create: `crates/forge-ports/src/lib.rs`
- Create: `crates/forge-ports/src/sidecar_port.rs`
- Test: unit test inline in `crates/forge-ports/src/sidecar_port.rs`

**Interfaces:**
- Consumes: `forge_domain::{PermissionMode, PermissionDecision}` (Task 1)
- Produces: `forge_ports::{SidecarPort, StartSessionParams, SidecarEvent, SidecarError, PermissionRequestId}`, and (under `#[cfg(test)]`) `forge_ports::MockSidecarPort` — the trait every later port consumer (forge-application in a later plan, this plan's Task 6) programs against:
  ```rust
  pub type PermissionRequestId = String;

  pub struct StartSessionParams {
      pub cwd: String,
      pub resume_session_id: Option<String>,
      pub permission_mode: forge_domain::PermissionMode,
      pub model: Option<String>,
  }

  pub enum SidecarEvent {
      SessionStarted { session_id: String },
      AssistantDelta { text: String },
      ToolUse { tool_call_id: String, name: String, input: serde_json::Value },
      ToolResult { tool_call_id: String, output: String, is_error: bool },
      PermissionRequest { permission_request_id: String, tool_name: String, input: serde_json::Value },
      TurnComplete { input_tokens: u32, output_tokens: u32 },
      Error { message: String },
      SessionEnded,
  }

  #[async_trait]
  pub trait SidecarPort: Send + Sync {
      async fn start_session(&self, params: StartSessionParams) -> Result<(), SidecarError>;
      async fn send_message(&self, text: String) -> Result<(), SidecarError>;
      async fn respond_to_permission(&self, permission_request_id: PermissionRequestId, decision: forge_domain::PermissionDecision) -> Result<(), SidecarError>;
      async fn interrupt(&self) -> Result<(), SidecarError>;
      async fn shutdown(&self) -> Result<(), SidecarError>;
      async fn next_event(&self) -> Option<SidecarEvent>;
  }
  ```

- [ ] **Step 1: Create the crate and write the trait + test together**

`crates/forge-ports/Cargo.toml`:
```toml
[package]
name = "forge-ports"
version.workspace = true
edition.workspace = true

[dependencies]
forge-domain = { path = "../forge-domain" }
async-trait = "0.1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "1"

[dev-dependencies]
mockall = "0.13"
tokio = { version = "1", features = ["macros", "rt"] }
```

`crates/forge-ports/src/lib.rs`:
```rust
mod sidecar_port;

pub use sidecar_port::{
    PermissionRequestId, SidecarError, SidecarEvent, SidecarPort, StartSessionParams,
};

#[cfg(test)]
pub use sidecar_port::MockSidecarPort;
```

`crates/forge-ports/src/sidecar_port.rs`:
```rust
use async_trait::async_trait;
use forge_domain::{PermissionDecision, PermissionMode};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[cfg(test)]
use mockall::automock;

pub type PermissionRequestId = String;

#[derive(Debug, Clone, PartialEq)]
pub struct StartSessionParams {
    pub cwd: String,
    pub resume_session_id: Option<String>,
    pub permission_mode: PermissionMode,
    pub model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SidecarEvent {
    SessionStarted { session_id: String },
    AssistantDelta { text: String },
    ToolUse { tool_call_id: String, name: String, input: serde_json::Value },
    ToolResult { tool_call_id: String, output: String, is_error: bool },
    PermissionRequest { permission_request_id: String, tool_name: String, input: serde_json::Value },
    TurnComplete { input_tokens: u32, output_tokens: u32 },
    Error { message: String },
    SessionEnded,
}

#[derive(Debug, Error)]
pub enum SidecarError {
    #[error("failed to spawn sidecar process: {0}")]
    SpawnFailed(String),
    #[error("sidecar process exited unexpectedly")]
    ProcessExited,
    #[error("failed to write command to sidecar: {0}")]
    WriteFailed(String),
}

#[cfg_attr(test, automock)]
#[async_trait]
pub trait SidecarPort: Send + Sync {
    async fn start_session(&self, params: StartSessionParams) -> Result<(), SidecarError>;
    async fn send_message(&self, text: String) -> Result<(), SidecarError>;
    async fn respond_to_permission(
        &self,
        permission_request_id: PermissionRequestId,
        decision: PermissionDecision,
    ) -> Result<(), SidecarError>;
    async fn interrupt(&self) -> Result<(), SidecarError>;
    async fn shutdown(&self) -> Result<(), SidecarError>;
    async fn next_event(&self) -> Option<SidecarEvent>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_sidecar_port_can_record_a_start_session_call() {
        let mut mock = MockSidecarPort::new();
        mock.expect_start_session()
            .withf(|p: &StartSessionParams| p.cwd == "/tmp/project")
            .times(1)
            .returning(|_| Ok(()));

        let result = mock
            .start_session(StartSessionParams {
                cwd: "/tmp/project".to_string(),
                resume_session_id: None,
                permission_mode: PermissionMode::Default,
                model: None,
            })
            .await;

        assert!(result.is_ok());
    }
}
```

- [ ] **Step 2: Run the test**

Run: `cargo test -p forge-ports`
Expected: 1 test passes, confirming the `#[automock]` + `#[async_trait]` combination compiles and behaves — this is load-bearing for the use-case tests a later plan writes against `MockSidecarPort`.

- [ ] **Step 3: Commit**

```bash
git add crates/forge-ports
git commit -m "feat: add forge-ports with SidecarPort trait"
```

---

### Task 3: Sidecar project scaffold + Agent SDK type inspection

**Files:**
- Create: `sidecar/package.json`
- Create: `sidecar/package-lock.json` (generated by npm)
- Create: `sidecar/tsconfig.json`
- Create: `sidecar/.nvmrc`
- Create: `sidecar/src/index.ts` (empty placeholder, just enough to typecheck)
- Create: `sidecar/docs/sdk-types-notes.md`

**Interfaces:**
- Consumes: nothing new
- Produces: a pinned, installed `node_modules/@anthropic-ai/claude-agent-sdk` and a written record of its actual exported types, which Tasks 4–5 write code against instead of guessing

- [ ] **Step 1: Create the sidecar project files**

`sidecar/.nvmrc`:
```
22.22.1
```

`sidecar/package.json`:
```json
{
  "name": "forge-sidecar",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "engines": {
    "node": "22.22.1"
  },
  "scripts": {
    "build": "tsc -p tsconfig.json",
    "test": "vitest run",
    "typecheck": "tsc --noEmit"
  }
}
```

`sidecar/tsconfig.json`:
```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "strict": true,
    "outDir": "dist",
    "rootDir": "src",
    "declaration": false,
    "skipLibCheck": true
  },
  "include": ["src"]
}
```

`sidecar/src/index.ts`:
```typescript
export {};
```

- [ ] **Step 2: Install pinned dependencies**

Run, from `sidecar/`:
```bash
npm install --save-exact @anthropic-ai/claude-agent-sdk@0.3.283
npm install --save-exact --save-dev typescript vitest @types/node
```

Expected: `package.json` now lists exact (no `^`/`~`) versions for all four packages, and `package-lock.json` is created/updated.

- [ ] **Step 3: Verify the scaffold typechecks**

Run: `npm run typecheck`
Expected: exits 0 with no output.

- [ ] **Step 4: Inspect the installed SDK's real type definitions**

Run:
```bash
find node_modules/@anthropic-ai/claude-agent-sdk -name "*.d.ts"
```

Read the resulting file(s) (the main entry, typically `dist/index.d.ts` or similar) and confirm, precisely:
- The literal shape of `SDKUserMessage` (the type `prompt` expects when passed as an `AsyncIterable`)
- The full `SDKMessage` discriminated union and each variant's field names (especially `session_start`, `text_delta`/`assistant` deltas, `tool_use`, `tool_result`, `result`)
- The exact shape of `CanUseTool`'s request and return types
- Whether `Options.resume` accepts a specific session-id `string` (needed to resume an arbitrary past session, not just "most recent") or is `boolean`-only — if it's boolean-only, note this so Task 5's `start()` method is adjusted accordingly instead of assuming a session id can be passed directly

Write findings to `sidecar/docs/sdk-types-notes.md`, e.g.:
```markdown
# Agent SDK type notes (v0.3.283)

Recorded by reading node_modules/@anthropic-ai/claude-agent-sdk/dist/*.d.ts directly.

## SDKUserMessage
<paste the actual interface>

## SDKMessage union
<list each variant and its fields as found>

## CanUseTool
<paste the actual type>

## Options.resume
<boolean | string — state which, and adjust Task 5 accordingly>
```

- [ ] **Step 5: Commit**

```bash
git add sidecar/package.json sidecar/package-lock.json sidecar/tsconfig.json sidecar/.nvmrc sidecar/src/index.ts sidecar/docs/sdk-types-notes.md
git commit -m "chore: scaffold sidecar project with pinned Agent SDK dependency"
```

---

### Task 4: Sidecar wire protocol (`protocol.ts` + `io.ts`)

**Files:**
- Create: `sidecar/src/protocol.ts`
- Create: `sidecar/src/io.ts`
- Test: `sidecar/test/protocol.test.ts`

**Interfaces:**
- Consumes: nothing new
- Produces: `SidecarCommand`, `SidecarEvent` TS types and `parseCommand(line: string): SidecarCommand`, `serializeEvent(event: SidecarEvent): string`, `readCommands(input, onCommand, onError): void`, `writeEvent(output, event): void` — consumed by Task 5's `session.ts` and `index.ts`. These types must serialize/deserialize to exactly the same JSON shapes as `forge-ports::SidecarEvent` and the future `forge-adapter-sidecar::protocol::SidecarCommand` (Task 6) — both sides are hand-matched, not code-generated from a shared schema, so field names and the `snake_case` `type` tag values must match character-for-character.

- [ ] **Step 1: Write the failing tests**

`sidecar/test/protocol.test.ts`:
```typescript
import { describe, expect, it } from "vitest";
import { parseCommand, serializeEvent } from "../src/protocol.js";

describe("parseCommand", () => {
  it("parses a start_session command", () => {
    const line = JSON.stringify({
      type: "start_session",
      id: "1",
      cwd: "/tmp",
      resume_session_id: null,
      permission_mode: "default",
      model: null,
    });
    expect(parseCommand(line)).toEqual({
      type: "start_session",
      id: "1",
      cwd: "/tmp",
      resume_session_id: null,
      permission_mode: "default",
      model: null,
    });
  });

  it("throws on a line missing a type field", () => {
    expect(() => parseCommand("{}")).toThrow();
  });

  it("throws on invalid JSON", () => {
    expect(() => parseCommand("not json")).toThrow();
  });
});

describe("serializeEvent", () => {
  it("serializes a session_started event", () => {
    expect(JSON.parse(serializeEvent({ type: "session_started", session_id: "abc" }))).toEqual({
      type: "session_started",
      session_id: "abc",
    });
  });

  it("serializes a permission_request event", () => {
    const event = {
      type: "permission_request" as const,
      permission_request_id: "perm-1",
      tool_name: "Bash",
      input: { command: "ls" },
    };
    expect(JSON.parse(serializeEvent(event))).toEqual(event);
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run (from `sidecar/`): `npx vitest run test/protocol.test.ts`
Expected: FAIL — `../src/protocol.js` has no exports yet.

- [ ] **Step 3: Implement `protocol.ts` and `io.ts`**

`sidecar/src/protocol.ts`:
```typescript
export type WirePermissionMode = "default" | "accept_edits" | "bypass_permissions" | "plan";
export type WirePermissionDecision = "allow_once" | "allow_always" | "deny";

export type SidecarCommand =
  | { type: "start_session"; id: string; cwd: string; resume_session_id: string | null; permission_mode: WirePermissionMode; model: string | null }
  | { type: "send_message"; id: string; text: string }
  | { type: "permission_response"; id: string; permission_request_id: string; decision: WirePermissionDecision }
  | { type: "interrupt"; id: string }
  | { type: "shutdown"; id: string };

export type SidecarEvent =
  | { type: "session_started"; session_id: string }
  | { type: "assistant_delta"; text: string }
  | { type: "tool_use"; tool_call_id: string; name: string; input: unknown }
  | { type: "tool_result"; tool_call_id: string; output: string; is_error: boolean }
  | { type: "permission_request"; permission_request_id: string; tool_name: string; input: unknown }
  | { type: "turn_complete"; input_tokens: number; output_tokens: number }
  | { type: "error"; message: string }
  | { type: "session_ended" };

export function parseCommand(line: string): SidecarCommand {
  const parsed: unknown = JSON.parse(line);
  if (typeof parsed !== "object" || parsed === null || typeof (parsed as { type?: unknown }).type !== "string") {
    throw new Error(`invalid sidecar command: ${line}`);
  }
  return parsed as SidecarCommand;
}

export function serializeEvent(event: SidecarEvent): string {
  return JSON.stringify(event);
}
```

`sidecar/src/io.ts`:
```typescript
import * as readline from "node:readline";
import { parseCommand, serializeEvent, type SidecarCommand, type SidecarEvent } from "./protocol.js";

export function readCommands(
  input: NodeJS.ReadableStream,
  onCommand: (command: SidecarCommand) => void,
  onError: (error: Error) => void,
): void {
  const rl = readline.createInterface({ input, terminal: false });
  rl.on("line", (line) => {
    const trimmed = line.trim();
    if (trimmed.length === 0) return;
    try {
      onCommand(parseCommand(trimmed));
    } catch (err) {
      onError(err instanceof Error ? err : new Error(String(err)));
    }
  });
}

export function writeEvent(output: NodeJS.WritableStream, event: SidecarEvent): void {
  output.write(serializeEvent(event) + "\n");
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `npx vitest run test/protocol.test.ts`
Expected: all 5 tests pass.

- [ ] **Step 5: Commit**

```bash
git add sidecar/src/protocol.ts sidecar/src/io.ts sidecar/test/protocol.test.ts
git commit -m "feat: add sidecar wire protocol types and line-delimited JSON io"
```

---

### Task 5: Sidecar session driver (`session.ts`) + `index.ts` wiring

> **Revised after Task 3.** Task 3's firsthand inspection of `sdk.d.ts` (v0.3.283) found the original version of this task assumed `SDKMessage` variant names (`session_start`, `text_delta`, `tool_use`, `tool_result`) that do not exist, and a `canUseTool` signature that doesn't match the real `CanUseTool` type. See `sidecar/docs/sdk-types-notes.md` for the full firsthand citations (file:line quotes from `sdk.d.ts`) this revision is based on. The wire protocol to our own Rust side (Task 4's `SidecarCommand`/`SidecarEvent`) is unaffected — only the *translation* from real SDK messages to that protocol changes.

**Files:**
- Create: `sidecar/src/session.ts`
- Modify: `sidecar/src/index.ts`
- Test: `sidecar/test/session.test.ts`

**Interfaces:**
- Consumes: `SidecarCommand`, `SidecarEvent`, `readCommands`, `writeEvent` (Task 4)
- Produces: `SidecarSession` class with `constructor(queryFn: QueryFn, emit: (event: SidecarEvent) => void)` and `async handle(command: SidecarCommand): Promise<void>` — this is the piece Task 8's spike drives indirectly through the real `query` from `@anthropic-ai/claude-agent-sdk`

**Confirmed facts from `sidecar/docs/sdk-types-notes.md` (Task 3), read before writing this task's code:**
- `Options.resume` is `string` (an arbitrary session id to resume) — the code below's `resume: command.resume_session_id ?? undefined` is correct as written, no adjustment needed.
- The real `SDKMessage` union has ~38 variants. The ones this task needs: `SDKSystemMessage` (`type:'system', subtype:'init'`, carries `session_id`), `SDKPartialAssistantMessage` (`type:'stream_event'`, carries a nested Anthropic `event` whose `event.delta.type === 'text_delta'` gives streamed text), `SDKAssistantMessage` (`type:'assistant'`, whose `message.content` is an array of content blocks including `{type:'tool_use', id, name, input}`), `SDKUserMessage` (`type:'user'`, whose `message.content` may contain `{type:'tool_result', tool_use_id, content, is_error}` blocks — the CLI synthesizes these to feed tool results back to the model), and `SDKResultMessage` (`type:'result'`, discriminated by `subtype: 'success'|'error_...'`, carrying `usage`).
- `CanUseTool`'s real signature is `(toolName: string, input: Record<string, unknown>, options: { requestId: string; toolUseID: string; signal: AbortSignal; ...more }) => Promise<PermissionResult | null>`, where `PermissionResult` is `{behavior:'allow', ...} | {behavior:'deny', message: string, ...}`. The real API already hands you a `requestId` — use it directly as the wire protocol's `permission_request_id` instead of maintaining your own counter.
- A `SDKUserMessage` requires `parent_tool_use_id: string | null` (not optional) — pass `null` for a top-level user turn, or the real SDK may reject the message at runtime even though this codebase's loosely-typed `QueryFn` won't catch it at compile time.

**Two details `sidecar/docs/sdk-types-notes.md` didn't pin down exactly — verify these yourself before finalizing Step 3, by grepping the installed `sdk.d.ts`:**
1. `grep -n "NonNullableUsage\|interface.*Usage" node_modules/@anthropic-ai/claude-agent-sdk/sdk.d.ts` — confirm the exact field names on the `usage` object (this task assumes `input_tokens`/`output_tokens`, matching the public Anthropic Messages API convention; adjust the `forwardMessage` code below if the installed version differs).
2. `grep -n "abortController" node_modules/@anthropic-ai/claude-agent-sdk/sdk.d.ts` — confirm `Options.abortController` is the real field name and accepts a plain `AbortController` (this task assumes so; adjust if different).

If either differs from what's written below, adjust the code accordingly and note the correction in your report — the same way Task 3 flagged its findings, don't silently force reality to match this text.

- [ ] **Step 1: Write the failing tests**

`sidecar/test/session.test.ts`:
```typescript
import { describe, expect, it } from "vitest";
import { SidecarSession, type QueryFn } from "../src/session.js";
import type { SidecarCommand, SidecarEvent } from "../src/protocol.js";

const startCommand: SidecarCommand = {
  type: "start_session",
  id: "1",
  cwd: "/tmp",
  resume_session_id: null,
  permission_mode: "default",
  model: null,
};

describe("SidecarSession", () => {
  it("forwards a session init and a streamed text delta as sidecar events, then session_ended", async () => {
    const events: SidecarEvent[] = [];
    const queryFn: QueryFn = () =>
      (async function* () {
        yield { type: "system", subtype: "init", session_id: "s-1" };
        yield {
          type: "stream_event",
          event: { type: "content_block_delta", index: 0, delta: { type: "text_delta", text: "Hello" } },
        };
      })();

    const session = new SidecarSession(queryFn, (e) => events.push(e));
    await session.handle(startCommand);

    expect(events).toEqual([
      { type: "session_started", session_id: "s-1" },
      { type: "assistant_delta", text: "Hello" },
      { type: "session_ended" },
    ]);
  });

  it("forwards a tool_use content block from an assistant message", async () => {
    const events: SidecarEvent[] = [];
    const queryFn: QueryFn = () =>
      (async function* () {
        yield {
          type: "assistant",
          message: { content: [{ type: "tool_use", id: "t-1", name: "Bash", input: { command: "ls" } }] },
        };
      })();

    const session = new SidecarSession(queryFn, (e) => events.push(e));
    await session.handle(startCommand);

    expect(events).toContainEqual({ type: "tool_use", tool_call_id: "t-1", name: "Bash", input: { command: "ls" } });
  });

  it("forwards a tool_result content block from a synthesized user message", async () => {
    const events: SidecarEvent[] = [];
    const queryFn: QueryFn = () =>
      (async function* () {
        yield {
          type: "user",
          message: {
            content: [{ type: "tool_result", tool_use_id: "t-1", content: [{ type: "text", text: "done" }], is_error: false }],
          },
        };
      })();

    const session = new SidecarSession(queryFn, (e) => events.push(e));
    await session.handle(startCommand);

    expect(events).toContainEqual({ type: "tool_result", tool_call_id: "t-1", output: "done", is_error: false });
  });

  it("forwards a result message as turn_complete", async () => {
    const events: SidecarEvent[] = [];
    const queryFn: QueryFn = () =>
      (async function* () {
        yield { type: "result", subtype: "success", usage: { input_tokens: 12, output_tokens: 34 } };
      })();

    const session = new SidecarSession(queryFn, (e) => events.push(e));
    await session.handle(startCommand);

    expect(events).toContainEqual({ type: "turn_complete", input_tokens: 12, output_tokens: 34 });
  });

  it("emits permission_request using the SDK's own requestId and resolves after permission_response (allow)", async () => {
    const events: SidecarEvent[] = [];
    let capturedCanUseTool: any;
    const queryFn: QueryFn = (args) => {
      capturedCanUseTool = (args.options as any).canUseTool;
      return (async function* () {
        const result = await capturedCanUseTool("Bash", { command: "ls" }, { requestId: "req-1" });
        yield {
          type: "stream_event",
          event: {
            type: "content_block_delta",
            index: 0,
            delta: { type: "text_delta", text: result.behavior === "allow" ? "approved" : "denied" },
          },
        };
      })();
    };

    const session = new SidecarSession(queryFn, (e) => events.push(e));
    const startPromise = session.handle(startCommand);

    await new Promise((resolve) => setTimeout(resolve, 0));
    const permissionEvent = events.find((e) => e.type === "permission_request");
    expect(permissionEvent).toBeDefined();
    const requestId = (permissionEvent as { permission_request_id: string }).permission_request_id;
    expect(requestId).toBe("req-1");

    await session.handle({ type: "permission_response", id: "2", permission_request_id: requestId, decision: "allow_once" });
    await startPromise;

    expect(events.some((e) => e.type === "assistant_delta" && e.text === "approved")).toBe(true);
  });

  it("resolves with a PermissionResult deny (with a message) when the decision is deny", async () => {
    const events: SidecarEvent[] = [];
    let capturedCanUseTool: any;
    const queryFn: QueryFn = (args) => {
      capturedCanUseTool = (args.options as any).canUseTool;
      return (async function* () {
        const result = await capturedCanUseTool("Bash", { command: "rm -rf /" }, { requestId: "req-2" });
        yield {
          type: "stream_event",
          event: {
            type: "content_block_delta",
            index: 0,
            delta: { type: "text_delta", text: result.behavior === "deny" ? result.message : "should not happen" },
          },
        };
      })();
    };

    const session = new SidecarSession(queryFn, (e) => events.push(e));
    const startPromise = session.handle(startCommand);

    await new Promise((resolve) => setTimeout(resolve, 0));
    await session.handle({ type: "permission_response", id: "3", permission_request_id: "req-2", decision: "deny" });
    await startPromise;

    const delta = events.find((e) => e.type === "assistant_delta") as { text: string } | undefined;
    expect(delta?.text).toBeTruthy();
    expect(delta?.text).not.toBe("should not happen");
  });

  it("ignores a permission_response for an unknown permission_request_id", async () => {
    const events: SidecarEvent[] = [];
    const queryFn: QueryFn = () => (async function* () {})();
    const session = new SidecarSession(queryFn, (e) => events.push(e));

    await expect(
      session.handle({ type: "permission_response", id: "1", permission_request_id: "does-not-exist", decision: "deny" }),
    ).resolves.toBeUndefined();
  });

  it("delivers queued send_message commands to the prompt generator in order", async () => {
    const received: string[] = [];
    const queryFn: QueryFn = (args) =>
      (async function* () {
        for await (const msg of args.prompt as AsyncIterable<{ message: { content: string } }>) {
          received.push(msg.message.content);
          if (received.length === 2) return;
        }
      })();

    const session = new SidecarSession(queryFn, () => {});
    const startPromise = session.handle(startCommand);
    await session.handle({ type: "send_message", id: "2", text: "first" });
    await session.handle({ type: "send_message", id: "3", text: "second" });
    await startPromise;

    expect(received).toEqual(["first", "second"]);
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `npx vitest run test/session.test.ts`
Expected: FAIL — `../src/session.js` does not exist yet.

- [ ] **Step 3: Implement `session.ts`**

`sidecar/src/session.ts`:
```typescript
import type { SidecarCommand, SidecarEvent } from "./protocol.js";

export type QueryFn = (args: { prompt: AsyncIterable<unknown>; options: Record<string, unknown> }) => AsyncIterable<any>;

type PermissionResult =
  | { behavior: "allow"; updatedInput?: Record<string, unknown> }
  | { behavior: "deny"; message: string };

export class SidecarSession {
  private queuedMessages: unknown[] = [];
  private waitingResolvers: Array<(msg: unknown) => void> = [];
  private pendingPermissions = new Map<string, (result: PermissionResult) => void>();
  private abortController = new AbortController();
  private started = false;

  constructor(
    private readonly queryFn: QueryFn,
    private readonly emit: (event: SidecarEvent) => void,
  ) {}

  async handle(command: SidecarCommand): Promise<void> {
    switch (command.type) {
      case "start_session":
        await this.start(command);
        return;
      case "send_message":
        this.pushUserMessage(command.text);
        return;
      case "permission_response": {
        const resolver = this.pendingPermissions.get(command.permission_request_id);
        if (!resolver) return;
        resolver(
          command.decision === "deny"
            ? { behavior: "deny", message: "Denied by user" }
            : { behavior: "allow" },
        );
        this.pendingPermissions.delete(command.permission_request_id);
        return;
      }
      case "interrupt":
        this.abortController.abort();
        return;
      case "shutdown":
        this.abortController.abort();
        for (const resolver of this.pendingPermissions.values()) {
          resolver({ behavior: "deny", message: "Sidecar is shutting down" });
        }
        this.pendingPermissions.clear();
        return;
    }
  }

  private async *userMessages(): AsyncGenerator<unknown> {
    while (!this.abortController.signal.aborted) {
      yield await this.nextUserMessage();
    }
  }

  private nextUserMessage(): Promise<unknown> {
    const queued = this.queuedMessages.shift();
    if (queued !== undefined) return Promise.resolve(queued);
    return new Promise((resolve) => this.waitingResolvers.push(resolve));
  }

  private pushUserMessage(text: string): void {
    const message = { type: "user", message: { role: "user", content: text }, parent_tool_use_id: null };
    const resolver = this.waitingResolvers.shift();
    if (resolver) {
      resolver(message);
    } else {
      this.queuedMessages.push(message);
    }
  }

  private async start(command: Extract<SidecarCommand, { type: "start_session" }>): Promise<void> {
    if (this.started) return;
    this.started = true;

    const canUseTool = async (
      toolName: string,
      input: Record<string, unknown>,
      options: { requestId: string; [key: string]: unknown },
    ): Promise<PermissionResult> => {
      const permissionRequestId = options.requestId;
      this.emit({
        type: "permission_request",
        permission_request_id: permissionRequestId,
        tool_name: toolName,
        input,
      });
      return new Promise<PermissionResult>((resolve) => {
        this.pendingPermissions.set(permissionRequestId, resolve);
      });
    };

    const stream = this.queryFn({
      prompt: this.userMessages(),
      options: {
        cwd: command.cwd,
        resume: command.resume_session_id ?? undefined,
        permissionMode: command.permission_mode,
        model: command.model ?? undefined,
        abortController: this.abortController,
        canUseTool,
      },
    });

    try {
      for await (const message of stream) {
        this.forwardMessage(message);
      }
      this.emit({ type: "session_ended" });
    } catch (err) {
      this.emit({ type: "error", message: err instanceof Error ? err.message : String(err) });
    }
  }

  private forwardMessage(message: any): void {
    switch (message.type) {
      case "system":
        if (message.subtype === "init") {
          this.emit({ type: "session_started", session_id: message.session_id });
        }
        return;
      case "stream_event": {
        const event = message.event;
        if (event?.type === "content_block_delta" && event.delta?.type === "text_delta") {
          this.emit({ type: "assistant_delta", text: event.delta.text });
        }
        return;
      }
      case "assistant": {
        const blocks = message.message?.content;
        if (!Array.isArray(blocks)) return;
        for (const block of blocks) {
          if (block?.type === "tool_use") {
            this.emit({ type: "tool_use", tool_call_id: block.id, name: block.name, input: block.input });
          }
        }
        return;
      }
      case "user": {
        const blocks = message.message?.content;
        if (!Array.isArray(blocks)) return;
        for (const block of blocks) {
          if (block?.type === "tool_result") {
            this.emit({
              type: "tool_result",
              tool_call_id: block.tool_use_id,
              output: extractTextContent(block.content),
              is_error: Boolean(block.is_error),
            });
          }
        }
        return;
      }
      case "result":
        this.emit({
          type: "turn_complete",
          input_tokens: message.usage?.input_tokens ?? 0,
          output_tokens: message.usage?.output_tokens ?? 0,
        });
        return;
      default:
        return;
    }
  }
}

function extractTextContent(content: unknown): string {
  if (typeof content === "string") return content;
  if (!Array.isArray(content)) return "";
  return content
    .filter((block): block is { type: "text"; text: string } => typeof block === "object" && block !== null && (block as { type?: unknown }).type === "text")
    .map((block) => block.text)
    .join("");
}
```

`sidecar/src/index.ts`:
```typescript
import { query } from "@anthropic-ai/claude-agent-sdk";
import { readCommands, writeEvent } from "./io.js";
import { SidecarSession, type QueryFn } from "./session.js";
import type { SidecarEvent } from "./protocol.js";

function emit(event: SidecarEvent): void {
  writeEvent(process.stdout, event);
}

// `query`'s real signature is more specific than `QueryFn`; the cast documents
// the one boundary where this codebase's simplified type meets the SDK's own.
const session = new SidecarSession(query as unknown as QueryFn, emit);

readCommands(
  process.stdin,
  (command) => {
    void session.handle(command).catch((err) => {
      emit({ type: "error", message: err instanceof Error ? err.message : String(err) });
    });
  },
  (err) => {
    emit({ type: "error", message: `failed to parse command: ${err.message}` });
  },
);
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `npx vitest run test/session.test.ts`
Expected: all 8 tests pass.

- [ ] **Step 5: Build and typecheck the whole sidecar**

Run: `npm run build`
Expected: exits 0, produces `sidecar/dist/index.js`.

- [ ] **Step 6: Commit**

```bash
git add sidecar/src/session.ts sidecar/src/index.ts sidecar/test/session.test.ts
git commit -m "feat: add sidecar session driver wired to the Agent SDK"
```

---

### Task 6: `forge-adapter-sidecar` — command protocol + process management

**Files:**
- Create: `crates/forge-adapter-sidecar/Cargo.toml`
- Create: `crates/forge-adapter-sidecar/src/lib.rs`
- Create: `crates/forge-adapter-sidecar/src/protocol.rs`
- Create: `crates/forge-adapter-sidecar/src/process.rs`
- Test: unit tests inline in `protocol.rs`

**Interfaces:**
- Consumes: `forge_domain::{PermissionMode, PermissionDecision}` (Task 1), `forge_ports::{SidecarPort, StartSessionParams, SidecarEvent, SidecarError}` (Task 2)
- Produces: `forge_adapter_sidecar::SidecarProcess`, with `SidecarProcess::spawn(node_binary: &str, script_path: &str) -> Result<Self, SidecarError>`, implementing `SidecarPort` — consumed by Task 7's contract tests and Task 8's spike

- [ ] **Step 1: Create the crate and write the command protocol with its test**

`crates/forge-adapter-sidecar/Cargo.toml`:
```toml
[package]
name = "forge-adapter-sidecar"
version.workspace = true
edition.workspace = true

[dependencies]
forge-domain = { path = "../forge-domain" }
forge-ports = { path = "../forge-ports" }
async-trait = "0.1"
tokio = { version = "1", features = ["process", "io-util", "io-std", "rt-multi-thread", "macros", "sync"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"

[dev-dependencies]
tokio = { version = "1", features = ["full"] }
```

`crates/forge-adapter-sidecar/src/protocol.rs`:
```rust
use forge_domain::{PermissionDecision, PermissionMode};
use forge_ports::StartSessionParams;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SidecarCommand {
    StartSession {
        id: String,
        cwd: String,
        resume_session_id: Option<String>,
        permission_mode: WireMode,
        model: Option<String>,
    },
    SendMessage { id: String, text: String },
    PermissionResponse { id: String, permission_request_id: String, decision: WireDecision },
    Interrupt { id: String },
    Shutdown { id: String },
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WireMode {
    Default,
    AcceptEdits,
    BypassPermissions,
    Plan,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WireDecision {
    AllowOnce,
    AllowAlways,
    Deny,
}

impl From<PermissionMode> for WireMode {
    fn from(mode: PermissionMode) -> Self {
        match mode {
            PermissionMode::Default => WireMode::Default,
            PermissionMode::AcceptEdits => WireMode::AcceptEdits,
            PermissionMode::BypassPermissions => WireMode::BypassPermissions,
            PermissionMode::Plan => WireMode::Plan,
        }
    }
}

impl From<PermissionDecision> for WireDecision {
    fn from(decision: PermissionDecision) -> Self {
        match decision {
            PermissionDecision::AllowOnce => WireDecision::AllowOnce,
            PermissionDecision::AllowAlways => WireDecision::AllowAlways,
            PermissionDecision::Deny => WireDecision::Deny,
        }
    }
}

impl SidecarCommand {
    pub fn start_session(id: String, params: StartSessionParams) -> Self {
        SidecarCommand::StartSession {
            id,
            cwd: params.cwd,
            resume_session_id: params.resume_session_id,
            permission_mode: params.permission_mode.into(),
            model: params.model,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_session_command_serializes_with_the_expected_shape() {
        let command = SidecarCommand::start_session(
            "1".to_string(),
            StartSessionParams {
                cwd: "/tmp/project".to_string(),
                resume_session_id: None,
                permission_mode: PermissionMode::Default,
                model: None,
            },
        );

        let json = serde_json::to_value(&command).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "type": "start_session",
                "id": "1",
                "cwd": "/tmp/project",
                "resume_session_id": null,
                "permission_mode": "default",
                "model": null
            })
        );
    }

    #[test]
    fn permission_response_command_serializes_with_the_expected_shape() {
        let command = SidecarCommand::PermissionResponse {
            id: "2".to_string(),
            permission_request_id: "perm-1".to_string(),
            decision: PermissionDecision::AllowAlways.into(),
        };

        let json = serde_json::to_value(&command).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "type": "permission_response",
                "id": "2",
                "permission_request_id": "perm-1",
                "decision": "allow_always"
            })
        );
    }
}
```

- [ ] **Step 2: Run the protocol tests**

Run: `cargo test -p forge-adapter-sidecar protocol::`
Expected: 2 tests pass.

- [ ] **Step 3: Implement `process.rs`**

`crates/forge-adapter-sidecar/src/process.rs`:
```rust
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};

use async_trait::async_trait;
use forge_domain::PermissionDecision;
use forge_ports::{SidecarError, SidecarEvent, SidecarPort, StartSessionParams};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, Mutex};

use crate::protocol::SidecarCommand;

pub struct SidecarProcess {
    _child: Mutex<Child>,
    stdin_tx: mpsc::UnboundedSender<String>,
    events_rx: Mutex<mpsc::UnboundedReceiver<SidecarEvent>>,
    next_id: AtomicU64,
}

impl SidecarProcess {
    pub fn spawn(node_binary: &str, script_path: &str) -> Result<Self, SidecarError> {
        let mut child = Command::new(node_binary)
            .arg(script_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| SidecarError::SpawnFailed(e.to_string()))?;

        let mut stdin = child.stdin.take().expect("stdin was piped");
        let stdout = child.stdout.take().expect("stdout was piped");

        let (stdin_tx, mut stdin_rx) = mpsc::unbounded_channel::<String>();
        tokio::spawn(async move {
            while let Some(line) = stdin_rx.recv().await {
                if stdin.write_all(line.as_bytes()).await.is_err() {
                    break;
                }
                if stdin.write_all(b"\n").await.is_err() {
                    break;
                }
            }
        });

        let (events_tx, events_rx) = mpsc::unbounded_channel::<SidecarEvent>();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            loop {
                match lines.next_line().await {
                    Ok(Some(line)) => {
                        let trimmed = line.trim();
                        if trimmed.is_empty() {
                            continue;
                        }
                        match serde_json::from_str::<SidecarEvent>(trimmed) {
                            Ok(event) => {
                                if events_tx.send(event).is_err() {
                                    break;
                                }
                            }
                            Err(err) => {
                                if events_tx
                                    .send(SidecarEvent::Error {
                                        message: format!("failed to parse sidecar event: {err} (line: {trimmed})"),
                                    })
                                    .is_err()
                                {
                                    break;
                                }
                            }
                        }
                    }
                    Ok(None) => break,
                    Err(_) => break,
                }
            }
        });

        Ok(Self {
            _child: Mutex::new(child),
            stdin_tx,
            events_rx: Mutex::new(events_rx),
            next_id: AtomicU64::new(1),
        })
    }

    fn next_id(&self) -> String {
        self.next_id.fetch_add(1, Ordering::Relaxed).to_string()
    }

    fn send_command(&self, command: SidecarCommand) -> Result<(), SidecarError> {
        let json = serde_json::to_string(&command).map_err(|e| SidecarError::WriteFailed(e.to_string()))?;
        self.stdin_tx.send(json).map_err(|_| SidecarError::ProcessExited)
    }
}

#[async_trait]
impl SidecarPort for SidecarProcess {
    async fn start_session(&self, params: StartSessionParams) -> Result<(), SidecarError> {
        let id = self.next_id();
        self.send_command(SidecarCommand::start_session(id, params))
    }

    async fn send_message(&self, text: String) -> Result<(), SidecarError> {
        let id = self.next_id();
        self.send_command(SidecarCommand::SendMessage { id, text })
    }

    async fn respond_to_permission(
        &self,
        permission_request_id: String,
        decision: PermissionDecision,
    ) -> Result<(), SidecarError> {
        let id = self.next_id();
        self.send_command(SidecarCommand::PermissionResponse {
            id,
            permission_request_id,
            decision: decision.into(),
        })
    }

    async fn interrupt(&self) -> Result<(), SidecarError> {
        let id = self.next_id();
        self.send_command(SidecarCommand::Interrupt { id })
    }

    async fn shutdown(&self) -> Result<(), SidecarError> {
        let id = self.next_id();
        self.send_command(SidecarCommand::Shutdown { id })
    }

    async fn next_event(&self) -> Option<SidecarEvent> {
        let mut rx = self.events_rx.lock().await;
        rx.recv().await
    }
}
```

`crates/forge-adapter-sidecar/src/lib.rs`:
```rust
mod process;
mod protocol;

pub use process::SidecarProcess;
```

- [ ] **Step 4: Run all unit tests for this crate**

Run: `cargo test -p forge-adapter-sidecar`
Expected: all tests pass (protocol tests from Step 1; `process.rs` has no unit tests of its own — it's exercised by Task 7's integration tests).

- [ ] **Step 5: Commit**

```bash
git add crates/forge-adapter-sidecar/Cargo.toml crates/forge-adapter-sidecar/src
git commit -m "feat: implement SidecarPort via a Node child process over stdio JSON"
```

---

### Task 7: Contract tests against a fixture sidecar

**Files:**
- Create: `crates/forge-adapter-sidecar/tests/fixtures/fake_sidecar.js`
- Create: `crates/forge-adapter-sidecar/tests/contract_test.rs`

**Interfaces:**
- Consumes: `forge_adapter_sidecar::SidecarProcess` (Task 6), `forge_ports::{SidecarPort, SidecarEvent, StartSessionParams}` (Task 2)
- Produces: nothing new for later tasks — this is the plan's main correctness gate for the Review Focus items

This fixture is a plain Node script with **no** dependency on the Agent SDK — it speaks the same wire protocol so these tests need no API credentials and run in CI. It only tests the Rust adapter's framing/process-management; the real sidecar's behavior against the actual SDK is covered by Task 5's vitest tests plus Task 8's manual spike.

- [ ] **Step 1: Write the fixture**

`crates/forge-adapter-sidecar/tests/fixtures/fake_sidecar.js`:
```javascript
#!/usr/bin/env node
import * as readline from "node:readline";

const rl = readline.createInterface({ input: process.stdin, terminal: false });

function emit(event) {
  process.stdout.write(JSON.stringify(event) + "\n");
}

rl.on("line", (line) => {
  const trimmed = line.trim();
  if (trimmed.length === 0) return;
  const command = JSON.parse(trimmed);

  switch (command.type) {
    case "start_session":
      emit({ type: "session_started", session_id: "fake-session-1" });
      return;
    case "send_message":
      if (command.text === "TRIGGER_MALFORMED") {
        process.stdout.write("this line is not valid json\n");
        emit({ type: "assistant_delta", text: "recovered-after-malformed" });
        return;
      }
      if (command.text === "TRIGGER_LARGE") {
        emit({ type: "tool_result", tool_call_id: "t-1", output: "x".repeat(200000), is_error: false });
        return;
      }
      emit({ type: "assistant_delta", text: `echo:${command.text}` });
      return;
    case "permission_response":
      emit({ type: "assistant_delta", text: `permission:${command.decision}` });
      return;
    case "interrupt":
      emit({ type: "error", message: "interrupted" });
      return;
    case "shutdown":
      emit({ type: "session_ended" });
      process.exit(0);
  }
});
```

- [ ] **Step 2: Write the failing integration tests**

`crates/forge-adapter-sidecar/tests/contract_test.rs`:
```rust
use forge_adapter_sidecar::SidecarProcess;
use forge_domain::PermissionMode;
use forge_ports::{SidecarEvent, SidecarPort, StartSessionParams};

fn fixture_path() -> String {
    concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fake_sidecar.js").to_string()
}

fn node_binary() -> String {
    std::env::var("NODE_BINARY").unwrap_or_else(|_| "node".to_string())
}

fn default_params() -> StartSessionParams {
    StartSessionParams {
        cwd: "/tmp".to_string(),
        resume_session_id: None,
        permission_mode: PermissionMode::Default,
        model: None,
    }
}

#[tokio::test]
async fn start_session_and_send_message_round_trip() {
    let sidecar = SidecarProcess::spawn(&node_binary(), &fixture_path()).unwrap();

    sidecar.start_session(default_params()).await.unwrap();
    let started = sidecar.next_event().await.unwrap();
    assert!(matches!(started, SidecarEvent::SessionStarted { .. }));

    sidecar.send_message("hello".to_string()).await.unwrap();
    match sidecar.next_event().await.unwrap() {
        SidecarEvent::AssistantDelta { text } => assert_eq!(text, "echo:hello"),
        other => panic!("unexpected event: {other:?}"),
    }
}

#[tokio::test]
async fn recovers_from_a_malformed_line_without_dropping_the_connection() {
    let sidecar = SidecarProcess::spawn(&node_binary(), &fixture_path()).unwrap();
    sidecar.start_session(default_params()).await.unwrap();
    sidecar.next_event().await.unwrap();

    sidecar.send_message("TRIGGER_MALFORMED".to_string()).await.unwrap();

    assert!(matches!(sidecar.next_event().await.unwrap(), SidecarEvent::Error { .. }));
    match sidecar.next_event().await.unwrap() {
        SidecarEvent::AssistantDelta { text } => assert_eq!(text, "recovered-after-malformed"),
        other => panic!("unexpected event: {other:?}"),
    }
}

#[tokio::test]
async fn handles_a_very_large_event_payload() {
    let sidecar = SidecarProcess::spawn(&node_binary(), &fixture_path()).unwrap();
    sidecar.start_session(default_params()).await.unwrap();
    sidecar.next_event().await.unwrap();

    sidecar.send_message("TRIGGER_LARGE".to_string()).await.unwrap();
    match sidecar.next_event().await.unwrap() {
        SidecarEvent::ToolResult { output, .. } => assert_eq!(output.len(), 200_000),
        other => panic!("unexpected event: {other:?}"),
    }
}

#[tokio::test]
async fn interleaves_writes_and_reads_without_deadlock() {
    let sidecar = SidecarProcess::spawn(&node_binary(), &fixture_path()).unwrap();
    sidecar.start_session(default_params()).await.unwrap();
    sidecar.next_event().await.unwrap();

    for i in 0..5 {
        sidecar.send_message(format!("msg-{i}")).await.unwrap();
    }
    for i in 0..5 {
        match sidecar.next_event().await.unwrap() {
            SidecarEvent::AssistantDelta { text } => assert_eq!(text, format!("echo:msg-{i}")),
            other => panic!("unexpected event: {other:?}"),
        }
    }
}

#[tokio::test]
async fn next_event_returns_none_after_the_process_exits() {
    let sidecar = SidecarProcess::spawn(&node_binary(), &fixture_path()).unwrap();
    sidecar.start_session(default_params()).await.unwrap();
    sidecar.next_event().await.unwrap();

    sidecar.shutdown().await.unwrap();
    sidecar.next_event().await.unwrap(); // session_ended
    assert!(sidecar.next_event().await.is_none());
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p forge-adapter-sidecar --test contract_test`
Expected: FAIL to compile or run — this test file is new, so confirm it fails only because it hasn't been run before (it should actually pass immediately since `process.rs` from Task 6 already implements everything needed; if so, this step just confirms that by running it once before treating it as "done" — there is no missing implementation to add here).

- [ ] **Step 4: Run the tests to verify they pass**

Run: `NODE_BINARY=$(which node) cargo test -p forge-adapter-sidecar --test contract_test`
Expected: all 5 tests pass. Requires Node.js on `PATH` (already true on this machine, v22.22.1) — this is a development/CI environment requirement for this repo, separate from the bundled-runtime requirement end users get (spec §9).

- [ ] **Step 5: Commit**

```bash
git add crates/forge-adapter-sidecar/tests
git commit -m "test: add contract tests for SidecarProcess against a fixture sidecar"
```

---

### Task 8: De-risking spike — verify session transcript compatibility

**This task requires a real `ANTHROPIC_API_KEY` and the `claude` CLI, and involves a manual comparison — it is not something a credential-less subagent can complete. Flag it back to the human partner, or run it in a session that has API credentials.**

**Files:**
- Create: `crates/forge-adapter-sidecar/examples/spike_drive_sidecar.rs`
- Modify: `docs/superpowers/specs/2026-09-27-forge-design.md` (§10, append findings)

**Interfaces:**
- Consumes: `forge_adapter_sidecar::SidecarProcess`, `forge_ports::{SidecarPort, SidecarEvent, StartSessionParams}`
- Produces: a resolved (or explicitly still-open, with details) risk note in the spec, which later plans' session-sharing adapter (`forge-adapter-claude-fs`) depend on

- [ ] **Step 1: Write a small driver example**

`crates/forge-adapter-sidecar/examples/spike_drive_sidecar.rs`:
```rust
use forge_adapter_sidecar::SidecarProcess;
use forge_domain::PermissionMode;
use forge_ports::{SidecarEvent, SidecarPort, StartSessionParams};

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let sidecar_script = args
        .next()
        .expect("usage: spike_drive_sidecar <path-to-sidecar/dist/index.js> <cwd>");
    let cwd = args
        .next()
        .expect("usage: spike_drive_sidecar <path-to-sidecar/dist/index.js> <cwd>");

    let sidecar = SidecarProcess::spawn("node", &sidecar_script).expect("failed to spawn sidecar");

    sidecar
        .start_session(StartSessionParams {
            cwd,
            resume_session_id: None,
            permission_mode: PermissionMode::AcceptEdits,
            model: None,
        })
        .await
        .expect("failed to send start_session");

    sidecar
        .send_message("Say hello and create a file named hello.txt with the text 'hi' in it.".to_string())
        .await
        .expect("failed to send message");

    loop {
        match sidecar.next_event().await {
            Some(event) => {
                println!("{event:?}");
                if matches!(event, SidecarEvent::SessionEnded) {
                    break;
                }
            }
            None => break,
        }
    }
}
```

- [ ] **Step 2: Build the sidecar and the example**

```bash
cd sidecar && npm run build && cd ..
cargo build --example spike_drive_sidecar -p forge-adapter-sidecar
```

- [ ] **Step 3: Run the spike against the real Agent SDK**

```bash
mkdir -p /tmp/forge-spike
ANTHROPIC_API_KEY=<your key> ./target/debug/examples/spike_drive_sidecar \
  "$(pwd)/sidecar/dist/index.js" /tmp/forge-spike
```

Let it run to completion (prints events, ends on `SessionEnded`).

- [ ] **Step 4: Run the equivalent prompt through the real CLI in a separate scratch dir**

```bash
mkdir -p /tmp/forge-spike-cli && cd /tmp/forge-spike-cli
claude -p "Say hello and create a file named hello.txt with the text 'hi' in it."
cd -
```

- [ ] **Step 5: Compare the resulting transcripts**

```bash
ls ~/.claude/projects/-tmp-forge-spike/*.jsonl
ls ~/.claude/projects/-tmp-forge-spike-cli/*.jsonl
```

Open both files. Compare structurally (ignore timestamps, UUIDs, and session IDs themselves): same top-level `type` values, same message envelope shape, same representation of tool calls/results. Note any differences.

- [ ] **Step 6: Record the finding in the spec**

Edit `docs/superpowers/specs/2026-09-27-forge-design.md`, in §10, appending (do not delete the original risk description):

```markdown
### Resolved <today's date>

<Either:>
Confirmed compatible: the sidecar's transcript at `~/.claude/projects/-tmp-forge-spike/*.jsonl`
matches the CLI's transcript at `~/.claude/projects/-tmp-forge-spike-cli/*.jsonl` in structure
(same envelope shape, same tool_use/tool_result representation). Session sharing (§4.2) proceeds
as designed.

<Or, if they diverged:>
Diverged: <describe exactly what differs>. This affects `forge-adapter-claude-fs` in a later plan —
it will need <describe the adapter needed to bridge the formats> before session sharing works as
specified in §4.2.
```

- [ ] **Step 7: Commit**

```bash
git add crates/forge-adapter-sidecar/examples/spike_drive_sidecar.rs docs/superpowers/specs/2026-09-27-forge-design.md
git commit -m "docs: record de-risking spike findings on session transcript compatibility"
```

---

## Self-Review Notes

**Spec coverage for this plan's slice:** §3 (two-process architecture, stdio JSON, sidecar-per-session, pinned bundled deps) → Tasks 3, 6, 7. §5 (protocol) → Tasks 4, 6. §6 (forge-domain, forge-ports, forge-adapter-sidecar) → Tasks 1, 2, 6. §10 (de-risking spike) → Task 8. §4, §6 (FSD crates), §7 (feature UI), §9 (packaging) are explicitly deferred to later plans, not gaps in this one.

**Type consistency check:** `StartSessionParams`, `SidecarEvent`, `SidecarError`, `SidecarPort`, `PermissionRequestId` are defined once in Task 2 and reused verbatim by name in Tasks 6–8; `PermissionMode`/`PermissionDecision` defined once in Task 1 and reused throughout; TS-side `SidecarCommand`/`SidecarEvent` defined once in Task 4 and reused in Tasks 5 and 7's fixture. No renames across tasks.

**Review Focus:** all five items above are each pinned to a specific test in Task 5 or Task 7.
