# Forge — Cross-Platform Claude Agent Desktop Client

Date: 2026-09-27
Status: Approved for planning

## 1. Purpose

Forge is a cross-platform desktop application that provides a UI equivalent to
the Claude Code desktop app, built on **Dioxus** (Rust) instead of Claude
Code's own stack, and driven by the **Claude Agent SDK** instead of shelling
out to the `claude` CLI.

Forge is a companion to Claude Code, not a replacement fork of it:

- Its **connection/extension configuration** (API base URL, API key, which
  skills/plugins/MCP servers are enabled) is entirely its own, independent of
  Claude Code's configuration.
- Its **session history, project registry, and skill/plugin/MCP *definitions***
  are shared with Claude Code by reading/writing the same on-disk locations,
  so a conversation started in one tool is visible and resumable in the
  other.

## 2. Non-Goals (v1)

- Mobile or web targets. Desktop only (macOS/Windows/Linux) via
  `dioxus-desktop`.
- Installing new skills/plugins/MCP servers from a marketplace. v1 only
  toggles activation of what is already present in the shared directories
  (installed via Claude Code or placed there manually).
- Hooks configuration UI.
- Memory-file browser/editor.
- Team/multi-user features.
- OS-keychain credential storage — the API key is stored in Forge's own
  plaintext config file, matching the trust model Claude Code itself uses
  for `settings.json`.

## 3. Architecture Overview

Two-process model per open session, mirroring how the `claude` CLI itself
runs one process per session:

```
┌────────────────────────────┐   stdio, newline-delimited JSON   ┌───────────────────────────────┐
│  Forge (Dioxus, Rust)      │ ◄────────────────────────────────► │  Sidecar (bundled Node.js)    │
│  - UI                      │                                     │  - @anthropic-ai/claude-agent-sdk │
│  - spawns one sidecar      │                                     │  - one process per open session│
│    child process per       │                                     │  - streams assistant/tool      │
│    open chat session       │                                     │    events back over stdout     │
└────────────────────────────┘                                     └───────────────────────────────┘
```

Rationale:

- The Agent SDK has no Rust bindings (TypeScript and Python only), so the
  Dioxus UI cannot call it in-process.
- **Sidecar-per-session** (not one shared multiplexing sidecar) was chosen
  over a single long-lived multiplexing process because: (a) it matches the
  `claude` CLI's own process shape, which reduces the risk that session
  transcripts diverge in format from what the CLI produces (see §10); (b) a
  crash in one session's sidecar cannot affect other open sessions; (c) desktop
  usage rarely has more than a handful of sessions open concurrently, so the
  per-process overhead is acceptable.
- **stdio JSON-RPC**, not a local HTTP/WebSocket server: no port management,
  no local network attack surface, and process lifetime equals session
  lifetime (killing the child cleanly ends the session).
- The **Node.js runtime is bundled inside the app**, not required on the
  host system. Every bundled component (Node version, the Agent SDK version,
  and all other sidecar npm dependencies) is pinned to an **exact version**
  (no floating semver ranges), locked via `package-lock.json`, and verified
  (checksum) at build time.

## 4. Config & Data Sharing Model

### 4.1 Forge-private config

Stored at the OS-conventional app-config location (via the `directories`
crate), e.g. `~/Library/Application Support/Forge/config.json` on macOS —
never inside `~/.claude/`:

```json
{
  "base_url": "https://api.anthropic.com",
  "api_key": "sk-ant-...",
  "enabled_skills": ["skill-name-a", "skill-name-b"],
  "enabled_plugins": ["plugin-name@marketplace"],
  "enabled_mcp_servers": ["server-name-a"]
}
```

Plaintext file, same trust model as Claude Code's own `settings.json`. No
OS-keychain integration in v1.

### 4.2 Shared with Claude Code (read/write same on-disk locations, unmodified format)

- **Session transcripts**: `~/.claude/projects/<escaped-cwd>/*.jsonl` and the
  project registry inside `~/.claude.json`. Sessions started in either tool
  are visible and resumable in both.
- **Skill/plugin definitions**: read from `~/.claude/skills/` and
  `~/.claude/plugins/` (and project-local equivalents). Forge does not fork
  or copy this content — it reads the same files Claude Code reads.
- **MCP server definitions**: read from `~/.claude.json`'s `mcpServers` and
  project-local `.mcp.json`.
- **Memory files** and general project directory structure.

### 4.3 The activation split

For skills, plugins, and MCP servers, *definitions* are shared but
*activation state* is not: Forge keeps its own `enabled_skills` /
`enabled_plugins` / `enabled_mcp_servers` lists (§4.1) instead of reading
Claude Code's `settings.json` `enabledPlugins` field or equivalent. Turning a
skill on in Claude Code does not turn it on in Forge, and vice versa — only
the underlying files are shared.

## 5. Sidecar Protocol (draft — refined during implementation)

Newline-delimited JSON, one object per line, over the child process's
stdin/stdout.

**Forge → sidecar (commands):**

```jsonc
{"type": "start_session", "id": "req-1", "cwd": "/path/to/project", "resume_session_id": null, "permission_mode": "default", "model": null}
{"type": "send_message", "id": "req-2", "text": "user message"}
{"type": "permission_response", "id": "req-3", "permission_request_id": "perm-1", "decision": "allow_once"}
{"type": "interrupt", "id": "req-4"}
{"type": "shutdown", "id": "req-5"}
```

**Sidecar → Forge (events):**

```jsonc
{"type": "session_started", "session_id": "claude-session-uuid"}
{"type": "assistant_delta", "text": "..."}
{"type": "tool_use", "tool_call_id": "t-1", "name": "Edit", "input": {...}}
{"type": "tool_result", "tool_call_id": "t-1", "output": "...", "is_error": false}
{"type": "permission_request", "permission_request_id": "perm-1", "tool_name": "Bash", "input": {...}}
{"type": "turn_complete", "usage": {"input_tokens": 123, "output_tokens": 456}}
{"type": "error", "message": "..."}
{"type": "session_ended"}
```

`decision` values: `allow_once`, `allow_always`, `deny`. `permission_mode`
values mirror the Agent SDK's own: `default`, `acceptEdits`,
`bypassPermissions`, `plan`.

## 6. Repository Structure — Hexagonal Core + FSD Frontend, Enforced by the Crate Graph

Each architectural layer is its own Cargo workspace member, so the
dependency direction is enforced by the compiler, not just code review.

**Hexagonal core:**

- `forge-domain` — pure models (`Session`, `Message`, `ToolCall`,
  `PermissionRequest`, `SkillDefinition`, `PluginDefinition`,
  `McpServerDefinition`, `AppConfig`). No I/O dependencies.
- `forge-ports` — trait interfaces only, depends only on `forge-domain`:
  `SidecarPort`, `SessionRepositoryPort`, `ConfigRepositoryPort`,
  `SkillRepositoryPort`, `PluginRepositoryPort`, `McpRegistryPort`.
- `forge-application` — use-cases (`StartSessionUseCase`,
  `SendMessageUseCase`, `ApprovePermissionUseCase`, `ToggleSkillUseCase`,
  `TogglePluginUseCase`, `ToggleMcpServerUseCase`, `ResumeSessionUseCase`,
  `InterruptTurnUseCase`). Depends only on `forge-domain` + `forge-ports` —
  **never** on a concrete adapter crate.
- `forge-adapter-sidecar` — implements `SidecarPort` via `tokio::process` +
  the stdio JSON protocol (§5).
- `forge-adapter-claude-fs` — implements `SessionRepositoryPort`,
  `SkillRepositoryPort`, `PluginRepositoryPort`, `McpRegistryPort` by
  reading/writing the shared Claude Code files (§4.2).
- `forge-adapter-config` — implements `ConfigRepositoryPort` via the
  Forge-private `config.json` (§4.1).

**FSD frontend** (`shared → entities → features → widgets → pages → app`):

- `forge-shared` — UI kit (buttons, inputs, design tokens), generic
  utilities.
- `forge-entities` — business-entity UI components: message bubble, tool-call
  card, session list item, skill/plugin/MCP-server list item. Depends on
  `forge-domain` (for types) + `forge-shared`.
- `forge-features` — one user interaction each: send-message,
  approve-permission, toggle-skill, toggle-plugin, toggle-mcp-server,
  resume-session, interrupt-turn. Each feature is a component plus the call
  into the corresponding `forge-application` use-case. Depends on
  `forge-entities` + `forge-shared` + `forge-application`.
- `forge-widgets` — composite UI blocks: chat window (message list + input +
  tool-call panel), settings panel. Depends on `forge-features` +
  `forge-entities` + `forge-shared`.
- `forge-pages` — top-level screens: chat page, settings page, project
  picker page. Depends on `forge-widgets` + `forge-features` +
  `forge-entities` + `forge-shared`.
- `forge-app` — the binary and composition root: wires concrete adapters
  into use-cases, injects them into the UI context, sets up the Dioxus
  window/app. Depends on everything.

```
forge/
├── Cargo.toml                      # workspace root
├── crates/
│   ├── forge-domain/
│   ├── forge-ports/
│   ├── forge-application/
│   ├── forge-adapter-sidecar/
│   ├── forge-adapter-claude-fs/
│   ├── forge-adapter-config/
│   ├── forge-shared/
│   ├── forge-entities/
│   ├── forge-features/
│   ├── forge-widgets/
│   ├── forge-pages/
│   └── forge-app/
├── sidecar/                        # Node.js/TypeScript project
│   ├── package.json                # exact pinned versions
│   ├── package-lock.json
│   └── src/index.ts
└── build/                          # packaging: bundles pinned Node runtime
                                     # + compiled sidecar JS per target OS
```

## 7. Feature Scope

**Tier 1 (must-have, v1):**

- Project picker (open folder, recent projects list)
- Session list per project — new / resume / rename / delete — shared with
  Claude Code
- Chat view: streaming text, markdown + syntax-highlighted code blocks
- Tool-use visualization (Bash/Read/Edit/Write, etc.) shown inline,
  collapsible
- File diff viewer for Edit/Write tool calls
- Permission prompts: allow-once / allow-always / deny
- Interrupt/stop a running turn
- Settings UI, tabs: Connection (base_url/api_key), Model, Skills, Plugins,
  MCP servers (each toggles Forge's own enabled state against the shared
  definitions)

**Tier 2 (v1 if it doesn't block Tier 1, otherwise immediate fast-follow):**

- Subagent/Task tool calls shown as nested sub-threads
- Slash command autocomplete
- Per-session token/cost usage display
- Native OS notification when a background turn finishes
- Multiple sessions open at once (tabs or multiple windows)

## 8. Testing Strategy

| Layer | Approach |
|---|---|
| `forge-domain` | Pure unit tests, no mocks |
| `forge-application` | Use-case tests with `mockall`-generated mocks of each port; verify orchestration and error handling in isolation from real I/O |
| `forge-adapter-claude-fs`, `forge-adapter-config` | Integration tests against temp-dir fixtures mimicking `~/.claude/` layouts; never touch the real directory |
| `forge-adapter-sidecar` | Contract test: spawn the real sidecar binary with fixture stdin, assert on its stdout event stream |
| Node sidecar (`sidecar/`) | Its own unit tests (vitest) for protocol/message handling |
| `forge-entities`/`forge-features`/`forge-widgets`/`forge-pages` | Logic extracted into plain functions/hooks and tested without rendering; headless Dioxus rendering for a few snapshot tests where valuable |
| Full visual/E2E | Manual verification via `dx serve` — Dioxus lacks mature automated UI testing; this will be stated explicitly rather than claimed as covered |

## 9. Build & Packaging

- Dioxus app built via `dx bundle` per target OS.
- A custom build step downloads and checksum-verifies a pinned Node runtime
  build per target OS, compiles/bundles the sidecar TypeScript, and embeds
  both as app resources, referenced via `current_exe()`-relative paths at
  runtime — no dependency on a system-installed Node.
- All pinned versions (Dioxus, Node runtime, Agent SDK, every sidecar npm
  dependency) recorded in lockfiles and treated as intentional upgrades, not
  floating ranges.

## 10. Open Risk / De-Risking Spike

**Risk**: it is not yet verified that the Agent SDK's own session
persistence writes to the exact same transcript file format/location
(`~/.claude/projects/<escaped-cwd>/<session-id>.jsonl`) as the interactive
`claude` CLI. Session sharing (§4.2) depends on this.

**First implementation task, before any UI work**: a throwaway spike that
spawns the sidecar, drives one `query()` call against a scratch project
directory, and diffs the resulting transcript file against a real `claude`
CLI run of the same prompt in the same directory. If formats match, §4.2 is
close to free. If they diverge, this spec's session-sharing section will
need a revision (a thin adapter converting between formats) before Tier 1
work proceeds.

### Resolved 2026-09-27

**Confirmed compatible.** Ran the sidecar (via `forge-adapter-sidecar`'s
`SidecarProcess`, driving the real `@anthropic-ai/claude-agent-sdk@0.3.283`)
against a scratch directory with the prompt "Say hello and create a file
named hello.txt with the text 'hi' in it.", then ran the equivalent prompt
through the real `claude` CLI (`--permission-mode acceptEdits`) in a
separate scratch directory. Both wrote to
`~/.claude/projects/<escaped-cwd>/<session-id>.jsonl` in the expected
location. Comparing the two transcripts' `type` sequence and per-type field
sets (ignoring content/timestamps/IDs) found them structurally identical:
same event types in the same order (`queue-operation` ×2, `attachment`
×many, `user`, `atis-latch`, `last-prompt`, `ai-title`, `assistant` ×N,
`user`, `system`, `last-prompt`, `cost-state`), same field names per type.
Session sharing (§4.2) proceeds as designed — no adapter needed for
transcript format.

**Bonus finding from the same spike run**: the first attempt failed before
producing any transcript at all, with the sidecar's Node process erroring
out at CLI-argument-validation time. Root cause: the sidecar's translation
layer was passing Forge's own wire-protocol `permission_mode` string
(snake_case: `default`/`accept_edits`/`bypass_permissions`/`plan`) straight
through as the SDK's `options.permissionMode`, but the real SDK's
`PermissionMode` type (`sdk.d.ts:2481`) is a different, camelCase value set
(`'default' | 'acceptEdits' | 'bypassPermissions' | 'plan' | 'dontAsk' |
'auto'`). Fixed by adding an explicit translation function in the sidecar's
session driver. This is exactly the class of integration bug static review
and unit tests (which mock the SDK) cannot catch — only running the real
thing end-to-end surfaced it, which is the whole reason this spike exists
as a required step rather than an optional nicety.

## 11. Naming

App name: **Forge**. Chosen to avoid implying an official Anthropic
product (no "Claude" in the name).

## 12. Detailed Design: `forge-application` (Plan 2)

§6 named `forge-application`'s use-cases and the three repository ports it
needs but did not specify their shapes. This section fixes that, plus one
Plan-1 follow-up.

### 12.1 Closing a Plan 1 follow-up: mock visibility

Plan 1's final review found that `MockSidecarPort` (in `forge-ports`) was
gated behind a plain `#[cfg(test)]`, which is per-crate — `forge-application`
cannot see it in its own tests. Fixed here: gate every `Mock*` type behind
`#[cfg(any(test, feature = "mocks"))]` with a Cargo feature named `mocks`,
and add `forge-ports = { path = "../forge-ports", features = ["mocks"] }`
to `forge-application`'s `[dev-dependencies]`.

### 12.2 New `forge-domain` types

```rust
pub struct SessionId(pub String);

pub struct Session {
    pub id: SessionId,
    pub project_path: PathBuf,
    pub title: Option<String>,
    pub messages: Vec<Message>,
    pub permission_mode: PermissionMode,
}

pub struct SessionSummary {          // for a project's session *list*, no full history
    pub id: SessionId,
    pub title: Option<String>,
    pub updated_at: String,          // ISO 8601, as read from the transcript
}

pub enum MessageRole { User, Assistant }

pub struct Message {
    pub role: MessageRole,
    pub content: String,             // grows as AssistantDelta events arrive
    pub tool_calls: Vec<ToolCall>,
}

pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub input: serde_json::Value,
    pub result: Option<ToolResult>,
}
pub struct ToolResult { pub output: String, pub is_error: bool }

pub struct PermissionRequest {
    pub id: String,
    pub tool_name: String,
    pub input: serde_json::Value,
}

pub struct SkillDefinition { pub name: String, pub path: PathBuf, pub description: Option<String> }
pub struct PluginDefinition { pub name: String, pub marketplace: Option<String>, pub path: PathBuf }
pub struct McpServerDefinition { pub name: String, pub config: serde_json::Value }

pub struct AppConfig {
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub enabled_skills: Vec<String>,
    pub enabled_plugins: Vec<String>,
    pub enabled_mcp_servers: Vec<String>,
}
```

`McpServerDefinition.config` is deliberately an opaque `serde_json::Value`
rather than a fully-modeled stdio/sse/http union — nobody has verified the
real `~/.claude.json`/`.mcp.json` `mcpServers` format firsthand yet. Plan 3
(`forge-adapter-claude-fs`) must do its own firsthand file-format check
(the same discipline as Task 3's SDK inspection) before this needs to get
more specific.

### 12.3 New `forge-ports` traits

```rust
#[async_trait]
pub trait ConfigRepositoryPort: Send + Sync {
    async fn load(&self) -> Result<AppConfig, ConfigError>;
    async fn save(&self, config: &AppConfig) -> Result<(), ConfigError>;
}

#[async_trait]
pub trait SessionRepositoryPort: Send + Sync {
    async fn list_sessions(&self, project_path: &Path) -> Result<Vec<SessionSummary>, SessionRepositoryError>;
    async fn load_session(&self, project_path: &Path, session_id: &SessionId) -> Result<Session, SessionRepositoryError>;
}

#[async_trait]
pub trait SkillRepositoryPort: Send + Sync {
    async fn list_skills(&self, project_path: &Path) -> Result<Vec<SkillDefinition>, SkillRepositoryError>;
}

#[async_trait]
pub trait PluginRepositoryPort: Send + Sync {
    async fn list_plugins(&self) -> Result<Vec<PluginDefinition>, PluginRepositoryError>;
}

#[async_trait]
pub trait McpRegistryPort: Send + Sync {
    async fn list_servers(&self, project_path: &Path) -> Result<Vec<McpServerDefinition>, McpRegistryError>;
}
```

Each error type is its own small enum living beside its trait, matching
`SidecarError`'s pattern from Plan 1. `ResumeSessionUseCase` (§12.5) touches
two ports, so it gets its own combining error in `forge-application`:

```rust
pub enum ResumeSessionError {
    SessionRepository(SessionRepositoryError),
    Sidecar(SidecarError),
}
```

**Credential injection happens outside `forge-application` entirely.**
`SidecarPort::start_session`'s params (Plan 1) carry no `base_url`/`api_key`
— a spawned sidecar gets credentials via environment variables set at
process-spawn time. That is a concrete-adapter/composition-root concern
(wiring `forge-adapter-sidecar`'s `spawn()`), invisible to
`forge-application`. `ConfigRepositoryPort` is used only by the three
`Toggle*UseCase`s below.

### 12.4 Event application (not one of the named use-cases)

`SidecarEvent` lives in `forge-ports`, so translating it into `Session`
state can't live in `forge-domain` (which must depend on nothing). It's a
plain function in `forge-application`, pure and I/O-free — pumping events
isn't a user-initiated command the way the 8 named use-cases are:

```rust
pub enum SessionUpdate {
    SessionStarted { session_id: SessionId },
    MessageDelta { text: String },
    ToolCallStarted { tool_call: ToolCall },
    ToolCallCompleted { tool_call_id: String },
    PermissionRequested(PermissionRequest),
    TurnCompleted { input_tokens: u32, output_tokens: u32 },
    Errored { message: String },
    SessionEnded,
}

pub fn apply_sidecar_event(session: &mut Session, event: &SidecarEvent) -> SessionUpdate;
```

Behavior: an `AssistantDelta` appends to the session's last message if it's
an in-progress `Assistant` message, else pushes a new one. `ToolUse` pushes
a `ToolCall` onto the last `Assistant` message. `ToolResult` finds the
matching `ToolCall` by id across the message list and attaches the result;
an unmatched id is ignored, not an error. Fully pure — plain `#[test]`s, no
mocks, covering at least: first delta creates a message, second delta
appends, tool_use creates a call, tool_result attaches by id, unknown
tool_result id is ignored without panicking.

### 12.5 The 8 named use-cases

Each holds `Arc<dyn Port>` (not borrowed references — friendlier to a
long-lived Dioxus app context later, and what `mockall`'s `Arc<Mock...>`
pattern expects):

```rust
StartSessionUseCase   { sidecar: Arc<dyn SidecarPort> }
  .execute(cwd: String, permission_mode: PermissionMode, model: Option<String>) -> Result<(), SidecarError>

ResumeSessionUseCase  { sidecar: Arc<dyn SidecarPort>, sessions: Arc<dyn SessionRepositoryPort> }
  .execute(project_path: &Path, session_id: SessionId, permission_mode: PermissionMode) -> Result<Session, ResumeSessionError>
  // loads history for display via SessionRepositoryPort, then starts the
  // sidecar with resume_session_id set so the live agent reconnects

SendMessageUseCase       { sidecar } .execute(text: String) -> Result<(), SidecarError>
ApprovePermissionUseCase { sidecar } .execute(permission_request_id: String, decision: PermissionDecision) -> Result<(), SidecarError>
InterruptTurnUseCase     { sidecar } .execute() -> Result<(), SidecarError>

ToggleSkillUseCase      { config: Arc<dyn ConfigRepositoryPort> } .execute(name: &str) -> Result<AppConfig, ConfigError>
TogglePluginUseCase     { config } .execute(name: &str) -> Result<AppConfig, ConfigError>
ToggleMcpServerUseCase  { config } .execute(name: &str) -> Result<AppConfig, ConfigError>
  // each: load config, flip membership of `name` in its own enabled_*
  // list, save, return the new config
```

Kept as three separate small structs for the Toggle* trio rather than one
generic `Toggle<T>` — YAGNI, and they may diverge later (e.g. toggling a
plugin might also need to refresh its bundled skills).

### 12.6 Testing

Matches the Testing Strategy table in §8: `forge-domain` additions get
plain unit tests; `apply_sidecar_event` gets plain unit tests (no mocks,
per §12.4); the 8 use-cases get `mockall`-generated mocks of whichever
port(s) they hold, verifying orchestration order and error propagation in
isolation from real I/O — no concrete adapter exists yet, and none is
needed for these tests to be meaningful.
