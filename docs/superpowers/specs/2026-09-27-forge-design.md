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

## 11. Naming

App name: **Forge**. Chosen to avoid implying an official Anthropic
product (no "Claude" in the name).
