# Forge Adapter Config — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `forge-adapter-config` — the first concrete adapter — implementing `ConfigRepositoryPort` via real file I/O against Forge's own private `config.json`, completing the hexagonal core's only fully-specified adapter (the other pending adapter, `forge-adapter-claude-fs`, needs its own plan given its much larger real-world-format surface).

**Architecture:** A single new crate, `forge-adapter-config`, depending on `forge-domain` + `forge-ports` (implementing `ConfigRepositoryPort`) plus `tokio` (async file I/O) and `directories` (OS-appropriate config path resolution). The adapter itself takes an explicit file path at construction time — it never resolves the OS config directory internally — so it's trivially testable against a temp directory. A separate free function, `default_config_path()`, resolves the real OS-appropriate location for whoever constructs the adapter in production (a later plan's composition root).

**Tech Stack:** Rust (`tokio` with the `fs` feature, `directories`, `async-trait`, `serde_json`, plus `tempfile` as a new dev-dependency for filesystem-based tests, not previously used in this workspace).

**Spec:** `docs/superpowers/specs/2026-09-27-forge-design.md` (§13, "Detailed Design: `forge-adapter-config`")

## Global Constraints

- The adapter never resolves its own file path — it's always constructed with an explicit `PathBuf`, so tests never touch the real OS config location (spec §13.2).
- `load()` on a missing config file returns `Ok(AppConfig::default())`, not an error — a config file that doesn't exist yet is normal on first run, not a failure (spec §13.2).
- Use `tokio::fs`, not `std::fs`, for all file I/O (spec §13.2).
- No atomic-write-via-temp-file for `save()` — explicitly out of scope for this plan (spec §13.2).
- App name is "Forge" — no "Claude" in the name (spec §11).

## Review Focus

- `save()` must create missing parent directories before writing, not fail when the OS config directory doesn't exist yet on a fresh install — covered by Task 2's `save_creates_missing_parent_directories`.
- Malformed JSON already on disk (e.g. from manual corruption, a truncated write, or any invalid syntax) must produce `ConfigError::ParseFailed`, not panic or silently return a default — covered by Task 1's `load_returns_parse_failed_for_malformed_json`. (Note: a config file with an *unknown but syntactically valid* extra field is NOT covered by this — `AppConfig` has no `deny_unknown_fields`, so serde silently ignores unrecognized fields rather than erroring. This is fine for now; if `AppConfig` ever needs strict versioning, revisit.)
- A `load()` immediately after a `save()` of a non-default config (all five fields populated, not just one) must round-trip every field exactly — covered by Task 2's `save_then_load_round_trips_a_fully_populated_config`, not just a partially-populated one that could hide a forgotten field.

---

### Task 1: Crate scaffold + `load()`

**Files:**
- Create: `crates/forge-adapter-config/Cargo.toml`
- Create: `crates/forge-adapter-config/src/lib.rs`

**Interfaces:**
- Consumes: `forge_domain::AppConfig` (Plan 2), `forge_ports::{ConfigError, ConfigRepositoryPort}` (Plan 2)
- Produces: `forge_adapter_config::{ConfigFileAdapter, default_config_path}` — `ConfigFileAdapter::new(path: PathBuf) -> Self`, implementing `ConfigRepositoryPort`; this task implements `load()` only (`save()` is Task 2)

- [ ] **Step 1: Create the crate**

`crates/forge-adapter-config/Cargo.toml`:
```toml
[package]
name = "forge-adapter-config"
version.workspace = true
edition.workspace = true

[dependencies]
forge-domain = { path = "../forge-domain" }
forge-ports = { path = "../forge-ports" }
async-trait = "0.1"
serde_json = "1"
tokio = { version = "1", features = ["fs"] }
directories = "6.0.0"

[dev-dependencies]
tokio = { version = "1", features = ["fs", "macros", "rt"] }
tempfile = "3"
```

- [ ] **Step 2: Write the failing tests**

`crates/forge-adapter-config/src/lib.rs`:
```rust
use std::path::PathBuf;

use async_trait::async_trait;
use forge_domain::AppConfig;
use forge_ports::{ConfigError, ConfigRepositoryPort};

pub fn default_config_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("", "", "Forge")
        .map(|dirs| dirs.config_dir().join("config.json"))
}

pub struct ConfigFileAdapter {
    path: PathBuf,
}

impl ConfigFileAdapter {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

#[async_trait]
impl ConfigRepositoryPort for ConfigFileAdapter {
    async fn load(&self) -> Result<AppConfig, ConfigError> {
        match tokio::fs::read_to_string(&self.path).await {
            Ok(contents) => {
                serde_json::from_str(&contents).map_err(|e| ConfigError::ParseFailed(e.to_string()))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(AppConfig::default()),
            Err(e) => Err(ConfigError::ReadFailed(e.to_string())),
        }
    }

    async fn save(&self, _config: &AppConfig) -> Result<(), ConfigError> {
        unimplemented!("implemented in Task 2")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn load_returns_default_config_when_file_does_not_exist() {
        let dir = tempfile::tempdir().unwrap();
        let adapter = ConfigFileAdapter::new(dir.path().join("does-not-exist.json"));

        let config = adapter.load().await.unwrap();
        assert_eq!(config, AppConfig::default());
    }

    #[tokio::test]
    async fn load_returns_parse_failed_for_malformed_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        tokio::fs::write(&path, "not valid json").await.unwrap();
        let adapter = ConfigFileAdapter::new(path);

        let result = adapter.load().await;
        assert!(matches!(result, Err(ConfigError::ParseFailed(_))));
    }

    #[tokio::test]
    async fn load_parses_an_existing_valid_config_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        tokio::fs::write(
            &path,
            r#"{"base_url":"https://api.anthropic.com","api_key":"sk-ant-test","enabled_skills":["brainstorming"],"enabled_plugins":[],"enabled_mcp_servers":[]}"#,
        )
        .await
        .unwrap();
        let adapter = ConfigFileAdapter::new(path);

        let config = adapter.load().await.unwrap();
        assert_eq!(config.base_url.as_deref(), Some("https://api.anthropic.com"));
        assert_eq!(config.enabled_skills, vec!["brainstorming".to_string()]);
    }

    #[test]
    fn default_config_path_ends_with_forge_config_json() {
        let path = default_config_path().expect("a config dir should be resolvable on this platform");
        assert_eq!(path.file_name().unwrap(), "config.json");
        assert!(path.to_string_lossy().contains("Forge") || path.to_string_lossy().contains("forge"));
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run (remember to `source "$HOME/.cargo/env"` first, or prefix with `PATH="$HOME/.cargo/bin:$PATH"` if your shell tool refuses `source`): `cargo test -p forge-adapter-config`
Expected: FAIL to compile — `save()`'s `unimplemented!()` is fine (it compiles, just panics if called, and no test calls it yet), but this is a from-scratch crate, so this step confirms the crate builds and the 4 tests you just wrote actually run and pass, since the real logic (`load()`) is already written alongside its tests (no separate empty-stub-first step makes sense here — `load()` is simple enough to write correctly the first time, matching how Plan 1/2 handled small, fully-specified pure/adapter logic).

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p forge-adapter-config`
Expected: all 4 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-adapter-config
git commit -m "feat: scaffold forge-adapter-config with ConfigRepositoryPort::load"
```

---

### Task 2: `save()` + round-trip

**Files:**
- Modify: `crates/forge-adapter-config/src/lib.rs`

**Interfaces:**
- Consumes: nothing new
- Produces: a complete `ConfigFileAdapter` implementing both `load()` and `save()` — this is the plan's final deliverable; no later task consumes it (the composition root that will use it is a future plan)

- [ ] **Step 1: Write the failing tests**

Add these tests to `crates/forge-adapter-config/src/lib.rs`'s existing `#[cfg(test)] mod tests` block:

```rust
    #[tokio::test]
    async fn save_then_load_round_trips_a_fully_populated_config() {
        let dir = tempfile::tempdir().unwrap();
        let adapter = ConfigFileAdapter::new(dir.path().join("config.json"));
        let config = AppConfig {
            base_url: Some("https://api.anthropic.com".to_string()),
            api_key: Some("sk-ant-test".to_string()),
            enabled_skills: vec!["brainstorming".to_string()],
            enabled_plugins: vec!["playwright@marketplace".to_string()],
            enabled_mcp_servers: vec!["playwright-mcp".to_string()],
        };

        adapter.save(&config).await.unwrap();
        let loaded = adapter.load().await.unwrap();

        assert_eq!(loaded, config);
    }

    #[tokio::test]
    async fn save_creates_missing_parent_directories() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("does").join("not").join("exist").join("config.json");
        let adapter = ConfigFileAdapter::new(path.clone());

        adapter.save(&AppConfig::default()).await.unwrap();

        assert!(path.exists());
    }

    #[tokio::test]
    async fn save_overwrites_an_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let adapter = ConfigFileAdapter::new(dir.path().join("config.json"));

        adapter
            .save(&AppConfig {
                enabled_skills: vec!["old-skill".to_string()],
                ..AppConfig::default()
            })
            .await
            .unwrap();
        adapter
            .save(&AppConfig {
                enabled_skills: vec!["new-skill".to_string()],
                ..AppConfig::default()
            })
            .await
            .unwrap();

        let loaded = adapter.load().await.unwrap();
        assert_eq!(loaded.enabled_skills, vec!["new-skill".to_string()]);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p forge-adapter-config`
Expected: FAIL — `save()` currently panics (`unimplemented!()`).

- [ ] **Step 3: Implement `save()`**

Replace the `save` method's body in `crates/forge-adapter-config/src/lib.rs`:
```rust
    async fn save(&self, config: &AppConfig) -> Result<(), ConfigError> {
        if let Some(parent) = self.path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| ConfigError::WriteFailed(e.to_string()))?;
        }
        let json = serde_json::to_string_pretty(config)
            .map_err(|e| ConfigError::WriteFailed(e.to_string()))?;
        tokio::fs::write(&self.path, json)
            .await
            .map_err(|e| ConfigError::WriteFailed(e.to_string()))
    }
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p forge-adapter-config`
Expected: all 7 tests pass (4 from Task 1 + 3 new).

- [ ] **Step 5: Run the full workspace test suite**

Run: `cargo test --workspace`
Expected: every test in the workspace passes — `forge-domain` (10), `forge-ports` (6), `forge-application` (29), `forge-adapter-sidecar` (7), `forge-adapter-config` (7) — 59 total.

- [ ] **Step 6: Commit**

```bash
git add crates/forge-adapter-config/src/lib.rs
git commit -m "feat: implement ConfigRepositoryPort::save with parent-directory creation"
```

---

## Self-Review Notes

**Spec coverage:** §13.2 (design: explicit-path adapter, missing-file defaults, tokio::fs, no atomic write) → Tasks 1–2. §13.3 (testing: missing-file default, round-trip, parent-dir creation, malformed JSON) → all four covered across Task 1's 4 tests and Task 2's 3 tests.

**Type consistency check:** `ConfigFileAdapter`, `default_config_path` defined once in Task 1, extended (not renamed) in Task 2. `AppConfig`, `ConfigError`, `ConfigRepositoryPort` reused verbatim from Plan 2 — no redefinition.

**Review Focus:** all three items are each pinned to a specific test (two in Task 1/2 already listed above, the parent-directory one in Task 2).
