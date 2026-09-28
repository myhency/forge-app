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
real `~/.claude.json`/`.mcp.json` `mcpServers` format firsthand yet. Plan 4
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

## 13. Detailed Design: `forge-adapter-config` (Plan 3)

The first concrete adapter. Implements `ConfigRepositoryPort` (§12.3) by
reading/writing Forge's own private `config.json` (§4.1) via real file I/O.
Plan 4 (§14) covers `forge-adapter-claude-fs` — the much larger body of
work parsing Claude Code's shared, externally-owned file formats (session
transcripts, skill/plugin/MCP-server definitions).

### 13.1 Firsthand format verification done before this plan

Before designing this adapter, real files on a machine with an active
Claude Code installation were inspected directly (not guessed), the same
discipline Task 3 applied to the Agent SDK's types in Plan 1:

- **Skills**: `~/.claude/skills/<name>/SKILL.md` — YAML frontmatter with
  `name`/`description` keys, then a markdown body.
- **Plugins**: `~/.claude/plugins/installed_plugins.json` — the
  authoritative registry, shape `{version, plugins: {"<name>@<marketplace>":
  [{scope, installPath, version, installedAt, lastUpdated, gitCommitSha}]}}`.
- **MCP servers**: `.mcp.json` files exist in **two different shapes** in
  the wild — a bare `{"<name>": {"command": ..., "args": [...]}}` with no
  wrapper key, and a `{"mcpServers": {"<name>": {"type": "http", "url":
  ..., "headers": {...}}}}` wrapped form. This confirms §12.2's decision to
  keep `McpServerDefinition.config` an opaque `serde_json::Value` was
  correct — a rigid typed shape would have been wrong for at least one of
  these two real examples.
- **Session transcripts**: confirmed the content-block vocabulary
  (`text`, `thinking`, `tool_use{id,name,input}`,
  `tool_result{tool_use_id,content}`) matches the Agent SDK's own message
  shapes (Plan 1, Task 3's findings) — transcripts are serialized
  `SDKMessage` records. Found dedicated `custom-title`/`ai-title` line
  types carrying a session's display title, separate from the
  `user`/`assistant` message lines.

This plan (config only) doesn't need the session/skill/plugin/MCP findings
directly, but they retire risk for Plan 4 and are recorded here so that
plan doesn't have to re-derive them.

### 13.2 Design

```rust
pub fn default_config_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("", "", "Forge")
        .map(|dirs| dirs.config_dir().join("config.json"))
}

pub struct ConfigFileAdapter {
    path: PathBuf,
}

impl ConfigFileAdapter {
    pub fn new(path: PathBuf) -> Self { ... }
}

#[async_trait]
impl ConfigRepositoryPort for ConfigFileAdapter {
    async fn load(&self) -> Result<AppConfig, ConfigError> { ... }
    async fn save(&self, config: &AppConfig) -> Result<(), ConfigError> { ... }
}
```

- `ConfigFileAdapter` takes an explicit `path`, never resolving the OS
  config directory itself — `default_config_path()` is a separate free
  function the future composition root (`forge-app`, a later plan) calls
  to get the real path before constructing the adapter. This keeps the
  adapter trivially testable against any temp path, never the real
  `~/Library/Application Support/Forge/config.json` (or platform
  equivalent) location.
- `load()` on a missing file returns `Ok(AppConfig::default())` (first-run
  behavior) rather than an error — a config file that doesn't exist yet is
  not a failure.
- `save()` creates parent directories as needed (`create_dir_all`) before
  writing, since the OS config directory may not exist on first run.
- Uses `tokio::fs` (not `std::fs`) for the actual reads/writes, consistent
  with the rest of the codebase's async I/O, and to avoid blocking the
  async runtime thread even though config files are small.
- No atomic-write-via-temp-file-then-rename for `save()` in this plan —
  YAGNI for v1; flagged here as a reasonable future hardening, not a
  requirement.

### 13.3 Testing

Integration tests against a real temp directory (never the real OS config
location), per §8's testing table: missing-file load returns default;
save-then-load round-trips; save creates missing parent directories;
malformed JSON on disk produces `ConfigError::ParseFailed`.

## 14. Detailed Design: `forge-adapter-claude-fs` (Plan 4)

Implements the four remaining ports from §12.3 —
`SessionRepositoryPort`, `SkillRepositoryPort`, `PluginRepositoryPort`,
`McpRegistryPort` — by reading Claude Code's shared, externally-owned
on-disk state (§4.2). All four are read-only in v1; nothing in the
current use-case set (§12.5) writes through these ports. The error enums
for all four already exist in `forge-ports` from Plan 2
(`crates/forge-ports/src/session_port.rs`,
`crates/forge-ports/src/extension_ports.rs`) — this plan implements
against them, it does not redesign them.

### 14.1 Firsthand format verification done before this plan

§13.1 recorded a first pass at these formats during Plan 3. Before writing
this plan, that pass was redone in more depth directly against a live
`~/.claude/` installation with 29 registered projects, 18 real session
directories, and 15 installed skills (30 real `SKILL.md` files counting
nested ones) — the findings below supersede and extend §13.1 where they
overlap:

- **Skills**: `SKILL.md` files are **not** one level deep under
  `~/.claude/skills/<name>/`. Real examples nest up to 3 levels
  (`skills/document-skills/xlsx/SKILL.md`,
  `skills/synced/<uuid>/deep-research/SKILL.md` — plugin-synced skill
  copies). Listing must recursively walk for any file literally named
  `SKILL.md`, not assume a fixed depth. Frontmatter across all 30 real
  files is flat, single-line `key: value` pairs only — `name` and
  `description` always present; other keys (`license` was the only one
  observed) vary and are ignored.
- **Plugins**: `installed_plugins.json`'s shape from §13.1
  (`{version, plugins: {"<name>@<marketplace>": [{scope, installPath,
  version, installedAt, lastUpdated, gitCommitSha}]}}`) reconfirmed
  exactly against 9 real installed plugins. Every array on this machine
  has exactly one entry, but nothing in the format forbids more (e.g. one
  entry per install scope) — the design does not assume length 1.
- **MCP servers**: no server is currently configured anywhere on this
  machine (global `~/.claude.json`'s `mcpServers` is `{}`, no `.mcp.json`
  file exists in any project directory found, and every one of the 29
  registered projects' own `mcpServers` entry inside `~/.claude.json` is
  also empty) — the two-shape finding from §13.1 (bare
  `{"name": {...}}` vs. wrapped `{"mcpServers": {"name": {...}}}`) is
  carried forward from that earlier verification rather than re-derived,
  since no live example exists right now to re-check it against. Because
  the per-project `~/.claude.json` entry has never been observed
  populated, this plan does not read it — only the global `mcpServers`
  key and a project-local `.mcp.json` file are merged (project-local wins
  on name collision).
- **Session directory escaping**: confirmed by reverse-mapping all 18
  real session directories under `~/.claude/projects/` back to their
  registered project paths in `~/.claude.json`'s `projects` map — the
  rule is "replace every `/` and every `.` in the absolute project path
  with `-`" (e.g. `/Users/james/.claude` → `-Users-james--claude`, note
  the doubled dash from the leading `.` of `.claude`). A session's
  transcript directory is independent of whether that path is registered
  in `~/.claude.json`'s `projects` map at all — several real directories
  (worktrees, `/private/tmp` scratch dirs) have transcripts with no
  corresponding `projects` entry. `list_sessions`/`load_session` therefore
  never need to read `~/.claude.json`'s `projects` map — the escaped
  directory is computed directly from the `project_path` parameter.
- **Session transcript line types**: a real transcript contains far more
  top-level `type` values than §13.1's original content-block-only
  finding: `user`, `assistant`, `system`, `ai-title`, `mode`,
  `permission-mode`, `atis-latch`, `attachment`, `last-prompt`,
  `cost-state` were all observed in a single real file. Of these:
  - `attachment` is hook-execution output (`command`, `stdout`, `stderr`,
    `exitCode`, a nested `type` like `hook_success`) — nothing to do with
    user file attachments despite the name.
  - `user` messages have **two** content shapes: a plain string (real
    user text) or a list of content blocks. Across all 66 real transcript
    files on this machine (1,223 list-content `user` lines), 1,212 lists
    contained only `tool_result` blocks (the harness's injected
    tool-execution feedback) and 11 contained only `text` blocks (real
    text, e.g. injected skill-loading content) — so a list is **not**
    always tool feedback, and text blocks must not be dropped.
  - A `tool_result` block's `content` is a **string** in 1,101 of 1,211
    real blocks and a **list of blocks** in the other 110 (the observed
    list contained `tool_reference` blocks, not `text`). Its `is_error`
    key is present in only 519 of 1,211 blocks — absent means not an
    error.
  - `assistant` message content blocks seen: `thinking`, `text`,
    `tool_use` — matching the Agent SDK shapes Plan 1 Task 3 already
    confirmed, since transcripts are serialized `SDKMessage` records.
    **Every one of the 2,574 real `assistant` lines carries exactly one
    content block**, and 1,242 of them share their `message.id` with the
    preceding `assistant` line: one API response is split across several
    lines, one block each. Loading one `Message` per line would produce a
    garbled history full of empty messages (e.g. a `thinking`-only line).
    The live path (§12.4's `apply_sidecar_event`) already merges
    consecutive assistant content into the last `Assistant` message until
    a user message intervenes; the loader follows the same rule so live
    and loaded sessions have the same shape.
  - A `permission-mode` line's value is in Claude Code's **camelCase**
    vocabulary, not `forge-domain`'s snake_case serde form. Observed
    across all 66 files: `"auto"` (584 lines), `"default"` (147),
    `"acceptEdits"` (2). `"auto"` (Claude Code's Auto Mode) has **no**
    corresponding variant in `forge-domain`'s `PermissionMode` enum
    (`Default`/`AcceptEdits`/`BypassPermissions`/`Plan`, shipped in Plan 2
    and used by the sidecar wire protocol in §5), and routing `"acceptEdits"`
    through the enum's snake_case serde would wrongly fail to parse. So the
    loader matches on the on-disk string explicitly (`"acceptEdits"`,
    `"bypassPermissions"`, `"plan"` — the SDK vocabulary Plan 1
    confirmed) and maps everything else, including `"default"` and
    `"auto"`, to `PermissionMode::Default`. This plan does not extend the
    enum.
  - No line with `isSidechain: true` was found in any of the 7 real
    session files inspected (each Task-tool/subagent dispatch in this
    project's own workflow produced its own separate top-level session
    file rather than an inline sidechain line) — the field is still
    checked defensively (skip if `true`) since its absence from every
    real example is not proof it is never populated by other flows.
  - Only `ai-title` was observed carrying a session title;
    `custom-title` (mentioned in §13.1) was not found in any of the 7
    files inspected, so its exact field name is carried forward from that
    earlier finding rather than re-verified.

### 14.2 Design

```rust
pub fn default_claude_home() -> Option<PathBuf> {
    directories::BaseDirs::new().map(|dirs| dirs.home_dir().join(".claude"))
}

// NOT inside ~/.claude/: the global MCP/project registry file is a sibling
// of that directory, at ~/.claude.json.
pub fn default_claude_json_path() -> Option<PathBuf> {
    directories::BaseDirs::new().map(|dirs| dirs.home_dir().join(".claude.json"))
}

pub struct ClaudeSessionAdapter { claude_home: PathBuf }
pub struct ClaudeSkillAdapter { claude_home: PathBuf }
pub struct ClaudePluginAdapter { claude_home: PathBuf }
pub struct ClaudeMcpAdapter { claude_json_path: PathBuf }
```

One struct per port (no shared state between them), each constructed with
an explicit root path — never resolving `~/.claude` or `~/.claude.json`
itself, mirroring `ConfigFileAdapter`'s `path`/`default_config_path()`
split in §13.2. The first three take the `~/.claude/` directory; the MCP
adapter takes the path of the `~/.claude.json` file (a different location,
see above). This keeps every adapter testable against a temp directory
that never touches the real files.

**`ClaudeSessionAdapter`** (`sessions.rs`):
- Escaping helper: replace `/` and `.` with `-` (§14.1) to compute
  `claude_home/projects/<escaped>/`. Verified on Unix-style paths only;
  Windows path escaping is unverified and out of scope for this plan.
- `list_sessions`: if the escaped directory doesn't exist, `Ok(vec![])`.
  Otherwise one `SessionSummary` per `*.jsonl` file (other entries — real
  project directories also hold e.g. a `memory/` subdirectory — are
  ignored): `id` from the filename stem, `updated_at` from the file's
  mtime (`tokio::fs::metadata` — one stat call, not a full parse)
  formatted as RFC 3339 (a strict profile of ISO 8601), `title` from the
  last `ai-title`/`custom-title` line seen when scanning the file
  (`None` if absent; a cheap substring check skips full JSON parsing of
  every non-title line, since real transcripts reach 13 MB). Results are
  sorted newest-first by mtime (ties broken by id, for deterministic
  output). A failure reading the directory itself →
  `SessionRepositoryError::ListFailed`; a failure on one individual file
  (vanished between listing and stat, unreadable, invalid UTF-8) degrades
  that file (skipped if its mtime can't be read; `title: None` if only
  the scan failed) instead of failing the whole listing.
- `load_session`: a session id that could escape the project directory
  (empty, containing `/` or `\`, or `..`) → `SessionRepositoryError::
  NotFound`, without touching the filesystem; missing file → `NotFound`.
  Parse line by line, skipping blank lines and any line with
  `isSidechain: true`: `user` (string) → new `Message{role: User, ...}`;
  `user` (list) → each `tool_result` block fills the `result` of the
  `ToolCall` with the matching `tool_use_id` (searching backward, like
  `apply_sidecar_event`), its `content` flattened to a string (a string
  as-is; a block list → its `text` blocks joined with `\n`, other block
  types ignored), `is_error` defaulting to `false`, and any `text` blocks
  in the list become a new `User` message; `assistant` → its `text` blocks
  and `tool_use` blocks are merged into the last message if that is an
  `Assistant` message (text appended, separated by `\n\n` from any
  existing text), else a new `Assistant` message — a line yielding neither
  text nor tool calls (e.g. `thinking` only) adds nothing; track the last
  `permission-mode` value (explicit camelCase match, §14.1, unrecognized
  → `Default`); track the last title line. Any other `type` is ignored.
  Because the transcript may be mid-append by a running Claude Code
  (sessions are shared, §4.2), a malformed **final** line is ignored,
  while a malformed line followed by any further non-blank line is real
  corruption → `SessionRepositoryError::LoadFailed`, as is any I/O error.

**`ClaudeSkillAdapter`** (`skills.rs`):
- `list_skills(project_path)`: recursively walk `claude_home/skills/` and
  `project_path/.claude/skills/` (either missing → contributes zero
  entries) for files named `SKILL.md`. Symlinked directories are followed
  (users commonly symlink a skill from a checkout) with a visited set of
  canonical paths so a symlink loop terminates. For each file, parse
  frontmatter with a hand-rolled parser: tolerate a leading UTF-8 BOM and
  CRLF line endings, require a first line of `---`, read lines until the
  next `---`, consider only top-level keys (lines with no leading
  whitespace), split each on the first `:`, trim key/value, and strip one
  pair of matching surrounding quotes from the value. A file with no
  frontmatter or no `name` is skipped, never failing the whole listing;
  a missing `description` is **not** a skip — `SkillDefinition.description`
  is `Option<String>` (§12.2), so it becomes `None`. YAML block scalars
  (`description: >-` plus indented lines) are not supported — a known
  limitation of the hand-rolled parser accepted in brainstorming.
  `SkillDefinition.path` is the file's parent directory. `ListFailed` is
  reserved for a failure to read a root directory that exists (e.g.
  permission denied).

**`ClaudePluginAdapter`** (`plugins.rs`):
- `list_plugins()`: parse `claude_home/plugins/installed_plugins.json`.
  Missing file → `Ok(vec![])`. For each `"<name>@<marketplace>"` key,
  `split_once('@')` (no `@` found → whole key as `name`, `marketplace:
  None`); emit one `PluginDefinition` per array entry, using its
  `installPath` as `path`. Malformed top-level JSON →
  `PluginRepositoryError::ListFailed`.

**`ClaudeMcpAdapter`** (`mcp.rs`):
- `list_servers(project_path)`: deserialize only the `mcpServers` field out
  of the file at `claude_json_path` (`~/.claude.json`) via a narrow struct
  (`#[serde(default, rename = "mcpServers")] HashMap<String,
  serde_json::Value>`) — every other key in that ~100-key file is never
  touched or exposed. Merge with `project_path/.mcp.json` if present,
  normalizing both the bare and `{"mcpServers": {...}}`-wrapped shapes to
  the same flat map; project-local entries win on name collision. Either
  source missing contributes zero entries. Malformed top-level JSON in
  either file → `McpRegistryError::ListFailed`.

### 14.3 Testing

Every test builds a real temp directory tree — never the real
`~/.claude/` — mirroring the exact shapes found in §14.1:

- Nested `SKILL.md` fixtures (including one 3 levels deep, one with no
  `name` proving it's skipped rather than failing the listing, one with no
  `description` proving it's kept with `None`, plus CRLF/BOM/quoted-value
  variants, a symlinked skill directory and a symlink loop).
- A synthetic `installed_plugins.json` with a multi-entry install array
  for one plugin.
- Both `.mcp.json` shapes (bare and wrapped), plus a global/project-local
  name collision proving project-local precedence.
- Synthetic `.jsonl` transcripts covering: a plain user/assistant turn, a
  `tool_use`→`tool_result` round trip (string-content and block-list
  `tool_result`s, with and without `is_error`), an assistant response
  split one-block-per-line (a `thinking`-only line must add nothing), a
  `user` list carrying a `text` block, an `ai-title` line, `permission-mode`
  lines (`"acceptEdits"` mapped, `"auto"` falling back to `Default`), and
  the ignorable line types (`mode`, `attachment`, `cost-state`)
  interleaved to prove they don't break parsing. A missing-directory
  `list_sessions` call, a missing-file `load_session` call (`NotFound`),
  a path-traversal session id (`NotFound`), a malformed final line
  (ignored) versus a malformed mid-file line (`LoadFailed`), and
  newest-first ordering with one corrupt transcript that must not blank
  the listing are all covered.
