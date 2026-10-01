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
  are visible and resumable in both. *(Correction, §15.2: the UI's project
  list is derived from the transcript directories, not from the
  `~/.claude.json` registry, which omits most projects that have sessions.)*
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
  *(§15 adds `ProjectRepositoryPort` and `SidecarFactoryPort`.)*
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
  observed) vary and are ignored. (The final review later found 3 further
  real `SKILL.md` files, in plugin caches outside the listed roots, whose
  `description` is a YAML block scalar, `description: >` — so block
  scalars do occur in the wild; see §14.2.)
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
  since no live example exists right now to re-check it against. Brainstorming
  first chose to read only the global `mcpServers` key and a
  project-local `.mcp.json` file, because the per-project
  `~/.claude.json` entry had never been observed populated on this
  machine. **Reversed after the final review, by the human:** the
  installed Claude Code 2.1.283 binary shows that `claude mcp add`
  defaults to `--scope local` ("Local config (private to you in this
  project)"), stored per project inside `~/.claude.json`
  (`projects[<key>].mcpServers`) — so the most common way to add a server
  would never have appeared. The per-project entry is now read too (Task
  6). Its key is the project's NFC-normalised, symlink-resolved working
  directory (the binary's `MTe()`: `O$(xn(We(cwd)))` with `xn` =
  `normalize("NFC")` — note this is NFC, whereas the transcript directory
  name uses the on-disk form); for a git worktree Claude Code keys by the
  main repository root, which this adapter does not resolve (callers pass
  the main repository path). Scope precedence on a name collision is
  local > project `.mcp.json` > global, per Claude Code's documented
  behaviour (not verified in the binary).
- **Session directory escaping**: reverse-mapping all 18 real session
  directories under `~/.claude/projects/` back to their registered project
  paths in `~/.claude.json`'s `projects` map first suggested "replace
  every `/` and every `.` with `-`" (e.g. `/Users/james/.claude` →
  `-Users-james--claude`) — but that was an under-fit: none of the 29
  registered paths on this machine contains any character outside
  `[A-Za-z0-9/.-]`, so no local data could distinguish it from the real
  rule. The real rule was then read out of the installed Claude Code
  2.1.283 binary (functions `k`, `Jx`, `oJ`; the directory is
  `~/.claude/projects/<Jx(cwd)>`):
  every UTF-16 code unit that is not an ASCII letter or digit becomes `-`
  (so `_`, spaces, `~` and each Hangul character each become one `-`, and
  a character outside the BMP becomes two); if the result is longer than
  200 characters it is cut to its first 200 characters followed by `-` and
  the base-36 form of the absolute value of a Java-style 32-bit hash
  (`h = h*31 + unit`, wrapping, over the UTF-16 units of the **original**
  path). Expected values used by the tests were generated by running the
  binary's own JavaScript in Node. Windows paths (`\`, `:`) fall under the
  same rule but were not exercised. The function's input is the
  process's *resolved* working directory (`getcwd`: symlinks resolved,
  on-disk Unicode form): `~/.claude/projects` holds `-private-tmp-forge-spike*`
  and no `-tmp-*` because the process ran in `/tmp/...`, and on APFS a
  folder stored in decomposed (NFD) form opens by its composed (NFC) name
  while `realpath` returns the stored form — so the adapter canonicalizes
  the caller's path before escaping, falling back to the path as given
  when it cannot be resolved. A session's
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
    tool-execution feedback) and 11 contained only `text` blocks — so a
    list is **not** always tool feedback, and `text` blocks must not be
    dropped. The final review found the deeper truth: 33 string-content
    and 9 of the text-list `user` lines carry `isMeta: true` — harness-
    injected content the user never typed (the full `Base directory for
    this skill: …` skill bodies, `<local-command-caveat>…`, scheduled-
    wakeup prompts) — and only 3 text-list lines are not meta. `isMeta`
    lines are skipped.
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
- Escaping helper: Claude Code's own directory-name rule (§14.1 — every
  non-ASCII-alphanumeric UTF-16 unit → `-`, over 200 characters → first
  200 plus `-` plus the base-36 hash of the original) to compute
  `claude_home/projects/<escaped>/`.
- Project path resolution: `list_sessions`/`load_session` canonicalize
  `project_path` (falling back to it as given when that fails) before
  escaping it (§14.1).
- `list_sessions`: if the escaped directory doesn't exist, `Ok(vec![])`.
  Otherwise one `SessionSummary` per `*.jsonl` file (other entries — real
  project directories also hold e.g. a `memory/` subdirectory — are
  ignored): `id` from the filename stem, `updated_at` from the file's
  mtime (`tokio::fs::metadata` — one stat call, not a full parse)
  formatted as RFC 3339 (a strict profile of ISO 8601), `title` from the
  last `ai-title`/`custom-title` line seen when scanning the file
  (`None` if absent; a cheap substring check skips full JSON parsing of
  every non-title line, since real transcripts reach 13 MB). The scan
  reads raw bytes: a line that is not UTF-8 (e.g. a live transcript's tail
  cut mid-character) is skipped, and a read error keeps the title found
  so far. Results are
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
  `isSidechain: true` or `isMeta: true`: `user` (string) → new `Message{role: User, ...}`;
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
  is `Option<String>` (§12.2), so it becomes `None`. A key whose value is a YAML
  block scalar (`>` or `|`, optionally followed by `+`, `-` or digits)
  collects the indented and blank lines that follow it — trimmed,
  non-empty ones joined with single spaces — until the next top-level line
  or the closing `---`; real `SKILL.md` files use `description: >`.
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
  the same flat map; `.mcp.json` entries win over global ones on name
  collision. Then merge the **local-scope** servers — `projects[<key>].
  mcpServers` in the same `~/.claude.json`, where `<key>` is the
  NFC-normalised form of the project path resolved like the transcript
  directory (§14.1: canonicalize, falling back to the path as given; both
  forms are tried, the resolved one first) — which win over both. The
  `projects` map is read in a second, isolated, best-effort parse that
  captures only each entry's `mcpServers` as a raw value: a malformed
  `projects` value or entry can never fail the global listing (it just
  means no local-scope servers), and no other project's servers or fields
  are used. Either source missing contributes zero entries. Malformed
  top-level JSON in either file → `McpRegistryError::ListFailed`.

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

## 15. Detailed Design: workspace walking skeleton (Plan 5)

Plan 5 is the first Dioxus-facing plan. It is a **vertical walking
skeleton**: the thinnest end-to-end slice that runs the real adapters
(Plans 1, 3, 4) and the Plan 2 use-cases inside a real desktop window, with
**several sessions running at once**. Its purpose is to retire the
integration risk (Dioxus runtime ↔ sidecar event stream) before the rest of
Tier 1 is built on top. It adds six frontend crates, two ports, one adapter
and a session controller in `forge-application`, and it changes the
signature of `forge-adapter-sidecar`'s `spawn`.

### 15.1 Scope, and the Claude Code desktop reference

§1 requires a UI equivalent to Claude Code desktop, so the layout follows
what that app actually does. Checked against its documentation on
2026-09-29 (`code.claude.com/docs/en/desktop`,
`claude.com/blog/claude-code-desktop-redesign`):

- Sessions are listed in a **sidebar** ("The sidebar lists your sessions and
  lets you run several in parallel"), active and recent together, groupable
  by project. It is **not** a tab bar.
- **+ New session** / Cmd+N starts one; Ctrl+Tab / Ctrl+Shift+Tab cycle
  through sessions; Cmd+W closes one. Archiving is a separate action.
- Before the first message a new session's prompt area chooses environment,
  **project folder**, **model** and **permission mode**.
- Cmd-clicking a sidebar session opens it in a second pane (split view);
  after the redesign every pane is drag-and-drop.
- Desktop also lists sessions started from the CLI.

The docs do **not** specify how the project folder is chosen or what marks a
running / awaiting-approval session in the sidebar. Those are this design's
own choices and are labelled as such below; they are not parity claims.
Earlier text in §7 ("tabs or multiple windows") is superseded: multiple
sessions are a sidebar list with one main pane; split panes are deferred.

**In scope:** a sidebar of sessions grouped by project (live and past
together); read-only viewing of a past session; a new-session panel (folder
from the known projects that still exist on disk, model, permission mode);
chat with streaming text, one-line tool-call display, sending and
interrupting; permission prompts (allow once / allow always / deny); any
number of sessions running concurrently; Cmd+N, Ctrl+Tab, Cmd+W.

**Out of scope (deferred, not forgotten):** split panes and the
drag-and-drop grid; folder-open dialog; settings UI (credentials come from
hand-edited `config.json`); model list (the model selector offers only
`default`, i.e. `None` — no port lists models and how the SDK exposes them
was not verified); markdown, syntax highlighting, diff viewer; terminal /
preview / side chat; environment selector (Local only); filters, archive;
file watching; packaging (`build/`); visual parity with Claude Code desktop
(the theme is neutral); Windows.

Layout (main pane shows one session; the others keep running):

```
┌────────────────────────┬────────────────────────────────────────────────────┐
│ [+ New session]   ⌘N   │ forge · Wire adapters into the app                 │
│ ▾ forge                │ You / Claude messages, streaming text              │
│   ● Wire adapters      │   ▸ Read  docs/…/forge-design.md   (tool call)     │
│   ○ MCP local scope    │ ┌─ Permission ─────────────────────────┐           │
│ ▾ dotfiles             │ │ Bash: cargo test  [Allow][Always][Deny]│         │
│   ○ zsh setup          │ └──────────────────────────────────────┘           │
│ ● running  ○ idle      │ [ Message…            Mode ▾ Model ▾   [Send] ]    │
└────────────────────────┴────────────────────────────────────────────────────┘
```

### 15.2 Firsthand verification done before this design

Run 2026-09-29 against a live `~/.claude/` (read-only; counts, not
contents). Findings that shaped or corrected the design:

- **The `~/.claude.json` `projects` map is not a usable project list.**
  29 entries; 22 point at paths no longer on disk; 26 have no transcripts;
  only 2 carry `lastSessionModified` and 23 carry `lastSessionId` (a first
  pass reported these keys as present "on every entry" — that was a union of
  keys, wrong). Meanwhile `~/.claude/projects/` holds 87 transcripts in 12
  directories, and only 3 of those 12 directories (10 of the 87 sessions)
  correspond to a registry entry. The missing 9 are git worktrees,
  a directory Claude Code was launched in but never registered, and
  `/private/tmp` spike directories. A registry-driven sidebar would hide
  ~88% of sessions. §4.2's "project registry inside `~/.claude.json`" does
  not hold for this purpose; that file remains the source of per-project
  MCP servers (§14) only.
- **The real project path is recoverable from the transcripts.** Directory
  names are a lossy escape of the path (§14.1) and cannot be reversed, but
  transcript lines carry a `cwd` field (not every line does — the first line
  of a transcript may be one without it). For all 12 transcript-bearing
  directories a `cwd` was found within the first 60 lines of every
  transcript (0 files without one), each
  directory had exactly one distinct `cwd`, and applying Claude Code's own
  escape rule (§14.1) to that `cwd` reproduced the directory name in all 12.
  Not yet observed here: a directory holding several distinct `cwd`s, or a
  200-character-capped name; §15.4 handles both defensively.
- **`SidecarProcess::spawn(node_binary, script_path)` takes no environment.**
  §12.3 says credentials are injected through environment variables at
  spawn time as a composition-root concern, but the current `spawn` has no
  parameter for them (`Command` inherits the parent environment).
- **Sidecar behaviour read from `sidecar/src/session.ts` and `index.ts`**
  (read, not run): `start_session` is ignored after the first call (a
  `started` flag); a thrown query emits `error` and **not** `session_ended`;
  the process exits only on a `shutdown` command; `session_started` is
  emitted from the SDK's `system`/`init` message; a `canUseTool` request
  auto-denies on abort. Whether `init` arrives before or after the first
  user message, and what events follow an `interrupt`, was **not** run and is
  a spike question (§15.8).
- **This worktree has no built sidecar** (`sidecar/node_modules`,
  `sidecar/dist` absent): `npm ci && npm run build` is a prerequisite for
  anything that runs the real sidecar.
- **Dioxus 0.7.10** (docs, not run): async event handlers, `spawn`,
  `use_future`, `use_context_provider`/`use_context` exist; signals are
  `Copy` and must not be held across `.await`; `onkeydown` exposes modifier
  keys and `prevent_default`; the desktop `Config` exposes `WindowBuilder`
  customization and a raw `custom_event_handler`. **Not** confirmed by docs:
  that the desktop runtime enables `tokio::process`, and whether the macOS
  native default menu swallows Cmd+W / Cmd+N before the webview.

### 15.3 New domain type and ports

```rust
// forge-domain
pub struct ProjectSummary {
    pub path: PathBuf,        // real project path, recovered from a transcript's cwd
    pub last_active: String,  // RFC 3339, mtime of the newest transcript
    pub session_count: usize,
    pub exists: bool,         // `path` is a directory on disk right now
}

// forge-ports (each with `#[cfg_attr(any(test, feature = "mocks"), automock)]`)
#[async_trait]
pub trait ProjectRepositoryPort: Send + Sync {
    async fn list_projects(&self) -> Result<Vec<ProjectSummary>, ProjectRepositoryError>;
}
pub enum ProjectRepositoryError { ListFailed(String) }

#[async_trait]
pub trait SidecarFactoryPort: Send + Sync {
    async fn spawn(&self) -> Result<Arc<dyn SidecarPort>, SidecarError>;
}
```

`exists` is computed by the adapter so `forge-application` performs no
filesystem I/O. `forge_domain::SessionId` also gains `Hash` (the sidebar
de-duplicates sessions by id). `SidecarPort` itself is **unchanged**: it stays
single-session (§3, one sidecar per session), which is what makes concurrent
sessions cheap — the isolation already exists.

### 15.4 Adapter changes

**`forge-adapter-claude-fs` — `ClaudeProjectAdapter::new(claude_home)`**
implements `ProjectRepositoryPort`. Read-only, same tolerance rules as §14.2:
1. `read_dir(<home>/projects)`; a missing directory → empty list; another
   error → `ListFailed("<path>: <error>")`.
2. For each sub-directory, collect its `*.jsonl` files with mtimes; none →
   directory skipped. `session_count` is the file count; `last_active` is the
   newest mtime formatted like `list_sessions`'s `updated_at`.
3. `path` is the first `cwd` found scanning transcripts **newest first**, at
   most the first 60 lines of each, ignoring lines that do not parse (a
   partial final line or invalid UTF-8 never fails the scan). No `cwd` in
   any transcript → the directory is skipped, never an error.
4. `exists` = the recovered path is a directory now.
5. Sorted by `last_active` descending, ties broken by `path`.

Two directories can recover the same `path` (a symlinked and a resolved
spelling); the sidebar merge (§15.6) therefore de-duplicates sessions by id.

**`forge-adapter-sidecar`**
- `SidecarProcess::spawn(node_binary, script_path, env: &[(String, String)])`
  applies `env` on top of the inherited environment. The Plan 1 call sites
  (the contract tests and the `spike_drive_sidecar` example) pass an empty slice.
- `SidecarProcessFactory::new(node_binary, script_path, config:
  Arc<dyn ConfigRepositoryPort>)` implements `SidecarFactoryPort`. Each
  `spawn()` loads `AppConfig` afresh (config edits apply to the next session
  opened) and sets `ANTHROPIC_API_KEY` from `api_key` and
  `ANTHROPIC_BASE_URL` from `base_url` **only when `Some` and non-empty**;
  otherwise the parent environment is inherited. The variable names are the
  expected SDK ones and are verified by the spike (§15.8 d). A missing config
  file is not an error (the config adapter returns `AppConfig::default()`, so
  nothing is injected); a config that cannot be loaded is
  `SidecarError::SpawnFailed("failed to load the Forge config: …")`, so a
  broken config is reported instead of silently ignored. The config is never
  formatted anywhere (its `Debug` output contains the API key).
- Node path and sidecar script path come from `FORGE_NODE` (default `node`)
  and `FORGE_SIDECAR_SCRIPT` (default `sidecar/dist/index.js`) in the
  composition root. Bundled resolution is packaging's job (§9), out of scope.

### 15.5 `forge-application`: `LiveSession` and `SessionRegistry`

New normal dependency: `tokio` (`sync`, `rt`, `macros`). It is a runtime
library, not an adapter, so §6's rule (no concrete adapter crate) holds; the
coupling — `open_new`/first send call `tokio::spawn` and must run inside a
tokio runtime — is accepted and pinned by the spike (§15.8 a).

**Identity.** `LiveSessionId(u64)` is an app-local handle. Claude's
`SessionId` is unknown for a brand-new session until the sidecar reports
`session_started`, so it cannot be the key.

**Ownership: an actor.** One tokio task per `LiveSession` owns its
`LiveSessionView` (including the `Session`) exclusively and `select!`s, in
priority order (`biased`), between the command channel, `sidecar.next_event()`
(only while a sidecar exists) and publishing. No locks; the existing
`SendMessageUseCase`'s `&mut Session` fits directly. The actor publishes a
clone of its view through a `tokio::sync::watch` channel (`send_replace`), and
only when neither a command nor a sidecar event is immediately ready, so a
burst of events is applied first and published once — no timer is needed.
(An earlier draft published with `send_modify` and let consumers clone on
read; that cannot work because the use-case holds `&mut Session` across an
`.await`.)

```rust
pub struct LiveSessionId(pub u64);
pub enum Phase { Idle, Running, Failed }     // Idle = no turn in flight

pub struct LiveSessionView {
    pub id: LiveSessionId,
    pub project_path: PathBuf,
    pub session: Session,                    // session.id is "" until known
    pub claude_session_id: Option<SessionId>,
    pub phase: Phase,
    pub pending_permissions: VecDeque<PermissionRequest>, // parallel tool calls
    pub last_error: Option<String>,
    pub can_send: bool,                      // false when the folder is gone
}

pub enum LiveCommand { Send(String), Approve { id: String, decision: PermissionDecision },
                       Interrupt, Close }

#[derive(Clone)]
pub struct LiveSessionHandle { /* id, command sender, watch::Receiver<LiveSessionView> */ }
// send(text) / approve(id, decision) / interrupt() / close() are fire-and-forget;
// subscribe() -> watch::Receiver<LiveSessionView>

pub struct SessionRegistry { /* factory, sessions repo, handles */ }
  .open_new(&ProjectSummary, PermissionMode, Option<String>) -> Result<LiveSessionHandle, OpenError>
  .open_existing(&ProjectSummary, SessionId) -> Result<OpenOutcome{ handle, already_open }, OpenError>
  .close(LiveSessionId)
pub enum OpenError { ProjectMissing, Load(SessionRepositoryError) }
```

**Lazy sidecar (this design's choice, not a verified Claude Code
behaviour).** `open_existing` calls `load_session` for a read-only view
(`phase = Idle`, no process). The **first `Send`** spawns a sidecar via the
factory, sends `start_session` (with `resume_session_id` for an existing
session, using the loaded `permission_mode`), then sends the message through
`SendMessageUseCase`. `start_session` is called directly on `SidecarPort`
because `StartSessionUseCase` cannot carry a resume id and
`ResumeSessionUseCase` loads history eagerly; the actor uses the Send,
Approve and Interrupt use-cases. `SendMessageUseCase` records a message
*before* writing it, so a failed write removes the message again.
`open_new` is the same without a resume id.
`ResumeSessionUseCase` (which starts the sidecar eagerly) is left in place
and unused by the sidebar. `open_existing` for a Claude session id that a
live handle already holds returns that handle with `already_open = true` —
never a second sidecar on the same transcript. `open_new` on a missing
folder → `OpenError::ProjectMissing`; `open_existing` on one succeeds
read-only with `can_send = false`.

`Close`: send `shutdown` if a sidecar exists, drop it (`kill_on_drop`), end
the task; the registry forgets the handle. Closing a running session ends
its turn (own choice).

**Event and failure rules**

| Situation | Result |
|---|---|
| spawn or `start_session` fails | `Failed`, `last_error` set; the user message is **not** added to history (the UI keeps the input text) |
| write to sidecar fails (`ProcessExited`, `WriteFailed`) | sidecar dropped, `Failed`; a message that was recorded but never delivered is removed from the history |
| `Error` event | `shutdown` + drop the sidecar (its query is over and `start_session` cannot be re-sent), `Failed`. The protocol cannot tell a fatal error from a non-fatal one (the adapter even reports an unparseable line as `Error`), so every `Error` is treated as fatal |
| `next_event()` returns `None` | process gone, `Failed`; stop polling |
| unexpected `SessionEnded` | `Failed("session ended")` |
| `Send` while `Failed` | respawn; resume with the known Claude session id, or start fresh if none was reported yet |
| `SessionStarted` | set `claude_session_id` and `session.id` |
| `PermissionRequested` | push on `pending_permissions`; `Approve` pops the matching id and calls `ApprovePermissionUseCase` |
| `TurnCompleted` | `Idle` |
| folder missing | history readable, `can_send = false` |

### 15.6 Frontend crates and UI state

Dependency direction is §6's, unchanged. Each crate is deliberately thin.

| Crate | Plan 5 contents |
|---|---|
| `forge-shared` | theme (CSS variables), `Button`, `Select`, `TextArea`, `StatusDot` |
| `forge-entities` | `MessageBubble` (text), `ToolCallLine` (name + running/done/error), `PermissionCard` (display only), `SidebarItem` (title, relative time, marker). Depends on `forge-domain` + `forge-shared` only, so the sidebar marker (`Idle`/`Running`/`NeedsAttention`) is a UI enum defined here and mapped from `Phase` by the widget |
| `forge-features` | `use_workspace` (session store hook), send-message, approve-permission, interrupt-turn, open-session, new-session, close-session, keyboard-shortcuts hook |
| `forge-widgets` | `Sidebar`, `ChatWindow`, `NewSessionPanel` |
| `forge-pages` | `WorkspacePage` (sidebar + main pane + empty state) |
| `forge-app` | adapter wiring, context injection, window config; the only crate enabling `dioxus/desktop` |

**State flow.** (1) `forge-app` builds the adapters, the `SessionRegistry`
and the two repositories and injects them with `use_context_provider`.
(2) `use_workspace()` owns the sidebar's past sessions (`list_projects`, then
`list_sessions` per project, merged and **de-duplicated by session id**), the
open sessions, and the active item. A past session that has a live handle is
drawn from the live entry. The sidebar is built from a small `LiveSummary` per
open session (id, project path, Claude session id, title, first message,
marker, `can_send`), never from whole views, so drawing it does not clone a
conversation. The past list is reloaded at start and when a session is
closed; there is no file watching. (3) **Send ↔ !Send bridge:**
each actor runs under `tokio::spawn`; for each handle the hook `spawn`s a
Dioxus-local task that awaits `watch::Receiver::changed()`, clones the value
out (no borrow held across an `.await`), and `set`s that session's `Signal`.
The task ends when the actor drops its sender. (4) Shortcuts are handled by
`onkeydown` on the workspace root: Cmd/Ctrl+N new session, Ctrl+Tab /
Ctrl+Shift+Tab cycle, Cmd/Ctrl+W close the active session. If the spike (§15.8
c) shows the native menu swallows any of them, they move to native menu
accelerators through the desktop `Config`.

Sidebar markers (own choice, not verified against the real app): `●`
running, `○` idle, and a distinct "needs attention" marker when
`pending_permissions` is non-empty or the session `Failed`. A folder-less
(missing) project shows its history but no message box, only a note that the
folder no longer exists. The message boxes ignore an Enter that confirms an
IME composition (`is_composing()`), so typing Korean does not send half a
syllable; this cannot be unit-tested and is checked by hand (§15.8 c, §15.7).

### 15.7 Testing

Per §8; the frontend crates have **no rendering tests** in this plan.

- **`ClaudeProjectAdapter`** — temp-dir fixtures (never the real `~/.claude`):
  `cwd` recovery; several distinct `cwd`s in one directory (newest
  transcript wins); a directory with no `.jsonl`; a transcript with no `cwd`
  (directory skipped); a partial final line and invalid UTF-8 (tolerated); a
  `cwd` on a later line than the first; sort order and tie-break; `exists`
  true/false; missing `projects` directory (empty list); a 200-character-capped
  directory name; round-trip with `ClaudeSessionAdapter::list_sessions` for the
  recovered path. A read-only smoke run against the real `~/.claude` is a
  manual verification step, not a committed test.
- **`forge-adapter-sidecar`** — the contract test gains an env-passthrough
  case (a fixture script that echoes an environment variable); the factory is
  tested with a mock `ConfigRepositoryPort` for: both values set, `None`,
  empty string, and a config load failure.
- **`LiveSession` / `SessionRegistry`** — `MockSidecarFactoryPort` and
  `MockSessionRepositoryPort`, plus a small fake sidecar driven through a
  control (`MockSidecarPort` cannot script a sidecar that is alive but quiet,
  i.e. a `next_event` that stays pending); one test per row of
  the §15.5 table, plus: no spawn on `open_existing`; the first `Send` orders
  spawn → `start_session` → `send_message`; resume id present for an existing
  session and absent for a new one; respawn after `Failed`; duplicate open
  returns the existing handle; `Close` sends `shutdown`; permission queue
  order; commands and events interleaved (`select!` cancel safety);
  `ProjectMissing`.
- **Frontend logic** — plain functions, unit-tested: phase→marker mapping,
  sidebar merge / group / de-dup, Ctrl+Tab cycle order, relative time.
- **Whole app** — manual `dx serve` checklist (Dioxus has no mature
  automated desktop UI testing; stated, not claimed as covered): sidebar
  lists the 12 projects / 87 sessions on the reference machine; a past
  session opens read-only; Cmd+N starts a session in a scratch folder;
  streaming text; a permission prompt each way; interrupt; two sessions
  streaming at once; Cmd+W closes one and the other continues.

### 15.8 Task order and the spike

Risk first, then the testable core, then the UI. Task 6 is a checkpoint (the
core is complete and the workspace tests are green); the plan stays a single
plan (decided 2026-09-29).

1. **Spike (throwaway, not committed).** A scratch desktop window, added as
   a temporary workspace member `crates/forge-spike` and run with
   `dx serve -p forge-spike --platform desktop` (the flags exist in
   `dx` 0.7.10; that they work in this workspace is question e), driving the
   real sidecar. Questions:
   (a) do `tokio::spawn` and `tokio::process` work inside the Dioxus
   desktop runtime;
   (b) does real sidecar stream → `watch` → local task → `Signal` render live;
   (c) do Cmd+N / Cmd+W / Ctrl+Tab reach `onkeydown` on macOS;
   (d) does the SDK honour `ANTHROPIC_API_KEY` / `ANTHROPIC_BASE_URL` from
   the spawn environment, and what happens with neither;
   (e) how to run a workspace-member desktop binary with `dx serve`;
   (f) when `session_started` arrives relative to the first user message;
   (g) which events follow an `interrupt` (does `turn_complete` arrive?).
   Prerequisite: `npm ci && npm run build` in `sidecar/` (verified to work:
   `dist/index.js` results and the sidecar's 18 tests pass). Questions (a),
   (c), (d) and (f) are meant to cost nothing: (d) is checked with an invalid
   key and an unreachable base URL, and starting a session is expected not to
   call the API (unverified). Only (b) and (g) make a real API call with the
   user's credentials — ask before making it; if the SDK ignores an
   environment variable that (d) sets, that check itself becomes a real
   one-word call, which is the finding. Output is findings (recorded as
   §15.10) and a recommendation, as for §10. The implementation plan is
   `docs/superpowers/plans/2026-09-29-forge-workspace-walking-skeleton.md`.
2. `ProjectSummary` and the two ports, with mocks.
3. `ClaudeProjectAdapter`, plus the read-only real-`~/.claude` smoke run.
4. `spawn` env parameter and `SidecarProcessFactory`.
5. `LiveSession` actor.
6. `SessionRegistry` — **checkpoint**.
7. `forge-shared`, `forge-entities`.
8. `forge-features`.
9. `forge-widgets`, `forge-pages`.
10. `forge-app` composition root and window; first `dx serve` run.
11. Manual checklist, §15 finalisation, final review.

### 15.9 Corrections to earlier sections, and known limits

- §4.2: the project list for the UI comes from the transcript directories
  (§15.2), not from `~/.claude.json`.
- §6: `forge-ports` gains `ProjectRepositoryPort` and `SidecarFactoryPort`;
  `forge-application` gains a normal `tokio` dependency.
- §7: "multiple sessions open at once" moves from Tier 2 into this skeleton,
  as a sidebar list (not tabs); split panes stay Tier 2.
- §12.3: `SidecarProcess::spawn` now takes an environment (§15.4).
- §15.1 Review Focus item 5: corrected by the spike (§15.10 (c)) from "checked
  by hand, expected to pass" to a **verified, currently unfixable gap**:
  Dioxus desktop 0.7.10 on macOS (wry/WKWebView) exposes no signal —
  `is_composing()`, composition events, or a raw key code — that
  distinguishes an in-progress Korean IME composition from ordinary typing,
  so pressing Enter mid-syllable can send a half-typed message. The
  `is_composing()` check stays in `SendMessageBox`/`NewSessionComposer`
  (harmless, may work on other platforms) but the plan's raw-key-code-229
  fallback is dropped — §15.10 (c) has why.
- Known limits: the marker style, folder-picker behaviour, lazy sidecar and
  close-while-running behaviour are own choices (§15.1, §15.5); a missing
  project folder cannot be resumed; the sidebar reflects sessions created
  elsewhere only at start and on close; `ResumeSessionUseCase` is unused by
  the UI; the model selector offers only `default`; Windows is unverified.
  Every publish and every render clones the whole conversation (a multi-
  megabyte tool output will make typing sluggish); the message list does not
  auto-scroll while streaming; every sidecar `Error` ends that sidecar (§15.5).
  The build prints one `block v0.1.6` future-incompatibility notice from the
  Dioxus desktop dependency tree; it is not from this repository. **Pressing
  Enter mid-Korean-IME-composition can send a half-typed message on macOS
  desktop** — confirmed by the Task 1 spike (§15.10 (c)), not fixable with
  Dioxus 0.7.10's public API. A misconfigured `AppConfig` (bad key or
  unreachable base URL) does not fail fast: the SDK retries silently and a
  turn just hangs from the UI's point of view (§15.10 (d)).

### 15.10 Spike findings (Task 1)

Run 2026-09-29 in the `plan-5` worktree, `crates/forge-spike` (throwaway,
deleted after this write-up), against the real sidecar built from this
repository. `dx serve -p forge-spike --platform desktop` works (build ~53 s
cold, ~3 s incremental) — **(e)**. The user gave consent for the two calls
that would touch the real API (§15.8 Step 7) before they were made.

- **(a) tokio runtime in UI event handlers — yes.** Every `start`/`send`
  closure (invoked from a button `onclick`, i.e. the Dioxus UI thread) saw
  `tokio::runtime::Handle::try_current().is_ok() == true`, and
  `SidecarProcess::spawn`, `tokio::spawn` and `next_event().await` all ran
  directly from there, across four separate runs (`dx serve`, `cargo run`,
  two `open -n` runs of the built `.app`). No dedicated runtime/handle
  plumbing is needed; Tasks 4–6 and 10 can call these directly from UI code.
- **(b) real stream → watch → Signal, window stays responsive — yes.**
  With the user's consent, a real turn ("count 1 to 100") streamed 25
  `AssistantDelta` events between 105.34 s and 106.44 s of the run,
  followed by `TurnComplete`; a 100 ms heartbeat running as a second
  Dioxus-local task never gapped past 123 ms across the whole ~140 s
  scripted run (idle or streaming). The bridge in §15.6 works as designed.
- **(c) keyboard and IME — mixed, one new finding, one design gap that
  cannot be closed with this stack.**
  - ⌘N, Ctrl+Tab and Ctrl+Shift+Tab reach `onkeydown` (logged with the
    right modifiers). ⌘⇧N was not confirmed in the log on the one run
    that tried it; it is not one of the app's actual shortcuts (§15.6), so
    this is not pursued further.
  - **⌘W reaches `onkeydown` *and* the native default menu still closes
    the OS window** — worse than the plan's outcome table anticipated
    (which only considered "swallowed silently" vs. "reaches the page").
    Traced to `dioxus::desktop`'s `default_menu_bar()`
    (`menubar.rs`), whose "Window" submenu includes
    `muda::PredefinedMenuItem::close_window(None)`, bound to the
    platform's standard accelerator (⌘W on macOS) independently of the
    webview's event handling. **Amendment (applies to Task 10):** build a
    custom menu with `dioxus::desktop::muda` — the default items minus
    `close_window` — and pass it via `Config::with_menu(..)`, so only
    `Workspace::on_key` decides what ⌘W does (close the active session,
    never the window). The window's own chrome (red traffic-light button,
    ⌘Q) still closes/quits normally; that is unaffected and fine.
  - **Enter mid-Korean-IME-composition: `is_composing()` reports `false`
    throughout, on two separate runs**, including on the very `Enter`
    that follows six Hangul jamo keydowns
    (`ㅇㅏㄴㄴㅕㅇ`) typed via the OS's built-in Korean input source.
    Added `oncompositionstart`/`oncompositionupdate`/`oncompositionend`
    handlers to the spike's textarea as a second signal (Dioxus does wire
    these — `dioxus-html`'s `convert_composition_data` — to the browser's
    real `compositionstart`/`update`/`end`): **none of the three fired at
    all**, on a repeat of the same test. The plan's proposed fallback (a
    raw key code of 229) is also **not reachable**: `serialize.ts` does
    capture `event.keyCode` into `SerializedKeyboardData.key_code`, but
    every field of that struct is private in `dioxus-html` 0.7.10 with no
    accessor, and `key_code`/`code_point` are not part of the
    `HasKeyboardData` trait — there is no public API path to it, downcast
    or otherwise. **Finding: on this stack (Dioxus desktop 0.7.10 /
    wry / WKWebView, macOS), there is no signal visible to application
    code that distinguishes an in-progress Korean IME composition from
    ordinary typing.** This is a platform/library gap, not a bug in this
    spike's code. **Amendment (applies to Task 8/9 and Review Focus
    item 5):** keep the `is_composing()` check in `SendMessageBox` and
    `NewSessionComposer` as written (it is free and correct wherever the
    underlying platform does report it, e.g. it may differ on Linux/
    Windows or in a future Dioxus release) but drop the raw-key-code
    fallback from the plan — it cannot be built. Move "Enter mid-IME-
    composition can send a half-typed message on macOS desktop" from
    Review Focus item 5's "checked by hand, expected to pass" framing
    into **Known limitations** below: it is a verified, currently
    unfixable gap in this stack, not an unverified risk.
- **(d) `ANTHROPIC_API_KEY` / `ANTHROPIC_BASE_URL` — both honoured, and
  precisely confirmed.** Driving the sidecar against a local fake HTTP
  server (`127.0.0.1:8765`) with `ANTHROPIC_BASE_URL` pointed at it and
  `ANTHROPIC_API_KEY=sk-ant-spike-invalid-key` showed the outbound
  `POST /v1/messages` carrying `x-api-key: sk-ant-spike-invalid-key` (the
  exact injected value, no `Authorization` header) against the injected
  base URL. Separately: pointing `ANTHROPIC_BASE_URL` at an unreachable
  address (`http://127.0.0.1:9`) or setting only an invalid
  `ANTHROPIC_API_KEY` against the real API produced **no `Error` event and
  no `TurnComplete` within 30 s** in either case — the SDK retries a bad
  credential/unreachable host silently rather than failing fast. Worth
  knowing for Tasks 4–5: a misconfigured `AppConfig` does not surface as
  an immediate, visible failure; the turn just hangs from the UI's point
  of view. No plan change forced by this (out of scope for the skeleton),
  noted as a known limitation.
- **(f) `session_started` timing — only after the first message, confirmed
  three times.** Zero `SessionStarted` events appeared in a 15 s idle
  window after `start_session` with nothing sent; it arrived 0.07–0.8 s
  after the first `send_message` in every run that sent one. Matches the
  plan's default assumption (§15.8 outcome table); no change needed.
- **(g) events after `interrupt` — a `TurnComplete` *does* follow.**
  Interrupting ~50 ms after `send_message` (before any `AssistantDelta`
  had arrived) returned `Ok(())` from `interrupt()` and was followed
  ~50 ms later by `TurnComplete { input_tokens: 0, output_tokens: 0 }`.
  **The Task 5 "no `turn_complete` after `interrupt`" amendment in the
  plan's outcome table is therefore not applied** — the default design
  (the actor returns to `Idle` on `TurnComplete`, spec §15.5's table) is
  what Task 5 implements, unmodified. `shutdown` was independently
  confirmed to always end the event stream with `SessionEnded`.

**Amendments made to this spec and the plan as a result (recorded, not
merely proposed, since they change what Tasks 8–10 build):**
1. Task 10's `window_config()` gains a custom menu (§15.10 (c), ⌘W).
2. Review Focus item 5 and the "Known limitations" paragraph (§15.1) are
   corrected: the IME-composing guard is confirmed non-functional on
   macOS with this Dioxus version, not merely untested — see the plan's
   own Amendment A for the exact wording.
No other question changed a task: (a), (b), (d) (no code change forced),
(f) and (g) confirm the plan's existing design.

### 15.11 As built

Plan 5 was built on branch `plan-5` and verified by hand on 2026-09-30 (macOS 26, Dioxus 0.7.10).

**Test count.** 247 workspace tests (133 before Plan 5), no warnings other than the `block v0.1.6` future-incompatibility notice from the Dioxus desktop dependency tree. The sidecar's own 18 tests pass.

**Manual checklist (Task 11), real `~/.claude`, real API turns.** Items 1–11, 13, 14, 15 passed as written. Item 12 (Korean IME) behaves as §15.10 (c) predicted: the first Enter sends. Details worth keeping:
- 1: 22 project groups on this machine; deleted folders struck through.
- 2/3: opening a past session spawns no process; the first send spawns exactly one `node` and the same transcript grows.
- 5: Write requests show the card and Allow / Always allow / Deny all work; a pending request shows as a filled dot in the sidebar (idle is a hollow ring). `Bash echo`/`date` never prompted in default mode — SDK behaviour, not Forge.
- 9: ⌘W closes the session only, its `node` process disappears and the window stays — the custom menu of §15.10 (c) works.
- 13: `kill` of a streaming session's sidecar gives an error banner and the attention marker; resending respawns and resumes.
- 14: a broken `config.json` gives the "failed to load the Forge config" banner; deleting it makes the next send work.
- 15: typing stays responsive while a 246 KB file is printed (tool output is drawn as one line, so the conversation grows but little is rendered).

**Runtime fixes** (the components had only been compiled before Task 10):
1. `fix: pin the live-session signal bridge to the app's root scope` — `Workspace::attach` used `spawn`, which Dioxus cancels when the calling component unmounts. `NewSessionComposer` unmounts the moment `select` runs, so a brand-new session never rendered anything. Now `dioxus::core::spawn_forever`. No automated test (needs a live window). One earlier theory — subscribing before spawning — was wrong (a cloned `watch::Receiver` keeps the original's version) and was left in only because it is harmless.
2. `fix: keep the text of a message that could not be sent` — spec §15.5 says the UI keeps the input text on a failed send, but the box was cleared immediately. `LiveSessionView` gained `unsent` and `failed_sends`; `SendMessageBox` restores the text. Three actor tests.

**Where the built system differs from §15 (new).**
- `Workspace::attach` runs its bridge in the root scope (above); `LiveSessionView` has two more fields.
- The Folder dropdown only offers folders that already have Claude Code history, so a brand-new empty folder cannot be chosen (no folder picker, §15.1). Task 11 item 4 says "pick the scratch folder", which only works after running `claude` once there.
- The sidebar markers are hard to tell apart: "running" and "needs attention" are both small warm filled dots (8 px). Verified correct in data, weak visually — a design-polish item (§15.1 puts visual polish out of scope).
- Testing note: this machine runs Claude Code under the Orca terminal, whose global `PermissionRequest` hook is driven by `ORCA_AGENT_HOOK_*` variables. Stripping them was tried; it did not change the results above, so they are not the cause of anything recorded here.
- Killing the app and its sidecars repeatedly while debugging left one session that reported `AbortError: Stream closed` on writes. It did not reproduce on a clean session and is treated as a debugging artifact.

### 15.12 Final review of Plan 5

A fresh reviewer read the whole branch. No Critical findings. Fixed (each with a test that failed first): a finished turn now drops its pending permission requests; sidebar entries have a unique DOM key; the sidecar exits when its stdin closes (no orphaned `node`/`claude` after quitting Forge). Test count is now 249 (+ sidecar 19).

**Not fixed, on purpose**
- *Lossy directory names can resume a session in the wrong folder* (`/work/my_app` and `/work/my-app` share one Claude directory; the project shows the newest `cwd`, and resuming an older session uses it). Needs each session to carry its own `cwd` through the port and an existence check for it; that is a change to §14/§15.3, not a fix. Rare (needs colliding names), and edits still go through permission prompts in the default mode. **Follow-up.**
- *Graceful shutdown before dropping the sidecar* (reviewer's guess that `kill_on_drop` beats the queued `shutdown`). Checked with `ps` after ⌘W mid-turn and after `kill`: no `node` or `claude` process was left. Revisit only if a leftover is ever seen.

**Deferred minors.** Closed sessions keep their signal (memory) until exit; a Folder that disappears makes Start do nothing silently; `exists`/`can_send` are snapshots until the next reload; a message can stay in the history if the sidecar dies between two events; after a failure before `SessionStarted` the retry starts a fresh Claude session while the view still shows the first message; `biased` batching could delay publishing under a very fast event stream.

## 16. Detailed Design: design polish and the settings overlay (Plan 6)

Plan 6 makes the Plan 5 skeleton look like a product and removes its most
annoying rough edge: credentials no longer have to be typed into
`config.json` by hand. It adds no new crate and no new port.

### 16.1 Scope, and the Claude Code desktop reference

Reference: screenshots of Claude Code desktop supplied by the user on
2026-09-30 (not documentation; these are parity observations, re-check them
against the docs before claiming more):

- The window is one flat tone; the sidebar is only slightly darker than the
  main pane. Dividers are rare, whitespace does the grouping.
- A project group's title is small and muted, with a `+` at its right end that
  starts a session in that folder.
- The prompt is one rounded bordered box. A chip row above it names the
  environment and folder; under it the model is shown in small text.
- A menu at the bottom of the sidebar holds **Settings (⌘,)**.
- Settings is a large overlay covering most of the window (the app behind it
  is dimmed and blurred): a left navigation (search, grouped sections) and, on
  the right, rows with the name and a one-line description on the left and the
  control on the right.

**Decisions of this design** (user-approved during brainstorming):

- Visual tone: flat single background. Cards only for permission requests,
  tool calls and the user's own messages; assistant text has no card.
- Settings opens as the overlay above. Sections in this plan: **Connection**
  and **Model**. No search box (two sections).
- Connection fields are saved with a Save button; the model is saved the
  moment it is chosen.
- Saved settings apply to sessions started afterwards. A running session keeps
  the environment it was spawned with (`SidecarProcessFactory` reads the config
  on every spawn, and its test `factory_test.rs` already asserts that).

**In scope:** theme rewrite (`theme.css`); sidebar group `+`, bottom Settings
menu, distinguishable markers; settings overlay with Connection and Model;
`AppConfig.model`; new sessions start with the saved model; `⌘,`.

**Out of scope (deferred, not forgotten):** Skills / Plugins / MCP tabs and
their definition readers (Plan 7 candidate); folder-open dialog; markdown,
syntax highlighting, diff viewer; per-session model choice and changing the
model mid-session; a theme chooser (the theme still follows
`prefers-color-scheme`); settings search; the session-`cwd` collision of §15.12;
split panes; session rename / delete.

### 16.2 Domain and application

- `AppConfig` gains `model: Option<String>` with `#[serde(default)]`. A
  `config.json` without the key loads with `None`; `None` is written as
  `null`. `forge-adapter-config` needs no change.
- An empty or all-whitespace `base_url`, `api_key` or `model` is stored as
  `None` (one normalising function in `forge-domain`, used by the settings
  feature). The factory already skips empty values, so behaviour is the same
  either way; `None` keeps the file tidy.
- `forge-application` gets thin use-cases `load_settings` and `save_settings`
  over `ConfigRepositoryPort` (§12.3), in `use_cases/`. `save_settings`
  **loads the current config, replaces only the fields the UI owns
  (`base_url`, `api_key`, `model`) and saves**, so `enabled_*` lists written by
  hand or by a later plan are never erased.
- `start_new` takes its model from the saved config: `Workspace` obtains the
  repository handle through its existing services struct and reads the model
  when a session starts. If the config cannot be loaded, the start fails with
  the existing "failed to load the Forge config" notice (§15.11, item 14).

### 16.3 `forge-features`: settings

A `settings` feature module, following the shape of the other features
(`use_settings()` returns a handle, state lives in signals):

- `open` (bool) and `section` (`Connection` | `Model`).
- A draft of the three fields, loaded from the config each time the overlay
  opens, so Cancel / ✕ discards edits.
- `save_connection()` writes `base_url` + `api_key`; `choose_model()` writes
  `model` immediately. Both set `saved` on success or keep the draft and set
  an `error` string on failure; a failed write never closes the overlay or
  drops the text (same rule as the unsent message, §15.11).
- `shortcut.rs` gains `Shortcut::OpenSettings` for `⌘,` (`Ctrl+,` off macOS).
  `shortcut_for` already takes `key`, `ctrl`, `meta`, `shift`, `mac`; the new
  arm matches `","`. Esc closes the overlay.
- The new-session panel's Model `select` (today offering only "Default model",
  §15.1) is replaced by a read-only line showing the saved model, or
  "Default model" when `None`. Choosing is done in Settings.

### 16.4 `forge-widgets` / `forge-pages`

- `SettingsOverlay` widget: backdrop, panel, left navigation, a row component
  (`SettingRow { title, description, children }`) in `forge-shared`.
- Connection: Base URL (text), API key (password input with a show / hide
  toggle), Save button (disabled until the draft differs), and the line
  "Sessions that are already open use the old values until they are started
  again."
- Model: a list of `Default (chosen by the SDK)`, Fable 5.1 (`claude-fable-5-1`),
  Opus 5.5 (`claude-opus-5-5`), Sonnet 5.5 (`claude-sonnet-5-5`), Haiku 4.5
  (`claude-haiku-4-5-20251001`), and a custom-ID text field. The IDs are one
  constant table in `forge-entities`; updating the list is a one-place edit.
  A saved ID not in the table is shown in the custom field.
- Sidebar: group title + `+` (starts the new-session panel with that folder
  preselected), and a bottom "Settings ⌘," entry. No account or plan line:
  Forge has no login.
- The workspace page renders the overlay above everything else.

### 16.5 Sidebar markers (answers §15.11)

| State | Look |
|---|---|
| idle | hollow ring (as now) |
| running | accent ring that slowly rotates |
| needs attention | filled warning dot with a small `!` |

Distinguished by shape and motion, not colour alone. The existing `marker`
enum in `forge-entities` is unchanged; only the CSS and the widget markup move.
The animation respects `prefers-reduced-motion` (static ring).

### 16.6 Theme

`theme.css` is rewritten around tokens: one background, a sidebar a few percent
darker, muted text, one accent, one warning, one danger, radii and spacing
steps. Light and dark both via `prefers-color-scheme`. Existing class names
are kept where they still mean the same thing so widget code changes little;
new names are added for cards, the composer box, the overlay and rows.

### 16.7 Testing

Automated (TDD, each test seen failing first):

- `forge-domain`: a config without `model` loads; with `model` round-trips;
  the normaliser maps `""` and `"  "` to `None`.
- `forge-application`: `save_settings` keeps `enabled_*` untouched; a failing
  repository returns the error and stores nothing.
- `forge-features`: draft / save / failed save keeps the draft; `choose_model`
  saves at once; cancel discards; `shortcut_for(",", …)` on macOS and elsewhere;
  `start_new` passes the saved model to the registry.

Manual checklist (real window, real `~/.claude`), recorded in §16.9 like §15.11:
light and dark; all three markers visible and distinct; `⌘,` opens, Esc closes;
save a key, start a new session, it uses the new key (and an open older
session still uses the old one); pick a model, the new session runs on it;
a hand-edited `config.json` with extra `enabled_skills` survives a save;
a broken `config.json` shows the notice and Save replaces it.

### 16.8 Task order

1. Domain: `model` field + normaliser.
2. Application: `load_settings`, `save_settings`.
3. Features: settings state, shortcut, `start_new` uses the saved model.
4. Theme rewrite and shared row component.
5. Sidebar: group `+`, markers, bottom entry.
6. Settings overlay (Connection, Model) and new-session panel change.
7. Manual checklist, fixes, §16.9 "As built".

Tasks 1–3 need no window and come first; 4–6 are compile-checked only until 7,
the lesson of §15.11 (runtime problems appear only in a live window), so task 7
is budgeted for fixes.

### 16.9 As built

Plan 6 was built on branch `plan-6-2` and checked on 2026-09-30 (macOS 26, Dioxus 0.7.10). 274 workspace tests pass. The live-window checks were driven by script (`cliclick` and System Events keystrokes) with screenshots, on the real `~/.claude`; one real turn was run.

**Checked live (passed):** the overlay opens with ⌘, and closes with Esc and with ✕; ⌘, reopens it afterwards, including after Esc from inside an input; the Connection tab saves ("Saved.", Save dims again), masks the key, stores a trailing space trimmed, writes the file `0600`, and stores an emptied field as `null`; the Model tab saves a chosen or typed model at once, and the new-session panel then shows "Haiku 4.5"; a real session started from that panel ran on `claude-haiku-4-5-20251001` (read from its transcript) and answered; the idle marker is a hollow ring and the running marker a turning partial ring; a `config.json` that is not JSON shows the parse error in the overlay and Save replaces it; ⌘N works after closing the overlay.

**Not checked:** dark mode; the needs-attention marker (no permission request was triggered); the Show button; the backdrop click; the group `+`; a wrong key failing a turn and an open older session keeping its old key; a typed model ID that is not in the table; Stop, permission answers, ⌘W and Ctrl+Tab; a hand-edited `enabled_skills` surviving a save (covered by the use-case tests only). These remain for a person to walk.

**Runtime fixes:**
1. `fix: keep saves and session start alive across unmounts and restore key focus` — from the final review: saves and `start_new` use `dioxus::core::spawn_forever` so switching tab, Esc or clicking away cannot cancel them half-way.
2. `fix: give the window focus back when a view change removes the focused element` — found live: after a session starts, the composer that had focus is unmounted, focus falls to `<body>`, and every shortcut (⌘, ⌘N ⌘W Ctrl+Tab) is dead until a click. This predates Plan 6 (Plan 5 has the same unmount) but the overlay made it visible. The page now refocuses the root whenever the overlay or the active view changes, and only if nothing has focus.

**Differences from §16:** `SettingsUseCase` is one struct with `load`, `saved_model`, `save_connection`, `save_model` rather than `load_settings` / `save_settings`; a saved model that is not in the table is shown as a "Custom: …" option in the dropdown rather than in the custom field; the chat composer has no folder chip or model line (only the new-session panel shows the model); the Settings footer shows `⌘,` on every platform.

**Not fixed, on purpose (final review, Minor):** a backdrop click after a drag-select closes the overlay and drops the draft; ⌘, while open reloads and drops edits; session shortcuts still act behind the overlay and there is no focus trap; the "Saved." line is shared across tabs (seen live: it stays after switching from Connection to Model); a failed model choice leaves the dropdown on the clicked value; concurrent saves can overwrite each other; the config file is briefly created with default permissions and written non-atomically (pre-existing); a few CSS width edge cases; the Model line shows "Default model" until the first load finishes. Also: a config file that parses badly (for example a wrong type) is replaced without a backup, and unknown keys are dropped on any save.

**Observed, unexplained:** while automating, the first click on Save after typing did not register once (the second did); and ⌘V did not paste into the new-session textarea although it pasted into a Settings input. Both may be artifacts of the automation (macOS autocorrect popups were visible), so they are recorded, not classified.

## 17. Detailed Design: Skills, Plugins and MCP servers (Plan 7)

Plan 7 makes the enable/disable lists that have sat in `AppConfig` since Plan 2
do something: three settings tabs manage them, and a session started
afterwards runs with exactly the skills, plugins and MCP servers that are
switched on. It also clears two small items Plan 6 left open (§16.9).

### 17.1 Scope and decisions

Decisions (user-approved during brainstorming, 2026-09-30):

- **Toggles take effect on sessions.** Not just stored: the active lists reach
  the SDK through `start_session`. A running session keeps what it was started
  with; the tabs say so (same rule as the connection and model, §16.1).
- **Forge's toggles are the only source of truth.** Today the sidecar passes
  no options, so the SDK loads every settings source
  (`~/.claude/settings.json`, project, local) and Claude Code's own enabled
  plugins, MCP servers and hooks leak into Forge sessions. That contradicts
  §4.3. From this plan a Forge session loads only the project settings source
  and only the servers, skills and plugins Forge has enabled.
  Consequence to state plainly: user-level hooks and permission rules from
  `~/.claude/settings.json` no longer apply to Forge sessions.
- Marketplace install, editing a skill, and adding or removing an MCP server
  stay out of scope (§2).

**In scope:** protocol fields; sidecar option mapping; resolving the active
set at session start; Skills / Plugins / MCP tabs; per-tab status; ignoring
session shortcuts while the overlay is open.

**Out of scope:** the other Minor items of §16.9 (backdrop click after a drag,
⌘, dropping edits, concurrent-save overwrite, non-atomic config write, config
backup); toggling inside a running session; per-project enabled lists (state is
by name, global, as §4.1).

### 17.2 SDK facts (read from `sdk.d.ts`, `@anthropic-ai/claude-agent-sdk` 0.3.283)

- `plugins?: {type: 'local', path}[]`: plugin directories by path.
- `skills?: string[] | 'all'`: names match the `SKILL.md` `name` or directory,
  or `plugin:skill`. **Omitted means "the CLI's defaults", not "off"**, so an
  empty active list must be sent as `[]`. It is a context filter, not a sandbox.
- `mcpServers?: Record<string, McpServerConfig>` and `strictMcpConfig?: boolean`
  (only the servers passed here, ignoring `.mcp.json`, user settings and plugins).
- `settingSources?: ('user'|'project'|'local')[]`: omitted loads all; `[]` is
  isolation; `'project'` is needed for `CLAUDE.md`.

### 17.3 Protocol and session start

- `start_session` gains `skills: object` (skill name to its directory),
  `plugins: string[]` (absolute paths) and `mcp_servers: object` (name to
  config, as read). `StartSessionParams`
  in `forge-ports` gains the same three fields; the Rust and TypeScript
  protocol types and the contract test change together.
- `forge-domain` gets `ActiveExtensions { skills, plugin_paths, mcp_servers }`;
  `skills` maps each enabled skill's name to its directory.
- `forge-application` gets a use-case that builds it: load the config, list
  the definitions for the session's project through the three existing ports,
  keep the definitions whose names are in the `enabled_*` lists. An enabled
  name with no definition (deleted, or not in this project) is skipped without
  an error. A failing reader fails the session start with the existing
  notice mechanism (§15.11 item 14), so a session never silently starts with
  fewer tools than the user switched on.
- Wherever the registry starts a sidecar (new session, and the lazy start of a
  resumed one) it passes the result in `StartSessionParams`.
- Sidecar mapping (adjusted by the spike, §17.7): `settingSources: ['project']`,
  `strictMcpConfig: true`, `mcpServers`, and `plugins` = the enabled plugin paths
  plus one generated plugin. Enabled skills do **not** go to the SDK by name:
  under `['project']` a user skill is not discovered, and `['user','project']`
  leaks the user's plugins (§17.7). Instead the sidecar builds, per session, a
  temporary local plugin `forge-skills` (`.claude-plugin/plugin.json` plus
  `skills/<name>` symlinks to each enabled skill's directory), passes it in
  `plugins`, and sets `skills` to `forge-skills:<name>` for each. A skill name
  that is not a single safe path component (it contains `/`, `\`, is `.` or
  `..`, or is empty) is skipped. With no enabled skills, no plugin is generated
  and `skills` is `[]`. The temporary directory is removed when the session
  ends. Empty values are sent as `[]` and `{}`.

### 17.4 Settings tabs

- `SettingsSection` gains `Skills`, `Plugins`, `McpServers`; the navigation
  lists them under Model.
- Each tab lists the definitions as `SettingRow`s: name, one-line description,
  a switch. Skills show the `SKILL.md` description; plugins show
  `name@marketplace`; MCP servers show `command` or `url` when present (the
  config stays opaque, §12.2) and the plain name otherwise. A switch saves at
  once through the existing `Toggle*UseCase` and shows the new state from the
  returned config.
- Project-local definitions need a project: the tabs read for the active
  session's project, and for the global definitions only when there is none.
  The tab names the project it is showing.
- Each tab carries the line "Sessions that are already open keep what they
  started with; a change applies to the next new session."
- A reader failure shows its message in that tab only. An empty list shows a
  one-line empty state.

### 17.5 Small items

- While the overlay is open, `⌘N`, `⌘W` and `Ctrl+Tab` are ignored; only `⌘,`
  and Esc act. (Found live in Plan 6: ⌘W closed a session behind the overlay.)
- The "Saved." / error status becomes per section, so it does not follow the
  user to another tab.

### 17.6 Testing

Automated (TDD, each test seen failing first):

- `forge-application`: the active set keeps only enabled definitions; an
  enabled name without a definition is skipped; nothing enabled gives empty
  lists; a failing reader is an error and starts nothing.
- `forge-adapter-sidecar`: `start_session` serialises the three fields; the
  contract test with the fake sidecar carries them through.
- `sidecar` (vitest): the fields become the SDK options above; empty lists are
  still passed.
- `forge-features`: toggling saves and reflects the returned config; status is
  per section; shortcuts other than `⌘,` and Esc do nothing while open.

**Spike first (Task 1, throwaway):** with the real SDK and the real
`~/.claude`, check (a) that a user skill in `~/.claude/skills` is still
discovered with `settingSources: ['project']` and listed by `skills: [...]`,
(b) that a plugin loads from its `installPath`, (c) that both MCP config shapes
of §13.1 are accepted as `mcpServers` values, (d) that nothing from
`~/.claude/settings.json` leaks in. The mapping in §17.3 is adjusted to the
findings, and the findings are recorded in §17.7.

Manual checklist (real window, real `~/.claude`), recorded in §17.8: the three
tabs list the real definitions; a switch persists across restart; a skill
switched on is usable in a new session and unusable when off; an enabled
plugin's skill appears; an MCP server appears in the session and not when off;
an open older session is unchanged; ⌘W behind the overlay does nothing; the
status line does not follow a tab change.

### 17.7 Spike findings

2026-09-30, `@anthropic-ai/claude-agent-sdk` 0.3.283, real `~/.claude`. Skill under
test: `report-writer` (a user skill in `~/.claude/skills`), passed as `skills: ["report-writer"]`.
Observed from the `system`/`init` message (no API key needed to reach it).
"Bundled" below means skills/plugins that appear in every case, including the
fully isolated one: skills `design, design-sync, dataviz, update-config, verify, debug,
code-review, simplify, batch, fewer-permission-prompts, doctor, loop, schedule,
claude-api, run, run-skill-generator` and plugins `agents-md, telemetry`.

| case | skills | plugins | mcp_servers |
|---|---|---|---|
| baseline (no options) | 3 user skills (frontend-design, internal-comms, webapp-testing) + plugin skills (superpowers:*, frontend-design:*, anthropic-skills:*) + bundled | claude-hud, context7, feature-dev, frontend-design, playwright, superpowers, rust-analyzer-lsp + bundled | plugin:context7, plugin:playwright, 5 claude.ai connectors |
| isolated (`['project']`, strict, all empty) | bundled only | agents-md, telemetry (bundled) | none |
| enabled (`['project']`, skills, 1 plugin path, 2 MCP) | `andrej-karpathy-skills:karpathy-guidelines` + bundled; `report-writer` is NOT present | andrej-karpathy-skills + bundled | `stdio_shape` failed/dynamic, `http_shape` failed/dynamic |
| userSources (`['user','project']`, `plugins: []`) | all 13 `~/.claude/skills` incl. `report-writer`, plus superpowers:*, frontend-design:*, anthropic-skills:*, bundled | claude-hud, context7, feature-dev, frontend-design, playwright, superpowers, rust-analyzer-lsp + bundled | none |

Answers:

- (a) NOT met. Under `settingSources: ['project']` the user skill `report-writer` is
  not discovered; the `skills` option did not add it. It appears only with `'user'` in
  the sources.
- (b) Met. A plugin passed as `{type: 'local', path: installPath}` loads and its skill
  is namespaced (`andrej-karpathy-skills:karpathy-guidelines`).
- (c) Met. Both the stdio shape and the `type: 'http'` shape are accepted (status
  `failed`, source `dynamic`, as expected for fake servers).
- (d) Partly. `strictMcpConfig` with `mcpServers: {}` removes all MCP servers, and
  with `['project']` no user-level plugins or user skills leak. But the isolated case
  is not empty: 16 bundled skills and 2 bundled plugins remain (they are SDK-provided,
  not from `~/.claude`). With `['user','project']`, the user's enabled plugins (from
  `~/.claude/settings.json` `enabledPlugins`) load even though `plugins: []` was
  passed, so plugins DO leak there; MCP servers do not.

settingSources for Task 4: NOT CHOSEN. `['project']` does not discover user skills;
`['user','project']` leaks user plugins. Blocked pending a human decision (for
example: copy or symlink enabled user skills into a per-session directory the SDK reads
as project, or pass skill contents another way).

**Round 2** (same date and SDK; bundled skills and plugins are not a leak, only
things coming from `~/.claude` count). All with `strictMcpConfig: true`,
`mcpServers: {}`.

| variant | options | skills | plugins | mcp_servers |
|---|---|---|---|---|
| V1 | `['user','project']`, `skills: ['report-writer']`, `plugins: []`, `settings: {enabledPlugins: {}}` | all 13 user skills (the `skills` filter hides none), superpowers:*, frontend-design:*, anthropic-skills:*, bundled | claude-hud, context7, feature-dev, frontend-design, playwright, superpowers, rust-analyzer-lsp + bundled (LEAK) | none |
| V2 | same, with `managedSettings: {enabledPlugins: {}}` instead of `settings` | identical to V1 | identical to V1 (LEAK) | none |
| V3 | `['project']`, `skills: ['forge-user-skills:report-writer']`, `plugins: [{type:'local', path: <tmp>}]` | `forge-user-skills:report-writer` + bundled only | forge-user-skills + bundled | none |

- V1 and V2 fail: neither `settings.enabledPlugins` nor `managedSettings.enabledPlugins`
  overrides the user's enabled plugins, and the `skills` option does not hide the other
  user skills or plugin skills.
- V3 works. `<tmp>/.claude-plugin/plugin.json` = `{"name":"forge-user-skills"}` and
  `<tmp>/skills/report-writer` a symlink to `~/.claude/skills/report-writer`. The skill
  appears in init as `forge-user-skills:report-writer` (namespaced by the plugin name),
  so the symlink is followed. Nothing else from `~/.claude` appears.

**settingSources for Task 4: `['project']`.**

How user skills reach a session: Forge generates a local plugin directory
(`.claude-plugin/plugin.json` named `forge-user-skills`, plus `skills/<name>` symlinks to
each enabled `~/.claude/skills/<name>`), passes it in `plugins` as
`{type: 'local', path}` with `settingSources: ['project']`, and lists the skills as
`forge-user-skills:<name>` in `skills`. User-level hooks and rules stay off.

### 17.8 As built

Verified 2026-10-01 on macOS 26.6.2, Dioxus =0.7.10, SDK 0.3.283. The GUI was **not** driven
(no window automation, no `cargo run -p forge-app`), and no file under `~/.claude`,
`~/.claude.json` or Forge's `config.json` was written; the scripted checks below ran against the
built sidecar, the real SDK and the real `~/.claude`, using temp directories for everything
they wrote.

Tests: `cargo test --workspace` 301 passed (baseline 274); sidecar `npm test` 26 passed
(baseline 19); `npm run typecheck` clean; `npm run build` produces `sidecar/dist/index.js`.

Checks:

- **A. Suites.** Rust 301/301, sidecar 26/26, typecheck clean. Pass.
- **B. Build.** `sidecar/dist/index.js` and `dist/session.js` exist (dist is git-ignored). Pass.
- **C. Real SDK, shipped `SidecarSession` (built `dist/session.js`), real `query`, one message sent
  and shut down at `init`.** Pass.
  - One user skill (`report-writer`, real dir under `~/.claude/skills`): options were
    `settingSources ['project']`, `strictMcpConfig true`, `skills ['forge-skills:report-writer']`,
    one local plugin `/…/T/forge-skills-XXXXXX`, `mcpServers {}`. `init.skills` listed
    `forge-skills:report-writer` plus only the SDK-bundled skills (design, dataviz, update-config,
    verify, debug, code-review, simplify, batch, …); no other user skill (`frontend-design`,
    `webapp-testing`, …) and nothing from the user's enabled plugins (claude-hud, superpowers, …).
    `init.plugins` = `forge-skills`, `agents-md`, `telemetry` (the last two are SDK-bundled).
    `init.mcp_servers` = `[]`. During the session the `forge-skills-*` directory existed; after
    the session ended it was gone, and `~/.claude/skills/report-writer/SKILL.md` was intact.
  - Names `"../evil"` (pointing at `frontend-design`) plus `report-writer`: only
    `forge-skills:report-writer` was listed; nothing was created outside the temp dir;
    `frontend-design` untouched; temp dir cleaned up.
  - Nothing enabled: `skills []`, `plugins []`, no `forge-skills` plugin, no user skills in
    `init`, `mcp_servers []`, no temp dir created.
- **D. Real adapters, read-only** (a throwaway example, removed; config was a temp file):
  `list_skills(None)` 30 (for example `algorithmic-art`, `brand-guidelines`, `built-in-browser`
  from `~/.claude/skills/synced/…`), `list_plugins()` 10 (for example
  `claude-hud@claude-hud`, `context7@claude-plugins-official`), `list_mcp_servers(None)` 0
  (no global servers in `~/.claude.json`). `resolve_active` with `report-writer` and
  `andrej-karpathy-skills@karpathy-skills` enabled returned skills
  `{report-writer: ~/.claude/skills/report-writer}`, `plugin_paths` = that plugin's `installPath`
  (`~/.claude/plugins/cache/karpathy-skills/andrej-karpathy-skills/1.0.0`), `mcp_servers {}`.
  Pass.
- **E. Reader failure.** Covered by the unit test
  `live_session::tests::a_reader_failure_spawns_nothing_and_keeps_the_typed_text` (passes);
  not redone.

Differences from §17:

- §17.3 originally had the sidecar pass enabled skills by name (`skills: [names]`) with
  user-level settings. As §17.7 found, `settingSources ['project']` does not discover user
  skills and `['user','project']` leaks the user's enabled plugins, so the sidecar generates a
  per-session local plugin named `forge-skills` (a temp directory of symlinks to each enabled
  skill directory, removed when the session ends) and lists the skills as `forge-skills:<name>`.
  The spike's throwaway plugin was called `forge-user-skills`; the plan chose `forge-skills`.
- A skill name that is not one safe path component (`..`, contains `/`, `\`, NUL, or empty) is
  skipped by the sidecar.

**Not checked (for a person to walk)** in the real window:

- The Skills, Plugins and MCP servers tabs list the real definitions (project or global), an
  empty kind says "Nothing found.".
- A switch persists across an app restart (`enabled_*` in `config.json`).
- A skill / plugin / MCP server is usable in a real conversation in a new session and not when
  off (also a project-local skill); the `forge-skills-*` directory exists while a session is open
  and is gone after closing it, from the app.
- An older open session is unchanged by toggling.
- With the overlay open, `⌘W`, `⌘N`, `Ctrl+Tab` do nothing to the sessions behind it while `Esc`
  and `⌘,` still work.
- "Saved." does not follow a tab change (Connection, Model, Skills).
- A broken reader is shown in the window with the typed text kept in the message box and no
  process left running.
- A skill name present both user-level and project-level shows once.
- A switched-off project skill (`<project>/.claude/skills/<name>`) is not usable.
- A plugin enabled only in `<project>/.claude/settings.json` does not load (if it does, that is
  project behavior to document).
- After closing a session in the app the `forge-skills-*` temp dir is gone.
- An enabled plugin whose `installPath` no longer exists (SDK behavior unverified).

Deferred Minor findings that matter to a user: the catalog is not reset when the overlay opens
(a stale list shows until the listing returns); overlapping quick toggles can race; `reload()`
can overwrite `saved` after an in-flight toggle; a rejecting `cleanup()` can mask the session
outcome.

Found while verifying (none in shipped code): the sidecar tests
(`sidecar/test/session.test.ts`, line 25) create `forge-test-skill-*` directories with
`mkdtempSync` and never remove them, leaving three per `npm test` run in the system temp dir.
The normal close path now shuts the sidecar down gracefully: `SidecarProcess::shutdown` sends
the shutdown command and waits (bounded, 3 s) for the process to exit by itself, so `cleanup()`
runs and the `forge-skills-*` directory is removed; `kill_on_drop` remains the backstop when the
wait times out. A crash or SIGKILL of the whole app can still leave a `forge-skills-*` directory
(only symlinks) in the system temp dir; sweeping directories of dead pids at startup is an
accepted follow-up.

### 17.9 Task order

1. Spike (§17.6), findings into §17.7.
2. Protocol: `StartSessionParams`, Rust and TypeScript wire types, contract test.
3. Sidecar: option mapping.
4. Domain and application: `ActiveExtensions` and its use-case.
5. Registry and `Workspace` pass it at session start.
6. Features: sections, per-section status, overlay-aware shortcuts.
7. Tabs in the overlay.
8. Manual checklist, fixes, §17.8.

Tasks 2–6 need no window. As in §16.8, task 8 is budgeted for fixes.

## 18. Detailed Design: chat rendering (Plan 8)

Plan 8 closes three Tier 1 items of §7 that are pure presentation: markdown,
syntax-highlighted code blocks, and a diff viewer for Edit/Write tool calls.
It touches no port and no adapter. Plan 9 (§19) covers the rest of Tier 1.

### 18.1 Scope and decisions

- Markdown and highlighting are rendered in Rust (`pulldown-cmark` and
  `syntect`), not in the webview: pure functions that unit tests can reach,
  and no JavaScript bundle to ship offline.
- Markdown applies to **assistant** messages only. User messages stay plain text.
- **Out of scope:** copy buttons, line numbers in diffs, side-by-side diffs,
  rendering Bash/Read output as markdown, a theme chooser, incremental
  (cached) re-rendering while streaming.

### 18.2 Markdown (`forge-entities`)

`render_markdown(&str) -> String` converts CommonMark plus tables and
strikethrough to HTML with `pulldown-cmark`.

- **Raw HTML is never passed through.** `Html` and `InlineHtml` events are
  emitted as escaped text, so model output cannot become live markup in the
  webview.
- **Link schemes:** only `http`, `https` and `mailto` keep their `href`; any
  other scheme renders as plain text. Images are never loaded: an image
  renders as its alt text (a remote image URL in model output would be a
  tracking channel).
- A code block over 20 000 bytes is shown escaped but not highlighted
  (the whole message is re-rendered on every streamed token).
- A message is re-parsed in full on every update. An unterminated code fence
  is treated by the parser as code to the end of the text, which is the
  right behavior mid-stream.
- `MessageBubble` inserts the result with `dangerous_inner_html` for the
  assistant role only.

### 18.3 Syntax highlighting

Code fences go through `syntect` with `ClassedHTMLGenerator`, which emits CSS
classes instead of inline colors. The stylesheet carries a light and a dark
palette selected by `prefers-color-scheme`, matching §16.6. The fence's
language tag picks the syntax; an unknown or missing tag renders as escaped,
unhighlighted code. Only syntect's pure-Rust regex backend is enabled; no C
dependency is added.

### 18.4 Diff viewer

`diff_for_tool_call(&ToolCall) -> Option<FileDiff>` in `forge-entities`,
with line diffs from the `similar` crate.

- `Edit`: `old_string` against `new_string`.
- `MultiEdit`: each edit in order, as separate hunks.
- `Write`: every line of `content` as an addition. The previous file content
  is not part of the tool input, so the header says "new content" and does not
  claim whether the file existed.
- Any other tool, or input missing the expected fields, gives `None`.
- No line numbers: the input does not say where in the file the edit lands.
  Lines carry a `+` / `-` / space prefix and a background color.
- A diff shows at most 300 lines in total; the rest is summarised as
  "… N more lines". The `+N −M` counts in the header are the full totals.
- The pure function takes a tool name and an input (`diff_for_tool`), with
  a `ToolCall` wrapper, because a permission request has no `ToolCall`.

`ToolCallLine` keeps its collapsed one-line form. For calls that have a diff,
clicking the line expands it (state is local to the line). `PermissionCard`
shows the same diff component for pending Edit/Write requests, so the content
can be read before allowing.

### 18.5 Testing

Unit tests, no rendering: markdown conversion for each supported construct,
HTML escaping, link-scheme filtering, unterminated fences, language
detection with an unknown tag, and diff output for Edit, MultiEdit, Write
and malformed input. Visual checks go in the manual checklist (§8).

### 18.6 Task order

1. Dependencies (`pulldown-cmark`, `syntect`, `similar`), confirming the
   build stays free of C dependencies, and `highlight_code`.
2. `render_markdown` with escaping, link filtering and highlighted fences.
3. `diff_for_tool` and the `FileDiff` model.
4. `DiffView`, the expandable `ToolCallLine`, the diff in `PermissionCard`.
5. `MessageBubble` markdown rendering and the stylesheet (both palettes).
6. Manual checklist, fixes, §18.7 "As built".

## 19. Detailed Design: session rename/delete and folder open (Plan 9)

Plan 9 finishes Tier 1: session rename and delete (shared with Claude Code on
disk) and a native folder-open dialog.

### 19.1 Scope and decisions

- Rename appends a `custom-title` entry to the session's `.jsonl`; delete
  moves the file to the OS trash. Neither is permanent from the user's side.
- A session open in Forge cannot be deleted. It can be renamed.
- **Out of scope:** trash restore UI, bulk delete, deleting a project,
  undo for rename, file watching (the list is re-read after each action),
  the §15.12 `cwd` collision.

### 19.2 Ports and adapters

`SessionRepositoryPort` gains:

- `rename_session(project_path, id, title)`
- `delete_session(project_path, id)`

with new `SessionRepositoryError` variants `RenameFailed` and `DeleteFailed`
(`NotFound` is reused). `forge-adapter-claude-fs` implements them.

- **Rename:** append `{"type":"custom-title","customTitle":<title>,"sessionId":<id>}`
  as one line. The reader already takes the last title entry (`title_from_value`),
  so reading is unchanged. If the file does not end with a newline, one is
  written first.
- **Delete:** the `.jsonl` goes to the trash through the `trash` crate,
  behind a thin wrapper so tests never touch a real trash. Whether a
  same-named sibling directory (sub-agent data) exists and should go with it
  is settled by the spike.

`FolderPickerPort::pick_folder() -> Option<PathBuf>` is a new port in
`forge-ports`; its adapter uses `rfd`.

### 19.3 Spike (first task)

Findings go to §19.7, as in §15 and §17.

1. What Claude Code itself writes when a session is renamed (field names,
   whether a `sessionId` is present), checked against a session renamed in
   Claude Code, not assumed.
2. Whether a session has sibling files or directories besides its `.jsonl`.
3. That `rfd` opens a folder dialog from inside the Dioxus desktop window
   without blocking the event loop.

### 19.4 Application and features

- Title validation: trimmed, non-empty, at most 200 characters.
- Delete is refused with a clear message when the session is open in Forge
  ("Close the session first (⌘W) to delete it.").
- Renaming a live session also updates its title in the live view.
- After either action, only that project's session list is reloaded.

### 19.5 UI

- **Sidebar:** hovering a session shows a `⋯` menu with "Rename" and "Delete".
  Rename turns the title into an input in place (Enter confirms, Esc cancels).
  Delete asks for confirmation ("Moves the session to the trash").
- **New session panel:** an "Open folder…" button beside the Folder
  dropdown. The chosen path goes to `set_new_folder`; a folder already in
  the list selects that project; a path that does not exist is rejected.
- Both keep working with the overlay-aware shortcuts of §17.

### 19.6 Testing

Adapter tests use temp-dir fixtures: after a rename, `list_sessions` and
`load_session` return the new title; newline handling; `NotFound` for a
missing session; delete through a fake trash wrapper. Feature logic (title
validation, refusing live sessions, reload scope) uses mock ports. The
dialog itself is manual.

### 19.7 Spike findings

Run 2026-10-01 by a non-interactive agent in the `plan-9` worktree. It could
not run an interactive `claude` session or click a native dialog, so those
answers are marked "Not checked" with what a person must do. No finding
contradicts a Decision of the Plan 9 plan.

- **Baseline — Checked.** `cargo test --workspace` reports 344 passing
  tests (the plan assumed 341).
- **(a) What `/rename` writes — Partly checked.** Not run live (needs an
  interactive `claude`). Real data in `~/.claude/projects` has two sessions
  (both in `-Users-james-Developer-mywork`) with `custom-title` lines, almost
  certainly written by `/rename`: exactly
  `{"type":"custom-title","customTitle":"<title>","sessionId":"<session id>"}`,
  field order `type`, `customTitle`, `sessionId`, `sessionId` present. Claude
  Code repeats the same line several times in one file (4 and 3 times, at
  lines 30/38/53/74 and 20/33/41), interleaved with other entries, so the
  reader must accept duplicates and take the last. `ai-title` lines are
  `{"type":"ai-title","aiTitle":"<title>","sessionId":"<id>"}` (98 files have
  one). No file holds both kinds, so the order of `ai-title` after
  `custom-title` is **Not checked** (Decision 2 stands on the reader rule,
  not on observed order). Both files ended with a newline, but that last line
  was a normal entry, so "file ended without a newline before a rename" is
  **Not checked**; Task 2 handles it defensively anyway. A person should: in
  a scratch directory run `claude`, send a message, `/rename spike-title`,
  exit, run `grep -n '"custom-title"\|"ai-title"'` and `tail -c 200 | od -c`
  on the file, then send one more message and exit and look again.
- **(b)/(c) Siblings of a session's `.jsonl` — Checked (read-only).** 135
  `.jsonl` files; only 7 have a same-named `<session-id>/` directory (other
  directories are `memory/`). Contents seen: `tool-results/*.txt` (large tool
  output) and `subagents/agent-*.jsonl` + `agent-*.meta.json` (sub-agent
  transcripts). All of it belongs only to that session, so trashing the
  `.jsonl` and the same-named sibling directory together is right, and the
  sibling lines stay in Task 3. Other places carrying the session id:
  `~/.claude/file-history/<session-id>/` (exists for some sessions, absent
  for most) and `~/.claude/session-env/<session-id>/` (empty directories).
  `todos`, `shell-snapshots` (named by timestamp), `tasks` (`session-*`) and
  `sessions` (named by pid) do not carry it. Decision: trash only the
  `.jsonl` and the sibling directory; leave `file-history` and `session-env`
  (Claude Code's own housekeeping, not this feature's to touch).
- **(d) Folder dialog in the Dioxus window — Partly checked.** `rfd = "0.17"`
  resolves (0.17.2) and builds with default features in a throwaway crate on
  macOS. Dialog opening, window responsiveness while it is open, the printed
  path, `None` on cancel and no hang are **Not checked**: a person must add
  the button from the brief to `NewSessionComposer`, `cargo run -p
  forge-app`, click it, pick a folder, cancel once, type and resize while it
  is open. Task 8 depends on this.
- **(e) `trash` — Partly checked.** `trash = "5"`: `trash::delete` on an
  existing temp file returned `Ok(())`, the file was gone, and the call
  blocked for about 3.7 s (on macOS it drives Finder through AppleScript), so
  it must run off the UI thread (`spawn_blocking`). A path that does not
  exist returns `Err(Os { code: 1, .. })` with an AppleScript/Finder message
  (localized, not a typed not-found), so check existence ourselves before
  calling and map that to `NotFound`. That the file lands in `~/.Trash` is
  **Not checked**: the sandbox denies listing `~/.Trash`; a person should
  confirm in Finder.
- **(f) Rename changes the modified time — Checked.** On a scratch copy,
  `stat -f %m` went from 1790857716 to 1790857717 after one `echo '{}' >>`;
  a rename therefore moves the session up the list (Decision 5).

### 19.8 Task order

1. Spike (§19.3), findings into §19.7.
2. Port methods and error variants; mocks.
3. Adapter rename.
4. Adapter delete behind the trash wrapper.
5. `FolderPickerPort` and its `rfd` adapter.
6. Application/feature logic: validation, live-session guard, reload.
7. Sidebar menu, in-place rename and delete confirmation.
8. Open-folder button in the new session panel.
9. Manual checklist, fixes, "As built".

### 19.9 As built

Plan 9 was built on branch `plan-9` and checked on 2026-10-01 by a
non-interactive agent. `cargo test --workspace` passes with 388 tests (344
before Plan 9, after the spike run; the old plan assumed 341). The sidecar was
not touched by Plan 9 and `sidecar/node_modules` is absent, so `npm test` and
`npm run typecheck` were not run (sidecar untouched, not run).

**Checked (passed):** the automated tests only.

**Not checked (needs a person):** the whole window walk. A person must run
`cd sidecar && npm run build`, then `cargo run -p forge-app`, on a scratch
project, and:

1. Hover a past session: a `⋯` appears; clicking it opens the menu and does
   not open the session; clicking elsewhere does not leave a stuck menu.
2. Rename: type a new name, Enter, so the title changes, the list reloads for
   that project only (other groups do not flicker), the renamed session moves
   to the top. Esc cancels. Clicking away cancels. An unchanged or blank name
   does nothing.
3. Korean IME: in the rename input, type a Korean word and press Enter to
   commit the composition; the rename is not submitted; a second Enter
   submits. (This also settles the parked WebKit IME Enter concern.)
4. Rename an open session: the sidebar and the live view show the new title at
   once; send another message, wait for the turn to finish, the title is still
   the new one. Open the same project in `claude`, the new title is shown
   there. Also rename a session that already has a custom title (set earlier
   by `/rename` or a Forge rename), send a message, then reload/reopen it: the
   new title must still show; Claude Code re-appends `custom-title` lines each
   turn (§19.7 a), so an older title could win again. This is an unverified
   risk; possible fixes are to re-append the title after each turn, or to
   refuse rename while a sidecar is attached.
5. A 201-character title: the notice says the limit; nothing changes on disk.
6. Delete a past session: confirmation reads "Moves the session to the
   trash"; Cancel keeps it; Delete removes it from the list and it is in the
   Trash (Finder) with its sibling directory if it had one.
7. An open session's menu has Delete disabled with the "Close the session
   first" note; after ⌘W it can be deleted.
8. Shortcuts still work while a rename input or the menu is open (⌘N,
   Ctrl+Tab); ⌘W while renaming closes the session as before and does not
   leave a stuck input. After a rename or a delete confirmation, shortcuts
   still work without clicking first (focus returns to the workspace).
9. "Open folder…": the dialog opens, the window stays responsive; cancel
   changes nothing; picking a listed folder selects it; picking a new folder
   adds it to the dropdown and selects it; Start works there and the project
   then shows in the sidebar; picking while the dialog is already open
   (double click) opens one dialog.
10. With no Claude Code history at all (set `HOME` to an empty temp directory
    for one run), "Open folder…" is usable and Start becomes enabled after a
    folder is chosen.

Also not checked: switching to another app while the rename input is open
cancels the rename (blur), so confirm that is acceptable; the Open-folder
button stays disabled if the dialog never returns, and the dialog has no
parent window (app-modal on macOS), so confirm that too.

No fixes were made from the walk, since nothing was walked. The §19.7 items still marked
"Not checked" (real `/rename` output, `~/.Trash`, the `rfd` dialog on the
main thread) are covered by the same walk.

**Differences from §19:**

1. Delete is refused for every session open in Forge, not only when "its
   sidecar is running" (message: "Close the session first (⌘W) to delete
   it."). §19.1 and §19.4 are corrected.
2. A `custom-title` always beats an `ai-title` in the reader; §19.2 said "the
   last title entry".
3. A folder chosen in the dialog that is not in the list is added to the
   dropdown for this run only.
4. Delete confirmation is inline in the sidebar row, not a modal.
5. Renaming moves the session to the top of its project's list, because the
   file's modified time changes (§19.7 f).
6. `rename_session` takes the title as `&str`; delete also trashes the
   same-named sibling directory (`<id>/`, sub-agent data), as settled by the
   spike (§19.7).

**Trash on macOS:** the trash now uses `NSFileManager` (no Finder or
AppleScript, so no Automation permission prompt and no multi-second delay).
Not checked: whether it works in a bundled app and that the item lands in
`~/.Trash`.

**Final review fixes applied:** (1) the rename input stops propagation only for
Enter and Escape, so app shortcuts still arrive; (2) session ids starting with
`.` are refused (a `..jsonl` file would have trashed the project directory);
(4) leaving the rename or delete-confirm mode returns focus to the workspace;
(5) the macOS trash uses `NSFileManager`; (6) a delete that trashed the
transcript but not its folder says so, and the project list reloads on a
failed delete. The "no test for the partial failure" and "message does not say
the transcript already moved" items under Task 3 below are now fixed.

**Known limitation:** `SessionRegistry::delete` checks that the session is not
open and then awaits the delete. A session opened in that window can leave a
view over a trashed transcript: a load or resume error, no corruption.

**Deferred minors:**

- Task 1: "Partly checked" wording in §19.7; the intro says "almost certainly
  written by /rename".
- Task 2: title lines flagged `isSidechain`/`isMeta` are now counted (none seen
  in real data); the newline check and the append are not atomic; the
  id-refusal test asserts only `NotFound`; the adapter writes empty titles
  as-is (validated in the registry).
- Task 3: a sibling symlink to a directory would be trashed (metadata follows
  links); no test for the partial failure where the transcript is trashed but
  the sibling fails, and the message does not say the transcript already
  moved; a duplicate `Arc` import in tests.
- Task 4: the failed-rename test asserts after only `yield_now` (weak);
  rename and delete do not cross-check the project path against the open
  handle. Ruling: the delete check-then-await race is accepted as written
  (see Known limitation).
- Task 5: `rfd` `AsyncFileDialog` main-thread behaviour on macOS is unverified.
- Task 6: `choose_folder` normalisation is lexical only (`..`, symlinks,
  case-insensitive filesystems may produce duplicates); `reload_project` keeps
  `last_active`; import order in `main.rs`.
- Task 7: several row menus can be open at once (no outside-click or Esc
  close); the controlled input value mid-IME may reset the composition;
  `ConfirmingDelete` has no Esc; Delete on a row without a `session_id` is a
  silent no-op (gated by `actions()`); `rename_submit` is `pub` rather than
  `pub(crate)`; the menu on the last row may scroll-clip.
- Parked: the WebKit/Safari IME concern that the Enter committing a Korean
  composition may arrive with `isComposing=false` (keyCode 229). The guard is
  the same as the app's other inputs and Dioxus exposes no keyCode accessor;
  checklist item 3 settles it. If it fails, the fix is composition-state
  tracking in all three inputs.
- Rulings: the Task 1 spike and this Task 9 checklist were recorded as "Not
  checked (needs a person)" because a subagent cannot run interactive
  `claude` or a GUI dialog, so UI bugs may surface when the user runs the app.

### 18.7 As built

Plan 8 was built on branch `plan-8` and checked on 2026-10-01 (macOS 26, Dioxus 0.7.10). 341 workspace tests pass (306 before). The build has no C dependency (`cargo tree` shows no `onig` or `-sys` crate under `forge-entities`).

**Checked (passed):** the real `render_markdown` output and the real `theme.css`, served as a static page and screenshotted in a browser in light and in dark mode: headings, lists, table, strikethrough, inline code, links, highlighted Rust and Python fences, the unhighlighted TypeScript fence, the block-quote, a raw `<script>` line shown as text, and the diff markup (context, removed, added, header counts, the collapsed `Read` line next to an expanded `Edit` line). The markup in that page was written by hand to mirror `DiffView`; the Dioxus components themselves were compiled and unit-tested for their pure helpers but not run.

**Not checked (needs a person):** the live window walk of the plan's Task 6 checklist — the sidecar is not built in this worktree and the walk needs real Claude turns. Specifically: clicking a link opens the system browser without navigating the window; the expand/collapse of an Edit/Write line by mouse and keyboard; the diff inside a real permission card; resuming an older session with Edit/Write calls; the 300-line cut on a real long `Write`; streaming a half-written code block; user messages shown verbatim.

**Differences from §18:** none beyond those already written into §18.2, §18.4 and §18.6 when the plan was made (images render as alt text; blocks over 20 000 bytes are not highlighted (the final review measured 280 ms per render at 91 KB, paid once per streamed token); line diffs have a 100 ms deadline; a diff inside a permission card is at most 40vh tall and scrolls; a diff is cut at 300 lines; `diff_for_tool` takes a name and an input; highlighting precedes markdown in the task order). A line that begins with raw HTML (for example `<script>…`) is an HTML block in CommonMark, so the rest of that paragraph is shown as literal text rather than parsed as markdown; it is escaped, so this is safe.

**Final review (Plan 8):** no XSS path was found in about 30 hostile inputs. Four Important findings were fixed, each with a failing test first: a long diff in the permission card pushed the buttons off screen (now capped at 40vh and scrolls); highlighting up to 100 KB on every streamed token froze the window (cap lowered to 20 000 bytes); an unrelated 10 000-line Edit took 10 s to diff in a debug build (100 ms deadline); the dark-palette stylesheet test could not fail (now checks the definitions inside the dark block). Deferred minors: an email autolink `<a@b.c>` loses its link; escaped raw HTML blocks collapse their line breaks and can touch the preceding word; whitespace-only assistant content leaves an empty wrapper; an empty `MultiEdit` or empty `Write` still expands into a header-only box; the tool-call button may differ slightly in line height from the plain lines (not checked visually).

**Not fixed, on purpose:** TypeScript and TOML fences are not highlighted (the bundled syntax set has neither); the whole message is re-parsed on every streamed update.

## 20. Detailed Design: session usage display (Plan 10)

Plan 10 shows how much a session has used: a context ring next to the message
box, and a popover with session totals and an estimated cost. Slash command
autocomplete is Plan 11 (decided in the Plan 10 brainstorm: it will list every
command the SDK reports in `system/init.slash_commands`).

### 20.1 Scope and decisions

- **Placement.** A ring next to the message box, like the desktop app's usage
  ring (the desktop docs: "click the usage ring next to the model picker").
  Forge has no model picker there, so the ring sits beside `SendMessageBox`.
  The popover shows context window use, session totals, an estimated cost and
  a per-model list. Plan usage (subscription limits) is shared by all Claude
  Code surfaces and Forge cannot read it; it is out of scope.
- **History.** Totals include usage from before the session was opened in
  this run: `load_session` sums the `.jsonl` file. A resumed session carries on
  from that sum.
- **One cost path.** Cost is always computed in Rust from token counts and one
  price table, for live turns and for restored history alike, so the number
  does not jump on resume. The SDK's `total_cost_usd` is not used. A model that
  is not in the table has no cost (`—`); tokens are still shown. The cost is
  labelled an estimate at list price, as the CLI's `/usage` does.
- **Web searches are priced.** Each search costs $0.01 (the pricing page: "$10 per
  1,000 searches", read 2026-10-01), on top of the tokens and the same for every
  model. Web fetch has no charge of its own, and code execution is billed by
  time, not tokens, so neither is counted. The `WebSearch` tool runs its own
  request, so its searches are **not** in the main loop's `usage`; they are read
  from the tool's result (§20.3, §20.4, §20.10 item 5).
- **Out of scope:** plan usage and usage-credit spend, a usage column in the
  sidebar, sub-agent files (`subagents/`) in the restored sum, user-set price
  overrides, budgets or limits (`maxBudgetUsd`), `modelPricing`-style
  contracted rates, history charts, slash commands (Plan 11).

### 20.2 Domain (`forge-domain`)

New `usage.rs`, no I/O:

- `TokenUsage { input, output, cache_read, cache_write_5m, cache_write_1h,
  web_search_requests }` (all `u64`), with `+` and `+=`. A cache write with no 5m/1h split counts as
  5m (the cheaper rate would understate; the spike confirms which side to
  err on).
- `SessionUsage { by_model: BTreeMap<String, TokenUsage> }` with `add(model, usage)`
  and `total() -> TokenUsage`.
- `PriceTable` and `estimate_cost(model, &TokenUsage) -> Option<Cost>`.
  `Cost` is integer micro-dollars (no floats in sums). The table is data in
  `usage.rs`, matched on the exact model id or the id plus a `-YYYYMMDD`
  suffix (`claude-sonnet-4-6` matches `claude-sonnet-4-6-20260101`, not
  `claude-sonnet-4-6-1`; final review: a longer id that only starts with a row's id must
  not borrow that row's rates, it has no cost until it gets a row). The numbers come from Anthropic's published
  pricing page and each row names its source and the date it was read. An
  unmatched model gives `None`.
- `SessionUsage::cost() -> CostSummary { total: Cost, unpriced_models: Vec<String> }`:
  the sum over priced models plus the names that were left out.

`Session` gains `usage: SessionUsage`.

### 20.3 Restoring history (`forge-adapter-claude-fs`)

`load_session` reads, for each `assistant` line, `message.id`, `message.model`
and `message.usage` (`input_tokens`, `output_tokens`, `cache_read_input_tokens`,
`cache_creation.ephemeral_5m_input_tokens` / `ephemeral_1h_input_tokens`,
falling back to `cache_creation_input_tokens`).

A real file repeats one `message.id` across several lines (one per content
block); in the sample checked for this design, 4 usage lines were 2 messages.
Usage is therefore kept **once per `message.id`, the last line wins**. Lines
without an id are counted once each. A line whose usage cannot be read is
skipped; it never fails the load. Synthetic or empty models are ignored.

Web searches are not in `message.usage` (§20.10 item 5). A `user` line whose
`toolUseResult.searchCount` is a positive number (the `WebSearch` tool's output;
a nested `toolStats.searchCount` is not one) adds that many searches to the model
of the closest earlier assistant message, once per `tool_use_id` (lines without
one count once each). `message.usage.server_tool_use.web_search_requests` is added
too, for searches the model ran server-side in the main loop. Sidechain lines and
a result with no earlier assistant message are ignored.

### 20.4 Sidecar and ports

`turn_complete` becomes
`{ type, usage: { model: { input, output, cache_read, cache_write_5m, cache_write_1h, web_search_requests } } }`
and carries only what this turn **added**. The numbers come from the `result`
message's `usage` (main agent loop only, per turn, with the `cache_creation`
5m/1h split), attributed to the model of the turn's main-loop assistant
messages (`message.model`, `parent_tool_use_id` empty). `result.modelUsage` is
**not** used (§20.10: it is a running total, it includes helper models such as
the title writer and sub-agents, and after a resume its first value already holds
the earlier turns). A turn with no main-loop assistant message sends an empty
`usage`. `web_search_requests` is the sum of `result.usage.server_tool_use.web_search_requests`
and the `searchCount` of the turn's main-loop `WebSearch` tool results (the user
message's `tool_use_result.searchCount`; sub-agent results are ignored). The old
`input_tokens` / `output_tokens` fields are removed (the only
consumer is `forge-adapter-sidecar`, changed in the same plan; protocol tests
are updated with it).

A new event `context_usage { total_tokens, max_tokens }` is sent after each
`turn_complete`: the sidecar calls `getContextUsage()` on the live `Query`.
If the call throws or takes over 3 s it sends nothing; the turn is not
affected. `SidecarEvent` in `forge-ports` gets `TurnComplete { usage }` and
`ContextUsage { total_tokens, max_tokens }`.

### 20.5 Application and features

- `LiveSession` holds `usage: SessionUsage` (seeded from the loaded `Session`
  when a session is resumed) and `context: Option<ContextUsage>`.
  `SessionUpdate::TurnCompleted` (today dropped at `live_session_actor.rs`)
  adds the turn's usage; a new `SessionUpdate::ContextUsage` sets `context`.
  `LiveSessionView` exposes both.
- `forge-entities` gets `usage_view.rs`: `UsageView` and `format_tokens`
  (`999`, `1.2k`, `3.4M`), `format_cost` (`<$0.01`, then two decimals) and
  `ring_level(percent)` (normal below 70, warn from 70, danger from 90).
  Entities does not depend on `forge-application`; `forge-features` maps the
  domain values into `UsageView`.

### 20.6 UI (`forge-widgets`)

New `usage_ring.rs`, placed beside `SendMessageBox` in `ChatWindow`.

- **Ring:** an SVG arc for the context percentage, coloured by `ring_level`;
  the percentage is also the tooltip text (colour is not the only signal).
  With no context value yet it is an empty ring with `—`.
- **Popover:** opens on click, closes on outside click and on Esc. Rows:
  context (`38% · 76.2k / 200k tokens`); session totals (input, output, cache
  read, cache write, and web searches when there were any); `Estimated cost $0.55` with a one-line note that it is
  list price and may differ from the bill; one line per model. A model without
  a price shows `—` and the total says `excludes <model>`.
- Both colour schemes use the existing tokens in `theme.css`; the dark-palette
  stylesheet test is extended to the new classes and must be able to fail.

### 20.7 Spike (first task)

Run the real sidecar with a real SDK (0.3.283) and answer, before any code
depends on them:

1. Are `result.usage` and `result.modelUsage` per turn or a running total for
   the session, with streaming input and after a resume?
2. Does `Query.getContextUsage()` work on the live streaming `Query`, and what
   are its latency and its value right after a resume with no turn yet?
3. Does a real `.jsonl` split cache writes into `ephemeral_5m` / `ephemeral_1h`
   consistently, and does summing it per `message.id` match the sum of the
   SDK's per-turn usage for the same session (the check that restore is
   right)?
4. Does a custom `base_url` change the model ids that appear in usage?

Findings are written into §20.9 and the plan's affected tasks are changed
before they start.

### 20.8 Testing

Domain: cost per model, 5m vs 1h cache writes, prefix match, unknown model,
sum with a partly unpriced set. Adapter: duplicate `message.id`, last line
wins, no id, unreadable line, no usage, synthetic model, a real fixture taken
in the spike. Sidecar (vitest): `turn_complete` carries deltas for per-turn
and for running-total inputs, `context_usage` is sent after `turn_complete`,
a failing or slow `getContextUsage` sends nothing. Application: a resume
seeds the totals, a turn adds to them, a `ContextUsage` update replaces the
value. Entities: formatting boundaries. Widgets: ring level, empty state,
popover content, unpriced model.

### 20.9 Task order

1. Spike (§20.7), findings recorded.
2. `forge-domain` usage types, price table, cost.
3. `forge-adapter-claude-fs` restore with `message.id` de-duplication.
4. Sidecar: `turn_complete` deltas and `context_usage`; `forge-ports` and
   `forge-adapter-sidecar` changes.
5. `forge-application`: seeding, `TurnCompleted`, `ContextUsage`.
6. `forge-entities` formatting; `forge-features` mapping.
7. `forge-widgets` ring and popover; theme and dark-palette test.
8. Walk-through in the real window (light and dark, a resumed session, an
   unpriced model); final review; as-built notes.

### 20.10 Spike findings

Run on 2026-10-01 with Agent SDK 0.3.283, Claude Code 2.1.286, a Claude login (no API key), model `claude-opus-5-5`, project `/tmp/forge-spike`: two turns ("write about 200 words", then "reply: ok"), then a resume with no prompt, then a resume with one turn. The driver was a throwaway script around the real `SidecarSession` and is not kept.

1. **`result.usage` is per turn; `result.modelUsage` is a running total.** Turn 1: `output_tokens` 459; turn 2: 4; resumed turn: 3 (a one-word reply each time). `usage` has `input_tokens`, `output_tokens`, `cache_read_input_tokens`, `cache_creation_input_tokens` and `cache_creation.{ephemeral_5m,ephemeral_1h}_input_tokens`. `modelUsage` after the resumed turn held opus `outputTokens` 466 (= 459 + 4 + 3) and `cacheReadInputTokens` 22379, so a resumed session's first `modelUsage` already carries the earlier turns, and it also listed `claude-haiku-4-5-20251001` (901 in / 11 out, an internal helper) that no assistant message used. The SDK's type notes say the same ("cumulative across turns in streaming-input sessions"; "a resumed session continues from the totals its transcript saved"). Decision: Forge uses `result.usage` and the main-loop model (spec §20.4), not `modelUsage`.
2. **`Query.getContextUsage()` works on the live streaming query.** Fields `totalTokens`, `maxTokens`, `rawMaxTokens`, `percentage`, `model`, `categories`. First call about 850 ms, later calls 9 ms (11 393 of 1 000 000 tokens after turn 1; 11 440 after turn 2). It also answered on a resumed session before any turn (880 ms, 11 440 / 1 000 000). Forge still starts the sidecar on the first send, so the ring stays empty until the first turn finishes; showing the value for an opened-but-idle session would need a sidecar at open time and is left for later. The 3 s limit of §20.4 stands.
3. **Summing the session file per `message.id` matches the SDK exactly.** File: input 6, output 466, cache read 22 379, cache write 11 479, against the sum of the three `result.usage` values: 6, 466, 22 379, 11 479. The first message was written on **two** lines (so de-duplication is needed) and its last line carried the final `output_tokens` (459; the first line was a partial). This login uses **1-hour** cache writes: all 11 479 were `ephemeral_1h`, and `result.usage.cache_creation` has the same split, so live and restored cost agree.
4. **Custom `base_url`: not tested** (no gateway at hand). The model id in usage is the `message.model` the endpoint returns; an id that is not in the price table shows tokens with no cost (§20.1).
5. **Web searches are not in the main loop's `usage`.** A real turn that called `WebSearch` once reported `result.usage.server_tool_use.web_search_requests: 0` (the session file's assistant lines say the same). The search runs as its own request inside the tool; its count is in the tool result: the session file's `user` line has `toolUseResult.searchCount: 1`, and the SDK's `WebSearchOutput` documents `searchCount` ("Number of web searches performed"), delivered live as the user message's `tool_use_result`. After this change the same turn gave `web_search_requests: 1` live and `1` restored, and every token field matched too (6 / 506 / 32 080 / 0 / 2 764).

### 20.11 As built

Plan 10 was built on branch `plan-10` and checked on 2026-10-01 (macOS 26, Dioxus 0.7.10, Agent SDK 0.3.283). 454 Rust workspace tests pass (390 before) and 60 sidecar tests pass (38 before).

**Checked (passed):**
- Real sidecar and real SDK, driven through `forge-adapter-sidecar` for one turn: `TurnComplete { usage: {"claude-opus-5-5": input 2, output 4, cache_read 9579, cache_write_5m 0, cache_write_1h 1363} }` and `ContextUsage { total_tokens: 10944, max_tokens: 1000000 }` arrived and parsed.
- The same session file read back through `ClaudeSessionAdapter::load_session` gave exactly the same numbers (2 / 4 / 9579 / 0 / 1363), so restore and live agree, including the 1-hour cache write split.
- The real `theme.css` with the ring (normal, warn, danger, empty) and the open popover (an unpriced model, the "excludes" note) served as a static page and screenshotted in light and in dark mode: both are readable, and the three ring levels are told apart by colour and by the arc length.

**Not checked (needs a person):** the live window. The widget code compiles and its pure parts are tested, but nobody has clicked it in the running app: opening and closing the popover with the ring, with Esc and with a click outside; the popover staying inside a narrow window; a resumed session in the sidebar showing its restored totals before the first send and an empty ring; a custom model id showing `—` and "excludes <id>"; and the custom `base_url` case (spike item 4).

**Web searches (added after the first review):** each search is priced at $0.01 and listed in the popover (a `Web searches` row, shown only when there were any). They come from the `WebSearch` tool result, not from `usage` (§20.10 item 5). A real turn with one search gave `web_search_requests: 1` live and restored, with every token field equal.

**Differences from §20:** §20.4 was changed by the spike: the sidecar sends `result.usage` under the main-loop assistant model, not `modelUsage` (§20.10). Because of that Task 5 has no `UsageTracker`; `sidecar/src/usage.ts` has `toWireUsage` and `turnUsage`. §20.2's price matching was narrowed by the final review (below). `PriceTable` lives in `usage.rs`, not a separate `prices.rs`.

**Final review (Plan 10):** no Critical or Important finding on the seven focus items. One finding was graded up after re-grading by effect and fixed with a failing test first: a longer model id that only starts with a listed id (`claude-fable-5-2`, `claude-opus-5-6`) was priced at the shorter row's rates, so a newer model showed a wrong cost with no hint. A row now matches only its exact id or the id plus `-YYYYMMDD`; anything else shows `—`.

**Not fixed, on purpose (Minor):**
- `LiveSessionView.context` is not cleared when a sidecar fails or a later `getContextUsage` fails or times out, so the ring can show the previous turn's value until the next turn ends.
- Token and cost sums use plain `+` on `u64`; a corrupt session file with absurd counts could panic a debug build or wrap in a release build (`saturating_add` would harden it).
- Esc closes the popover only while the ring button has focus; the popover has no `role="dialog"`.
- A turn that ends without a `result` (an error, an interrupt) loses its tokens from the live total; the restore path counts them after a reload.
- If the model changes in the middle of one turn, all of that turn's `result.usage` is attributed to the last main-loop model.
- The ring is part of the message box, so a session that can only be viewed (its folder is gone) shows no ring or popover.
- Sub-agent and helper-model usage, code-execution time, and data-residency (1.1x), fast-mode and batch pricing are not counted or modelled (§20.1).

## 21. Detailed Design: slash command autocomplete (Plan 11)

Plan 11 lists the slash commands of a session when the user types `/` in the
message box. The list is whatever the SDK reports; Forge defines no commands.

### 21.1 Scope and decisions

- **Verified against the desktop docs (read 2026-10-01):** typing `/` in the
  prompt box, or the **+** button then **Slash commands**, shows what is
  available: built-in commands, custom skills, project skills and skills from
  installed plugins. Selecting one makes it appear highlighted in the input
  field; the user types the task after it and sends as usual. A command can be
  sent while a turn is running, like any message.
- **Not specified by the docs (Forge's own choices, labelled as such):** the
  filter rule (case-insensitive prefix, then substring, on the name), the keys
  (Up/Down move, Enter or Tab insert, Esc close), and the popup look (the same
  style as the usage popover). The **+** button is out of scope; only `/`.
- **List available before the first message.** The SDK sends `system/init`
  only once a turn has started, so the sidecar asks for the list as soon as the
  query is running (§21.3) and again on `init`; the later value replaces the
  earlier. A session whose list never arrives shows no popup.
- **Names, plus description when the SDK gives one.** `init.slash_commands` is
  a list of names. If the spike (§21.6) shows `supportedCommands()` also
  returns a description and an argument hint, they are shown beside the name.
- **Out of scope:** running commands in a special way (they are sent as plain
  text, as today), argument completion, the **+** menu, a cache of the list
  across sessions, editing or hiding commands.

### 21.2 Domain (`forge-domain`)

`SlashCommand { name: String, description: Option<String>, argument_hint:
Option<String> }`, with `name` stored without the leading `/`. The live
session view keeps `commands: Vec<SlashCommand>`; it is empty until the first
`CommandsAvailable` event and is replaced, not merged, by each later one.

### 21.3 Sidecar

- New event `commands_available { commands: [{ name, description?,
  argument_hint? }] }` in `protocol.ts` and in the Rust `SidecarEvent`.
- After the query is created, the session calls `supportedCommands()` (with a
  3 s timeout, like `getContextUsage`) and emits the event. A failure or
  timeout emits nothing and is not an error.
- On `system/init` it emits the event again from `slash_commands` (names
  only), but only when the list differs from the last one, so a richer list
  from `supportedCommands()` is not replaced by a poorer one with the same
  names.

### 21.4 Message box

A pure function `filter_commands(commands, input) -> Vec<&SlashCommand>`: the
popup is open only while the input starts with `/` and has no whitespace
after the name part; the query is the text after `/`. Prefix matches come
first, then substring matches, each in the SDK's order. Up/Down wrap around,
Enter and Tab insert `/name ` and close the popup, Esc closes it until the
input changes. With no match the popup is not shown. Enter with the popup
closed sends as today.

### 21.5 Testing

Sidecar: the event on `supportedCommands()`, on `init`, the timeout and the
no-downgrade rule. Rust: protocol parsing, the replace rule in the view.
Widgets: `filter_commands` and the key handling as pure functions. A person
checks the live window (not automatable here).

### 21.6 Spike (first task of the plan)

Before any code: with the real SDK (0.3.283) check (1) `supportedCommands()`
before any user message returns, and how long it takes; (2) its fields; (3)
whether it lists plugin and skill commands that Forge enables; (4) what
`init.slash_commands` contains for the same session. If (1) fails, the plan
falls back to starting a turn-less init another way, and the spike's result is
recorded here as §21.7.

### 21.7 Spike result

Checked 2026-10-01 with the real SDK 0.3.283 (`query()` with a prompt iterable that had not yielded anything), cwd `sidecar/`, `settingSources: ["project"]`, no Forge plugins or skills.

1. **`supportedCommands()` answers before any user message.** First run 3 972 ms (it includes starting the SDK process), second run 886 ms. It returned 53 and 52 commands in the two runs (the count is not stable between runs), so the list is read as given and never counted on. No timeout was hit; the 10 s limit in §21.3 stays.
2. **Fields:** `name`, `description`, `argumentHint` and `builtin` (boolean). Descriptions can be very long (a skill's whole "when to use" text), so the popup shows one truncated line. `builtin` is not used.
3. **Names have no leading `/`.** The sidecar still strips one (§21.3) in case another SDK version adds it.
4. **`init.slash_commands` is a list of names without `/`**, 52 names in the run with a message, in a different order from the one `supportedCommands()` gave first. It contains an internal-looking name, `__remote-workflow`. The names are not filtered: the spec says to list what the SDK reports. Whether the two lists always have the same names was not established beyond these two runs, so the "names changed" rule in §21.3 stays as designed (the later list wins when its names differ).
5. Not checked: that Forge's own enabled skills and plugins show up (they were not loaded in this probe). The live check in the last task covers it.

### 21.8 As built

Plan 11 was built on branch `plan-11` and checked on 2026-10-01 (macOS 26, Dioxus 0.7.10, Agent SDK 0.3.283). 472 Rust workspace tests pass (454 before) and 68 sidecar tests pass (60 before).

**Checked (passed):** the real SDK answers `supportedCommands()` before any message (§21.7). Every other part is covered by tests: the sidecar events (`supportedCommands()`, `init`, the same-names rule, a failing and a never-answering call, names with a slash, empty or duplicated); the Rust event and view (a new list replaces the old); the warm-up (starts the sidecar with no send, the first send reuses it, a failed warm-up shows no error and the first send starts again, no warm-up when the folder is gone, close right after warm-up); and the pure menu rules (query, prefix then substring order, wrap-around selection, the inserted text, keys, IME Enter).

**Not checked (needs a person):** the live window. The popup code builds and its rules are tested, but nobody has used it in the running app: that `/` lists the commands in a new session before the first message, typing narrows them, Up/Down/Enter/Tab/Esc and a mouse click work, Korean IME Enter does not pick, the popup looks right in light and dark mode with a very long description, and that Forge's own enabled skills and plugins appear in the list (the §21.7 probe did not load them).

**Differences from §21 and the plan:**
- The warm-up is opt-in: `SessionRegistry::with_warm_up()`, called once in `forge-app`'s `main.rs`, instead of an unconditional call in the registry. An unconditional call made the registry tests that use an unscripted factory spawn a sidecar in the session task and made their "opening spawns nothing" assertions untrue.
- The `supportedCommands()` timeout is 10 s (spike: 4 s for the first call).
- `builtin` (a field of the SDK's answer) is not used.

**Known costs:** every opened session whose folder exists now holds a sidecar process from the moment it is opened; the enabled extensions are resolved at that moment, not at the first send, so toggling one later does not reach an already open session until it is reopened.

**Final review (Plan 11):** no Critical finding. Two Important findings were fixed, each with a failing test first: (1) the `init` list (names only, in another order, sometimes another length than `supportedCommands()`) replaced the richer list, so descriptions and hints vanished after the first message; the sidecar now merges what it already knew into the `init` names and compares without regard to order; (2) a warm-up sidecar that failed after starting (SDK start-up error, process exit) showed the error banner and "needs attention" before the person had asked for anything; the actor now drops such a sidecar quietly until something has been sent to it.

**Not fixed, on purpose (Minor):**
- `Enter` picks the row stored in `selected`, which is not clamped like the highlighted row: if the list shrinks while the person has moved the selection down, Enter picks nothing and sends nothing for that press.
- Shift+Enter and Shift+Tab pick a command while the popup is open, instead of adding a line.
- A failed send of a bare `/command` is put back in the box and reopens the popup, so Enter completes the text instead of resending.
- Not tested: whether warming a resumed session (`resume` with no message) changes its transcript file; check it in the live window.
- The popup and the usage popover use the same layer and could overlap.

## 22. Detailed Design: slash commands on the new-session screen (Plan 11, part 2)

§21 lists commands in the message box of an **open** session. The new-session
screen (`NewSessionComposer`) has its own text box and no session: `start_new`
opens the session only when the first message is sent, so no sidecar exists
before that and §21's warm-up never applies. The first live check of §21
(2026-10-01) showed exactly this: `/` on the new-session screen listed nothing.
§21.1's "before the first message" is therefore true for a reopened past
session and false for a new one until §22 is built.

### 22.1 Scope and decisions

- **Goal.** On the new-session screen, once a folder is chosen, typing `/` in
  the first-message box lists that folder's commands.
- **A probe, not an adopted sidecar.** For the chosen folder Forge starts one
  sidecar only to learn the command list (the *probe*). It is not handed to
  the session: permission mode and model are fixed when a sidecar is started,
  and the person can still change them on this screen. The session starts its
  own sidecar at the first send, as today (§17.4).
- **The list carries over.** When the first message is sent, the probe's last
  list becomes the new session's initial `commands`, so the popup in the new
  session's message box is not empty while its own sidecar starts. The session's
  own `commands_available` events replace it (§21.2).
- **Lifetime.** One probe at a time. It is replaced when the chosen folder
  changes (choosing the same folder again keeps it), and ended when the person
  leaves the screen, when the first message is sent, and when the app closes.
  Dropping the probe handle ends the probe's sidecar.
- **Quiet failure.** If the probe cannot resolve the extensions, cannot start,
  or its sidecar errors or exits, there is no banner and no state change: the
  list stays as it was (empty if nothing arrived). The first send reports its
  own errors as before.
- **Same start parameters as a session, minus what does not matter.** The probe
  uses the folder, the extensions resolved for it (§17.4, the same
  `resolve_active`), permission mode `default` and no model: neither changes
  which commands exist. It sends no message.
- **Shared popup.** The popup and its key handling move out of
  `SendMessageBox` into one component used by both text boxes. The rules stay
  those of §21.4 (`forge-entities`), with three changes that come from the Plan
  11 final review and are made here because the code moves anyway:
  1. Enter/Tab pick the row that is highlighted (the selection is brought back
     into range before use), so a list that shrinks cannot make Enter do nothing.
  2. Enter or Tab with Shift held does not pick; Shift+Enter still adds a line.
  3. Text put back after a failed send does not reopen the popup.
- **Out of scope:** adopting the probe's sidecar for the session, probing for
  folders that are not chosen (no pre-start while browsing the folder list),
  caching lists between runs, showing a loading state in the popup (it simply
  is not shown until a list arrives).

### 22.2 Application (`forge-application`)

`SessionRegistry::probe_commands(&self, project: &ProjectSummary) -> Option<CommandProbe>`
returns `None` when the folder does not exist. `CommandProbe` holds a
`watch::Receiver<Vec<SlashCommand>>` (`commands()` for the latest list,
`subscribe()` to wake on a new one) and the sender side of a close channel;
dropping it ends its task. The task: resolve the active extensions, spawn a
sidecar through the factory, `start_session` with the parameters above, then
loop on `next_event`, publishing each `CommandsAvailable` and ignoring every
other event; on a stream end, an `Error` or a `SessionEnded` it just stops. On
close it calls `shutdown` on the sidecar (bounded, as `discard_sidecar` does).

`open_new` gets a fourth argument, `commands: Vec<SlashCommand>`, stored in the
new session's first view. Callers with no probe pass an empty list.

### 22.3 Workspace and screen (`forge-features`)

`Workspace` owns the current probe and the folder it was started for:
`probe_new_folder(folder: Option<PathBuf>)` replaces it only when the folder
differs, `new_commands() -> Vec<SlashCommand>` is the latest list for the chosen
folder (empty without one), and `take_new_commands()` is used by `start_new` to
seed the session and end the probe. `NewSessionComposer` calls
`probe_new_folder` whenever the chosen folder changes and `probe_new_folder(None)`
when it unmounts. The probe is not started for a folder that does not exist.

### 22.4 Testing

Application: the probe publishes a list, ignores other events, replaces its list
with a later event, stops quietly on an error, a stream end and a failed spawn,
shuts its sidecar down when dropped, and `probe_commands` returns `None` for a
missing folder; `open_new` seeds `commands`. Features: the workspace keeps the
probe for the same folder, replaces it for another, and `start_new` ends it and
passes its list on. Entities: the selection is clamped before picking and Shift
disables Enter/Tab picking (`menu_action` takes the modifier). A person checks
the live window (new-session screen: `/` lists the commands after a few
seconds; changing the folder changes the list; Start with a `/command` still
works; leaving the screen leaves no extra node process running).

### 22.5 Costs

While the new-session screen is shown there is one more sidecar process (the
probe), and choosing another folder starts another; the list appears about 1 to
4 seconds after the folder is chosen (§21.7). A person who starts typing a `/`
before that sees no popup yet.

### 22.6 As built

Built on branch `plan-11` and checked on 2026-10-01 (macOS 26, Dioxus 0.7.10, Agent SDK 0.3.283). 489 Rust workspace tests pass (472 before §22) and 68 sidecar tests pass (unchanged: §22 needed no sidecar change).

**Checked (passed):**
- The probe (starts a default-mode sidecar for the folder with no message, publishes each list, ignores other events, stops quietly on an error, a failed spawn and a missing folder, shuts its sidecar down when dropped), `ProbeSlot` (starts for a folder, keeps the probe for the same folder, ends it and clears the list for another folder so a late event of the old folder does not show, `end()` returns the latest list), `open_new` seeding the session's `commands`, and the pure popup rules (Shift+Enter does not pick; text that was not typed does not open the popup) are covered by tests.
- The real app, started with the sidecar built: with the new-session screen showing its default folder there is exactly one `sidecar/dist/index.js` process (`pgrep`), and none after the app is killed. No Dioxus warnings in the log (the first version created the list signal in `ScopeId::APP` and warned that `spawn_forever` runs in `ScopeId::ROOT`; it is created in `ROOT` now).

**Not checked (needs a person):** the popup itself in the new-session screen and in an open session (the list appearing about 1 to 4 s after the folder is chosen, narrowing, Up/Down/Enter/Tab/Esc, a mouse click, Shift+Enter with the popup open, Korean IME Enter, light and dark mode); that choosing another folder replaces the list; that Start with a `/command` first message sends it and the new session's box shows the carried-over list; that leaving the screen for an open session ends the probe and coming back starts one; the failure runs (a folder that no longer exists, `node` missing from `PATH`).

**Differences from §22:** none in behaviour. The popup's "selection brought back into range" is `move_selection(selected, count, 0)` computed in `CommandTextArea` rather than a separate function.

**Final review (Plan 11, part 2):** no Critical finding. Two Important findings were fixed, each with a failing test first: (1) text put back in the box after a failed send reopened the popup in the common path (type `/compact`, pick, send, send fails): the "typed" text was never forgotten when the box was cleared or a command was picked, so the put-back text equalled it again; what counts as typed is now forgotten as soon as the text changes without a keystroke (`typed_after_change`); (2) on the multi-thread runtime `abort()` cannot stop a forwarder that is already mid-poll, so a list from a replaced probe could land after the list was cleared and show one folder's commands under another; every list is now published under the slot's lock only if its probe is still the running one (a generation number).

**Not fixed, on purpose (Minor):**
- `start_new` ends whatever probe the slot holds when `saved_model()` returns: if the person picked another folder or session during that read, the new session is seeded with that folder's list or an empty one (its own sidecar sends the real list a moment later).
- An ended probe still resolves extensions, spawns node and runs `start_session` before it shuts down (it watches the end signal only after `start_session` returns), so stepping through the folder list quickly can start several sidecars at once. They end; they do not leak.
- Changing the enabled extensions in Settings does not re-probe: the list is stale until the folder changes or the screen is shown again. Every return to the new-session screen starts a fresh probe.
- If `open_new` fails after the probe was ended, the composer stays with an empty list until a folder signal changes.
- Not checked: whether a probe that never sends a message leaves a transcript file on disk.
