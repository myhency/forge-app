# Forge Application Layer — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `forge-application` — the hexagonal core's use-case layer — plus the `forge-domain` types and `forge-ports` traits it needs, so the business logic for starting/resuming sessions, sending messages, approving permissions, interrupting turns, and toggling skills/plugins/MCP servers is fully implemented and tested against mocked ports, with no concrete adapter required.

**Architecture:** Extends Plan 1's hexagonal core. `forge-domain` gains session/message/tool-call/permission-request/extension/config types. `forge-ports` gains five new port traits (`ConfigRepositoryPort`, `SessionRepositoryPort`, `SkillRepositoryPort`, `PluginRepositoryPort`, `McpRegistryPort`) alongside Plan 1's `SidecarPort`. `forge-application` — a new crate depending only on `forge-domain` + `forge-ports`, never a concrete adapter — holds a pure event-translation function (`SidecarEvent` → `Session` state) and 8 use-cases, each tested with `mockall`-generated mocks.

**Tech Stack:** Rust (`tokio`, `async-trait`, `serde`/`serde_json`, `thiserror`, `mockall`) — no new external dependencies beyond what Plan 1 already introduced.

**Spec:** `docs/superpowers/specs/2026-09-27-forge-design.md` (§12, "Detailed Design: `forge-application`")

## Global Constraints

- `forge-application` depends only on `forge-domain` + `forge-ports`, never on a concrete adapter crate (spec §6, §12.3).
- Every `Mock*` type in `forge-ports` must be reachable from a downstream crate's own tests via a `mocks` Cargo feature — not a bare `#[cfg(test)]`, which is per-crate only (spec §12.1).
- Credential injection (`base_url`/`api_key`) happens entirely outside `forge-application` — `StartSessionUseCase` never touches `ConfigRepositoryPort`, and `StartSessionParams` never carries credentials (spec §12.3).
- `McpServerDefinition.config` stays an opaque `serde_json::Value` — the real `~/.claude.json`/`.mcp.json` `mcpServers` format has not been verified firsthand yet; do not invent a more specific shape (spec §12.2).
- App name is "Forge" — no "Claude" in the name (spec §11).

## Review Focus

- A `ToolResult` event whose `tool_call_id` doesn't match any known `ToolCall` must be ignored, not panic or silently corrupt state — covered by Task 7's `tool_result_for_an_unknown_id_is_ignored_without_panicking`.
- A `ToolUse` event arriving when the most recent message isn't an in-progress `Assistant` message (e.g. right after a `User` message) must still produce a well-formed new `Assistant` message rather than attaching to the wrong one — covered by Task 7's `tool_use_with_no_preceding_assistant_message_starts_a_new_one`.
- If the same `tool_call_id` value appears more than once across a session's history (shouldn't happen in practice, but nothing enforces uniqueness), a `ToolResult` must update the most recent matching call, not an older one — covered by Task 7's `tool_result_updates_the_most_recent_matching_tool_call_when_ids_repeat`.
- A `Toggle*UseCase` whose `save()` call fails after a successful `load()` must return an error, not a false-success with an unsaved toggle — covered by Task 10's `toggle_skill_use_case_propagates_a_save_error`.
- `ResumeSessionUseCase` must return the sidecar's error (not the loaded session) if `start_session` fails after history already loaded successfully — covered by Task 9's `resume_session_use_case_returns_a_sidecar_error_even_after_history_loads_successfully`.

---

### Task 1: `forge-ports` — make `Mock*` types reachable from downstream crates

**Files:**
- Modify: `crates/forge-ports/Cargo.toml`
- Modify: `crates/forge-ports/src/sidecar_port.rs`
- Modify: `crates/forge-ports/src/lib.rs`

**Interfaces:**
- Consumes: nothing new
- Produces: a `mocks` Cargo feature on `forge-ports` that, when enabled by a downstream crate's `[dev-dependencies]` entry, makes `MockSidecarPort` (and every future `Mock*` type added by this plan) buildable outside `forge-ports` itself, without adding `mockall` to any crate's production build (Cargo resolver v2, already set at the workspace root, isolates a dev-dependency's extra features to test/dev builds only)

- [ ] **Step 1: Add `mockall` as an optional regular dependency and a `mocks` feature**

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
mockall = { version = "0.13", optional = true }

[dev-dependencies]
mockall = "0.13"
tokio = { version = "1", features = ["macros", "rt"] }

[features]
mocks = ["dep:mockall"]
```

- [ ] **Step 2: Widen the `#[cfg(test)]` gates to also accept the `mocks` feature**

In `crates/forge-ports/src/sidecar_port.rs`, change:
```rust
#[cfg(test)]
use mockall::automock;
```
to:
```rust
#[cfg(any(test, feature = "mocks"))]
use mockall::automock;
```
and change:
```rust
#[cfg_attr(test, automock)]
#[async_trait]
pub trait SidecarPort: Send + Sync {
```
to:
```rust
#[cfg_attr(any(test, feature = "mocks"), automock)]
#[async_trait]
pub trait SidecarPort: Send + Sync {
```
Leave everything else in this file unchanged.

- [ ] **Step 3: Update the re-export gate in `lib.rs`**

`crates/forge-ports/src/lib.rs`:
```rust
mod sidecar_port;

pub use sidecar_port::{
    PermissionRequestId, SidecarError, SidecarEvent, SidecarPort, StartSessionParams,
};

#[cfg(any(test, feature = "mocks"))]
pub use sidecar_port::MockSidecarPort;
```

- [ ] **Step 4: Verify the existing test suite still passes unchanged**

Run: `source "$HOME/.cargo/env" 2>/dev/null; cargo test -p forge-ports` (or, if `source` is unavailable in your shell tool, prefix with `PATH="$HOME/.cargo/bin:$PATH"` instead)
Expected: the same test that existed before this task (`mock_sidecar_port_can_record_a_start_session_call`) still passes — `cfg(test)` alone already satisfies `cfg(any(test, feature = "mocks"))`, so no test code changes were needed for this to keep working.

- [ ] **Step 5: Verify the feature actually gates what it should**

Run: `cargo build -p forge-ports --features mocks`
Expected: exits 0. This proves the `mocks` feature compiles cleanly on its own (not just under `cfg(test)`), which is what a downstream crate's dev-dependency will rely on.

Run: `cargo tree -p forge-ports -e normal`
Expected: `mockall` does NOT appear in the output (only `forge-domain`, `async-trait`, `serde`, `serde_json`, `thiserror` should appear) — this proves the optional dependency stays out of forge-ports' own default/production dependency graph when the `mocks` feature isn't explicitly requested.

- [ ] **Step 6: Commit**

```bash
git add crates/forge-ports/Cargo.toml crates/forge-ports/src/sidecar_port.rs crates/forge-ports/src/lib.rs
git commit -m "feat: gate forge-ports mocks behind a Cargo feature, not bare cfg(test)"
```

---

### Task 2: `forge-domain` — session, message, tool-call, and permission-request types

**Files:**
- Create: `crates/forge-domain/src/session.rs`
- Modify: `crates/forge-domain/src/lib.rs`

**Interfaces:**
- Consumes: `forge_domain::PermissionMode` (Plan 1)
- Produces: `forge_domain::{SessionId, Session, SessionSummary, MessageRole, Message, ToolCall, ToolResult, PermissionRequest}` — consumed by Task 7 (`apply_sidecar_event`) and Task 9 (`ResumeSessionUseCase`)

- [ ] **Step 1: Write the types and their tests together**

`crates/forge-domain/src/session.rs`:
```rust
use std::path::PathBuf;

use crate::PermissionMode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionId(pub String);

#[derive(Debug, Clone, PartialEq)]
pub struct Session {
    pub id: SessionId,
    pub project_path: PathBuf,
    pub title: Option<String>,
    pub messages: Vec<Message>,
    pub permission_mode: PermissionMode,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SessionSummary {
    pub id: SessionId,
    pub title: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageRole {
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
}

impl Message {
    pub fn new(role: MessageRole) -> Self {
        Self {
            role,
            content: String::new(),
            tool_calls: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub input: serde_json::Value,
    pub result: Option<ToolResult>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolResult {
    pub output: String,
    pub is_error: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PermissionRequest {
    pub id: String,
    pub tool_name: String,
    pub input: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_new_starts_empty() {
        let message = Message::new(MessageRole::Assistant);
        assert_eq!(message.role, MessageRole::Assistant);
        assert_eq!(message.content, "");
        assert!(message.tool_calls.is_empty());
    }

    #[test]
    fn session_id_equality_is_value_based() {
        assert_eq!(SessionId("s-1".to_string()), SessionId("s-1".to_string()));
        assert_ne!(SessionId("s-1".to_string()), SessionId("s-2".to_string()));
    }
}
```

- [ ] **Step 2: Update `lib.rs` to export the new module**

`crates/forge-domain/src/lib.rs`:
```rust
mod permission;
mod session;

pub use permission::{PermissionDecision, PermissionMode};
pub use session::{
    Message, MessageRole, PermissionRequest, Session, SessionId, SessionSummary, ToolCall,
    ToolResult,
};
```

- [ ] **Step 3: Run the tests**

Run: `cargo test -p forge-domain`
Expected: the 2 new tests pass alongside the 3 existing `permission` tests (5 total).

- [ ] **Step 4: Commit**

```bash
git add crates/forge-domain/src/session.rs crates/forge-domain/src/lib.rs
git commit -m "feat: add Session/Message/ToolCall/PermissionRequest domain types"
```

---

### Task 3: `forge-domain` — skill, plugin, MCP-server, and config types

**Files:**
- Create: `crates/forge-domain/src/extension.rs`
- Create: `crates/forge-domain/src/config.rs`
- Modify: `crates/forge-domain/src/lib.rs`

**Interfaces:**
- Consumes: nothing new
- Produces: `forge_domain::{SkillDefinition, PluginDefinition, McpServerDefinition, AppConfig}` — consumed by Task 6 (repository ports) and Task 10 (toggle use-cases)

- [ ] **Step 1: Write the extension types and their tests**

`crates/forge-domain/src/extension.rs`:
```rust
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub struct SkillDefinition {
    pub name: String,
    pub path: PathBuf,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PluginDefinition {
    pub name: String,
    pub marketplace: Option<String>,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq)]
pub struct McpServerDefinition {
    pub name: String,
    pub config: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_definition_holds_its_fields() {
        let skill = SkillDefinition {
            name: "brainstorming".to_string(),
            path: PathBuf::from("/skills/brainstorming"),
            description: Some("Explore ideas".to_string()),
        };
        assert_eq!(skill.name, "brainstorming");
        assert_eq!(skill.description.as_deref(), Some("Explore ideas"));
    }

    #[test]
    fn mcp_server_definition_config_is_opaque_json() {
        let server = McpServerDefinition {
            name: "playwright".to_string(),
            config: serde_json::json!({"command": "npx", "args": ["playwright-mcp"]}),
        };
        assert_eq!(server.config["command"], "npx");
    }
}
```

- [ ] **Step 2: Write the config type and its tests**

`crates/forge-domain/src/config.rs`:
```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct AppConfig {
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    #[serde(default)]
    pub enabled_skills: Vec<String>,
    #[serde(default)]
    pub enabled_plugins: Vec<String>,
    #[serde(default)]
    pub enabled_mcp_servers: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_config_round_trips_through_json() {
        let config = AppConfig {
            base_url: Some("https://api.anthropic.com".to_string()),
            api_key: Some("sk-ant-test".to_string()),
            enabled_skills: vec!["brainstorming".to_string()],
            enabled_plugins: vec![],
            enabled_mcp_servers: vec!["playwright".to_string()],
        };
        let json = serde_json::to_string(&config).unwrap();
        let back: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, back);
    }

    #[test]
    fn app_config_defaults_to_empty_lists_and_no_credentials() {
        let config = AppConfig::default();
        assert_eq!(config.base_url, None);
        assert_eq!(config.api_key, None);
        assert!(config.enabled_skills.is_empty());
        assert!(config.enabled_plugins.is_empty());
        assert!(config.enabled_mcp_servers.is_empty());
    }

    #[test]
    fn app_config_deserializes_from_json_missing_optional_lists() {
        let json = r#"{"base_url": null, "api_key": null}"#;
        let config: AppConfig = serde_json::from_str(json).unwrap();
        assert!(config.enabled_skills.is_empty());
    }
}
```

- [ ] **Step 3: Update `lib.rs`**

`crates/forge-domain/src/lib.rs`:
```rust
mod config;
mod extension;
mod permission;
mod session;

pub use config::AppConfig;
pub use extension::{McpServerDefinition, PluginDefinition, SkillDefinition};
pub use permission::{PermissionDecision, PermissionMode};
pub use session::{
    Message, MessageRole, PermissionRequest, Session, SessionId, SessionSummary, ToolCall,
    ToolResult,
};
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p forge-domain`
Expected: 5 new tests pass alongside the 5 from Task 2 (10 total).

- [ ] **Step 5: Commit**

```bash
git add crates/forge-domain/src/extension.rs crates/forge-domain/src/config.rs crates/forge-domain/src/lib.rs
git commit -m "feat: add SkillDefinition/PluginDefinition/McpServerDefinition/AppConfig"
```

---

### Task 4: `forge-ports` — `ConfigRepositoryPort`

**Files:**
- Create: `crates/forge-ports/src/config_port.rs`
- Modify: `crates/forge-ports/src/lib.rs`

**Interfaces:**
- Consumes: `forge_domain::AppConfig` (Task 3)
- Produces: `forge_ports::{ConfigRepositoryPort, ConfigError}` and (under `mocks`/test) `MockConfigRepositoryPort` — consumed by Task 10

- [ ] **Step 1: Write the trait and its test**

`crates/forge-ports/src/config_port.rs`:
```rust
use async_trait::async_trait;
use forge_domain::AppConfig;
use thiserror::Error;

#[cfg(any(test, feature = "mocks"))]
use mockall::automock;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config file: {0}")]
    ReadFailed(String),
    #[error("failed to write config file: {0}")]
    WriteFailed(String),
    #[error("failed to parse config file: {0}")]
    ParseFailed(String),
}

#[cfg_attr(any(test, feature = "mocks"), automock)]
#[async_trait]
pub trait ConfigRepositoryPort: Send + Sync {
    async fn load(&self) -> Result<AppConfig, ConfigError>;
    async fn save(&self, config: &AppConfig) -> Result<(), ConfigError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_config_repository_port_can_record_a_load_call() {
        let mut mock = MockConfigRepositoryPort::new();
        mock.expect_load().times(1).returning(|| Ok(AppConfig::default()));

        let result = mock.load().await;
        assert!(result.is_ok());
    }
}
```

- [ ] **Step 2: Update `lib.rs`**

`crates/forge-ports/src/lib.rs`:
```rust
mod config_port;
mod sidecar_port;

pub use config_port::{ConfigError, ConfigRepositoryPort};
pub use sidecar_port::{
    PermissionRequestId, SidecarError, SidecarEvent, SidecarPort, StartSessionParams,
};

#[cfg(any(test, feature = "mocks"))]
pub use config_port::MockConfigRepositoryPort;
#[cfg(any(test, feature = "mocks"))]
pub use sidecar_port::MockSidecarPort;
```

- [ ] **Step 3: Run the tests**

Run: `cargo test -p forge-ports`
Expected: 2 tests pass (1 new + the existing sidecar one).

- [ ] **Step 4: Commit**

```bash
git add crates/forge-ports/src/config_port.rs crates/forge-ports/src/lib.rs
git commit -m "feat: add ConfigRepositoryPort"
```

---

### Task 5: `forge-ports` — `SessionRepositoryPort`

**Files:**
- Create: `crates/forge-ports/src/session_port.rs`
- Modify: `crates/forge-ports/src/lib.rs`

**Interfaces:**
- Consumes: `forge_domain::{Session, SessionId, SessionSummary}` (Task 2)
- Produces: `forge_ports::{SessionRepositoryPort, SessionRepositoryError}` and (under `mocks`/test) `MockSessionRepositoryPort` — consumed by Task 9

- [ ] **Step 1: Write the trait and its test**

`crates/forge-ports/src/session_port.rs`:
```rust
use std::path::Path;

use async_trait::async_trait;
use forge_domain::{Session, SessionId, SessionSummary};
use thiserror::Error;

#[cfg(any(test, feature = "mocks"))]
use mockall::automock;

#[derive(Debug, Error)]
pub enum SessionRepositoryError {
    #[error("failed to list sessions: {0}")]
    ListFailed(String),
    #[error("session not found: {0:?}")]
    NotFound(SessionId),
    #[error("failed to load session: {0}")]
    LoadFailed(String),
}

#[cfg_attr(any(test, feature = "mocks"), automock)]
#[async_trait]
pub trait SessionRepositoryPort: Send + Sync {
    async fn list_sessions(
        &self,
        project_path: &Path,
    ) -> Result<Vec<SessionSummary>, SessionRepositoryError>;
    async fn load_session(
        &self,
        project_path: &Path,
        session_id: &SessionId,
    ) -> Result<Session, SessionRepositoryError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[tokio::test]
    async fn mock_session_repository_port_can_record_a_list_sessions_call() {
        let mut mock = MockSessionRepositoryPort::new();
        mock.expect_list_sessions()
            .withf(|p: &Path| p == Path::new("/tmp/project"))
            .times(1)
            .returning(|_| Ok(vec![]));

        let result = mock.list_sessions(&PathBuf::from("/tmp/project")).await;
        assert!(result.is_ok());
    }
}
```

- [ ] **Step 2: Update `lib.rs`**

`crates/forge-ports/src/lib.rs`:
```rust
mod config_port;
mod session_port;
mod sidecar_port;

pub use config_port::{ConfigError, ConfigRepositoryPort};
pub use session_port::{SessionRepositoryError, SessionRepositoryPort};
pub use sidecar_port::{
    PermissionRequestId, SidecarError, SidecarEvent, SidecarPort, StartSessionParams,
};

#[cfg(any(test, feature = "mocks"))]
pub use config_port::MockConfigRepositoryPort;
#[cfg(any(test, feature = "mocks"))]
pub use session_port::MockSessionRepositoryPort;
#[cfg(any(test, feature = "mocks"))]
pub use sidecar_port::MockSidecarPort;
```

- [ ] **Step 3: Run the tests**

Run: `cargo test -p forge-ports`
Expected: 3 tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/forge-ports/src/session_port.rs crates/forge-ports/src/lib.rs
git commit -m "feat: add SessionRepositoryPort"
```

---

### Task 6: `forge-ports` — `SkillRepositoryPort`, `PluginRepositoryPort`, `McpRegistryPort`

**Files:**
- Create: `crates/forge-ports/src/extension_ports.rs`
- Modify: `crates/forge-ports/src/lib.rs`

**Interfaces:**
- Consumes: `forge_domain::{SkillDefinition, PluginDefinition, McpServerDefinition}` (Task 3)
- Produces: `forge_ports::{SkillRepositoryPort, SkillRepositoryError, PluginRepositoryPort, PluginRepositoryError, McpRegistryPort, McpRegistryError}` and their `Mock*` types — none of these are consumed by this plan's use-cases (no `Toggle*UseCase` reads them; toggling only touches `ConfigRepositoryPort`'s enabled-lists), but they complete the port surface spec §6 named. Grouped in one file since all three are small, same-shaped "list only" ports.

- [ ] **Step 1: Write the three traits and their tests**

`crates/forge-ports/src/extension_ports.rs`:
```rust
use std::path::Path;

use async_trait::async_trait;
use forge_domain::{McpServerDefinition, PluginDefinition, SkillDefinition};
use thiserror::Error;

#[cfg(any(test, feature = "mocks"))]
use mockall::automock;

#[derive(Debug, Error)]
pub enum SkillRepositoryError {
    #[error("failed to list skills: {0}")]
    ListFailed(String),
}

#[cfg_attr(any(test, feature = "mocks"), automock)]
#[async_trait]
pub trait SkillRepositoryPort: Send + Sync {
    async fn list_skills(&self, project_path: &Path) -> Result<Vec<SkillDefinition>, SkillRepositoryError>;
}

#[derive(Debug, Error)]
pub enum PluginRepositoryError {
    #[error("failed to list plugins: {0}")]
    ListFailed(String),
}

#[cfg_attr(any(test, feature = "mocks"), automock)]
#[async_trait]
pub trait PluginRepositoryPort: Send + Sync {
    async fn list_plugins(&self) -> Result<Vec<PluginDefinition>, PluginRepositoryError>;
}

#[derive(Debug, Error)]
pub enum McpRegistryError {
    #[error("failed to list mcp servers: {0}")]
    ListFailed(String),
}

#[cfg_attr(any(test, feature = "mocks"), automock)]
#[async_trait]
pub trait McpRegistryPort: Send + Sync {
    async fn list_servers(&self, project_path: &Path) -> Result<Vec<McpServerDefinition>, McpRegistryError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[tokio::test]
    async fn mock_skill_repository_port_can_record_a_list_skills_call() {
        let mut mock = MockSkillRepositoryPort::new();
        mock.expect_list_skills().times(1).returning(|_| Ok(vec![]));
        let result = mock.list_skills(&PathBuf::from("/tmp/project")).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn mock_plugin_repository_port_can_record_a_list_plugins_call() {
        let mut mock = MockPluginRepositoryPort::new();
        mock.expect_list_plugins().times(1).returning(|| Ok(vec![]));
        let result = mock.list_plugins().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn mock_mcp_registry_port_can_record_a_list_servers_call() {
        let mut mock = MockMcpRegistryPort::new();
        mock.expect_list_servers().times(1).returning(|_| Ok(vec![]));
        let result = mock.list_servers(&PathBuf::from("/tmp/project")).await;
        assert!(result.is_ok());
    }
}
```

- [ ] **Step 2: Update `lib.rs`**

`crates/forge-ports/src/lib.rs`:
```rust
mod config_port;
mod extension_ports;
mod session_port;
mod sidecar_port;

pub use config_port::{ConfigError, ConfigRepositoryPort};
pub use extension_ports::{
    McpRegistryError, McpRegistryPort, PluginRepositoryError, PluginRepositoryPort,
    SkillRepositoryError, SkillRepositoryPort,
};
pub use session_port::{SessionRepositoryError, SessionRepositoryPort};
pub use sidecar_port::{
    PermissionRequestId, SidecarError, SidecarEvent, SidecarPort, StartSessionParams,
};

#[cfg(any(test, feature = "mocks"))]
pub use config_port::MockConfigRepositoryPort;
#[cfg(any(test, feature = "mocks"))]
pub use extension_ports::{MockMcpRegistryPort, MockPluginRepositoryPort, MockSkillRepositoryPort};
#[cfg(any(test, feature = "mocks"))]
pub use session_port::MockSessionRepositoryPort;
#[cfg(any(test, feature = "mocks"))]
pub use sidecar_port::MockSidecarPort;
```

- [ ] **Step 3: Run the tests**

Run: `cargo test -p forge-ports`
Expected: 6 tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/forge-ports/src/extension_ports.rs crates/forge-ports/src/lib.rs
git commit -m "feat: add SkillRepositoryPort, PluginRepositoryPort, McpRegistryPort"
```

---

### Task 7: `forge-application` — crate scaffold + `apply_sidecar_event`

**Files:**
- Create: `crates/forge-application/Cargo.toml`
- Create: `crates/forge-application/src/lib.rs`
- Create: `crates/forge-application/src/event.rs`

**Interfaces:**
- Consumes: `forge_domain::{Session, SessionId, Message, MessageRole, ToolCall, ToolResult, PermissionRequest}` (Task 2), `forge_ports::SidecarEvent` (Plan 1)
- Produces: `forge_application::{apply_sidecar_event, SessionUpdate}` — consumed by whatever future plan drives the sidecar's event loop (UI plan); not consumed by this plan's other tasks, but this plan's own tests exercise it directly

- [ ] **Step 1: Create the crate**

`crates/forge-application/Cargo.toml`:
```toml
[package]
name = "forge-application"
version.workspace = true
edition.workspace = true

[dependencies]
forge-domain = { path = "../forge-domain" }
forge-ports = { path = "../forge-ports" }
async-trait = "0.1"
serde_json = "1"
thiserror = "1"

[dev-dependencies]
forge-ports = { path = "../forge-ports", features = ["mocks"] }
mockall = "0.13"
tokio = { version = "1", features = ["macros", "rt"] }
```

- [ ] **Step 2: Write `event.rs` (implementation and tests together — this logic is pure, so there is no separate RED step; write both, then verify green)**

`crates/forge-application/src/event.rs`:
```rust
use forge_domain::{Message, MessageRole, PermissionRequest, Session, SessionId, ToolCall, ToolResult};
use forge_ports::SidecarEvent;

#[derive(Debug, Clone, PartialEq)]
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

pub fn apply_sidecar_event(session: &mut Session, event: &SidecarEvent) -> SessionUpdate {
    match event {
        SidecarEvent::SessionStarted { session_id } => SessionUpdate::SessionStarted {
            session_id: SessionId(session_id.clone()),
        },
        SidecarEvent::AssistantDelta { text } => {
            match session.messages.last_mut() {
                Some(message) if message.role == MessageRole::Assistant => {
                    message.content.push_str(text);
                }
                _ => {
                    let mut message = Message::new(MessageRole::Assistant);
                    message.content.push_str(text);
                    session.messages.push(message);
                }
            }
            SessionUpdate::MessageDelta { text: text.clone() }
        }
        SidecarEvent::ToolUse { tool_call_id, name, input } => {
            let tool_call = ToolCall {
                id: tool_call_id.clone(),
                name: name.clone(),
                input: input.clone(),
                result: None,
            };
            match session
                .messages
                .last_mut()
                .filter(|m| m.role == MessageRole::Assistant)
            {
                Some(message) => message.tool_calls.push(tool_call.clone()),
                None => {
                    let mut message = Message::new(MessageRole::Assistant);
                    message.tool_calls.push(tool_call.clone());
                    session.messages.push(message);
                }
            }
            SessionUpdate::ToolCallStarted { tool_call }
        }
        SidecarEvent::ToolResult { tool_call_id, output, is_error } => {
            'search: for message in session.messages.iter_mut().rev() {
                for tool_call in message.tool_calls.iter_mut() {
                    if &tool_call.id == tool_call_id {
                        tool_call.result = Some(ToolResult {
                            output: output.clone(),
                            is_error: *is_error,
                        });
                        break 'search;
                    }
                }
            }
            SessionUpdate::ToolCallCompleted { tool_call_id: tool_call_id.clone() }
        }
        SidecarEvent::PermissionRequest { permission_request_id, tool_name, input } => {
            SessionUpdate::PermissionRequested(PermissionRequest {
                id: permission_request_id.clone(),
                tool_name: tool_name.clone(),
                input: input.clone(),
            })
        }
        SidecarEvent::TurnComplete { input_tokens, output_tokens } => SessionUpdate::TurnCompleted {
            input_tokens: *input_tokens,
            output_tokens: *output_tokens,
        },
        SidecarEvent::Error { message } => SessionUpdate::Errored { message: message.clone() },
        SidecarEvent::SessionEnded => SessionUpdate::SessionEnded,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_domain::PermissionMode;
    use std::path::PathBuf;

    fn empty_session() -> Session {
        Session {
            id: SessionId("s-1".to_string()),
            project_path: PathBuf::from("/tmp/project"),
            title: None,
            messages: Vec::new(),
            permission_mode: PermissionMode::Default,
        }
    }

    #[test]
    fn session_started_returns_the_session_id() {
        let mut session = empty_session();
        let update = apply_sidecar_event(
            &mut session,
            &SidecarEvent::SessionStarted { session_id: "real-id".to_string() },
        );
        assert_eq!(
            update,
            SessionUpdate::SessionStarted { session_id: SessionId("real-id".to_string()) }
        );
    }

    #[test]
    fn first_assistant_delta_creates_a_new_message() {
        let mut session = empty_session();
        apply_sidecar_event(&mut session, &SidecarEvent::AssistantDelta { text: "Hello".to_string() });

        assert_eq!(session.messages.len(), 1);
        assert_eq!(session.messages[0].role, MessageRole::Assistant);
        assert_eq!(session.messages[0].content, "Hello");
    }

    #[test]
    fn second_assistant_delta_appends_to_the_same_message() {
        let mut session = empty_session();
        apply_sidecar_event(&mut session, &SidecarEvent::AssistantDelta { text: "Hel".to_string() });
        apply_sidecar_event(&mut session, &SidecarEvent::AssistantDelta { text: "lo".to_string() });

        assert_eq!(session.messages.len(), 1);
        assert_eq!(session.messages[0].content, "Hello");
    }

    #[test]
    fn assistant_delta_after_a_user_message_starts_a_new_message() {
        let mut session = empty_session();
        session.messages.push(Message::new(MessageRole::User));
        apply_sidecar_event(&mut session, &SidecarEvent::AssistantDelta { text: "Hi".to_string() });

        assert_eq!(session.messages.len(), 2);
        assert_eq!(session.messages[1].role, MessageRole::Assistant);
        assert_eq!(session.messages[1].content, "Hi");
    }

    #[test]
    fn tool_use_appends_a_tool_call_to_the_last_assistant_message() {
        let mut session = empty_session();
        apply_sidecar_event(&mut session, &SidecarEvent::AssistantDelta { text: "Let me check".to_string() });
        apply_sidecar_event(
            &mut session,
            &SidecarEvent::ToolUse {
                tool_call_id: "t-1".to_string(),
                name: "Bash".to_string(),
                input: serde_json::json!({"command": "ls"}),
            },
        );

        assert_eq!(session.messages[0].tool_calls.len(), 1);
        assert_eq!(session.messages[0].tool_calls[0].id, "t-1");
        assert_eq!(session.messages[0].tool_calls[0].name, "Bash");
        assert!(session.messages[0].tool_calls[0].result.is_none());
    }

    #[test]
    fn tool_use_with_no_preceding_assistant_message_starts_a_new_one() {
        let mut session = empty_session();
        session.messages.push(Message::new(MessageRole::User));
        apply_sidecar_event(
            &mut session,
            &SidecarEvent::ToolUse {
                tool_call_id: "t-1".to_string(),
                name: "Bash".to_string(),
                input: serde_json::json!({}),
            },
        );

        assert_eq!(session.messages.len(), 2);
        assert_eq!(session.messages[1].role, MessageRole::Assistant);
        assert_eq!(session.messages[1].tool_calls.len(), 1);
    }

    #[test]
    fn tool_result_attaches_to_the_matching_tool_call_by_id() {
        let mut session = empty_session();
        apply_sidecar_event(
            &mut session,
            &SidecarEvent::ToolUse { tool_call_id: "t-1".to_string(), name: "Bash".to_string(), input: serde_json::json!({}) },
        );
        apply_sidecar_event(
            &mut session,
            &SidecarEvent::ToolResult { tool_call_id: "t-1".to_string(), output: "done".to_string(), is_error: false },
        );

        let tool_call = &session.messages[0].tool_calls[0];
        assert_eq!(tool_call.result.as_ref().unwrap().output, "done");
        assert!(!tool_call.result.as_ref().unwrap().is_error);
    }

    #[test]
    fn tool_result_for_an_unknown_id_is_ignored_without_panicking() {
        let mut session = empty_session();
        apply_sidecar_event(
            &mut session,
            &SidecarEvent::ToolResult { tool_call_id: "does-not-exist".to_string(), output: "ignored".to_string(), is_error: false },
        );
        assert!(session.messages.is_empty());
    }

    #[test]
    fn tool_result_updates_the_most_recent_matching_tool_call_when_ids_repeat() {
        let mut session = empty_session();
        apply_sidecar_event(
            &mut session,
            &SidecarEvent::ToolUse { tool_call_id: "t-1".to_string(), name: "Bash".to_string(), input: serde_json::json!({}) },
        );
        apply_sidecar_event(
            &mut session,
            &SidecarEvent::ToolResult { tool_call_id: "t-1".to_string(), output: "first".to_string(), is_error: false },
        );
        session.messages.push(Message::new(MessageRole::User));
        apply_sidecar_event(
            &mut session,
            &SidecarEvent::ToolUse { tool_call_id: "t-1".to_string(), name: "Bash".to_string(), input: serde_json::json!({}) },
        );
        apply_sidecar_event(
            &mut session,
            &SidecarEvent::ToolResult { tool_call_id: "t-1".to_string(), output: "second".to_string(), is_error: false },
        );

        assert_eq!(session.messages[0].tool_calls[0].result.as_ref().unwrap().output, "first");
        assert_eq!(session.messages[2].tool_calls[0].result.as_ref().unwrap().output, "second");
    }

    #[test]
    fn permission_request_is_surfaced_as_a_domain_permission_request() {
        let mut session = empty_session();
        let update = apply_sidecar_event(
            &mut session,
            &SidecarEvent::PermissionRequest {
                permission_request_id: "perm-1".to_string(),
                tool_name: "Bash".to_string(),
                input: serde_json::json!({"command": "rm -rf /"}),
            },
        );
        assert_eq!(
            update,
            SessionUpdate::PermissionRequested(PermissionRequest {
                id: "perm-1".to_string(),
                tool_name: "Bash".to_string(),
                input: serde_json::json!({"command": "rm -rf /"}),
            })
        );
    }

    #[test]
    fn turn_complete_reports_usage() {
        let mut session = empty_session();
        let update = apply_sidecar_event(&mut session, &SidecarEvent::TurnComplete { input_tokens: 10, output_tokens: 20 });
        assert_eq!(update, SessionUpdate::TurnCompleted { input_tokens: 10, output_tokens: 20 });
    }

    #[test]
    fn error_and_session_ended_pass_through() {
        let mut session = empty_session();
        assert_eq!(
            apply_sidecar_event(&mut session, &SidecarEvent::Error { message: "boom".to_string() }),
            SessionUpdate::Errored { message: "boom".to_string() }
        );
        assert_eq!(
            apply_sidecar_event(&mut session, &SidecarEvent::SessionEnded),
            SessionUpdate::SessionEnded
        );
    }
}
```

- [ ] **Step 3: Write `lib.rs`**

`crates/forge-application/src/lib.rs`:
```rust
mod event;

pub use event::{apply_sidecar_event, SessionUpdate};
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p forge-application`
Expected: all 12 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-application/Cargo.toml crates/forge-application/src/lib.rs crates/forge-application/src/event.rs
git commit -m "feat: scaffold forge-application with apply_sidecar_event"
```

---

### Task 8: `forge-application` — `SendMessageUseCase`, `ApprovePermissionUseCase`, `InterruptTurnUseCase`

**Files:**
- Create: `crates/forge-application/src/use_cases/mod.rs`
- Create: `crates/forge-application/src/use_cases/sidecar_commands.rs`
- Modify: `crates/forge-application/src/lib.rs`

**Interfaces:**
- Consumes: `forge_ports::{SidecarPort, SidecarError, MockSidecarPort}` (Plan 1 / Task 1), `forge_domain::PermissionDecision` (Plan 1)
- Produces: `forge_application::{SendMessageUseCase, ApprovePermissionUseCase, InterruptTurnUseCase}`

Grouped in one file since all three are tiny (one-call delegations to `SidecarPort`) and share no logic worth extracting — not because they're the same concept.

- [ ] **Step 1: Write the three use-cases and their tests**

`crates/forge-application/src/use_cases/sidecar_commands.rs`:
```rust
use std::sync::Arc;

use forge_domain::PermissionDecision;
use forge_ports::{SidecarError, SidecarPort};

pub struct SendMessageUseCase {
    sidecar: Arc<dyn SidecarPort>,
}

impl SendMessageUseCase {
    pub fn new(sidecar: Arc<dyn SidecarPort>) -> Self {
        Self { sidecar }
    }

    pub async fn execute(&self, text: String) -> Result<(), SidecarError> {
        self.sidecar.send_message(text).await
    }
}

pub struct ApprovePermissionUseCase {
    sidecar: Arc<dyn SidecarPort>,
}

impl ApprovePermissionUseCase {
    pub fn new(sidecar: Arc<dyn SidecarPort>) -> Self {
        Self { sidecar }
    }

    pub async fn execute(
        &self,
        permission_request_id: String,
        decision: PermissionDecision,
    ) -> Result<(), SidecarError> {
        self.sidecar.respond_to_permission(permission_request_id, decision).await
    }
}

pub struct InterruptTurnUseCase {
    sidecar: Arc<dyn SidecarPort>,
}

impl InterruptTurnUseCase {
    pub fn new(sidecar: Arc<dyn SidecarPort>) -> Self {
        Self { sidecar }
    }

    pub async fn execute(&self) -> Result<(), SidecarError> {
        self.sidecar.interrupt().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_ports::MockSidecarPort;

    #[tokio::test]
    async fn send_message_use_case_delegates_to_the_sidecar_port() {
        let mut mock = MockSidecarPort::new();
        mock.expect_send_message()
            .withf(|text: &String| text == "hello")
            .times(1)
            .returning(|_| Ok(()));

        let use_case = SendMessageUseCase::new(Arc::new(mock));
        assert!(use_case.execute("hello".to_string()).await.is_ok());
    }

    #[tokio::test]
    async fn send_message_use_case_propagates_a_sidecar_error() {
        let mut mock = MockSidecarPort::new();
        mock.expect_send_message().returning(|_| Err(SidecarError::ProcessExited));

        let use_case = SendMessageUseCase::new(Arc::new(mock));
        assert!(matches!(
            use_case.execute("hi".to_string()).await,
            Err(SidecarError::ProcessExited)
        ));
    }

    #[tokio::test]
    async fn approve_permission_use_case_delegates_to_the_sidecar_port() {
        let mut mock = MockSidecarPort::new();
        mock.expect_respond_to_permission()
            .withf(|id: &String, decision: &PermissionDecision| {
                id == "perm-1" && *decision == PermissionDecision::AllowOnce
            })
            .times(1)
            .returning(|_, _| Ok(()));

        let use_case = ApprovePermissionUseCase::new(Arc::new(mock));
        assert!(use_case
            .execute("perm-1".to_string(), PermissionDecision::AllowOnce)
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn interrupt_turn_use_case_delegates_to_the_sidecar_port() {
        let mut mock = MockSidecarPort::new();
        mock.expect_interrupt().times(1).returning(|| Ok(()));

        let use_case = InterruptTurnUseCase::new(Arc::new(mock));
        assert!(use_case.execute().await.is_ok());
    }
}
```

- [ ] **Step 2: Write `use_cases/mod.rs`**

`crates/forge-application/src/use_cases/mod.rs`:
```rust
mod sidecar_commands;

pub use sidecar_commands::{ApprovePermissionUseCase, InterruptTurnUseCase, SendMessageUseCase};
```

- [ ] **Step 3: Update `lib.rs`**

`crates/forge-application/src/lib.rs`:
```rust
mod event;
mod use_cases;

pub use event::{apply_sidecar_event, SessionUpdate};
pub use use_cases::{ApprovePermissionUseCase, InterruptTurnUseCase, SendMessageUseCase};
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p forge-application`
Expected: 4 new tests pass alongside the 12 from Task 7 (16 total).

- [ ] **Step 5: Commit**

```bash
git add crates/forge-application/src/use_cases crates/forge-application/src/lib.rs
git commit -m "feat: add SendMessage/ApprovePermission/InterruptTurn use-cases"
```

---

### Task 9: `forge-application` — `StartSessionUseCase`, `ResumeSessionUseCase`

**Files:**
- Create: `crates/forge-application/src/use_cases/session_lifecycle.rs`
- Modify: `crates/forge-application/src/use_cases/mod.rs`
- Modify: `crates/forge-application/src/lib.rs`

**Interfaces:**
- Consumes: `forge_ports::{SidecarPort, SessionRepositoryPort, SessionRepositoryError, MockSidecarPort, MockSessionRepositoryPort}` (Task 5), `forge_domain::{PermissionMode, Session, SessionId}` (Task 2)
- Produces: `forge_application::{StartSessionUseCase, ResumeSessionUseCase, ResumeSessionError}`

- [ ] **Step 1: Write the two use-cases and their tests**

`crates/forge-application/src/use_cases/session_lifecycle.rs`:
```rust
use std::path::Path;
use std::sync::Arc;

use forge_domain::{PermissionMode, Session, SessionId};
use forge_ports::{
    SessionRepositoryError, SessionRepositoryPort, SidecarError, SidecarPort, StartSessionParams,
};
use thiserror::Error;

pub struct StartSessionUseCase {
    sidecar: Arc<dyn SidecarPort>,
}

impl StartSessionUseCase {
    pub fn new(sidecar: Arc<dyn SidecarPort>) -> Self {
        Self { sidecar }
    }

    pub async fn execute(
        &self,
        cwd: String,
        permission_mode: PermissionMode,
        model: Option<String>,
    ) -> Result<(), SidecarError> {
        self.sidecar
            .start_session(StartSessionParams {
                cwd,
                resume_session_id: None,
                permission_mode,
                model,
            })
            .await
    }
}

#[derive(Debug, Error)]
pub enum ResumeSessionError {
    #[error("failed to load session history: {0}")]
    SessionRepository(#[from] SessionRepositoryError),
    #[error("failed to resume sidecar session: {0}")]
    Sidecar(#[from] SidecarError),
}

pub struct ResumeSessionUseCase {
    sidecar: Arc<dyn SidecarPort>,
    sessions: Arc<dyn SessionRepositoryPort>,
}

impl ResumeSessionUseCase {
    pub fn new(sidecar: Arc<dyn SidecarPort>, sessions: Arc<dyn SessionRepositoryPort>) -> Self {
        Self { sidecar, sessions }
    }

    pub async fn execute(
        &self,
        project_path: &Path,
        session_id: SessionId,
        permission_mode: PermissionMode,
    ) -> Result<Session, ResumeSessionError> {
        let session = self.sessions.load_session(project_path, &session_id).await?;

        self.sidecar
            .start_session(StartSessionParams {
                cwd: project_path.display().to_string(),
                resume_session_id: Some(session_id.0.clone()),
                permission_mode,
                model: None,
            })
            .await?;

        Ok(session)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_ports::{MockSessionRepositoryPort, MockSidecarPort};
    use std::path::PathBuf;

    #[tokio::test]
    async fn start_session_use_case_delegates_to_the_sidecar_port_with_no_resume_id() {
        let mut mock = MockSidecarPort::new();
        mock.expect_start_session()
            .withf(|p: &StartSessionParams| p.cwd == "/tmp/project" && p.resume_session_id.is_none())
            .times(1)
            .returning(|_| Ok(()));

        let use_case = StartSessionUseCase::new(Arc::new(mock));
        assert!(use_case
            .execute("/tmp/project".to_string(), PermissionMode::Default, None)
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn resume_session_use_case_loads_history_then_starts_the_sidecar_with_the_resume_id() {
        let mut sessions_mock = MockSessionRepositoryPort::new();
        sessions_mock.expect_load_session().times(1).returning(|_, id| {
            Ok(Session {
                id: id.clone(),
                project_path: PathBuf::from("/tmp/project"),
                title: None,
                messages: Vec::new(),
                permission_mode: PermissionMode::Default,
            })
        });

        let mut sidecar_mock = MockSidecarPort::new();
        sidecar_mock
            .expect_start_session()
            .withf(|p: &StartSessionParams| p.resume_session_id.as_deref() == Some("s-1"))
            .times(1)
            .returning(|_| Ok(()));

        let use_case = ResumeSessionUseCase::new(Arc::new(sidecar_mock), Arc::new(sessions_mock));
        let result = use_case
            .execute(&PathBuf::from("/tmp/project"), SessionId("s-1".to_string()), PermissionMode::Default)
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().id, SessionId("s-1".to_string()));
    }

    #[tokio::test]
    async fn resume_session_use_case_does_not_start_the_sidecar_if_history_fails_to_load() {
        let mut sessions_mock = MockSessionRepositoryPort::new();
        sessions_mock
            .expect_load_session()
            .times(1)
            .returning(|_, id| Err(SessionRepositoryError::NotFound(id.clone())));

        let mut sidecar_mock = MockSidecarPort::new();
        sidecar_mock.expect_start_session().times(0);

        let use_case = ResumeSessionUseCase::new(Arc::new(sidecar_mock), Arc::new(sessions_mock));
        let result = use_case
            .execute(&PathBuf::from("/tmp/project"), SessionId("s-1".to_string()), PermissionMode::Default)
            .await;

        assert!(matches!(result, Err(ResumeSessionError::SessionRepository(_))));
    }

    #[tokio::test]
    async fn resume_session_use_case_returns_a_sidecar_error_even_after_history_loads_successfully() {
        let mut sessions_mock = MockSessionRepositoryPort::new();
        sessions_mock.expect_load_session().times(1).returning(|_, id| {
            Ok(Session {
                id: id.clone(),
                project_path: PathBuf::from("/tmp/project"),
                title: None,
                messages: Vec::new(),
                permission_mode: PermissionMode::Default,
            })
        });

        let mut sidecar_mock = MockSidecarPort::new();
        sidecar_mock
            .expect_start_session()
            .times(1)
            .returning(|_| Err(SidecarError::ProcessExited));

        let use_case = ResumeSessionUseCase::new(Arc::new(sidecar_mock), Arc::new(sessions_mock));
        let result = use_case
            .execute(&PathBuf::from("/tmp/project"), SessionId("s-1".to_string()), PermissionMode::Default)
            .await;

        assert!(matches!(result, Err(ResumeSessionError::Sidecar(_))));
    }
}
```

- [ ] **Step 2: Update `use_cases/mod.rs`**

`crates/forge-application/src/use_cases/mod.rs`:
```rust
mod session_lifecycle;
mod sidecar_commands;

pub use session_lifecycle::{ResumeSessionError, ResumeSessionUseCase, StartSessionUseCase};
pub use sidecar_commands::{ApprovePermissionUseCase, InterruptTurnUseCase, SendMessageUseCase};
```

- [ ] **Step 3: Update `lib.rs`**

`crates/forge-application/src/lib.rs`:
```rust
mod event;
mod use_cases;

pub use event::{apply_sidecar_event, SessionUpdate};
pub use use_cases::{
    ApprovePermissionUseCase, InterruptTurnUseCase, ResumeSessionError, ResumeSessionUseCase,
    SendMessageUseCase, StartSessionUseCase,
};
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p forge-application`
Expected: 4 new tests pass alongside the 16 from Task 8 (20 total).

- [ ] **Step 5: Commit**

```bash
git add crates/forge-application/src/use_cases crates/forge-application/src/lib.rs
git commit -m "feat: add StartSession/ResumeSession use-cases"
```

---

### Task 10: `forge-application` — `ToggleSkillUseCase`, `TogglePluginUseCase`, `ToggleMcpServerUseCase`

**Files:**
- Create: `crates/forge-application/src/use_cases/toggles.rs`
- Modify: `crates/forge-application/src/use_cases/mod.rs`
- Modify: `crates/forge-application/src/lib.rs`

**Interfaces:**
- Consumes: `forge_ports::{ConfigRepositoryPort, ConfigError, MockConfigRepositoryPort}` (Task 4), `forge_domain::AppConfig` (Task 3)
- Produces: `forge_application::{ToggleSkillUseCase, TogglePluginUseCase, ToggleMcpServerUseCase}`

- [ ] **Step 1: Write the three use-cases and their tests**

`crates/forge-application/src/use_cases/toggles.rs`:
```rust
use std::sync::Arc;

use forge_domain::AppConfig;
use forge_ports::{ConfigError, ConfigRepositoryPort};

fn toggle_membership(list: &mut Vec<String>, name: &str) {
    if let Some(pos) = list.iter().position(|existing| existing == name) {
        list.remove(pos);
    } else {
        list.push(name.to_string());
    }
}

pub struct ToggleSkillUseCase {
    config: Arc<dyn ConfigRepositoryPort>,
}

impl ToggleSkillUseCase {
    pub fn new(config: Arc<dyn ConfigRepositoryPort>) -> Self {
        Self { config }
    }

    pub async fn execute(&self, name: &str) -> Result<AppConfig, ConfigError> {
        let mut config = self.config.load().await?;
        toggle_membership(&mut config.enabled_skills, name);
        self.config.save(&config).await?;
        Ok(config)
    }
}

pub struct TogglePluginUseCase {
    config: Arc<dyn ConfigRepositoryPort>,
}

impl TogglePluginUseCase {
    pub fn new(config: Arc<dyn ConfigRepositoryPort>) -> Self {
        Self { config }
    }

    pub async fn execute(&self, name: &str) -> Result<AppConfig, ConfigError> {
        let mut config = self.config.load().await?;
        toggle_membership(&mut config.enabled_plugins, name);
        self.config.save(&config).await?;
        Ok(config)
    }
}

pub struct ToggleMcpServerUseCase {
    config: Arc<dyn ConfigRepositoryPort>,
}

impl ToggleMcpServerUseCase {
    pub fn new(config: Arc<dyn ConfigRepositoryPort>) -> Self {
        Self { config }
    }

    pub async fn execute(&self, name: &str) -> Result<AppConfig, ConfigError> {
        let mut config = self.config.load().await?;
        toggle_membership(&mut config.enabled_mcp_servers, name);
        self.config.save(&config).await?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_ports::MockConfigRepositoryPort;

    #[tokio::test]
    async fn toggle_skill_use_case_adds_a_skill_that_was_not_enabled() {
        let mut mock = MockConfigRepositoryPort::new();
        mock.expect_load().times(1).returning(|| Ok(AppConfig::default()));
        mock.expect_save()
            .withf(|c: &AppConfig| c.enabled_skills == vec!["brainstorming".to_string()])
            .times(1)
            .returning(|_| Ok(()));

        let use_case = ToggleSkillUseCase::new(Arc::new(mock));
        let result = use_case.execute("brainstorming").await.unwrap();
        assert_eq!(result.enabled_skills, vec!["brainstorming".to_string()]);
    }

    #[tokio::test]
    async fn toggle_skill_use_case_removes_a_skill_that_was_already_enabled() {
        let mut mock = MockConfigRepositoryPort::new();
        mock.expect_load().times(1).returning(|| {
            Ok(AppConfig {
                enabled_skills: vec!["brainstorming".to_string()],
                ..AppConfig::default()
            })
        });
        mock.expect_save()
            .withf(|c: &AppConfig| c.enabled_skills.is_empty())
            .times(1)
            .returning(|_| Ok(()));

        let use_case = ToggleSkillUseCase::new(Arc::new(mock));
        let result = use_case.execute("brainstorming").await.unwrap();
        assert!(result.enabled_skills.is_empty());
    }

    #[tokio::test]
    async fn toggle_plugin_use_case_only_touches_enabled_plugins() {
        let mut mock = MockConfigRepositoryPort::new();
        mock.expect_load().times(1).returning(|| Ok(AppConfig::default()));
        mock.expect_save()
            .withf(|c: &AppConfig| {
                c.enabled_plugins == vec!["playwright".to_string()] && c.enabled_skills.is_empty()
            })
            .times(1)
            .returning(|_| Ok(()));

        let use_case = TogglePluginUseCase::new(Arc::new(mock));
        let result = use_case.execute("playwright").await.unwrap();
        assert_eq!(result.enabled_plugins, vec!["playwright".to_string()]);
    }

    #[tokio::test]
    async fn toggle_mcp_server_use_case_only_touches_enabled_mcp_servers() {
        let mut mock = MockConfigRepositoryPort::new();
        mock.expect_load().times(1).returning(|| Ok(AppConfig::default()));
        mock.expect_save()
            .withf(|c: &AppConfig| c.enabled_mcp_servers == vec!["playwright-mcp".to_string()])
            .times(1)
            .returning(|_| Ok(()));

        let use_case = ToggleMcpServerUseCase::new(Arc::new(mock));
        let result = use_case.execute("playwright-mcp").await.unwrap();
        assert_eq!(result.enabled_mcp_servers, vec!["playwright-mcp".to_string()]);
    }

    #[tokio::test]
    async fn toggle_skill_use_case_propagates_a_load_error_without_calling_save() {
        let mut mock = MockConfigRepositoryPort::new();
        mock.expect_load()
            .times(1)
            .returning(|| Err(ConfigError::ReadFailed("disk error".to_string())));
        mock.expect_save().times(0);

        let use_case = ToggleSkillUseCase::new(Arc::new(mock));
        assert!(use_case.execute("brainstorming").await.is_err());
    }

    #[tokio::test]
    async fn toggle_skill_use_case_propagates_a_save_error() {
        let mut mock = MockConfigRepositoryPort::new();
        mock.expect_load().times(1).returning(|| Ok(AppConfig::default()));
        mock.expect_save()
            .times(1)
            .returning(|_| Err(ConfigError::WriteFailed("disk full".to_string())));

        let use_case = ToggleSkillUseCase::new(Arc::new(mock));
        assert!(use_case.execute("brainstorming").await.is_err());
    }
}
```

- [ ] **Step 2: Update `use_cases/mod.rs`**

`crates/forge-application/src/use_cases/mod.rs`:
```rust
mod session_lifecycle;
mod sidecar_commands;
mod toggles;

pub use session_lifecycle::{ResumeSessionError, ResumeSessionUseCase, StartSessionUseCase};
pub use sidecar_commands::{ApprovePermissionUseCase, InterruptTurnUseCase, SendMessageUseCase};
pub use toggles::{ToggleMcpServerUseCase, TogglePluginUseCase, ToggleSkillUseCase};
```

- [ ] **Step 3: Update `lib.rs`**

`crates/forge-application/src/lib.rs`:
```rust
mod event;
mod use_cases;

pub use event::{apply_sidecar_event, SessionUpdate};
pub use use_cases::{
    ApprovePermissionUseCase, InterruptTurnUseCase, ResumeSessionError, ResumeSessionUseCase,
    SendMessageUseCase, StartSessionUseCase, ToggleMcpServerUseCase, TogglePluginUseCase,
    ToggleSkillUseCase,
};
```

- [ ] **Step 4: Run the full workspace test suite**

Run: `cargo test --workspace`
Expected: every test in the workspace passes — `forge-domain` (10), `forge-ports` (6), `forge-application` (26), plus Plan 1's `forge-adapter-sidecar` tests (7: 2 lib + 5 contract) — 49 total.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-application/src/use_cases crates/forge-application/src/lib.rs
git commit -m "feat: add Toggle Skill/Plugin/McpServer use-cases"
```

---

## Self-Review Notes

**Spec coverage:** §12.1 (mocks feature) → Task 1. §12.2 (domain types) → Tasks 2–3. §12.3 (port traits + credential-injection note) → Tasks 4–6. §12.4 (event application) → Task 7. §12.5 (8 use-cases) → Tasks 8–10. §12.6 (testing approach) → satisfied throughout (mockall for use-cases, plain tests for pure logic). Nothing in §12 is left uncovered.

**Type consistency check:** `SessionId`, `Session`, `Message`, `MessageRole`, `ToolCall`, `ToolResult`, `PermissionRequest` (Task 2) are reused verbatim by name in Tasks 7 and 9. `AppConfig` (Task 3) is reused verbatim in Tasks 4 and 10. `ConfigRepositoryPort`/`ConfigError` (Task 4), `SessionRepositoryPort`/`SessionRepositoryError` (Task 5) are reused verbatim in Tasks 9–10. No renames across tasks.

**Review Focus:** all five items are each pinned to a specific test added above (three in Task 7, one in Task 9, one in Task 10).
